"""Own source-only GitHub checks: no native client/server execution or activation."""
import argparse,hashlib,json,os,shutil,subprocess,tarfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
MAIN='8b9a87182b7f540ff56d1afa45775078882739b4'
REVIEW='2fe9577a2b9abbcf5a8660282f32c4b5ad9a088e'
WORKFLOW='.github/workflows/private768-chunk-source-checks.yml'
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
def sha(data):return hashlib.sha256(data).hexdigest()
def selected():
 paths=git('ls-files','-z').decode().strip('\0').split('\0')
 return sorted(p for p in paths if p.endswith('.rs') or p.endswith('Cargo.toml') or p in [WORKFLOW,'tools/blackbox/ci/simulation-distance-two-runtime.lock','tools/protocol-bridge/private768_chunk_ci.py','tools/protocol-bridge/private768-acceptance.json','tools/protocol-bridge/client-chain-sources.json','tools/protocol-bridge/private768-extra-sources.json','tools/protocol-bridge/private768-chunk-review-pin.json'] or p.startswith('tools/protocol-bridge/testdata/private768-reference/'))
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
def archive(out):
 out.mkdir(exist_ok=True)
 paths=selected()
 with tarfile.open(out/'source.tar.gz','w:gz') as tar:
  for path in paths:tar.add(ROOT/path,arcname=path)
 lock=ROOT/'Cargo.lock'
 if lock.is_file():shutil.copyfile(lock,out/'Cargo.lock')
 files={str(p.relative_to(out)):{'sha256':sha(p.read_bytes()),'bytes':p.stat().st_size} for p in sorted(out.rglob('*')) if p.is_file() and p.name!='receipt.json'}
 (out/'receipt.json').write_text(json.dumps({'source':git('rev-parse','HEAD').decode().strip(),'tree':git('rev-parse','HEAD^{tree}').decode().strip(),'run_id':os.environ.get('GITHUB_RUN_ID'),'attempt':os.environ.get('GITHUB_RUN_ATTEMPT'),'status':os.environ.get('MCSA_JOB_STATUS'),'sources':{p:sha((ROOT/p).read_bytes()) for p in paths},'files':files,'scope':'private singleton chunk producer source/unit checks only; no native/client/server/release/admission/required38 activation'},indent=2)+'\n')
def main():
 parser=argparse.ArgumentParser();parser.add_argument('mode',choices=['prepare','archive']);parser.add_argument('--output',type=Path,required=True);a=parser.parse_args()
 if os.environ.get('GITHUB_ACTIONS')!='true':raise SystemExit('GitHub-only evidence preparation; no local Cargo/server')
 if a.mode=='prepare':prepare(a.output)
 else:archive(a.output)
if __name__=='__main__':main()
