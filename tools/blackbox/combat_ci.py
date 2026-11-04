#!/usr/bin/env python3
"""CI-only повтор13 прежних боевых случаев; без Cargo и чужого protocol codec."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
import urllib.request
from combat_process import group_exists, ports_free, process_identity, signal_group, signal_process

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'tools/blackbox/ci/sources-777.json'
PIN = ROOT / 'tools/blackbox/ci/runtime-pin.json'
PORTS = [26140, 26141, 26142, 26143]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
    temporary.replace(path)


def resolve_pin(saved, run_id='', head='', runtime_sha=''):
    explicit = [run_id, head, runtime_sha]
    if any(explicit) and not all(explicit):
        raise ValueError('all three runtime inputs are required together')
    value = {'build_run_id': str(saved.get('build_run_id') or ''),
             'head': saved.get('head') or '', 'runtime_sha256': saved.get('runtime_sha256') or ''}
    if all(explicit):
        chosen = dict(zip(value, explicit))
        if all(value.values()) and chosen != value:
            raise ValueError('explicit runtime does not match checked pin')
        value = chosen
    if not re.fullmatch('[1-9][0-9]*', value['build_run_id']):
        raise ValueError('runtime run ID is unset or invalid; no latest fallback')
    for key, length in [('head', 40), ('runtime_sha256', 64)]:
        if not re.fullmatch(f'[0-9a-f]{{{length}}}', value[key]):
            raise ValueError(f'runtime {key} is unset or invalid')
    return value


def validate_run(data, pin, repository):
    if (str(data.get('id')) != pin['build_run_id'] or data.get('head_sha') != pin['head']
            or data.get('status') != 'completed' or data.get('conclusion') != 'success'
            or data.get('path') != '.github/workflows/migration-checks.yml'
            or data.get('repository', {}).get('full_name') != repository
            or data.get('head_repository', {}).get('full_name') != repository):
        raise ValueError('build run/head/workflow/repository/conclusion mismatch')
    attempt = data.get('run_attempt')
    if not isinstance(attempt, int) or isinstance(attempt, bool) or attempt < 1:
        raise ValueError('invalid run attempt')
    return f'migration-checks-{attempt}'


def preflight(pin, output):
    repository = os.environ['GITHUB_REPOSITORY']
    if not re.fullmatch('[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
        raise ValueError('invalid repository')
    url = f'https://api.github.com/repos/{repository}/actions/runs/{pin["build_run_id"]}'
    request = urllib.request.Request(url, headers={
        'Authorization': 'Bearer ' + os.environ['GH_TOKEN'],
        'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28'})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = json.load(response)
    artifact = validate_run(data, pin, repository)
    result = {'pin': pin, 'artifact_name': artifact, 'run_attempt': data['run_attempt'],
              'repository': repository, 'build_workflow': data['path']}
    write_json(output / 'build-selection.json', result)
    return result


def checked_file(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f'not a regular file: {path.name}')
    return path


def verify_artifact(folder, selected):
    paths = list(folder.rglob('*'))
    if any(p.is_symlink() for p in paths):
        raise ValueError('downloaded artifact contains symlink')
    binaries = [p for p in paths if p.name == 'mcsheriffanya' and p.is_file()]
    receipts = [p for p in paths if p.name == 'receipt.json' and p.is_file()]
    if len(binaries) != 1 or len(receipts) != 1:
        raise ValueError('artifact must contain exactly one runtime and receipt')
    runtime, receipt_path = checked_file(binaries[0]), checked_file(receipts[0])
    receipt = json.loads(receipt_path.read_text())
    pin = selected['pin']
    if (receipt.get('head') != pin['head'] or str(receipt.get('run_id')) != pin['build_run_id']
            or str(receipt.get('run_attempt')) != str(selected['run_attempt'])
            or receipt.get('status_before_receipt') != 'success' or receipt.get('runtime_built') is not True
            or receipt.get('sha256', {}).get('target/release/mcsheriffanya') != pin['runtime_sha256']
            or sha(runtime) != pin['runtime_sha256']):
        raise ValueError('artifact receipt/runtime pin mismatch')
    return runtime, receipt_path


def fetch(entry, target):
    if not entry['url'].startswith('https://'):
        raise ValueError('only pinned HTTPS sources')
    temporary = target.with_suffix(target.suffix + '.download')
    # Сеть только для закреплённых opaque JAR и JSON, архивы не открываются.
    with urllib.request.urlopen(entry['url'], timeout=30) as source, temporary.open('wb') as dest:
        limit = entry['bytes']
        count = 0
        while part := source.read(1024 * 1024):
            count += len(part)
            if count > limit:
                raise ValueError('download larger than pinned size')
            dest.write(part)
    if temporary.stat().st_size != entry['bytes'] or sha(temporary) != entry['sha256']:
        raise ValueError('pinned download hash/size mismatch')
    if 'sha1' in entry and hashlib.sha1(temporary.read_bytes()).hexdigest() != entry['sha1']:
        raise ValueError('official JAR SHA1 mismatch')
    temporary.replace(target)


def helper_sources(root=ROOT):
    directories = [root / 'tools/blackbox', root / 'tools/protocol-bridge', root / 'tools/measure']
    pending = [directories[0] / name for name in ('record777.py', 'analyze777.py', 'combat_ci.py')]
    result = set()
    while pending:
        path = pending.pop()
        if path in result:
            continue
        checked_file(path)
        result.add(path)
        for node in ast.walk(ast.parse(path.read_text())):
            names = [a.name for a in node.names] if isinstance(node, ast.Import) else [node.module] if isinstance(node, ast.ImportFrom) and node.module else []
            for name in names:
                for directory in directories:
                    candidate = directory / (name.split('.')[0] + '.py')
                    if candidate.is_file():
                        pending.append(candidate)
                        break
    result.update(directories[0] / name for name in ('bot.js', 'attr-phase.js', 'compare.js', 'replay777.js', 'value-order.js', 'data/sounds.json'))
    result.update(root / name for name in ('tools/protocol-bridge/testdata/attribute-id-observations.json',
                  'tools/blackbox/ci/sources-777.json', 'tools/blackbox/ci/runtime-pin.json',
                  '.github/workflows/combat-checks.yml', 'docs/combat-ci-26.3.md'))
    return sorted(result)


def snapshot_sources(output, scenarios):
    files = helper_sources() + [ROOT / f'tools/blackbox/scenarios/{name}.json' for name in scenarios]
    for path in files:
        checked_file(path)
        dest = output / 'source' / path.relative_to(ROOT)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, dest)


def cleanup_manifest(results):
    """Последний watchdog: PID из файла сам по себе не даёт права на kill."""
    path = results / 'lifecycle.json'
    if not path.exists():
        return []
    errors = []
    for row in json.loads(path.read_text()).get('servers', []):
        pid = row['pid']
        if not group_exists(pid):
            continue
        identity = row.get('identity')
        witnesses=row.get('members',[])
        try:
            signal_group(pid, signal.SIGTERM, identity, witnesses)
            deadline = time.monotonic() + 2
            while group_exists(pid) and time.monotonic() < deadline:
                time.sleep(.05)
            if group_exists(pid):
                signal_group(pid, signal.SIGKILL, identity, witnesses)
            if group_exists(pid):
                errors.append({'pid': pid, 'error': 'group still exists after watchdog'})
        except (ValueError, OSError) as error:
            errors.append({'pid': pid, 'error': str(error)})
    return errors


def run_tool(argv, log, timeout):
    with log.open('w') as stream:
        process = subprocess.Popen(argv, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        identity = None
        try:
            identity = process_identity(process.pid)
            return process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            signal_process(process, signal.SIGTERM, identity)
            try:
                process.wait(timeout=35)
            except subprocess.TimeoutExpired:
                signal_process(process, signal.SIGKILL, identity)
                process.wait(timeout=3)
            return 124
        except BaseException:
            if process.poll() is None:
                signal_process(process, signal.SIGTERM, identity)
                try:
                    process.wait(timeout=35)
                except subprocess.TimeoutExpired:
                    signal_process(process, signal.SIGKILL, identity)
                    process.wait(timeout=3)
            raise


def audit_results(results, scenarios, expected_binary):
    errors, comparisons = [], []
    try:
        runs = list(results.glob('run-*.json'))
        if len(runs) != 1:
            raise ValueError('exactly one fresh recorder manifest required')
        manifest = json.loads(runs[0].read_text())
        if manifest.get('binary_sha256') != expected_binary or manifest.get('protocol') != 777 or manifest.get('primary') != '26.3' or manifest.get('scenario_names') != scenarios or manifest.get('isolate_vitals') is not True or manifest.get('recorder_sha256') != sha(ROOT / 'tools/blackbox/record777.py'):
            raise ValueError('recorder manifest does not match selected runtime/scenarios')
        summary = json.loads((results / 'comparison-summary.json').read_text())
        if len(summary) != len(scenarios) or {r['scenario'] for r in summary} != set(scenarios):
            raise ValueError('comparison scenario coverage mismatch')
        for name in scenarios:
            row = next(r for r in summary if r['scenario'] == name)
            if row.get('record_status') != 'complete' or row.get('record_error'):
                errors.append(f'{name}: recording/late cleanup did not complete')
            for side in ('official', 'ours'):
                path = checked_file(results / f'{name}-{side}.json')
                raw = json.loads(path.read_text())
                if raw.get('scenario') != name or raw.get('protocol') != 777 or raw.get('recorder_sha256') != manifest['recorder_sha256']:
                    errors.append(f'{name}/{side}: raw identity mismatch')
                clients = raw.get('clients')
                if not isinstance(clients, list) or not clients or any(
                        not isinstance(c, dict) or c.get('failure')
                        or not isinstance(c.get('events'), list) or not c['events'] for c in clients):
                    errors.append(f'{name}/{side}: missing/empty client events or client failure')
                projected = json.loads(checked_file(results / f'{name}-{side}-events.json').read_text())
                if not isinstance(projected, dict) or not isinstance(projected.get('events'), list) or not projected['events']:
                    errors.append(f'{name}/{side}: missing/empty projected events')
            checked_file(results / f'{name}-comparison.txt')
            counts = row.get('comparison_counts')
            gaps = row.get('decode_errors')
            if not isinstance(counts, list) or len(counts) != 4 or any(type(n) is not int or n < 0 for n in counts):
                errors.append(f'{name}: comparison counts missing/invalid')
            elif not any(counts):
                errors.append(f'{name}: all-zero comparison for nonempty combat scenario')
            if not isinstance(gaps, list) or len(gaps) != 2 or any(gaps):
                errors.append(f'{name}: decoder errors/missing decoder status')
            expected = 'mismatch-open' if (counts and any(counts[1:])) or row.get('warnings') else 'match'
            if row.get('status') != expected:
                errors.append(f'{name}: status/counts inconsistent')
            comparisons.append({'scenario': name, 'counts': counts, 'status': row.get('status'),
                                'warnings': row.get('warnings'), 'decode_errors': gaps})
        lifecycle = json.loads((results / 'lifecycle.json').read_text())
        expected_pids = {r['pid'] for r in lifecycle.get('servers', [])}
        clean = lifecycle.get('cleanup', [])
        if lifecycle.get('cleanup_errors') or len(expected_pids) != 2 or {r.get('pid') for r in clean} != expected_pids or any(r.get('error') or r.get('exit_code') != 0 or not r.get('group_gone') for r in clean) or not all(lifecycle.get('ports_free', {}).get(str(p)) for p in PORTS):
            errors.append('incomplete or unclean server lifecycle')
    except (OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        errors.append(str(error))
    status = 'infrastructure-failed' if errors else 'mismatch-open' if any(r['status'] != 'match' for r in comparisons) else 'match'
    return {'status': status, 'errors': errors, 'comparisons': comparisons}


def collect(output, results, data):
    if results.exists():
        for path in results.iterdir():
            if path.is_symlink():
                raise ValueError('symlink in result root')
            if path.is_file() and path.suffix in ('.json', '.txt'):
                shutil.copyfile(path, output / path.name)
        for label in ('official', 'ours'):
            server = results / label
            for path in [server / 'server.log', *(server / 'config').glob('*'), server / 'server.properties']:
                if path.exists():
                    checked_file(path)
                    target = output / 'servers' / label / path.relative_to(server)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(path, target)
    if data.exists():
        if any(p.is_symlink() for p in data.rglob('*')):
            raise ValueError('symlink in protocol data')
        shutil.copytree(data, output / 'data', dirs_exist_ok=True)
    paths = sorted(output.rglob('*'))
    if any(p.is_symlink() for p in paths):
        raise ValueError('output artifact contains symlink')
    if any(p.suffix == '.jar' or 'world' in p.relative_to(output).parts for p in paths):
        raise ValueError('JAR/world must not enter artifact')
    write_json(output / 'files.json', [{'path': str(p.relative_to(output)), 'bytes': p.stat().st_size,
                                      'sha256': sha(p)} for p in paths if p.is_file() and p.name != 'files.json'])


def execute(artifact, selection, work, output, java):
    sources = json.loads(SOURCES.read_text())
    if work.exists() or (output / 'ci-receipt.json').exists():
        raise ValueError('fresh work/output required; selection and initial logs only')
    work.mkdir(parents=True)
    results, data = work / 'recordings', work / 'data'
    data.mkdir()
    receipt = {'status': 'infrastructure-failed', 'stage': 'prepare', 'selection': selection,
               'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'scenarios': sources['scenarios'], 'commands': []}
    write_json(output / 'ci-receipt.json', receipt)
    def interrupted(sig, frame):
        raise InterruptedError(f'CI interrupted: {sig}')
    previous = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        snapshot_sources(output, sources['scenarios'])
        runtime, build_receipt = verify_artifact(artifact, selection)
        shutil.copyfile(build_receipt, output / 'build-receipt.json')
        copied = work / 'mcsheriffanya'
        shutil.copyfile(runtime, copied)
        copied.chmod(0o700)
        version = subprocess.check_output([java, '-version'], stderr=subprocess.STDOUT, text=True, timeout=10)
        if not re.search(r'version "25(?:[.\"]|$)', version):
            raise ValueError('Java25 required')
        receipt['java_version'] = version
        jar = work / 'server.jar'
        fetch(sources['official_jar'], jar)
        for entry in sources['json_files']:
            fetch(entry, data / entry['file'])
        receipt['stage'] = 'record'
        command = [sys.executable, str(ROOT / 'tools/blackbox/record777.py'), '--data', str(data),
                   '--jar', str(jar), '--binary', str(copied), '--output', str(results),
                   '--java', java, '--max-seconds', '1440', '--version', '777', '--isolate-vitals', *sources['scenarios']]
        receipt['commands'].append(command)
        write_json(output / 'ci-receipt.json', receipt)
        receipt['record_exit'] = run_tool(command, output / 'recorder.log', 1500)
        if receipt['record_exit']:
            receipt['early_watchdog_errors'] = cleanup_manifest(results)
        receipt['stage'] = 'analyze'
        command = [sys.executable, str(ROOT / 'tools/blackbox/analyze777.py'), str(results), '--protocol', str(data / 'protocol.json')]
        receipt['commands'].append(command)
        receipt['analyze_exit'] = run_tool(command, output / 'analyzer.log', 180)
        receipt.update(audit_results(results, sources['scenarios'], selection['pin']['runtime_sha256']))
        if receipt['record_exit'] or receipt['analyze_exit']:
            receipt['status'] = 'infrastructure-failed'
            receipt['errors'].append('recorder/analyzer nonzero exit')
        receipt['stage'] = 'complete'
    except Exception as error:
        receipt['error'] = type(error).__name__ + ': ' + str(error)
        receipt['status'] = 'infrastructure-failed'
    finally:
        for sig in previous:
            signal.signal(sig, signal.SIG_IGN)
        try:
            receipt['watchdog_errors'] = cleanup_manifest(results)
            receipt['final_ports_free'] = ports_free(PORTS)
            if receipt['watchdog_errors'] or not all(receipt['final_ports_free'].values()):
                receipt['status'] = 'infrastructure-failed'
        except Exception as error:
            receipt['status'] = 'infrastructure-failed'
            receipt['cleanup_error'] = str(error)
        write_json(output / 'ci-receipt.json', receipt)
        try:
            collect(output, results, data)
        except Exception as error:
            receipt['status'] = 'infrastructure-failed'
            receipt['collection_error'] = str(error)
            write_json(output / 'ci-receipt.json', receipt)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    return receipt


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=('pin', 'run'))
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--artifact', type=Path)
    p.add_argument('--work', type=Path)
    p.add_argument('--java', default='java')
    a = p.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        p.error('CI-only entrypoint: GITHUB_ACTIONS=true required')
    a.output.mkdir(parents=True, exist_ok=True)
    try:
        if a.mode == 'pin':
            pin = resolve_pin(json.loads(PIN.read_text()), os.environ.get('INPUT_BUILD_RUN_ID', ''),
                              os.environ.get('INPUT_BUILD_HEAD', ''), os.environ.get('INPUT_RUNTIME_SHA256', ''))
            result = preflight(pin, a.output)
            with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
                stream.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={result["artifact_name"]}\n')
        else:
            if not a.artifact or not a.work:
                p.error('run requires --artifact and --work')
            result = execute(a.artifact, json.loads((a.output / 'build-selection.json').read_text()),
                             a.work.resolve(), a.output.resolve(), a.java)
            if result['status'] != 'match':
                raise SystemExit(1)
    except Exception as error:
        write_json(a.output / 'entrypoint-error.json', {'status': 'infrastructure-failed', 'error': type(error).__name__ + ': ' + str(error)})
        raise SystemExit(1)


if __name__ == '__main__':
    main()
