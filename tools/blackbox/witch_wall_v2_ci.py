#!/usr/bin/env python3
"""Отдельный контроль стены: фиксируем оригинал до запуска нашего сервера."""
import hashlib, json, os, shutil, subprocess, sys, traceback
from pathlib import Path
import combat_ci
from witch_bounded_audit import inspect

ROOT=Path(__file__).resolve().parents[2]
NAME='witch-bounded-wall-v2'

def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def main():
    assert os.environ.get('GITHUB_ACTIONS')=='true'
    import argparse
    a=argparse.ArgumentParser();a.add_argument('--output',type=Path,required=True);a=a.parse_args()
    a.output.mkdir(parents=True,exist_ok=False)
    work=Path(os.environ['RUNNER_TEMP'])/'mcsa-witch-wall-v2';work.mkdir();data=work/'data';data.mkdir()
    pin=json.loads((ROOT/'tools/blackbox/ci/sources-777.json').read_text());jar=work/'server.jar';combat_ci.fetch(pin['official_jar'],jar)
    for e in pin['json_files']:
        if e['file'] in ('protocol.json','items.json','blocks.json','entities.json'):combat_ci.fetch(e,data/e['file'])
    binary=ROOT/'target/release/mcsheriffanya';head=binary.read_bytes()[:20]
    assert head[:6]==b'\x7fELF\x02\x01' and int.from_bytes(head[18:20],'little')==62
    source=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();assert source==os.environ['GITHUB_SHA']
    criteria=ROOT/'tools/blackbox/ci/witch-wall-v2.json';scenario=ROOT/'tools/blackbox/scenarios'/f'{NAME}.json'
    assert sha(criteria)=='c3589bd0a5c2570a3d84a6422e61493563ec4964e5b35d4c54c1fb0106a78473'
    assert sha(scenario)=='38560698914facfe994cb6d3f8bb804aa713c076f43f7a57b16c0be42bc6ddd5'
    manifest={'source':source,'runtime_sha256':sha(binary),'jar_sha256':sha(jar),'data':{p.name:sha(p) for p in data.iterdir()},'criteria_sha256':sha(criteria),'scenario_sha256':sha(scenario),'status':'running'}
    (a.output/'before.json').write_text(json.dumps(manifest,indent=2)+'\n')
    out=work/'recordings'
    cmd=[sys.executable,str(ROOT/'tools/blackbox/witch_record777.py'),'--data',str(data),'--output',str(out),'--binary',str(binary),'--jar',str(jar),'--java','java','--official-port','26190','--core-port','26192','--max-seconds','400','--isolate-vitals']
    protocol=json.loads((data/'protocol.json').read_text());items=json.loads((data/'items.json').read_text());entities={x['name']:x['id'] for x in json.loads((data/'entities.json').read_text())};entities['splash_item']=next(x['id'] for x in items if x['name']=='splash_potion')
    def capture(label,args):
        with (a.output/(label+'-recorder.log')).open('w') as log:subprocess.run(cmd+args+[NAME],stdout=log,stderr=subprocess.STDOUT,check=True,timeout=450)
        life=json.loads((out/'lifecycle.json').read_text());assert not life['cleanup_errors'] and len(life['cleanup'])==1 and all(x['exit_code']==0 and x['group_gone'] for x in life['cleanup']) and len(life['ports_free'])==4 and all(life['ports_free'].values())
        shutil.copy2(out/'lifecycle.json',a.output/(label+'-lifecycle.json'))
        raw=out/f'{NAME}-{label}.json';row=inspect(json.loads(raw.read_text()),protocol,entities,NAME)
        return raw,row
    try:
        official,row=capture('official',['--official-only']);manifest['official']=row
        frozen=sha(official);manifest['official_frozen_sha256']=frozen
        (a.output/'official-freeze.json').write_text(json.dumps(manifest,indent=2)+'\n')
        candidate,crow=capture('ours',['--resume','--core-only']);manifest['ours']=crow
        assert sha(official)==frozen,'original changed after freeze'
        manifest['candidate_sha256']=sha(candidate);manifest['status']='wall-v2-pass'
    except Exception as error:
        manifest['status']='inconclusive-or-fail';manifest['error']=repr(error);(a.output/'error.txt').write_text(traceback.format_exc())
    finally:
        if out.exists():
            for p in out.glob('*.json'):shutil.copy2(p,a.output/p.name)
            for side in ('official','ours'):
                p=out/side/'server.log'
                if p.is_file():shutil.copy2(p,a.output/(side+'-server.log'))
        manifest['evidence_sha256']={p.name:sha(p) for p in a.output.iterdir() if p.is_file() and p.name!='after.json'}
        (a.output/'after.json').write_text(json.dumps(manifest,indent=2)+'\n')
    if manifest['status']!='wall-v2-pass':raise SystemExit(manifest['error'])
    print(manifest['status'])

if __name__=='__main__':main()
