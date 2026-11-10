"""Own source-only GitHub checks: no native client/server execution or activation."""
import argparse,hashlib,json,os,shutil,subprocess,tarfile,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
MAIN='8b9a87182b7f540ff56d1afa45775078882739b4'
REVIEW='2fe9577a2b9abbcf5a8660282f32c4b5ad9a088e'
WORKFLOW='.github/workflows/private768-chunk-source-checks.yml'
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
def sha(data):return hashlib.sha256(data).hexdigest()
def literal_inputs(paths):
 tracked=set(paths);result=set()
 for name in paths:
  if not name.endswith('.rs'):continue
  text=(ROOT/name).read_text()
  for match in re.finditer(r'include_(?:str|bytes)!\s*\(',text):
   literal=re.match(r'\s*"([^"\\]+)"\s*\)',text[match.end():])
   if not literal:raise ValueError('unreviewed nonliteral compiled input: '+name)
   path=(ROOT/name).parent.joinpath(literal[1]).resolve()
   if not path.is_relative_to(ROOT.resolve()):raise ValueError('compiled input outside own source')
   relative=str(path.relative_to(ROOT.resolve()))
   if path.suffix.lower() in {'.jar','.apk','.env','.token','.pem'} or relative=='config/rustcraft.toml':raise ValueError('forbidden compiled input archive path')
   if relative not in tracked or not path.is_file():raise ValueError('untracked compiled input: '+relative)
   result.add(relative)
 return result

def selected():
 paths=git('ls-files','-z').decode().strip('\0').split('\0')
 selected={p for p in paths if p.endswith('.rs') or p.endswith('.wit') or p.endswith('Cargo.toml') or p in [WORKFLOW,'tools/blackbox/ci/simulation-distance-two-runtime.lock','tools/protocol-bridge/private768_chunk_ci.py','tools/protocol-bridge/private768-acceptance.json','tools/protocol-bridge/client-chain-sources.json','tools/protocol-bridge/private768-extra-sources.json','tools/protocol-bridge/private768-chunk-review-pin.json','tools/protocol-bridge/testdata/container-button-fields.json','tools/protocol-bridge/audit_container_button_fields.py','tools/protocol-bridge/step1-sources-774.json','tools/protocol-bridge/packet-776-report.json','tools/protocol-bridge/test_private768_chunk_ci.py','tools/protocol-bridge/testdata/owned-rust-examples-lock.json'] or p.startswith('tools/protocol-bridge/testdata/private768-reference/')}
 return sorted(selected|literal_inputs(paths))
def prepare(out):
 out.mkdir(exist_ok=False)
 subprocess.run(['git','merge-base','--is-ancestor',MAIN,'HEAD'],cwd=ROOT,check=True)
 subprocess.run(['git','merge-base','--is-ancestor',REVIEW,'HEAD'],cwd=ROOT,check=True)
 pin=json.loads((ROOT/'tools/protocol-bridge/private768-chunk-review-pin.json').read_text())
 for path,expected in pin['unchanged_reviewed_sources'].items():assert sha((ROOT/path).read_bytes())==expected,path
 for path in ['src/network/play/initial_776_probe.rs','tools/protocol-bridge/src/private768_tags.rs']:
  assert (ROOT/path).read_bytes()==git('show',REVIEW+':'+path),path
 for needle in ['RequiredTags::reviewed37()', '.complete()']:
  actual=(ROOT/'tools/protocol-bridge/src/private768.rs').read_text().split('pub struct Session {')[1].split('fn serverbound(body:')[0]
  assert needle not in actual,needle
 lock=ROOT/'tools/blackbox/ci/simulation-distance-two-runtime.lock';shutil.copyfile(lock,ROOT/'Cargo.lock')
 (out/'prepare.json').write_text(json.dumps({'source':git('rev-parse','HEAD').decode().strip(),'tree':git('rev-parse','HEAD^{tree}').decode().strip(),'main':MAIN,'review':REVIEW,'lock_sha256':sha(lock.read_bytes()),'no_native':True,'tag37_inactive':True},indent=2)+'\n')
