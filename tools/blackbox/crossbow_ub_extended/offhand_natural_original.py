#!/usr/bin/env python3
"""Один original offhand-natural128+4: genuine charge, без core/mainhand-natural/build."""
import argparse,hashlib,json,os,shutil,subprocess,sys,tarfile,tempfile,traceback
from pathlib import Path
import extended_probe as probe
import candidate_only as inputs
import original_first
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/blackbox'))
import combat_ci
PIN=probe.BASE/'offhand-natural-original-pin.json'
GROUP='offhand-natural'
sha=probe.sha;read=probe.read;write=probe.write

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def freeze_first(output,raw_path,observation,capture,pin):
 assert not output.exists(),'first complete offhand-natural original cannot be overwritten'
 assert capture['requested_capture_sides']==['official'] and capture['criteria_sha256']==pin['criteria_sha256'] and capture['scenario_sha256']==pin['scenario_sha256']
 assert capture['recorder_sha256']==pin['recorder_sha256'] and capture['raw_sha256']==sha(raw_path)
 assert capture['lifecycle_clean'] and not capture['core_executed']
 output.mkdir(exist_ok=False);shutil.copy2(raw_path,output/'original-raw.json');shutil.copy2(probe.BASE/'criteria.json',output/'criteria.json')
 write(output/'observation.json',observation)
 write(output/'original-first-pin.json',dict(group=GROUP,capture_source=capture['source'],gameplay_base=pin['gameplay_base'],raw_sha256=sha(raw_path),criteria_sha256=pin['criteria_sha256'],scenario_sha256=pin['scenario_sha256'],first_complete_original=True,statistical_pass=observation['statistics']['statistical_pass'],no_retry_for_green=True,historical_mainhand_commandloaded_PRIMARY_unchanged=True))

