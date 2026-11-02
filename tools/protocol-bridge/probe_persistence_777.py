#!/usr/bin/env python3
"""Ограниченная свежая проверка входа XP и сохранения эффектов Java777."""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
from probe_gameplay import Client, connect
from smoke import read_varint


class Probe(Client):
    def __init__(self, *args):
        super().__init__(*args)
        self.wire = []
        self.xp = None
        self.effects = {}
        self.health = None
        self.absorption = None
        self.entity_id = None

    def send(self, name, payload=b''):
        self.wire.append({'direction':'out', 'name':name, 'payload':payload.hex(), 'time':time.monotonic()})
        super().send(name, payload)

    def observe_packet(self, name, payload):
        self.wire.append({'direction':'in', 'name':name, 'payload':payload.hex(), 'time':time.monotonic()})
        data = io.BytesIO(payload)
        if name == 'login':
            self.entity_id = struct.unpack('>i',data.read(4))[0]
        elif name == 'entity_metadata':
            if read_varint(data.read) != self.entity_id:
                return
            entity, fields = player_metadata(payload)
            if entity == self.entity_id and 17 in fields:
                self.absorption = fields[17]
        elif name == 'experience':
            self.xp = {'progress':struct.unpack('>f', data.read(4))[0],
                       'level':read_varint(data.read), 'total':read_varint(data.read)}
            assert not data.read()
        elif name == 'entity_effect':
            entity, kind, amplifier, duration = [read_varint(data.read) for _ in range(4)]
            if getattr(self,'entity_id',None) is not None and entity != self.entity_id:
                return
            flags = data.read(1)
            assert len(flags) == 1 and not data.read(), 'unexpected effect layout'
            duration = duration - 2**32 if duration >= 2**31 else duration
            self.effects[str(kind)] = {'amplifier':amplifier, 'duration':duration, 'flags':flags[0]}
        elif name == 'remove_entity_effect':
            entity=read_varint(data.read)
            if getattr(self,'entity_id',None) is not None and entity != self.entity_id:
                return
            self.effects.pop(str(read_varint(data.read)), None)
            assert not data.read()
        elif name == 'update_health':
            self.health = {'health':struct.unpack('>f',data.read(4))[0],
                           'food':read_varint(data.read), 'saturation':struct.unpack('>f',data.read(4))[0]}
            assert not data.read()


def player_metadata(payload):
    """Наблюдённые простые свойства собственного игрока; неизвестное — отказ."""
    data = io.BytesIO(payload)
    entity = read_varint(data.read)
    fields = {}
    while True:
        raw = data.read(1)
        assert raw, 'truncated player metadata'
        index = raw[0]
        if index == 255:
            break
        assert index not in fields, 'duplicate metadata index'
        kind = read_varint(data.read)
        if kind == 0:
            value = struct.unpack('>b',data.read(1))[0]
        elif kind in (1,19,20,23,24):
            value = read_varint(data.read)
        elif kind == 3:
            value = struct.unpack('>f',data.read(4))[0]
            assert math.isfinite(value)
        elif kind == 8:
            value = data.read(1)
            assert value in (b'\0',b'\1')
            value = bool(value[0])
        elif kind == 17:
            value=[]
            for _ in range(read_varint(data.read)):
                # Official41456459340: entity_effect particle28 followed by RGBA.
                particle=read_varint(data.read)
                assert particle == 28, ('unexpected authored effect particle',particle)
                color=data.read(4)
                assert len(color)==4
                value.append(color.hex())
        else:
            raise AssertionError(('unexpected player metadata serializer',index,kind))
        if index == 17:
            assert kind == 3, 'absorption must be float'
        fields[index]=value
    assert not data.read(), 'trailing metadata'
    return entity, fields


