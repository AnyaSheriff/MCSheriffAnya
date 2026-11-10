#!/usr/bin/env python3
"""Один candidate93 offhand-natural против exactsuccessfulreference; безoriginal/build."""
import argparse,json,os,shutil,subprocess,sys,tarfile,tempfile,traceback
from pathlib import Path
import extended_probe as probe
import candidate_only as infrastructure
import offhand_natural_reference as admission
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/blackbox'))
import combat_ci
import record777
PIN=admission.PIN
GROUP='offhand-natural'
sha=probe.sha;read=probe.read;write=probe.write

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def run(output,original_zip,runtime_zip):
 assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('GITHUB_RUN_ATTEMPT')=='1','GitHub first attempt only'
 output.mkdir(exist_ok=False);work=Path(tempfile.mkdtemp(prefix='offhand-natural-candidate93-',dir=os.environ['RUNNER_TEMP']));recordings=work/'recordings';reference=None
 pin=read(PIN);source=git('rev-parse','HEAD')
 result=dict(status='INVALID',helper_source=source,runtime_source=pin['runtime_source'],attempts=1,runtime_executed=False,original_executed=False,mainhand_natural_executed=False,compiled=False,original_CI_run=pin['original_run'],original_CI_conclusion='success')
 try:
  assert sha(ROOT/'tools/blackbox/record777.py')==pin['candidate_recorder_sha256']
  assert GROUP=='offhand-natural' and git('rev-parse','HEAD:src')==git('rev-parse',pin['runtime_source']+':src')==git('rev-parse',pin['gameplay_base']+':src')
  assert sha(probe.BASE/'extended_probe.py')==pin['accepted_fixed_parser_sha256'] and sha(probe.BASE/'criteria.json')==pin['criteria_sha256']
  assert sha(ROOT/'tools/blackbox/scenarios'/(pin['scenario']+'.json'))==pin['scenario_sha256']
  api={which:dict(run=infrastructure.api('actions/runs/'+str(pin[which+'_run'])),artifact=infrastructure.api('actions/artifacts/'+str(pin[which+'_artifact']))) for which in ['original','runtime']}
  proof=dict(api=api,actual_zip_sha256=dict(original=sha(original_zip),runtime=sha(runtime_zip)))
  admission.validate_input_api_and_zip(proof,pin);write(output/'input-proof.json',proof);proof_path=work/'input-proof.json';write(proof_path,proof)
  original_root=infrastructure.extract_locked(original_zip,work/'original-input',pin['original_zip_sha256']);reference=original_root/'offhand-natural-original-evidence'
  original_snapshot={str(p.relative_to(original_root)):sha(p) for p in original_root.rglob('*') if p.is_file()}
  selected={'ub128-evidence/runtime/mcsheriffanya','ci-logs/receipt.json','ci-logs/Cargo.lock'}
  runtime_root=infrastructure.extract_locked(runtime_zip,work/'runtime-input',pin['runtime_zip_sha256'],selected);binary=runtime_root/'ub128-evidence/runtime/mcsheriffanya';build_receipt=read(runtime_root/'ci-logs/receipt.json')
  assert sha(binary)==pin['runtime_sha256']==build_receipt['runtime_sha256'] and build_receipt['source']==pin['runtime_source'] and build_receipt['status']=='success'
  assert sha(runtime_root/'ci-logs/Cargo.lock')==pin['runtime_lock_sha256'] and binary.read_bytes()[:6]==b'\x7fELF\x02\x01'
  binary.chmod(0o500);assert binary.stat().st_mode&0o777==0o500
  shutil.copy2(runtime_root/'ci-logs/receipt.json',output/'runtime-build-receipt.json');shutil.copy2(runtime_root/'ci-logs/Cargo.lock',output/'runtime-Cargo.lock')
  raw_original,original_sha,original_observation,lineage=admission.load_reference(reference,proof,ROOT,binary)
  write(output/'original-import-lineage.json',lineage);write(output/'original-readonly-reanalysis.json',original_observation)
  data=reference/'data';source_pin=read(reference/'approved-source-pin.json')
  for entry in source_pin['json_files']:
   if (data/entry['file']).exists():assert sha(data/entry['file'])==entry['sha256'] and (data/entry['file']).stat().st_size==entry['bytes']
  combat_ci.fetch(source_pin['official_jar'],work/'server.jar') # Opaque hash inputCLI, Java вcandidate-only не запускается.
  args=argparse.Namespace(causal_offhand_natural_reference=reference,causal_offhand_natural_input_proof=proof_path,data=data,binary=binary)
  _,checked_sha=record777.checked_offhand_natural_candidate_reference(args,probe.scenario(GROUP,ROOT));assert checked_sha==original_sha
  command=[sys.executable,'-B',str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(recordings),'--binary',str(binary),'--jar',str(work/'server.jar'),'--official-port','26190','--core-port','26192','--max-seconds','900','--core-only','--isolate-vitals','--causal-menu-readiness','--causal-offhand-natural-reference',str(reference),'--causal-offhand-natural-input-proof',str(proof_path),pin['scenario']]
  write(output/'command.json',command)
  with (output/'recorder.log').open('w') as log:
   process=subprocess.Popen(command,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
   try:code=process.wait(timeout=960)
   except subprocess.TimeoutExpired:
    process.terminate()
    try:process.wait(timeout=25)
    except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
    raise
  assert code==0,'candidate INVALID; retain without retry'
  infrastructure.lifecycle(recordings/'lifecycle.json',binary)
  assert not(recordings/'official').exists() and not list(recordings.glob('*-official*.json'))
  assert original_snapshot=={str(p.relative_to(original_root)):sha(p) for p in original_root.rglob('*') if p.is_file()}
  assert sha(binary)==pin['runtime_sha256'] and binary.stat().st_mode&0o777==0o500
  raw_path=recordings/(pin['scenario']+'-ours.json');raw=read(raw_path)
  assert raw['capture_side']=='ours' and raw['phase_alignment']['reference_sha256']==pin['original_raw_sha256']
  manifests=list(recordings.glob('run-*.json'));assert len(manifests)==1;m=read(manifests[0]);assert m['requested_capture_sides']==['ours'] and m['binary_sha256']==pin['runtime_sha256'] and m['scenario_sha256']=={pin['scenario']:pin['scenario_sha256']}
  assert m['recorder_sha256']==raw['recorder_sha256']==sha(ROOT/'tools/blackbox/record777.py') and m['data_sha256']=={p.name:sha(p) for p in data.glob('*.json')}
  assert m['causal_offhand_natural_reference']['original_CI_conclusion']=='success'
  observation=probe.inspect(GROUP,raw,data,ROOT);admission.validate_offhand_using_flags(raw);write(output/'candidate-observation.json',observation)
  negatives=probe.negative_raw_controls(GROUP,raw,data,ROOT);write(output/'negative-controls.json',negatives);write(output/'auxiliary-status-OPEN.json',probe.report_auxiliary(original_observation,observation))
  result.update(status='candidate93-offhand-natural-bounded-PASS' if observation['statistics']['statistical_pass'] else 'candidate93-offhand-natural-statistical-FAIL',runtime_executed=True,runtime_sha256=sha(binary),runtime_mode='0500',original_raw_sha256=original_sha,candidate_raw_sha256=sha(raw_path),original_statistics=original_observation['statistics'],candidate_statistics=observation['statistics'],original_input_unchanged=True,import_mode='exact-successful-first-original',negative_controls=negatives,lifecycle_clean=True,not_fresh_e3_runtime=True)
 except BaseException as error:
  result['error']=repr(error);(output/'error.txt').write_text(traceback.format_exc());raise
 finally:
  if reference is not None and reference.exists():shutil.copytree(reference,output/'immutable-first-original')
  if recordings.exists():
   target=output/'recordings';target.mkdir(exist_ok=True)
   for p in recordings.glob('*.json'):shutil.copy2(p,target/p.name)
   if (recordings/'ours/server.log').exists():shutil.copy2(recordings/'ours/server.log',output/'ours-server.log')
  if (recordings/'lifecycle.json').exists():result['runtime_executed']=any(row['label']=='ours' for row in read(recordings/'lifecycle.json')['servers'])
  selected_source=['tools/blackbox/record777.py',*(str(p.relative_to(ROOT)) for p in probe.BASE.glob('*') if p.is_file()),'.github/workflows/crossbow-offhand-natural-candidate.yml']
  with tarfile.open(output/'selected-own-source.tar.gz','w:gz') as archive:
   for name in selected_source:archive.add(ROOT/name,arcname=name)
  write(output/'pin.json',pin);write(output/'result.json',result)
  write(output/'files.json',{str(p.relative_to(output)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in output.rglob('*') if p.is_file() and p.name!='files.json'})
 if result['status']!='candidate93-offhand-natural-bounded-PASS':raise RuntimeError('statistical FAIL retained; no retry')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--original-zip',type=Path,required=True);p.add_argument('--runtime-zip',type=Path,required=True);a=p.parse_args();run(a.output.resolve(),a.original_zip.resolve(),a.runtime_zip.resolve())
