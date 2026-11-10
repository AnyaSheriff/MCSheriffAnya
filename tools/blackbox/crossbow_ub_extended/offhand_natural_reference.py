"""Узкий read-only import успешного первого offhand-natural original; default guards unchanged."""
import hashlib,json,os
from pathlib import Path
import extended_probe as probe
import original_first
BASE=Path(__file__).parent
PIN=BASE/'offhand-natural-candidate-pin.json'
sha=probe.sha;read=probe.read

def validate_input_api_and_zip(proof,pin):
 for which in ['original','runtime']:
  run=proof['api'][which]['run'];artifact=proof['api'][which]['artifact']
  assert proof['actual_zip_sha256'][which]==pin[which+'_zip_sha256']
  assert run['repository']['full_name']==run['head_repository']['full_name']==pin['repository']
  assert artifact['workflow_run']['repository_id']==pin['repository_id']
  assert run['id']==pin[which+'_run'] and run['head_sha']==pin[which+'_source'] and run['status']=='completed' and run['conclusion']=='success'
  assert artifact['id']==pin[which+'_artifact'] and artifact['digest']=='sha256:'+pin[which+'_zip_sha256'] and not artifact['expired']
  assert artifact['workflow_run']['id']==pin[which+'_run'] and artifact['workflow_run']['head_sha']==pin[which+'_source']
 assert proof['api']['original']['run']['run_attempt']==1

def allow_fresh_mode(args,github_actions,attempt):
 return (github_actions=='true' and attempt=='1' and args.version==777 and args.core_only and args.isolate_vitals and args.causal_menu_readiness
  and not(args.resume or args.parallel or args.official_only or args.causal_original_reference or args.causal_candidate_reference or args.causal_candidate_input_proof or args.causal_mainhand_natural_reference or args.causal_mainhand_natural_input_proof)
  and args.scenarios==['crossbow-ub-offhand-natural-777'] and not args.output.exists()
  and args.causal_offhand_natural_reference and args.causal_offhand_natural_reference.is_dir()
  and args.causal_offhand_natural_input_proof and args.causal_offhand_natural_input_proof.is_file()
  and args.causal_offhand_natural_reference.resolve() not in args.output.resolve().parents
  and args.output.resolve() not in args.causal_offhand_natural_reference.resolve().parents)

def validate_reference(reference,proof,repo):
 pin=read(PIN);validate_input_api_and_zip(proof,pin)
 assert sha(BASE/'extended_probe.py')==pin['accepted_fixed_parser_sha256'] and sha(BASE/'criteria.json')==pin['criteria_sha256']
 assert sha(reference/'files.json')==pin['reference_ledger_sha256'] and read(reference/'files.json')==pin['reference_files']
 for name,info in pin['reference_files'].items():
  p=reference/name;assert p.is_file() and not p.is_symlink() and p.stat().st_size==info['bytes'] and sha(p)==info['sha256']
 assert sha(reference/'scenario.json')==sha(repo/'tools/blackbox/scenarios'/(pin['scenario']+'.json'))==pin['scenario_sha256']
 assert sha(reference/'criteria.json')==pin['criteria_sha256']
 verdict=read(reference/'result.json');assert verdict['status']=='original-statistical-PASS' and verdict['source']==pin['original_source'] and verdict['attempts']==1 and not verdict['core_executed'] and not verdict['compiled']
 assert not(reference/'error.txt').exists()
 first=read(reference/'FIRST-ORIGINAL/original-first-pin.json');capture=read(reference/'capture-receipt.json')
 assert first['first_complete_original'] and first['capture_source']==capture['source']==pin['original_source'] and first['statistical_pass']
 assert capture['requested_capture_sides']==['official'] and not capture['core_executed'] and capture['lifecycle_clean'] and capture['runtime_identity_mode']=='0400'
 assert capture['raw_sha256']==first['raw_sha256']==pin['original_raw_sha256'] and capture['recorder_sha256']==pin['original_recorder_sha256']
 summary=read(reference/'recordings/summary.json');assert len(summary)==1 and summary[0]['record_status']=='complete' and summary[0]['scenario']==pin['scenario']
 rawpath=reference/'recordings'/(pin['scenario']+'-official.json');assert sha(rawpath)==sha(reference/'FIRST-ORIGINAL/original-raw.json')==pin['original_raw_sha256']
 manifests=list((reference/'recordings').glob('run-*.json'));assert len(manifests)==1;m=read(manifests[0]);assert m['requested_capture_sides']==['official'] and m['causal_original_reference'] and m['recorder_sha256']==pin['original_recorder_sha256']
 assert m['scenario_sha256']=={pin['scenario']:pin['scenario_sha256']} and m['binary_sha256']==pin['runtime_sha256']
 for name,h in m['data_sha256'].items():assert sha(reference/'data'/name)==h
 for name,h in pin['reader_sha256'].items():assert sha(repo/name)==h
 original_first.validate_lifecycle(reference/'recordings/lifecycle.json')
 raw=read(rawpath);assert raw['capture_side']=='official' and raw['protocol']==777 and raw['isolate_vitals'] and raw['recorder_sha256']==pin['original_recorder_sha256']
 observation=probe.inspect('offhand-natural',raw,reference/'data',repo)
 validate_offhand_using_flags(raw)
 assert json.loads(json.dumps(observation))==read(reference/'FIRST-ORIGINAL/observation.json') and observation['statistics']['statistical_pass'] and observation['statistics']['counts']==pin['original_counts']
 return raw,pin['original_raw_sha256'],observation,dict(kind='exact-successful-first-offhand-natural-original',original_run=pin['original_run'],original_conclusion='success',default_success_requirement_unchanged=True,reference_ledger_sha256=sha(reference/'files.json'))

def load_reference(reference,proof,repo,binary):
 result=validate_reference(reference,proof,repo)
 assert sha(binary)==read(PIN)['runtime_sha256']
 return result

def validate_offhand_using_flags(raw):
 # Собственный prior living_flags: using mask1, offhand mask2; после release3→2.
 from probe_persistence_777 import player_metadata
 owner=raw['initial'][0]['entity'];uses=[w for w in raw['clients'][0]['wire_sent'] if w['name']=='use_item'];releases=[w for w in raw['clients'][0]['wire_sent'] if w['name']=='block_dig'];assert len(uses)==264 and len(releases)==132
 for client in raw['clients']:
  updates=[]
  for event in client['events']:
   if event['name']=='entity_metadata':
    entity,fields=player_metadata(bytes.fromhex(event['hex']))
    if entity==owner and 8 in fields:updates.append((event['at'],fields[8]))
  for i,release in enumerate(releases):
   load,fire=uses[2*i:2*i+2];active=[v for at,v in updates if load['at']<=at<release['at'] and v&1];stop=[v for at,v in updates if release['at']<=at<fire['at']]
   assert active and all(v==3 for v in active) and stop and all(v==2 for v in stop)
 return dict(pairs_each=132,active=3,inactive_offhand=2)
