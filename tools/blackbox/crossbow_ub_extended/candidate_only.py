#!/usr/bin/env python3
"""Один candidate93 против immutable original; без нового original/natural/Cargo."""
import argparse,hashlib,json,os,shutil,subprocess,sys,tarfile,tempfile,traceback,urllib.request,zipfile
from pathlib import Path
import extended_probe as probe
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/blackbox'))
import combat_ci
import record777
PIN=probe.BASE/'candidate-pin.json'
sha=probe.sha;read=probe.read;write=probe.write

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def api(path):
 with urllib.request.urlopen('https://api.github.com/repos/AnyaSheriff/MCSheriffAnya/'+path,timeout=30) as response:return json.load(response)
def validate_api(pin,which):
 run=api('actions/runs/'+str(pin[which+'_run']));artifact=api('actions/artifacts/'+str(pin[which+'_artifact']))
 assert run['status']=='completed' and run['conclusion']=='success' and run['head_sha']==pin[which+'_source']
 assert artifact['workflow_run']['id']==pin[which+'_run'] and artifact['workflow_run']['head_sha']==pin[which+'_source']
 assert artifact['digest']=='sha256:'+pin[which+'_zip_sha256'] and not artifact['expired']
 return dict(run=run,artifact=artifact)

def extract_locked(archive,destination,expected,selected=None):
 assert sha(archive)==expected,'immutable API ZIP SHA mismatch'
 destination.mkdir(exist_ok=False)
 with zipfile.ZipFile(archive) as z:
  for member in z.infolist():
   path=Path(member.filename);assert not path.is_absolute() and '..' not in path.parts and member.is_dir() is False
   if selected is not None and member.filename not in selected:continue
   if member.filename=='offhand-original-evidence/own-source.tar.gz':continue
   target=destination/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(z.read(member))
 return destination

def lifecycle(path,binary):
 life=read(path)
 assert not life['cleanup_errors'] and life['ports_free']=={str(p):True for p in range(26190,26194)}
 assert len(life['servers'])==len(life['cleanup'])==1
 assert life['servers'][0]['label']=='ours' and life['servers'][0]['port']==26192 and life['servers'][0]['argv']==[str(binary.resolve())]
 assert life['cleanup'][0]['exit_code']==0 and life['cleanup'][0]['group_gone']
 return life

