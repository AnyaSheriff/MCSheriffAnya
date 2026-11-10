"""Узкий read-only import ОДНОГО known technical-FAIL original; обычные guards не меняет."""
import hashlib,importlib.util,io,json,sys,tarfile
from pathlib import Path
BASE=Path(__file__).parent
PIN=BASE/'mainhand-natural-candidate-pin.json'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
read=lambda p:json.loads(p.read_text())

def validate_input_api_and_zip(proof,pin):
 """FAIL допустим только для exact42767030823; runtime всегда требует SUCCESS."""
 for which in ['original','runtime']:
  run=proof['api'][which]['run'];artifact=proof['api'][which]['artifact']
  assert proof['actual_zip_sha256'][which]==pin[which+'_zip_sha256']
  assert run['repository']['full_name']==pin['repository'] and run['head_repository']['full_name']==pin['repository']
  assert artifact['workflow_run']['repository_id']==pin['repository_id']
  assert run['id']==pin[which+'_run'] and run['head_sha']==pin[which+'_source'] and run['status']=='completed'
  assert run['conclusion']==('failure' if which=='original' else 'success')
  assert artifact['id']==pin[which+'_artifact'] and artifact['digest']=='sha256:'+pin[which+'_zip_sha256'] and not artifact['expired']
  assert artifact['workflow_run']['id']==pin[which+'_run'] and artifact['workflow_run']['head_sha']==pin[which+'_source']
 assert proof['api']['original']['run']['run_attempt']==1

def allow_fresh_mode(args,github_actions,attempt):
 """Только будущийявный mainhand-natural candidate режим, не запуск из import."""
 return (github_actions=='true' and attempt=='1' and args.version==777 and args.core_only and args.isolate_vitals and args.causal_menu_readiness
         and not(args.resume or args.parallel or args.official_only or args.causal_original_reference or args.causal_candidate_reference or args.causal_candidate_input_proof)
         and args.scenarios==['crossbow-ub-mainhand-natural-777'] and not args.output.exists()
         and args.causal_mainhand_natural_reference and args.causal_mainhand_natural_reference.is_dir()
         and args.causal_mainhand_natural_input_proof and args.causal_mainhand_natural_input_proof.is_file()
         and args.causal_mainhand_natural_reference.resolve() not in args.output.resolve().parents
         and args.output.resolve() not in args.causal_mainhand_natural_reference.resolve().parents)

