#!/usr/bin/env python3
"""План по умолчанию; --run dry step-only matrix19.
Candidate путь требует server pose + independent observer; неподтверждённое INCONCLUSIVE.
"""
import argparse
import io
import struct
import hashlib
import json
import math
import os
import signal
from pathlib import Path
import re
import socket
import subprocess
import sys
import time
import traceback

HELPERS = Path(__file__).resolve().parent
sys.path.insert(0, str(HELPERS))
from probe_gameplay import Client, connect, mapping
from smoke import varint, read_varint, send, handshake
from observe import frame
from probe_white_concrete_slab import changes
from generate_tables import states

JAR_SHA1 = '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
HASHES = {
 'protocol.json': '13a10695aa1ed3604ed674ce8fdc8a759f48118714eab98644e685a3d1960727',
 'blocks.json': '776312e51207352734304a293d9300f6c69b4efa40f00ac574c99d1ca7889873',
 'items.json': 'f0d0e447dd0311b7322f4548971d6ebf6074b5842fd4c7b4e3d8698e00b5edcb',
 'entities.json': '33c2d7569e10cb98d86240e8135396702edb92c98e6ccb1c18bf6a481f22e3d5',
}
SURFACES = [dict(label='stone',surface='stone',feet=65.,source_y=64),
 dict(label='full-wool',surface='white_wool',feet=65.,source_y=64),
 dict(label='bottom',surface='white_wool_slab[type=bottom,waterlogged=false]',feet=64.5,source_y=64),
 dict(label='top',surface='white_wool_slab[type=top,waterlogged=false]',feet=65.,source_y=64),
 dict(label='double',surface='white_wool_slab[type=double,waterlogged=false]',feet=65.,source_y=64),
 dict(label='carpet',surface='white_carpet',feet=65.0625,source_y=65)]
CASES = [dict(surface, label=surface['label']+'-'+mode+'-'+action,action=action,sneak=sneak)
 for surface in SURFACES for mode,sneak in [('normal',False),('sneak',True)] for action in ['step','noaction'] if not (sneak and action=='noaction')]
CASES.append(dict(SURFACES[0],label='stone-normal-step-end',action='step',sneak=False))

NUMBER = r'[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?'


def parse_entity_data(field, value):
    """Только точная форма выбранного поля; никаких guessed defaults."""
    value = value.strip()
    if field == 'Pos':
        match = re.fullmatch(r'\[\s*(' + NUMBER + r')[dD]?\s*,\s*(' + NUMBER + r')[dD]?\s*,\s*(' + NUMBER + r')[dD]?\s*\]', value)
        if not match:
            raise ValueError(('invalid Pos', value))
        result = [float(x) for x in match.groups()]
        if not all(math.isfinite(x) for x in result):
            raise ValueError('nonfinite Pos')
        return result
    if field == 'OnGround':
        if value not in ('0b', '1b', 'false', 'true'):
            raise ValueError(('invalid OnGround', value))
        return value in ('1b', 'true')
    raise ValueError(('unsupported field', field))


class RawClient(Client):
    def __init__(self, *args):
        super().__init__(*args)
        self.raw = []

    def observe_packet(self, name, payload):
        if len(self.raw) >= 20000:
            raise RuntimeError('raw packet bound')
        self.raw.append(dict(direction='in', name=name, hex=payload.hex(), at=time.monotonic()))

    def send(self, name, payload=b''):
        if len(self.raw) >= 20000:
            raise RuntimeError('raw packet bound')
        self.raw.append(dict(direction='out', name=name, hex=payload.hex(), at=time.monotonic()))
        # Никаких автоматически добавленных position/ground пакетов.
        super().send(name, payload)



