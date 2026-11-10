import argparse,copy,hashlib,json,os,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import offhand_natural_candidate as candidate
import offhand_natural_reference as admission
class OffhandNaturalCandidateTests(unittest.TestCase):
 def test_exact_current_recorder_parser_and_immutable_pins_before_download(self):
  pin=admission.read(admission.PIN)
  self.assertEqual(pin['candidate_recorder_sha256'],admission.sha(candidate.ROOT/'tools/blackbox/record777.py'))
  self.assertEqual(pin['accepted_fixed_parser_sha256'],admission.sha(admission.BASE/'extended_probe.py'))
  self.assertEqual(pin['original_run'],42784228484);self.assertEqual(pin['original_artifact'],13748212998)
  self.assertEqual(pin['original_CI_conclusion_REQUIRED'],'success');self.assertEqual(pin['N'],128)
 def test_local_and_rerun_rejected_without_output(self):
  with tempfile.TemporaryDirectory() as d:
   for actions,attempt in [('false','1'),('true','2')]:
    output=Path(d)/'output'
    with patch.dict(os.environ,dict(GITHUB_ACTIONS=actions,GITHUB_RUN_ATTEMPT=attempt)):
     with self.assertRaises(AssertionError):candidate.run(output,Path('no-original'),Path('no-runtime'))
    self.assertFalse(output.exists())
 def test_fresh_mode_narrow_and_no_aliases(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);reference=root/'reference';reference.mkdir();proof=root/'proof';proof.write_text('{}')
   a=argparse.Namespace(version=777,core_only=True,isolate_vitals=True,causal_menu_readiness=True,resume=False,parallel=False,official_only=False,causal_original_reference=False,causal_candidate_reference=None,causal_candidate_input_proof=None,causal_mainhand_natural_reference=None,causal_mainhand_natural_input_proof=None,scenarios=['crossbow-ub-offhand-natural-777'],output=root/'output',causal_offhand_natural_reference=reference,causal_offhand_natural_input_proof=proof)
   self.assertTrue(admission.allow_fresh_mode(a,'true','1'))
   for key,value in [('resume',True),('official_only',True),('parallel',True),('core_only',False),('scenarios',['crossbow-ub-mainhand-natural-777']),('causal_mainhand_natural_reference',reference),('output',reference/'child')]:
    b=copy.copy(a);setattr(b,key,value);self.assertFalse(admission.allow_fresh_mode(b,'true','1'))
 def test_fresh_record777_package_import_and_loadreference_context(self):
  import subprocess,sys
  root=candidate.ROOT
  code="""import sys,json,tempfile
from pathlib import Path
sys.path.insert(0,BLACKBOX)
import record777
module=record777.offhand_natural_import_guard()
pin=module.read(module.PIN);proof=dict(api={},actual_zip_sha256={})
for which in ['original','runtime']:
 proof['actual_zip_sha256'][which]=pin[which+'_zip_sha256']
 proof['api'][which]=dict(run=dict(repository=dict(full_name=pin['repository']),head_repository=dict(full_name=pin['repository']),id=pin[which+'_run'],head_sha=pin[which+'_source'],status='completed',conclusion='success',run_attempt=1),artifact=dict(id=pin[which+'_artifact'],digest='sha256:'+pin[which+'_zip_sha256'],expired=False,workflow_run=dict(repository_id=pin['repository_id'],id=pin[which+'_run'],head_sha=pin[which+'_source'])))
with tempfile.TemporaryDirectory() as d:
 try:module.load_reference(Path(d),proof,Path(REPO),Path(d)/'no-binary')
 except FileNotFoundError as error:assert Path(error.filename).name=='files.json'
 else:raise AssertionError('missing immutable reference accepted')
print('package-import-and-real-loadreference-preflight-PASS')
""".replace('BLACKBOX',repr(str(root/'tools/blackbox'))).replace('REPO',repr(str(root)))
  with tempfile.TemporaryDirectory() as d:
   result=subprocess.run([sys.executable,'-E','-s','-B','-c',code],cwd=d,text=True,capture_output=True,timeout=30)
   self.assertEqual(result.returncode,0,result.stdout+result.stderr)
 def test_fresh_direct_entry_import_context(self):
  import subprocess,sys
  with tempfile.TemporaryDirectory() as d:
   result=subprocess.run([sys.executable,'-E','-s','-B',str(candidate.ROOT/'tools/blackbox/crossbow_ub_extended/offhand_natural_candidate.py'),'--help'],cwd=d,text=True,capture_output=True,timeout=30)
   self.assertEqual(result.returncode,0,result.stdout+result.stderr)
   self.assertIn('--original-zip',result.stdout)
 def test_api_requires_success_and_actual_zip_and_exact_repo(self):
  pin=admission.read(admission.PIN);proof=dict(api={},actual_zip_sha256={})
  for which in ['original','runtime']:
   proof['actual_zip_sha256'][which]=pin[which+'_zip_sha256'];proof['api'][which]=dict(run=dict(repository=dict(full_name=pin['repository']),head_repository=dict(full_name=pin['repository']),id=pin[which+'_run'],head_sha=pin[which+'_source'],status='completed',conclusion='success',run_attempt=1),artifact=dict(id=pin[which+'_artifact'],digest='sha256:'+pin[which+'_zip_sha256'],expired=False,workflow_run=dict(repository_id=pin['repository_id'],id=pin[which+'_run'],head_sha=pin[which+'_source'])))
  admission.validate_input_api_and_zip(proof,pin)
  for which in ['original','runtime']:
   v=copy.deepcopy(proof);v['api'][which]['run']['conclusion']='failure'
   with self.assertRaises(AssertionError):admission.validate_input_api_and_zip(v,pin)
   v=copy.deepcopy(proof);v['actual_zip_sha256'][which]='0'*64
   with self.assertRaises(AssertionError):admission.validate_input_api_and_zip(v,pin)
if __name__=='__main__':unittest.main()
