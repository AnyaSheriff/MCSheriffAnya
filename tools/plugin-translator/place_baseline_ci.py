#!/usr/bin/env python3
"""Только собственный BlockPlace baseline: отдельный процесс на каждое действие."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys
ROOT=Path(__file__).resolve().parents[2]
TOOL=ROOT/'tools/plugin-translator'
OUT=ROOT/'place-evidence'
CAP=ROOT/'tmp/place-baseline'
BASE='815e26af014e42f9ca025ccc00b1969901203fad'
SDK_BASE='3c80f6c6ea1af60571291553aa109e50552a48f8'
TEST='network::play::place_plugin_baseline_tests::place_baseline_cohort'
SPEC=TOOL/'fixtures/place-priority-expected.json'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
def dump(p,v):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n')
def run(name,cmd,env=None,timeout=3600):
    OUT.mkdir(exist_ok=True)
    log=OUT/(name+'.log');status=OUT/(name+'.status.json')
    assert not log.exists() and not status.exists(),name
    with log.open('w') as f:
        try:r=subprocess.run(cmd,cwd=ROOT,stdout=f,stderr=subprocess.STDOUT,env=env,timeout=timeout);code=r.returncode
        except subprocess.TimeoutExpired:code=124
    dump(status,{'command':cmd,'exit_code':code,'timeout_seconds':timeout})
    print(name,code,flush=True)
    return code

def prepare():
    assert not CAP.exists()
    subprocess.run(['git','merge-base','--is-ancestor',BASE,'HEAD'],cwd=ROOT,check=True)
    freeze=json.loads((TOOL/'fixtures/place-frozen.json').read_text())
    for f,d in freeze['files'].items():assert sha(ROOT/f)==d,f
    pin=json.loads((TOOL/'fixtures/place-source-pin.json').read_text())
    assert pin['source']==BASE and len(pin['files'])==16
    for f,v in pin['files'].items():assert hashlib.sha256(git('show',BASE+':'+f)).hexdigest()==v['sha256'],f
    unchanged=['tools/plugin-translator/mcsa-bukkit','tools/plugin-translator/src','tools/plugin-translator/adapter.c','wit','src/plugins/host.rs']
    assert not git('diff',SDK_BASE,'HEAD','--',*unchanged)
    for f in ['src/network/play.rs','src/plugins/mod.rs']:
        original=git('show',BASE+':'+f);assert (ROOT/f).read_bytes().startswith(original),f
    changed=git('diff','--name-only',BASE,'HEAD').decode().splitlines()
    allowed={'src/network/play.rs','src/plugins/mod.rs','src/network/play/place_plugin_baseline_tests.rs','.github/workflows/bukkit-place-baseline-checks.yml','tools/plugin-translator/place_baseline_ci.py'}
    allowed.update(freeze['files'])
    allowed.add('tools/plugin-translator/fixtures/place-frozen.json')
    assert set(changed)<=allowed,sorted(set(changed)-allowed)
    workflow=(ROOT/'.github/workflows/bukkit-place-baseline-checks.yml').read_text()
    assert "branches: ['feature/bukkit-place-baseline-20261110']" in workflow
    assert 'pull_request:' not in workflow and '\n  place-baseline:\n' in workflow
    dump(OUT/'baseline-contract.json',{'base_candidate':BASE,'sdk_exact':SDK_BASE,'freeze':freeze,'expected_6_2_is_prediction_only':True,'hand_item_against_replaced':'UNSUPPORTED','canBuild_native_permission':False,'denial':'commit prevention; no client rollback claim'})
def fixtures():
    assert run('sdk-build',[sys.executable,'-B',str(TOOL/'translate.py'),'--build-sdk'])==0
    for letter in 'ab':
        name='place-priority-'+letter;folder=TOOL/'fixtures'/name;target=TOOL/'target/fixtures'/name;target.mkdir(parents=True)
        assert run(name+'-javac',['javac','-proc:none','--release','11','-encoding','UTF-8','-cp',str(TOOL/'target/mcsa-bukkit.jar'),'-d',str(target),*map(str,folder.glob('*.java'))])==0
        jar=TOOL/'target'/ (name+'.jar')
        assert run(name+'-jar',['jar','cf',str(jar),'-C',str(target),'.','-C',str(folder),'plugin.yml'])==0
        assert run(name+'-inspect',[sys.executable,'-B',str(TOOL/'translate.py'),str(jar),'--check-only','--report',str(OUT/(name+'-inspect.json'))])==0
        assert run(name+'-translate',[sys.executable,'-B',str(TOOL/'translate.py'),str(jar),'--output',str(CAP/'plugins'),'--report',str(OUT/(name+'-translation.json'))])==0

def cases():
    spec=json.loads(SPEC.read_text());host=[c['name'] for c in spec['cohorts']]
    native=host[:6]+['offhand-cancel','offhand-uncancel','creative-order','creative-cancel']
    return [('host',x) for x in host]+[('native',x) for x in native]
def tests():
    failures=[]
    for kind,name in cases():
        env=os.environ.copy();env['MCSA_PLACE_CASE']=kind+':'+name
        code=run(kind+'-'+name,['cargo','test','--release','--bin','mcsheriffanya',TEST,'--','--ignored','--exact','--nocapture','--test-threads=1'],env,300)
        log=(OUT/(kind+'-'+name+'.log')).read_text()
        if code or '\nrunning 1 test\n' not in log or '1 passed; 0 failed;' not in log:failures.append([kind,name,code])
    dump(OUT/'case-exitcodes.json',{'failures':failures,'expected_processes':22})
    assert not failures,failures

def text(raw):
    b=bytes.fromhex(raw);assert b[0]==8 and int.from_bytes(b[1:3],'big')==len(b)-3
    return b[3:].decode('utf-8')
def varint(b,i):
    n=0
    for k in range(5):
        x=b[i];i+=1;n|=(x&127)<<(k*7)
        if not x&128:return n,i
    raise ValueError('long varint')
def packets(raw):
    b=bytes.fromhex(raw);out=[];i=0
    while i<len(b):
        size,i=varint(b,i);end=i+size;assert 0<size<=len(b)-i;out.append(b[i:end]);i=end
    return out

def verify():
    spec={c['name']:c for c in json.loads(SPEC.read_text())['cohorts']};rows=[]
    for kind,name in cases():
        row=json.loads((CAP/'capture'/f'{kind}-{name}.json').read_text());assert row['id']==kind+':'+name
        assert row['actual_worker'] and row['ready_java_view'] and row['event_hand_item_against_replaced']=='UNSUPPORTED'
        trace=[]
        for raw in row['raw_message_nbt_hex']:
            fields=text(raw).split('|');assert len(fields)==9 and fields[0]=='PLACE' and fields[2:]==[row['uuid'],'world','3','64','3','STONE','same=1'],fields
            trace.append('trace '+fields[1])
        ref=spec[row['cohort']]
        row['trace']=trace
        row['public_match']=trace==ref['public_contract']['trace'] and row['verdict']==ref['public_contract']['verdict']
        row['legacy_prediction_match']=trace==ref['source_predicted_legacy']['trace'] and row['verdict']==ref['source_predicted_legacy']['verdict']
        if kind=='native':
            inp=bytes.fromhex(row['native_input_hex']);hand,i=varint(inp,0)
            assert hand==row['native_hand'] and inp[i:i+8]==(((3 & 0x3ffffff)<<38)|((3 & 0x3ffffff)<<12)|63).to_bytes(8,'big')
            i+=8;face,i=varint(inp,i);assert face==1
            import struct
            assert struct.unpack('>fff',inp[i:i+12])==(0.5,0.5,0.5)
            i+=12;assert inp[i]==0;i+=1;seq,i=varint(inp,i);assert seq==41 and i==len(inp)
            ps=packets(row['wire_hex']);acks=[];blocks=[]
            for p in ps:
                ident,i=varint(p,0)
                if ident==4:
                    seq,end=varint(p,i);assert end==len(p);acks.append(seq)
                if ident==8:
                    pos=int.from_bytes(p[i:i+8],'big');i+=8;state,end=varint(p,i);assert end==len(p)
                    blocks.append([pos,state])
            assert acks==[41],acks
            assert row['reaction']['acknowledge']==41 and row['reaction']['wire_protocol']==777
            assert row['native_hand']==int(name.startswith('offhand-'))
            before=row['before_stacks'];after=row['after_stacks']
            if row['verdict']=='deny':
                assert before==after and row['before_cells']==row['after_cells']
                for y,state in zip([63,64],row['after_cells']):
                    assert [((3 & 0x3ffffff)<<38)|((3 & 0x3ffffff)<<12)|y,state] in blocks
            else:
                slot=45 if row['native_hand'] else 36
                wanted=json.loads(json.dumps(before));wanted['slots'][slot]['count']=2 if name.startswith('creative-') else 1
                assert after==wanted
        else:assert row['before_stacks']==row['after_stacks'] and row['before_cells']==row['after_cells']
        rows.append(row)
    assert len({r['pid'] for r in rows})==22,'fresh process evidence required'
    hosts=[r for r in rows if r['kind']=='host']
    dump(OUT/'verification.json',{'capture_valid':True,'host_cases':12,'native_action_cases':10,'public_mismatches':[r['cohort'] for r in hosts if not r['public_match']],'public_verdict_mismatches':[r['cohort'] for r in hosts if r['verdict']!=spec[r['cohort']]['public_contract']['verdict']],'legacy_prediction_disagreements':[r['cohort'] for r in hosts if not r['legacy_prediction_match']],'prediction_6_2_not_asserted':True,'rows':rows})

def finish():
    OUT.mkdir(exist_ok=True)
    selected=git('ls-files','src','wit','Cargo.toml','Cargo.lock','tools/plugin-translator','.github/workflows/bukkit-place-baseline-checks.yml').decode().splitlines()
    if (ROOT/'Cargo.lock').is_file() and 'Cargo.lock' not in selected:selected.append('Cargo.lock')
    # Полные наши SDK/Java/C/WIT и ровно изменённые Rust файлы; остальные Rust SHA отдельно.
    source={}
    for f in selected:
        p=ROOT/f
        if not p.is_file():continue
        source[f]=sha(p)
        if not f.startswith('src/') or f in ['src/network/play.rs','src/network/play/place_plugin_baseline_tests.rs','src/plugins/mod.rs','src/plugins/host.rs']:
            dst=OUT/'source'/f;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,dst)
    if CAP.exists():shutil.copytree(CAP,OUT/'native',ignore=shutil.ignore_patterns('work'))
    for n in ['mcsa-bukkit.jar','place-priority-a.jar','place-priority-b.jar']:
        p=TOOL/'target'/n
        if p.exists():(OUT/'artifacts').mkdir(exist_ok=True);shutil.copyfile(p,OUT/'artifacts'/n)
    dump(OUT/'source-sha256.json',source)
    files={str(p.relative_to(OUT)):{'sha256':sha(p),'bytes':p.stat().st_size} for p in OUT.rglob('*') if p.is_file()}
    dump(OUT/'receipt.json',{'source':git('rev-parse','HEAD').decode().strip(),'base_candidate':BASE,'run':os.getenv('GITHUB_RUN_ID'),'job_status':os.getenv('MCSA_JOB_STATUS'),'files':files})
if __name__=='__main__':
    stage=sys.argv[1]
    if stage=='compile':assert run('rust-compile',['cargo','test','--release','--bin','mcsheriffanya','--no-run'])==0
    else:globals()[stage]()
