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

RUN=43710307624
ARTIFACT=14161281253
DIGEST='6854595df385a6c50c6efa9ce445d3566b262782b06c7e2b56a42c4e26024c16'
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

def verify_jobs(jobs):
    rows=jobs.get('jobs',[])
    if not rows or jobs.get('total_count')!=len(rows): raise ValueError('incomplete source jobs')
    if any(j.get('run_id')!=RUN or j.get('head_sha')!=BASE or j.get('status')!='completed' or j.get('conclusion')!='success' for j in rows):
        raise ValueError('source job mismatch or unsuccessful')


def read_verified_archive(archive):
    if sha(archive)!=DIGEST: raise ValueError('artifact actual archive digest mismatch')
    # Только наш CI ZIP. Официальный JAR здесь никогда не открывается.
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        names=z.namelist()
        if len(names)!=len(set(names)): raise ValueError('duplicate artifact members')
        receipt=json.loads(z.read('receipt.json'))
        if receipt.get('source')!=BASE or not receipt.get('scope') or not receipt.get('files'):
            raise ValueError('source receipt mismatch')
        for name,identity in receipt['files'].items():
            path=Path(name)
            if path.is_absolute() or '..' in path.parts: raise ValueError('unsafe receipt member')
            data=z.read(name)
            if len(data)!=identity['bytes'] or sha(data)!=identity['sha256']:
                raise ValueError('receipt member SHA mismatch: '+name)
        data=z.read('runtime-elf')
        if 'runtime-elf' not in receipt['files'] or sha(data)!=ELF_SHA or not data.startswith(b'\x7fELF'):
            raise ValueError('runtime/source receipt mismatch')
    return data,receipt


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
    jobs=json.loads(request(f'{API}/actions/runs/{RUN}/jobs?per_page=100',True))
    verify_jobs(jobs)
    artifact=json.loads(request(f'{API}/actions/artifacts/{ARTIFACT}',True))
    if run['status']!='completed' or run['conclusion']!='success' or run['head_sha']!=BASE: raise ValueError('source run mismatch')
    if artifact['expired'] or artifact['workflow_run']['id']!=RUN or artifact['workflow_run']['head_sha']!=BASE or artifact['name']!=f'adaptive-{RUN}-1' or artifact['digest']!='sha256:'+DIGEST: raise ValueError('artifact metadata mismatch')
    archive=request(f'{API}/actions/artifacts/{ARTIFACT}/zip',True)
    data,receipt=read_verified_archive(archive)
    (out/'runtime-elf').write_bytes(data);(out/'runtime-elf').chmod(0o700)
    pins=json.loads(Path('tools/blackbox/ci/sources-777.json').read_text())
    downloaded={}
    for name,pin in [('server.jar',pins['official_jar'])]+[(p['file'],p) for p in pins['json_files'] if p['file'] in ('protocol.json','entities.json')]:
        data=request(pin['url'])
        if sha(data)!=pin['sha256'] or len(data)!=pin['bytes']: raise ValueError('pin mismatch '+name)
        (out/name).write_bytes(data);downloaded[name]=dict(sha256=sha(data),bytes=len(data),url=pin['url'])
    (evidence/'inputs.json').write_text(json.dumps(dict(run=run,jobs=jobs,artifact=artifact,archive_sha256=sha(archive),elf_sha256=ELF_SHA,receipt=receipt,downloaded=downloaded,diff=diff),indent=2)+'\n')

if __name__=='__main__':main()