def snapshot(client, stage, session):
    receipts = {}
    for index, packet in enumerate(client.wire):
        if packet['direction']=='in' and packet['name']=='entity_effect':
            data=io.BytesIO(bytes.fromhex(packet['payload']))
            entity=read_varint(data.read)
            if getattr(client,'entity_id',None) is not None and entity != client.entity_id:
                continue
            receipts[str(read_varint(data.read))]={'time':packet['time'],'packet_index':index}
    return {'stage':stage, 'session':session, 'packet_index':len(client.wire), 'time':time.monotonic(),
            'xp':client.xp, 'xp_packets':sum(r['direction']=='in' and r['name']=='experience' for r in client.wire),
            'effects':json.loads(json.dumps(client.effects)), 'effect_receipts':receipts,
            'health':client.health, 'absorption':getattr(client,'absorption',None)}


STAGES = ['fresh-login', 'zero-bar-positive-total-before', 'zero-bar-positive-total-login',
          'nonzero-before', 'nonzero-login', 'poison-before', 'poison-relogin',
          'effects-before', 'effects-relogin', 'effects-restart', 'hidden-promoted']
LOGIN_STAGES = ['fresh-login','zero-bar-positive-total-login','nonzero-login',
                'poison-relogin','effects-relogin','effects-restart']


def compare(expected, actual):
    failures=[]
    left_stages=[s['stage'] for s in expected['snapshots']]
    right_stages=[s['stage'] for s in actual['snapshots']]
    if left_stages != right_stages or len(set(left_stages)) != len(left_stages) or len(set(right_stages)) != len(right_stages):
        failures.append('snapshot stages differ, are reordered or duplicated')
    left={s['stage']:s for s in expected['snapshots']}
    right={s['stage']:s for s in actual['snapshots']}
    for stage in left.keys() & right.keys():
        a,b=left[stage],right[stage]
        if a['xp'] != b['xp']:
            failures.append(stage+': exact XP differs')
        if stage in LOGIN_STAGES and (a['xp_packets'] != 1 or b['xp_packets'] != 1):
            failures.append(stage+': initial XP packet count differs from one')
        if a.get('absorption') != b.get('absorption') and stage in ('effects-before','effects-relogin','effects-restart','hidden-promoted'):
            failures.append(stage+': exact absorption differs')
        if set(a['effects']) != set(b['effects']):
            failures.append(stage+': effect kinds differ')
        for kind in a['effects'].keys() & b['effects'].keys():
            x,y=a['effects'][kind],b['effects'][kind]
            # Duration is bounded against each run's measured online intervals below.
            if x['amplifier'] != y['amplifier'] or (x['duration']==-1) != (y['duration']==-1):
                failures.append(stage+': effect amplifier/infinite differs '+kind)
            mask=255 if stage in LOGIN_STAGES else 7
            if x['flags'] & mask != y['flags'] & mask:
                failures.append(stage+': effect flags differ '+kind)
    return failures


def online_seconds(report, start, end):
    assert start <= end
    return sum(max(0,min(end,s.get('closed_at',end))-max(start,s['connect_started']))
               for s in report['sessions'])


def duration_failure(report, before, after, kind):
    old,new=before['effects'].get(kind),after['effects'].get(kind)
    if old is None or new is None:
        return True
    if old['duration']==-1:
        return new['duration'] != -1
    if new['duration'] <= 0 or new['duration'] > old['duration']:
        return True
    start=before['effect_receipts'][kind]['time']
    end=after['effect_receipts'][kind]['time']
    # Packet receipt, not snapshot time. Six-second offline intervals are excluded.
    allowance=math.ceil(online_seconds(report,start,end)*20)+20
    return old['duration']-new['duration'] > allowance