def load_reference(reference,proof,repo,binary):
 """Детерминированный oldFAIL→acceptedloaded-onlyreanalyze; ни серверов, ни записи input."""
 pin=read(PIN);validate_input_api_and_zip(proof,pin)
 assert sha(BASE/'mainhand-natural-loaded-correction.patch')==pin['accepted_loaded_patch_sha256']
 assert sha(BASE/'mainhand-natural-old-reference-pin.json')==pin['accepted_old_reference_pin_sha256']
 assert sha(BASE/'extended_probe.py')==pin['accepted_fixed_parser_sha256']
 assert sha(BASE/'criteria.json')==pin['criteria_sha256']
 assert sha(reference/'files.json')==pin['reference_ledger_sha256'] and read(reference/'files.json')==pin['reference_files']
 for name,info in pin['reference_files'].items():
  path=reference/name;assert path.is_file() and not path.is_symlink() and path.stat().st_size==info['bytes'] and sha(path)==info['sha256']
 assert sha(reference/'error.txt')==pin['original_error_sha256'] and sha(reference/'result.json')==pin['original_result_sha256']
 verdict=read(reference/'result.json');assert verdict['status']=='INVALID' and verdict['source']==pin['original_source'] and verdict['attempts']==1 and verdict['error']=='AssertionError()'
 assert "assert loaded.get('projectiles')==[dict(item='arrow',count=1)]*3" in (reference/'error.txt').read_text()
 summary=read(reference/'recordings/summary.json');assert len(summary)==1 and summary[0]['record_status']=='complete' and summary[0]['scenario']==pin['scenario']
 raw_path=reference/'recordings'/(pin['scenario']+'-official.json');assert sha(raw_path)==pin['original_raw_sha256']
 assert sha(reference/'criteria.json')==pin['criteria_sha256'] and sha(reference/'scenario.json')==pin['scenario_sha256']
 assert sha(repo/'tools/blackbox/scenarios'/(pin['scenario']+'.json'))==pin['scenario_sha256']
 assert sha(binary)==pin['runtime_sha256']
 for name,h in pin['reader_sha256'].items():assert sha(repo/name)==h
 capture=read(reference/'capture-receipt.json');assert capture['source']==pin['original_source'] and capture['requested_capture_sides']==['official'] and not capture['core_executed'] and capture['lifecycle_clean']
 assert capture['raw_sha256']==pin['original_raw_sha256'] and capture['recorder_sha256']==pin['original_recorder_sha256']
 manifest=read(next((reference/'recordings').glob('run-*.json')));assert manifest['requested_capture_sides']==['official'] and manifest['causal_original_reference'] and manifest['recorder_sha256']==pin['original_recorder_sha256']
 assert manifest['scenario_sha256']=={pin['scenario']:pin['scenario_sha256']} and manifest['binary_sha256']==pin['runtime_sha256']
 assert manifest['jar_sha1']=='33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
 for name,h in manifest['data_sha256'].items():assert sha(reference/'data'/name)==h
 life=read(reference/'recordings/lifecycle.json');assert not life['cleanup_errors'] and len(life['servers'])==len(life['cleanup'])==1 and life['servers'][0]['label']=='official' and life['servers'][0]['argv'][0]=='java'
 assert life['cleanup'][0]['exit_code']==0 and life['cleanup'][0]['group_gone'] and all(life['ports_free'].values()) and len(life['ports_free'])==4
 raw=read(raw_path);assert raw['capture_side']=='official' and raw['protocol']==777 and raw['isolate_vitals'] and raw['recorder_sha256']==pin['original_recorder_sha256']
 assert sha(reference/'selected-own-source.tar.gz')==pin['original_selected_source_sha256']
 with tarfile.open(reference/'selected-own-source.tar.gz') as source:
  old_bytes=source.extractfile('tools/blackbox/crossbow_ub_extended/extended_probe.py').read()
 assert hashlib.sha256(old_bytes).hexdigest()==pin['old_parser_source_sha256']
 old_namespace={'__file__':str(BASE/'extended_probe.py'),'__name__':'known_exact_old_parser_failure_reproduction'}
 exec(compile(old_bytes,'known-exact-old-parser','exec'),old_namespace)
 try:old_namespace['inspect']('mainhand-natural',raw,reference/'data',repo)
 except AssertionError as error:
  tb=error.__traceback__
  while tb.tb_next:tb=tb.tb_next
  assert tb.tb_frame.f_code.co_name=='validate_case' and tb.tb_lineno==pin['old_loaded_assert_line'],'a different old guard failed; stop'
 else:raise AssertionError('old technical loaded failure was not reproduced')
 spec=importlib.util.spec_from_file_location('approved_exact_loaded_only_parser',BASE/'extended_probe.py');fixed=importlib.util.module_from_spec(spec);spec.loader.exec_module(fixed)
 assert fixed.CRITERIA_SHA256==pin['criteria_sha256']
 observation=fixed.inspect('mainhand-natural',raw,reference/'data',repo)
 assert observation['statistics']['statistical_pass'] and observation['statistics']['counts']==pin['original_counts'] and len(observation['rows'])==132
 assert sha(raw_path)==pin['original_raw_sha256']
 lineage=dict(kind='ONLY-known-loaded-validation-technical-failure',original_CI_run=pin['original_run'],original_CI_conclusion='failure-retained-not-green',original_raw_sha256=pin['original_raw_sha256'],original_helper_source=pin['original_source'],accepted_patch_sha256=pin['accepted_loaded_patch_sha256'],accepted_old_reference_pin_sha256=pin['accepted_old_reference_pin_sha256'],fixed_parser_sha256=pin['accepted_fixed_parser_sha256'],all_other_guards_unchanged=True,deterministic_readonly_reanalysis='pass',default_success_requirement_elsewhere_unchanged=True)
 return raw,pin['original_raw_sha256'],observation,lineage
