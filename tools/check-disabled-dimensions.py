#!/usr/bin/env python3
"""CI-only проверка запуска четырёх сочетаний измерений на собственном сервере."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
import traceback

DIMENSIONS = {'nether': 'the_nether', 'end': 'the_end'}


def tree(path):
    if not path.exists():
        return None
    return {str(p.relative_to(path)): (None if p.is_dir() else hashlib.sha256(p.read_bytes()).hexdigest())
            for p in sorted(path.rglob('*'))}


def port_available(port):
    with socket.socket() as sock:
        try:
            sock.bind(('127.0.0.1', port))
            return True
        except OSError:
            return False


def launch(binary, folder, port, phase):
    result = {'phase': phase, 'commands': [], 'ready': False, 'forced_kill': False}
    process = None
    log_path = folder / (phase + '.log')
    try:
        if not port_available(port):
            raise RuntimeError('isolated port is unavailable')
        with log_path.open('w', encoding='utf-8') as log:
            process = subprocess.Popen([str(binary)], cwd=folder, stdin=subprocess.PIPE,
                                       stdout=log, stderr=subprocess.STDOUT, text=True,
                                       start_new_session=True)
            deadline = time.monotonic() + 120
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError('server exited before ready')
                if 'Готово (' in log_path.read_text(encoding='utf-8', errors='replace'):
                    result['ready'] = True
                    break
                time.sleep(.1)
            if not result['ready']:
                raise TimeoutError('server did not report ready')
    except Exception as error:
        result['error'] = repr(error)
        result['traceback'] = traceback.format_exc()
    finally:
        if process is not None:
            try:
                if process.poll() is None:
                    result['commands'].append('stop')
                    process.stdin.write('stop\n')
                    process.stdin.flush()
                    process.wait(timeout=45)
            except Exception as error:
                result['stop_error'] = repr(error)
                result['forced_kill'] = True
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=10)
            result['exit_code'] = process.returncode
            try:
                os.killpg(process.pid, 0)
                result['process_group_gone'] = False
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                result['process_group_gone'] = True
        result['port_free'] = port_available(port)
        result['passed'] = (result['ready'] and not result.get('error') and
                            result.get('exit_code') == 0 and not result['forced_kill'] and
                            result.get('process_group_gone') and result['port_free'])
    return result


def run(binary, output, port, nether, end, source, digest):
    name = f'nether-{int(nether)}-end-{int(end)}'
    folder = output / name
    folder.mkdir()
    config = folder / 'config'
    config.mkdir()
    settings = ('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\n'
                'auth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\n'
                'prefetch-chunks=0\nenable-nether=' + str(nether).lower() + '\nenable-end=' +
                str(end).lower() + '\n')
    properties = (f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\n'
                  'white-list=false\nenforce-secure-profile=false\nlevel-type=minecraft:flat\n'
                  'level-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\n'
                  'spawn-monsters=false\nspawn-animals=false\n')
    (config / 'mcsa.properties').write_text(settings, encoding='utf-8')
    (config / 'server.properties').write_text(properties, encoding='utf-8')
    report = {'source': source, 'binary': str(binary), 'binary_sha256': digest, 'port': port,
              'settings': settings, 'server_properties': properties, 'phases': [], 'checks': {}}
    enabled = {'nether': nether, 'end': end}
    base = folder / 'world' / 'dimensions' / 'minecraft'
    report['phases'].append(launch(binary, folder, port, 'fresh'))
    log = (folder / 'fresh.log').read_text(encoding='utf-8', errors='replace') if (folder / 'fresh.log').exists() else ''
    report['checks']['overworld_created'] = (folder / 'world' / 'region').is_dir()
    report['checks']['overworld_logged'] = 'Подготовка мира "world"' in log
    for key, dirname in DIMENSIONS.items():
        target = base / dirname
        report['checks'][key + '_fresh_folder_matches'] = target.is_dir() == enabled[key]
        report['checks'][key + '_fresh_log_matches'] = (f'Подготовка мира "world/dimensions/minecraft/{dirname}"' in log) == enabled[key]
        if not enabled[key]:
            target.mkdir(parents=True)
            (target / 'blocks.mcrw').write_bytes(b'previous world must remain untouched\n')
            (target / 'sentinel').write_bytes(b'preserve this file\n')
    before = {k: tree(base / v) for k, v in DIMENSIONS.items() if not enabled[k]}
    report['phases'].append(launch(binary, folder, port, 'existing'))
    after = {k: tree(base / v) for k, v in DIMENSIONS.items() if not enabled[k]}
    report['disabled_before'] = before
    report['disabled_after'] = after
    for key in before:
        report['checks'][key + '_existing_tree_unchanged'] = before[key] == after[key]
    report['passed'] = all(p['passed'] for p in report['phases']) and all(report['checks'].values())
    (folder / 'observations.json').write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('CI-only runtime probe')
    binary = args.binary.resolve(strict=True)
    if not binary.is_file():
        parser.error('binary must be a file')
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    reports = []
    for index, (nether, end) in enumerate(((False, False), (False, True), (True, False), (True, True))):
        try:
            reports.append(run(binary, output, 26500 + index, nether, end, source, digest))
        except Exception as error:
            reports.append({'passed': False, 'nether': nether, 'end': end,
                            'error': repr(error), 'traceback': traceback.format_exc()})
    summary = {'source': source, 'binary_sha256': digest, 'cases': reports,
               'passed': len(reports) == 4 and all(r['passed'] for r in reports)}
    (output / 'summary.json').write_text(json.dumps(summary, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    if not summary['passed']:
        raise SystemExit('dimension checks failed; evidence retained')


if __name__ == '__main__':
    main()
