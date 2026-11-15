#!/usr/bin/env python3
"""Только GitHub: 12 прежних парных опытов bee/dolphin с обоими режимами ON.

Это регрессия выбранных наблюдений vanilla. Она не доказывает активацию
помощников: фактические параллельные пути проверяются отдельно Rust-тестами.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import traceback
import urllib.request

from common_cases import common_cases

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCOPE = ('12 existing bee/dolphin common cases compared with opaque pinned vanilla; '
         'candidate has both parallel flags enabled. No helper activation, speedup, '
         'packet latency equality or complete AI/gameplay claim.')
FAMILIES = {'controlled_spawn', 'turn', 'teleport', 'tracking_leave_return', 'damage', 'death'}


def selected_cases():
    cases = [c for c in common_cases() if c['mob'] in ('bee', 'dolphin')]
    if len(cases) != 12 or {(c['mob'], c['family']) for c in cases} != {
            (mob, family) for mob in ('bee', 'dolphin') for family in FAMILIES}:
        raise ValueError('expected exact12 existing common cases')
    return cases


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def dump(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')


def copy_evidence(source, output):
    """Копируем только собственные наблюдения; world/JAR/ELF исключены."""
    for path in source.rglob('*'):
        relative = path.relative_to(source)
        if (not path.is_file() or 'world' in relative.parts
                or path.name not in ('side.json', 'server.log', 'commands.jsonl')):
            continue
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)


def side_command(side, output, inputs, cases_file, source, binary, elf_sha, jar_sha):
    cmd = [sys.executable, '-B', str(HERE / 'run_side.py'), '--side', side,
           '--output', str(output), '--cases-file', str(cases_file), '--data', str(inputs),
           '--source', source, '--port', str(27820 if side == 'reference' else 27821)]
    if side == 'reference':
        return cmd + ['--jar', str(inputs / 'server.jar'), '--jar-sha256', jar_sha]
    return cmd + ['--binary', str(binary), '--binary-sha256', elf_sha, '--combined-parallel']


def run(binary, output):
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=ROOT, text=True).strip():
        raise ValueError('tracked source must be clean for exact HEAD receipt')
    with binary.open('rb') as f:
        if f.read(4) != b'\x7fELF':
            raise ValueError('ordinary native ELF required')
    elf_sha = sha(binary)
    pins_path = ROOT / 'tools/blackbox/ci/sources-777.json'
    pins = json.loads(pins_path.read_text())
    cases_file = output / 'cases.json'
    dump(cases_file, selected_cases())
    shutil.copyfile(pins_path, output / 'sources-777.json')
    receipt = dict(source=source, binary_sha256=elf_sha, scope=SCOPE,
                   helper_activity_proved=False, downloaded={}, recording_returncodes={},
                   candidate_settings={'parallel-dimension-ticks': 'true', 'experimental-parallel-mobs': 'true'})
    try:
        # Официальный JAR сохраняется непрозрачным файлом; никакого ZIP/JAR-разбора.
        with tempfile.TemporaryDirectory(prefix='mcsa-alerts-') as temporary:
            temporary = Path(temporary)
            inputs = temporary / 'inputs'
            inputs.mkdir()
            entries = [('server.jar', pins['official_jar'])] + [
                (p['file'], p) for p in pins['json_files'] if p['file'] in ('protocol.json', 'entities.json')]
            if {name for name, _ in entries} != {'server.jar', 'protocol.json', 'entities.json'}:
                raise ValueError('incomplete pinned inputs')
            for name, pin in entries:
                with urllib.request.urlopen(pin['url'], timeout=120) as response:
                    data = response.read(pin['bytes'] + 1)
                if len(data) != pin['bytes'] or hashlib.sha256(data).hexdigest() != pin['sha256']:
                    raise ValueError('pinned input mismatch: ' + name)
                (inputs / name).write_bytes(data)
                receipt['downloaded'][name] = dict(url=pin['url'], bytes=len(data), sha256=pin['sha256'])
            for side in ('reference', 'candidate'):
                folder = temporary / side
                try:
                    command = side_command(side, folder, inputs, cases_file, source,
                                           binary, elf_sha, pins['official_jar']['sha256'])
                    receipt['recording_returncodes'][side] = subprocess.run(command, cwd=ROOT).returncode
                    if side == 'candidate' and (folder / 'world/config/mcsa.properties').exists():
                        settings = dict(line.split('=', 1) for line in (folder / 'world/config/mcsa.properties').read_text().splitlines()
                                        if line and not line.startswith('#') and '=' in line)
                        receipt['candidate_flags_read_back'] = {k: settings.get(k) for k in receipt['candidate_settings']}
                finally:
                    if folder.exists():
                        copy_evidence(folder, output / side)
        sides = [output / side / 'side.json' for side in ('reference', 'candidate')]
        if all(p.exists() for p in sides):
            command = [sys.executable, '-B', str(HERE / 'report.py'), '--reference', str(sides[0]),
                       '--candidate', str(sides[1]), '--cases-file', str(cases_file),
                       '--output', str(output / 'comparison'), '--gate']
            receipt['parity_gate_exit'] = subprocess.run(command, cwd=ROOT).returncode
        receipt['recording_complete'] = (receipt['recording_returncodes'] == {'reference': 0, 'candidate': 0})
        candidate_report = json.loads(sides[1].read_text()) if sides[1].exists() else {}
        log = (output / 'candidate/server.log').read_text(errors='replace') if (output / 'candidate/server.log').exists() else ''
        receipt['candidate_modes_logged'] = ('Экспериментальный поиск угроз и целей мобов включён' in log
                                             and 'Параллельная блочная часть измерений включена' in log)
        receipt['candidate_flags_verified'] = (receipt.get('candidate_flags_read_back') == receipt['candidate_settings']
            and candidate_report.get('candidate_parallel_flags') == {k: True for k in receipt['candidate_settings']}
            and receipt['candidate_modes_logged'])
        receipt['binary_unchanged'] = sha(binary) == elf_sha
        receipt['passed'] = (receipt['recording_complete'] and receipt.get('parity_gate_exit') == 0
                             and receipt['candidate_flags_verified'] and receipt['binary_unchanged'])
    except Exception as error:
        receipt.update(passed=False, error=repr(error), traceback=traceback.format_exc())
    finally:
        dump(output / 'receipt.json', receipt)
    print(json.dumps(receipt, ensure_ascii=False, indent=2))
    return receipt['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('GitHub-only; local server execution forbidden')
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    if not run(binary, args.output.resolve()):
        raise SystemExit('paired alerts regression failed; raw evidence retained')


if __name__ == '__main__':
    main()
