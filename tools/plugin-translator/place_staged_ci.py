#!/usr/bin/env python3
"""Один bounded E2E: старые WASM immutable/OLD inputs; новые реально переводятся NEW SDK."""
from pathlib import Path
import hashlib,json,os,shutil,signal,subprocess,sys,zipfile
ROOT=Path(__file__).resolve().parents[2]
TOOL=ROOT/'tools/plugin-translator'
OUT=ROOT/'place-staged-evidence'
PINS={
 'place-baseline':{'id':13742938896,'sha256':'137e101ee864de14551f8b198b4d01526375096c5d9812cf5d477c168ecde714','source':'f141103e54860a40c2fe26eb40af8e659e8fff82'},
 'break-staged':{'id':13736169830,'sha256':'ef99d8235d016900c1487d7de7600fea4c44f316a9c9a501ba7771f57733b1c6','source':'def3439f9fc445d5308cc718ff951f18549c37c0'},
 'break-baseline':{'id':13707523078,'sha256':'87e30ea72931475940fe4d802425ba2db0ff47d8c26b7d2817a65e501707b7aa','source':'354a0d137b60dbd89d32246c3e0aef2ddbd062c1'},
 'old-five':{'id':13703641973,'sha256':'b2daaafb8753161205289123fe81be2be4ddd92234c0c887c813608f002586b6','source':'a76b0d755a9e4830b0759e7d72563cc667efeda5','actual_compile_source':'e9cf7cdd67b4f2f9b883d5eab54343ac5fdc97dd'},
}
def digest(b):return hashlib.sha256(b).hexdigest()
def write(p,x):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(x,indent=2)+'\n')
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
def run(name,args,env=None,timeout=1800):
 OUT.mkdir(exist_ok=True);log=OUT/(name+'.log');assert not log.exists()
 with log.open('w') as stream:
  child=subprocess.Popen([str(x)for x in args],cwd=ROOT,env=env,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
  timed=False
  try:code=child.wait(timeout=timeout)
  except subprocess.TimeoutExpired:timed=True;os.killpg(child.pid,signal.SIGKILL);code=child.wait()
 row={'command':[str(x)for x in args],'exit_code':code,'timeout':timed,'timeout_seconds':timeout};write(OUT/(name+'.status.json'),row)
 print(name,code,flush=True)
 assert code==0 and not timed,row
 return log.read_text()
def archive(label):
 pin=PINS[label];folder=OUT/'old-inputs'/label;folder.mkdir(parents=True,exist_ok=True);path=folder/'artifact.zip'
 if not path.exists():
  api=json.loads(subprocess.check_output(['gh','api',f'repos/AnyaSheriff/MCSheriffAnya/actions/artifacts/{pin["id"]}']))
  assert api['id']==pin['id'] and api['digest']=='sha256:'+pin['sha256'] and api['workflow_run']['head_sha']==pin['source'] and not api['expired']
  write(folder/'api.json',api)
  with path.open('wb') as stream:subprocess.run(['gh','api',f'repos/AnyaSheriff/MCSheriffAnya/actions/artifacts/{pin["id"]}/zip'],stdout=stream,check=True)
 assert digest(path.read_bytes())==pin['sha256']
 z=zipfile.ZipFile(path)
 for name in z.namelist():assert not name.startswith('/') and '..' not in Path(name).parts
 return z

def verify_receipt(z,label):
 pin=PINS[label];source=pin['source'];folder=OUT/'old-inputs'/label
 if label=='old-five':
  receipt=json.loads(z.read('reused/plugins/plugin-evidence/receipt.json'));assert receipt['source']==pin['actual_compile_source'] and receipt['status']=='failure'
  compact=json.loads(z.read('compact-files.json'))
  for name,v in compact.items():
   b=z.read(name);assert len(b)==v['bytes'] and digest(b)==v['sha256'],name
  inputs=receipt['own_sources_sha256'];source=receipt['source']
  assert digest(z.read('reused/plugins/plugin-evidence/mcsa-bukkit.jar'))==receipt['sdk_sha256']
  for name,v in receipt['components'].items():
   b=z.read('reused/plugins/'+name);assert digest(b)==v['sha256'] and len(b)==v['bytes']
  for name in ['compact-files.json','input-audit.json','recheck-result.json','reused/plugins/plugin-evidence/receipt.json']:
   p=folder/'proof'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(z.read(name))
 else:
  receipt=json.loads(z.read('receipt.json'));assert receipt['source']==source
  for name,v in receipt['files'].items():
   b=z.read(name);assert len(b)==v['bytes'] and digest(b)==v['sha256'],name
  inputs={}
  for name,v in receipt['files'].items():
   if name.startswith('source/') and name!='source/Cargo.lock':
    relative=name.removeprefix('source/');assert z.read(name)==git('show',source+':'+relative),name;inputs[relative]=v['sha256']
  if label=='place-baseline':
   assert receipt['job_status']=='success'
   # Snapshot source index covers additional own source blobs absent from source/ subset.
   for name,value in json.loads(z.read('source-sha256.json')).items():
    if name!='Cargo.lock':assert digest(git('show',source+':'+name))==value;inputs[name]=value
  if label=='break-staged':assert receipt['status']=='failure' # Original failed test-message expectation remains failure.
  p=folder/'proof/receipt.json';p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(z.read('receipt.json'))
 checked={}
 for name,value in inputs.items():
  b=git('show',source+':'+name);assert digest(b)==value,name
  p=folder/'old-source'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b);checked[name]=value
 assert len(checked)>100
 write(folder/'provenance.json',{'artifact_source':pin['source'],'actual_compile_source':source,'old_source_files':checked,'new_sdk_inputs_equal_old':False,'old_retranslated':False,'producer_status':receipt.get('status',receipt.get('job_status'))})
 return receipt

