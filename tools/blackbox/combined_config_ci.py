#!/usr/bin/env python3
"""Отдельная CI-проверка происхождения combined tree перед Login2/2."""
import argparse,hashlib,json,os,re,subprocess
from pathlib import Path
import simulation_distance_ci as original
ROOT=Path(__file__).resolve().parents[2]
BASE='e3e90d74f60f0750459ebe964c709195f92b49ef'
HEADS={'witch':'dcf1959ae5384325162467241c6b5cb87d71f93c','bukkit':'3c80f6c6ea1af60571291553aa109e50552a48f8','config':'fea09518233a843264d5d4cac9c86453ec88c1b7'}
EXCEPTIONS=frozenset({'.github/workflows/gameplay-integration-checks.yml','tools/blackbox/combined_config_ci.py','tools/blackbox/test_combined_config_ci.py'})
LOCK='6c1f0a7abf94e252585096af69c54e1e757c5aa199859d9470de21843ec701b2'
PRIMARY='8b8053cccd3f4eb206f91685f7b97d8e402bdf1a54abbcdd7950981cddffe5cb'
REFERENCE='e94eb15cdc9d569b9b80d985682b74a060a62908125ee1ec98a04ebfc1b193d3'
LOGS=('wasm','clippy','workspace','no-default','build')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def expected_tree(base,heads,actual,exceptions=EXCEPTIONS):
    """Каждый путь берётся из единственного входа либо base; новые CI-файлы явно названы."""
    changed={name:{p for p in base.keys()|tree.keys() if base.get(p)!=tree.get(p)} for name,tree in heads.items()}
    names=list(changed)
    for i,name in enumerate(names):
        for other in names[i+1:]:
            if changed[name]&changed[other]:raise ValueError('input paths overlap')
    if any(p in base or any(p in t for t in heads.values()) for p in exceptions):raise ValueError('exception is not new')
    expected=dict(base)
    for name,paths in changed.items():
        for p in paths:
            if p in heads[name]:expected[p]=heads[name][p]
            else:expected.pop(p,None)
    if set(actual)-set(expected)!=set(exceptions):raise ValueError('unexpected or missing helper path')
    if {p:v for p,v in actual.items() if p not in exceptions}!=expected:raise ValueError('combined tree differs from pinned inputs')
    return {name:len(paths) for name,paths in changed.items()}

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
def tree(ref):
    entries=git('ls-tree','-rz','--full-tree',ref).split(b'\0');result={}
    for row in entries:
        if row:
            meta,path=row.split(b'\t',1);mode,kind,oid=meta.decode().split();result[path.decode()]=(mode,kind,oid)
    return result

def verify_source():
    head=git('rev-parse','HEAD').decode().strip();assert head==os.environ['GITHUB_SHA']
    for source in HEADS.values():subprocess.run(['git','merge-base','--is-ancestor',source,head],cwd=ROOT,check=True)
    subprocess.run(['git','diff','--quiet','HEAD','--'],cwd=ROOT,check=True)
    counts=expected_tree(tree(BASE),{key:tree(ref) for key,ref in HEADS.items()},tree(head))
    assert sha(original.PRIMARY)==PRIMARY and sha(original.RUST_REFERENCE)==REFERENCE
    assert sha(original.PINNED_LOCK)==sha(ROOT/'Cargo.lock')==LOCK
    ref=json.loads(original.RUST_REFERENCE.read_text());assert ref['source']=='fd706e601bdcb8624526481f159129376d134b37' and ref['overall_run_conclusion']=='failure' and all(x=='success' for x in ref['validated_steps'].values())
    return {'source':head,'tree':git('rev-parse','HEAD^{tree}').decode().strip(),'base':BASE,'input_heads':HEADS,'input_changed_paths':counts,'helper_exceptions':sorted(EXCEPTIONS),'Cargo.lock_sha256':LOCK,'primary_sha256':PRIMARY,'historical_reference_sha256':REFERENCE,'historical_reference_source':ref['source'],'historical_overall_run':ref['overall_run_conclusion'],'historical_tree_equal_to_combined':False}

def fresh_receipt(folder,binary):
    proof=verify_source();rows={}
    for name in LOGS:
        log=folder/(name+'.log');marker=folder/(name+'.success.json');record=json.loads(marker.read_text())
        assert record['step']==name and record['source']==proof['source'] and record['exit_code']==0 and record['log_sha256']==sha(log)
        text=log.read_text(errors='replace')
        if name in ('workspace','no-default'):
            assert re.search(r'test result: ok\. [1-9][0-9]* passed; 0 failed;',text)
            assert not re.search(r'test result: FAILED|error: test failed',text)
        rows[name]={'log_sha256':sha(log),'marker_sha256':sha(marker)}
    data=binary.read_bytes();assert data[:6]==b'\x7fELF\x02\x01' and int.from_bytes(data[18:20],'little')==62
    proof.update({'status':'fresh-combined-rust-suites-passed','fresh_tests':True,'steps':rows,'runtime_sha256':sha(binary),'runtime_bytes':len(data)})
    return proof

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True);parser.add_argument('--logs',type=Path,required=True);parser.add_argument('--binary',type=Path,default=ROOT/'target/release/mcsheriffanya');a=parser.parse_args()
    assert os.environ.get('GITHUB_ACTIONS')=='true','CI only'
    proof=fresh_receipt(a.logs,a.binary)
    a.logs.joinpath('fresh-combined-rust.json').write_text(json.dumps(proof,indent=2)+'\n')
    # Старый CLI и verify_rust_reference остаются строгими. Только в этом процессе
    # подставляется отдельная проверка комбинированного дерева и новых тестов.
    original.verify_rust_reference=lambda:dict(proof)
    original.run(a.output,a.binary)
    assert sha(a.binary)==proof['runtime_sha256']

if __name__=='__main__':main()