def behavior_checks(report):
    failures=[]
    stages={s['stage']:s for s in report['snapshots']}
    if [s['stage'] for s in report['snapshots']] != STAGES:
        return ['missing canonical stages']
    for stage in LOGIN_STAGES:
        if stages[stage]['xp_packets'] != 1:
            failures.append(stage+': expected exactly one initial XP packet')
    if stages['fresh-login']['xp'] != {'progress':0.0,'level':0,'total':0}:
        failures.append('fresh login must contain exact XP0')
    if stages['zero-bar-positive-total-login']['xp'] != {'progress':0.0,'level':0,'total':7}:
        failures.append('zero-bar login must contain exact total7')
    for before,after in (('zero-bar-positive-total-before','zero-bar-positive-total-login'),('nonzero-before','nonzero-login')):
        if stages[before]['xp'] != stages[after]['xp']:
            failures.append(after+': XP changed across disconnect')
    for before,after in (('poison-before','poison-relogin'),('effects-before','effects-relogin'),('effects-relogin','effects-restart')):
        x,y=stages[before],stages[after]
        if set(x['effects']) != set(y['effects']):
            failures.append(after+': active effect kinds changed across disconnect')
        for kind in x['effects']:
            old,new=x['effects'][kind],y['effects'].get(kind,{})
            if old['amplifier'] != new.get('amplifier') or old['flags']&7 != new.get('flags',-1)&7:
                failures.append(after+': effect amplifier/semantic flags changed '+kind)
            if duration_failure(report,x,y,kind):
                failures.append(after+': remaining duration outside measured online bound '+kind)
    for stage in ('poison-before','poison-relogin'):
        if set(stages[stage]['effects']) != {'0','18'}:
            failures.append(stage+': active Speed+Poison missing')
    for stage in ('effects-before','effects-relogin','effects-restart','hidden-promoted'):
        x=stages[stage]
        if set(x['effects']) != {'0','15','21'}:
            failures.append(stage+': benign Speed/NightVision/Absorption missing')
        if x['absorption'] != 2.0:
            failures.append(stage+': actual own-player metadata absorption is not exactly2')
        if x['effects'].get('15',{}).get('duration') != -1:
            failures.append(stage+': infinite NightVision missing')
    for stage in ('effects-before','effects-relogin','effects-restart'):
        if stages[stage]['effects'].get('0',{}).get('amplifier') != 1:
            failures.append(stage+': strong Speed did not survive')
    promoted=stages['hidden-promoted']['effects'].get('0',{})
    # Weak120s, strong30s. At the actual promotion packet weak duration is <=1800,
    # and >=1700 allows command/transport delay but rejects a fresh full120s grant.
    if promoted.get('amplifier') != 0 or promoted.get('flags',-1)&7 != 0 or not 1700 <= promoted.get('duration',-1) <= 1800:
        failures.append('hidden Speed was not promoted with remaining weak duration1700..1800')
    return failures


def evidence_failures(report, official=False):
    failures=[]
    if report.get('protocol') != 777 or report.get('error'):
        failures.append('invalid protocol or report error')
    if report.get('server_exits') != [0,0]:
        failures.append('both real server exits must be zero')
    if official and (report.get('target') != 'official' or report.get('behavior_failures') or report.get('status') not in ('OBSERVATION','PASS')):
        failures.append('official evidence is not clean')
    if not official and (report.get('target') != 'core' or report.get('observe_only') is not False):
        failures.append('core evidence must use strict mode')
    snapshots=report.get('snapshots',[])
    if [s.get('stage') for s in snapshots] != STAGES:
        failures.append('missing, reordered or duplicate canonical snapshot stages')
    sessions=report.get('sessions',[])
    by_name={v['name']:v for v in sessions}
    if len(by_name) != len(sessions):
        failures.append('duplicate sessions')
    for current in sessions:
        if not (isinstance(current.get('connect_started'),(int,float)) and isinstance(current.get('closed_at'),(int,float))
                and math.isfinite(current['connect_started']) and math.isfinite(current['closed_at'])
                and current['connect_started'] <= current['closed_at']):
            failures.append('invalid measured session interval')
    for value in snapshots:
        packets=by_name.get(value.get('session'),{}).get('packets',[])
        index=value.get('packet_index',-1)
        if not isinstance(index,int) or not 0 < index <= len(packets):
            failures.append(str(value.get('stage'))+': invalid raw packet boundary')
            continue
        replay=object.__new__(Probe)
        replay.wire=[];replay.xp=None;replay.effects={};replay.health=None;replay.absorption=None;replay.entity_id=None
        try:
            for packet in packets[:index]:
                assert packet['direction'] in ('in','out')
                payload=bytes.fromhex(packet['payload'])
                assert math.isfinite(packet['time'])
                if packet['direction']=='in':
                    replay.observe_packet(packet['name'],payload)
                    replay.wire[-1]['time']=packet['time']
                else:
                    replay.wire.append(dict(packet))
            rebuilt=snapshot(replay,value['stage'],value['session'])
            for field in ('xp','xp_packets','effects','effect_receipts','health','absorption'):
                if rebuilt[field] != value[field]:
                    failures.append(value['stage']+': raw replay differs for '+field)
            incoming={p['name'] for p in packets[:index] if p['direction']=='in'}
            if not {'login','position','experience','update_health'} <= incoming:
                failures.append(value['stage']+': raw login/XP/health packets missing')
            if value['stage'].startswith(('effects-','poison-')) or value['stage']=='hidden-promoted':
                if not value['effects'] or 'entity_effect' not in incoming:
                    failures.append(value['stage']+': raw active effect packets missing')
        except (ValueError,KeyError,AssertionError,struct.error,EOFError) as error:
            failures.append(value['stage']+': raw replay error '+repr(error))
    return failures


