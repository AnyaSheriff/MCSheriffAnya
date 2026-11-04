#!/usr/bin/env python3
"""Собственная проба item pickup/motion340; общий сценарий без Motion NBT."""
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
import time

from probe_legacy_inventory import InventoryProbe, slot
from probe_legacy_session import string
from smoke import handshake, read_varint, send


class ItemProbe(InventoryProbe):
    def __init__(self, sock, protocol, report, origin):
        super().__init__(sock, protocol, report)
        self.origin = origin
        self.items = {}
        self.events = []

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        data = io.BytesIO(payload)
        event = {'stage': self.stage, 'name': name}
        if name == 'spawn_entity':
            entity = read_varint(data.read)
            uuid = data.read(16).hex()
            kind = data.read(1)[0]
            x, y, z, pitch, yaw, object_data, vx, vy, vz = struct.unpack('>dddBBihhh', data.read())
            assert kind == 2 and object_data == 1, (kind, object_data)
            position = [x, y + self.origin, z]
            self.items[entity] = {'position': position, 'stack': None, 'removed': False}
            event.update(entity=entity, uuid=uuid, kind=kind, object_data=object_data,
                         position=position, velocity=[v / 8000 for v in (vx, vy, vz)])
        elif name == 'entity_metadata':
            entity = read_varint(data.read)
            if entity not in self.items:
                return
            fields = {}
            while True:
                index = data.read(1)[0]
                if index == 255:
                    break
                kind = read_varint(data.read)
                if kind == 0:
                    value = struct.unpack('>b', data.read(1))[0]
                elif kind == 1:
                    value = read_varint(data.read)
                elif kind == 2:
                    value = struct.unpack('>f', data.read(4))[0]
                elif kind in (3, 4):
                    value = string(data)
                elif kind == 5:
                    value = slot(data)
                elif kind == 6:
                    value = data.read(1)[0]
                    assert value in (0, 1)
                else:
                    raise AssertionError(('Unexpected item metadata serializer', kind))
                fields[str(index)] = value
                if index == 6:
                    assert kind == 5
                    self.items[entity]['stack'] = value
            event.update(entity=entity, fields=fields)
        elif name in ('rel_entity_move', 'entity_move_look', 'entity_look', 'entity_teleport', 'entity_velocity'):
            entity = read_varint(data.read)
            assert entity in self.items, (name, entity)
            if name == 'entity_velocity':
                event['velocity'] = [v / 8000 for v in struct.unpack('>hhh', data.read(6))]
            elif name == 'entity_teleport':
                x, y, z, yaw, pitch, ground = struct.unpack('>dddBBB', data.read())
                assert ground in (0, 1)
                self.items[entity]['position'] = [x, y + self.origin, z]
                event.update(position=self.items[entity]['position'], ground=bool(ground))
            else:
                if name != 'entity_look':
                    delta = struct.unpack('>hhh', data.read(6))
                    position = [round(p * 4096) / 4096 + d / 4096
                                for p, d in zip(self.items[entity]['position'], delta)]
                    self.items[entity]['position'] = position
                    event.update(delta=list(delta), position=position)
                if name != 'rel_entity_move':
                    event['angles'] = list(data.read(2))
                ground = data.read(1)[0]
                assert ground in (0, 1)
                event['ground'] = bool(ground)
            event['entity'] = entity
        elif name == 'collect':
            entity, collector, count = [read_varint(data.read) for _ in range(3)]
            assert entity in self.items
            event.update(entity=entity, collector=collector, count=count)
        elif name == 'entity_destroy':
            ids = [read_varint(data.read) for _ in range(read_varint(data.read))]
            for entity in ids:
                if entity in self.items:
                    self.items[entity]['removed'] = True
            event['entities'] = ids
        else:
            return
        assert not data.read(), name
        self.events.append(event)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar', type=Path)
    target.add_argument('--test-binary', type=Path)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int)
    a = p.parse_args()
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    a.port = a.port or (26027 if a.test_binary else 26025)
    if a.test_binary:
        assert a.port == 26027
    else:
        assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == '886945bfb2b978778c3a0288fd7fab09d315b25f'
    sources = json.loads((a.data / 'sources.json').read_text())
    for row in sources['files']:
        assert hashlib.sha256((a.data / row['file']).read_bytes()).hexdigest() == row['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    report = {'protocol': 340, 'target': 'core' if a.test_binary else 'official',
              'port': a.port, 'height_origin': -64 if a.test_binary else 0,
              'checks': [], 'packets': [], 'commands': []}
    a.output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as check:
        check.bind(('127.0.0.1', a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-item-pickup-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\n'
            'network-compression-threshold=-1\nlevel-type=FLAT\nlevel-seed=1\n'
            'view-distance=2\ngamemode=0\nspawn-protection=0\nallow-flight=true\n')
        log = a.output / 'server.log'
        with log.open('w') as output:
            if a.test_binary:
                (root / 'mcsa-gameplay-probe.marker').touch()
                argv = [str(a.test_binary.resolve()), '--ignored', '--exact',
                        'gameplay_probe::serve_java_1_12_2_probe', '--nocapture']
            else:
                argv = ['/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui']
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output,
                                      stderr=subprocess.STDOUT, text=True)
            sock = client = None

            def command(value):
                report['commands'].append(value)
                server.stdin.write(value + '\n')
                server.stdin.flush()

            try:
                deadline = time.monotonic() + 120
                ready = 'MCSA gameplay probe ready on 26027' if a.test_binary else 'Done ('
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'Startup failed'
                    time.sleep(.1)
                command('gamerule doMobSpawning false')
                command('gamerule randomTickSpeed 0')
                # Core готовит площадку до соединения; vanilla — после загрузки чанков.
                if not a.test_binary:
                    command('setworldspawn 0 64 0')
                if a.test_binary:
                    command('fill -4 63 -4 8 63 4 minecraft:bedrock')
                    command('fill -4 64 -4 8 76 4 minecraft:air')
                time.sleep(.5)
                sock = socket.create_connection(('127.0.0.1', a.port), timeout=10)
                handshake(sock, 340, a.port, 2)
                send(sock, b'\0\x09McsaProbe')
                client = ItemProbe(sock, protocol, report, report['height_origin'])
                client.pump(20, lambda r: r['name'] == 'position')
                client.pump(1)
                if not a.test_binary:
                    # setworldspawn не загружает чанки: fill только после входа.
                    command('fill -4 63 -4 8 63 4 minecraft:bedrock')
                    command('fill -4 64 -4 8 76 4 minecraft:air')
                    client.pump(.5)
                    assert '117 blocks filled' in log.read_text(), 'Authored platform was not created'
                command('tp McsaProbe 0.5 64 0.5')
                client.pump(5, lambda r: r['name'] == 'position')
                client.pump(.3)
                assert client.position[:3] == [0.5, 64 - report['height_origin'], 0.5]
                own_id = next(r['entity'] for r in report['packets'] if r['name'] == 'login' and r['phase'] == 'play')
                command('clear McsaProbe')
                client.pump(.3)
                assert sum(s['count'] for s in client.slots if s) == 0

                def summon(stage, y, no_gravity):
                    client.stage = stage
                    count = 'count:3' if a.test_binary else 'Count:3b'
                    command(f'summon item 3.5 {y} 0.5 {{Item:{{id:"minecraft:bedrock",{count}}},NoGravity:{int(no_gravity)}b}}')
                    client.pump(5, lambda r: r['name'] == 'spawn_entity')
                    client.pump(.4)
                    born = [e for e in client.events if e['stage'] == stage and e['name'] == 'spawn_entity']
                    assert len(born) == 1, born
                    entity = born[0]['entity']
                    assert born[0]['position'] == [3.5, y, .5], born[0]
                    assert client.items[entity]['stack'] == {'item': 7, 'count': 3, 'damage': 0}
                    report['checks'].append(stage + '-spawn-and-stack3')
                    return entity

                entity = summon('no-gravity', 64.5, True)
                assert client.items[entity]['position'] == [3.5, 64.5, .5]
                report['checks'].append('no-gravity-position-stable')
                client.stage = 'pickup'
                command('tp McsaProbe 3.5 64 0.5')
                client.pump(5, lambda r: r['name'] == 'collect')
                client.pump(.5)
                collects = [e for e in client.events if e['stage'] == 'pickup' and e['name'] == 'collect']
                assert len(collects) == 1 and (collects[0]['entity'], collects[0]['collector'], collects[0]['count']) == (entity, own_id, 3), collects
                assert client.items[entity]['removed']
                assert sum(s['count'] for s in client.slots if s and s['item'] == 7) == 3
                report['checks'].extend(['collect3-to-own-player', 'inventory-gained3', 'collected-item-removed'])

                client.stage = 'reset'
                command('clear McsaProbe')
                command('tp McsaProbe 0.5 64 0.5')
                client.pump(5, lambda r: r['name'] == 'position')
                client.pump(.3)
                entity = summon('falling', 74.0, False)
                client.pump(3)
                events = [e for e in client.events if e.get('entity') == entity]
                motion = [e for e in events if e['name'] == 'entity_velocity']
                positions = [e for e in events if e['name'] in ('rel_entity_move', 'entity_teleport')]
                assert any(e['velocity'][1] < 0 for e in motion), motion
                assert any(e.get('position', [0, 74])[1] < 74 for e in positions), positions
                assert any(e.get('ground') for e in positions), positions
                assert math.isclose(client.items[entity]['position'][1], 64.0, abs_tol=.001), client.items[entity]
                assert motion and all(abs(v) <= .000125 for v in motion[-1]['velocity']), motion[-1:]
                assert not client.items[entity]['removed']
                report['checks'].extend(['falling-downward-motion', 'falling-position-changed', 'landed-on-authored-platform', 'stopped-motion', 'fallen-item-still-present'])
                report['outcome'] = {'pickup_count': 3, 'pickup_inventory_total': 3,
                                     'final_fall_position': client.items[entity]['position'],
                                     'final_fall_velocity': motion[-1]['velocity']}
            except BaseException as error:
                report['error'] = repr(error)
                raise
            finally:
                if client:
                    report['item_events'] = client.events
                    report['final_items'] = client.items
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
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'checks': report['checks'], 'outcome': report['outcome']}))


if __name__ == '__main__':
    main()