def observer_connect(protocol):
    # Собственный login по уже проверенному raw helper; имя отдельно от исполнителя.
    sock = socket.create_connection(('127.0.0.1', 26051), timeout=5)
    try:
        handshake(sock, 777, 26051, 2)
        name = b'McsaObserve'
        send(sock, b'\0'+varint(len(name))+name+bytes([38])*16)
        packet = io.BytesIO(frame(sock))
        assert read_varint(packet.read)==2
        send(sock,b'\x03');send(sock,b'\x00\x05en_us\x02\x00\x01\x7f\x01\x00\x01\x00')
        known = mapping(protocol,'configuration','toClient')['select_known_packs']
        for _ in range(160):
            packet=io.BytesIO(frame(sock));kind=read_varint(packet.read);payload=packet.read()
            if kind==known:send(sock,b'\x07'+payload)
            elif kind==4:send(sock,b'\x04'+payload)
            elif kind==3:
                send(sock,b'\x03');return RawClient(sock,protocol)
        raise RuntimeError('observer configuration budget')
    except BaseException:
        sock.close();raise


def exact(data, count):
    value=data.read(count)
    if len(value)!=count:raise ValueError('truncated movement payload')
    return value


def uint31(data):
    value=0
    for shift in range(0,35,7):
        byte=exact(data,1)[0]
        if shift==28 and byte&248:raise ValueError('invalid/nonpositive VarInt')
        value|=(byte&127)<<shift
        if byte<128:return value
    raise ValueError('VarInt bound')


def decode_movement(name, data):
    """777 JSON sync positionType; opaque entityDelta from our movement.rs.
    Preserve tick offsets; packet receipt is NOT interpolated client execution time.
    """
    steps=[]
    if name=='sync_entity_position':
        tag=uint31(data)
        if tag==0:count=1
        elif tag==1:
            count=uint31(data)
            if not 1<=count<=1024:raise ValueError('absolute step count')
        else:raise ValueError('unknown positionType')
        for _ in range(count):
            position=list(struct.unpack('>ddd',exact(data,24)))
            if not all(math.isfinite(x) for x in position):raise ValueError('nonfinite position')
            steps.append(dict(position=position,tick_offset=uint31(data) if tag==1 else 0))
        angles=struct.unpack('>ff',exact(data,8))
        if not all(math.isfinite(x) for x in angles):raise ValueError('nonfinite rotation')
        ground=exact(data,1)[0]
        if ground>1:raise ValueError('invalid ground')
    elif name in ('rel_entity_move','entity_move_look'):
        properties=uint31(data);count=properties>>1;ground=properties&1
        if count>1024:raise ValueError('relative step count')
        for _ in range(count or 1):
            tick=uint31(data) if count else 0
            steps.append(dict(delta=list(struct.unpack('>hhh',exact(data,6))),tick_offset=tick))
        if name=='entity_move_look':exact(data,2)
    else:raise ValueError('unsupported movement packet')
    if data.read():raise ValueError('trailing movement payload')
    return bool(ground),steps


def observe_actor(raw, player_type):
    actors={};events=[]
    for row in raw:
        if row['direction']!='in':continue
        name=row['name'];data=io.BytesIO(bytes.fromhex(row['hex']))
        if name not in ('spawn_entity','sync_entity_position','rel_entity_move','entity_move_look'):continue
        entity=uint31(data)
        if name=='spawn_entity':
            uid=exact(data,16).hex();kind=uint31(data)
            if kind==player_type:
                position=list(struct.unpack('>ddd',exact(data,24)))
                if not all(math.isfinite(x) for x in position):raise ValueError('nonfinite spawn')
                actors[entity]=dict(uuid=uid,position=position)
            continue
        if entity not in actors:continue
        actor=actors[entity]
        ground,steps=decode_movement(name,data)
        for index,step in enumerate(steps):
            if 'position' in step:actor['position']=step['position']
            else:actor['position']=[math.floor(v*4096+.5)/4096+d/4096 for v,d in zip(actor['position'],step['delta'])]
            actor['ground']=ground
            events.append(dict(at=row['at'],entity=entity,packet_name=name,step_index=index,tick_offset=step['tick_offset'],**actor))
    if len(actors)!=1:raise RuntimeError(('expected one other player',actors))
    return events