def run(output,original_zip,runtime_zip):
 assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('GITHUB_RUN_ATTEMPT')=='1','GitHub first attempt only'
 output.mkdir(exist_ok=False);work=Path(tempfile.mkdtemp(prefix='offhand-candidate93-',dir=os.environ['RUNNER_TEMP']));recordings=work/'recordings'
 reference=None
 pin=read(PIN);source=git('rev-parse','HEAD');result=dict(status='INVALID',helper_source=source,runtime_source=pin['runtime_source'],attempts=1,runtime_executed=False,original_executed=False,natural_executed=False,compiled=False)
 try:
  assert git('rev-parse','HEAD:src')==git('rev-parse',pin['runtime_source']+':src')==git('rev-parse',pin['gameplay_base']+':src')
  assert sha(probe.BASE/'criteria.json')==pin['criteria_sha256']==probe.CRITERIA_SHA256
  assert sha(ROOT/'tools/blackbox/scenarios'/(pin['scenario']+'.json'))==pin['scenario_sha256']
  api_receipts={which:validate_api(pin,which) for which in ['original','runtime']};write(output/'input-api.json',api_receipts)
  original_root=extract_locked(original_zip,work/'original-input',pin['original_zip_sha256']);reference=original_root/'offhand-original-evidence'
  original_files=read(reference/'files.json')
  for name,info in original_files.items():
   if name=='own-source.tar.gz':assert info['sha256']==pin['original_source_archive_sha256'];continue
   assert sha(reference/name)==info['sha256'] and (reference/name).stat().st_size==info['bytes']
  input_proof=dict(api=api_receipts,actual_zip_sha256=dict(original=sha(original_zip),runtime=sha(runtime_zip)))
  proof_path=work/'input-proof.json';write(proof_path,input_proof);write(output/'input-proof.json',input_proof)
  original_snapshot={str(p.relative_to(original_root)):sha(p) for p in original_root.rglob('*') if p.is_file()}
  selected={'ub128-evidence/runtime/mcsheriffanya','ci-logs/receipt.json','ci-logs/Cargo.lock'}
  runtime_root=extract_locked(runtime_zip,work/'runtime-input',pin['runtime_zip_sha256'],selected)
  binary=runtime_root/'ub128-evidence/runtime/mcsheriffanya';binary_receipt=read(runtime_root/'ci-logs/receipt.json')
  assert sha(binary)==pin['runtime_sha256']==binary_receipt['runtime_sha256'] and binary_receipt['source']==pin['runtime_source'] and binary_receipt['status']=='success'
  assert binary.read_bytes()[:6]==b'\x7fELF\x02\x01' and sha(runtime_root/'ci-logs/Cargo.lock')==pin['runtime_lock_sha256']
  binary.chmod(0o500);assert binary.stat().st_mode&0o777==0o500
  shutil.copy2(runtime_root/'ci-logs/receipt.json',output/'runtime-build-receipt.json');shutil.copy2(runtime_root/'ci-logs/Cargo.lock',output/'runtime-Cargo.lock')
  data=reference/'data';source_pin=read(reference/'approved-source-pin.json')
  for entry in source_pin['json_files']:
   if (data/entry['file']).exists():assert sha(data/entry['file'])==entry['sha256'] and (data/entry['file']).stat().st_size==entry['bytes']
  args=argparse.Namespace(causal_candidate_reference=reference,causal_candidate_input_proof=proof_path,data=data,binary=binary,version=777,isolate_vitals=True)
  original,original_sha=record777.checked_causal_candidate_reference(args,probe.scenario('offhand-commandloaded',ROOT))
  original_observation=probe.inspect('offhand-commandloaded',original,data,ROOT)
  assert json.loads(json.dumps(original_observation))==read(reference/'FIRST-ORIGINAL/observation.json') and original_observation['statistics']['statistical_pass']
  combat_ci.fetch(source_pin['official_jar'],work/'server.jar') # Только opaque hash input старого CLI, Java не запускается.
  command=[sys.executable,'-B',str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(recordings),'--binary',str(binary),'--jar',str(work/'server.jar'),'--official-port','26190','--core-port','26192','--max-seconds','600','--core-only','--isolate-vitals','--causal-menu-readiness','--causal-candidate-reference',str(reference),'--causal-candidate-input-proof',str(proof_path),pin['scenario']]
  write(output/'command.json',command)
  with (output/'recorder.log').open('w') as log:
   process=subprocess.Popen(command,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
   try:code=process.wait(timeout=660)
   except subprocess.TimeoutExpired:
    process.terminate()
    try:process.wait(timeout=25)
    except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
    raise
  assert code==0,'candidate INVALID; retain without retry'
  lifecycle(recordings/'lifecycle.json',binary)
  assert not(recordings/'official').exists() and not list(recordings.glob('*-official*.json'))
  assert original_snapshot=={str(p.relative_to(original_root)):sha(p) for p in original_root.rglob('*') if p.is_file()}
  assert sha(binary)==pin['runtime_sha256']
  raw_path=recordings/(pin['scenario']+'-ours.json');raw=read(raw_path)
  assert raw['capture_side']=='ours' and raw['phase_alignment']['reference_sha256']==pin['original_raw_sha256']
  observation=probe.inspect('offhand-commandloaded',raw,data,ROOT);write(output/'candidate-observation.json',observation)
  negatives=probe.negative_raw_controls('offhand-commandloaded',raw,data,ROOT);write(output/'negative-controls.json',negatives)
  write(output/'auxiliary-status55-OPEN.json',probe.report_auxiliary(original_observation,observation))
  shutil.copytree(reference/'FIRST-ORIGINAL',output/'immutable-original')
  target=output/'data';target.mkdir()
  for f in data.glob('*.json'):shutil.copy2(f,target/f.name)
  shutil.copy2(reference/'scenario.json',output/'scenario.json');shutil.copy2(reference/'criteria.json',output/'criteria.json');shutil.copy2(reference/'capture-receipt.json',output/'original-capture-receipt.json')
  result.update(status='candidate93-bounded-statistical-PASS' if observation['statistics']['statistical_pass'] else 'candidate93-statistical-FAIL',runtime_sha256=sha(binary),runtime_executed=True,runtime_mode='0500',original_raw_sha256=original_sha,candidate_raw_sha256=sha(raw_path),original_statistics=original_observation['statistics'],candidate_statistics=observation['statistics'],negative_controls=negatives,lifecycle_clean=True,original_input_unchanged=True,core_game_src_exact_merged_e3=True,integrated_e3_runtime_claim=False)
 except BaseException as error:
  result['error']=repr(error);(output/'error.txt').write_text(traceback.format_exc());raise
 finally:
  if reference is not None and reference.exists():
   if not(output/'immutable-original').exists():shutil.copytree(reference/'FIRST-ORIGINAL',output/'immutable-original')
   if not(output/'data').exists():shutil.copytree(reference/'data',output/'data')
   for name in ['scenario.json','criteria.json']:
    if not(output/name).exists():shutil.copy2(reference/name,output/name)
  if (recordings/'lifecycle.json').exists():
   result['runtime_executed']=any(row['label']=='ours' for row in read(recordings/'lifecycle.json')['servers'])
  if recordings.exists():
   target=output/'recordings';target.mkdir(exist_ok=True)
   for f in recordings.glob('*.json'):shutil.copy2(f,target/f.name)
   if (recordings/'ours/server.log').exists():shutil.copy2(recordings/'ours/server.log',output/'ours-server.log')
  selected_source=['tools/blackbox/record777.py',*(str(p.relative_to(ROOT)) for p in probe.BASE.glob('*') if p.is_file()),'.github/workflows/crossbow-offhand-candidate.yml']
  with tarfile.open(output/'selected-own-source.tar.gz','w:gz') as archive:
   for name in selected_source:archive.add(ROOT/name,arcname=name)
  write(output/'candidate-pin.json',pin);write(output/'result.json',result)
  write(output/'files.json',{str(p.relative_to(output)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in output.rglob('*') if p.is_file() and p.name!='files.json'})
 if result['status']!='candidate93-bounded-statistical-PASS':raise RuntimeError('statistical FAIL retained, no retries')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--original-zip',type=Path,required=True);p.add_argument('--runtime-zip',type=Path,required=True);a=p.parse_args();run(a.output.resolve(),a.original_zip.resolve(),a.runtime_zip.resolve())
