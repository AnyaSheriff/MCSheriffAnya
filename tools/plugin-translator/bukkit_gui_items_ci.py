#!/usr/bin/env python3
"""CI: новый SDK и контроль старого ChestInventory на одном обычном ELF."""
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
import time

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / 'tools/plugin-translator'
OUT = ROOT / 'bukkit-gui-items-evidence'
CONTROL = ROOT / 'tmp/bukkit-gui-items-old-control'
BASE = 'aa889fbc7b8a04643fdc9d91154f243805f2f423'
CHEST = 'tools/plugin-translator/mcsa-bukkit/src/main/java/ru/mcsa/bukkit/ChestInventory.java'
LOCK_SHA = '6508363c084753b8b60f779a7b7fae6f4222fa6b6c900ddbaa6575ea36fbe1f8'
WORKFLOW = '.github/workflows/bukkit-gui-items-checks.yml'
NODE = 'tools/blackbox/ci/attr-phase'


def digest(path):
    state = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(4 * 1024 * 1024), b''):
            state.update(chunk)
    return state.hexdigest()


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def source_paths():
    # Только собственные отслеживаемые исходники и разрешённые входные таблицы.
    sys.path.insert(0, str(ROOT / 'tools/protocol-bridge'))
    from public_container_button_ci import selected
    paths = set(selected())
    for name in git('ls-files', '-z').decode().rstrip('\0').split('\0'):
        p = Path(name)
        if name.startswith('tools/plugin-translator/') and p.suffix in {'.java', '.py', '.c', '.h', '.xml', '.json', '.cjs', '.yml', '.md'}:
            if not {'target', '.toolchain', '__pycache__'} & set(p.parts):
                paths.add(name)
    paths.update([WORKFLOW, NODE + '/package.json', NODE + '/package-lock.json', '.github/release.lock', '.github/release-lock.sha256'])
    return sorted(paths)


def prepare():
    OUT.mkdir(exist_ok=False)
    subprocess.run(['git', 'diff', '--exit-code'], cwd=ROOT, check=True)
    subprocess.run(['git', 'merge-base', '--is-ancestor', BASE, 'HEAD'], cwd=ROOT, check=True)
    lock = ROOT / '.github/release.lock'
    if digest(lock) != LOCK_SHA:
        raise ValueError('unexpected tested root dependency lock')
    # Cargo.lock не отслеживается; меняется только в одноразовом checkout CI.
    shutil.copyfile(lock, ROOT / 'Cargo.lock')
    shutil.copyfile(lock, OUT / 'Cargo.lock')
    paths = source_paths()
    ledger = {name: digest(ROOT / name) for name in paths}
    with tarfile.open(OUT / 'source.tar.gz', 'w:gz') as archive:
        for name in paths:
            archive.add(ROOT / name, arcname=name, recursive=False)
    write(OUT / 'inputs.json', {
        'source': git('rev-parse', 'HEAD').decode().strip(),
        'tree': git('rev-parse', 'HEAD^{tree}').decode().strip(),
        'old_sdk_base': BASE, 'sources': ledger, 'lock_sha256': LOCK_SHA,
        'scope': 'own translated SDK GUI observation and existing 26.1 client regressions; same ordinary host ELF in both cohorts',
        'excludes': ['all Bukkit compatibility', 'all gameplay parity', 'Java777-only proof', 'foreign plugin execution'],
    })


def run(name, command, cwd=ROOT, env=None, timeout=3600):
    OUT.mkdir(exist_ok=True)
    timed_out = False
    cleanup = []
    with (OUT / (name + '.log')).open('w') as log:
        process = subprocess.Popen(list(map(str, command)), cwd=cwd, env=env, stdout=log,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            result = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGTERM)
            try:
                result = process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                result = process.wait(timeout=10)
        finally:
            # Группа создана именно этим запуском; чужие процессы не ищем/не трогаем.
            try:
                os.killpg(process.pid, signal.SIGTERM)
                cleanup.append('SIGTERM remaining own process group')
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    try:
                        os.killpg(process.pid, 0)
                    except ProcessLookupError:
                        break
                    time.sleep(0.1)
                else:
                    os.killpg(process.pid, signal.SIGKILL)
                    cleanup.append('SIGKILL remaining own process group')
            except ProcessLookupError:
                pass
    receipt = {'command': list(map(str, command)), 'cwd': str(cwd), 'pid': process.pid,
               'returncode': result, 'timed_out': timed_out, 'remaining_group_cleanup': cleanup}
    write(OUT / (name + '-process.json'), receipt)
    if timed_out or cleanup:
        raise RuntimeError('unfinished or timed-out own process group: ' + name)
    return result


