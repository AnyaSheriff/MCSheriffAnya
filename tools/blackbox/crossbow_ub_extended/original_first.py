#!/usr/bin/env python3
"""Один original-only offhand128+4 на GitHub; candidate и natural здесь не запускаются."""
import argparse,hashlib,json,os,shutil,subprocess,sys,tempfile,traceback,urllib.request
from pathlib import Path
if __package__:
 from . import extended_probe as probe
else:
 import extended_probe as probe
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/blackbox'))
import combat_ci
GROUP='offhand-commandloaded'
BASE='e3e90d74f60f0750459ebe964c709195f92b49ef'
BINARY_SOURCE='93bbae85da4d2b90df9979b5ba3a5f7c965ee21f'
BINARY_SHA='4a456bd552edd3a46bd3755b40055c731a97e199a5116cb7d9bdae1d5aa65110'
PRODUCER_RUN=42690720281
ARTIFACT_ID=13710052720
ARTIFACT_SHA='72961169c5235ea2390d8334ef7c574e8f5324a9efcee085825bef4baba42077'
sha=probe.sha;read=probe.read;write=probe.write

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def api(path):
 with urllib.request.urlopen('https://api.github.com/repos/AnyaSheriff/MCSheriffAnya/'+path,timeout=30) as response:return json.load(response)
def validate_prebuilt(folder,output):
 binary=folder/'ub128-evidence/runtime/mcsheriffanya';receipt=read(folder/'ci-logs/receipt.json')
 assert receipt['source']==BINARY_SOURCE and receipt['status']=='success' and receipt['runtime_sha256']==BINARY_SHA
 assert sha(binary)==BINARY_SHA and binary.read_bytes()[:6]==b'\x7fELF\x02\x01'
 assert sha(folder/'ci-logs/Cargo.lock')==receipt['lock_sha256']['Cargo.lock']=='9675f5683ab3b533faad08f32352dffbd79242c9043b7cec4b0caf8d8512ea35'
 run=api('actions/runs/'+str(PRODUCER_RUN));meta=api('actions/artifacts/'+str(ARTIFACT_ID))
 assert run['status']=='completed' and run['conclusion']=='success' and run['head_sha']==BINARY_SOURCE
 assert meta['workflow_run']['id']==PRODUCER_RUN and meta['workflow_run']['head_sha']==BINARY_SOURCE and meta['digest']=='sha256:'+ARTIFACT_SHA
 binary.chmod(0o400) # Только читаемая identity-заглушка: исполнять её в этом опыте нельзя.
 assert binary.stat().st_mode&0o111==0
 shutil.copy2(folder/'ci-logs/Cargo.lock',output/'prebuilt-Cargo.lock')
 shutil.copy2(folder/'ci-logs/receipt.json',output/'prebuilt-receipt.json')
 write(output/'prebuilt-provenance.json',dict(run=run,artifact=meta,binary_sha256=BINARY_SHA,binary_source=BINARY_SOURCE,binary_executed=False,binary_mode=oct(binary.stat().st_mode&0o777)))
 return binary,receipt['lock_sha256']['Cargo.lock']

def validate_lifecycle(path):
 life=read(path)
 assert not life['cleanup_errors'] and life['ports_free']=={str(p):True for p in range(26190,26194)}
 assert len(life['servers'])==len(life['cleanup'])==1
 assert life['servers'][0]['label']=='official' and life['servers'][0]['port']==26190
 assert life['servers'][0]['argv'][1:]==['-Xmx1G','-XX:ActiveProcessorCount=2','-jar','server.jar','nogui']
 assert all(row['exit_code']==0 and row['group_gone'] for row in life['cleanup'])
 return life

def preserve(recordings,output):
 if recordings.exists():
  target=output/'recordings';target.mkdir(exist_ok=True)
  for p in recordings.glob('*.json'):shutil.copy2(p,target/p.name)
  p=recordings/'official/server.log'
  if p.exists():shutil.copy2(p,output/'official-server.log')

