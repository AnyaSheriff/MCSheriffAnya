#!/usr/bin/env python3
"""Только CI: получить ровно проверенный собственный ELF для повторения наблюдения."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile

API = 'repos/AnyaSheriff/MCSheriffAnya'
SOURCE = '0a370b95005d1919832829dfe5446d6183bacdf1'
RUN = 43740798269
ARTIFACT = 14173725025
ARCHIVE_SHA = '4781b2a8cef2962ed3ffac169d4874f9eaa75811ab58c00c301f667bc1312c29'
ELF_SHA = '3fdc8ad9f6211ee950c3f45095e6339f8faf07c7c989cfc6175e3c4dece1b042'


def request(path):
    return subprocess.check_output(['gh', 'api', '--method', 'GET', API + path], timeout=240)


def main():
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise SystemExit('GitHub only')
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    run = json.loads(request(f'/actions/runs/{RUN}'))
    jobs = json.loads(request(f'/actions/runs/{RUN}/jobs?per_page=100'))
    artifact = json.loads(request(f'/actions/artifacts/{ARTIFACT}'))
    if run['head_sha'] != SOURCE or run['status'] != 'completed' or run['conclusion'] != 'success':
        raise ValueError('source run mismatch')
    if not jobs['jobs'] or jobs['total_count'] != len(jobs['jobs']) or any(
            job['run_id'] != RUN or job['head_sha'] != SOURCE or job['status'] != 'completed'
            or job['conclusion'] != 'success' for job in jobs['jobs']):
        raise ValueError('source jobs incomplete or unsuccessful')
    if (artifact['id'] != ARTIFACT or artifact['expired']
            or artifact['name'] != f'alerts-{RUN}-1' or artifact['digest'] != 'sha256:' + ARCHIVE_SHA
            or artifact['workflow_run']['id'] != RUN or artifact['workflow_run']['head_sha'] != SOURCE):
        raise ValueError('artifact identity mismatch')
    archive = request(f'/actions/artifacts/{ARTIFACT}/zip')
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA:
        raise ValueError('actual archive SHA mismatch')
    # Только ZIP нашего CI; официальный JAR здесь даже не скачивается.
    with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
        names = bundle.namelist()
        if len(names) != len(set(names)):
            raise ValueError('duplicate members')
        receipt = json.loads(bundle.read('receipt.json'))
        entry = receipt['files']['runtime-elf']
        data = bundle.read('runtime-elf')
        if (receipt['source'] != SOURCE or entry['sha256'] != ELF_SHA
                or len(data) != entry['bytes'] or hashlib.sha256(data).hexdigest() != ELF_SHA
                or not data.startswith(b'\x7fELF')):
            raise ValueError('runtime receipt mismatch')
    (out / 'runtime-elf').write_bytes(data)
    (out / 'runtime-elf').chmod(0o700)
    (out / 'baseline-inputs.json').write_text(json.dumps(dict(
        run=run, jobs=jobs, artifact=artifact, archive_sha256=ARCHIVE_SHA,
        elf_sha256=ELF_SHA, source_receipt=receipt), indent=2) + '\n')


if __name__ == '__main__':
    main()
