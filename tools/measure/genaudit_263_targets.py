"""Run genaudit_263.py separately for every located biome/structure window.

Keeps targeted samples out of unbiased spawn-area distributions. Reports are
resumable only when source hashes, region manifests, seed and bounds match.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import threading

from genaudit_263_io import manifest


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('ours');p.add_argument('vanilla');p.add_argument('--plan',required=True)
    p.add_argument('--output',required=True);p.add_argument('--cache')
    p.add_argument('--starts-executable')
    p.add_argument('--jobs',type=int,choices=range(1,5),default=2)
    args=p.parse_args()
    plan=json.loads(Path(args.plan).read_text())
    folder=Path(args.output);folder.mkdir(parents=True,exist_ok=True)
    script=Path(__file__).with_name('genaudit_263.py')
    source_hash=hashlib.sha256(script.read_bytes()+script.with_name('genaudit_263_io.py').read_bytes()).hexdigest()
    fingerprints={'ours':manifest(args.ours),'vanilla':manifest(args.vanilla)}
    starts=None
    if args.starts_executable:
        starts=folder/'eligible-starts.json'
        stamp=folder/'eligible-starts.provenance.json'
        inputs={'executable_sha256':hashlib.sha256(Path(args.starts_executable).read_bytes()).hexdigest(),
                'plan_sha256':hashlib.sha256(Path(args.plan).read_bytes()).hexdigest()}
        valid=False
        if starts.exists() and stamp.exists():
            meta=json.loads(stamp.read_text())
            valid=all(meta.get(k)==v for k,v in inputs.items()) and meta.get('data_sha256')==hashlib.sha256(starts.read_bytes()).hexdigest()
        if not valid:
            starts.write_text(subprocess.check_output([args.starts_executable,args.plan],text=True))
            stamp.write_text(json.dumps(dict(inputs,data_sha256=hashlib.sha256(starts.read_bytes()).hexdigest()),indent=2)+'\n')
    index=[]
    targets=[('biome',t) for t in plan.get('biome_targets',[])]+[('structure',t) for t in plan.get('structure_targets',[])]
    # Different target names can refer to the very same window/cache token.
    locks={(t['dimension'],tuple(t['bounds'] or [])):threading.Lock() for _,t in targets}
    def run_target(item):
        category,t=item
        name=t[category]
        row={'category':category,'name':name,'dimension':t['dimension'],'bounds':t['bounds']}
        if not t['bounds']:
            row['status']='not located by official server';return row
        prefix=folder/(category+'-'+name)
        report=Path(str(prefix)+'.json')
        cached=None
        if report.exists():
            try:old=json.loads(report.read_text())
            except json.JSONDecodeError:old={}
            if old.get('analyzer_source_sha256')==source_hash and old['ours_level']['seed']==plan['seed']:
                if all(old['statistics'][side]['manifest']==fp for side,fp in fingerprints.items()):
                    dim=old['statistics']['ours']['dimensions'][t['dimension']]
                    if dim['coverage']['bounds_chunks']==t['bounds']:cached=old
        if cached is None:
            command=[sys.executable,str(script),args.ours,args.vanilla,'--plan',args.plan,
                     '--target-name',('structure:' if category=='structure' else '')+name,'--output',str(prefix)]
            if args.cache:command+=['--cache',args.cache]
            if starts:command+=['--starts-json',str(starts)]
            with Path(str(prefix)+'.log').open('w') as log:
                done=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
            if done.returncode:
                row.update({'status':'analysis failed','exit_code':done.returncode});return row
            cached=json.loads(report.read_text())
        row.update({'status':'measured','report':str(report.resolve()),
                    'outside_tolerance':sum(r['verdict']=='outside' for r in cached['rows'])})
        return row
    def locked_target(item):
        _,target=item
        with locks[target['dimension'],tuple(target['bounds'] or [])]:return run_target(item)
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        pending={pool.submit(locked_target,item):i for i,item in enumerate(targets)}
        completed={}
        for future in as_completed(pending):
            row=future.result();completed[pending[future]]=row
            index=[completed[i] for i in sorted(completed)]
            (folder/'index.json').write_text(json.dumps(index,indent=2)+'\n')
            print(row['category'],row['name'],row['status'],row.get('outside_tolerance',''),flush=True)
    (folder/'index.json').write_text(json.dumps(index,indent=2)+'\n')
    if any(r['status']=='analysis failed' for r in index):
        raise SystemExit(1)


if __name__=='__main__':main()
