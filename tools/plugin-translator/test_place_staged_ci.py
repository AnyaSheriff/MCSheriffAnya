"""Защита от локального запуска и потери доступа к проверяемым CI-артефактам."""
import ast
import importlib.util
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('place_ci_guard_subject',HERE/'place_staged_ci.py')
ci=importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)

class PlaceCiGuards(unittest.TestCase):
 def test_local_and_repeat_stop_before_mode_side_effects(self):
  for actions,attempt in [('false','1'),('true','2')]:
   with patch.dict(os.environ,{'GITHUB_ACTIONS':actions,'GITHUB_RUN_ATTEMPT':attempt},clear=True):
    for name in ['lock','inputs','strict','translations','tests','finish']:
     with patch.object(ci,name) as called,patch.object(ci.sys,'argv',['place_staged_ci.py',name]):
      with self.assertRaises(SystemExit):ci.main()
      called.assert_not_called()

 def test_missing_token_stops_before_download_or_directory_creation(self):
  with tempfile.TemporaryDirectory() as directory:
   out=Path(directory)/'no-output'
   with patch.dict(os.environ,{},clear=True),patch.object(ci,'OUT',out),patch.object(ci.subprocess,'check_output') as api:
    with self.assertRaisesRegex(RuntimeError,'GH_TOKEN'):ci.archive('reuse-failed')
    api.assert_not_called()
    self.assertFalse(out.exists())

 def test_every_transitive_archive_mode_has_workflow_token(self):
  tree=ast.parse((HERE/'place_staged_ci.py').read_text())
  functions={node.name:node for node in tree.body if isinstance(node,ast.FunctionDef)}
  calls={name:{node.func.id for node in ast.walk(body) if isinstance(node,ast.Call) and isinstance(node.func,ast.Name)} for name,body in functions.items()}
  def needs_archive(name,seen=None):
   seen=set() if seen is None else seen
   if name=='archive':return True
   if name in seen:return False
   seen.add(name)
   return any(needs_archive(child,seen) for child in calls.get(name,set()))
  handlers=next(node.value for node in ast.walk(functions['main']) if isinstance(node,ast.Assign) and any(isinstance(target,ast.Name) and target.id=='handlers' for target in node.targets))
  modes={key.value:value.id for key,value in zip(handlers.keys,handlers.values)}
  needed={mode for mode,function in modes.items() if needs_archive(function)}
  self.assertEqual(needed,{'lock','inputs','strict'})
  workflow=(ci.ROOT/'.github/workflows/bukkit-place-e2e-checks.yml').read_text()
  found=set()
  for block in re.split(r'(?m)^      - ',workflow):
   mode=re.search(r'run: python3 -B tools/plugin-translator/place_staged_ci.py (\w+)\s*$',block,re.M)
   if not mode:continue
   found.add(mode.group(1))
   if mode.group(1) in needed:self.assertIn('GH_TOKEN: ${{ github.token }}',block)
  self.assertEqual(found,set(modes))

if __name__=='__main__':unittest.main()
