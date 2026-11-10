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
