"""Synthetic owned-file metadata checks only: no Cargo/executable/runtime launch."""
import importlib.util,tempfile,unittest,json,copy
from pathlib import Path
SPEC=importlib.util.spec_from_file_location('chunk_ci',Path(__file__).with_name('public_container_button_ci.py'));CI=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(CI)

class Ownership(unittest.TestCase):
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name);self.old=CI.ROOT;CI.ROOT=self.root
  self.manifest=self.root/'Cargo.toml';self.manifest.write_text('[package]\nname="own-fixture"\nversion="0.1.0"\n')
  self.source=self.root/'src/main.rs';self.source.parent.mkdir();self.source.write_text('fn main() {}')
  self.exe=self.root/'target/release/deps/own_fixture-0123456789abcdef';self.exe.parent.mkdir(parents=True)
  header=bytearray(64);header[:6]=b'\x7fELF\x02\x01';header[18:20]=(62).to_bytes(2,'little');self.exe.write_bytes(header)
  self.message={'reason':'compiler-artifact','profile':{'test':True},'executable':str(self.exe),'manifest_path':str(self.manifest),'target':{'name':'own-fixture','kind':['bin'],'src_path':str(self.source)},'features':[]}
 def tearDown(self):CI.ROOT=self.old;self.tmp.cleanup()
 def test_owned_test_metadata_is_distinct_from_standalone(self):
  self.assertEqual(CI.test_executable(self.message,{self.manifest},{self.source}),(self.exe,self.manifest))
  self.message['profile']['test']=False;self.assertIsNone(CI.test_executable(self.message,{self.manifest},{self.source}))
 def test_unowned_manifest_or_path_is_rejected(self):
  with self.assertRaises(ValueError):CI.test_executable(self.message,set(),{self.source})
  foreign=self.root/'foreign-elf';foreign.write_bytes(self.exe.read_bytes());self.message['executable']=str(foreign)
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},{self.source})
 def test_unreviewed_entry_source_is_rejected(self):
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},set())
 def test_symlink_reused_path_and_wrong_arch_cannot_be_retained(self):
  link=self.exe.parent/'link';link.symlink_to(self.exe);self.message['executable']=str(link)
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},{self.source})
  self.message['executable']=str(self.exe);raw=bytearray(self.exe.read_bytes());raw[18:20]=(183).to_bytes(2,'little');self.exe.write_bytes(raw)
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},{self.source})
 def test_malformed_features_or_non_test_target_is_rejected(self):
  self.message['features']='malformed-features'
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},{self.source})
  self.message['features']=[];self.message['target']['kind']=['custom-build']
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},{self.source})
 def test_noncompiler_output_cannot_create_executable_claim(self):
  for value in [None,{},[],{'reason':'build-script-executed'}, {'reason':'compiler-artifact','profile':{'test':False},'executable':str(self.exe)}]:self.assertIsNone(CI.test_executable(value,{self.manifest},{self.source}))
 def test_actual_seedmap_cdylib_rlib_test_metadata_is_retained(self):
  fixture=json.loads((self.old/'tools/protocol-bridge/testdata/own-seedmap-test-artifacts.json').read_text())
  self.assertEqual(len(fixture['messages']),3)
  for entry in fixture['messages']:
   message=copy.deepcopy(entry['message']);self.assertEqual(message['target']['kind'],['cdylib','rlib'])
   self.assertTrue(message['profile']['test']);self.assertEqual(message['target']['name'],'seedmap_core')
   message['manifest_path']=str(self.manifest);message['target']['src_path']=str(self.source);message['executable']=str(self.exe)
   self.assertEqual(CI.test_executable(message,{self.manifest},{self.source}),(self.exe,self.manifest))
   message['profile']['test']=False;self.assertIsNone(CI.test_executable(message,{self.manifest},{self.source}))
 def test_unknown_multi_kind_or_unowned_seedmap_source_still_rejects(self):
  self.message['target']['kind']=['cdylib','rlib']
  with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},set())
  for kinds in [['cdylib'],['rlib'],['cdylib','bin'],['lib','test']]:
   self.message['target']['kind']=kinds
   with self.assertRaises(ValueError):CI.test_executable(self.message,{self.manifest},{self.source})
class GuestInputs(unittest.TestCase):
 def test_actual_public_lock_and_all20_current_inputs_match(self):
  pin=CI.example_pin();self.assertEqual(len(pin['old_inputs_sha256']),20)
  self.assertEqual(pin['lock_sha256'],'5c9b19a1cc30825633fc26d39910a6649ad26a3c15f7a2014a2c290fe7a8e179')
 def test_changed_public_lock_or_unsafe_path_refuses_before_guest_build(self):
  original=json.loads((CI.ROOT/CI.EXAMPLE_PIN).read_text());old_root=CI.ROOT
  for kind in ['changed-lock','unsafe-path']:
   with tempfile.TemporaryDirectory() as t:
    CI.ROOT=Path(t);pin=copy.deepcopy(original)
    if kind=='changed-lock':pin['lock_text']+='bad'
    else:
     entries=list(pin['old_inputs_sha256'].items());pin['old_inputs_sha256']={'../not-own-source':'0'*64,**dict(entries[1:])}
    path=CI.ROOT/CI.EXAMPLE_PIN;path.parent.mkdir(parents=True);path.write_text(json.dumps(pin))
    try:
     with self.assertRaises(ValueError):CI.example_pin()
    finally:CI.ROOT=old_root
 def test_changed_source_inputs_cannot_use_the_previous_lock_lineage(self):
  old_root=CI.ROOT;pin=json.loads((old_root/CI.EXAMPLE_PIN).read_text())
  with tempfile.TemporaryDirectory() as t:
   CI.ROOT=Path(t)
   try:
    for name in pin['old_inputs_sha256']:
     path=CI.ROOT/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes((old_root/name).read_bytes())
    first=next(iter(pin['old_inputs_sha256']));(CI.ROOT/first).write_bytes(b'changed own source')
    path=CI.ROOT/CI.EXAMPLE_PIN;path.parent.mkdir(parents=True);path.write_text(json.dumps(pin))
    with self.assertRaises(ValueError):CI.example_pin()
   finally:CI.ROOT=old_root

class CompiledInputs(unittest.TestCase):
 def test_current_metadata_and_guest_config_are_in_source_archive_closure(self):
  paths=CI.selected();self.assertIn('tools/protocol-bridge/metadata-776-report.json',paths);self.assertIn('plugins-examples/custom-tablist/config.yml',paths)
 def test_unknown_or_untracked_compiled_input_fails_instead_of_silent_omission(self):
  old_root=CI.ROOT
  with tempfile.TemporaryDirectory() as t:
   CI.ROOT=Path(t);source=CI.ROOT/'own.rs'
   try:
    for text in ['include_str!(dynamic_path)','include_bytes!("missing.bin")']:
     source.write_text(text)
     with self.assertRaises(ValueError):CI.literal_inputs(['own.rs'])
   finally:CI.ROOT=old_root
 def test_forbidden_archive_extension_and_outside_path_refuse_without_reading(self):
  old_root=CI.ROOT
  with tempfile.TemporaryDirectory() as t:
   CI.ROOT=Path(t);source=CI.ROOT/'own.rs'
   try:
    for text in ['include_bytes!("never-read.jar")','include_str!("../outside.txt")']:
     source.write_text(text)
     with self.assertRaises(ValueError):CI.literal_inputs(['own.rs','never-read.jar'])
   finally:CI.ROOT=old_root

if __name__=='__main__':unittest.main(verbosity=2)
