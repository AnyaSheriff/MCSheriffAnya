"""Fixture identity must reach optional ABI validation, never manifest-name refusal."""
from pathlib import Path
import ast,re,tomllib,unittest
ROOT=Path(__file__).resolve().parents[2]
class IdentityGuard(unittest.TestCase):
 def test_actual_builder_and_embedded_rust_names_match_manifest_limit(self):
  builder=(ROOT/'tools/plugin-translator/build-place-invalid.py').read_text()
  tree=ast.parse(builder)
  names=[node.value.left.value for node in ast.walk(tree) if isinstance(node,ast.Assign) and any(isinstance(t,ast.Name)and t.id=='name'for t in node.targets)and isinstance(node.value,ast.BinOp)and isinstance(node.value.left,ast.Constant)and isinstance(node.value.right,ast.Name)and node.value.right.id=='kind']
  self.assertEqual(len(names),1)
  source=(ROOT/'src/plugins/place_priority_tests.rs').read_text()
  self.assertIn('format!("'+names[0]+'{kind}")',source)
  limit=int(re.search(r'const MAX_NAME: usize = (\d+);',(ROOT/'src/plugins/manifest.rs').read_text())[1])
  for kind in ['missing','type']:
   actual=names[0]+kind
   self.assertLessEqual(len(actual),limit)
   self.assertRegex(actual,r'^[a-z0-9_-]+$')
   # Exact builder transformation of translator's own valid manifest identity.
   manifest='name = "java-place-staged-a"\nmain = "plugin.wasm"\n'
   changed=manifest.replace('java-place-staged-a',actual)
   self.assertEqual(tomllib.loads(changed)['name'],actual)
  self.assertIn("p.write_text(p.read_text().replace('java-place-staged-a',name))",builder)
