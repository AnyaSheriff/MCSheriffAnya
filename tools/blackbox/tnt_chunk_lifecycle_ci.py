#!/usr/bin/env python3
"""Характеризация TNT нашего ядра; не vanilla parity и не причина остатков 8000 TNT."""
import argparse
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import time

from combat_ci import ROOT, preflight, resolve_pin, sha, verify_artifact, write_json
from combat_process import group_exists, ports_free, process_identity, stop_process
from tnt_diagnostics_ci import clean_shutdown, records

PIN = ROOT / 'tools/blackbox/ci/tnt-diagnostics-pin.json'
PORT = 26152


def aligned(due, observed):
    # Запас до зажигания: не допускаем исполнения до проверки/снятия расписания.
    return type(due) is int and 61 <= due % 100 <= 75 and due - observed >= 80


def schedule_due(text, name, delay):
    match = re.search(r'Функция «' + re.escape(name) + r'» запланирована через '
                      + str(delay) + r' тактов, на такт (\d+)', text)
    if not match:
        raise ValueError('missing exact schedule acknowledgment')
    return int(match[1])


def validate(rows, prime_due, complete=True):
    if not 61 <= prime_due % 100 <= 75 or not (5 if complete else 1) <= len(rows) <= 9:
        raise ValueError('phase or sampled count invalid')
    unload_due = (prime_due // 100 + 1) * 100
    reload_due = prime_due + 100
    prior = None
    identity = None
    frozen = []
    loaded_before = loaded_after = 0
    for row in rows:
        tick = row['tick']
        if type(tick) is not int or tick % 20 or tick <= prime_due:
            raise ValueError('invalid sampled tick')
        if prior is not None and tick != prior['tick'] + 20:
            raise ValueError('sampled gap or stale record')
        if row['dimension'] != 'minecraft:overworld' or row['sampling'] != 'one_step_every_20_ticks':
            raise ValueError('wrong dimension or sampling')
        if row['budget_exhausted'] is not False or row['exploding'] != 0:
            raise ValueError('exhaustion or explosion before proof')
        expected_loaded = tick <= unload_due or tick >= reload_due
        pair = []
        for stage in ('before', 'after_step'):
            snapshot = row[stage]
            if (snapshot['total'], snapshot['loaded'], snapshot['omitted'], len(snapshot['samples'])) != (1, int(expected_loaded), 0, 1):
                raise ValueError('missing, omitted or incorrectly loaded TNT')
            sample = snapshot['samples'][0]
            if type(sample['id']) is not int or type(sample['fuse']) is not int or not 1 <= sample['fuse'] <= 80:
                raise ValueError('invalid identity/fuse')
            identity = sample['id'] if identity is None else identity
            if sample['id'] != identity or sample['loaded'] is not expected_loaded or sample['chunk'] != [0, 0]:
                raise ValueError('changed identity or chunk')
            for key in ('position', 'velocity'):
                if len(sample[key]) != 3 or any(type(v) not in (int, float) or not math.isfinite(v) for v in sample[key]):
                    raise ValueError('invalid vector')
            if [math.floor(sample['position'][i]) // 16 for i in (0, 2)] != [0, 0]:
                raise ValueError('position outside sampled chunk')
            pair.append(sample)
        before, after = pair
        if after['fuse'] != before['fuse']:
            raise ValueError('step changed fuse')
        after_fuse = before['fuse'] - int(expected_loaded)
        if row['sampled_fuses_after_burn'] != [[identity, after_fuse]]:
            raise ValueError('burn contradicts loaded flag')
        if not expected_loaded:
            if before != after or (frozen and before != frozen[-1]):
                raise ValueError('unloaded TNT moved or fuse changed')
            frozen.append(before)
        elif tick <= unload_due:
            loaded_before += 1
        else:
            loaded_after += 1
        if prior is not None:
            # Выгрузка после burn на unload_due, reload до burn на reload_due.
            active = sum(t <= unload_due or t >= reload_due for t in range(prior['tick'], tick))
            if before['fuse'] != prior['before']['samples'][0]['fuse'] - active:
                raise ValueError('fuse contradicts active world ticks')
        prior = row
    if complete and (not loaded_before or len(frozen) < 2 or not loaded_after):
        raise ValueError('incomplete loaded-unloaded-reloaded proof')
    return {'id': identity, 'unload_due': unload_due, 'reload_due': reload_due,
            'unloaded_samples': len(frozen), 'loaded_before': loaded_before, 'loaded_after': loaded_after}


def experiment(runtime, output):
    folder = output / 'lifecycle'
    folder.mkdir()
    config = folder / 'config'
    config.mkdir()
    (config / 'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    (config / 'skins.toml').write_text('mojang=false\nely=false\n')
    (config / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={PORT}\nonline-mode=false\nenable-rcon=false\nnetwork-compression-threshold=-1\nlevel-name=world\nlevel-type=minecraft:flat\ngenerator-settings={{"layers":[{{"block":"minecraft:bedrock","height":1}},{{"block":"minecraft:stone","height":3}}],"biome":"minecraft:plains"}}\ngenerate-structures=false\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\npause-when-empty-seconds=0\n')
    fixture = folder / 'world/datapacks/probe/data/probe/function'
    fixture.mkdir(parents=True)
    (fixture / 'noop.mcfunction').write_text('# Собственная пустая функция для чтения глобального due.\n')
    (fixture / 'prime.mcfunction').write_text('setblock 8 5 8 minecraft:tnt\nsetblock 9 5 8 minecraft:redstone_block\nforceload remove 0 0\nschedule function probe:reload 100t\n')
    (fixture / 'reload.mcfunction').write_text('forceload add 0 0\n')
    shutil.copytree(config, folder / 'config-before')
    receipt = {'scope': 'own-core-characterization-not-vanilla-or-8000-causality',
               'port': PORT, 'runtime_sha256': sha(runtime), 'commands': [], 'alignment_attempts': [],
               'fixture_sha256': {str(p.relative_to(folder)): sha(p) for p in fixture.iterdir()},
               'config_sha256_before_launch': {p.name: sha(p) for p in config.iterdir()}}
    env = dict(os.environ)
    for key in ('GH_TOKEN', 'GITHUB_TOKEN'):
        env.pop(key, None)
    env['MCSA_TNT_DIAGNOSTICS'] = '1'
    logpath = folder / 'console.log'
    proc = identity = None
    clean = False
    deadline = time.monotonic() + 80

    def read():
        if time.monotonic() > deadline:
            raise TimeoutError('80s experiment deadline')
        if logpath.stat().st_size > 4 * 1024 * 1024:
            raise ValueError('console exceeds bounded size')
        return logpath.read_text(errors='replace')

    def wait(needle, offset=0):
        until = min(deadline, time.monotonic() + 20)
        while time.monotonic() < until:
            text = read()
            if needle in text[offset:]:
                return text[offset:]
            if proc.poll() is not None:
                raise RuntimeError('runtime exited before acknowledgment')
            time.sleep(0.025)
        raise TimeoutError('acknowledgment: ' + needle)

    def command(text, acknowledgment):
        offset = len(read())
        item = {'command': text, 'offset': offset}
        receipt['commands'].append(item)
        proc.stdin.write(text + '\n')
        proc.stdin.flush()
        answer = wait(acknowledgment, offset)
        item['acknowledgment'] = answer
        return answer

    try:
        with logpath.open('w') as log:
            proc = subprocess.Popen([str(runtime)], cwd=folder, env=env, stdin=subprocess.PIPE,
                                    stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
            identity = process_identity(proc.pid)
            receipt['process_identity'] = identity
            wait('Готово')
            command('tick freeze', 'The game is frozen')
            # При alignment даже медленное чтение ack не догонит будущий prime.
            command('tick rate 1', 'Set the target tick rate to 1.0 per second')
            command('forceload add 0 0', 'to be force loaded')
            command('fill 0 4 0 15 4 15 minecraft:stone', 'Заполнено блоков: 256')
            for attempt in range(3):
                answer = command('schedule function probe:noop 1t', 'на такт ')
                observed = schedule_due(answer, 'probe:noop', 1)
                target = (observed // 100 + 2) * 100 + 65
                delay = target - observed
                answer = command(f'schedule function probe:prime {delay}t', 'на такт ')
                actual = schedule_due(answer, 'probe:prime', delay)
                accepted = aligned(actual, observed)
                receipt['alignment_attempts'].append({'observed_due': observed, 'target': target,
                                                      'actual_due': actual, 'accepted': accepted})
                if accepted:
                    break
                # Мир всё ещё frozen: функция может выполняться, поэтому запас проверяем явно.
                if actual - observed < 80:
                    raise ValueError('unsafe alignment cancellation window')
                command('schedule clear probe:prime', 'probe:prime')
            else:
                raise ValueError('three alignment attempts rejected before ignition')
            receipt['prime_due'] = actual
            receipt['reload_due_derived_from_fixture'] = actual + 100
            command('tick rate 20', 'Set the target tick rate to 20.0 per second')
            command('tick unfreeze', 'The game is running normally')
            receipt['active_world_interval'] = 'unfreeze acknowledgment through final accepted diagnostic; no freeze/step/sprint'
            proof_until = ((actual + 100 + 19) // 20) * 20
            while True:
                rows = records(read())
                receipt['records'] = rows
                if rows:
                    validate(rows, actual, complete=False)
                if rows and rows[-1]['tick'] >= proof_until:
                    receipt['proof'] = validate(rows, actual)
                    receipt['proof_records'] = rows
                    break
                if proc.poll() is not None:
                    raise RuntimeError('runtime exited before lifecycle proof')
                time.sleep(0.025)
            receipt['shutdown'] = clean_shutdown(proc, identity)
            clean = True
            receipt['status'] = 'lifecycle-pass'
    except Exception as error:
        receipt['error'] = type(error).__name__ + ': ' + str(error)
        receipt['status'] = 'failed'
        raise
    finally:
        try:
            if proc is not None:
                receipt['final_cleanup'] = stop_process(proc, identity=identity)
                receipt['owned_group_gone'] = not group_exists(proc.pid)
                if not receipt['owned_group_gone'] or not receipt['final_cleanup']['group_gone']:
                    raise RuntimeError('owned process group survived')
                if receipt['final_cleanup']['method'] not in ('already-exited', 'stop-and-eof'):
                    clean = False
        except Exception as error:
            clean = False
            receipt['cleanup_error'] = str(error)
        receipt['clean_exit'] = clean
        receipt['ports_free'] = ports_free([PORT])
        receipt['config_sha256_after_shutdown'] = {p.name: sha(p) for p in config.iterdir() if p.is_file()}
        shutil.copytree(config, folder / 'config-after')
        if logpath.exists():
            try:
                receipt['records'] = records(logpath.read_text(errors='replace'))
            except Exception as error:
                receipt['record_error'] = str(error)
                clean = False
        if not clean or not all(receipt['ports_free'].values()):
            receipt['status'] = 'infrastructure-failed'
        write_json(folder / 'receipt.json', receipt)
    if receipt['status'] != 'lifecycle-pass':
        raise RuntimeError('lifecycle/cleanup failed')
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
        raise TimeoutError('105s lifecycle deadline')
    for sig in (signal.SIGALRM, signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, expired)
    signal.alarm(105)
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
        if not all(ports_free([PORT]).values()):
            raise RuntimeError('dedicated ports occupied before start')
        result = {'scope': 'own-core-characterization-not-vanilla-or-8000-causality', 'selection': selection,
                  'build_receipt_sha256': sha(receipt),
                  'source_sha256': {str(p.relative_to(ROOT)): sha(p) for p in [Path(__file__),
                      ROOT / 'src/tick.rs', ROOT / 'src/tick/tnt_diagnostics.rs', PIN,
                      ROOT / 'tools/blackbox/tnt_diagnostics_ci.py', ROOT / 'tools/blackbox/combat_ci.py', ROOT / 'tools/blackbox/combat_process.py',
                      ROOT / 'tools/blackbox/test_tnt_chunk_lifecycle_ci.py',
                      ROOT / '.github/workflows/tnt-chunk-lifecycle.yml']}, 'runs': []}
        write_json(args.output / 'receipt.json', result)
        result['runs'].append(experiment(runtime.resolve(), args.output.resolve()))
        result['status'] = 'lifecycle-pass'
        write_json(args.output / 'receipt.json', result)
    except Exception as error:
        write_json(args.output / 'failure.json', {'error': type(error).__name__ + ': ' + str(error)})
        raise SystemExit(1)
    finally:
        signal.alarm(0)


if __name__ == '__main__':
    main()