EXAMPLE_NAMES=('hello','misbehave','chat_filter','custom_tablist','modinfo','protect_spawn')
EXAMPLE_PIN='tools/protocol-bridge/testdata/owned-rust-examples-lock.json'
def example_pin():
 pin=json.loads((ROOT/EXAMPLE_PIN).read_text())
 if sha(pin['lock_text'].encode())!=pin['lock_sha256'] or len(pin['old_inputs_sha256'])!=20:raise ValueError('unverified public own-example lock/input pin')
 for name,expected in pin['old_inputs_sha256'].items():
  path=Path(name)
  if path.is_absolute() or '..' in path.parts or not (name.startswith('plugins-examples/') or name=='wit/mcsa-api.wit'):raise ValueError('unexpected own-example source path')
  if sha((ROOT/path).read_bytes())!=expected:raise ValueError('own-example source differs from approved old input: '+name)
 return pin

def examples_lock(out):
 pin=example_pin();lock=ROOT/'plugins-examples/Cargo.lock';raw=pin['lock_text'].encode()
 if lock.exists() and lock.read_bytes()!=raw:raise ValueError('unexpected existing guest lock')
 lock.write_bytes(raw);shutil.copyfile(lock,out/'plugins-examples-Cargo.lock')
 (out/'own-examples-inputs.json').write_text(json.dumps({'source':git('rev-parse','HEAD').decode().strip(),
  'public_guest_lock_sha256':pin['lock_sha256'],'all20_source_inputs_byte_exact':pin['old_inputs_sha256'],'additional_current_compiled_guest_data':{p:sha((ROOT/p).read_bytes()) for p in literal_inputs(git('ls-files','-z').decode().strip('\0').split('\0')) if p.startswith('plugins-examples/')},
  'old_successful_artifact':pin['artifact'],'old_source':pin['source'],'old_zip_sha256':pin['archive_sha256'],
  'scope':'reuse exact public guest lock only; six own components built by this job, no precompiled pack claim'},indent=2)+'\n')

def examples_check(out):
 pin=example_pin()
 if sha((ROOT/'plugins-examples/Cargo.lock').read_bytes())!=pin['lock_sha256']:raise ValueError('guest lock changed during build')
 components={}
 for name in EXAMPLE_NAMES:
  path=ROOT/'plugins-examples/target/wasm32-wasip2/release'/f'{name}.wasm'
  if path.is_symlink() or not path.is_file():raise ValueError('missing mandatory own example: '+name)
  with path.open('rb') as stream:header=stream.read(8)
  if header!=b'\0asm\x0d\0\x01\0':raise ValueError('own example is not a WASI component: '+name)
  destination=out/'own-example-components'/path.name;destination.parent.mkdir(exist_ok=True);shutil.copyfile(path,destination)
  digest=file_sha(path)
  if file_sha(destination)!=digest:raise ValueError('own component changed while retained')
  components[name]={'sha256':digest,'bytes':path.stat().st_size,'artifact_path':str(destination.relative_to(out))}
 (out/'own-examples-components.json').write_text(json.dumps({'source':git('rev-parse','HEAD').decode().strip(),
  'target':'wasm32-wasip2','guest_lock_sha256':pin['lock_sha256'],'mandatory_count':6,'components':components,
  'scope':'own example WASM files available before plugin-enabled tests; no client/game assets'},indent=2)+'\n')

def file_sha(path):
 digest=hashlib.sha256()
 with path.open('rb') as stream:
  for chunk in iter(lambda:stream.read(4*1024*1024),b''):digest.update(chunk)
 return digest.hexdigest()

def test_executable(message,manifests,sources):
 if not isinstance(message,dict) or message.get('reason')!='compiler-artifact':return None
 if not isinstance(message.get('profile'),dict):raise ValueError('malformed compiler profile')
 if message['profile'].get('test') is not True or not message.get('executable'):return None
 if not isinstance(message.get('target'),dict) or message['target'].get('kind') not in [['bin'],['lib'],['test']]:raise ValueError('unexpected own test target')
 if not isinstance(message.get('features'),list) or not all(isinstance(x,str) for x in message['features']):raise ValueError('malformed test features')
 manifest=Path(message['manifest_path']).resolve();executable=Path(message['executable'])
 source=Path(message['target']['src_path']).resolve()
 if source not in sources:raise ValueError('unreviewed own test source')
 if manifest not in manifests:raise ValueError('unreviewed test manifest')
 if executable.is_symlink():raise ValueError('test executable symlink')
 executable=executable.resolve()
 own_target=(ROOT/'target/release/deps').resolve()
 if not own_target.is_relative_to(ROOT.resolve()) or executable.parent!=own_target or not executable.is_file():raise ValueError('test executable outside own release/deps')
 with executable.open('rb') as stream:header=stream.read(20)
 if header[:6]!=b'\x7fELF\x02\x01' or len(header)!=20 or int.from_bytes(header[18:20],'little')!=62:raise ValueError('test executable is not own x64 ELF')
 return executable,manifest

