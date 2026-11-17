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
TEST = 'plugins::tests::own_mod_source_tests::translated_own_mod_source_lifecycle_events_and_isolation'


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
    commands = [
        ('build-fixtures.log', [sys.executable, '-B', 'tools/mod-translator/build_fixtures.py']),
        ('runtime.log', ['cargo', 'test', '--release', '--locked', '--bin', 'mcsheriffanya', TEST, '--', '--ignored', '--exact', '--test-threads=1']),
    ]
    for name, command in commands:
        with (output / name).open('w') as log:
            result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        print((output / name).read_text(), flush=True)
        result.check_returncode()
    spec = importlib.util.spec_from_file_location('own_runtime_gate', ROOT / 'tools/plugin-translator/bossbar_ci.py')
    gate = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gate)
    gate.verify_runtime_log(TEST, (output / 'runtime.log').read_text())
    after = fingerprint()
    if after != before:
        raise ValueError('Исходники или lock изменились при проверке')
    sys.path.insert(0, str(ROOT / 'tools/mod-translator'))
    from compiler import parse, generate
    fixture_source = (ROOT / 'tools/mod-api/fixtures/welcome/Welcome.java').read_text(encoding='utf-8')
    wit_sha = hashlib.sha256((ROOT / 'wit/mcsa-api.wit').read_bytes()).hexdigest()
    for name in ('mod-welcome', 'mod-welcome-b'):
        generated_dir = ROOT / 'tmp/mod-test/plugins' / name
        java = fixture_source if name == 'mod-welcome' else fixture_source.replace('mod:', 'mod-b:').replace('every(2,', 'every(3,')
        program = parse(java)
        conversion = json.loads((generated_dir / 'conversion.json').read_text())
        expected = dict(format=1, source_sha256=hashlib.sha256(java.encode('utf-8')).hexdigest(),
            compiler_profile='mcsa-mod-source-0.1', target='26.3/777', callbacks=len(program['callbacks']),
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
    (output / 'receipt.json').write_text(json.dumps({'source': source, 'test': TEST, 'files': files, 'scope': 'Own fixtures only; not Fabric/Forge compatibility'}, indent=2) + '\n')


if __name__ == '__main__':
    main()
