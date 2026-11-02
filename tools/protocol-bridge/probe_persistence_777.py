#!/usr/bin/env python3
"""Ограниченная свежая проверка входа XP и сохранения эффектов Java777."""
import argparse
import hashlib
import io
import json
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

    def send(self, name, payload=b''):
        self.wire.append({'direction':'out', 'name':name, 'payload':payload.hex()})
        super().send(name, payload)

    def observe_packet(self, name, payload):
        self.wire.append({'direction':'in', 'name':name, 'payload':payload.hex()})
        data = io.BytesIO(payload)
        if name == 'experience':
            self.xp = {'progress':struct.unpack('>f', data.read(4))[0],
                       'level':read_varint(data.read), 'total':read_varint(data.read)}
            assert not data.read()
        elif name == 'entity_effect':
            entity, kind, amplifier, duration = [read_varint(data.read) for _ in range(4)]
            flags = data.read(1)
            assert len(flags) == 1 and not data.read(), 'unexpected effect layout'
            duration = duration - 2**32 if duration >= 2**31 else duration
            self.effects[str(kind)] = {'amplifier':amplifier, 'duration':duration, 'flags':flags[0]}
        elif name == 'remove_entity_effect':
            read_varint(data.read)
            self.effects.pop(str(read_varint(data.read)), None)
            assert not data.read()
        elif name == 'update_health':
            self.health = {'health':struct.unpack('>f',data.read(4))[0],
                           'food':read_varint(data.read), 'saturation':struct.unpack('>f',data.read(4))[0]}
            assert not data.read()


def snapshot(client, stage, session):
    return {'stage':stage, 'session':session, 'packet_index':len(client.wire),
            'xp':client.xp, 'xp_packets':sum(r['direction']=='in' and r['name']=='experience' for r in client.wire),
            'effects':json.loads(json.dumps(client.effects)), 'health':client.health}


def compare(expected, actual):
    failures = []
    left = {s['stage']:s for s in expected['snapshots']}
    right = {s['stage']:s for s in actual['snapshots']}
    left_stages = [s['stage'] for s in expected['snapshots']]
    right_stages = [s['stage'] for s in actual['snapshots']]
    if left_stages != right_stages or len(set(left_stages)) != len(left_stages) or len(set(right_stages)) != len(right_stages):
        failures.append('snapshot stages differ, are reordered or duplicated')
    for stage in left.keys() & right.keys():
        for field in ('xp', 'xp_packets', 'effects', 'health'):
            if left[stage][field] != right[stage][field]:
                failures.append(f'{stage}: {field}: expected {left[stage][field]!r}, actual {right[stage][field]!r}')
    return failures


STAGES = ['fresh-login', 'zero-bar-positive-total-before', 'zero-bar-positive-total-login',
          'nonzero-before', 'nonzero-login', 'effects-before', 'effects-relogin',
          'effects-restart', 'hidden-promoted']


