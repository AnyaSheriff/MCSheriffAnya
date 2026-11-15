#!/usr/bin/env python3
"""Только GitHub: записи пакетов мобов в отдельном временном мире 777."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time
import traceback

from wire import MobObserver, connect

ROOT = Path(__file__).resolve().parents[3]
ERRORS = re.compile(r'Unknown or incomplete command|Unknown command|Incorrect argument|Invalid.*(?:argument|entity|resource)|No entity was found|No entities found|Expected .+ at position|Unknown entity|Summon failed|Unable to summon|Неверн|Неизвестн|Сущность не найдена', re.I)


def sha(path):
    with path.open('rb') as f: return hashlib.file_digest(f, 'sha256').hexdigest()


def port_free(port):
    with socket.socket() as s:
        s.settimeout(.3)
        return s.connect_ex(('127.0.0.1', port)) != 0


def parse_args():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--side', choices=['reference','candidate'], required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--cases-file', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--jar', type=Path); p.add_argument('--binary', type=Path)
    p.add_argument('--jar-sha256'); p.add_argument('--binary-sha256')
    p.add_argument('--source', required=True)
    p.add_argument('--java', default='java')
    p.add_argument('--port', type=int, required=True)
    a = p.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true': p.error('GitHub Actions only; local runtime forbidden')
    if not 26400 <= a.port <= 65000: p.error('isolated port >=26400 required')
    if a.side == 'reference' and (not a.jar or not a.jar_sha256 or a.binary): p.error('reference requires --jar and --jar-sha256')
    if a.side == 'candidate' and (not a.binary or not a.binary_sha256 or a.jar): p.error('candidate requires --binary and --binary-sha256')
    return a


def run(a):
    cases = json.loads(a.cases_file.read_text())
    if not isinstance(cases,list) or not 1 <= len(cases) <= 90: raise ValueError('shard requires1..90 cases')
    if len({c['id'] for c in cases}) != len(cases): raise ValueError('duplicate case IDs')
    pins = json.loads((ROOT/'tools/blackbox/ci/sources-777.json').read_text())
    identities = {}
    for name in ('protocol.json','entities.json'):
        pin = next(p for p in pins['json_files'] if p['file'] == name)
        path = a.data/name
        if sha(path) != pin['sha256'] or path.stat().st_size != pin['bytes']: raise ValueError('pinned JSON mismatch: '+name)
        identities[name] = pin['sha256']
    protocol = json.loads((a.data/'protocol.json').read_text())
    entities = json.loads((a.data/'entities.json').read_text())
    types = {e['name']:e['id'] for e in entities}
    executable = (a.jar if a.side == 'reference' else a.binary).resolve()
    executable_sha = sha(executable)
    expected = a.jar_sha256 if a.side == 'reference' else a.binary_sha256
    if executable_sha != expected: raise ValueError('executable SHA mismatch')
    if a.side == 'reference' and executable_sha != pins['official_jar']['sha256']: raise ValueError('official opaque jar pin mismatch')
    identities['jar_sha256' if a.side == 'reference' else 'binary_sha256'] = executable_sha
    identities['cases_sha256'] = sha(a.cases_file)
    a.output.mkdir(parents=True,exist_ok=False)
    folder = a.output/'world'; folder.mkdir()
    config = folder if a.side == 'reference' else folder/'config'; config.mkdir(exist_ok=True)
    properties = (f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\n'
                  'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n'
                  'level-seed=1\nview-distance=4\nsimulation-distance=4\nspawn-protection=0\ngamemode=creative\n'
                  'difficulty=normal\nmax-players=3\nmax-tick-time=0\n')
    (config/'server.properties').write_text(properties)
    (folder/'eula.txt').write_text('eula=true\n')
    if a.side == 'candidate':
        (config/'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    report = dict(source=a.source,side=a.side,identities=identities,cases=[],commands=[],cleanup={})
    process = client = None
    logpath = a.output/'server.log'; commandpath = a.output/'commands.jsonl'
    barrier_index = 0
    def save(): (a.output/'side.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    def command(text):
        row = dict(phase=client.phase if client else 'startup',text=text,time=time.monotonic())
        report['commands'].append(row)
        with commandpath.open('a') as f: f.write(json.dumps(row)+'\n')
        if process.poll() is not None: raise RuntimeError('server exited during command')
        process.stdin.write(text+'\n'); process.stdin.flush()
    def pump(seconds, deadline=None):
        if deadline and time.monotonic()+seconds > deadline: raise TimeoutError('case deadline30s')
        client.pump(seconds)
    def barrier(deadline=None):
        nonlocal barrier_index
        barrier_index += 1; marker = f'MCSA_MOB_BARRIER_{barrier_index:06d}'
        command('say '+marker)
        end = min(time.monotonic()+3, deadline or float('inf'))
        while marker not in logpath.read_text(errors='replace'):
            if time.monotonic() >= end: raise TimeoutError('console barrier not observed')
            pump(.05,deadline)
        pump(.1,deadline)
    try:
        if not port_free(a.port): raise RuntimeError('port already in use')
        launch = [a.java,'-Xmx1G','-XX:ActiveProcessorCount=2','-jar',str(executable),'nogui'] if a.side == 'reference' else [str(executable)]
        with logpath.open('w') as log:
            process = subprocess.Popen(launch,cwd=folder,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True)
        until = time.monotonic()+120
        while port_free(a.port):
            if process.poll() is not None or time.monotonic() > until: raise RuntimeError('startup failed or timed out')
            time.sleep(.1)
        client = connect(a.port,protocol,MobObserver,version=777,username='MobProbe',view_distance=4)
        client.set_entity_types(entities);client.pump(5,lambda r:client.position is not None)
        for text in ('tick freeze','gamerule minecraft:spawn_mobs false','time set 18000','weather clear','tp MobProbe 0.5 101 0.5 0 0',
                     'forceload add 128 0','tick step 20'):
            command(text)
        pump(1.2);barrier()
        if not client.loaded or client.position is None:
            raise RuntimeError('observer login/position not confirmed')
        startup_log = logpath.read_text(errors='replace')
        if ERRORS.search(startup_log): raise RuntimeError('setup command rejected; see server.log')
        environment = None
        for index, case in enumerate(cases):
            row = dict(id=case['id'],mob=case['mob'],status='ERROR',entity=None,initial=None,phases=[],events=[])
            report['cases'].append(row)
            deadline = time.monotonic()+30
            client.phase = case['id']+':cleanup'
            try:
                # Убираем прежних испытуемых из области слежения до смерти.
                command('tp @e[tag=mcsa_mob_subject] 128.5 101 0.5')
                command('kill @e[tag=mcsa_mob_subject]')
                command('tick step 2');pump(.25,deadline);barrier(deadline)
                client.events.clear();client.records.clear();client.entities.clear();client.max_events=4000
                client.phase = case['id']+':setup'
                env = case['environment']
                if env not in ('dry','air','water'): raise ValueError('unsupported environment '+str(env))
                if env != environment:
                    command('fill -8 100 -8 12 100 8 stone')
                    command('fill 2 101 -3 9 110 4 air')
                    if env == 'water': command('fill 2 101 -3 9 108 4 water')
                    command('tp MobProbe 0.5 101 0.5 0 0')
                    barrier(deadline);environment = env
                y = {'dry':101,'air':108,'water':103}[env]
                point = [4.5,y,.5];tag = f'mcsa_mob_{index:04d}'
                target = f'@e[tag={tag},limit=1]'
                start_log = len(logpath.read_text(errors='replace'))
                nbt = case.get('nbt','').strip().strip('{}')
                nbt += (',' if nbt else '')+f'Tags:["mcsa_mob_subject","{tag}"]'
                command(f'summon minecraft:{case["mob"]} 4.5 {y} 0.5 {{{nbt}}}')
                # Консольный say не означает обновления сетевого слежения.
                # Обе стороны получают одинаковые два установочных тика.
                command('tick step 2');pump(.25,deadline);barrier(deadline)
                def locate():
                    return [e for e,s in client.entities.items() if s.get('entity_type') == types.get(case['mob']) and 'position' in s and sum((a-b)**2 for a,b in zip(s['position'],point)) < 16 and not s.get('removed')]
                found = locate()
                if len(found) != 1:
                    row['observer_diagnostic']=dict(position=client.position,loaded=client.loaded,
                        packet_counts=dict(__import__('collections').Counter(r['name'] for r in client.records)),
                        entity_snapshots=list(client.entities.values()))
                    raise RuntimeError('missing or ambiguous subject spawn: '+str(found))
                entity = found[0];uid = client.entities[entity]['uuid'];row['entity']=entity;row['initial']=client.snapshot(entity)
                for phase in case['phases']:
                    if phase.get('restart'): raise ValueError('restart phase unsupported; no persistence claim')
                    client.phase = case['id']+':'+phase['id'];begin = len(client.events)
                    for text in phase.get('commands',[]): command(text.format(target=target,x=4.5,y=y,z=.5))
                    ticks = phase.get('ticks',0)
                    if not isinstance(ticks,int) or not 0 <= ticks <= 400: raise ValueError('tick step bound')
                    if ticks: command(f'tick step {ticks}')
                    pump(ticks/20+.1,deadline);barrier(deadline)
                    matches = [e for e,s in client.entities.items() if s.get('uuid') == uid and not s.get('removed')]
                    if len(matches) == 1: entity = matches[0]
                    row['phases'].append(dict(id=phase['id'],snapshot=client.snapshot(entity),begin_index=begin,end_index=len(client.events),events=client.marked_events(begin)))
                errors = ERRORS.findall(logpath.read_text(errors='replace')[start_log:])
                if errors: raise RuntimeError('case command rejected: '+repr(errors))
                subject_ids = {e for e,s in client.entities.items() if s.get('uuid') == uid}
                failures = [e for e in client.events if e.get('entity') in subject_ids and e.get('status') in ('decode_error','unparsed_relative_teleport')]
                if failures: raise RuntimeError('subject decoding incomplete: '+repr([(e['name'],e['status']) for e in failures[:5]]))
                row['status']='RECORDED'
            except Exception as error:
                row['error']=repr(error);row['traceback']=traceback.format_exc()
            finally:
                row['events']=client.marked_events(0);save()
    except Exception as error:
        report['error']=repr(error);report['traceback']=traceback.format_exc()
    finally:
        if client:
            client.sock.close()
        cleanup = report['cleanup'];cleanup['forced']=False
        if process:
            try:
                if process.poll() is None:
                    process.stdin.write('stop\n');process.stdin.flush();process.wait(timeout=30)
            except Exception as error:
                cleanup['stop_error']=repr(error);cleanup['forced']=True
                try: os.killpg(process.pid,signal.SIGTERM);process.wait(timeout=5)
                except subprocess.TimeoutExpired: os.killpg(process.pid,signal.SIGKILL);process.wait(timeout=5)
            cleanup['server_exit']=process.returncode
            try: os.killpg(process.pid,0);cleanup['group_gone']=False
            except ProcessLookupError: cleanup['group_gone']=True
        else: cleanup.update(server_exit=None,group_gone=True)
        cleanup['port_free']=port_free(a.port);save()
    return report


if __name__ == '__main__':
    args = parse_args(); result = run(args)
    if result.get('error') or any(c['status'] != 'RECORDED' for c in result['cases']) or result['cleanup'] != dict(server_exit=0,forced=False,group_gone=True,port_free=True):
        raise SystemExit('side observation incomplete; raw evidence retained')
