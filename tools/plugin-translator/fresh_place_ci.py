#!/usr/bin/env python3
"""Fresh optionalBlockPlace CI: genuinely new default test ELF; no restored b5 provenance."""
from pathlib import Path
import hashlib,importlib.util,json,os,shutil,subprocess,sys,tarfile
ROOT=Path(__file__).resolve().parents[2]
BASE='ce5720486d42f8da46e5816bc4792b7ba4d2ed02'
PIN='tools/plugin-translator/fixtures/place-fresh-review.json'
HISTORICAL_TEST_SHA='b5c46d45b52b6f0a2f8ea6cc24c8fbd002cb8d553e6cbacd9762170d3bfa33c9'
TEST_NAME='mcsheriffanya';TEST_MANIFEST='Cargo.toml';TEST_SOURCE='src/main.rs'
WORKFLOW='.github/workflows/bukkit-place-fresh-checks.yml'
CLOSURE=[PIN,WORKFLOW,'tools/plugin-translator/fresh_place_ci.py','tools/plugin-translator/test_fresh_place_ci.py','tools/plugin-translator/verify-place-staged.py','tools/plugin-translator/place_staged_ci.py','tools/plugin-translator/test_place_staged_ci.py']
OUT=ROOT/'place-staged-evidence'
sys.path.insert(0,str(Path(__file__).resolve().parent))
import place_staged_ci as p
_spec=importlib.util.spec_from_file_location('public_container_button_ci',ROOT/'tools/protocol-bridge/public_container_button_ci.py')
public=importlib.util.module_from_spec(_spec);_spec.loader.exec_module(public)
def git(*a):return subprocess.check_output(['git',*a],cwd=ROOT)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
FORBIDDEN=('.jar','.apk','.env','.token','.pem')
def check_path(relative):
 path=Path(relative)
 if not relative or path.is_absolute() or '..'in path.parts or '\\'in relative:raise ValueError('unsafe reviewed path: '+relative)
 if relative.endswith(FORBIDDEN) or relative.startswith(('target/','tmp/')):raise ValueError('forbidden reviewed path: '+relative)
 if not (ROOT/relative).is_file() or (ROOT/relative).is_symlink():raise ValueError('reviewed path is not a regular source file: '+relative)
def review():
 pin=json.loads((ROOT/PIN).read_text())
 if not isinstance(pin,dict)or pin.get('base')!=BASE:raise ValueError('review pin is not fixed merged MAIN ce572')
 reviewed=pin.get('changed_files')
 if not isinstance(reviewed,dict):raise ValueError('review pin has no changed_files map')
 if PIN in reviewed:raise ValueError('reviewed map must exclude the pin itself')
 changed=[name for name in git('diff','--name-only',BASE,'HEAD').decode().splitlines() if name]
 if set(changed)!=set(reviewed)|{PIN}:raise ValueError('git diff base..HEAD differs from reviewed changed_files scope')
 for name,value in reviewed.items():
  check_path(name)
  if not isinstance(value,str)or sha(ROOT/name)!=value:raise ValueError('reviewed source hash mismatch: '+name)
 return sorted(reviewed)
def prepare():
 # public.prepare() covers only the ContainerButton scope; deliberately never called here.
 changed=review();p.OUT=OUT
 p.lock();p.inputs()
 pin=json.loads((ROOT/PIN).read_text())
 sources={name:sha(ROOT/name)for name in public.selected()}
 sources.update({name:sha(ROOT/name)for name in changed})
 sources.update({name:sha(ROOT/name)for name in CLOSURE})
 record={'base':BASE,'source':git('rev-parse','HEAD').decode().strip(),'tree':git('rev-parse','HEAD^{tree}').decode().strip(),
  'lock_sha256':public.file_sha(ROOT/'Cargo.lock'),'changed_files':pin['changed_files'],'sources':sources,
  'scope':'reviewed optionalBlockPlace source only; OLD WASM inputs immutable; genuinely new default test ELF'}
 (OUT/'prepare.json').write_text(json.dumps(record,indent=2)+'\n')
def _confirm(record):
 for name,expected in record['sources'].items():
  if sha(ROOT/name)!=expected:raise ValueError('prepared source changed during the run: '+name)
 if public.file_sha(ROOT/'Cargo.lock')!=record['lock_sha256']:raise ValueError('verified root Cargo.lock changed during the run')
def _elf(path):
 with Path(path).open('rb')as stream:header=stream.read(20)
 if len(header)!=20 or header[:6]!=b'\x7fELF\x02\x01'or int.from_bytes(header[18:20],'little')!=62:raise ValueError('not an own x64 ELF: '+str(path))
def _candidate(message):
 if not isinstance(message,dict):raise ValueError('malformed compiler-artifact line')
 profile=message.get('profile');target=message.get('target')
 if message.get('reason')!='compiler-artifact'or not isinstance(profile,dict)or not isinstance(target,dict):return None
 if profile.get('test')is not True or target.get('name')!=TEST_NAME:return None
 if target.get('kind')!=['bin']:raise ValueError('fresh mcsheriffanya test artifact is not the own bin target')
 features=message.get('features')
 if not isinstance(features,list)or'plugins'not in features:raise ValueError('fresh test binary lacks the plugins feature')
 if Path(message.get('manifest_path','')).resolve()!=(ROOT/TEST_MANIFEST).resolve():raise ValueError('fresh test artifact is not from the root manifest')
 if Path(target.get('src_path','')).resolve()!=(ROOT/TEST_SOURCE).resolve():raise ValueError('fresh test artifact is not built from root src/main.rs')
 executable=Path(message.get('executable')or'')
 if executable.is_symlink():raise ValueError('fresh default test executable is a symlink')
 executable=executable.resolve();deps=(ROOT/'target/release/deps').resolve()
 if executable.parent!=deps or not executable.is_file():raise ValueError('fresh default test executable outside own target/release/deps')
 _elf(executable);digest=public.file_sha(executable)
 if digest==HISTORICAL_TEST_SHA:raise ValueError('restored b5 historical test ELF bytes are invalid for the fresh run')
 return message,executable,digest
