#!/usr/bin/env python3
"""Собственная сетевая проверка cow340 на изолированном marker-only ядре."""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import socket
import shutil
import struct
import subprocess
import tempfile
import time

from probe_legacy_cow import CowProbe
from probe_legacy_session import string
from smoke import handshake, read_varint, send


def metadata(payload):
    """Только наблюдённые у коровы340 простые metadata; неизвестное — ошибка."""
    data = io.BytesIO(payload)
    fields = {}
    while True:
        index = data.read(1)
        assert index, 'Truncated cow metadata'
        if index == b'\xff':
            break
        index = index[0]
        kind = read_varint(data.read)
        assert index not in fields, ('Duplicate cow metadata index', index)
        if kind == 0:
            value = struct.unpack('>b', data.read(1))[0]
        elif kind == 1:
            value = read_varint(data.read)
        elif kind == 2:
            value = struct.unpack('>f', data.read(4))[0]
            assert math.isfinite(value)
        elif kind in (3, 4):
            value = string(data)
        elif kind == 6:
            value = data.read(1)
            assert value in (b'\0', b'\1')
            value = value[0]
        else:
            raise AssertionError(('Unexpected cow metadata serializer', index, kind))
        fields[index] = {'serializer': kind, 'value': value}
    assert not data.read(), 'Trailing cow metadata'
    return fields


