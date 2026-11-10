import json,os,tempfile,unittest,subprocess,sys
from pathlib import Path
from unittest.mock import patch
import calibrate as c
class CalibrationTests(unittest.TestCase):
 def test_local_rerun_and_overwrite_rejected_before_any_output(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'none'
   for actions,attempt in [('false','1'),('true','2')]:
    with patch.dict(os.environ,dict(GITHUB_ACTIONS=actions,GITHUB_RUN_ATTEMPT=attempt)):
     with self.assertRaises(AssertionError):c.preflight(p)
    self.assertFalse(p.exists())
   p.mkdir()
   with patch.dict(os.environ,dict(GITHUB_ACTIONS='true',GITHUB_RUN_ATTEMPT='1')):
    with self.assertRaises(AssertionError):c.preflight(p)
 def test_fixed_finite_queries_states_and_no_combat(self):
  self.assertEqual(len(c.STATES),3);self.assertEqual(len(c.QUERIES),12)
  self.assertEqual([x['selected'] for x in c.STATES],[0,0,2])
  self.assertIn('data get entity blackbox',c.QUERIES)
  self.assertIn('data get entity @a Pos',c.QUERIES)
  self.assertTrue(all(q.startswith('data get entity ') for q in c.QUERIES))
 def test_fresh_cli_has_no_import_path_dependency(self):
  with tempfile.TemporaryDirectory() as d:
   p=subprocess.run([sys.executable,'-E','-s','-B',str(Path(c.__file__).resolve()),'--help'],cwd=d,capture_output=True,text=True,timeout=20)
   self.assertEqual(p.returncode,0,p.stderr)
 def test_missing_inventory_seed_and_malformed_equipment_rejected(self):
  with self.assertRaises(AssertionError):c.inventory_proof([],[])
  with self.assertRaises((AssertionError,EOFError,IndexError,TypeError)):c.equipment([dict(name='entity_equipment',at=0,hex='01')],[])
if __name__=='__main__':unittest.main()

class CorrectionTests(unittest.TestCase):
 def test_manifest_covers_first_namespace_after_freeze(self):
  with tempfile.TemporaryDirectory() as d:
   out=Path(d);(out/'a.json').write_text('{}');first=out/'FIRST-ORIGINAL';first.mkdir();(first/'a.json').write_text('{}')
   c.finalize_manifest(out);rows=json.loads((out/'files.json').read_text())
   self.assertEqual(set(rows),{'a.json','FIRST-ORIGINAL/a.json'})
 def test_exact_unbreaking_three_not_arbitrary_nonempty(self):
  names=['minecraft:thorns','minecraft:unbreaking']
  c.assert_unbreaking_three(dict(enchantments=[[1,3]]),names)
  for ench in [[[0,3]],[[1,2]],[[1,3],[0,7]]]:
   with self.assertRaises(AssertionError):c.assert_unbreaking_three(dict(enchantments=ench),names)
 def test_deadline_without_successful_ready_control_rejected(self):
  for control,done,status in [(None,True,dict(version=dict(protocol=777,name='26.3'))),(object(),False,dict(version=dict(protocol=777,name='26.3'))),(object(),True,dict(version=dict(protocol=775,name='26.3')))]:
   with self.assertRaises(AssertionError):c.assert_ready(control,done,status)
  c.assert_ready(object(),True,dict(version=dict(protocol=777,name='26.3')))
 def test_cleanup_missing_reply_cannot_escape_adapter(self):
  class Control:
   def stop_only(self):raise AssertionError('no stop reply')
  errors=[];self.assertEqual(c.CleanupAdapter(Control(),errors).run('stop'),'');self.assertEqual(len(errors),1)
 def test_own_runtime_imports_are_in_frozen_source_superset(self):
  pin=c.read(c.BASE/'pins.json')
  self.assertTrue(set(c.imported_own_files())<=set(pin['source_files']))

class SemanticCorrectionTests(unittest.TestCase):
 def test_padded_registry_packet_id_is_decoded(self):
  from unittest.mock import patch
  protocol={'configuration':{'toClient':{'types':{'packet':['container',[{'type':['mapper',{'mappings':{'0x07':'registry_data'}}]}]]}}}}
  class Decoder:
   def __init__(self,*args):self.types={}
   def read(self,b,*args):return dict(id='minecraft:enchantment',entries=[dict(key='minecraft:unbreaking')])
  with patch('analyze777.Decoder',Decoder):
   self.assertEqual(c.enchantments([dict(id=7,hex='')],protocol),['minecraft:unbreaking'])
 def test_first_semantic_failure_is_not_retried(self):
  class Broken:
   calls=0
   def run(self,*args):self.calls+=1;raise AssertionError('invalid actual frame')
  broken=Broken()
  with self.assertRaises(AssertionError):c.semantic_startup(broken,True,lambda:{})
  self.assertEqual(broken.calls,1)
  class WrongProtocol:
   calls=0
   def run(self,*args):self.calls+=1;return 'list'
  wrong=WrongProtocol()
  with self.assertRaises(AssertionError):c.semantic_startup(wrong,True,lambda:dict(version=dict(protocol=775,name='26.3')))
  self.assertEqual(wrong.calls,1)
 def test_actual777_gamerule_success_and_old_names_not_used(self):
  c.validate_setup_reply('gamerule spawn_mobs false','Game rule spawn_mobs is now set to false')
  c.validate_setup_reply('gamerule natural_health_regeneration false','Game rule natural_health_regeneration is now set to false')
  with self.assertRaises(AssertionError):c.validate_setup_reply('gamerule spawn_mobs false','Unknown gamerule')
