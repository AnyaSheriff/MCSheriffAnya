import json,unittest
from place_link_helper import choose_artifacts,dependency_command,integration_command
class GuardTests(unittest.TestCase):
 def test_dependency_only_command(self):
  args=dependency_command();self.assertEqual(args[args.index('-p')+1],'serde_json');self.assertIn('--locked',args);self.assertNotIn('--workspace',args)
 def test_missing_ambiguous_and_core_outputs_rejected(self):
  def row(name,files):return json.dumps({'reason':'compiler-artifact','target':{'name':name},'filenames':files})
  for log in ['',row('serde_json',['a.rlib','b.rlib']),row('mcsheriffanya',['core'])]:
   with self.assertRaises(ValueError):choose_artifacts(log)
 def test_exact_generated_link_input(self):
  log=json.dumps({'reason':'compiler-artifact','target':{'name':'serde_json'},'filenames':['/deps/libserde_json-actual.rlib']})
  rlib,rows=choose_artifacts(log);args=integration_command('/tool/rustc','tests/world_startup.rs','out',rlib,'/deps')
  self.assertIn('serde_json=/deps/libserde_json-actual.rlib',args);self.assertIn('dependency=/deps',args);self.assertEqual(len(rows),1)
if __name__=='__main__':unittest.main()

class PathGuards(unittest.TestCase):
 def test_modified_command_rejected_before_spawn(self):
  from place_link_helper import validate_command
  for args in [['cargo','build','--workspace'],dependency_command()+['-p','mcsheriffanya']]:
   with self.assertRaises(ValueError):validate_command(args)
 def test_symlink_and_escape_rejected(self):
  from pathlib import Path
  from tempfile import TemporaryDirectory
  from place_link_helper import confined_file
  with TemporaryDirectory() as directory:
   root=Path(directory);deps=root/'deps';deps.mkdir();outside=root/'outside.rlib';outside.write_bytes(b'x')
   link=deps/'libserde_json-fake.rlib';link.symlink_to(outside)
   for path in [link,outside,Path('relative.rlib'),deps/'sub'/'..'/'file.rlib']:
    with self.assertRaises(ValueError):confined_file(path,deps)