def assert_old_failure(initial):
    # Общий отказ зависимостей/запуска/другого теста не считается контролем.
    error = initial.get('error') or {}
    window = initial.get('window') or {}
    slots = window.get('slots') or []
    assert initial.get('status') == 'failed' and initial.get('phase') == 'gui-items-observe'
    assert initial.get('restart') is False and initial.get('clientVersion') == '26.1'
    assert 'existing-gui' in initial.get('checkpoints', [])
    assert any('GUI set item observation ready' in m for m in initial.get('messages', []))
    assert error.get('name') == 'AssertionError' and error.get('code') == 'ERR_ASSERTION'
    assert error.get('message', '').splitlines()[0] == 'GUI_ITEM_OBSERVATION_AMOUNT'
    assert error.get('actual') == 1 and error.get('expected') == 2
    assert len(slots) >= 16 and slots[13] == {'name': 'diamond', 'count': 1}
    assert slots[14] == {'name': 'emerald', 'count': 3}
    assert any(row.get('name') == 'window_items' for row in initial.get('guiPackets', []))


def collect(cohort, repo):
    target = OUT / cohort / 'compiled-own'
    source = repo / 'tools/plugin-translator/target'
    for name in ['mcsa-bukkit.jar', 'command.jar', 'events.jar', 'scheduler.jar']:
        path = source / name
        if path.is_file():
            target.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target / name)
    for name in ['java-command', 'java-events', 'java-scheduler']:
        source_plugin = source / 'blackbox-plugins' / name
        for filename in ['plugin.wasm', 'plugin.toml', '.fixture-hash']:
            path = source_plugin / filename
            if path.is_file():
                destination = target / name / filename
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, destination)


def cohort(name, repo, port, expected_old=False):
    output = OUT / name
    if output.exists():
        raise ValueError('cohort output already exists: ' + name)
    env = dict(os.environ, MCSA_GUI_NODE_MODULES=str(ROOT / NODE / 'node_modules'))
    runner = repo / 'tools/plugin-translator/run-blackbox.py'
    try:
        result = run(name, [sys.executable, '-B', runner, '--port', str(port), '--evidence', output], cwd=repo, env=env)
    finally:
        collect(name, repo)
    initial = json.loads((output / 'initial-node.json').read_text())
    stopped = json.loads((output / 'stopped-processes.json').read_text())
    assert all(row['forced_kill'] is False and row['returncode'] == 0 for row in stopped), stopped
    if expected_old:
        assert result != 0 and len(stopped) == 1, (result, stopped)
        assert_old_failure(initial)
        assert not (output / 'restart-node.json').exists()
        verdict = 'expected-specific-gui-assertion'
    else:
        assert result == 0 and len(stopped) == 2, (result, stopped)
        assert initial['status'] == 'passed' and initial['phase'] == 'complete' and initial['error'] is None
        assert initial['checkpoints'] == ['existing-gui', 'gui-items', 'remaining-regressions']
        restart = json.loads((output / 'restart-node.json').read_text())
        assert restart['status'] == 'passed' and restart['checkpoints'] == ['restart-persistence'] and restart['error'] is None
        verdict = 'full-initial-and-restart-passed'
    write(output / 'verdict.json', {'verdict': verdict, 'returncode': result, 'port': port,
                                    'host_sha256': digest(repo / 'target/release/mcsheriffanya')})