def retain_tests(out):
 paths=selected();manifests={(ROOT/p).resolve() for p in paths if p.endswith('Cargo.toml')};sources={(ROOT/p).resolve() for p in paths if p.endswith('.rs')}
 records=[];seen=set();errors=[]
 for log in sorted(out.glob('*.log')):
  if log.name not in {'focused-protocol.log','focused-runtime.log','focused-stale.log','button-default.log','button-private.log','default-tests.log','no-default-tests.log','private-tests.log'}:continue
  for line in log.read_text(errors='replace').splitlines():
   if not line.startswith('{'):continue
   try:message=json.loads(line)
   except json.JSONDecodeError:continue
   try:result=test_executable(message,manifests,sources)
   except (ValueError,KeyError,OSError) as error:errors.append({'log':log.name,'error':str(error)});continue
   if result is None:continue
   try:
    executable,manifest=result;digest=file_sha(executable);key=(log.name,str(executable),digest)
    if key in seen:continue
    seen.add(key);target=out/'test-executables'/digest;target.parent.mkdir(exist_ok=True)
    if not target.exists():shutil.copyfile(executable,target);target.chmod(0o500)
    assert file_sha(target)==digest
    records.append({'log':log.name,'path':str(executable.relative_to(ROOT)),'manifest':str(manifest.relative_to(ROOT)),
     'target':message['target'],'features':message['features'],'cargo_profile':message['profile'],
     'sha256':digest,'bytes':executable.stat().st_size,'artifact_path':str(target.relative_to(out)),
     'source':git('rev-parse','HEAD').decode().strip(),'lock_sha256':file_sha(ROOT/'Cargo.lock')})
   except (ValueError,KeyError,OSError,AssertionError) as error:
    errors.append({'log':log.name,'error':str(error)})
 receipt={'status':'failed' if errors else 'retained' if records else 'none_compiled','executables':records,'errors':errors,
  'scope':'own test executables only; not standalone server runtime; no execution by collector'}
 (out/'retained-test-executables.json').write_text(json.dumps(receipt,indent=2)+'\n')
 return errors

def archive(out):
 out.mkdir(exist_ok=True)
 retention_errors=retain_tests(out)
 paths=selected()
 with tarfile.open(out/'source.tar.gz','w:gz') as tar:
  for path in paths:tar.add(ROOT/path,arcname=path)
 lock=ROOT/'Cargo.lock'
 if lock.is_file():shutil.copyfile(lock,out/'Cargo.lock')
 files={str(p.relative_to(out)):{'sha256':sha(p.read_bytes()),'bytes':p.stat().st_size} for p in sorted(out.rglob('*')) if p.is_file() and p.name!='receipt.json'}
 (out/'receipt.json').write_text(json.dumps({'source':git('rev-parse','HEAD').decode().strip(),'tree':git('rev-parse','HEAD^{tree}').decode().strip(),'run_id':os.environ.get('GITHUB_RUN_ID'),'attempt':os.environ.get('GITHUB_RUN_ATTEMPT'),'status':'failure' if retention_errors else os.environ.get('MCSA_JOB_STATUS'),'pre_archive_job_status':os.environ.get('MCSA_JOB_STATUS'),'sources':{p:sha((ROOT/p).read_bytes()) for p in paths},'files':files,'test_retention_errors':retention_errors,'scope':'private chunk producer plus schema-only ContainerButton source/unit checks; no native/client/server/release/admission/required38 activation'},indent=2)+'\n')
def main():
 parser=argparse.ArgumentParser();parser.add_argument('mode',choices=['prepare','examples-lock','examples-check','archive']);parser.add_argument('--output',type=Path,required=True);a=parser.parse_args()
 if os.environ.get('GITHUB_ACTIONS')!='true':raise SystemExit('GitHub-only evidence preparation; no local Cargo/server')
 if a.mode=='prepare':prepare(a.output)
 elif a.mode=='examples-lock':examples_lock(a.output)
 elif a.mode=='examples-check':examples_check(a.output)
 else:
  archive(a.output)
  retention=json.loads((a.output/'retained-test-executables.json').read_text())
  if retention['errors']:raise SystemExit('owned test ELF retention failed; evidence retained')
if __name__=='__main__':main()
