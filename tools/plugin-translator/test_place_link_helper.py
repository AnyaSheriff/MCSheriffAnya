import json,unittest
from place_link_helper import choose_artifacts,dependency_command,integration_command
class GuardTests(unittest.TestCase):
 def test_dependency_only_command(self):
  args=dependency_command();self.assertEqual(args[args.index('-p')+1],'serde_json');self.assertIn('--locked',args);self.assertNotIn('--workspace',args)
 def test_missing_ambiguous_and_core_outputs_rejected(self):
  def row(name,files):return json.dumps({'reason':'compiler-artifact','target':{'name':name,'kind':['lib'],'crate_types':['lib']},'package_id':'registry+https://github.com/rust-lang/crates.io-index#serde_json@1.0.154','filenames':files})
  for log in ['',row('serde_json',['a.rlib','b.rlib']),row('mcsheriffanya',['core'])]:
   with self.assertRaises(ValueError):choose_artifacts(log)
 def test_exact_generated_link_input(self):
  log=json.dumps({'reason':'compiler-artifact','target':{'name':'serde_json','kind':['lib'],'crate_types':['lib']},'package_id':'registry+https://github.com/rust-lang/crates.io-index#serde_json@1.0.154','filenames':['/deps/libserde_json-actual.rlib']})
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

class CanonicalRegression(unittest.TestCase):
 def test_existing_dotdot_path_resolves_inside_but_lexical_parents_skip_root(self):
  from pathlib import Path
  from tempfile import TemporaryDirectory
  from place_link_helper import confined_file
  with TemporaryDirectory() as directory:
   base=Path(directory);deps=base/'deps';deps.mkdir();(base/'other').mkdir()
   target=deps/'libserde_json-real.rlib';target.write_bytes(b'actual')
   path=base/'other'/'..'/'deps'/target.name
   self.assertTrue(path.is_file());self.assertEqual(path.resolve(),target)
   with self.assertRaises(ValueError):confined_file(path,deps)

class ActualCargoLayout(unittest.TestCase):
 def test_real_metadata_and_only_two_layouts(self):
  from pathlib import Path
  from tempfile import TemporaryDirectory
  from place_link_helper import serde_rlib
  row=json.loads((Path(__file__).parent/'fixtures/place-serde-artifact.json').read_text())
  self.assertEqual(choose_artifacts(json.dumps(row))[0],row['filenames'][0])
  for field,value in [('package_id','registry+other#serde_json@1.0.154'),('package_id','registry+https://github.com/rust-lang/crates.io-index#serde_json@0.0.0')]:
   changed=dict(row);changed[field]=value
   with self.assertRaises(ValueError):choose_artifacts(json.dumps(changed))
  changed=json.loads(json.dumps(row));changed['target']['kind']=['bin']
  with self.assertRaises(ValueError):choose_artifacts(json.dumps(changed))
  changed=json.loads(json.dumps(row));changed['target']['crate_types']=['cdylib']
  with self.assertRaises(ValueError):choose_artifacts(json.dumps(changed))
  with TemporaryDirectory()as directory:
   release=Path(directory)/'release';release.mkdir();deps=release/'deps';deps.mkdir()
   for name in [release/'libserde_json.rlib',deps/'libserde_json-012abcdef.rlib']:
    name.write_bytes(b'rlib');self.assertEqual(serde_rlib(name,release),name)
   for name in [release/'wrong.rlib',deps/'libserde_json-ZZZ.rlib',deps/'wrong.rlib']:
    name.write_bytes(b'wrong')
    with self.assertRaises(ValueError):serde_rlib(name,release)
   link=release/'libserde_json-copy.rlib';link.symlink_to(release/'libserde_json.rlib')
   with self.assertRaises(ValueError):serde_rlib(link,release)