def run(output,prebuilt):
 assert os.environ.get('GITHUB_ACTIONS')=='true','GitHub-only; no local execution'
 assert os.environ.get('GITHUB_RUN_ATTEMPT')=='1','capture reruns are forbidden; stop for root review'
 output.mkdir(exist_ok=False);work=Path(tempfile.mkdtemp(prefix='crossbow-offhand-original-',dir=os.environ['RUNNER_TEMP']));recordings=work/'recordings'
 result=dict(status='INVALID',group=GROUP,source=git('rev-parse','HEAD'),gameplay_base=BASE,core_executed=False,natural_executed=False,attempts=1)
 try:
  pins=read(probe.BASE/'source-pins.json');assert pins['source']==BASE and git('rev-parse','HEAD:src')==pins['core_tree']==git('rev-parse',BASE+':src')
  assert git('diff',BASE,'HEAD','--','src')==''
  assert sha(probe.BASE/'criteria.json')==pins['criteria_sha256']==probe.CRITERIA_SHA256
  assert sha(ROOT/'tools/blackbox/record777.py')==pins['proposed_recorder_sha256']
  probe.prepare(work/'generated',ROOT)
  source_scenario=work/'generated'/GROUP/'scenario.json';scenario=ROOT/'tools/blackbox/scenarios'/('crossbow-ub-'+GROUP+'-777.json')
  assert source_scenario.read_bytes()==scenario.read_bytes()
  shutil.copy2(source_scenario,output/'scenario.json');shutil.copy2(probe.BASE/'criteria.json',output/'criteria.json');shutil.copy2(probe.BASE/'source-pins.json',output/'source-pins.json')
  subprocess.run(['git','archive','--format=tar.gz','--output='+str(output/'own-source.tar.gz'),'HEAD'],cwd=ROOT,check=True)
  binary,lock_sha=validate_prebuilt(prebuilt,output)
  source_pin=read(ROOT/'tools/blackbox/ci/protection-edges-777.json');data=work/'data';data.mkdir()
  combat_ci.fetch(source_pin['official_jar'],work/'server.jar')
  for entry in source_pin['json_files']:
   if entry['file'] in ['protocol.json','items.json','entities.json','blocks.json']:combat_ci.fetch(entry,data/entry['file'])
  target=output/'data';target.mkdir()
  for p in data.glob('*.json'):shutil.copy2(p,target/p.name)
  write(output/'approved-source-pin.json',source_pin)
  command=[sys.executable,'-B',str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(recordings),'--binary',str(binary),'--jar',str(work/'server.jar'),'--java','java','--official-port','26190','--core-port','26192','--max-seconds','600','--official-only','--isolate-vitals','--causal-menu-readiness','--causal-original-reference','crossbow-ub-'+GROUP+'-777']
  write(output/'command.json',command)
  with (output/'recorder.log').open('w') as log:
   process=subprocess.Popen(command,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT)
   try:code=process.wait(timeout=660)
   except subprocess.TimeoutExpired:
    process.terminate()
    try:process.wait(timeout=25)
    except subprocess.TimeoutExpired:process.kill();process.wait(timeout=5)
    raise
  assert code==0,'first original recording INVALID; never auto-retry'
  life=validate_lifecycle(recordings/'lifecycle.json')
  assert sha(binary)==BINARY_SHA and binary.stat().st_mode&0o111==0
  assert not list(recordings.glob('*-ours*.json')) and not(recordings/'ours').exists()
  manifests=list(recordings.glob('run-*.json'));assert len(manifests)==1;m=read(manifests[0]);raw=recordings/('crossbow-ub-'+GROUP+'-777-official.json')
  assert m['causal_original_reference'] is True and m['requested_capture_sides']==['official'] and m['protocol']==777 and m['binary_sha256']==BINARY_SHA
  assert m['jar_sha1']==source_pin['official_jar']['sha1'] and m['recorder_sha256']==pins['proposed_recorder_sha256']
  assert m['scenario_sha256']=={scenario.stem:sha(scenario)}
  assert m['data_sha256']=={p.name:sha(p) for p in data.glob('*.json')}
  capture=dict(source=result['source'],gameplay_base=BASE,core_tree=pins['core_tree'],requested_capture_sides=['official'],raw_sha256=sha(raw),criteria_sha256=probe.CRITERIA_SHA256,recorder_sha256=m['recorder_sha256'],jar_sha1=m['jar_sha1'],scenario_sha256=sha(scenario),runtime_sha256=BINARY_SHA,runtime_source=BINARY_SOURCE,runtime_executed=False,lock_sha256=lock_sha,plugins_active=False,lifecycle_clean=True)
  write(output/'capture-receipt.json',capture)
  observation=probe.freeze_original(GROUP,raw,data,ROOT,output/'FIRST-ORIGINAL',capture)
  negatives=probe.negative_raw_controls(GROUP,read(raw),data,ROOT);write(output/'negative-controls.json',negatives)
  result.update(status='original-statistical-PASS' if observation['statistics']['statistical_pass'] else 'original-statistical-FAIL-FROZEN',statistics=observation['statistics'],raw_sha256=sha(raw),criteria_sha256=probe.CRITERIA_SHA256,auxiliary_swap55_counts=[len(rows) for rows in observation['auxiliary_swap55']],bounded_trials=128,fresh_controls=4,negative_controls=negatives,lifecycle_clean=True,immutable_first_original=True)
 except BaseException as error:
  result['error']=repr(error);(output/'error.txt').write_text(traceback.format_exc());raise
 finally:
  preserve(recordings,output)
  write(output/'result.json',result)
  files={str(p.relative_to(output)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in output.rglob('*') if p.is_file() and p.name!='files.json'}
  write(output/'files.json',files)
 if result['status']!='original-statistical-PASS':raise RuntimeError('Original statistical FAIL retained/frozen; stop before candidate')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--prebuilt',type=Path,required=True);a=p.parse_args();run(a.output.resolve(),a.prebuilt.resolve())