def select_runtime():
 prepare=json.loads((OUT/'prepare.json').read_text());_confirm(prepare)
 head=git('rev-parse','HEAD').decode().strip();tree=git('rev-parse','HEAD^{tree}').decode().strip()
 if head!=prepare['source']or tree!=prepare['tree']:raise ValueError('current checkout is not the prepared source')
 candidates=[]
 for line in (OUT/'default-tests.log').read_text(errors='replace').splitlines():
  if not line.startswith('{'):continue
  try:message=json.loads(line)
  except json.JSONDecodeError:continue
  found=_candidate(message)
  if found is not None:candidates.append(found)
 if len(candidates)!=1:raise ValueError('default-tests.log must contain exactly one own plugins test-bin artifact, got %d'%len(candidates))
 message,executable,digest=candidates[0]
 server=ROOT/'target/release/mcsheriffanya'
 if not server.is_file()or server.is_symlink():raise ValueError('ordinary fresh server binary missing')
 _elf(server)
 record={'base':BASE,'source':head,'tree':tree,
  'lock_sha256':public.file_sha(ROOT/'Cargo.lock'),'test_elf':str(executable.relative_to(ROOT)),'test_elf_sha256':digest,
  'server_elf_sha256':public.file_sha(server),'log':'default-tests.log',
  'compiler_record':{'manifest_path':message['manifest_path'],'target':message['target'],'features':message['features'],'profile':message['profile'],'executable':message['executable']}}
 retained=OUT/'fresh-test-elf'/executable.name;retained.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(executable,retained)
 if public.file_sha(retained)!=digest:raise ValueError('fresh default test ELF changed while retained')
 (OUT/'fresh-runtime.json').write_text(json.dumps(record,indent=2)+'\n')
def _revalidate():
 prepare=json.loads((OUT/'prepare.json').read_text());_confirm(prepare)
 head=git('rev-parse','HEAD').decode().strip();tree=git('rev-parse','HEAD^{tree}').decode().strip()
 if head!=prepare['source']or tree!=prepare['tree']:raise ValueError('current checkout is not the prepared source')
 record=json.loads((OUT/'fresh-runtime.json').read_text())
 if record['source']!=head or record['tree']!=tree:raise ValueError('fresh runtime record inconsistent with current prepared source')
 relative=record['test_elf']
 if Path(relative).is_absolute()or'..'in Path(relative).parts or not relative.startswith('target/release/deps/'):raise ValueError('inconsistent fresh test ELF path')
 executable=ROOT/relative
 if executable.is_symlink()or not executable.is_file()or public.file_sha(executable)!=record['test_elf_sha256']:raise ValueError('recorded fresh test ELF missing or changed')
 _elf(executable)
 if record['test_elf_sha256']==HISTORICAL_TEST_SHA:raise ValueError('historical b5 ELF cannot be the fresh runtime record')
 if public.file_sha(ROOT/'target/release/mcsheriffanya')!=record['server_elf_sha256']:raise ValueError('fresh server binary changed after selection')
 return record
def translations():
 _revalidate()
 p.OUT=OUT
 p.translations()
def tests():
 record=_revalidate()
 p.OUT=OUT
 p.RETAINED_TEST=Path(record['test_elf']).name;p.RETAINED_TEST_SHA=record['test_elf_sha256']
 p.tests()
def source_closure():
 prepare=json.loads((OUT/'prepare.json').read_text());archive=OUT/'source-closure.tar.gz'
 assert not archive.exists()
 with tarfile.open(archive,'w:gz') as tar:
  for name,expected in sorted(prepare['sources'].items()):
   path=ROOT/name;check_path(name)
   if sha(path)!=expected:raise ValueError('pinned source changed: '+name)
   tar.add(path,arcname=name)
def finish():
 p.OUT=OUT
 try:
  source_closure();errors=public.retain_tests(OUT)
 finally:p.finish()
 if errors:raise SystemExit('retained test ELF errors: %r; all evidence preserved'%errors)
def main():
 if os.environ.get('GITHUB_ACTIONS')!='true':raise SystemExit('Этот сценарий разрешён только в GitHub Actions')
 if os.environ.get('GITHUB_RUN_ATTEMPT')!='1':raise SystemExit('Повторный запуск этого сценария запрещён')
 modes={'prepare':prepare,'select-runtime':select_runtime,'translations':translations,'tests':tests,'finish':finish}
 if len(sys.argv)!=2 or sys.argv[1] not in modes:raise SystemExit('usage: fresh_place_ci.py prepare|select-runtime|translations|tests|finish')
 p.OUT=OUT
 modes[sys.argv[1]]()
if __name__=='__main__':main()
