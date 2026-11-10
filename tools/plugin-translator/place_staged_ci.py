#!/usr/bin/env python3
"""Один bounded E2E: старые WASM immutable/OLD inputs; новые реально переводятся NEW SDK."""
from pathlib import Path
import hashlib,json,os,shutil,signal,subprocess,sys,zipfile
ROOT=Path(__file__).resolve().parents[2]
TOOL=ROOT/'tools/plugin-translator'
OUT=ROOT/'place-staged-evidence'
PINS={
 'reuse-failed':{'id':13755882155,'sha256':'238bb44c7b67834b8dc0c6fade1a98d51ecde1cfd438df33e7794f6dfa98101a','source':'8760e5714f0057d6d7dd88fbb0299acad4cc55de'},
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
 if 'GH_TOKEN' not in os.environ:raise RuntimeError('Для получения CI-артефакта нужен GH_TOKEN в окружении шага')
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
   if name.startswith('source/') and name not in ['source/Cargo.lock','source/plugins-examples/Cargo.lock']:
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
 write(folder/'provenance.json',{'artifact_source':pin['source'],'actual_compile_source':source,'old_source_files':checked,'new_sdk_inputs_equal_old':label=='reuse-failed','old_retranslated':False,'producer_status':receipt.get('status',receipt.get('job_status'))})
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

RETAINED_TEST='mcsheriffanya-628cb65e72047376'
RETAINED_TEST_SHA='b5c46d45b52b6f0a2f8ea6cc24c8fbd002cb8d553e6cbacd9762170d3bfa33c9'
RETAINED_SERVER_SHA='f522a9fc3ea62305127b007d0902356eec5184ee29975e1fcc1ba57ac7ade146'

def restore_runtime():
 # env!(CARGO_MANIFEST_DIR) in retained libtest must resolve to the same absolute checkout.
 assert str(ROOT)=='/home/runner/work/MCSheriffAnya/MCSheriffAnya'
 with archive('reuse-failed') as z:
  receipt=verify_receipt(z,'reuse-failed');assert receipt['status']=='failure'
  complete=json.loads(z.read('complete.json'));assert complete['status']=='failure'
  for name,value in complete['files'].items():
   b=z.read(name);assert digest(b)==value['sha256']and len(b)==value['bytes'],name
  assert not git('diff',PINS['reuse-failed']['source'],'HEAD','--','src','tests','Cargo.toml','plugins-examples','wit','tools/seedmap','tools/protocol-bridge')
  assert digest(z.read('source/Cargo.lock'))==digest((ROOT/'Cargo.lock').read_bytes())
  inherited=json.loads(z.read('retained-runtime.json'));assert inherited['test_elf']==RETAINED_TEST_SHA and inherited['server_elf']==RETAINED_SERVER_SHA
  for name in ['tests-default','tests-no-default','tests-seedmap-default','tests-protocol-doc','rust-examples']:
   status=json.loads(z.read(name+'.status.json'));assert status['exit_code']==0 and not status['timeout']
   folder=OUT/'inherited-876';folder.mkdir(exist_ok=True)
   for suffix in ['.status.json','.log']:(folder/(name+suffix)).write_bytes(z.read(name+suffix))
   (OUT/(name+'.log')).write_bytes(z.read(name+'.log'))
  for name in ['retained-runtime.json','no-default-runtime.json','rust-example-inputs.json']:
   (OUT/name).write_bytes(z.read(name))
  restored=['runtime-elf/mcsheriffanya','test-elf/'+RETAINED_TEST]
  restored += [name for name in receipt['files'] if name.startswith(('no-default-artifacts/','rust-examples/'))]
  for name in restored:
   data=z.read(name);assert digest(data)==receipt['files'][name]['sha256']
   if name.startswith('runtime-elf/'):target=ROOT/'target/release/mcsheriffanya';assert digest(data)==RETAINED_SERVER_SHA
   elif name.startswith('test-elf/'):target=ROOT/'target/release/deps'/RETAINED_TEST;assert digest(data)==RETAINED_TEST_SHA
   else:target=OUT/name
   target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
   if data[:4]==b'\x7fELF':target.chmod(0o755)
  for name in ['hello','misbehave','chat_filter','custom_tablist','modinfo','protect_spawn']:
   target=ROOT/'plugins-examples/target/wasm32-wasip2/release'/(name+'.wasm');target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(z.read('rust-examples/'+name+'.wasm'))
  (ROOT/'plugins-examples/Cargo.lock').write_bytes(z.read('source/plugins-examples/Cargo.lock'))
 write(OUT/'reuse-876.json',{'source':PINS['reuse-failed']['source'],'status':'failure','both_profiles_and_six_rebuilt':False,'inherited_checks':['default3454','nodefault3400','seedmap21','protocol-doc','nodefault-integration3']})

def strict():
 restore_runtime()
 from place_link_helper import dependency_command,validate_command,choose_artifacts,confined_file,integration_command,serde_rlib
 # rustup locates the actual compiler; same resolved executable links newly built dependency closure and harness.
 rustc=Path(subprocess.check_output(['rustup','which','rustc'],text=True).strip()).resolve()
 assert rustc.is_absolute() and rustc.is_file() and os.access(rustc,os.X_OK)
 version=subprocess.check_output([str(rustc),'-vV'],text=True)
 env=os.environ.copy();env['RUSTC']=str(rustc)
 command=dependency_command();validate_command(command) # BEFORE spawning Cargo.
 log=run('serde-deps',command,env)
 raw_graph=[]
 for line in log.splitlines():
  try: row=json.loads(line)
  except ValueError:continue
  if row.get('reason')=='compiler-artifact':raw_graph.append(row)
 write(OUT/'serde-link-raw.json',{'artifacts':raw_graph,'rustc':str(rustc),'rustc_sha256':digest(rustc.read_bytes()),'rustc_vV':version,'command':command})
 rlib,artifacts=choose_artifacts(log);deps=ROOT/'target/release/deps'
 rlib=serde_rlib(rlib,ROOT/'target/release')
 files={}
 for row in artifacts:
  if row['package_id'].startswith('path+'):raise ValueError('unexpected own workspace dependency artifact')
  for name in row['filenames']:
   path=confined_file(name,ROOT/'target/release')
   data=path.read_bytes();files[name]={'sha256':digest(data),'bytes':len(data)}
   target=OUT/'serde-dependencies'/path.relative_to(ROOT/'target/release');target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
 write(OUT/'serde-link.json',{'artifacts':artifacts,'files':files,'rustc':str(rustc),'rustc_sha256':digest(rustc.read_bytes()),'rustc_vV':version,'rlib':str(rlib),'command':command})
 integration=ROOT/'tests/world_startup.rs'
 assert integration.read_bytes()==git('show',PINS['reuse-failed']['source']+':tests/world_startup.rs')
 server=ROOT/'target/release/mcsheriffanya';assert digest(server.read_bytes())==RETAINED_SERVER_SHA
 env['CARGO_BIN_EXE_mcsheriffanya']=str(server)
 binary=ROOT/'target/release/deps/world_startup_retained_default'
 run('integration-default-compile',integration_command(str(rustc),integration,binary,rlib,deps),env)
 result=run('integration-default',[binary]);assert '3 passed; 0 failed;'in result
 write(OUT/'default-integration.json',{'source_sha256':digest(integration.read_bytes()),'source_unmodified':True,'env':{'CARGO_BIN_EXE_mcsheriffanya':str(server)},'server_sha256':digest(server.read_bytes()),'test_elf_sha256':digest(binary.read_bytes()),'tests':3,'standalone_std_only_harness':False,'serde_dependency_linked':True,'core_rebuilt':False})


def translations():
 run('new-fixtures',[sys.executable,'-B',TOOL/'build-place-staged-fixtures.py'],timeout=2400)
 run('new-fault',[sys.executable,'-B',TOOL/'build-place-faults.py'],timeout=1200)
 run('new-invalid',[sys.executable,'-B',TOOL/'build-place-invalid.py'],timeout=1800)

def one(name,test,env=None):
 binary=ROOT/'target/release/deps'/RETAINED_TEST;assert digest(binary.read_bytes())==RETAINED_TEST_SHA
 log=run(name,[binary,test,'--ignored','--exact','--nocapture','--test-threads=1'],env=env,timeout=300)
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
 for name in ['src','tools/plugin-translator','tools/seedmap','tools/protocol-bridge','plugins-examples','tests','wit','.github/workflows']:
  for p in (ROOT/name).rglob('*'):
   if not p.is_file()or any(x in p.relative_to(ROOT/name).parts for x in ['target','.toolchain','__pycache__']):continue
   if p.suffix not in ['.rs','.java','.py','.c','.h','.wit','.xml','.yml','.json','.toml']:continue
   q=OUT/'source'/p.relative_to(ROOT);q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 for name in ['Cargo.toml','Cargo.lock','plugins-examples/Cargo.lock','build.rs']:
  p=ROOT/name
  if p.is_file():q=OUT/'source'/name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 jar_dir=TOOL/'target'
 for p in list(jar_dir.glob('place-staged-*.jar'))+[jar_dir/'mcsa-bukkit.jar']:
  if p.is_file():q=OUT/'new-jars'/p.name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 for p in [*list((ROOT/'target/release/deps').glob('mcsheriffanya-*')),ROOT/'target/release/deps/world_startup_retained_default']:
  if p.is_file()and os.access(p,os.X_OK):q=OUT/'test-elf'/p.name;q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 p=ROOT/'target/release/mcsheriffanya'
 if p.is_file():q=OUT/'runtime-elf/mcsheriffanya';q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 for name in ['hello','misbehave','chat_filter','custom_tablist','modinfo','protect_spawn']:
  p=ROOT/'plugins-examples/target/wasm32-wasip2/release'/(name+'.wasm')
  if p.is_file():q=OUT/'rust-examples'/(name+'.wasm');q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 tools={}
 for name in ['wit-bindgen-0.63.0-x86_64-linux/wit-bindgen','wasi-sdk-34.0-x86_64-linux/bin/clang','wasm-tools-1.262.0-x86_64-linux/wasm-tools']:
  p=TOOL/'.toolchain'/name
  if p.is_file():
   version=subprocess.run([str(p),'--version'],capture_output=True,text=True,check=False)
   tools[name]={'sha256':digest(p.read_bytes()),'bytes':p.stat().st_size,'version':version.stdout,'stderr':version.stderr,'exit_code':version.returncode}
 write(OUT/'toolpins.json',tools)
 host_tools={}
 for executable in ['rustc','cargo']:
  version=subprocess.run([executable,'--version','--verbose'],capture_output=True,text=True,check=False);host_tools[executable]={'stdout':version.stdout,'stderr':version.stderr,'exit_code':version.returncode}
 write(OUT/'host-toolchain.json',host_tools)
 files={str(p.relative_to(OUT)):{'sha256':digest(p.read_bytes()),'bytes':p.stat().st_size}for p in sorted(OUT.rglob('*'))if p.is_file()}
 write(OUT/'receipt.json',{'source':git('rev-parse','HEAD').decode().strip(),'run_id':os.environ.get('GITHUB_RUN_ID'),'attempt':os.environ.get('GITHUB_RUN_ATTEMPT'),'status':os.environ.get('MCSA_JOB_STATUS'),'files':files})
def main():
 if os.environ.get('GITHUB_ACTIONS')!='true':raise SystemExit('Этот сценарий разрешён только в GitHub Actions')
 if os.environ.get('GITHUB_RUN_ATTEMPT')!='1':raise SystemExit('Повторный запуск этого сценария запрещён')
 handlers={'lock':lock,'inputs':inputs,'strict':strict,'translations':translations,'tests':tests,'finish':finish}
 handlers[sys.argv[1]]()
if __name__=='__main__':main()
