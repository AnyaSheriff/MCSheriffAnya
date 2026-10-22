#!/usr/bin/env python3
"""Настоящий340-клиент в Xvfb на обычном Play ядра; прозрачная запись своих пакетов."""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time

from live_client import stop
from live_legacy_client import launch
from observe import frame
from probe_gameplay import mapping
from probe_legacy_session import string
from probe_legacy_cow_core import CoreCowProbe
from probe_legacy_chunks import sections as chunk_sections
from smoke import read_varint, send


def camera_stable(rows, after, now):
    """Три свежих подтверждения за2сек; неподвижный340 не шлёт20пакетов/сек."""
    streak = []
    for row in rows:
        state = row.get('motion_state')
        if state is None or row['elapsed'] < after:
            continue
        valid = (state['ground'] and state['position'] is not None
                 and all(abs(v - e) < .01 for v, e in zip(state['position'], [.5, 128, .5]))
                 and state['rotation'] == [0.0, 0.0])
        if not valid:
            streak.clear()
            continue
        if streak and row['elapsed'] - streak[-1] > 1.5:
            streak.clear()
        streak.append(row['elapsed'])
    return (len(streak) >= 3 and streak[-1] - streak[0] >= 2
            and 0 <= now - streak[-1] <= 1.5)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--test-binary', type=Path, required=True)
    p.add_argument('--client-dir', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--proxy-port', type=int, default=26028)
    p.add_argument('--cows', action='store_true', help='Adult/baby cow metadata, movement and retracking on the real client')
    a = p.parse_args()
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    assert a.proxy_port == 26028, 'Only the isolated client proxy port is permitted'
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    names = {direction: {v:k for k,v in mapping(protocol,'play',direction).items()} for direction in ['toServer','toClient']}
    report = {'checks': [], 'packets': [], 'complete_gameplay': False,
              'test_binary_sha256':hashlib.sha256(a.test_binary.read_bytes()).hexdigest()}
    cow_report = {'height_origin':-64}
    cow_observer = CoreCowProbe(None, protocol, cow_report) if a.cows else None
    client_motion = {'position':None, 'rotation':None, 'ground':False}
    began=time.monotonic()
    lock = threading.Lock(); stopped = threading.Event(); phase = threading.Event()
    peers = []; workers = []
    a.output.mkdir(parents=True,exist_ok=True)
    server_log = a.output / 'server.log'; client_log = a.output / 'client.log'
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-core-') as directory, socket.socket() as listener:
        root=Path(directory); core=root/'core'; game=root/'client'; core.mkdir(); game.mkdir()
        (core/'mcsa-gameplay-probe.marker').touch()
        listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
        listener.bind(('127.0.0.1',a.proxy_port)); listener.listen(1); listener.settimeout(30)
        with server_log.open('w') as slog, client_log.open('w') as clog:
            server=subprocess.Popen([str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::serve_java_1_12_2_probe','--nocapture'],cwd=core,stdin=subprocess.PIPE,stdout=slog,stderr=subprocess.STDOUT,text=True)
            client=None

            def command(value):
                server.stdin.write(value+'\n');server.stdin.flush()

            def relay(source,target,direction):
                try:
                    while not stopped.is_set():
                        body=frame(source); data=io.BytesIO(body); ident=read_varint(data.read); payload=data.read()
                        play=phase.is_set()
                        record={'direction':direction,'id':ident,'bytes':len(payload),
                                'elapsed':round(time.monotonic()-began,6)}
                        if not play and direction=='toClient':
                            assert ident==2, f'Unexpected Login packet {ident}'
                            record['name']='login_success';phase.set()
                        else:
                            name=names[direction].get(ident,f'unknown-{ident}');record['name']=name
                            if cow_observer is not None and direction == 'toClient':
                                with lock:
                                    cow_observer.observe_packet(name, payload)
                            data=io.BytesIO(payload)
                            if name=='teleport_confirm': record['teleport']=read_varint(data.read)
                            elif name=='keep_alive': record['token']=struct.unpack('>q',payload)[0]
                            elif name=='position' and direction=='toClient':
                                record['position']=list(struct.unpack('>dddff',data.read(32)))
                                record['flags']=data.read(1)[0];record['teleport']=read_varint(data.read)
                            elif name in ('position','position_look') and direction=='toServer':
                                record['position']=list(struct.unpack('>ddd',payload[:24]))
                                if name=='position_look':
                                    record['rotation']=list(struct.unpack('>ff',payload[24:32]))
                                record['ground']=bool(payload[-1])
                            elif name in ('look','flying') and direction=='toServer':
                                if name=='look':
                                    record['rotation']=list(struct.unpack('>ff',payload[:8]))
                                record['ground']=bool(payload[-1])
                            elif name=='map_chunk':
                                record['chunk']=list(struct.unpack('>ii',payload[:8]));record['sha256']=hashlib.sha256(payload).hexdigest()
                                if a.cows and record['chunk']==[0,0]:
                                    number=len(report.get('floor_chunk_checks',[]))
                                    path=a.output/f'initial-floor-chunk-{number}.bin'
                                    path.write_bytes(payload)
                                    _,_,full,sections=chunk_sections(payload)
                                    section=next((section for section in sections if section['y']==7),None)
                                    floor=[] if section is None else section['blocks'][15*256:16*256]
                                    check={'file':path.name,'sha256':record['sha256'],'full':full,
                                           'section':7,'legacy_y':127,'floor_count':len(floor),
                                           'floor_states':sorted(set(floor))}
                                    report.setdefault('floor_chunk_checks',[]).append(check)
                                    assert full and floor==[16]*256,('Initial stone platform missing from MapChunk',check)
                        if direction=='toServer' and record.get('name') in ('position','position_look','look','flying'):
                            for key in ('position','rotation','ground'):
                                if key in record:
                                    client_motion[key]=record[key]
                            record['motion_state']=dict(client_motion)
                        with lock: report['packets'].append(record)
                        send(target,body)
                except BaseException as error:
                    if not stopped.is_set():
                        with lock: report.setdefault('relay_errors',[]).append(f'{direction}: {type(error).__name__}: {error}')

            def wait(predicate,seconds=30):
                deadline=time.monotonic()+seconds
                while time.monotonic()<deadline:
                    log=server_log.read_text()
                    assert server.poll() is None,'core exited'
                    assert 'probe connection:' not in log and 'MCSA play probe failure:' not in log, log[-1500:]
                    assert client is None or client.poll() is None,'client exited'
                    with lock:
                        assert not report.get('relay_errors'), report.get('relay_errors')
                        if predicate(report['packets']): return
                    time.sleep(.05)
                raise AssertionError('Timed out waiting for client evidence')

            try:
                wait(lambda _: 'MCSA gameplay probe ready on 26027' in server_log.read_text())
                if a.cows:
                    # Java стартует дольше периода выгрузки. Площадка fixture
                    # должна пережить отсутствие игроков до первого Login.
                    command('forceload add -24 -24 23 23')
                    wait(lambda _: 'Marked 16 chunks' in server_log.read_text()
                         and 'to be force loaded' in server_log.read_text())
                    report['fixture_floor_retention']={'command':'forceload add -24 -24 23 23',
                                                       'chunks':16,'before_client_launch':True}
                client=launch(a.client_dir.resolve(),game,a.proxy_port,clog,pause_on_lost_focus=not a.cows)
                peer,_=listener.accept();peers.append(peer)
                backend=socket.create_connection(('127.0.0.1',26027),timeout=10);backend.settimeout(None);peers.append(backend)
                hello=frame(peer);data=io.BytesIO(hello)
                assert read_varint(data.read)==0 and read_varint(data.read)==340
                string(data);data.read(2);assert read_varint(data.read)==2 and not data.read()
                send(backend,hello);login=frame(peer);data=io.BytesIO(login)
                assert read_varint(data.read)==0 and string(data)=='McsaProbe' and not data.read()
                send(backend,login);report['checks'].append('actual-client-login340')
                for source,target,direction in [(backend,peer,'toClient'),(peer,backend,'toServer')]:
                    worker=threading.Thread(target=relay,args=(source,target,direction),daemon=True);workers.append(worker);worker.start()
                wait(lambda rows:any(r.get('name')=='map_chunk' for r in rows) and any(r.get('name')=='teleport_confirm' for r in rows))
                command('say MCSA-legacy-runtime-ready')
                wait(lambda _: 'MCSA-legacy-runtime-ready' in client_log.read_text())
                report['checks'].append('actual-client-world-and-chat')
                command('tp McsaProbe 1.5 64 1.5')
                wait(lambda rows:len({r.get('teleport') for r in rows if r.get('name')=='teleport_confirm'})>=2)
                report['checks'].append('second-core-teleport-ack')
                wait(lambda rows:any(r.get('name')=='position_look' and r['direction']=='toServer' and all(abs(v-e)<.01 for v,e in zip(r['position'],[1.5,128,1.5])) for r in rows))
                report['checks'].append('client-position-matches-world-window')
                if a.cows:
                    report['cow_render_review']='pending: screenshots require visual inspection'
                    report['cow_limits']=['Rotation uses data merge; /tp yaw/pitch is a separate known gameplay gap.',
                                          'No combat, sound, or effects are covered by this cow session.',
                                          'Visibility only: cows remain in retained chunks of World::in_memory.']
                    report['cow_visibility_scope']={'tracking_range_blocks':32,'retained_chunk_radius':4,
                        'far_player_position':[48.5,64.0,.5],'chunk_unload_and_disk_persistence':False}

                    def stage(name, text):
                        with lock:
                            cow_observer.stage=name
                        command(text)

                    def entity_at(x,z):
                        return next((ident for ident,cow in cow_observer.cows.items()
                                     if cow['tracked'] and abs(cow['position'][0]-x)<.01
                                     and abs(cow['position'][2]-z)<.01),None)

                    def field_is(ident,index,value):
                        return cow_observer.cows[ident]['fields'].get(index,{}).get('value')==value

                    def grounded_camera(label):
                        start=time.monotonic()-began
                        wait(lambda rows:camera_stable(rows,start,time.monotonic()-began),20)
                        report.setdefault('camera_ground_checks',[]).append(
                            {'stage':label,'stable_seconds':2,'position':[.5,128,.5],
                             'rotation':[0.0,0.0],'ground':True,'minimum_confirmations':3,
                             'maximum_gap_and_age_seconds':1.5})

                    def capture(label):
                        # Только DISPLAY/XAUTHORITY нашего отдельного Xvfb;
                        # окружение рабочего стола и токены не используются.
                        private=None
                        for candidate in Path('/proc').iterdir():
                            if not candidate.name.isdigit():
                                continue
                            try:
                                if os.getsid(int(candidate.name))!=client.pid:
                                    continue
                                if not (candidate/'exe').resolve().name.startswith('java'):
                                    continue
                                pairs=(candidate/'environ').read_bytes().split(b'\0')
                                private={k.decode():v.decode() for item in pairs if b'=' in item
                                         for k,v in [item.split(b'=',1)] if k in (b'DISPLAY',b'XAUTHORITY')}
                                if private.get('DISPLAY'):
                                    break
                            except (OSError,ProcessLookupError):
                                continue
                        assert private and private.get('DISPLAY'),'Private Xvfb display not found'
                        environment=os.environ.copy()
                        for key in ('DISPLAY','XAUTHORITY','WAYLAND_DISPLAY','WAYLAND_SOCKET'):
                            environment.pop(key,None)
                        environment.update(private)
                        path=a.output/(label+'.png')
                        subprocess.run(['import','-window','root',str(path.resolve())],
                                       env=environment,check=True,timeout=10,
                                       stdout=subprocess.PIPE,stderr=subprocess.PIPE)
                        report.setdefault('screenshots',[]).append({'stage':label,'file':path.name,
                            'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})

                    stage('cow-camera','tp McsaProbe 0.5 64 0.5 0 0')
                    wait(lambda rows:len({r.get('teleport') for r in rows if r.get('name')=='teleport_confirm'})>=3)
                    stage('adult-spawn','summon cow -1.5 64 5.5 {NoAI:1b,Silent:1b,Tags:["mcsa_visual_adult"]}')
                    wait(lambda _:entity_at(-1.5,5.5) is not None)
                    adult=entity_at(-1.5,5.5)
                    wait(lambda _:field_is(adult,7,10.0) and field_is(adult,11,1))
                    stage('baby-spawn','summon cow 1.5 64 5.5 {NoAI:1b,Silent:1b,Age:-24000,Tags:["mcsa_visual_baby"]}')
                    wait(lambda _:entity_at(1.5,5.5) is not None)
                    baby=entity_at(1.5,5.5)
                    wait(lambda _:field_is(baby,12,1) and field_is(baby,7,10.0))
                    command('say MCSA-cow-pair-ready')
                    wait(lambda _:'MCSA-cow-pair-ready' in client_log.read_text())
                    grounded_camera('adult-and-baby')
                    capture('core-cow-adult-and-baby')
                    report['checks'].append('actual-client-cow-adult-and-baby')
                    selector='@e[type=cow,tag=mcsa_visual_adult,limit=1]'
                    stage('health4',f'data merge entity {selector} {{Health:4f}}')
                    wait(lambda _:field_is(adult,7,4.0))
                    stage('cow-turn',f'data merge entity {selector} {{Rotation:[90f,10f]}}')
                    wait(lambda _:any(e['stage']=='cow-turn' and e['name']=='entity_head_rotation'
                                      and e['entity']==adult for e in cow_observer.events))
                    stage('cow-teleport',f'tp {selector} -2.5 64 6.5')
                    wait(lambda _:entity_at(-2.5,6.5)==adult)
                    report['checks'].append('actual-client-cow-health-and-motion')
                    for entity in (adult,baby):
                        x,_,z=cow_observer.cows[entity]['position']
                        assert abs(x-48.5)>32 or abs(z-.5)>32,('Cow still visible',entity)
                        assert abs(math.floor(x/16)-3)<=4 and abs(math.floor(z/16))<=4,('Cow chunk would unload',entity)
                    stage('cow-untrack','tp McsaProbe 48.5 64 0.5')
                    wait(lambda _:not cow_observer.cows[adult]['tracked'] and not cow_observer.cows[baby]['tracked'])
                    report['checks'].append('actual-client-cow-range-remove')
                    stage('cow-retrack','tp McsaProbe 0.5 64 0.5 0 0')
                    wait(lambda _:cow_observer.cows[adult]['tracked'] and cow_observer.cows[baby]['tracked']
                         and field_is(adult,7,4.0) and field_is(baby,12,1))
                    command('say MCSA-cow-retracked')
                    wait(lambda _:'MCSA-cow-retracked' in client_log.read_text())
                    grounded_camera('retracked')
                    capture('core-cow-retracked')
                    report['checks'].append('actual-client-cow-identities-and-state-retracked')
                    with lock:
                        previous_keepalives={r['token'] for r in report['packets']
                                             if r.get('name')=='keep_alive' and r['direction']=='toClient'}
                    wait(lambda rows:bool(({r['token'] for r in rows if r.get('name')=='keep_alive'
                                            and r['direction']=='toClient'}-previous_keepalives)
                                          & {r['token'] for r in rows if r.get('name')=='keep_alive'
                                             and r['direction']=='toServer'}),25)
                    report['checks'].append('actual-client-keepalive-after-cow-retracking')
                wait(lambda rows:bool({r['token'] for r in rows if r.get('name')=='keep_alive' and r['direction']=='toClient'} & {r['token'] for r in rows if r.get('name')=='keep_alive' and r['direction']=='toServer'}),25)
                report['checks'].append('keepalive-after-world-loaded')
                assert not report.get('relay_errors'), report.get('relay_errors')
                assert not any(word in client_log.read_text() for word in ['DecoderException','ReportedException','Unexpected error'])
            except BaseException as error:
                report['error']=str(error);raise
            finally:
                stopped.set()
                if client is not None: stop(client)
                for peer in peers:
                    try: peer.shutdown(socket.SHUT_RDWR)
                    except OSError: pass
                    peer.close()
                for worker in workers: worker.join(timeout=2)
                if server.poll() is None:
                    command('stop')
                    server.stdin.close()
                    try: server.wait(timeout=10)
                    except subprocess.TimeoutExpired: server.kill();server.wait()
                report['exit_code']=server.returncode
                if cow_observer is not None:
                    report['cow_events']=cow_observer.events
                    report['cow_state']=cow_observer.cows
                    report['cow_observer']=cow_report
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0
    print(json.dumps({'checks':report['checks'],'packets':len(report['packets'])}))


if __name__=='__main__':main()
