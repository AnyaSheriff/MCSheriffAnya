#!/usr/bin/env python3
"""Plan-only default: SAME selected floor24 ordinary/core, no preset collision decision."""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import signal
import socket
import struct
import subprocess
import sys
import time
import traceback

HELPERS=Path(__file__).resolve().parent
sys.path.insert(0,str(HELPERS))
import probe_wool_steps_777 as known
from probe_gameplay import connect, mapping
from smoke import handshake,send,varint
from observe import frame
OFFICIAL_PORT=26052
CORE_PORT=26026
CASES=[dict(label='stone-noaction',kind='noaction'),dict(label='stone-horizontal',kind='horizontal')]
CASES += [dict(label=f'intrusion-{depth}-flag{flag}',kind='intrusion',depth=depth,flag=flag) for depth in [.125,.5,1.] for flag in [0,1]]
CASES += [dict(label=f'airborne-intrusion-flag{flag}',kind='intrusion',flag=flag,prepared_airborne=True,target_y=64.875) for flag in [0,1]]
CASES += [dict(label='lawful-jump-descent',kind='descent'),dict(label='lawful-edge-descent',kind='edge')]

CASES += [dict(label=f'nearcontact-{depth:.7g}-flag{flag}',kind='intrusion',depth=depth,flag=flag,nearcontact=True) for depth in [1e-7,1e-5,.0001,.001,.01,.0625] for flag in [0,1]]


def observer_connect(protocol,port):
    sock=socket.create_connection(('127.0.0.1',port),timeout=5)
    try:
        handshake(sock,777,port,2);name=b'McsaObserve'
        send(sock,b'\0'+varint(len(name))+name+bytes([38])*16)
        packet=io.BytesIO(frame(sock));assert known.uint31(packet)==2
        send(sock,b'\x03');send(sock,b'\x00\x05en_us\x02\x00\x01\x7f\x01\x00\x01\x00')
        packs=mapping(protocol,'configuration','toClient')['select_known_packs']
        until=time.monotonic()+15
        for _ in range(160):
            if time.monotonic()>until:raise TimeoutError('observer handshake')
            packet=io.BytesIO(frame(sock));kind=known.uint31(packet);payload=packet.read()
            if kind==packs:send(sock,b'\x07'+payload)
            elif kind==4:send(sock,b'\x04'+payload)
            elif kind==3:
                send(sock,b'\x03');return known.RawClient(sock,protocol)
        raise RuntimeError('observer packet budget')
    except BaseException:
        sock.close();raise


def teleport_evidence(raw, since, until):
    """Exact raw corrections/ACKs, including setup separately. No silent ACK guess."""
    result=[];position=None
    for row in raw:
        if row['direction']!='in' or row['name']!='position':continue
        data=io.BytesIO(bytes.fromhex(row['hex']));ident=known.uint31(data)
        values=struct.unpack('>ddddddffI',known.exact(data,60))
        if data.read():raise ValueError('position tail')
        pose=list(values[:3])+list(values[6:8]);flags=values[8]
        if flags&~511:raise ValueError('unknown position flags')
        if flags&7:raise ValueError(('relative XYZ teleport unsupported; preserve raw and review',flags))
        if flags&31 and position is None:raise ValueError('relative initial teleport')
        if position is not None:pose=[v+position[i] if flags&(1<<i) else v for i,v in enumerate(pose)]
        position=pose
        if not since<=row['at']<=until:continue
        acks=[]
        for ack in raw:
            if ack['direction']!='out' or ack['name']!='teleport_confirm' or not row['at']<=ack['at']<=until:continue
            source=io.BytesIO(bytes.fromhex(ack['hex']))
            if known.uint31(source)!=ident:continue
            ack_pose=list(struct.unpack('>dddff',known.exact(source,32)))
            if source.read():raise ValueError('teleport ACK tail')
            if any(abs(a-b)>1e-10 for a,b in zip(ack_pose[:3],pose[:3])) or any(abs(a-b)>1e-4 for a,b in zip(ack_pose[3:],pose[3:])):raise ValueError('teleport ACK pose mismatch')
            acks.append(ack)
        if len(acks)!=1:raise ValueError(('missing/duplicate teleport ACK',ident,len(acks)))
        result.append(dict(id=ident,position=pose,flags=flags,raw=row,ack=acks[0]))
    return result


