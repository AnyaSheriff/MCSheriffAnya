#!/usr/bin/env python3
"""GitHub-only own Java fixture -> translator -> exact Wasmtime host tests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TRANSLATOR = ROOT / 'tools/plugin-translator'
TESTS = (
    'plugins::tests::translated_bukkit_bossbar_multiplayer_updates_and_cleanup',
    'plugins::tests::translated_bukkit_bossbar_remove_all_and_disconnected_viewers',
    'plugins::tests::translated_bukkit_bossbar_visibility_failure_rolls_back_all_viewers',
)


def verify_runtime_log(name, log):
    """cargo exit 0 с нулём найденных или проигнорированным тестом — ошибка."""
    if not re.search(r'^test ' + re.escape(name) + r' \.\.\. ok$', log, re.MULTILINE):
        raise ValueError('Не исполнен обязательный тест: ' + name)
    if not re.search(r'^test result: ok\. 1 passed; 0 failed; 0 ignored;', log, re.MULTILINE):
        raise ValueError('Нет ровно одного успешного обязательного теста: ' + name)


def run(command, path):
    with path.open('w') as log:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    print(path.read_text(), end='', flush=True)
    result.check_returncode()


def receipt(output, label):
    paths = [p for base in [TRANSLATOR / 'mcsa-bukkit/src', TRANSLATOR / 'src', TRANSLATOR / 'fixtures/bossbar', ROOT / 'src/plugins'] for p in base.rglob('*') if p.is_file()]
    paths += list(TRANSLATOR.glob('*.py'))
    paths += [ROOT / p for p in ['wit/mcsa-api.wit', 'tools/plugin-translator/adapter.c', 'tools/plugin-translator/bridge.h', 'tools/plugin-translator/pom.xml', 'Cargo.toml', 'Cargo.lock']]
    hashes = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths)) if p.is_file()}
    payload = {'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'run': os.environ.get('GITHUB_RUN_ID'), 'attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
               'sources': hashes}
    (output / (label + '-source.json')).write_text(json.dumps(payload, ensure_ascii=False, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['translate', 'runtime'])
    parser.add_argument('--output', type=Path, default=ROOT / 'bossbar-evidence')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('Тяжёлые проверки разрешены только на GitHub')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    receipt(output, args.mode + '-before')
    if args.mode == 'translate':
        if 'include!("bossbar_tests.rs");' not in (ROOT / 'src/plugins/tests.rs').read_text():
            raise ValueError('Нет согласованного root include bossbar_tests.rs; нельзя заявить runtime coverage')
        run([sys.executable, '-B', TRANSLATOR / 'build-fixtures.py'], output / 'build-fixtures.log')
        run([sys.executable, '-B', TRANSLATOR / 'test-bossbar-guards.py', '--output', output / 'guards'], output / 'guards.log')
        run([sys.executable, '-B', TRANSLATOR / 'translate.py', TRANSLATOR / 'target/bossbar.jar', '--output', ROOT / 'tmp/java-test/plugins', '--report', output / 'translation-report.json'], output / 'translation.log')
        for source in [TRANSLATOR / 'target/bossbar.jar', TRANSLATOR / 'target/mcsa-bukkit.jar', ROOT / 'tmp/java-test/plugins/java-bossbar/plugin.wasm', ROOT / 'tmp/java-test/plugins/java-bossbar/plugin.toml']:
            shutil.copyfile(source, output / source.name)
    else:
        for index, name in enumerate(TESTS):
            log = output / ('runtime-' + str(index + 1) + '.log')
            run(['cargo', 'test', '--release', '--locked', '--bin', 'mcsheriffanya', name, '--', '--ignored', '--exact', '--test-threads=1'], log)
            verify_runtime_log(name, log.read_text())
    receipt(output, args.mode + '-after')
    before = json.loads((output / (args.mode + '-before-source.json')).read_text())
    after = json.loads((output / (args.mode + '-after-source.json')).read_text())
    if before != after:
        raise ValueError('Исходники или Cargo.lock поменялись во время проверки')
    files = {str(p.relative_to(output)): {'sha256': hashlib.sha256(p.read_bytes()).hexdigest(), 'bytes': p.stat().st_size} for p in sorted(output.rglob('*')) if p.is_file() and p.name != args.mode + '-receipt.json'}
    (output / (args.mode + '-receipt.json')).write_text(json.dumps({'source': before['source'], 'mode': args.mode, 'files': files}, indent=2) + '\n')


if __name__ == '__main__':
    main()
