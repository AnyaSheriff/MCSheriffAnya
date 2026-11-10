"""Гарды GitHub-only, точность области review и выбор свежего тестового ELF."""
import ast,hashlib,importlib.util,json,os,re,struct,tempfile,unittest
from contextlib import contextmanager
from pathlib import Path
from unittest.mock import patch
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('fresh_place_ci_subject',HERE/'fresh_place_ci.py')
fresh=importlib.util.module_from_spec(spec);spec.loader.exec_module(fresh)
MODES=['prepare','select-runtime','translations','tests','finish']
def elf_bytes(arch=62):
 header=bytearray(20);header[:4]=b'\x7fELF';header[4:7]=b'\x02\x01\x01';header[18:20]=struct.pack('<H',arch);return bytes(header)
def digest(data):return hashlib.sha256(data).hexdigest()
def artifact(root,path,**over):
 record={'reason':'compiler-artifact','profile':{'test':True},'target':{'kind':['bin'],'name':'mcsheriffanya','src_path':str(root/'src/main.rs')},'features':['plugins'],'manifest_path':str(root/'Cargo.toml'),'executable':str(path)}
 for key,value in over.items():
  if key=='src_path':record['target']['src_path']=value
  elif key=='profile':record['profile'].update(value)
  else:record[key]=value
 return json.dumps(record)
def build(root):
 (root/'src').mkdir(parents=True,exist_ok=True)
 (root/'src/main.rs').write_text('// fresh')
 (root/'Cargo.toml').write_text('[package]\nname="mcsheriffanya"\n')
 (root/'Cargo.lock').write_text('lock')
 (root/'target/release').mkdir(parents=True,exist_ok=True)
 (root/'target/release/mcsheriffanya').write_bytes(elf_bytes())
 (root/'target/release/deps').mkdir(parents=True,exist_ok=True)
 executable=root/'target/release/deps/mcsheriffanya-fresh123'
 executable.write_bytes(elf_bytes())
 return executable
@contextmanager
def runtime_root():
 with tempfile.TemporaryDirectory() as holder:
  root=Path(holder)/'root';out=Path(holder)/'out';root.mkdir();out.mkdir()
  executable=build(root)
  (out/'default-tests.log').write_text(artifact(root,executable)+'\n')
  prepare={'source':'fresh','tree':'fresh','lock_sha256':digest(b'lock'),'sources':{'src/main.rs':digest((root/'src/main.rs').read_bytes()),'Cargo.toml':digest((root/'Cargo.toml').read_bytes()),'Cargo.lock':digest(b'lock')}}
  (out/'prepare.json').write_text(json.dumps(prepare))
  with patch.object(fresh,'OUT',out),patch.object(fresh,'ROOT',root),patch.object(fresh,'git',lambda *a:b'fresh\n'):
   yield root,out,executable
def rewrite(out,lines):
 (out/'default-tests.log').write_text(''.join(line+'\n' for line in lines))
class FreshCiGuards(unittest.TestCase):
 def test_github_only_attempt1_guard_before_all_modes(self):
  for actions,attempt in [('false','1'),('true','2')]:
   with patch.dict(os.environ,{'GITHUB_ACTIONS':actions,'GITHUB_RUN_ATTEMPT':attempt},clear=True):
    with patch.object(fresh,'prepare')as a,patch.object(fresh,'select_runtime')as b,patch.object(fresh,'translations')as c,patch.object(fresh,'tests')as d,patch.object(fresh,'finish')as e:
     for mode in MODES:
      with patch.object(fresh.sys,'argv',['fresh_place_ci.py',mode]):
       with self.assertRaises(SystemExit):fresh.main()
      for spy in (a,b,c,d,e):spy.assert_not_called()
 def test_prepare_rejects_and_accepts_review_scope_exact_hashes(self):
  with tempfile.TemporaryDirectory() as holder:
   root=Path(holder);fixture=root/fresh.PIN;fixture.parent.mkdir(parents=True)
   (root/'src').mkdir();keep=root/'src/keep.rs';keep.write_text('x')
   reviewed={'src/keep.rs':digest(keep.read_bytes())}
   def mounted(text,pin):
    fixture.write_text(json.dumps({'base':fresh.BASE,'changed_files':pin}))
    return patch.object(fresh,'ROOT',root),patch.object(fresh,'git',lambda *a:text)
   exact=('src/keep.rs\n'+fresh.PIN+'\n').encode()
   extra=exact+b'src/hack.rs\n'
   patches=mounted(extra,reviewed)
   with patches[0],patches[1]:
    with self.assertRaisesRegex(ValueError,'scope'):fresh.review()
   patches=mounted(exact,reviewed)
   with patches[0],patches[1]:self.assertEqual(fresh.review(),['src/keep.rs'])
   keep.write_text('tampered')
   with patches[0],patches[1]:
    with self.assertRaisesRegex(ValueError,'hash mismatch'):fresh.review()
   patches=mounted(exact,{**reviewed,fresh.PIN:digest(b'should be excluded')})
   with patches[0],patches[1]:
    with self.assertRaisesRegex(ValueError,'exclude the pin'):fresh.review()
