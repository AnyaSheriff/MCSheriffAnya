#!/usr/bin/env python3
"""Закреплённые входы CI; официальный JAR только сохраняется непрозрачным файлом."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import zipfile
from suite import BASE, ELF_SHA

RUN=43591025465
ARTIFACT=14112883972
DIGEST='eefcc6243b290878454ace76ab5351513d2e5f4fc50922e918da9af71fedaf26'
API='https://api.github.com/repos/AnyaSheriff/MCSheriffAnya'

def request(url, authenticated=False):
    if authenticated:
        if not url.startswith(API+'/actions/'):
            raise ValueError('authenticated request outside pinned repository API')
        # gh управляет авторизацией; Go HTTP не переносит её на чужой blob-host.
        # Не читаем токен и не создаём собственный Authorization header.
        return subprocess.check_output(['gh','api','--method','GET',url], timeout=180)
    with urllib.request.urlopen(urllib.request.Request(url),timeout=120) as r: return r.read()

def sha(data): return hashlib.sha256(data).hexdigest()

def main():
    if os.environ.get('GITHUB_ACTIONS')!='true': raise SystemExit('GitHub only')
    import sys
    out=Path(sys.argv[1]); evidence=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True);evidence.mkdir(parents=True,exist_ok=True)
    diff=subprocess.check_output(['git','diff','--name-status',BASE,'HEAD'],text=True)
    for line in diff.splitlines():
        status,name=line.split('\t')
        if status!='A' or not (name.startswith('tools/blackbox/mobs/') or name=='.github/workflows/mob-blackbox-checks.yml'):
            raise ValueError('unexpected source diff: '+line)
    run=json.loads(request(f'{API}/actions/runs/{RUN}',True))
    artifact=json.loads(request(f'{API}/actions/artifacts/{ARTIFACT}',True))
    if run['status']!='completed' or run['conclusion']!='success' or run['head_sha']!=BASE: raise ValueError('source run mismatch')
    if artifact['expired'] or artifact['workflow_run']['id']!=RUN or artifact['workflow_run']['head_sha']!=BASE or artifact['name']!=f'dispenser-source-{RUN}-1' or artifact['digest']!='sha256:'+DIGEST: raise ValueError('artifact metadata mismatch')
    archive=request(f'{API}/actions/artifacts/{ARTIFACT}/zip',True)
    if sha(archive)!=DIGEST: raise ValueError('artifact actual archive digest mismatch')
    # Это наш CI-архив, не официальный JAR/APK. Извлекается только наш ELF.
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        data=z.read('runtime-elf')
        receipt=json.loads(z.read('receipt.json'))
    if sha(data)!=ELF_SHA or not data.startswith(b'\x7fELF') or receipt['source']!=BASE or receipt['job_status']!='success': raise ValueError('runtime/source receipt mismatch')
    (out/'runtime-elf').write_bytes(data);(out/'runtime-elf').chmod(0o700)
    pins=json.loads(Path('tools/blackbox/ci/sources-777.json').read_text())
    downloaded={}
    for name,pin in [('server.jar',pins['official_jar'])]+[(p['file'],p) for p in pins['json_files'] if p['file'] in ('protocol.json','entities.json')]:
        data=request(pin['url'])
        if sha(data)!=pin['sha256'] or len(data)!=pin['bytes']: raise ValueError('pin mismatch '+name)
        (out/name).write_bytes(data);downloaded[name]=dict(sha256=sha(data),bytes=len(data),url=pin['url'])
    (evidence/'inputs.json').write_text(json.dumps(dict(run=run,artifact=artifact,archive_sha256=sha(archive),elf_sha256=ELF_SHA,receipt=receipt,downloaded=downloaded,diff=diff),indent=2)+'\n')

if __name__=='__main__':main()