def install(z,prefix,destination,count):
 assert not destination.exists();files={}
 for name in z.namelist():
  if name.startswith(prefix) and not name.endswith('/'):
   rel=Path(name.removeprefix(prefix));assert '..' not in rel.parts
   assert (len(rel.parts)==2 and rel.name in ['plugin.wasm','plugin.toml']) or str(rel)=='java-scheduler/data/config.yml',name
   b=z.read(name)
   if rel.name=='plugin.wasm':assert b[:8]==b'\0asm\r\0\1\0'
   target=destination/rel;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(b);files[str(rel)]=digest(b)
 assert sum(n.endswith('/plugin.wasm')for n in files)==count
 return files

def lock():
 OUT.mkdir(exist_ok=True)
 freeze=json.loads((TOOL/'fixtures/place-staged-frozen.json').read_text())
 for f,h in freeze['files'].items():assert digest((ROOT/f).read_bytes())==h,f
 assert (TOOL/'fixtures/place-old-expected.json').read_bytes()==git('show',PINS['place-baseline']['source']+':tools/plugin-translator/fixtures/place-priority-expected.json')
 with archive('place-baseline') as z:
  verify_receipt(z,'place-baseline');data=z.read('source/Cargo.lock');assert digest(data)=='6c1f0a7abf94e252585096af69c54e1e757c5aa199859d9470de21843ec701b2'
  assert not (ROOT/'Cargo.lock').exists();(ROOT/'Cargo.lock').write_bytes(data)
 write(OUT/'frozen.json',freeze)

def inputs():
 for label,prefix,dest,count in [
  ('place-baseline','native/plugins/','place-old-baseline',2),
  ('break-staged','java-priority-staged/plugins/','java-priority-staged',9),
  ('break-baseline','native/plugins/','java-priority-baseline',2),
  ('old-five','reused/plugins/tmp/java-test/plugins/','java-test',5)]:
  with archive(label) as z:
   verify_receipt(z,label);files=install(z,prefix,ROOT/'tmp'/dest/'plugins',count)
   write(OUT/'old-inputs'/label/'installed.json',files)
 for letter in 'ab':
  target=ROOT/f'tmp/java-priority-mixed-{letter}/plugins';target.mkdir(parents=True)
  for member in 'ab':
   src=ROOT/('tmp/java-priority-staged/plugins'if member==letter else 'tmp/java-priority-baseline/plugins')/f'java-priority-{member}'
   shutil.copytree(src,target/f'java-priority-{member}')
 write(OUT/'pins.json',PINS)

def strict():
 # Failed42787380610 keeps the newly broadened no-default -Dwarnings result.
 # Its pre-existing dead_code/unused_imports remain OPEN; no suppression/source cleanup.
 # Required default strict workspace + default/no-default tests stay mandatory.
 commands=[
 ('clippy-default',['cargo','clippy','--release','--locked','--workspace','--all-targets','--','-D','warnings']),
 ('tests-default',['cargo','test','--release','--locked','--workspace']),
 ('tests-no-default',['cargo','test','--release','--locked','--workspace','--no-default-features']),
 ('build',['cargo','build','--release','--locked'])]
 for name,args in commands:run(name,args,timeout=1800)

def translations():
 run('new-fixtures',[sys.executable,'-B',TOOL/'build-place-staged-fixtures.py'],timeout=2400)
 run('new-fault',[sys.executable,'-B',TOOL/'build-place-faults.py'],timeout=1200)
 run('new-invalid',[sys.executable,'-B',TOOL/'build-place-invalid.py'],timeout=1800)

