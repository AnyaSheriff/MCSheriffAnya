#!/usr/bin/env python3
"""Strict selected sculk comparison; local IDs/packet formats/timing omitted."""
import argparse,hashlib,json,re
from pathlib import Path
from probe_wool_sculk_777 import TYPES,LINES,HASHES,JAR_SHA1,changes
from generate_tables import states

def provenance(official,core,expected_core_sha256):
 is_hash=lambda value:isinstance(value,str) and re.fullmatch('[0-9a-f]{64}',value) is not None
 assert is_hash(expected_core_sha256) and core['binary_sha256']==expected_core_sha256,'unexpected core binary'
 assert official['jar_sha1']==JAR_SHA1 and official['binary_sha256'] is None and core['jar_sha1'] is None,'unexpected official source'
 assert official['pins']==core['pins']==HASHES,'unexpected pinned data'
 assert is_hash(official['runner_sha256']) and official['runner_sha256']==core['runner_sha256'],'different or empty runner'
 keys={'probe_gameplay.py','probe_white_concrete_slab.py','generate_tables.py','smoke.py','observe.py'}
 assert set(official['helper_sha256'])==set(core['helper_sha256'])==keys,'unexpected helper keys'
 assert official['helper_sha256']==core['helper_sha256'] and all(is_hash(h) for h in official['helper_sha256'].values()),'different or empty helper hash'
def load_state_map(data):
 raw=(data/'blocks.json').read_bytes();assert hashlib.sha256(raw).hexdigest()==HASHES['blocks.json']
 return {value:dict(name=name,properties=dict(props)) for (name,props),value in states(json.loads(raw)).items()}
def review(report,state_map):
 assert report['result']=='OBSERVED' and report['server_exit']==0 and report['group_gone'] and report['port_free']
 assert report['survival_confirmed_by_packet'] and report['mode_events'][-1]['mode']==0
 materials={x['label']:x for x in TYPES}
 expected_labels={'emission/'+kind['label']+'/'+action for kind in TYPES for action in ['place','break']}|{'occlusion/'+line['name']+'/'+kind for line in LINES for kind in ['air']+[x['label'] for x in TYPES]+['no-event']}
 assert len(report['cases'])==36 and {c['label'] for c in report['cases']}==expected_labels
 cases={c['label']:c for c in report['cases']}
 expected_controls={'emission/'+kind+'/'+action:kind=='stone' for kind in ['stone','full-wool'] for action in ['place','break']}
 expected_controls.update({'occlusion/'+line['name']+'/'+kind:kind in ['air','stone'] for line in LINES for kind in ['air','stone','full-wool','no-event']})
 assert len(report['controls'])==12 and {c['label'] for c in report['controls']}==set(expected_controls)
 for control in report['controls']:
  expected=expected_controls[control['label']]
  assert control['matched'] and control['expected_active']==expected and control['observed_active']==expected and cases[control['label']]['active']==expected
 for case in report['cases']:
  assert case['observation_seconds']==(5.5 if case.get('action')=='break' else 3.5)
  events=case['selected_events']
  for event in events:
   assert event['state'] in state_map
   assert (event['position'],event['state']) in changes(event['name'],bytes.fromhex(event['payload_hex'])),'selected event not in raw packet'
  actual_active=[dict(e,properties=state_map[e['state']]['properties']) for e in events if e['position']==case['sensor'] and state_map[e['state']]['name']=='sculk_sensor' and state_map[e['state']]['properties']['sculk_sensor_phase']=='active']
  assert case['activations']==actual_active and case['active']==bool(actual_active),'activation not backed by sensor raw event'
  assert all(int(e['properties']['power'])>0 for e in actual_active)
  source_updates=[dict(e,decoded=state_map[e['state']]) for e in events if e['position']==case['source']]
  assert case['final_source_updates']==source_updates,'source summary not backed by raw selected event'
  if case['kind']=='emission':
   _,label,action=case['label'].split('/');kind=materials[label];assert case['material']==kind and case['action']==action
   assert case['source']==[2,65,0] and case['sensor']==[6,65,0]
   assert source_updates,'missing actual source event'
   final=source_updates[-1]['decoded']
   if action=='place':
    assert final['name']==kind['block']
    expected_props={} if 'half' not in kind else dict(type=kind['half'],waterlogged=str(kind['wet'] and kind['half']!='double').lower())
    assert final['properties']==expected_props,'wrong placed shape/water'
   else:
    wet=kind.get('wet',False);assert final==dict(name='water' if wet else 'air',properties=dict(level='0') if wet else {}),'wrong break replacement'
  else:
   assert case['kind']=='occlusion'
   _,line,label=case['label'].split('/');geometry=next(x for x in LINES if x['name']==line)
   assert case['source']==geometry['source'] and case['sensor']==geometry['sensor'] and case['geometry']==geometry
   assert case['barrier']==(dict(label=label,block='air') if label in ['air','no-event'] else materials[label])
   if label=='no-event':assert not source_updates
   else:assert source_updates and source_updates[-1]['decoded']==dict(name='lever',properties=dict(face='floor',facing='north',powered='true')),'lever action unconfirmed'
 return cases
def normalized(case):
 return dict(kind=case['kind'],active=case['active'],source=case['source'],sensor=case['sensor'],initial=case['initial'],observation_seconds=case['observation_seconds'],geometry=case.get('geometry'),material=case.get('material'),barrier=case.get('barrier'),final_source=case['final_source_updates'][-1]['decoded'] if case['final_source_updates'] else None)
def main():
 p=argparse.ArgumentParser();p.add_argument('official',type=Path);p.add_argument('core',type=Path);p.add_argument('--output',type=Path,required=True);p.add_argument('--expected-core-sha256',required=True);p.add_argument('--data',type=Path,default=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/candidate-json'));a=p.parse_args();result=dict(result='FAIL',limits=['white wool selected actions only','no frequency/calibration/event-origin/shapealgorithm/timing/packet-format parity claim','fire/steps/landings/all16ordinary not observed'])
 try:
  off=json.loads(a.official.read_text());core=json.loads(a.core.read_text());provenance(off,core,a.expected_core_sha256);state_map=load_state_map(a.data);x=review(off,state_map);y=review(core,state_map);assert off['jar_sha1'] and core['binary_sha256'] and not core['jar_sha1']
  assert off['runner_sha256']==core['runner_sha256'] and off['helper_sha256']==core['helper_sha256'];assert x.keys()==y.keys()
  checks={label:dict(equal=normalized(c)==normalized(y[label]),official=normalized(c),core=normalized(y[label])) for label,c in x.items()};result.update(cases=checks,case_count=36,runner_sha256=off['runner_sha256'],official_result_sha256=hashlib.sha256(a.official.read_bytes()).hexdigest(),core_result_sha256=hashlib.sha256(a.core.read_bytes()).hexdigest());assert all(c['equal'] for c in checks.values()),'selected cases differ';result['result']='PASS'
 except (AssertionError,KeyError,ValueError) as e:result['error']=repr(e)
 a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='cases'}));assert result['result']=='PASS'
if __name__=='__main__':main()
