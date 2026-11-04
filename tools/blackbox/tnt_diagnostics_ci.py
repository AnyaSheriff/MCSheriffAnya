#!/usr/bin/env python3
"""Малый CI-only smoke диагностики; не проверка parity или производительности."""
import argparse
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import time

from combat_ci import ROOT, preflight, resolve_pin, sha, verify_artifact, write_json
from combat_process import group_exists, ports_free, process_identity, stop_process

PIN = ROOT / 'tools/blackbox/ci/tnt-diagnostics-pin.json'
PREFIX = 'MCSA_TNT_DIAGNOSTICS '
PORTS = [26150, 26151]


def records(text):
    result = []
    for line in text.splitlines():
        if PREFIX in line:
            result.append(json.loads(line.split(PREFIX, 1)[1]))
    return result


def validate(enabled, rows):
    if not enabled:
        if rows:
            raise ValueError('disabled diagnostics emitted records')
        return
    if len(rows) < 2 or len(rows) > 4:
        raise ValueError('expected two to four sampled steps')
    prior_tick, prior_fuse, entity_id = None, None, None
    for row in rows:
        tick = row['tick']
        if type(tick) is not int or tick % 20 or (prior_tick is not None and tick != prior_tick + 20):
            raise ValueError('invalid sampled tick sequence')
        if row['dimension'] != 'minecraft:overworld' or row['sampling'] != 'one_step_every_20_ticks':
            raise ValueError('wrong dimension/sampling')
        if row['budget_exhausted'] is not False or row['exploding'] != 0:
            raise ValueError('unexpected exhaustion/explosion')
        for key in ('world_lock_wait_us', 'item_stage_elapsed_us'):
            if type(row[key]) is not int or row[key] < 0:
                raise ValueError('invalid timing')
        samples = []
        for stage in ('before', 'after_step'):
            snapshot = row[stage]
            if (snapshot['total'], snapshot['loaded'], snapshot['omitted'], len(snapshot['samples'])) != (1, 1, 0, 1):
                raise ValueError('expected one authoritative loaded TNT')
            sample = snapshot['samples'][0]
            if type(sample['id']) is not int or type(sample['fuse']) is not int or not 1 <= sample['fuse'] <= 80:
                raise ValueError('invalid TNT identity/fuse')
            if sample['loaded'] is not True or sample['chunk'] != [0, 0]:
                raise ValueError('wrong loaded chunk')
            for key in ('position', 'velocity'):
                if len(sample[key]) != 3 or any(type(v) not in (int, float) or not math.isfinite(v) for v in sample[key]):
                    raise ValueError('invalid vector')
            if [math.floor(sample['position'][i]) // 16 for i in (0, 2)] != sample['chunk']:
                raise ValueError('position/chunk mismatch')
            samples.append(sample)
        before, after = samples
        entity_id = before['id'] if entity_id is None else entity_id
        if before['id'] != entity_id or after['id'] != entity_id or after['fuse'] != before['fuse']:
            raise ValueError('identity/stage fuse mismatch')
        if row['sampled_fuses_after_burn'] != [[entity_id, before['fuse'] - 1]]:
            raise ValueError('burn did not decrement exactly once')
        if prior_fuse is not None and before['fuse'] != prior_fuse - 20:
            raise ValueError('fuse did not follow sampled ticks')
        prior_tick, prior_fuse = tick, before['fuse']


def clean_shutdown(proc, identity):
    result = stop_process(proc, identity=identity)
    if (result['method'] != 'stop-and-eof' or result['exit_code'] != 0
            or result['group_gone'] is not True):
        raise RuntimeError('unclean stop/EOF or surviving owned group: ' + str(result))
    return result


def experiment(runtime, output, enabled, port):
    folder = output / ('enabled' if enabled else 'disabled')
    folder.mkdir()
    config = folder / 'config'
    config.mkdir()
    (config / 'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    (config / 'skins.toml').write_text('mojang=false\nely=false\n')
    (config / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nenable-rcon=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\ngenerator-settings={{"layers":[{{"block":"minecraft:bedrock","height":1}},{{"block":"minecraft:stone","height":3}}],"biome":"minecraft:plains"}}\ngenerate-structures=false\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\npause-when-empty-seconds=0\n')
    env = dict(os.environ)
    env.pop('MCSA_TNT_DIAGNOSTICS', None)
    env.pop('GH_TOKEN', None)
    env.pop('GITHUB_TOKEN', None)
    if enabled:
        env['MCSA_TNT_DIAGNOSTICS'] = '1'
    logpath = folder / 'console.log'
    transcript = []
    proc = None
    identity = None
    clean = False
    receipt = {'enabled': enabled, 'port': port, 'runtime_sha256': sha(runtime),
               'config_sha256_before_launch': {p.name: sha(p) for p in config.iterdir()}}
    def wait(needle, offset=0):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if logpath.stat().st_size > 4 * 1024 * 1024:
                raise ValueError('console log exceeds smoke bound')
            text = logpath.read_text(errors='replace')
            if needle in text[offset:]:
                return
            if proc.poll() is not None:
                raise RuntimeError('runtime exited before console acknowledgment')
            time.sleep(0.05)
        raise TimeoutError('console acknowledgment: ' + needle)
    def command(text, acknowledgment):
        offset = len(logpath.read_text(errors='replace'))
        transcript.append(text)
        proc.stdin.write(text + '\n')
        proc.stdin.flush()
        wait(acknowledgment, offset)
    try:
        with logpath.open('w') as log:
            proc = subprocess.Popen([str(runtime)], cwd=folder, env=env, stdin=subprocess.PIPE,
                                    stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
            identity = process_identity(proc.pid)
            wait('Готово')
            command('tick freeze', 'The game is frozen')
            command('forceload add 0 0', 'to be force loaded')
            command('setblock 8 120 8 minecraft:tnt', 'Changed the block at 8, 120, 8')
            command('setblock 9 120 8 minecraft:redstone_block', 'Changed the block at 9, 120, 8')
            command('tick sprint 40', 'Sprint completed with')
            transcript.append('stop')
            receipt['shutdown'] = clean_shutdown(proc, identity)
            receipt['exit'] = proc.returncode
            clean = True
        rows = records(logpath.read_text())
        validate(enabled, rows)
        receipt['records'] = rows
        receipt['status'] = 'diagnostics-validated' if enabled else 'disabled-no-output-console-smoke'
        receipt['sampled_fuse_transition_verified'] = enabled
    finally:
        cleanup_error = None
        try:
            if proc is not None:
                receipt['final_cleanup'] = stop_process(proc, identity=identity)
                receipt['owned_group_gone'] = not group_exists(proc.pid)
                if not receipt['owned_group_gone'] or not receipt['final_cleanup']['group_gone']:
                    raise RuntimeError('owned process group survived cleanup')
                if receipt['final_cleanup']['method'] not in ('already-exited', 'stop-and-eof'):
                    clean = False
        except Exception as error:
            cleanup_error = str(error)
            receipt['cleanup_error'] = cleanup_error
            clean = False
        receipt['clean_exit'] = clean
        receipt['config_sha256_after_shutdown'] = {p.name: sha(p) for p in config.iterdir() if p.is_file()}
        receipt['ports_free'] = ports_free([port])
        receipt['commands'] = transcript
        if cleanup_error or not clean or not all(receipt['ports_free'].values()):
            receipt['status'] = 'infrastructure-failed'
        write_json(folder / 'receipt.json', receipt)
        if cleanup_error:
            raise RuntimeError('cleanup failed: ' + cleanup_error)
    if not clean or not all(receipt['ports_free'].values()):
        raise RuntimeError('runtime cleanup or port check failed')
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('pin', 'run'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--artifact', type=Path)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('GitHub-only; no local runtime execution')
    args.output.mkdir(parents=True, exist_ok=True)
    def expired(signum, frame):
        raise TimeoutError('100s smoke deadline')
    for sig in (signal.SIGALRM, signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, expired)
    signal.alarm(100)
    try:
        if args.mode == 'pin':
            pin = resolve_pin(json.loads(PIN.read_text()))
            selection = preflight(pin, args.output)
            with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
                stream.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={selection["artifact_name"]}\n')
            return
        selection = json.loads((args.output / 'build-selection.json').read_text())
        if selection['pin'] != resolve_pin(json.loads(PIN.read_text())):
            raise ValueError('selection differs from checked pin')
        runtime, receipt = verify_artifact(args.artifact, selection)
        runtime.chmod(0o700)
        if not all(ports_free(PORTS).values()):
            raise RuntimeError('dedicated ports occupied before start')
        result = {'scope': 'diagnostic-console-smoke-only', 'selection': selection,
                  'build_receipt_sha256': sha(receipt),
                  'source_sha256': {str(p.relative_to(ROOT)): sha(p) for p in [Path(__file__),
                      ROOT / 'src/tick.rs', ROOT / 'src/tick/tnt_diagnostics.rs', PIN,
                      ROOT / 'tools/blackbox/combat_ci.py', ROOT / 'tools/blackbox/combat_process.py',
                      ROOT / 'tools/blackbox/test_tnt_diagnostics_ci.py',
                      ROOT / '.github/workflows/tnt-diagnostics.yml']}, 'runs': []}
        for enabled, port in zip((True, False), PORTS):
            result['runs'].append(experiment(runtime.resolve(), args.output.resolve(), enabled, port))
        result['status'] = 'smoke-pass'
        write_json(args.output / 'receipt.json', result)
    except Exception as error:
        write_json(args.output / 'failure.json', {'error': type(error).__name__ + ': ' + str(error)})
        raise SystemExit(1)
    finally:
        signal.alarm(0)


if __name__ == '__main__':
    main()