def endpoint_observation(before, requested, after):
    near=lambda a,b:all(abs(x-y)<1e-10 for x,y in zip(a,b))
    if near(after,before):return 'endpoint_unchanged'
    if near(after,requested):return 'requested_endpoint_observed'
    return 'different_endpoint_observed; inspect raw path/corrections before interpretation'


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--run',action='store_true');target=ap.add_mutually_exclusive_group();target.add_argument('--jar',type=Path);target.add_argument('--core-test',type=Path);ap.add_argument('--expected-sha256');ap.add_argument('--data',type=Path);ap.add_argument('--output',type=Path)
    args=ap.parse_args()
    if args.core_test is not None and (args.expected_sha256 is None or not re.fullmatch('[0-9a-f]{64}',args.expected_sha256)):ap.error('--core-test requires exact lowercase --expected-sha256')
    if args.expected_sha256 is not None and args.core_test is None:ap.error('--expected-sha256 requires --core-test')
    port=CORE_PORT if args.core_test else OFFICIAL_PORT
    report=dict(scope='selected stone floor24 including12 nearcontact inputs; no thresholds/general collision claim',port=port,protocol=777,cases=[dict(c,status='not-run') for c in CASES],target='core-test' if args.core_test else 'ordinary',core_binary_sha256=args.expected_sha256,jar_sha1=known.JAR_SHA1 if args.jar or not args.core_test else None,json_sha256=known.HASHES,runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),helper_sha256={name:hashlib.sha256((HELPERS/name).read_bytes()).hexdigest() for name in ['probe_wool_steps_777.py','probe_gameplay.py','probe_white_concrete_slab.py','generate_tables.py','observe.py','smoke.py']},commands=[],server_fields_history=[],deferred=['wall/corner','stale teleport before ACK','piston','vehicle','sleeping','creative flight','spectator actor','crouch geometry','slabs/water/steps'],limits=['pure vertical endpoint alone cannot distinguish whole rejection from clipped boundary','OnGround readback is accepted server state, not proof every input flag is physical support','jump+descent composite does not isolate landing event','no sculk/vitals/fall side effects instrumented'])
    if not args.run:print(json.dumps(dict(report,mode='plan-only'),indent=2));return 0
    if (args.jar is None and args.core_test is None) or args.data is None or args.output is None:ap.error('--run requires --jar or --core-test, plus --data --output')
    if args.jar:assert hashlib.sha1(args.jar.read_bytes()).hexdigest()==known.JAR_SHA1,'JAR pin'
    else:assert hashlib.sha256(args.core_test.read_bytes()).hexdigest()==args.expected_sha256,'core binary SHA256 pin'
    for name,digest in known.HASHES.items():assert hashlib.sha256((args.data/name).read_bytes()).hexdigest()==digest,name
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();(out/'runner.py').write_bytes(Path(__file__).read_bytes())
    (world/'eula.txt').write_text('eula=true\n')
    (world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nwhite-list=false\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=3\nsimulation-distance=3\ngamemode=survival\nspawn-protection=0\n')
    with socket.socket() as check:check.bind(('127.0.0.1',port))
    deadline=time.monotonic()+420;server=client=observer=None;log=out/'server.log'
    try:
        if args.jar:
            argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(args.jar.resolve()),'nogui'];ready='Done ('
        else:
            for marker in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']:(world/marker).touch()
            argv=[str(args.core_test.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture'];ready='MCSA gameplay probe ready'
        with log.open('w') as output:server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
        while ready not in log.read_text():
            assert server.poll() is None and time.monotonic()<deadline,'startup bound';time.sleep(.1)
        protocol=json.loads((args.data/'protocol.json').read_text());entities=json.loads((args.data/'entities.json').read_text());player_kind=next(x['id'] for x in entities if x['name']=='player')
        client=connect(port,protocol,known.RawClient,777);client.pump(10,lambda x:x['name']=='position');assert client.position is not None
        observer=observer_connect(protocol,port);observer.pump(5,lambda x:x['name']=='position');assert observer.position is not None
        def pump(seconds):
            end=time.monotonic()+seconds
            while time.monotonic()<end:
                if time.monotonic()>deadline:raise TimeoutError('global420s bound')
                client.pump(.025);observer.pump(.025)
        def command(text):
            assert time.monotonic()<deadline
            start=len(log.read_text());entry=dict(text=text,at=time.monotonic());report['commands'].append(entry)
            server.stdin.write(text+'\n');server.stdin.flush();pump(.2)
            entry['stdout']=log.read_text()[start:].splitlines()
            if ' run say FLOOR_QUERY_' in text:
                ack='[Server] '+text.rsplit(' ',1)[-1];until=min(deadline,time.monotonic()+3)
                while not any(line.endswith(ack) for line in log.read_text()[start:].splitlines()) and time.monotonic()<until:pump(.1)
                lines=[line for line in log.read_text()[start:].splitlines() if line.endswith(ack)]
                if len(lines)!=1:raise RuntimeError('source ACK missing/duplicate: '+ack)
                entry.update(ack=ack,server_log_line=lines[0])
        def query(field):
            start=len(log.read_text());command('data get entity McsaProbe '+field);until=min(deadline,time.monotonic()+4)
            while time.monotonic()<until:
                matches=re.findall(r'McsaProbe has the following entity data: ([^\r\n]+)',log.read_text()[start:])
                if len(matches)==1:
                    value=known.parse_entity_data(field,matches[0]);report['server_fields_history'].append(dict(field=field,raw=matches[0],parsed=value,at=time.monotonic()));return value
                pump(.1)
            raise RuntimeError('server field unavailable: '+field)
        def pose():return dict(position=query('Pos'),ground=query('OnGround'))
        def packet(x,y,z,ground):client.send('position',struct.pack('>dddB',x,y,z,int(ground)));pump(.05)
        command('gamemode spectator McsaObserve');pump(.3)
        observer_modes=[struct.unpack('>f',bytes.fromhex(r['hex'])[1:])[0] for r in observer.raw if r['direction']=='in' and r['name']=='game_state_change' and bytes.fromhex(r['hex'])[:1]==b'\3']
        if not observer_modes or observer_modes[-1]!=3:raise RuntimeError('observer spectator not confirmed')
        report['observer_modes']=observer_modes
        command('setblock 3 69 16 stone');command('tp McsaObserve 3.5 70 16.5');pump(.4);observer.send('position',struct.pack('>dddB',3.5,70.,16.5,0));pump(.2)
        for text in ['gamerule minecraft:spawn_mobs false','gamerule minecraft:random_tick_speed 0','weather clear','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(text)
        modes=[struct.unpack('>f',bytes.fromhex(r['hex'])[1:])[0] for r in client.raw if r['direction']=='in' and r['name']=='game_state_change' and bytes.fromhex(r['hex'])[:1]==b'\3']
        report['mode_changes']=modes
        if not modes or modes[-1]!=0:raise RuntimeError('actor survival not confirmed')
        command('scoreboard objectives add mcsa_pose dummy')
        def score_observation(field,holder):
            first=len(report['commands'])
            command(f'execute store result score {holder} mcsa_pose run data get entity McsaProbe {field}')
            command(f'scoreboard players get {holder} mcsa_pose')
            report.setdefault('data_result_observations',[]).append(dict(field=field,holder=holder,commands=report['commands'][first:]))
        serial=0
        def block_is(x,y,z,spec):
            nonlocal serial
            serial+=1;command(f'execute if block {x} {y} {z} {spec} run say FLOOR_QUERY_{serial}')
        for case in report['cases']:
            case.update(status='INCONCLUSIVE',reason='incomplete');case['setup_start']=time.monotonic()
            command('fill -2 62 -2 9 69 2 air');command('fill -2 61 -2 9 61 2 stone')
            command('fill -2 64 -2 '+('2' if case['kind']=='edge' else '9')+' 64 2 stone')
            command('tp McsaProbe 0.5 65.25 0.5');pump(.3)
            y=65.25;v=0.
            for _ in range(10):
                v=(v-.08)*.98;y=max(65,y+v);packet(.5,y,.5,y==65)
                if y==65:break
            pump(.3)
            for x in [0,1,2]:block_is(x,64,0,'stone')
            if case['kind']=='edge':
                for x in [3,4]:block_is(x,64,0,'air');block_is(x,61,0,'stone')
            pump(.3)
            tracked=known.observe_actor(observer.raw,player_kind);actor_id=tracked[-1]['entity']
            client.send('player_input',b'\x20');pump(.3);standing_start=time.monotonic();client.send('player_input',b'\0');pump(.5)
            standing,evidence=known.metadata_sneak(observer.raw,actor_id,standing_start);case['standing_proof']=dict(values=standing,evidence=evidence)
            if 'flags' not in standing or standing['flags']&2 or standing.get('pose')!=0:raise RuntimeError('standing metadata not confirmed')
            if case.get('prepared_airborne'):
                block_is(0,65,0,'air')
                command('tp McsaProbe 0.5 65.5 0.5');pump(.1);packet(.5,65.5,.5,False)
                case['airborne_preparation']='TP+ACK then one nongrounded position; not a real fall trajectory'
            case['before']=pose()
            if case['label']=='stone-noaction':
                score_observation('Pos','floor_pos');score_observation('OnGround','floor_ground1')
            elif case['label']=='airborne-intrusion-flag0':
                score_observation('OnGround','floor_ground0')
            pump(.1 if case.get('prepared_airborne') else .3)
            case['setup_end']=time.monotonic();case['setup_teleports']=teleport_evidence(client.raw,case['setup_start'],case['setup_end'])
            if not case['setup_teleports']:raise RuntimeError('setup TP+ACK not proved')
            expected_y=65.5 if case.get('prepared_airborne') else 65
            if any(abs(a-b)>.02 for a,b in zip(case['before']['position'],[.5,expected_y,.5])) or case['before']['ground']!= (not case.get('prepared_airborne',False)):raise RuntimeError('unsupported initial server pose/ground')
            published=known.observe_actor(observer.raw,player_kind);case['observer_before']=published[-1]
            if any(abs(a-b)>.04 for a,b in zip(case['observer_before']['position'],case['before']['position'])):raise RuntimeError('observer setup endpoint differs')
            case['start']=time.monotonic();requested=[.5,65,.5]
            if case['kind']=='noaction':pump(1.)
            elif case['kind']=='horizontal':
                for n in range(1,25):packet(.5+n*.1,65,.5,True)
                requested=[2.9,65,.5]
            elif case['kind']=='intrusion':
                requested=[.5,case.get('target_y',65-case.get('depth',0)),.5];packet(*requested,case['flag'])
            elif case['kind']=='descent':
                y=65.;v=.42;trajectory=[]
                for _ in range(40):
                    y=max(65,y+v);trajectory.append(y);packet(.5,y,.5,y==65);v=(v-.08)*.98
                    if y==65:break
                case['candidate_y']=trajectory
            elif case['kind']=='edge':
                for n in range(1,30):packet(.5+n*.1,65,.5,n<28)
                y=65.;v=0.;trajectory=[]
                for _ in range(30):
                    v=(v-.08)*.98;y=max(62,y+v);trajectory.append(y);packet(3.4,y,.5,y==62)
                    if y==62:break
                case['candidate_y']=trajectory;requested=[3.4,62,.5]
            pump(.5);case['requested_endpoint']=requested;case['after']=pose();pump(.5);case['end']=time.monotonic()
            case['teleports']=teleport_evidence(client.raw,case['start'],case['end'])
            all_events=known.observe_actor(observer.raw,player_kind);case['observer_path']=[e for e in all_events if case['start']<=e['at']<=case['end']];case['observer_after']=all_events[-1]
            if any(abs(a-b)>.04 for a,b in zip(case['observer_after']['position'],case['after']['position'])):raise RuntimeError('observer/server endpoint mismatch')
            case['observation']=endpoint_observation(case['before']['position'],requested,case['after']['position'])
            case['coordinate_comparison_tolerance']=1e-10
            if case.get('nearcontact'):case['resolution_limit']='observer delta quantization cannot prove tiny shifts; use exact server Pos raw readback and requested double, not observer tolerance as collision outcome'
            if case['kind']=='intrusion':case['identification_limit']='airborne65.5 vs floor65 vs requested64.875 are distinct measured coordinates; no decision preset' if case.get('prepared_airborne') else 'grounded purevertical unchanged cannot distinguish whole reject from clip-to-boundary'
            if case['kind']=='intrusion':case.update(status='OBSERVED',reason='measured endpoint/corrections/ACK; no expected collision decision imposed')
            else:
                valid=not case['teleports'] and all(abs(a-b)<.03 for a,b in zip(requested,case['after']['position']))
                path=case['observer_path']
                if case['kind']!='noaction':valid=valid and len(path)>=2
                if case['kind']=='horizontal':valid=valid and bool(path) and max(e['position'][0] for e in path)-min(e['position'][0] for e in path)>1 and all(abs(e['position'][1]-65)<.03 and abs(e['position'][2]-.5)<.03 for e in path)
                if case['kind']=='noaction':valid=valid and all(all(abs(a-b)<.03 for a,b in zip(e['position'],case['before']['position'])) for e in path)
                if case['kind']=='descent':valid=valid and any(e['position'][1]>65.1 for e in path) and any(e['position'][1]<65.1 for e in path)
                if case['kind']=='edge':valid=valid and any(e['position'][0]>3.3 and e['position'][1]<64.9 for e in path)
                valid=valid and case['after']['ground']
                case.update(status='OBSERVED' if valid else 'INCONCLUSIVE',reason='selected legal control confirmed' if valid else 'legal control needs raw review')
        report['controls_valid']=all(c['status']=='OBSERVED' for c in report['cases'] if c['kind']!='intrusion')
    except Exception as error:report.update(error=repr(error),traceback=traceback.format_exc())
    finally:
        for client_obj,key in [(client,'raw'),(observer,'observer_raw')]:
            if client_obj:report[key]=client_obj.raw;client_obj.sock.close()
        if server:
            def exists():
                try:os.killpg(server.pid,0);return True
                except ProcessLookupError:return False
            if server.poll() is None:
                try:server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=15)
                except (OSError,subprocess.TimeoutExpired):pass
            for sig,seconds in [(signal.SIGTERM,5),(signal.SIGKILL,5)]:
                if not exists():break
                try:os.killpg(server.pid,sig)
                except ProcessLookupError:pass
                until=time.monotonic()+seconds
                while time.monotonic()<until:
                    server.poll()
                    if not exists():break
                    time.sleep(.05)
            server.poll()
            with socket.socket() as check:
                try:check.bind(('127.0.0.1',port));free=True
                except OSError:free=False
            report['cleanup']=dict(pid=server.pid,exit=server.returncode,group_gone=not exists(),port_free=free)
            report['cleanup']['clean']=server.returncode==0 and report['cleanup']['group_gone'] and free
        for case in report['cases']:
            if case['status']=='not-run':case.update(status='INCONCLUSIVE',reason=report.get('error','not reached'))
        complete=report.get('controls_valid',False) and all(c['status']=='OBSERVED' for c in report['cases']) and report.get('cleanup',{}).get('clean',False) and 'error' not in report
        report['status']='OBSERVED' if complete else 'INCONCLUSIVE';(out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],output=str(out),cleanup=report.get('cleanup'))));return 0 if complete else 2


if __name__=='__main__':sys.exit(main())