class CoreCowProbe(CowProbe):
    def __init__(self, sock, protocol, report):
        super().__init__(sock, protocol, report)
        self.cows = {}

    def observe_packet(self, name, payload):
        if name.startswith('unknown-') or name == 'kick_disconnect':
            self.report.setdefault('unexpected_packets', []).append(
                {'stage': self.stage, 'name': name, 'payload': payload.hex()})
            raise AssertionError(('Unexpected packet or server disconnect', name))
        before = len(self.events)
        super().observe_packet(name, payload)
        if len(self.events) == before:
            return
        event = self.events[-1]
        origin = self.report['height_origin']
        entity = event.get('entity')
        if name == 'spawn_entity_living':
            assert event['kind'] == 92, ('Unexpected living entity', event)
            fields = metadata(bytes.fromhex(event['metadata']))
            event['fields'] = fields
            position = list(event['position'])
            position[1] += origin
            if entity in self.cows:
                assert self.cows[entity]['uuid'] == event['uuid'], 'Entity UUID changed on retrack'
                assert not self.cows[entity]['tracked'], 'Entity spawned twice without removal'
            self.cows[entity] = {'uuid': event['uuid'], 'position': position,
                                 'fields': fields, 'tracked': True}
            event['world_position'] = position
        elif name == 'entity_destroy':
            for ident in event['entities']:
                if ident in self.cows:
                    self.cows[ident]['tracked'] = False
        elif entity in self.cows:
            cow = self.cows[entity]
            if name == 'entity_metadata':
                fields = metadata(bytes.fromhex(event['metadata']))
                event['fields'] = fields
                cow['fields'].update(fields)
            elif name == 'entity_teleport':
                position = list(event['position'])
                position[1] += origin
                cow['position'] = position
                event['world_position'] = position
            elif name in ('rel_entity_move', 'entity_move_look'):
                cow['position'] = [round(p * 4096) / 4096 + d / 4096
                                   for p, d in zip(cow['position'], event['delta'])]
                event['world_position'] = list(cow['position'])

    def field(self, entity, index, serializer, default=None):
        value = self.cows[entity]['fields'].get(index)
        if value is None and default is not None:
            return default
        assert value is not None and value['serializer'] == serializer, (entity, index, value)
        return value['value']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test-binary', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26027)
    args = parser.parse_args()
    assert args.port == 26027, 'Only the isolated marker-test port is permitted'
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    sources = json.loads((args.data / 'sources.json').read_text())
    for row in sources['files']:
        assert hashlib.sha256((args.data / row['file']).read_bytes()).hexdigest() == row['sha256']
    protocol = json.loads((args.data / 'protocol.json').read_text())
    report = {'protocol': 340, 'target': 'core', 'port': args.port, 'height_origin': -64,
              'test_binary_sha256': hashlib.sha256(args.test_binary.read_bytes()).hexdigest(),
              'sources': sources, 'checks': [], 'commands': [], 'packets': [],
              'visibility_scope': {'world': 'in_memory', 'tracking_range_blocks': 32,
                                   'retained_chunk_radius': 4, 'far_player_position': [48.5, 64.0, .5],
                                   'chunk_unload_and_disk_persistence': False},
              'limits': ['Cow motion without fall damage; damage/sounds/effects are not tested.',
                         'This is an ordinary TCP probe, not a graphical-client test.',
                         'Rotation uses data merge: the five-argument /tp form does not reach mob dispatch, so it changes neither cow position nor angles.',
                         'Absent initial baby/NoAI fields use observed 340 defaults false/0; true requires a field.',
                         'Visibility only: cows remain inside retained chunks of the in-memory fixture.']}
    args.output.mkdir(parents=True, exist_ok=False)
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-cow-core-') as directory:
        root = Path(directory)
        (root / 'mcsa-gameplay-probe.marker').touch()
        log = args.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(
                [str(args.test_binary.resolve()), '--ignored', '--exact',
                 'gameplay_probe::serve_java_1_12_2_probe', '--nocapture'],
                cwd=root, stdin=subprocess.PIPE, stdout=output,
                stderr=subprocess.STDOUT, text=True)
            sock = client = None

            def command(value):
                report['commands'].append(value)
                server.stdin.write(value + '\n')
                server.stdin.flush()

            try:
                deadline = time.monotonic() + 120
                while 'MCSA gameplay probe ready on 26027' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'Startup failed'
                    time.sleep(.1)
                command('gamerule doMobSpawning false')
                command('gamerule randomTickSpeed 0')
                command('fill -8 63 -8 8 63 8 minecraft:bedrock')
                command('fill -8 64 -8 8 76 8 minecraft:air')
                time.sleep(.5)
                sock = socket.create_connection(('127.0.0.1', args.port), timeout=10)
                handshake(sock, 340, args.port, 2)
                send(sock, b'\0\x09McsaProbe')
                client = CoreCowProbe(sock, protocol, report)
                client.pump(20, lambda row: row['name'] == 'position')
                client.pump(.5)
                command('tp McsaProbe 0.5 64 0.5')
                client.pump(5, lambda row: row['name'] == 'position')
                client.pump(.3)
                assert client.position[:3] == [.5, 128.0, .5], client.position
                report['checks'].append('login-and-height-origin')

                def stage(name, text, seconds=1):
                    client.stage = name
                    command(text)
                    client.pump(seconds)

                def born(name, position, baby, no_ai):
                    events = [event for event in client.events if event['stage'] == name
                              and event['name'] == 'spawn_entity_living']
                    assert len(events) == 1, (name, events)
                    event = events[0]
                    entity = event['entity']
                    if no_ai:
                        assert event['world_position'] == position, event
                    else:
                        # Первая публикация может следовать уже после такта
                        # падения: проверяем место, не требуем нулевой задержки.
                        assert abs(event['world_position'][0] - position[0]) < 1
                        assert abs(event['world_position'][2] - position[2]) < 1
                        assert 64.0 <= event['world_position'][1] <= position[1], event
                    assert client.field(entity, 7, 2) == 10.0, client.cows[entity]
                    # Inline metadata может быть пустой; старый клиент начинает
                    # с baby=false/NoAI=0. Положительный флаг обязан прийти явно.
                    assert client.field(entity, 12, 6, None if baby else 0) == int(baby), client.cows[entity]
                    assert client.field(entity, 11, 0, None if no_ai else 0) & 1 == int(no_ai), client.cows[entity]
                    assert client.field(entity, 4, 6) == 1, client.cows[entity]
                    report['checks'].append(name + '-identity-position-health-age-flags')
                    return entity

                stage('adult-spawn', 'summon cow 3.5 64 0.5 {NoAI:1b,Silent:1b,Tags:["mcsa_adult"]}')
                adult = born('adult-spawn', [3.5, 64.0, .5], False, True)
                stage('baby-spawn', 'summon cow 6.5 64 0.5 {NoAI:1b,Silent:1b,Age:-24000,Tags:["mcsa_baby"]}')
                baby = born('baby-spawn', [6.5, 64.0, .5], True, True)
                selector = '@e[type=cow,tag=mcsa_adult,limit=1]'
                stage('health4', f'data merge entity {selector} {{Health:4f}}')
                assert client.field(adult, 7, 2) == 4.0, client.cows[adult]
                report['checks'].append('adult-health4')
                stage('turn', f'data merge entity {selector} {{Rotation:[90f,10f]}}')
                assert any(e['stage'] == 'turn' and e['name'] == 'entity_head_rotation'
                           and e['entity'] == adult for e in client.events), 'Head rotation missing'
                report['checks'].append('adult-head-rotation')
                stage('teleport', f'tp {selector} -6.5 64 0.5')
                assert client.cows[adult]['position'] == [-6.5, 64.0, .5], client.cows[adult]
                assert any(e['stage'] == 'teleport' and e['name'] == 'entity_teleport'
                           and e['entity'] == adult for e in client.events), 'Absolute teleport missing'
                report['checks'].append('adult-absolute-teleport')

                # Высота1 не вызывает урон: проверяем обычное движение, не DamageEvent.
                stage('falling-spawn', 'summon cow 3.5 65 3.5 {Silent:1b,Tags:["mcsa_falling"]}', 2)
                falling = born('falling-spawn', [3.5, 65.0, 3.5], False, False)
                assert client.cows[falling]['position'][1] < 65.0, client.cows[falling]
                movements = [e for e in client.events if e.get('entity') == falling
                             and e['name'] in ('rel_entity_move', 'entity_move_look', 'entity_teleport')]
                assert movements, 'Falling cow movement absent'
                report['checks'].append('falling-cow-movement-without-damage')
                own = {adult, baby, falling}
                report['authored_entities'] = {'adult': adult, 'baby': baby, 'falling': falling}
                for entity in own:
                    attributes = [row for row in report['packets']
                                  if row['name'] == 'entity_update_attributes' and row['entity'] == entity]
                    assert attributes, ('Cow attributes missing', entity)
                    report.setdefault('cow_attributes', {})[str(entity)] = attributes
                report['checks'].append('all-cow-attributes-decoded')
                # Дальше32блоков, но не дальше4чанков: World::in_memory
                # не имеет диска для восстановления выгруженных сущностей.
                for entity in own:
                    x, _, z = client.cows[entity]['position']
                    assert abs(x - 48.5) > 32 or abs(z - .5) > 32, ('Cow still visible', entity)
                    assert abs(math.floor(x / 16) - 3) <= 4 and abs(math.floor(z / 16)) <= 4, ('Cow chunk would unload', entity)
                report['visibility_scope']['positions_before_untrack'] = {
                    str(entity): list(client.cows[entity]['position']) for entity in own}
                stage('untrack', 'tp McsaProbe 48.5 64 0.5', 2)
                removed = {entity for e in client.events if e['stage'] == 'untrack'
                           and e['name'] == 'entity_destroy' for entity in e['entities']}
                assert own <= removed, (own, removed)
                assert all(not client.cows[entity]['tracked'] for entity in own)
                report['checks'].append('out-of-range-remove')
                stage('retrack', 'tp McsaProbe 0.5 64 0.5', 0)
                retrack_started = time.monotonic()
                deadline = retrack_started + 20
                report['retrack_timeout_seconds'] = 20
                try:
                    client.pump(max(0, deadline - time.monotonic()),
                                lambda _: own <= {e['entity'] for e in client.events
                                    if e['stage'] == 'retrack' and e['name'] == 'spawn_entity_living'})
                    # Последний Spawn может предшествовать своей Metadata:
                    # тот же общий deadline, никаких ослабленных ожиданий.
                    def state_ready(_):
                        return (all(client.cows[entity]['tracked'] for entity in own)
                                and client.cows[adult]['fields'].get(7, {}).get('value') == 4.0
                                and client.cows[baby]['fields'].get(12, {}).get('value') == 1)
                    if not state_ready(None):
                        client.pump(max(0, deadline - time.monotonic()), state_ready)
                finally:
                    report['retrack_wait_seconds'] = round(time.monotonic() - retrack_started, 3)
                spawned = {e['entity'] for e in client.events if e['stage'] == 'retrack'
                           and e['name'] == 'spawn_entity_living'}
                assert own <= spawned, (own, spawned)
                assert all(client.cows[entity]['tracked'] for entity in own)
                assert client.field(adult, 7, 2) == 4.0
                assert client.field(baby, 12, 6) == 1
                report['checks'].append('same-identities-health-and-age-retracked')
                report['result'] = 'PASS'
            except BaseException as error:
                report.update(result='FAIL', error=repr(error))
                raise
            finally:
                if client is not None:
                    report['entity_events'] = client.events
                    report['cow_state'] = client.cows
                if sock is not None:
                    sock.close()
                if server.poll() is None:
                    try:
                        command('stop')
                        server.stdin.close()
                    except (BrokenPipeError, OSError):
                        pass
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['exit_code'] = server.returncode
                if server.returncode != 0:
                    report['result'] = 'FAIL'
                if report.get('result') == 'FAIL':
                    preserved = args.output / 'failed-server-state'
                    shutil.copytree(root, preserved)
                    report['preserved_server_state'] = str(preserved.resolve())
                report['server_log_tail'] = log.read_text()[-12000:]
                (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'result': report['result'], 'checks': report['checks']}))


if __name__ == '__main__':
    main()
