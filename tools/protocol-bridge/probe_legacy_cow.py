#!/usr/bin/env python3
"""Собственное наблюдение cow340; metadata сохраняется без догадок об индексах."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_legacy_session import Probe
from smoke import handshake, read_varint, send


class CowProbe(Probe):
    def __init__(self, sock, protocol, report):
        super().__init__(sock, protocol, report)
        self.stage = 'join'
        self.events = []

    def receive(self):
        record = super().receive()
        record['stage'] = self.stage
        return record

    def observe_packet(self, name, payload):
        wanted = {
            'spawn_entity_living', 'entity_metadata', 'entity_update_attributes',
            'entity_velocity', 'entity_teleport', 'rel_entity_move',
            'entity_move_look', 'entity_look', 'entity_head_rotation',
            'entity_destroy', 'entity_status', 'animation', 'entity_equipment',
            'named_sound_effect', 'sound_effect',
        }
        if name not in wanted:
            return
        event = {'stage': self.stage, 'name': name, 'payload': payload.hex()}
        data = io.BytesIO(payload)
        if name == 'spawn_entity_living':
            event['entity'] = read_varint(data.read)
            event['uuid'] = data.read(16).hex()
            event['kind'] = read_varint(data.read)
            event['position'] = list(struct.unpack('>ddd', data.read(24)))
            event['angles'] = list(data.read(3))
            event['velocity'] = list(struct.unpack('>hhh', data.read(6)))
            event['metadata'] = data.read().hex()
        elif name == 'entity_destroy':
            event['entities'] = [read_varint(data.read) for _ in range(read_varint(data.read))]
            assert not data.read()
        elif name not in ('named_sound_effect', 'sound_effect'):
            event['entity'] = (struct.unpack('>i', data.read(4))[0] if name == 'entity_status'
                               else read_varint(data.read))
            if name == 'entity_metadata':
                event['metadata'] = data.read().hex()
            elif name == 'entity_teleport':
                event['position'] = list(struct.unpack('>ddd', data.read(24)))
                event['angles'] = list(data.read(2))
                event['ground'] = data.read(1)[0]
                assert not data.read()
            elif name in ('rel_entity_move', 'entity_move_look'):
                event['delta'] = list(struct.unpack('>hhh', data.read(6)))
                if name == 'entity_move_look':
                    event['angles'] = list(data.read(2))
                event['ground'] = data.read(1)[0]
                assert not data.read()
            elif name == 'entity_head_rotation':
                event['head_yaw'] = data.read(1)[0]
                assert not data.read()
            elif name == 'entity_velocity':
                event['velocity'] = list(struct.unpack('>hhh', data.read(6)))
                assert not data.read()
        self.events.append(event)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26025)
    args = parser.parse_args()
    assert args.port == 26025
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    jar_hash = hashlib.sha1(args.jar.read_bytes()).hexdigest()
    assert jar_hash == '886945bfb2b978778c3a0288fd7fab09d315b25f'
    sources = json.loads((args.data / 'sources.json').read_text())
    for row in sources['files']:
        assert hashlib.sha256((args.data / row['file']).read_bytes()).hexdigest() == row['sha256']
    protocol = json.loads((args.data / 'protocol.json').read_text())
    report = {'protocol': 340, 'target': 'official', 'jar_sha1': jar_hash,
              'port': args.port, 'sources': sources, 'checks': [], 'packets': [], 'commands': []}
    args.output.mkdir(parents=True, exist_ok=False)
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-cow-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'network-compression-threshold=-1\nlevel-type=FLAT\nlevel-seed=1\n'
            'view-distance=2\ngamemode=1\nspawn-protection=0\nallow-flight=true\n')
        log = args.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(
                ['/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G', '-jar',
                 str(args.jar.resolve()), 'nogui'], cwd=root, stdin=subprocess.PIPE,
                stdout=output, stderr=subprocess.STDOUT, text=True)
            sock = client = None

            def command(value):
                report['commands'].append(value)
                server.stdin.write(value + '\n')
                server.stdin.flush()

            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'Startup failed'
                    time.sleep(.1)
                command('gamerule doMobSpawning false')
                command('gamerule randomTickSpeed 0')
                command('setworldspawn 0 64 0')
                time.sleep(.5)
                sock = socket.create_connection(('127.0.0.1', args.port), timeout=10)
                handshake(sock, 340, args.port, 2)
                send(sock, b'\0\x09McsaProbe')
                client = CowProbe(sock, protocol, report)
                client.pump(20, lambda r: r['name'] == 'position')
                client.pump(1)
                command('fill -8 63 -8 8 63 8 minecraft:bedrock')
                command('fill -8 64 -8 8 76 8 minecraft:air')
                client.pump(.5)
                assert '289 blocks filled' in log.read_text(), 'Authored platform absent'
                command('tp McsaProbe 0.5 64 0.5')
                client.pump(5, lambda r: r['name'] == 'position')
                client.pump(.3)
                report['checks'].append('login-and-platform')

                def stage(name, text, seconds=1):
                    client.stage = name
                    command(text)
                    client.pump(seconds)

                def born(name):
                    rows = [e for e in client.events if e['stage'] == name
                            and e['name'] == 'spawn_entity_living' and e['kind'] == 92]
                    assert len(rows) == 1, (name, rows)
                    report['checks'].append(name)
                    return rows[0]['entity']

                stage('adult-spawn', 'summon cow 3.5 64 0.5 {NoAI:1b,Silent:1b,Tags:["mcsa_adult"]}')
                adult = born('adult-spawn')
                stage('baby-spawn', 'summon cow 6.5 64 0.5 {NoAI:1b,Silent:1b,Age:-24000,Tags:["mcsa_baby"]}')
                baby = born('baby-spawn')
                selector = '@e[type=cow,tag=mcsa_adult,c=1]'
                stage('health4', f'entitydata {selector} {{Health:4f}}')
                assert any(e['stage'] == 'health4' and e['name'] == 'entity_metadata'
                           and e['entity'] == adult for e in client.events)
                report['checks'].append('health-change-raw-metadata')
                stage('speed031', f'entitydata {selector} {{Attributes:[{{Name:"generic.movementSpeed",Base:0.31d}}]}}')
                attrs = [r for r in report['packets'] if r['stage'] == 'speed031'
                         and r['name'] == 'entity_update_attributes' and r['entity'] == adult]
                assert any(a['name'] == 'generic.movementSpeed' and a['base'] == .31
                           for r in attrs for a in r['attributes']), attrs
                report['checks'].append('movement-speed031')
                stage('turn', f'tp {selector} 3.5 64 0.5 90 10')
                stage('teleport', f'tp {selector} -6.5 64 0.5 180 0')
                assert any(e['stage'] == 'teleport' and e['name'] == 'entity_teleport'
                           and e['entity'] == adult for e in client.events)
                report['checks'].append('adult-absolute-teleport')
                stage('falling-spawn', 'summon cow 3.5 70 3.5 {Silent:1b,Tags:["mcsa_falling"]}', 4)
                falling = born('falling-spawn')
                report['authored_entities'] = {'adult': adult, 'baby': baby, 'falling': falling}
                stage('untrack', 'tp McsaProbe 128.5 64 0.5', 2)
                removed = {i for e in client.events if e['stage'] == 'untrack'
                           and e['name'] == 'entity_destroy' for i in e['entities']}
                assert {adult, baby, falling} <= removed, removed
                report['checks'].append('out-of-range-remove')
                stage('retrack', 'tp McsaProbe 0.5 64 0.5', 2)
                spawned = {e['entity'] for e in client.events if e['stage'] == 'retrack'
                           and e['name'] == 'spawn_entity_living'}
                assert {adult, baby, falling} <= spawned, spawned
                report['checks'].append('same-entities-retracked')
                own = {adult, baby, falling}
                names = {e['name'] for e in client.events if e.get('entity') in own}
                report['coverage'] = {name: name in names for name in (
                    'entity_head_rotation', 'rel_entity_move', 'entity_move_look',
                    'entity_teleport', 'entity_velocity', 'entity_metadata',
                    'entity_update_attributes')}
                assert report['coverage']['entity_head_rotation']
                assert report['coverage']['rel_entity_move'] or report['coverage']['entity_move_look']
                report['checks'].extend(['head-rotation-observed', 'relative-motion-observed'])
            except BaseException as error:
                report['error'] = repr(error)
                raise
            finally:
                if client:
                    report['entity_events'] = client.events
                if sock:
                    sock.close()
                if server.poll() is None:
                    command('stop')
                    server.stdin.close()
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['exit_code'] = server.returncode
                (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'checks': report['checks'], 'coverage': report['coverage']}))


if __name__ == '__main__':
    main()