def one(name,test,env=None):
 log=run(name,['cargo','test','--release','--locked','--bin','mcsheriffanya',test,'--','--ignored','--exact','--nocapture','--test-threads=1'],env=env,timeout=300)
 assert 'running 1 test' in log and 'test result: ok. 1 passed; 0 failed; 0 ignored;' in log,name

def tests():
 # Exact immutable old WASMs test CURRENT runtime, never current compiler-input equality.
 from priority_staged_ci import TESTS
 for test in TESTS:one('old-'+test,'plugins::tests::'+test)
 for test in ['translated_bukkit_place_public_and_whole_legacy_fallback','translated_bukkit_place_frames_two_states_order_replay_capacity','translated_bukkit_place_faults_retain_both_flags_and_finish_survivors','translated_bukkit_place_partial_and_wrong_type_are_rejected']:
  one('new-'+test,'plugins::tests::'+test)
 new=json.loads((TOOL/'fixtures/place-staged-cases.json').read_text());old=json.loads((TOOL/'fixtures/place-old-expected.json').read_text())
 old_cases={'host':[x['name']for x in old['cohorts']],'native':[x['name']for x in old['cohorts'][:6]]+['offhand-cancel','offhand-uncancel','creative-order','creative-cancel']}
 for legacy,matrix in [(False,new),(True,old_cases)]:
  for kind in ['host','native']:
   for case in matrix[kind]:
    env=os.environ.copy();env['MCSA_PLACE_CASE']=kind+':'+case;env['MCSA_PLACE_LEGACY']='1'if legacy else'0'
    one(('legacy-'if legacy else'new-')+kind+'-'+case,'network::play::place_plugin_staged_tests::place_staged_cohort',env)

def finish():
 OUT.mkdir(exist_ok=True)
 for name in ['place-staged','place-old-baseline','java-priority-staged','java-priority-baseline','java-priority-mixed-a','java-priority-mixed-b','java-test']:
  p=ROOT/'tmp'/name
  if p.exists():shutil.copytree(p,OUT/'runtime'/name)
 for name in ['src','tools/plugin-translator','tools/seedmap','tools/protocol-bridge','wit','.github/workflows']:
  for p in (ROOT/name).rglob('*'):
   if not p.is_file()or any(x in p.relative_to(ROOT/name).parts for x in ['target','.toolchain','__pycache__']):continue
   if p.suffix not in ['.rs','.java','.py','.c','.h','.wit','.xml','.yml','.json','.toml']:continue
   q=OUT/'source'/p.relative_to(ROOT);q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 for name in ['Cargo.toml','Cargo.lock','build.rs']:
  p=ROOT/name
  if p.is_file():q=OUT/'source'/name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 jar_dir=TOOL/'target'
 for p in list(jar_dir.glob('place-staged-*.jar'))+[jar_dir/'mcsa-bukkit.jar']:
  if p.is_file():q=OUT/'new-jars'/p.name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 for p in (ROOT/'target/release/deps').glob('mcsheriffanya-*'):
  if p.is_file()and os.access(p,os.X_OK):q=OUT/'test-elf'/p.name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 p=ROOT/'target/release/mcsheriffanya'
 if p.is_file():q=OUT/'runtime-elf/mcsheriffanya';q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 tools={}
 for name in ['wit-bindgen-0.63.0-x86_64-linux/wit-bindgen','wasi-sdk-34.0-x86_64-linux/bin/clang','wasm-tools-1.262.0-x86_64-linux/wasm-tools']:
  p=TOOL/'.toolchain'/name
  if p.is_file():
   version=subprocess.run([str(p),'--version'],capture_output=True,text=True,check=False)
   tools[name]={'sha256':digest(p.read_bytes()),'bytes':p.stat().st_size,'version':version.stdout,'stderr':version.stderr,'exit_code':version.returncode}
 write(OUT/'toolpins.json',tools)
 files={str(p.relative_to(OUT)):{'sha256':digest(p.read_bytes()),'bytes':p.stat().st_size}for p in sorted(OUT.rglob('*'))if p.is_file()}
 write(OUT/'receipt.json',{'source':git('rev-parse','HEAD').decode().strip(),'run_id':os.environ.get('GITHUB_RUN_ID'),'attempt':os.environ.get('GITHUB_RUN_ATTEMPT'),'status':os.environ.get('MCSA_JOB_STATUS'),'files':files})
if __name__=='__main__':{'lock':lock,'inputs':inputs,'strict':strict,'translations':translations,'tests':tests,'finish':finish}[sys.argv[1]]()