def main():
    p = argparse.ArgumentParser(description=__doc__)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar',type=Path)
    target.add_argument('--binary',type=Path)
    p.add_argument('--java',default='/usr/lib/jvm/java-25-openjdk/bin/java')
    p.add_argument('--data',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--port',type=int,default=26156)
    p.add_argument('--observe-only',action='store_true')
    p.add_argument('--expected',type=Path)
    a = p.parse_args()
    assert 26000 <= a.port <= 26299
    assert a.jar or a.expected, 'core requires fresh official expected report'
    binary = (a.jar or a.binary).resolve()
    if a.jar:
        assert hashlib.sha1(binary.read_bytes()).hexdigest() == '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
    manifest = json.loads((a.data/'sources.json').read_text())
    for entry in manifest['files'] + manifest.get('baseline_files',[]):
        assert hashlib.sha256((a.data/entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data/'protocol.json').read_text())
    with socket.socket() as check:
        check.bind(('127.0.0.1',a.port))
    a.output.mkdir(parents=True,exist_ok=True)
    report = {'protocol':777,'target':'official' if a.jar else 'core',
              'target_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
              'observe_only':a.observe_only,'status':'OBSERVATION' if a.observe_only else 'INCOMPLETE',
              'snapshots':[],'sessions':[],'command_events':[],'behavior_failures':[], 'server_exits':[],
              'limits':['No Bedrock or cross-adapter claim', 'No exact periodic damage phase assertion',
                        'Finite durations bounded by measured online time plus20ticks; player effects tick while world frozen']}
    client = None
    server = None
    session = None
    log_handle = None
    with tempfile.TemporaryDirectory(prefix='mcsa-persistence-777-') as directory:
        root = Path(directory)
        (root/'eula.txt').write_text('eula=true\n')
        properties = f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
        (root/'server.properties').write_text(properties)
        if a.binary:
            (root/'config').mkdir()
            (root/'config/server.properties').write_text(properties)
            (root/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
            (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
        def command(value):
            report['command_events'].append({'command':value,'session':session,
                                            'time':time.monotonic(),
                                            'packet_index':len(client.wire) if client else None,
                                            'server_log_offset':log_handle.tell() if log_handle else None})
            server.stdin.write(value+'\n'); server.stdin.flush()
            if client:
                client.pump(.25)
            else:
                time.sleep(.25)
        def start(index):
            nonlocal server, log_handle
            path = a.output/f'server-{index}.log'
            log_handle = path.open('w')
            argv = [a.java,'-Xmx1G','-jar',str(binary),'nogui'] if a.jar else [str(binary)]
            server = subprocess.Popen(argv,cwd=root,stdin=subprocess.PIPE,stdout=log_handle,stderr=subprocess.STDOUT,text=True)
            deadline = time.monotonic()+120
            while ('Done (' if a.jar else 'Готово (') not in path.read_text():
                assert server.poll() is None and time.monotonic()<deadline, 'server readiness failed'
                time.sleep(.1)
        def stop():
            if server and server.poll() is None:
                command('stop'); server.stdin.close()
                try:
                    server.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    server.kill(); server.wait()
            if server:
                report['server_exits'].append(server.returncode)
            if log_handle:
                log_handle.close()
        def join(label):
            nonlocal client, session
            session = label
            started=time.monotonic()
            client = connect(a.port,protocol,Probe,777)
            report['sessions'].append({'name':label,'connect_started':started,'packets':client.wire})
            client.pump(10,lambda r:r['name']=='position');client.pump(1)
            assert client.position is not None, 'no real join position'
        def snap(label):
            report['snapshots'].append(snapshot(client,label,session))
        def leave():
            nonlocal client
            report['sessions'][-1]['closed_at']=time.monotonic()
            client.sock.shutdown(socket.SHUT_RDWR);client.sock.close();client=None
            # Frozen НЕ останавливает эффекты игрока: доказанный official факт.
            # Длительный offline отдельно исключён из допустимого расхода ticks.
            time.sleep(6)
            command('save-all flush')
        try:
            start(0)
            command('gamerule minecraft:spawn_mobs false')
            command('tick freeze')
            join('fresh'); snap('fresh-login')
            command('experience add McsaProbe 7 points')
            command('experience set McsaProbe 0 levels')
            command('experience set McsaProbe 0 points')
            snap('zero-bar-positive-total-before')
            leave();join('zero-bar-positive-total');snap('zero-bar-positive-total-login')
            command('experience set McsaProbe 17 levels')
            command('experience set McsaProbe 3 points')
            command('experience add McsaProbe 7 points')
            snap('nonzero-before')
            leave();join('nonzero');snap('nonzero-login')
            command('fill -2 63 -2 2 63 2 stone')
            command('tp McsaProbe 0.5 64 0.5')
            command('gamemode survival McsaProbe')
            command('effect clear McsaProbe')
            command('effect give McsaProbe speed 120 0 true')
            command('effect give McsaProbe poison 120 0 false')
            snap('poison-before')
            leave();join('poison-relogin');snap('poison-relogin')
            # Poison отдельно: он расходует absorption даже при tick freeze.
            command('effect clear McsaProbe')
            command('effect give McsaProbe speed 120 0 true')
            command('effect give McsaProbe speed 30 1 false')
            command('effect give McsaProbe absorption 120 0 false')
            command('effect give McsaProbe night_vision infinite 0 true')
            command('damage McsaProbe 2 minecraft:generic')
            snap('effects-before')
            leave();join('effects-relogin');snap('effects-relogin')
            leave();stop();start(1)
            command('tick freeze')
            join('effects-restart');snap('effects-restart')
            remaining=client.effects.get('0',{}).get('duration',600)
            # Реальная смена amp1→amp0 в пакете, а не ожидание заранее выбранного sleep.
            client.pump(max(1,remaining/20)+8,
                        lambda r:client.effects.get('0',{}).get('amplifier')==0)
            snap('hidden-promoted')
            report['behavior_failures'].extend(behavior_checks(report))
            if a.expected:
                expected = json.loads(a.expected.read_text())
                assert not evidence_failures(expected, official=True), evidence_failures(expected, official=True)
                report['expected_sha256']=hashlib.sha256(a.expected.read_bytes()).hexdigest()
                report['behavior_failures'].extend(compare(expected,report))
            report['status'] = 'OBSERVATION' if a.observe_only else 'FAIL' if report['behavior_failures'] else 'PASS'
        except BaseException as error:
            report['error']=repr(error)
            report['status']='ERROR'
            raise
        finally:
            if client:
                report['sessions'][-1]['closed_at']=time.monotonic()
                client.sock.close();client=None
            stop()
            if any(value != 0 for value in report['server_exits']):
                report['status']='ERROR'
                report['error']='nonzero server exit'
            (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    if report['status'] != 'ERROR':
        report['behavior_failures'].extend(evidence_failures(report,official=bool(a.jar)))
        if report['behavior_failures'] and not a.observe_only:
            report['status']='FAIL'
        (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['status'] not in ('FAIL','ERROR'), report['behavior_failures']
    print(json.dumps({'status':report['status'],'behavior_failures':report['behavior_failures']}))


if __name__=='__main__':
    main()