def checks():
    host = ROOT / 'target/release/mcsheriffanya'
    with host.open('rb') as stream:
        header = stream.read(20)
    assert header[:6] == b'\x7fELF\x02\x01' and int.from_bytes(header[18:20], 'little') == 62
    assert digest(ROOT / 'Cargo.lock') == LOCK_SHA
    if CONTROL.exists():
        raise ValueError('control tree already exists')
    ledger = json.loads((OUT / 'inputs.json').read_text())['sources']
    for name in ledger:
        if name.startswith('tools/plugin-translator/') or name == 'wit/mcsa-api.wit':
            target = CONTROL / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
    old = git('show', BASE + ':' + CHEST)
    assert old != (ROOT / CHEST).read_bytes()
    (CONTROL / CHEST).write_bytes(old)
    write(OUT / 'old-control-inputs.json', {
        'base': BASE, 'only_changed_source': CHEST,
        'old_chest_sha256': hashlib.sha256(old).hexdigest(), 'new_chest_sha256': digest(ROOT / CHEST),
        'same_other_sources': {name: digest(CONTROL / name) for name in ledger if (CONTROL / name).is_file() and name != CHEST},
        'host_sha256': digest(host),
    })
    destination = CONTROL / 'target/release/mcsheriffanya'
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(host, destination)
    destination.chmod(0o755)
    assert digest(destination) == digest(host)
    # Инструменты компилятора непрозрачны; только повторное использование тех же байтов.
    # Первый перевод готовит инструменты для обоих деревьев; старый SDK переводится отдельно.
    rc = run('build-new-fixtures', [sys.executable, '-B', TOOL / 'build-fixtures.py'])
    assert rc == 0
    cp = str(TOOL / 'target/sdk') + os.pathsep + str(TOOL / 'target/lib/*')
    classes = TOOL / 'target/gui-contracts'
    classes.mkdir(exist_ok=False)
    assert run('sdk-contracts-compile', ['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, TOOL / 'src/test/java/ru/mcsa/translator/SdkContracts.java']) == 0
    assert run('sdk-contracts', ['java', '-cp', str(classes) + os.pathsep + cp, 'ru.mcsa.translator.SdkContracts']) == 0
    assert 'PASS: 173 ' in (OUT / 'sdk-contracts.log').read_text()
    # Новое дерево запускается первым: его полный проход нужен независимо от контроля.
    cohort('candidate', ROOT, 26210)
    control_tool = CONTROL / 'tools/plugin-translator'
    (control_tool / 'target').mkdir()
    (control_tool / 'target/lib').symlink_to(TOOL / 'target/lib', target_is_directory=True)
    (control_tool / '.toolchain').symlink_to(TOOL / '.toolchain', target_is_directory=True)
    cohort('old-control', CONTROL, 26211, expected_old=True)
    for name, expected in ledger.items():
        assert digest(ROOT / name) == expected, 'current source changed: ' + name
    assert digest(host) == digest(destination)
    assert digest(ROOT / 'Cargo.lock') == LOCK_SHA
    write(OUT / 'comparison.json', {'status': 'passed', 'same_host_sha256': digest(host),
          'candidate': 'full initial + persisted restart regressions',
          'old_control': 'same new fixture and bot; only own ChestInventory from base; exact missing live amount update 1 != 2',
          'root_lock_sha256': digest(ROOT / 'Cargo.lock')})


def finish():
    OUT.mkdir(exist_ok=True)
    for cohort_name, repo in [('candidate', ROOT), ('old-control', CONTROL)]:
        collect(cohort_name, repo)
    host = ROOT / 'target/release/mcsheriffanya'
    if host.is_file():
        shutil.copyfile(host, OUT / 'ordinary-runtime-elf')
    if (CONTROL / CHEST).is_file():
        shutil.copyfile(CONTROL / CHEST, OUT / 'old-control-ChestInventory.java')
    tools = {}
    for name in ['wit-bindgen-0.63.0-x86_64-linux/wit-bindgen', 'wasi-sdk-34.0-x86_64-linux/bin/clang', 'wasm-tools-1.262.0-x86_64-linux/wasm-tools']:
        tool = TOOL / '.toolchain' / name
        if tool.is_file():
            tools[name] = {'sha256': digest(tool), 'bytes': tool.stat().st_size}
    write(OUT / 'compiler-tools.json', tools)
    libs = TOOL / 'target/lib'
    write(OUT / 'opaque-java-dependencies.json', {p.name: {'sha256': digest(p), 'bytes': p.stat().st_size} for p in sorted(libs.glob('*.jar')) if p.is_file()})
    files = {str(p.relative_to(OUT)): {'sha256': digest(p), 'bytes': p.stat().st_size}
             for p in sorted(OUT.rglob('*')) if p.is_file() and p.name != 'receipt.json'}
    write(OUT / 'receipt.json', {'source': git('rev-parse', 'HEAD').decode().strip(),
          'tree': git('rev-parse', 'HEAD^{tree}').decode().strip(),
          'run': os.environ.get('GITHUB_RUN_ID'), 'attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
          'job_status': os.environ.get('MCSA_JOB_STATUS'), 'files': files,
          'scope': 'own SDK translated WASM and actual isolated GUI packets; existing client version 26.1'})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', 'checks', 'finish'])
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise SystemExit('GitHub-only runner; no local Cargo/Java/WASM/server launches')
    {'prepare': prepare, 'checks': checks, 'finish': finish}[args.mode]()


if __name__ == '__main__':
    main()
