#!/usr/bin/env python3
"""Pins и receipt isolated staged BlockBreakEvent: только собственные артефакты."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
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
PRODUCER=(13736169830,'ef99d8235d016900c1487d7de7600fea4c44f316a9c9a501ba7771f57733b1c6','def3439f9fc445d5308cc718ff951f18549c37c0')
TESTS=['translated_bukkit_staged_partial_and_wrong_type_are_not_legacy','translated_bukkit_staged_faults_and_cumulative_native_budgets','translated_bukkit_staged_monitor_epoch_nested_trap_and_empty','translated_bukkit_staged_empty_fuel_quarantines_and_reload_is_fresh','translated_bukkit_staged_priority_contract','translated_bukkit_staged_frames_reject_invalid_order_replay_and_capacity','translated_bukkit_staged_mixed_a_uses_full_legacy_fallback','translated_bukkit_staged_mixed_b_uses_full_legacy_fallback','translated_bukkit_legacy_priority_baseline','translated_bukkit_jars','translated_bukkit_listener_unregistration','translated_bukkit_enchantment_removal_reaches_native_inventory']
def reuse():
 OUT.mkdir(exist_ok=True)
 number,digest,source=PRODUCER
 api=json.loads(subprocess.check_output(['gh','api',f'repos/{REPO}/actions/artifacts/{number}']))
 assert api['digest']=='sha256:'+digest and api['workflow_run']['head_sha']==source and not api['expired']
 archive=Path(os.environ['RUNNER_TEMP'])/'staged-producer.zip'
 with archive.open('wb') as stream:subprocess.run(['gh','api',f'repos/{REPO}/actions/artifacts/{number}/zip'],stdout=stream,check=True)
 assert sha(archive)==digest
 checked=[];components={}
 with zipfile.ZipFile(archive) as z:
  receipt=json.loads(z.read('receipt.json'));assert receipt['source']==source and receipt['status']=='failure'
  for name,value in receipt['files'].items():
   assert '..' not in Path(name).parts and not name.startswith('/')
   content=z.read(name);assert len(content)==value['bytes'] and hashlib.sha256(content).hexdigest()==value['sha256'],name
  for name,value in receipt['files'].items():
   if not name.startswith('source/'):continue
   relative=name.removeprefix('source/')
   exact=relative.startswith(('tools/plugin-translator/mcsa-bukkit/src/main/java/','tools/plugin-translator/src/main/java/','tools/plugin-translator/fixtures/','wit/')) or relative in ['Cargo.toml','tools/plugin-translator/adapter.c','tools/plugin-translator/bridge.h','tools/plugin-translator/pom.xml','tools/plugin-translator/translate.py','tools/plugin-translator/priority_wit.py','tools/plugin-translator/build-priority-fixtures.py','tools/plugin-translator/build-priority-safety.py','tools/plugin-translator/build-priority-faults.py']
   if exact:assert sha(ROOT/relative)==value['sha256'],relative;checked.append(relative)
  assert len(checked)>250
  prefix='java-priority-staged/plugins/'
  target=ROOT/'tmp/java-priority-staged/plugins';assert not target.exists()
  for name,value in receipt['files'].items():
   if not name.startswith(prefix):continue
   relative=Path(name.removeprefix(prefix));assert len(relative.parts)==2 and relative.name in ['plugin.wasm','plugin.toml']
   dest=target/relative;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(z.read(name));components[str(relative)]=value
  assert len(components)==18 and sum(name.endswith('/plugin.wasm') for name in components)==9
  for name in receipt['files']:
   if name.startswith(('sdk-inputs/','own-jars/')) or name=='fault-adapter.c':
    dest=OUT/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(z.read(name))
   if name.startswith('translate-') and name.endswith('.log') or name in ['fixtures.log','safety-fixtures.log','wit.log']:
    dest=OUT/'producer'/name;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(z.read(name))
  assert not (ROOT/'Cargo.lock').exists();(ROOT/'Cargo.lock').write_bytes(z.read('source/Cargo.lock'))
 write(OUT/'reused-staged-producer.json',dict(api=api,producer_source=source,producer_actual_status='failure',failure_was_overly_specific_rejection_message_assertion=True,archive_sha256=digest,all_producer_receipt_files_verified=len(receipt['files']),exact_component_input_sources=checked,components=components,new_translation_performed=False))
 archive.unlink()
def tests():
 results=[]
 for name in TESTS:
  full='plugins::tests::'+name;command=['cargo','test','--release','--locked','--bin','mcsheriffanya',full,'--','--ignored','--exact','--nocapture','--test-threads=1']
  log=OUT/(name+'.log');assert not log.exists()
  with log.open('w') as stream:
   child=subprocess.Popen(command,cwd=ROOT,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True,text=True)
   timed_out=False
   try:code=child.wait(timeout=180)
   except subprocess.TimeoutExpired:
    timed_out=True;os.killpg(child.pid,signal.SIGKILL);code=child.wait()
  text=log.read_text();print(text,flush=True)
  passed=code==0 and ('test '+full+' ...') in text and 'test result: ok. 1 passed; 0 failed; 0 ignored;' in text
  row=dict(test=full,command=command,exit_code=code,timed_out=timed_out,actual_one_test_passed=passed)
  write(OUT/(name+'.status.json'),row);results.append(row);write(OUT/'native-test-results.json',results)
 assert all(row['actual_one_test_passed'] for row in results),results
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
    relative=Path(name.removeprefix(prefix));assert '..' not in relative.parts
    assert (len(relative.parts)==2 and relative.name in ['plugin.toml','plugin.wasm']) or (label=='old-five' and str(relative)=='java-scheduler/data/config.yml'),name
    path=destination/relative;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(z.read(name));hashes[str(relative)]=sha(path)
   assert len(hashes)==(4 if label=='baseline' else 11)
   assert sum(name.endswith('/plugin.wasm') for name in hashes)==(2 if label=='baseline' else 5)
  write(OUT/(label+'-input.json'),dict(api=api,files=hashes,archive_sha256=digest));archive.unlink()
 # Только собственный component: меняются имена optional exports, длины binary строк прежние.
 # Оба malformed bytes проходят wasm-tools validate до проверки host type gate.
 original=(ROOT/'tmp/java-priority-staged/plugins/java-priority-empty/plugin.wasm').read_bytes()
 assert b'\x05phase' in original and b'\x05begin' in original
 variants={
  'missing':original.replace(b'\x05phase',b'\x05phaze'),
  'type':original.replace(b'\x05phase',b'\x05zzzzz').replace(b'\x05begin',b'\x05phase').replace(b'\x05zzzzz',b'\x05begin'),
 }
 for label,content in variants.items():
  target=ROOT/('tmp/java-priority-staged/plugins/java-priority-invalid-'+label);target.mkdir()
  (target/'plugin.wasm').write_bytes(content)
  (target/'plugin.toml').write_text('name = "java-priority-invalid-'+label+'"\nversion = "1.0"\napi = "0.1"\nmain = "plugin.wasm"\n')
  subprocess.run([str(ROOT/'tools/plugin-translator/.toolchain/wasm-tools-1.262.0-x86_64-linux/wasm-tools'),'validate','--features','all',str(target/'plugin.wasm')],check=True)
 write(OUT/'invalid-components.json',{label:hashlib.sha256(value).hexdigest() for label,value in variants.items()})
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
 jars=ROOT/'tools/plugin-translator/target'
 for p in jars.glob('priority-*.jar'):
  target=OUT/'own-jars'/p.name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,target)
 binaries={str(p.relative_to(ROOT)):dict(sha256=sha(p),bytes=p.stat().st_size) for p in (ROOT/'target/release/deps').glob('mcsheriffanya-*') if p.is_file() and os.access(p,os.X_OK)}
 write(OUT/'runtime-test-binaries.json',binaries)
 files={str(p.relative_to(OUT)):dict(sha256=sha(p),bytes=p.stat().st_size) for p in sorted(OUT.rglob('*')) if p.is_file()}
 write(OUT/'receipt.json',dict(source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),run_id=os.environ.get('GITHUB_RUN_ID'),status=os.environ.get('MCSA_JOB_STATUS'),files=files))
if __name__=='__main__':{'reuse':reuse,'inputs':inputs,'tests':tests,'finish':finish}[sys.argv[1]]()
