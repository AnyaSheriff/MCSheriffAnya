#!/usr/bin/env python3
"""Только GitHub: пошаговые сетевые наблюдения лечения тихони на двух серверах."""
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

from allay_cases import allay_cases
from analyze_allay_regen import analyze
from compare_parallel_alerts import copy_evidence

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCOPE = ('Six NoAI allay cases: damage ages2/7/12, amounts1/4, then40 individual steps. '
         'Health is a received network snapshot; transition ticks do not prove internal healing time. '
         'No full gameplay/AI/packet latency or physical ARM claim.')


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def dump(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def side_command(side, folder, inputs, cases_file, source, binary, binary_sha, jar_sha):
    cmd = [sys.executable, '-B', str(HERE / 'run_side.py'), '--side', side,
           '--output', str(folder), '--cases-file', str(cases_file), '--data', str(inputs),
           '--source', source, '--port', str(27920 if side == 'reference' else 27921)]
    if side == 'reference':
        return cmd + ['--jar', str(inputs / 'server.jar'), '--jar-sha256', jar_sha]
    return cmd + ['--binary', str(binary), '--binary-sha256', binary_sha]


def run(binary, binary_source, output, require_match=False):
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if subprocess.check_output(['git', 'diff', '--name-only', 'HEAD'], cwd=ROOT, text=True).strip():
        raise ValueError('tracked source must be clean')
    with binary.open('rb') as stream:
        if stream.read(4) != b'\x7fELF':
            raise ValueError('ordinary native ELF required')
    binary_sha = sha(binary)
    pins_path = ROOT / 'tools/blackbox/ci/sources-777.json'
    pins = json.loads(pins_path.read_text())
    cases = allay_cases()
    cases_file = output / 'cases.json'
    dump(cases_file, cases)
    shutil.copyfile(pins_path, output / 'sources-777.json')
    receipt = dict(source=source, binary_source=binary_source, binary_sha256=binary_sha,
                   scope=SCOPE, require_match=require_match, downloaded={}, recording_returncodes={})
    try:
        with tempfile.TemporaryDirectory(prefix='mcsa-allay-') as temporary:
            temporary = Path(temporary)
            inputs = temporary / 'inputs'
            inputs.mkdir()
            entries = [('server.jar', pins['official_jar'])] + [
                (entry['file'], entry) for entry in pins['json_files']
                if entry['file'] in ('protocol.json', 'entities.json')]
            if {name for name, _ in entries} != {'server.jar', 'protocol.json', 'entities.json'}:
                raise ValueError('incomplete pinned inputs')
            # Официальный JAR — только непрозрачный файл для запуска, без разбора.
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
                    cmd = side_command(side, folder, inputs, cases_file, source, binary,
                                       binary_sha, pins['official_jar']['sha256'])
                    receipt['recording_returncodes'][side] = subprocess.run(cmd, cwd=ROOT).returncode
                finally:
                    if folder.exists():
                        copy_evidence(folder, output / side)
        paths = [output / side / 'side.json' for side in ('reference', 'candidate')]
        if all(path.exists() for path in paths):
            reports = [json.loads(path.read_text()) for path in paths]
            analysis = analyze(reports[0], reports[1], cases)
            dump(output / 'analysis.json', analysis)
            receipt['recording_valid'] = analysis['recording_valid']
            receipt['exact_received_health_match'] = analysis['exact_received_health_match']
        receipt['binary_unchanged'] = sha(binary) == binary_sha
        receipt['recording_complete'] = receipt['recording_returncodes'] == {'reference': 0, 'candidate': 0}
        receipt['passed'] = (receipt['recording_complete'] and receipt.get('recording_valid', False)
                             and receipt['binary_unchanged']
                             and (not require_match or receipt.get('exact_received_health_match', False)))
    except Exception as error:
        receipt.update(passed=False, error=repr(error), traceback=traceback.format_exc())
    finally:
        dump(output / 'receipt.json', receipt)
    print(json.dumps(receipt, ensure_ascii=False, indent=2))
    return receipt['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--binary-source', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--require-match', action='store_true')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('GitHub-only; local server execution forbidden')
    args.output.mkdir(parents=True, exist_ok=False)
    if not run(args.binary.resolve(strict=True), args.binary_source, args.output.resolve(), args.require_match):
        raise SystemExit('allay recording or requested parity gate failed; raw evidence retained')


if __name__ == '__main__':
    main()