def evidence_failures(report, official=False):
    failures = []
    if report.get('protocol') != 777 or report.get('error'):
        failures.append('invalid protocol or report error')
    if report.get('server_exits') != [0, 0]:
        failures.append('both real server exits must be zero')
    if official and (report.get('target') != 'official' or report.get('behavior_failures')
                     or report.get('status') not in ('OBSERVATION', 'PASS')):
        failures.append('official evidence is not clean')
    if not official and (report.get('target') != 'core' or report.get('observe_only') is not False):
        failures.append('core evidence must use strict mode')
    snapshots = report.get('snapshots', [])
    if [s.get('stage') for s in snapshots] != STAGES:
        failures.append('missing, reordered or duplicate canonical snapshot stages')
    sessions = report.get('sessions', [])
    by_name = {value['name']:value for value in sessions}
    if len(by_name) != len(sessions):
        failures.append('duplicate sessions')
    for value in snapshots:
        packets = by_name.get(value.get('session'), {}).get('packets', [])
        index = value.get('packet_index', -1)
        if not isinstance(index, int) or not 0 < index <= len(packets):
            failures.append(str(value.get('stage'))+': invalid raw packet boundary')
            continue
        replay = object.__new__(Probe)
        replay.wire=[]; replay.xp=None; replay.effects={}; replay.health=None
        try:
            for packet in packets[:index]:
                assert packet['direction'] in ('in','out')
                payload = bytes.fromhex(packet['payload'])
                if packet['direction'] == 'in':
                    replay.observe_packet(packet['name'],payload)
            rebuilt = snapshot(replay,value['stage'],value['session'])
            for field in ('xp','xp_packets','effects','health'):
                if rebuilt[field] != value[field]:
                    failures.append(value['stage']+': raw replay differs for '+field)
            incoming = {p['name'] for p in packets[:index] if p['direction']=='in'}
            if not {'login','position','experience','update_health'} <= incoming:
                failures.append(value['stage']+': raw login/XP/health packets missing')
            if value['stage'].startswith('effects-') or value['stage']=='hidden-promoted':
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
                        'AbsorptionAmount console replies retained in server logs; reviewed separately']}
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
            client = connect(a.port,protocol,Probe,777)
            report['sessions'].append({'name':label,'packets':client.wire})
            client.pump(10,lambda r:r['name']=='position');client.pump(1)
            assert client.position is not None, 'no real join position'
        def snap(label):
            report['snapshots'].append(snapshot(client,label,session))
        def leave():
            nonlocal client
            client.sock.shutdown(socket.SHUT_RDWR);client.sock.close();client=None
            # Офлайн-интервал заметно длиннее одного тика; frozen сохраняет точную длительность.
            time.sleep(1)
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
            command('effect give McsaProbe speed 30 1 false')
            command('effect give McsaProbe poison 120 0 false')
            command('effect give McsaProbe absorption 120 0 false')
            command('effect give McsaProbe night_vision infinite 0 true')
            command('damage McsaProbe 2 minecraft:generic')
            command('data get entity McsaProbe AbsorptionAmount')
            snap('effects-before')
            leave();join('effects-relogin');snap('effects-relogin')
            command('data get entity McsaProbe AbsorptionAmount')
            leave();stop();start(1)
            command('tick freeze')
            join('effects-restart');snap('effects-restart')
            command('data get entity McsaProbe AbsorptionAmount')
            command('tick step 601')
            client.pump(34)
            snap('hidden-promoted')
            command('data get entity McsaProbe AbsorptionAmount')
            stages = {s['stage']:s for s in report['snapshots']}
            for before, after in (('zero-bar-positive-total-before','zero-bar-positive-total-login'),
                                  ('nonzero-before','nonzero-login')):
                if stages[before]['xp'] != stages[after]['xp']:
                    report['behavior_failures'].append(after+': XP changed across disconnect')
            for after in ('effects-relogin','effects-restart'):
                if stages['effects-before']['effects'] != stages[after]['effects']:
                    report['behavior_failures'].append(after+': effects changed while offline/frozen')
            if not stages['effects-before']['effects']:
                report['behavior_failures'].append('effect setup returned no effect packets')
            if a.expected:
                expected = json.loads(a.expected.read_text())
                assert not evidence_failures(expected, official=True), evidence_failures(expected, official=True)
                report['expected_sha256']=hashlib.sha256(a.expected.read_bytes()).hexdigest()
                report['behavior_failures'].extend(compare(expected,report))
            for stage in ('fresh-login','zero-bar-positive-total-login','nonzero-login','effects-relogin','effects-restart'):
                value = next(s for s in report['snapshots'] if s['stage']==stage)
                if value['xp_packets'] != 1:
                    report['behavior_failures'].append(stage+': expected exactly one initial XP packet')
            report['status'] = 'OBSERVATION' if a.observe_only else 'FAIL' if report['behavior_failures'] else 'PASS'
        except BaseException as error:
            report['error']=repr(error)
            report['status']='ERROR'
            raise
        finally:
            if client:
                client.sock.close();client=None
            stop()
            if any(value != 0 for value in report['server_exits']):
                report['status']='ERROR'
                report['error']='nonzero server exit'
            (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    if a.binary and report['status'] not in ('ERROR',):
        report['behavior_failures'].extend(evidence_failures(report))
        if report['behavior_failures']:
            report['status']='FAIL'
        (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['status'] not in ('FAIL','ERROR'), report['behavior_failures']
    print(json.dumps({'status':report['status'],'behavior_failures':report['behavior_failures']}))


if __name__=='__main__':
    main()
