#!/usr/bin/env python3
"""CI-only парный FireProtection, без сборки и без изменения старых масок."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tarfile

import combat_ci as ci
from fire_stacking_sample import analyze

CONFIG = ci.ROOT / 'tools/blackbox/ci/fire-stacking-777.json'
# Это точный архив СВОЕГО исходника из уже проверенного sourceCI, не Mojang JAR.
ARCHIVE_SHA = 'e5f3fbfc267fd724692f7e7547dbca4dd62ea764028eb7d4ce5e86d98d15888a'
LOCK_SHA = '6c1f0a7abf94e252585096af69c54e1e757c5aa199859d9470de21843ec701b2'


def verify_runtime(folder, pin):
    """Явный адаптер реального source-artifact; старый migration-pin не подменяем."""
    folder = folder.resolve()
    runtime, receipt_path = folder / 'default-runtime-elf', folder / 'receipt.json'
    receipt = json.loads(ci.checked_file(receipt_path).read_text())
    for key in ('source', 'tree', 'run_id', 'attempt'):
        assert receipt[key] == pin[key], 'source build identity: ' + key
    assert receipt['status'] == receipt['pre_archive_job_status'] == 'success'
    assert not receipt['test_retention_errors']
    for name, digest in [('default-runtime-elf', pin['sha256']), ('source.tar.gz', ARCHIVE_SHA), ('Cargo.lock', LOCK_SHA)]:
        path = ci.checked_file(folder / name)
        assert ci.sha(path) == digest == receipt['files'][name]['sha256']
        assert path.stat().st_size == receipt['files'][name]['bytes']
    header = runtime.open('rb').read(20)
    assert header[:6] == b'\x7fELF\x02\x01' and int.from_bytes(header[18:20], 'little') == 62
    compiled = receipt['sources']
    assert compiled and all(ci.sha(ci.checked_file(ci.ROOT / name)) == digest for name, digest in compiled.items()), 'compiled inputs differ'
    with tarfile.open(folder / 'source.tar.gz') as archive:
        members = archive.getmembers()
        assert members and all(m.isfile() and not Path(m.name).is_absolute() and '..' not in Path(m.name).parts for m in members)
        assert {m.name for m in members} == set(compiled), 'source archive coverage'
        for member in members:
            assert hashlib.sha256(archive.extractfile(member).read()).hexdigest() == compiled[member.name]
    return runtime, receipt


def execute(artifact, work, output, java):
    assert os.environ.get('GITHUB_ACTIONS') == 'true', 'CI-only: no local server starts'
    assert not work.exists() and not output.exists(), 'fresh independent directories required'
    output.mkdir(parents=True)
    work.mkdir(parents=True)
    config = json.loads(CONFIG.read_text())
    names = config['scenarios']
    assert names == [r['name'] for r in config['cases']] and len(set(names)) == 6
    for case in config['cases']:
        path = ci.ROOT / 'tools/blackbox/scenarios' / (case['name'] + '.json')
        assert ci.sha(path) == case['scenario_sha256']
    result = {'status': 'infrastructure-failed', 'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ci.ROOT, text=True).strip(),
              'pin': config['runtime'], 'commands': []}
    data, recordings = work / 'data', work / 'recordings'
    data.mkdir()
    def interrupt(sig, frame):
        raise InterruptedError('interrupted: ' + str(sig))
    handlers = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGTERM, signal.SIGINT)}
    try:
        ci.snapshot_sources(output, names, [CONFIG, Path(__file__), ci.ROOT / 'tools/blackbox/fire_stacking_sample.py',
                                          ci.ROOT / 'tools/blackbox/fire_stacking_qualifier.py',
                                          ci.ROOT / 'tools/blackbox/test_fire_stacking_qualifier.py',
                                          ci.ROOT / 'tools/blackbox/test_fire_stacking_sample.py',
                                          ci.ROOT / '.github/workflows/fire-stacking-checks.yml'])
        runtime, receipt = verify_runtime(artifact, config['runtime'])
        ci.write_json(output / 'actual-source-build-receipt.json', receipt)
        shutil.copyfile(artifact / 'source.tar.gz', output / 'actual-compiled-source.tar.gz')
        shutil.copyfile(artifact / 'Cargo.lock', output / 'actual-build-Cargo.lock')
        binary = work / 'mcsheriffanya'
        shutil.copyfile(runtime, binary)
        binary.chmod(0o700)
        version = subprocess.check_output([java, '-version'], stderr=subprocess.STDOUT, text=True, timeout=10)
        assert 'version "25' in version
        result['java_version'] = version
        jar = work / 'server.jar'
        ci.fetch(config['official_jar'], jar)
        for entry in config['json_files']:
            ci.fetch(entry, data / entry['file'])
        command = [sys.executable, str(ci.ROOT / 'tools/blackbox/record777.py'), '--data', str(data),
                   '--jar', str(jar), '--binary', str(binary), '--output', str(recordings), '--java', java,
                   '--max-seconds', '720', '--version', '777', '--isolate-vitals', *names]
        result['commands'].append(command)
        result['record_exit'] = ci.run_tool(command, output / 'recorder.log', 780)
        if result['record_exit']:
            result['early_cleanup'] = ci.cleanup_manifest(recordings)
        command = [sys.executable, str(ci.ROOT / 'tools/blackbox/analyze777.py'), str(recordings), '--protocol', str(data / 'protocol.json')]
        result['commands'].append(command)
        result['analyze_exit'] = ci.run_tool(command, output / 'analyzer.log', 120)
        result['comparison'] = ci.audit_results(recordings, names, config['runtime']['sha256'])
        semantic = analyze(recordings, data, config)
        ci.write_json(output / 'fire-stacking-observations.json', semantic)
        if not result['record_exit'] and not result['analyze_exit'] and result['comparison']['status'] != 'infrastructure-failed' and not semantic['errors']:
            result['status'] = 'qualified-discovery'  # Даже при различиях это не parity PASS.
    except Exception as error:
        result['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        for sig in handlers:
            signal.signal(sig, signal.SIG_IGN)
        try:
            result['watchdog_errors'] = ci.cleanup_manifest(recordings)
            result['ports_free'] = ci.ports_free(ci.PORTS)
            if result['watchdog_errors'] or not all(result['ports_free'].values()):
                result['status'] = 'infrastructure-failed'
        except Exception as error:
            result['status'] = 'infrastructure-failed'
            result['cleanup_error'] = repr(error)
        ci.write_json(output / 'ci-receipt.json', result)
        ci.collect(output, recordings, data)
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('artifact', 'work', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--java', default='java')
    args = parser.parse_args()
    observed = execute(args.artifact, args.work, args.output, args.java)
    print(json.dumps(observed))
    raise SystemExit(0 if observed['status'] == 'qualified-discovery' else 1)
