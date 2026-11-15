#!/usr/bin/env python3
"""Экспорт описаний и последовательная запись двух сторон; запуск только в CI."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from catalog import catalog, coverage_matrix
from common_cases import common_cases
from special_cases import special_cases

BASE = '0d3db6fe24bbde186af9d2d4cb62f64d12ae7285'
ELF_SHA = '4a8d148c670b3c2813778a50aa8925ff339d3e36ef1fe5828382f7b6588fbfe4'
PREFLIGHT = ('cow.controlled_spawn','cow.turn','cow.teleport','cow.tracking_leave_return',
             'cow.damage','cow.death','salmon.controlled_spawn','salmon.damage',
             'special-pose-negative','special-water-salmon-ai',
             'special-flight-happy_ghast-noai','special-flight-happy_ghast-ai')
HERE = Path(__file__).resolve().parent

def dump(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n')

def export(output, mode, shard):
    cases = common_cases()+special_cases()
    if len({c['id'] for c in cases}) != len(cases): raise ValueError('duplicate catalog IDs')
    selected = [next(c for c in cases if c['id']==i) for i in PREFLIGHT] if mode=='preflight' else cases[shard::8]
    output.mkdir(parents=True, exist_ok=True)
    dump(output/'cases.json',selected)
    dump(output/'catalog.json',dict(mobs=catalog(),coverage=coverage_matrix(cases),
         total_descriptions=len(cases),selected_descriptions=len(selected),runtime_passes=None))
    return selected

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('command',choices=['export','run'])
    p.add_argument('--mode',choices=['preflight','suite'],default='preflight')
    p.add_argument('--shard',type=int,choices=range(8),default=0)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--inputs',type=Path)
    a=p.parse_args(); selected=export(a.output,a.mode,a.shard)
    if a.command=='export': return
    if os.environ.get('GITHUB_ACTIONS')!='true' or not a.inputs: p.error('run requires GitHub and --inputs')
    source=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    statuses={}
    for side,port in [('reference',26400+4*a.shard),('candidate',26401+4*a.shard)]:
        cmd=[sys.executable,'-B',str(HERE/'run_side.py'),'--side',side,'--output',str(a.output/side),
             '--cases-file',str(a.output/'cases.json'),'--data',str(a.inputs),'--source',source,'--port',str(port)]
        if side=='reference':
            pins=json.loads((HERE.parent/'ci/sources-777.json').read_text())
            cmd+=['--jar',str(a.inputs/'server.jar'),'--jar-sha256',pins['official_jar']['sha256']]
        else: cmd+=['--binary',str(a.inputs/'runtime-elf'),'--binary-sha256',ELF_SHA]
        statuses[side]=subprocess.run(cmd).returncode
    dump(a.output/'recording-status.json',dict(returncodes=statuses,selected=len(selected),source=source,baseline=BASE))
    paths=[a.output/s/'side.json' for s in ('reference','candidate')]
    parity=2
    if all(x.exists() for x in paths):
        parity=subprocess.run([sys.executable,'-B',str(HERE/'report.py'),'--reference',str(paths[0]),
             '--candidate',str(paths[1]),'--cases-file',str(a.output/'cases.json'),'--output',str(a.output/'comparison'),'--gate']).returncode
    summary=dict(recording_complete=all(v==0 for v in statuses.values()),parity_gate_exit=parity)
    comparison=a.output/'comparison/report.json'
    if comparison.exists(): summary['comparison_counts']=json.loads(comparison.read_text())['counts']
    for side,path in zip(('reference','candidate'),paths):
        if path.exists():
            side_report=json.loads(path.read_text())
            summary[side]=dict(error=side_report.get('error'),cleanup=side_report.get('cleanup'),
                recorded=sum(c['status']=='RECORDED' for c in side_report['cases']),
                failed=[dict(id=c['id'],error=c.get('error')) for c in side_report['cases'] if c['status']!='RECORDED'])
    dump(a.output/'status.json',summary)
    print(json.dumps(summary,ensure_ascii=False,indent=2))
    if any(statuses.values()) or parity: raise SystemExit(1)

if __name__=='__main__': main()
