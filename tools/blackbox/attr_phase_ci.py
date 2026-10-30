#!/usr/bin/env python3
"""Узкий CI: три живых сценария × два повтора штатного bot.js, без сборки."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys

from combat_ci import (ROOT, checked_file, cleanup_manifest, fetch, preflight,
                       resolve_pin, run_tool, sha, verify_artifact, write_json)
from combat_process import ports_free

CONFIG = ROOT / 'tools/blackbox/ci/attr-phase'
NAMES = ['pvp-weakness', 'pvp-resistance', 'pvp-shield-back']
REPEAT = 2
PORTS = [26170, 26171, 26174, 26270, 26271, 26274]


def validate_capture(raw, name):
    if not isinstance(raw, dict) or raw.get('scenario') != name or raw.get('lane') != 1:
        raise ValueError(f'{name}: wrong capture identity')
    events = raw.get('events')
    if not isinstance(events, list) or not events or not all(isinstance(e, dict) for e in events):
        raise ValueError(f'{name}: empty/invalid capture')
    # Список команд сам по себе не доказывает отклика сервера.
    if not any(isinstance(e.get('kind'), str) and e['kind'] != 'do' for e in events):
        raise ValueError(f'{name}: capture has no server responses')
    phase = raw.get('attribute_phase')
    if not isinstance(phase, dict):
        raise ValueError(f'{name}: no final attribute phase evidence')
    players = phase.get('players')
    base, attempts = phase.get('base_tick'), phase.get('attempts')
    window = raw.get('window')
    if (not isinstance(players, list) or len(players) != 2
            or any(type(p) is not int or p not in (0, 1) for p in players)
            or players[0] != players[1] or type(base) is not int or base < 0
            or base % 2 != players[0] or type(attempts) is not int or not 0 <= attempts <= 8
            or not isinstance(window, dict) or window.get('first') != base
            or type(window.get('last')) is not int or window['last'] <= base):
        raise ValueError(f'{name}: invalid/stale/mismatched final phases/window')
    return phase


def audit(results, expected_runtime):
    errors, captures = [], []
    try:
        manifest = json.loads(checked_file(results / 'manifest.json').read_text())
        sources = json.loads((CONFIG / 'sources.json').read_text())
        if (manifest.get('primary') != '26.3' or manifest.get('scenarios') != NAMES
                or manifest.get('repeat') != REPEAT or manifest.get('translation') is not True
                or manifest.get('official_recordings') is not None
                or manifest.get('port') != 26170 or manifest.get('ours_port') != 26270
                or manifest.get('binary', {}).get('sha256') != expected_runtime
                or any(manifest.get(key, {}).get('sha256') != sources[source]['sha256']
                       for key, source in [('jar', 'official_jar'), ('proxy', 'proxy')])):
            raise ValueError('capture manifest differs from checked runtime/scope/sources')
        for name, checksum in manifest['scripts'].items():
            if sha(ROOT / 'tools/blackbox' / name) != checksum:
                raise ValueError(f'script identity mismatch: {name}')
        if not {'bot.js', 'attr-phase.js', 'compare.js', 'via_ticks.js'} <= set(manifest['scripts']):
            raise ValueError('required script hashes missing')
        rows = json.loads(checked_file(results / 'summary.json').read_text())
        expected = {(name, repeat) for name in NAMES for repeat in range(REPEAT)}
        if (not isinstance(rows, list) or len(rows) != len(expected)
                or {(r['scenario'], r['repeat']) for r in rows} != expected):
            raise ValueError('fresh comparison coverage mismatch')
        for row in rows:
            name, repeat = row['scenario'], row['repeat']
            counts = row.get('counts')
            if (row.get('exits') != {'official': 0, 'ours': 0} or row.get('compare_exit') != 0
                    or row.get('status') != 'match' or row.get('warnings') != []
                    or not isinstance(counts, list) or len(counts) != 4
                    or any(type(n) is not int or n < 0 for n in counts)
                    or counts[0] == 0 or any(counts[1:])):
                errors.append(f'{name}/{repeat}: capture/comparison failed')
            checked_file(results / f'{name}-{repeat}-compare.log')
            for side in ('official', 'ours'):
                path = checked_file(results / f'{name}-{repeat}-{side}.json')
                phase = validate_capture(json.loads(path.read_text()), name)
                checked_file(results / f'{name}-{repeat}-{side}.log')
                captures.append({'scenario': name, 'repeat': repeat, 'side': side,
                                 'sha256': sha(path), 'attribute_phase': phase})
        lifecycle = json.loads(checked_file(results / 'lifecycle.json').read_text())
        spawned = lifecycle['servers']
        cleaned = lifecycle['cleanup']
        if (len(spawned) != 4 + len(expected) * 2 or lifecycle.get('cleanup_errors')
                or len(cleaned) != len(spawned)
                or {r['pid'] for r in cleaned} != {r['pid'] for r in spawned}
                or any(not r.get('group_gone') or r.get('exit_code') is None for r in cleaned)
                or not all(lifecycle.get('ports_free', {}).get(str(p)) for p in PORTS)):
            errors.append('server/bot lifecycle incomplete')
        if any(r.get('exit_code') != 0 for r in cleaned if r['pid'] in {
                c['pid'] for c in lifecycle.get('commands', []) if c.get('argv', [''])[0] == 'node'}):
            errors.append('bot process did not exit cleanly')
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append(str(error))
    return {'status': 'match' if not errors else 'failed', 'errors': errors, 'captures': captures}


def snapshot(output):
    paths = [ROOT / '.github/workflows/attr-phase-checks.yml', ROOT / 'docs/attr-phase-ci.md']
    paths += [ROOT / 'tools/blackbox' / name for name in (
        'attr_phase_ci.py', 'test_attr_phase_ci.py', 'via_harness.py', 'via_ticks.js',
        'combat_ci.py', 'combat_process.py', 'bot.js', 'attr-phase.js', 'attr-phase.test.js',
        'compare.js', 'value-order.js', 'data/sounds.json')]
    paths += list(CONFIG.glob('*.json'))
    paths += [ROOT / f'tools/blackbox/scenarios/{name}.json' for name in NAMES]
    for path in paths:
        checked_file(path)
        dest = output / 'source' / path.relative_to(ROOT)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, dest)


def collect(output, results):
    if results.exists():
        for path in results.iterdir():
            if path.is_file() and path.suffix in ('.json', '.log'):
                shutil.copyfile(checked_file(path), output / path.name)
        for label in ('official', 'ours', 'official-proxy', 'ours-proxy'):
            folder = results / label
            # Только созданные нами настройки; никаких миров/JAR/библиотек.
            for relative in ('server.log', 'server.properties', 'ops.json', 'viabedrock.yml',
                             'config/server.properties', 'config/mcsa.properties', 'config/skins.toml'):
                path = folder / relative
                if path.is_file():
                    dest = output / 'servers' / label / relative
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(checked_file(path), dest)
    files = list(output.rglob('*'))
    if any(p.is_symlink() or p.suffix == '.jar' or {'node_modules', 'world'} & set(p.relative_to(output).parts) for p in files):
        raise ValueError('unsafe artifact contents')
    write_json(output / 'files.json', [{'path': str(p.relative_to(output)), 'bytes': p.stat().st_size,
                                      'sha256': sha(p)} for p in sorted(files)
                                     if p.is_file() and p.name != 'files.json'])


def execute(artifact, work, output, java, modules):
    if work.exists() or (output / 'ci-receipt.json').exists():
        raise ValueError('fresh work/output required; no previous PASS reuse')
    work.mkdir(parents=True)
    results = work / 'recordings'
    receipt = {'status': 'failed', 'stage': 'prepare', 'commands': [],
               'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'scenarios': NAMES, 'repeat': REPEAT}
    selection = json.loads((output / 'build-selection.json').read_text())
    expected_pin = resolve_pin(json.loads((CONFIG / 'runtime-pin.json').read_text()))
    if selection.get('pin') != expected_pin:
        raise ValueError('selection differs from checked runtime pin')
    receipt['selection'] = selection
    write_json(output / 'ci-receipt.json', receipt)
    def interrupted(sig, frame):
        raise InterruptedError(f'CI interrupted: {sig}')
    previous = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        snapshot(output)
        sources = json.loads((CONFIG / 'sources.json').read_text())
        if sources['scenarios'] != NAMES or sources['repeat'] != REPEAT:
            raise ValueError('unexpected scope in checked manifest')
        runtime, build_receipt = verify_artifact(artifact, selection)
        shutil.copyfile(build_receipt, output / 'build-receipt.json')
        binary = work / 'mcsheriffanya'
        shutil.copyfile(runtime, binary)
        binary.chmod(0o700)
        version = subprocess.check_output([java, '-version'], stderr=subprocess.STDOUT, text=True, timeout=10)
        if not re.search(r'version "25(?:[.\"]|$)', version):
            raise ValueError('Java25 required')
        receipt['java_version'] = version
        for name, command in [('node', ['node', '--version']), ('npm', ['npm', '--version'])]:
            receipt[name + '_version'] = subprocess.check_output(command, text=True, timeout=10).strip()
        installed = {}
        lock = json.loads((CONFIG / 'package-lock.json').read_text())
        for name in ('mineflayer', 'minecraft-protocol', 'minecraft-data', 'vec3'):
            metadata = json.loads(checked_file(modules / name / 'package.json').read_text())
            if metadata.get('version') != lock['packages']['node_modules/' + name]['version']:
                raise ValueError(f'installed dependency version mismatch: {name}')
            installed[name] = metadata['version']
        receipt['node_dependencies'] = installed
        receipt['dependency_lock_sha256'] = sha(CONFIG / 'package-lock.json')
        jar, proxy = work / 'server.jar', work / 'proxy.jar'
        fetch(sources['official_jar'], jar)
        fetch(sources['proxy'], proxy)
        command = [sys.executable, str(ROOT / 'tools/blackbox/via_harness.py'), '--output', str(results),
                   '--binary', str(binary), '--jar', str(jar), '--proxy', str(proxy), '--java', java,
                   '--max-seconds', '1800', '--port', '26170', '--ours-port', '26270',
                   '--official-heap', '3G', '--repeat', str(REPEAT), *NAMES]
        receipt['commands'].append(command)
        receipt['stage'] = 'record'
        write_json(output / 'ci-receipt.json', receipt)
        os.environ['MCSA_BB_NODE_MODULES'] = str(modules)
        receipt['harness_exit'] = run_tool(command, output / 'harness.log', 1850)
        receipt.update(audit(results, selection['pin']['runtime_sha256']))
        if receipt['harness_exit']:
            receipt['status'] = 'failed'
            receipt['errors'].append('nonzero harness exit')
        receipt['stage'] = 'complete'
    except Exception as error:
        receipt['status'] = 'failed'
        receipt['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        for sig in previous:
            signal.signal(sig, signal.SIG_IGN)
        try:
            receipt['watchdog_errors'] = cleanup_manifest(results)
            receipt['final_ports_free'] = ports_free(PORTS)
            if receipt['watchdog_errors'] or not all(receipt['final_ports_free'].values()):
                receipt['status'] = 'failed'
        except Exception as error:
            receipt['status'] = 'failed'
            receipt['cleanup_error'] = str(error)
        write_json(output / 'ci-receipt.json', receipt)
        try:
            collect(output, results)
        except Exception as error:
            receipt['status'] = 'failed'
            receipt['collection_error'] = str(error)
            write_json(output / 'ci-receipt.json', receipt)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('pin', 'run'))
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--work', type=Path)
    parser.add_argument('--java')
    parser.add_argument('--modules', type=Path)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('GitHub CI-only entrypoint')
    args.output.mkdir(parents=True, exist_ok=True)
    try:
        if args.mode == 'pin':
            snapshot(args.output)
            pin = resolve_pin(json.loads((CONFIG / 'runtime-pin.json').read_text()))
            selected = preflight(pin, args.output)
            with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
                output.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={selected["artifact_name"]}\n')
        else:
            if not all((args.artifact, args.work, args.java, args.modules)):
                parser.error('run requires --artifact --work --java --modules')
            result = execute(args.artifact.resolve(), args.work.resolve(), args.output.resolve(),
                             args.java, args.modules.resolve())
            return 0 if result['status'] == 'match' else 1
    except Exception as error:
        write_json(args.output / 'entrypoint-error.json', {'status': 'failed', 'error': str(error)})
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
