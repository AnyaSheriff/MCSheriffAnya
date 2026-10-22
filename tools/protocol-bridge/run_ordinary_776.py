#!/usr/bin/env python3
"""Последовательные обычные776-пробы. По умолчанию лишь план; --execute запускает.

Root выдаёт отдельное окно без Cargo/других MC. Три существующие пробы,
каждая со своим временным миром, последовательно на одном loopback26028.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time

from probe_776_payloads import authored_states, validate_slots


def stop_group(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--jar', type=Path, required=True)
    p.add_argument('--server-metadata', type=Path, required=True)
    p.add_argument('--registry-cache', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--stage', choices=['all','light','slots','entities'], default='all')
    p.add_argument('--execute', action='store_true')
    a = p.parse_args()
    root = a.root.resolve()
    here = Path(__file__).resolve().parent
    base = root / 'tools/protocol-bridge'
    def script(name):
        path = here / name
        return path if path.exists() else base / name
    a.output = a.output.resolve()
    a.output.mkdir(parents=True, exist_ok=False)  # не теряем старые отказы/записи
    section_commands, _ = authored_states(a.registry_cache)
    cases_path = script('testdata/ordinary-776-item-commands.json')
    cases = json.loads(cases_path.read_text())
    common = ['--version','776','--jar',str(a.jar.resolve()),'--server-metadata',str(a.server_metadata.resolve()),
              '--registry-cache',str(a.registry_cache.resolve()),'--port','26028']
    jobs = [
        ('light', [sys.executable,'-B',str(script('probe_light.py')),*common,'--authored-section','--output',str(a.output/'light')]),
        ('slots', [sys.executable,'-B',str(script('probe_item_slots.py')),*common,'--cases',str(cases_path),'--output',str(a.output/'slots')]),
        ('entities', [sys.executable,'-B',str(script('probe_entities.py')),*common,'--ordinary-only','--output',str(a.output/'entities')]),
    ]
    jobs = [(name, cmd) for name, cmd in jobs if a.stage in ('all', name)]
    plan = {'execute':a.execute,'port':26028,'jobs':[{'name':name,'argv':cmd} for name,cmd in jobs],
            'authored_section_commands':section_commands,'slot_commands':cases,
            'estimated_window':'2–4 minutes normally, at most6 minutes per probe before process-group cleanup',
            'scope':'No official registry/world dumps; only light and authored section,8 own slots,3 own mobs with motion. No40-attribute/Respawn rerun.'}
    (a.output/'plan.json').write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')
    if not a.execute:
        print(json.dumps({'planned':len(jobs),'launched':False,'plan':str(a.output/'plan.json')}))
        return
    cpus = sorted(os.sched_getaffinity(0))[:2]
    assert len(cpus) == 2, 'requires two available logical CPUs'
    os.sched_setaffinity(0, cpus)
    environment = os.environ.copy()
    environment['PYTHONPATH'] = os.pathsep.join([str(here),str(base)])
    environment['PYTHONDONTWRITEBYTECODE'] = '1'
    summary = {'cpus':cpus,'results':[],'complete':False}
    try:
        for name, argv in jobs:
            with socket.socket() as check:
                check.bind(('127.0.0.1',26028))
            start = time.monotonic()
            with (a.output/(name+'-runner.log')).open('w') as log:
                process = subprocess.Popen(argv,cwd=root,stdout=log,stderr=subprocess.STDOUT,
                                           env=environment,start_new_session=True)
                try:
                    code = process.wait(timeout=360)
                except BaseException:
                    stop_group(process)
                    raise
            result = {'name':name,'exit_code':code,'elapsed_seconds':round(time.monotonic()-start,3)}
            summary['results'].append(result)
            assert code == 0, (name,'probe failed; raw outputs retained')
            report_path = a.output/name/'result.json'
            report = json.loads(report_path.read_text())
            result['report_sha256'] = hashlib.sha256(report_path.read_bytes()).hexdigest()
            if name == 'slots':
                verified = validate_slots(report,cases,a.registry_cache)
                (a.output/'slots/verified.json').write_text(json.dumps(verified,indent=2)+'\n')
            elif name == 'light':
                assert [x['case'] for x in report['cases']] == ['placed','removed']
                assert [x['torch_cell_level'] for x in report['cases']] == [14,0]
                assert len(report['authored_sections']) == 2
                assert all(x['width'] == 15 and x['unique_states'] == 300 for x in report['authored_sections'])
            else:
                assert [x['type'] for x in report['entities']] == ['cow','slime','magma_cube']
                assert len(report['motions']) == 3 and not report['respawns']
            with socket.socket() as check:
                check.bind(('127.0.0.1',26028))
        summary['complete'] = True
    except BaseException as error:
        summary['error'] = repr(error)
        raise
    finally:
        (a.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary))


if __name__ == '__main__':
    main()