class FreshSelectRuntime(unittest.TestCase):
 def test_valid_selection_writes_record_and_retained_copy(self):
  with runtime_root()as (root,out,executable):
   fresh.select_runtime()
   record=json.loads((out/'fresh-runtime.json').read_text())
   self.assertEqual(record['source'],'fresh');self.assertEqual(record['tree'],'fresh')
   self.assertEqual(record['test_elf'],str(executable.relative_to(root)))
   self.assertEqual(record['test_elf_sha256'],digest(executable.read_bytes()))
   copy=out/'fresh-test-elf'/executable.name
   self.assertTrue(copy.is_file());self.assertEqual(digest(copy.read_bytes()),record['test_elf_sha256'])
   self.assertEqual(record['compiler_record']['features'],['plugins'])
 def test_each_rejected_variant(self):
  rewritten={'features':['other'],'manifest_path':None,'src_path':None,'executable':None}
  for label in sorted(rewritten):
   with self.subTest(label):
    with runtime_root()as (root,out,executable):
     alternate={'manifest_path':str(root/'plugins-examples/Cargo.toml'),'src_path':str(root/'src/lib.rs'),'executable':str(root/'target/release/mcsheriffanya')}
     value=rewritten[label]if rewritten[label]is not None else alternate[label]
     rewrite(out,[artifact(root,executable,**{'features':value}if label=='features'else{label:value})])
     with self.assertRaises(ValueError):fresh.select_runtime()
 def test_wrong_profile_missing_and_duplicate_records_rejected(self):
  for label,maker in zip(['wrong_profile','no_record','garbage','duplicate'],[
   lambda root,executable:[artifact(root,executable,profile={'test':False})],
   lambda root,executable:[artifact(root,executable,features=['x'])],
   lambda root,executable:['plain output line'],
   lambda root,executable:[artifact(root,executable),artifact(root,executable)]]):
   with self.subTest(label):
    with runtime_root()as (root,out,executable):
     rewrite(out,maker(root,executable))
     with self.assertRaises(ValueError):fresh.select_runtime()
 def test_symlink_arch_and_historical_sha_rejections(self):
  with runtime_root()as (root,out,executable):
   real=root/'target/release/deps/mcsheriffanya-real';real.write_bytes(elf_bytes())
   link=root/'target/release/deps/mcsheriffanya-link';link.symlink_to(real)
   rewrite(out,[artifact(root,link)])
   with self.assertRaisesRegex(ValueError,'symlink'):fresh.select_runtime()
  with runtime_root()as (root,out,executable):
   executable.write_bytes(elf_bytes(arch=183))
   with self.assertRaisesRegex(ValueError,'x64 ELF'):fresh.select_runtime()
  with runtime_root()as (root,out,executable):
   with patch.object(fresh,'HISTORICAL_TEST_SHA',digest(executable.read_bytes())):
    with self.assertRaisesRegex(ValueError,'historical'):fresh.select_runtime()
 def test_historical_basename_with_new_bytes_accepted_and_mocked_sha_rejected(self):
  with runtime_root()as (root,out,executable):
   preserved=root/'target/release/deps/mcsheriffanya-628cb65e72047376';preserved.write_bytes(elf_bytes())
   with patch.object(fresh,'HISTORICAL_TEST_SHA',digest(b'old b5 bytes')):
    rewrite(out,[artifact(root,preserved)])
    fresh.select_runtime()
    record=json.loads((out/'fresh-runtime.json').read_text())
    self.assertEqual(record['test_elf'],str(preserved.relative_to(root)))
    self.assertEqual(record['test_elf_sha256'],digest(preserved.read_bytes()))
  with runtime_root()as (root,out,executable):
   actual=digest(executable.read_bytes())
   def known_sha(path):
    return actual if str(path)==str(executable)else hashlib.sha256(Path(path).read_bytes()).hexdigest()
   with patch.object(fresh,'HISTORICAL_TEST_SHA',actual),patch.object(fresh.public,'file_sha',known_sha):
    with self.assertRaisesRegex(ValueError,'historical'):fresh.select_runtime()
 def test_prepared_source_mutation_rejected(self):
  with runtime_root()as (root,out,executable):
   (root/'src/main.rs').write_text('// changed')
   with self.assertRaisesRegex(ValueError,'changed during the run'):fresh.select_runtime()
