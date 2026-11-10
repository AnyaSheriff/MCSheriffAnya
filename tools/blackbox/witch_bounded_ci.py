#!/usr/bin/env python3
"""Свежее 26.3 сравнение ведьмы и один набор прежних боевых контролей."""
import hashlib,json,os,shutil,subprocess,sys,traceback
from pathlib import Path
from witch_bounded_audit import NAMES,audit_pair
import combat_ci
ROOT=Path(__file__).resolve().parents[2]
OLD=['pvp-fire-protection-control-777','pvp-fire-protection-iv-777','pvp-thorns-control-777','pvp-thorns-break-777']

def sha(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def lifecycle(folder):
    value=json.loads((folder/'lifecycle.json').read_text())
    assert not value['cleanup_errors'] and len(value['cleanup'])==2 and len(value['ports_free'])==4
    assert all(item['exit_code']==0 and item['group_gone'] for item in value['cleanup'])
    assert all(value['ports_free'].values())
    return value

def preserve(work,dest):
    dest.mkdir(parents=True,exist_ok=True)
    if not work.exists():return
    for path in work.glob('*.json'):shutil.copy2(path,dest/path.name)
    for side in ('official','ours'):
        path=work/side/'server.log'
        if path.is_file():shutil.copy2(path,dest/(side+'-server.log'))

def run_record(data,jar,binary,work,log,names,witch):
    script='witch_record777.py' if witch else 'record777.py'
    cmd=[sys.executable,str(ROOT/'tools/blackbox'/script),'--data',str(data),'--output',str(work),
         '--binary',str(binary),'--jar',str(jar),'--java','java','--official-port','26190','--core-port','26192',
         '--parallel','--max-seconds', '1200' if witch else '400','--isolate-vitals',*names]
    with log.open('w') as out:subprocess.run(cmd,stdout=out,stderr=subprocess.STDOUT,check=True,timeout=1260 if witch else 460)
    lifecycle(work)

def main():
    assert os.environ.get('GITHUB_ACTIONS')=='true','CI-only; local blackbox requires separate authorization'
    import argparse
    p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    work=Path(os.environ['RUNNER_TEMP'])/'mcsa-witch-paired';work.mkdir(exist_ok=False);data=work/'data';data.mkdir()
    pin=json.loads((ROOT/'tools/blackbox/ci/sources-777.json').read_text());jar=work/'server.jar';combat_ci.fetch(pin['official_jar'],jar)
    for e in pin['json_files']:
        if e['file'] in ('protocol.json','items.json','blocks.json','entities.json'):combat_ci.fetch(e,data/e['file'])
    binary=ROOT/'target/release/mcsheriffanya';assert binary.is_file()
    with binary.open('rb') as f: header=f.read(20)
    assert header[:6]==b'\x7fELF\x02\x01' and int.from_bytes(header[18:20],'little')==62, 'expected Linux x86_64 ELF'
    source=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();assert source==os.environ['GITHUB_SHA'], 'checkout SHA differs from triggered commit'
    manifest={'source':source,'runtime_sha256':sha(binary),'official_jar_sha256':sha(jar),'data_sha256':{f.name:sha(f) for f in data.iterdir()},'criteria_sha256':sha(ROOT/'tools/blackbox/ci/witch-throw-777.json'),'scenario_sha256':{n:sha(ROOT/'tools/blackbox/scenarios'/f'{n}.json') for n in NAMES},'harness_sha256':{n:sha(ROOT/'tools/blackbox'/n) for n in ('witch_record777.py','record777.py','witch_bounded_audit.py','witch_bounded_ci.py')},'status':'running','witch':'pending','combat':'pending'}
    (a.output/'manifest-before.json').write_text(json.dumps(manifest,indent=2)+'\n')
    outcomes=[]
    for label,names,witch in [('witch',NAMES,True),('combat',OLD,False)]:
        temp=work/(label+'-recordings');target=a.output/label;target.mkdir()
        try:
            run_record(data,jar,binary,temp,target/'recorder.log',names,witch)
            if witch:
                result=audit_pair(temp,json.loads((data/'protocol.json').read_text()),json.loads((data/'entities.json').read_text()),json.loads((data/'items.json').read_text()))
                (target/'analysis.json').write_text(json.dumps(result,indent=2)+'\n');manifest[label]=result['status']
                if result['status']!='strict-bounded-pass':outcomes.append(label+': '+result['status'])
            else:
                from protection_edges_ci import semantics
                result=semantics(temp,data);(target/'semantics.json').write_text(json.dumps(result,indent=2)+'\n');manifest[label]=result['status']
                if result['status']!='verified-semantic-controls':outcomes.append(label+': '+result['status'])
        except Exception as error:
            manifest[label]='error';outcomes.append(label+': '+repr(error));(target/'error.txt').write_text(traceback.format_exc())
        finally:
            preserve(temp,target)
        if (temp/'lifecycle.json').exists():
            try:lifecycle(temp)
            except Exception as error:outcomes.append(label+': unsafe cleanup '+repr(error));break
        else:
            outcomes.append(label+': missing lifecycle');break
    manifest['status']='fail' if outcomes else 'strict-bounded-pass';manifest['errors']=outcomes
    manifest['evidence_sha256']={str(f.relative_to(a.output)):sha(f) for f in a.output.rglob('*') if f.is_file() and f.name not in ('manifest-after.json',)}
    (a.output/'manifest-after.json').write_text(json.dumps(manifest,indent=2)+'\n')
    if outcomes:raise SystemExit('; '.join(outcomes))
    print(json.dumps({'status':manifest['status'],'source':source,'runtime_sha256':manifest['runtime_sha256']}))

if __name__=='__main__':main()