def run(output,runtime_zip):
 assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('GITHUB_RUN_ATTEMPT')=='1','GitHub first attempt only'
 output.mkdir(exist_ok=False);work=Path(tempfile.mkdtemp(prefix='offhand-natural-original-',dir=os.environ['RUNNER_TEMP']));recordings=work/'recordings'
 pin=read(PIN);result=dict(status='INVALID',group=GROUP,source=git('rev-parse','HEAD'),attempts=1,core_executed=False,mainhand_natural_executed=False,compiled=False)
 try:
  assert GROUP=='offhand-natural' and pin['N']==128 and pin['fresh_controls']==4
  assert git('rev-parse','HEAD:src')==git('rev-parse',pin['gameplay_base']+':src')
  assert sha(probe.BASE/'criteria.json')==probe.CRITERIA_SHA256==pin['criteria_sha256']
  assert sha(ROOT/'tools/blackbox/record777.py')==pin['recorder_sha256'],'reuse exact already-tested recorder'
  scenario=ROOT/'tools/blackbox/scenarios'/(pin['scenario']+'.json');assert sha(scenario)==pin['scenario_sha256']
  generated=(json.dumps(probe.scenario(GROUP,ROOT),ensure_ascii=False,indent=2)+'\n').encode();assert generated==scenario.read_bytes()
  provenance=inputs.validate_api(pin,'runtime');assert sha(runtime_zip)==pin['runtime_zip_sha256'];write(output/'runtime-input-API-ZIP.json',dict(api=provenance,actual_zip_sha256=sha(runtime_zip)))
  selected={'ub128-evidence/runtime/mcsheriffanya','ci-logs/receipt.json','ci-logs/Cargo.lock'}
  root=inputs.extract_locked(runtime_zip,work/'prebuilt-identity',pin['runtime_zip_sha256'],selected);binary=root/'ub128-evidence/runtime/mcsheriffanya';receipt=read(root/'ci-logs/receipt.json')
  assert sha(binary)==pin['runtime_sha256']==receipt['runtime_sha256'] and receipt['source']==pin['runtime_source'] and receipt['status']=='success'
  assert sha(root/'ci-logs/Cargo.lock')==pin['runtime_lock_sha256'];binary.chmod(0o400)
  assert binary.stat().st_mode&0o777==0o400
  shutil.copy2(root/'ci-logs/receipt.json',output/'prebuilt-build-receipt.json');shutil.copy2(root/'ci-logs/Cargo.lock',output/'prebuilt-Cargo.lock')
  source_pin=read(ROOT/'tools/blackbox/ci/protection-edges-777.json');write(output/'approved-source-pin.json',source_pin)
  data=work/'data';data.mkdir();combat_ci.fetch(source_pin['official_jar'],work/'server.jar')
  for entry in source_pin['json_files']:
   if entry['file'] in ['protocol.json','items.json','entities.json','blocks.json']:combat_ci.fetch(entry,data/entry['file'])
  shutil.copytree(data,output/'data');shutil.copy2(scenario,output/'scenario.json');shutil.copy2(probe.BASE/'criteria.json',output/'criteria.json')
  command=[sys.executable,'-B',str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(recordings),'--binary',str(binary),'--jar',str(work/'server.jar'),'--java','java','--official-port','26190','--core-port','26192','--max-seconds','900','--official-only','--isolate-vitals','--causal-menu-readiness','--causal-original-reference',pin['scenario']]
  write(output/'command.json',command)
  with (output/'recorder.log').open('w') as log:
   process=subprocess.Popen(command,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
   try:code=process.wait(timeout=960)
   except subprocess.TimeoutExpired:
    process.terminate()
    try:process.wait(timeout=25)
    except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
    raise
  assert code==0,'first natural original INVALID; stop without retry'
  original_first.validate_lifecycle(recordings/'lifecycle.json')
  assert sha(binary)==pin['runtime_sha256'] and binary.stat().st_mode&0o777==0o400
  assert not(recordings/'ours').exists() and not list(recordings.glob('*-ours*.json'))
  manifests=list(recordings.glob('run-*.json'));assert len(manifests)==1;m=read(manifests[0]);raw_path=recordings/(pin['scenario']+'-official.json');raw=read(raw_path)
  assert m['requested_capture_sides']==['official'] and m['causal_original_reference'] and m['scenario_sha256']=={pin['scenario']:pin['scenario_sha256']}
  assert m['binary_sha256']==pin['runtime_sha256'] and m['jar_sha1']==source_pin['official_jar']['sha1'] and m['recorder_sha256']==pin['recorder_sha256']
  assert m['data_sha256']=={p.name:sha(p) for p in data.glob('*.json')}
  assert raw['capture_side']=='official' and raw['recorder_sha256']==pin['recorder_sha256']
  capture=dict(source=result['source'],gameplay_base=pin['gameplay_base'],core_tree=git('rev-parse','HEAD:src'),requested_capture_sides=['official'],raw_sha256=sha(raw_path),criteria_sha256=pin['criteria_sha256'],scenario_sha256=pin['scenario_sha256'],recorder_sha256=pin['recorder_sha256'],jar_sha1=m['jar_sha1'],runtime_identity_source=pin['runtime_source'],runtime_identity_sha256=pin['runtime_sha256'],runtime_identity_mode='0400',core_executed=False,lifecycle_clean=True)
  write(output/'capture-receipt.json',capture)
  observation=probe.inspect(GROUP,raw,data,ROOT)
  freeze_first(output/'FIRST-ORIGINAL',raw_path,observation,capture,pin)
  negatives=probe.negative_raw_controls(GROUP,raw,data,ROOT);write(output/'negative-controls.json',negatives)
  result.update(status='original-statistical-PASS' if observation['statistics']['statistical_pass'] else 'original-statistical-FAIL-FROZEN',statistics=observation['statistics'],raw_sha256=sha(raw_path),criteria_sha256=pin['criteria_sha256'],scenario_sha256=pin['scenario_sha256'],fresh_controls=4,bounded_trials=128,aux55_counts=[len(rows) for rows in observation['auxiliary_swap55']],negative_controls=negatives,first_original_immutable=True,lifecycle_clean=True)
 except BaseException as error:
  result['error']=repr(error);(output/'error.txt').write_text(traceback.format_exc());raise
 finally:
  original_first.preserve(recordings,output)
  selected=['tools/blackbox/record777.py',*(str(p.relative_to(ROOT)) for p in probe.BASE.glob('*') if p.is_file()),'.github/workflows/crossbow-offhand-natural-original.yml']
  with tarfile.open(output/'selected-own-source.tar.gz','w:gz') as archive:
   for name in selected:archive.add(ROOT/name,arcname=name)
  write(output/'pin.json',pin);write(output/'result.json',result)
  write(output/'files.json',{str(p.relative_to(output)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in output.rglob('*') if p.is_file() and p.name!='files.json'})
 if result['status']!='original-statistical-PASS':raise RuntimeError('original statistical FAIL frozen; no retry or candidate')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--runtime-zip',type=Path,required=True);a=p.parse_args();run(a.output.resolve(),a.runtime_zip.resolve())
