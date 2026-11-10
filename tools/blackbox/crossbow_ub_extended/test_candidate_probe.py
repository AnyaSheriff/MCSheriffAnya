import copy,json,os,tempfile,types,unittest
from pathlib import Path
from unittest.mock import patch
import candidate_only as c
import record777
class CandidateTests(unittest.TestCase):
 def test_explicit_mode_guards(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory);reference=root/'immutable';reference.mkdir();proof=root/'proof.json';proof.write_text('{}')
   good=dict(version=777,causal_menu_readiness=True,core_only=True,isolate_vitals=True,resume=False,parallel=False,official_only=False,causal_original_reference=False,scenarios=['crossbow-ub-offhand-commandloaded-777'],output=root/'new-output',causal_candidate_reference=reference,causal_candidate_input_proof=proof)
   allowed=record777.allow_causal_candidate_reference
   self.assertTrue(allowed(types.SimpleNamespace(**good),'true','1'))
   for key,value in [('version',775),('causal_menu_readiness',False),('core_only',False),('isolate_vitals',False),('resume',True),('parallel',True),('official_only',True),('causal_original_reference',True),('scenarios',['crossbow-ub-offhand-natural-777']),('scenarios',[]),('output',reference),('output',reference/'new'),('causal_candidate_input_proof',None)]:
    with self.subTest(key=key,value=value):self.assertFalse(allowed(types.SimpleNamespace(**dict(good,**{key:value})),'true','1'))
   self.assertFalse(allowed(types.SimpleNamespace(**good),'false','1'));self.assertFalse(allowed(types.SimpleNamespace(**good),'true','2'))
 def test_local_and_rerun_entry_rejected_before_output(self):
  with tempfile.TemporaryDirectory() as directory:
   output=Path(directory)/'never-written'
   for env in [dict(GITHUB_ACTIONS='false',GITHUB_RUN_ATTEMPT='1'),dict(GITHUB_ACTIONS='true',GITHUB_RUN_ATTEMPT='2')]:
    with patch.dict(os.environ,env):
     with self.assertRaises(AssertionError):c.run(output,Path('absent-original'),Path('absent-runtime'))
    self.assertFalse(output.exists())
 def test_lifecycle_one_core_and_no_java(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory);binary=root/'own-binary';binary.touch();file=root/'life.json'
   good=dict(cleanup_errors=[],ports_free={str(p):True for p in range(26190,26194)},servers=[dict(label='ours',port=26192,argv=[str(binary.resolve())])],cleanup=[dict(exit_code=0,group_gone=True)])
   file.write_text(json.dumps(good));c.lifecycle(file,binary)
   for mutate in [lambda d:d['servers'][0].update(label='official'),lambda d:d['servers'][0].update(argv=['java']),lambda d:d['cleanup'][0].update(group_gone=False),lambda d:d['ports_free'].update({'26192':False})]:
    changed=copy.deepcopy(good);mutate(changed);file.write_text(json.dumps(changed))
    with self.assertRaises(AssertionError):c.lifecycle(file,binary)
 def test_bad_actual_zip_sha_is_rejected_before_extract(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory);archive=root/'input.zip';archive.write_bytes(b'not-the-pinned-zip');output=root/'no-output'
   with self.assertRaises(AssertionError):c.extract_locked(archive,output,'0'*64)
   self.assertFalse(output.exists())
 def test_missing_or_wrong_api_zip_receipt_is_rejected_before_raw_read(self):
  with tempfile.TemporaryDirectory() as directory:
   path=Path(directory)/'proof.json';pin=c.read(c.PIN)
   good=dict(actual_zip_sha256={which:pin[which+'_zip_sha256'] for which in ['original','runtime']},api={which:dict(artifact=dict(digest='sha256:'+pin[which+'_zip_sha256']),run=dict(head_sha=pin[which+'_source'],conclusion='success')) for which in ['original','runtime']})
   for mutate in [lambda d:d['actual_zip_sha256'].update(original='0'*64),lambda d:d['api']['original']['artifact'].update(digest='sha256:'+'0'*64),lambda d:d['api']['runtime']['run'].update(head_sha='0'*40)]:
    bad=copy.deepcopy(good);mutate(bad);path.write_text(json.dumps(bad));args=types.SimpleNamespace(causal_candidate_reference=Path(directory)/'missing-raw',causal_candidate_input_proof=path)
    with self.assertRaises(AssertionError):record777.checked_causal_candidate_reference(args,dict(name='crossbow-ub-offhand-commandloaded-777'))
 def test_exact_original_and_runtime_pins(self):
  pin=c.read(c.PIN)
  self.assertEqual(pin['original_raw_sha256'],'178b9b57069ecc584990b346513f70264ad495e83a9468d9c6445678b8154bfa')
  self.assertEqual(pin['runtime_source'],'93bbae85da4d2b90df9979b5ba3a5f7c965ee21f')
  self.assertTrue(pin['no_new_original'] and pin['no_natural'] and pin['no_recompile'])
if __name__=='__main__':unittest.main()
