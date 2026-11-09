#!/usr/bin/env python3
"""Проверяет и сохраняет компактные доказательства; повторяет те же WASM-байты."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tarfile
import time
import zipfile

ROOT = Path(__file__).resolve().parents[2]
BASE = 'e9cf7cdd67b4f2f9b883d5eab54343ac5fdc97dd'
RUN = 42667667634
REPO = 'AnyaSheriff/MCSheriffAnya'
ARTIFACTS = {
    'combat': (13698059681, '24d374e81f5177d9d4cb79ec9d4dc356f7dddad89348efed6c26defe469edfbc'),
    'plugins': (13698002711, '358140e0920f4810b1179e71b3df8a4ee589b4d84d8eebf97115ea0dfc7bf99b'),
}
TESTS = ['translated_bukkit_jars', 'translated_bukkit_listener_unregistration',
         'translated_bukkit_enchantment_removal_reaches_native_inventory']
ALLOWED = {'src/plugins/tests.rs', '.github/workflows/plugin-fixtures-checks.yml',
           'tools/blackbox/recheck_gameplay_inputs.py'}


def sha(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', 'repos/' + REPO + '/' + path]))


def relative(name):
    path = PurePosixPath(name)
    assert not path.is_absolute() and '..' not in path.parts and '\\' not in name
    return path


def verify_own_source_archive(path):
    # Это наш git archive. Вложенные JAR/APK не распаковываются: только хеш blob.
    blobs = {}
    for row in git('ls-tree', '-rz', BASE).split(b'\0'):
        if not row:
            continue
        metadata, name = row.split(b'\t', 1)
        mode, kind, oid = metadata.split()
        if kind == b'blob':
            blobs[name.decode()] = (mode.decode(), oid.decode())
    seen = set()
    with tarfile.open(path, 'r:gz') as archive:
        for member in archive:
            relative(member.name)
            if member.isdir():
                continue
            assert member.name in blobs and member.name not in seen
            mode, expected = blobs[member.name]
            if member.issym():
                assert mode == '120000'
                raw = member.linkname.encode()
                actual = hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()
            else:
                assert member.isfile() and mode in ['100644', '100755']
                value = hashlib.sha1(b'blob ' + str(member.size).encode() + b'\0')
                with archive.extractfile(member) as stream:
                    for block in iter(lambda: stream.read(1024 * 1024), b''):
                        value.update(block)
                actual = value.hexdigest()
            assert actual == expected, member.name
            seen.add(member.name)
    assert seen == set(blobs), 'source archive does not contain the exact Git tree'
    return {'source': BASE, 'sha256': sha(path), 'matching_git_blobs': len(seen)}


def prepare(out):
    out.mkdir(parents=True, exist_ok=False)
    changed = set(git('diff', '--name-only', BASE, 'HEAD').decode().splitlines())
    assert changed == ALLOWED, changed
    module = git('show', 'HEAD:src/plugins/mod.rs').decode()
    assert '#[cfg(all(feature = "plugins", test))]\nmod tests;' in module
    run = api('actions/runs/' + str(RUN))
    assert run['head_sha'] == BASE
    positive = {'checks', 'paired', 'build-other (windows-latest)', 'build-other (macos-latest)'}
    for attempt in range(90):
        jobs = api('actions/runs/' + str(RUN) + '/jobs?per_page=100')
        selected = [job for job in jobs['jobs'] if job['name'] in positive]
        assert len(selected) == 4
        if all(job['conclusion'] == 'success' for job in selected):
            break
        assert not any(job['conclusion'] in ['failure', 'cancelled', 'timed_out'] for job in selected)
        time.sleep(5)
    else:
        raise AssertionError('producer checks are not complete')
    write(out / 'producer-run.json', run)
    write(out / 'producer-jobs.json', jobs)
    sources, artifacts = [], {}
    for label, (number, expected) in ARTIFACTS.items():
        metadata = api('actions/artifacts/' + str(number))
        assert metadata['name'] == 'gameplay-' + label + '-' + str(RUN) + '-1'
        assert metadata['digest'] == 'sha256:' + expected
        assert metadata['workflow_run']['head_sha'] == BASE and not metadata['expired']
        archive = Path(os.environ['RUNNER_TEMP']) / ('producer-' + label + '.zip')
        with archive.open('wb') as stream:
            subprocess.run(['gh', 'api', 'repos/' + REPO + '/actions/artifacts/' + str(number) + '/zip'], stdout=stream, check=True)
        assert sha(archive) == expected and archive.stat().st_size == metadata['size_in_bytes']
        target = out / 'reused' / label
        target.mkdir(parents=True)
        with zipfile.ZipFile(archive) as value:
            assert value.testzip() is None
            assert sum(member.file_size for member in value.infolist()) < 4 * 1024 ** 3
            for member in value.infolist():
                relative(member.filename)
                assert not stat.S_ISLNK(member.external_attr >> 16)
            value.extractall(target)
        snapshots = list(target.rglob('source.tar.gz'))
        assert len(snapshots) == 1
        sources.append(verify_own_source_archive(snapshots[0]))
        # Полный исходный снимок остаётся в неизменном артефакте производителя.
        # Локально уже есть Git; повторно передавать сотни МБ сырых fixtures не нужно.
        snapshots[0].unlink()
        archive.unlink()
        artifacts[label] = metadata
    old = out / 'reused/plugins'
    receipt = json.loads((old / 'plugin-evidence/receipt.json').read_text())
    assert receipt['source'] == BASE and receipt['status'] == 'failure'
    assert sha(old / 'plugin-evidence/mcsa-bukkit.jar') == receipt['sdk_sha256']
    for name, expected in receipt['own_sources_sha256'].items():
        relative(name)
        assert name.startswith(('tools/plugin-translator/mcsa-bukkit/src/main/java/', 'tools/plugin-translator/fixtures/'))
        assert sha(ROOT / name) == expected, name
    assert len(receipt['components']) == 10
    for name, record in receipt['components'].items():
        relative(name)
        assert name.startswith('tmp/java-test/plugins/')
        assert sha(old / name) == record['sha256'] and (old / name).stat().st_size == record['bytes']
    components = old / 'tmp/java-test/plugins'
    assert {p.name for p in components.iterdir()} == {'java-command', 'java-events', 'java-scheduler', 'java-handlers', 'java-enchantments'}
    for component in components.glob('*/plugin.wasm'):
        with component.open('rb') as stream:
            assert stream.read(4) == b'\0asm'
    target = ROOT / 'tmp/java-test/plugins'
    assert not target.exists()
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(components, target)
    lock = old / 'plugin-evidence/Cargo.lock'
    assert not (ROOT / 'Cargo.lock').exists()
    shutil.copy2(lock, ROOT / 'Cargo.lock')
    combat = out / 'reused/combat'
    crossbow = json.loads((combat / 'crossbow-evidence/run.json').read_text())
    armor = json.loads((combat / 'armor-evidence/run.json').read_text())
    assert crossbow['source'] == armor['source'] == BASE
    assert crossbow['status'] == armor['status'] == 'strict-pass'
    assert crossbow['runtime_sha256'] == armor['runtime_sha256']
    write(out / 'input-audit.json', {'status': 'verified', 'base': BASE, 'head': git('rev-parse', 'HEAD').decode().strip(),
          'changed_only_test_and_ci': sorted(changed), 'exact_production_tree': True,
          'producer_artifacts': artifacts, 'own_source_archives': sources,
          'same_five_wasm_bytes': receipt['components'], 'cargo_lock_sha256': sha(lock),
          'producer_plugin_failure_retained': True, 'raw_semantics_reaudit_pending': True})


def tests(out):
    assert json.loads((out / 'input-audit.json').read_text())['status'] == 'verified'
    logs = out / 'test-logs'
    logs.mkdir()
    results = []
    for name in TESTS:
        full = 'plugins::tests::' + name
        command = ['cargo', 'test', '--release', '--locked', full, '--', '--ignored', '--exact', '--nocapture', '--test-threads=1']
        result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors='replace', timeout=2400)
        (logs / (name + '.log')).write_text(result.stdout)
        print(result.stdout, flush=True)
        plain = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', result.stdout)
        passed = result.returncode == 0 and ('test ' + full + ' ...') in plain and 'test result: ok. 1 passed; 0 failed' in plain
        results.append({'name': full, 'return_code': result.returncode, 'actual_one_test_passed': passed})
    write(out / 'runtime-test-results.json', results)
    assert all(result['actual_one_test_passed'] for result in results), results


def finish(out):
    out.mkdir(parents=True, exist_ok=True)
    status = os.environ['MCSA_JOB_STATUS']
    write(out / 'recheck-result.json', {'source': git('rev-parse', 'HEAD').decode().strip(), 'producer_source': BASE,
          'job_status': status, 'scope': 'test-ready snapshot fix; exact same compiled SDK/WASM; production tree unchanged'})
    files = {str(p.relative_to(out)): {'bytes': p.stat().st_size, 'sha256': sha(p)}
             for p in out.rglob('*') if p.is_file() and p.name != 'compact-files.json' and not p.name.endswith(('.tar.gz', '.zip'))}
    write(out / 'compact-files.json', files)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', 'tests', 'finish'])
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    {'prepare': prepare, 'tests': tests, 'finish': finish}[args.mode](args.output)
