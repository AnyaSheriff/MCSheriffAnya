#!/usr/bin/env python3
"""Общий GitHub gate: собственный ModSDK → WASM → настоящий Host."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TESTS = {
    'runtime.log': 'plugins::tests::own_mod_source_tests::translated_own_mod_source_lifecycle_events_and_isolation',
    'runtime-chat.log': 'plugins::tests::own_mod_source_tests::translated_own_mod_chat_is_typed_ordered_and_cancelled',
}


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise SystemExit('Тяжёлые проверки разрешены только на GitHub')
    output = ROOT / 'mod-evidence'
    output.mkdir(exist_ok=True)
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if source != os.environ.get('GITHUB_SHA'):
        raise ValueError('Исходный commit отличается от GitHub SHA')
    paths = [ROOT / 'Cargo.lock', ROOT / 'wit/mcsa-api.wit', ROOT / 'src/plugins/tests.rs']
    for folder in ('tools/mod-api', 'tools/mod-translator'):
        paths.extend(p for p in (ROOT / folder).rglob('*') if p.is_file() and p.suffix in ('.py', '.java', '.toml', '.rs', '.md'))
    fingerprint = lambda: {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))}
    before = fingerprint()
    (output / 'before.json').write_text(json.dumps({'source': source, 'files': before}, indent=2) + '\n')
    commands = [('build-fixtures.log', [sys.executable, '-B', 'tools/mod-translator/build_fixtures.py', '--chat'])]
    commands += [
        (log, ['cargo', 'test', '--release', '--locked', '--bin', 'mcsheriffanya', test,
               '--', '--ignored', '--exact', '--test-threads=1'])
        for log, test in TESTS.items()
    ]
    for name, command in commands:
        with (output / name).open('w') as log:
            result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        print((output / name).read_text(), flush=True)
        result.check_returncode()
    spec = importlib.util.spec_from_file_location('own_runtime_gate', ROOT / 'tools/plugin-translator/bossbar_ci.py')
    gate = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gate)
    for log, test in TESTS.items():
        gate.verify_runtime_log(test, (output / log).read_text())
    after = fingerprint()
    if after != before:
        raise ValueError('Исходники или lock изменились при проверке')
    sys.path.insert(0, str(ROOT / 'tools/mod-translator'))
    from compiler import parse, generate
    fixture_source = (ROOT / 'tools/mod-api/fixtures/welcome/Welcome.java').read_text(encoding='utf-8')
    chat_source = (ROOT / 'tools/mod-api/fixtures/chat/ChatAudit.java').read_text(encoding='utf-8')
    fixtures = {
        'mod-welcome': fixture_source,
        'mod-welcome-b': fixture_source.replace('mod:', 'mod-b:').replace('every(2,', 'every(3,'),
        'mod-chat': chat_source,
    }
    wit_sha = hashlib.sha256((ROOT / 'wit/mcsa-api.wit').read_bytes()).hexdigest()
    for name, java in fixtures.items():
        generated_dir = ROOT / 'tmp/mod-test/plugins' / name
        program = parse(java)
        conversion = json.loads((generated_dir / 'conversion.json').read_text())
        expected = dict(format=1, source_sha256=hashlib.sha256(java.encode('utf-8')).hexdigest(),
            compiler_profile='mcsa-mod-source-0.1', target='26.3/777', sdk_abi='mcsa:api@0.1.0',
            callbacks=len(program['callbacks']),
            wasm_sha256=hashlib.sha256((generated_dir / 'plugin.wasm').read_bytes()).hexdigest(),
            wit_sha256=wit_sha, generated_c_sha256=hashlib.sha256(generate(program).encode('utf-8')).hexdigest())
        if conversion != expected:
            raise ValueError('Метаданные перевода не совпали с фактическими исходниками и WASM: ' + name)
        for filename in ('plugin.wasm', 'plugin.toml', 'conversion.json'):
            source_file = ROOT / 'tmp/mod-test/plugins' / name / filename
            if not source_file.is_file() or source_file.is_symlink():
                raise ValueError('Нет собственного результата: ' + str(source_file))
            target = output / name / filename
            target.parent.mkdir(exist_ok=True)
            shutil.copyfile(source_file, target)
    files = {str(p.relative_to(output)): {'bytes': p.stat().st_size, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in output.rglob('*') if p.is_file()}
    (output / 'receipt.json').write_text(json.dumps({'source': source, 'tests': TESTS, 'files': files, 'scope': 'Own ModSDK fixtures'}, indent=2) + '\n')


if __name__ == '__main__':
    main()
