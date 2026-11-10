#!/usr/bin/env python3
"""Pins и receipt isolated staged BlockBreakEvent: только собственные артефакты."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import sys
import zipfile

ROOT=Path(__file__).resolve().parents[2]
OUT=ROOT/'staged-evidence'
REPO='AnyaSheriff/MCSheriffAnya'
PINS={
 'baseline':(13707523078,'87e30ea72931475940fe4d802425ba2db0ff47d8c26b7d2817a65e501707b7aa','354a0d137b60dbd89d32246c3e0aef2ddbd062c1'),
 'old-five':(13703641973,'b2daaafb8753161205289123fe81be2be4ddd92234c0c887c813608f002586b6','a76b0d755a9e4830b0759e7d72563cc667efeda5'),
}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,data):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(data,indent=2)+'\n')
def inputs():
 OUT.mkdir(exist_ok=True)
 for label,(number,digest,source) in PINS.items():
  api=json.loads(subprocess.check_output(['gh','api',f'repos/{REPO}/actions/artifacts/{number}']))
  assert api['digest']=='sha256:'+digest and api['workflow_run']['head_sha']==source and not api['expired']
  archive=Path(os.environ['RUNNER_TEMP'])/(label+'.zip')
  with archive.open('wb') as stream:subprocess.run(['gh','api',f'repos/{REPO}/actions/artifacts/{number}/zip'],stdout=stream,check=True)
  assert sha(archive)==digest
  with zipfile.ZipFile(archive) as z:
   prefix='native/plugins/' if label=='baseline' else 'reused/plugins/tmp/java-test/plugins/'
   destination=ROOT/('tmp/java-priority-baseline/plugins' if label=='baseline' else 'tmp/java-test/plugins')
   assert not destination.exists()
   hashes={}
   for name in z.namelist():
    if not name.startswith(prefix) or name.endswith('/'):continue
    relative=Path(name.removeprefix(prefix));assert '..' not in relative.parts and len(relative.parts)==2 and relative.name in ['plugin.toml','plugin.wasm']
    path=destination/relative;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(z.read(name));hashes[str(relative)]=sha(path)
   assert len(hashes)==(4 if label=='baseline' else 10)
  write(OUT/(label+'-input.json'),dict(api=api,files=hashes,archive_sha256=digest));archive.unlink()
 for staged_letter in 'ab':
  target=ROOT/f'tmp/java-priority-mixed-{staged_letter}/plugins';target.mkdir(parents=True)
  for letter in 'ab':
   base=ROOT/('tmp/java-priority-staged/plugins' if letter==staged_letter else 'tmp/java-priority-baseline/plugins')
   shutil.copytree(base/f'java-priority-{letter}',target/f'java-priority-{letter}')
def finish():
 OUT.mkdir(exist_ok=True)
 for name in ['java-priority-staged','java-priority-baseline','java-priority-mixed-a','java-priority-mixed-b','java-test']:
  src=ROOT/'tmp'/name
  if src.exists():shutil.copytree(src,OUT/name,dirs_exist_ok=False)
 selected=[]
 for base in ['src/plugins','tools/plugin-translator','wit']:
  selected.extend(p for p in (ROOT/base).rglob('*') if p.is_file() and not any(x in p.relative_to(ROOT/base).parts for x in ['target','.toolchain','__pycache__']) and p.suffix in ['.rs','.java','.py','.c','.h','.wit','.xml','.yml','.json'])
 selected += [ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'.github/workflows/bukkit-priority-staged-checks.yml']
 for p in selected:
  if p.exists():
   d=OUT/'source'/p.relative_to(ROOT);d.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,d)
 rust={str(p.relative_to(ROOT)):sha(p) for p in sorted((ROOT/'src').rglob('*.rs'))};write(OUT/'all-rust-sha256.json',rust)
 files={str(p.relative_to(OUT)):dict(sha256=sha(p),bytes=p.stat().st_size) for p in sorted(OUT.rglob('*')) if p.is_file()}
 write(OUT/'receipt.json',dict(source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),run_id=os.environ.get('GITHUB_RUN_ID'),status=os.environ.get('MCSA_JOB_STATUS'),files=files))
if __name__=='__main__':{'inputs':inputs,'finish':finish}[sys.argv[1]]()