def metadata_sneak(raw, entity, since):
    values={};evidence=[]
    for row in raw:
        if row['direction']!='in' or row['name']!='entity_metadata' or row['at']<since:continue
        data=io.BytesIO(bytes.fromhex(row['hex']))
        if uint31(data)!=entity:continue
        seen=set();selected={}
        while True:
            key=exact(data,1)[0]
            if key==255:break
            if key in seen:raise ValueError('duplicate metadata key')
            seen.add(key);kind=uint31(data)
            if kind==0:value=exact(data,1)[0]
            elif kind in (1,12,14,15,19,20,42,43):value=uint31(data)
            elif kind==3:value=struct.unpack('>f',exact(data,4))[0]
            elif kind==8:
                value=exact(data,1)[0]
                if value>1:raise ValueError('metadata bool')
            elif kind==4:
                size=uint31(data)
                if size>32767:raise ValueError('metadata string bound')
                value=exact(data,size).decode()
            elif kind in (6,11,13,33):
                present=exact(data,1)[0]
                if present!=0:raise ValueError('nonempty metadata option outside selected decoder')
                value=None
            else:raise ValueError(('unsupported metadata serializer',kind))
            if key==0:
                if kind!=0:raise ValueError('shared_flags serializer')
                selected['flags']=value
            if key==6:
                if kind!=20:raise ValueError('pose serializer')
                selected['pose']=value
        if data.read():raise ValueError('trailing metadata')
        values.update(selected)
        if selected:evidence.append(dict(row,selected=selected))
    return values,evidence