class FreshModes(unittest.TestCase):
 def test_translations_runs_existing_translation_only_after_revalidate(self):
  with runtime_root()as (root,out,executable):
   calls=[]
   with patch.object(fresh,'_revalidate',side_effect=lambda:calls.append('revalidate')),patch.object(fresh.p,'translations',side_effect=lambda:calls.append('translations')),patch.object(fresh.p,'OUT',None):
    fresh.translations()
    self.assertEqual(calls,['revalidate','translations'])
    self.assertEqual(fresh.p.OUT,out)
  with runtime_root()as (root,out,executable):
   with patch.object(fresh,'_revalidate',side_effect=ValueError('rejected')),patch.object(fresh.p,'translations')as plan:
    with self.assertRaisesRegex(ValueError,'rejected'):fresh.translations()
    plan.assert_not_called()
 def test_tests_mode_arms_actual_fresh_record_and_invokes_existing_plan(self):
  with runtime_root()as (root,out,executable):
   fresh.select_runtime()
   with patch.object(fresh.p,'tests')as plan,patch.object(fresh.p,'RETAINED_TEST',''),patch.object(fresh.p,'RETAINED_TEST_SHA',''):
    fresh.tests()
    self.assertEqual(fresh.p.RETAINED_TEST,executable.name)
    self.assertEqual(fresh.p.RETAINED_TEST_SHA,digest(executable.read_bytes()))
    plan.assert_called_once_with()
 def test_finish_preserves_then_acts_on_retention_errors(self):
  with patch.object(fresh.p,'finish')as keep,patch.object(fresh.public,'retain_tests')as retain,patch.object(fresh,'source_closure')as closure:
   retain.return_value=[]
   fresh.finish()
   closure.assert_called_once_with();retain.assert_called_once_with(fresh.OUT);keep.assert_called_once_with()
   retain.return_value=[{'log':'default-tests.log','error':'x'}]
   with self.assertRaises(SystemExit):fresh.finish()
   self.assertEqual(keep.call_count,2)
   self.assertEqual(retain.call_count,2)
class FreshWorkflowContract(unittest.TestCase):
 def test_workflow_cli_modes_match_actual_dispatch_keys(self):
  tree=ast.parse((fresh.ROOT/'tools/plugin-translator/fresh_place_ci.py').read_text())
  main=next(node for node in tree.body if isinstance(node,ast.FunctionDef)and node.name=='main')
  modes=next(node for node in ast.walk(main) if isinstance(node,ast.Dict)and node.keys and all(isinstance(k,ast.Constant)for k in node.keys))
  keys={k.value for k in modes.keys}
  workflow=(fresh.ROOT/'.github/workflows/bukkit-place-fresh-checks.yml').read_text()
  invoked={re.search(r'tools/plugin-translator/fresh_place_ci\.py\s+(\S+)',line).group(1)for line in workflow.splitlines()if re.search(r'tools/plugin-translator/fresh_place_ci\.py\s+(\S+)',line)}
  self.assertEqual(invoked,keys)
 def test_focused_filters_are_exact_and_focused_counts_required(self):
  workflow=(fresh.ROOT/'.github/workflows/bukkit-place-fresh-checks.yml').read_text()
  self.assertIn('mcsa-protocol --message-format=json-render-diagnostics container_button::tests::',workflow)
  self.assertIn('--bin mcsheriffanya --message-format=json-render-diagnostics container_button_menu_tests::',workflow)
  for line in workflow.splitlines():
   if 'container_button::'in line:self.assertIn('--test-threads=1',line)
  verifier=(fresh.ROOT/'tools/plugin-translator/verify-place-staged.py').read_text()
  self.assertIn("'in (A/'button-default.log')",verifier)
  self.assertIn("'in (A/'focused-runtime.log')",verifier)
  self.assertIn('ok. 8 passed; 0 failed;',verifier)
  self.assertIn('ok. 4 passed; 0 failed;',verifier)
if __name__=='__main__':unittest.main()