def calibrate(client, observer, protocol, blocks, entities, command, query_field, report, deadline):
    lookup=states(blocks);sensor=(3,65,4)
    sensor_states={value:dict(props) for (name,props),value in lookup.items() if name=='sculk_sensor'}
    player_type=next(x['id'] for x in entities if x['name']=='player')
    def pump(duration):
        end=time.monotonic()+duration
        while time.monotonic()<end:
            assert time.monotonic()<deadline,'global deadline'
            client.pump(.025);observer.pump(.025)
    serial=0
    def block_is(pos,spec):
        nonlocal serial
        serial+=1;token='MCSA_MOVEMENT_QUERY_'+str(serial)
        command('execute if block '+' '.join(map(str,pos))+' '+spec+' run say '+token)
        return token
    def pose():return dict(position=query_field('Pos'),ground=query_field('OnGround'))
    def packet(x,y,z,ground):
        client.send('position',struct.pack('>dddB',x,y,z,int(ground)));pump(.05)
    command('gamemode spectator McsaObserve');pump(.3)
    observer_modes=[struct.unpack('>f',bytes.fromhex(row['hex'])[1:])[0] for row in observer.raw if row['direction']=='in' and row['name']=='game_state_change' and bytes.fromhex(row['hex'])[:1]==b'\3']
    report['observer_modes']=observer_modes
    if not observer_modes or observer_modes[-1]!=3:raise RuntimeError('INCONCLUSIVE observer spectator packet missing')
    command('setblock 3 69 16 stone');command('tp McsaObserve 3.5 70 16.5');pump(.5)
    observer.send('position',struct.pack('>dddB',3.5,70.,16.5,0));pump(.3)
    for text in ['gamerule minecraft:spawn_mobs false','gamerule minecraft:random_tick_speed 0','weather clear','gamemode creative McsaProbe','gamemode survival McsaProbe','gamemode spectator McsaObserve']:
        command(text)
    modes=[struct.unpack('>f',bytes.fromhex(row['hex'])[1:])[0] for row in client.raw if row['direction']=='in' and row['name']=='game_state_change' and bytes.fromhex(row['hex'])[:1]==b'\3']
    report['mode_changes']=modes
    if not modes or modes[-1]!=0:raise RuntimeError('INCONCLUSIVE survival packet missing')
    def settle(feet=65.):
        command(f'tp McsaProbe 0.5 {feet+.25} 0.5');pump(.3)
        y=feet+.25;v=0.
        for _ in range(10):
            v=(v-.08)*.98;y=max(feet,y+v);packet(.5,y,.5,y==feet)
            if y==feet:break
        pump(.3)
    # Отдельная pre-window collision sanity, без сенсора.
    command('fill -2 64 -2 9 64 2 stone');settle()
    before=pose();start=time.monotonic();packet(.5,64.875,.5,False);pump(.4);after=pose()
    corrected=any(x['direction']=='in' and x['name']=='position' and x['at']>=start for x in client.raw)
    report['collision_sanity']=dict(before=before,attempt=[.5,64.875,.5],after=after,correction=corrected)
    if abs(after['position'][1]-65)>.02 or (not corrected and after['position']==[.5,64.875,.5]):
        raise RuntimeError('INCONCLUSIVE collision/support sanity failed')
    for case in report['cases']:
        case['status']='INCONCLUSIVE';case['reason']='trial incomplete'
        feet=case['feet']
        command('setblock 3 65 4 air');command('fill -2 64 -2 9 65 2 air');command('fill -2 63 -2 9 63 2 stone')
        if case['source_y']==65:command('fill -2 64 -2 9 64 2 stone')
        command(f"fill -2 {case['source_y']} -2 9 {case['source_y']} 2 "+case['surface'])
        settle(feet)
        collision_start=time.monotonic();packet(.5,feet-.125,.5,False);pump(.3)
        supported=pose();case['support_sanity']=dict(at=collision_start,attempt_y=feet-.125,server=supported)
        if abs(supported['position'][1]-feet)>.02:raise RuntimeError('INCONCLUSIVE selected surface support failed')
        settle(feet)
        actor_events=observe_actor(observer.raw,player_type);actor_id=actor_events[-1]['entity']
        # Force opposite then selected input so both normal/sneak require fresh proof.
        client.send('player_input',bytes([0 if case['sneak'] else 32]));pump(.3)
        mode_start=time.monotonic();client.send('player_input',bytes([32 if case['sneak'] else 0]));pump(.5)
        mode,evidence=metadata_sneak(observer.raw,actor_id,mode_start)
        case['sneak_proof']=dict(expected=case['sneak'],values=mode,evidence=evidence)
        if bool(mode.get('flags',0)&2)!=case['sneak'] or mode.get('pose')!=(5 if case['sneak'] else 0) or 'flags' not in mode:
            raise RuntimeError('INCONCLUSIVE selected sneak/standing metadata not proved')
        command('setblock 3 64 4 stone');command('setblock 3 65 4 sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]')
        pump(3.5)
        block_is(sensor,'sculk_sensor[power=0,sculk_sensor_phase=inactive]')
        for floor_x in [0,1,2]:
            block_is((floor_x,case['source_y'],0),case['surface'])
            if case['source_y']==65:block_is((floor_x,64,0),'stone')
        case['before']=pose()
        expected_start_y=feet
        if any(abs(a-b)>.03 for a,b in zip(case['before']['position'],[.5,expected_start_y,.5])):
            raise RuntimeError('INCONCLUSIVE unexpected server initial pose')
        case['start']=time.monotonic();start=case['start']
        if not case['before']['ground']:
            raise RuntimeError('INCONCLUSIVE grounded start missing')
        if case['action']=='step':
            if not case['before']['ground']:raise RuntimeError('INCONCLUSIVE step start not server grounded')
            for n in range(1,61):packet(.5+n*.04,feet,.5,True)
        else:pump(1.25)
        pump(.3);case['after']=pose();pump(3.)
        events=observe_actor(observer.raw,player_type)
        selected=[x for x in events if x['at']>=start]
        case['observer_path']=selected
        corrections=[x for x in client.raw if x['direction']=='in' and x['name']=='position' and x['at']>=start]
        case['corrections']=corrections
        active=[]
        for row in client.raw:
            if row['direction']=='in' and row['at']>=start and row['name'] in ('block_change','multi_block_change'):
                for pos,value in changes(row['name'],bytes.fromhex(row['hex'])):
                    if tuple(pos)==sensor and value in sensor_states and sensor_states[value]['sculk_sensor_phase']=='active':
                        assert int(sensor_states[value]['power'])>0
                        active.append(dict(row,state=value,properties=sensor_states[value]))
        case['activations']=active
        end=case['after'];expected_x=2.9 if case['action']=='step' else .5
        if corrections or abs(end['position'][0]-expected_x)>.03 or abs(end['position'][1]-feet)>.03 or not end['ground']:
            case['reason']='server pose/ground/correction proof failed';continue
        if case['action']!='noaction' and len(selected)<2:
            case['reason']='missing independent intermediate observer movement';continue
        if any(abs(point['position'][1]-feet)>.03 or abs(point['position'][2]-.5)>.03 for point in selected):
            case['reason']='observer path leaves flat selected surface';continue
        if selected and any(abs(a-b)>.04 for a,b in zip(selected[-1]['position'],end['position'])):
            case['reason']='observer endpoint differs from server query';continue
        if case['action']=='noaction' and any(abs(x['position'][0]-.5)>.03 or abs(x['position'][1]-feet)>.03 for x in selected):
            case['reason']='noaction observer moved';continue
        if case['action']=='step' and max(x['position'][0] for x in selected)-min(x['position'][0] for x in selected)<1:
            case['reason']='observer path too short';continue
        final_mode,final_evidence=metadata_sneak(observer.raw,actor_id,mode_start)
        case['sneak_proof']['final_values']=final_mode
        if any(bool(e['selected'].get('flags',32 if case['sneak'] else 0)&2)!=case['sneak'] for e in final_evidence if 'flags' in e['selected']):
            case['reason']='sneak metadata changed in measurement';continue
        if any(e['selected']['pose']!=(5 if case['sneak'] else 0) for e in final_evidence if 'pose' in e['selected']):
            case['reason']='pose metadata changed in measurement';continue
        case['status']='OBSERVED';case['reason']='accepted selected path; activation recorded, no general parity claim'
    positives=[x for x in report['cases'] if x['label'] in ('stone-normal-step','stone-normal-step-end')]
    negatives=[x for x in report['cases'] if x['action']=='noaction']
    report['controls_valid']=all(x['status']=='OBSERVED' and x.get('activations') for x in positives) and all(x['status']=='OBSERVED' and not x.get('activations') for x in negatives)
    if not report['controls_valid']:
        for case in report['cases']:
            case['status']='INCONCLUSIVE';case['reason']='stone/noaction controls not validated'

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--run', action='store_true')
    ap.add_argument('--jar', type=Path)
    ap.add_argument('--data', type=Path)
    ap.add_argument('--output', type=Path)
    args = ap.parse_args()
    report = dict(scope='dry white step-only6surfaces19cases; no landing/wet/fullfamily claim', protocol=777,
      port=26051, jar_sha1=JAR_SHA1, pins=HASHES, cases=[dict(x, status='not-run') for x in CASES],
      implemented=['pinned preflight', 'isolated server lifecycle', 'raw transport capture', 'strict server Pos/OnGround query', 'independent777observer', 'fresh selected sneak/standing metadata', 'flat path +source ACKs x0/1/2', 'bookend stone positives +6noaction controls'],
      missing=['ordinary validation of candidate walk/fall and collision sanity; no full physical client'],
      limits=['step-only6surfaces19cases; actual sneak metadata and accepted independent path required', 'no hold/ground=1 synthesis', 'no landing, no wet, no all16family claim'])
    report['runner_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    report['helper_sha256'] = {n: hashlib.sha256((HELPERS/n).read_bytes()).hexdigest() for n in ['probe_gameplay.py', 'smoke.py', 'observe.py', 'probe_white_concrete_slab.py', 'generate_tables.py']}
    if not args.run:
        print(json.dumps(dict(report, mode='plan-only'), indent=2))
        return
    if args.output is None or args.jar is None or args.data is None:
        ap.error('--jar, --data and --output required with --run')
    assert hashlib.sha1(args.jar.read_bytes()).hexdigest() == JAR_SHA1, 'jar pin'
    for name, digest in HASHES.items():
        assert hashlib.sha256((args.data/name).read_bytes()).hexdigest() == digest, name
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    world = out/'world'
    world.mkdir()
    (out/'runner.py').write_bytes(Path(__file__).read_bytes())
    (world/'eula.txt').write_text('eula=true\n')
    (world/'server.properties').write_text('server-ip=127.0.0.1\nserver-port=26051\nwhite-list=false\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=3\nsimulation-distance=3\ngamemode=survival\nspawn-protection=0\n')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 26051))
    deadline = time.monotonic()+420
    server = client = observer = None
    log = out/'server.log'
    report['commands'] = []
    try:
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(args.jar.resolve()), 'nogui'], cwd=world, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True, start_new_session=True)
        while 'Done (' not in log.read_text():
            assert server.poll() is None and time.monotonic()<deadline, 'startup bound'
            time.sleep(.1)
        protocol = json.loads((args.data/'protocol.json').read_text())
        client = connect(26051, protocol, RawClient, 777)
        client.pump(10, lambda x: x['name']=='position')
        assert client.position is not None, 'no initial server position'
        def query_field(field):
            start = len(log.read_text())
            command = 'data get entity McsaProbe '+field
            report['commands'].append(dict(text=command, at=time.monotonic()))
            server.stdin.write(command+'\n'); server.stdin.flush()
            field_deadline = min(deadline, time.monotonic()+5)
            while time.monotonic()<field_deadline:
                client.pump(.1)
                found = re.search(r'McsaProbe has the following entity data: ([^\r\n]+)', log.read_text()[start:])
                if found:
                    evidence=dict(field=field,raw=found[1],parsed=parse_entity_data(field,found[1]),at=time.monotonic())
                    report.setdefault('server_fields_history',[]).append(evidence)
                    return evidence['parsed']
            raise RuntimeError('INCONCLUSIVE server field unavailable: '+field)
        def command(text):
            assert time.monotonic()<deadline
            start=len(log.read_text());report['commands'].append(dict(text=text,at=time.monotonic()))
            server.stdin.write(text+'\n');server.stdin.flush();client.pump(.2)
            if ' run say MCSA_MOVEMENT_QUERY_' in text:
                token=text.rsplit(' ',1)[-1]
                until=min(deadline,time.monotonic()+3)
                ack='[Server] '+token
                while ack not in log.read_text()[start:] and time.monotonic()<until:client.pump(.1)
                if ack not in log.read_text()[start:]:raise RuntimeError('query acknowledgement missing: '+token)
                report['commands'][-1]['ack']=ack
        observer=observer_connect(protocol);observer.pump(5,lambda x:x['name']=='position')
        calibrate(client,observer,protocol,json.loads((args.data/'blocks.json').read_text()),json.loads((args.data/'entities.json').read_text()),command,query_field,report,deadline)
    except Exception as error:
        report['error'] = repr(error)
        report['traceback']=traceback.format_exc()
    finally:
        if observer:
            report['observer_raw']=observer.raw;observer.sock.close()
        if client:
            report['raw'] = client.raw
            client.sock.close()
        if server:
            def group_exists():
                try:
                    os.killpg(server.pid,0);return True
                except ProcessLookupError:
                    return False
            if server.poll() is None:
                try:
                    server.stdin.write('stop\n');server.stdin.flush();server.wait(timeout=15)
                except (OSError,subprocess.TimeoutExpired):
                    pass
            for sig,timeout in [(signal.SIGTERM,5),(signal.SIGKILL,5)]:
                if not group_exists():break
                try:os.killpg(server.pid,sig)
                except ProcessLookupError:pass
                until=time.monotonic()+timeout
                while time.monotonic()<until:
                    server.poll()
                    if not group_exists():break
                    time.sleep(.05)
            server.poll()
            with socket.socket() as check:
                try:
                    check.bind(('127.0.0.1',26051));port_free=True
                except OSError:
                    port_free=False
            report['cleanup']=dict(pid=server.pid,exit=server.returncode,group_gone=not group_exists(),port_free=port_free)
            report['cleanup']['clean']=server.returncode==0 and report['cleanup']['group_gone'] and port_free
        for case in report['cases']:
            if case['status']=='not-run':case.update(status='INCONCLUSIVE',reason=report.get('error','not reached'))
        (out/'result.json').write_text(json.dumps(report, indent=2)+'\n')
    calibrated=bool(report.get('controls_valid')) and all(c['status']=='OBSERVED' for c in report['cases']) and report.get('cleanup',{}).get('clean',False) and 'error' not in report
    report['status']='CALIBRATED' if calibrated else 'INCONCLUSIVE'
    (out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(output=str(out),status=report['status'],cleanup=report.get('cleanup'))))
    return 0 if calibrated else 2


if __name__ == '__main__':
    sys.exit(main())
