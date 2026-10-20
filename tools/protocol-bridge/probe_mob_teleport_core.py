#!/usr/bin/env python3
"""Сравнение /tp моба с own official775 fixture через реальные пакеты ядра.

Только сохранённый test binary, существующая marker-only точка входа26026,
клиент775. Без data get и без нового сетевого клиента: подкласс нашего Client.
Запуск только в отдельном MC-окне после общего Cargo.
"""
import argparse
import copy
import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import socket
import struct
import subprocess
import tempfile
import time

from probe_gameplay import Client, connect
from smoke import read_varint, varint

SELECTOR = '@e[type=cow,tag=mcsa_tp,limit=1]'


def angle_byte(value):
    return math.floor(value * 256 / 360) & 255


def snbt_vector(text, count):
    """Только числовые списки из нашей сохранённой fixture; не SNBT parser."""
    assert text.startswith('[') and text.endswith(']'), text
    values = text[1:-1].split(',')
    assert len(values) == count
    result = []
    for value in values:
        assert re.fullmatch(r'\s*[+-]?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fd]?\s*', value), value
        number = float(value.strip().rstrip('fd'))
        assert math.isfinite(number)
        result.append(number)
    return result


class MobProbe(Client):
    def __init__(self, sock, protocol, cow_type):
        super().__init__(sock, protocol)
        self.cow_type = cow_type
        self.cows = {}
        self.observed = []
        self.stage = 'login'

    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport))

    def command(self, value):
        raw = value.encode()
        self.send('chat_command', varint(len(raw)) + raw)

    def observe_packet(self, name, payload):
        if name.startswith('unknown-') or name in ('disconnect', 'kick_disconnect'):
            raise AssertionError(('Unexpected packet/disconnect', name, payload.hex()))
        relevant = ('spawn_entity', 'sync_entity_position', 'rel_entity_move',
                    'entity_move_look', 'entity_look', 'entity_head_rotation',
                    'entity_teleport', 'system_chat')
        if name not in relevant:
            return
        event = {'stage': self.stage, 'name': name, 'payload_hex': payload.hex()}
        self.observed.append(event)
        if name == 'system_chat':
            return
        data = io.BytesIO(payload)
        entity = read_varint(data.read)
        event['entity'] = entity
        if name == 'spawn_entity':
            uuid = data.read(16).hex()
            kind = read_varint(data.read)
            if kind != self.cow_type:
                return
            position = list(struct.unpack('>ddd', data.read(24)))
            first = data.read(1)[0]
            if first:
                assert len(data.read(5)) == 5
                if first & 4:
                    read_varint(data.read)
            pitch, yaw, head = data.read(3)
            read_varint(data.read)
            assert not data.read()
            assert entity not in self.cows, 'Unexpected cow retrack'
            self.cows[entity] = {'uuid': uuid, 'position': position,
                'yaw_byte': yaw, 'pitch_byte': pitch, 'head_byte': head,
                'exact_rotation': None, 'sync_count': 0}
        elif entity in self.cows:
            cow = self.cows[entity]
            if name == 'sync_entity_position':
                values = struct.unpack('>ddddddff?', data.read())
                assert all(math.isfinite(v) for v in values[:-1])
                cow['position'] = list(values[:3])
                cow['velocity'] = list(values[3:6])
                cow['exact_rotation'] = list(values[6:8])
                cow['yaw_byte'], cow['pitch_byte'] = map(angle_byte, values[6:8])
                cow['on_ground'] = values[8]
                cow['sync_count'] += 1
            elif name in ('rel_entity_move', 'entity_move_look', 'entity_look'):
                if name != 'entity_look':
                    delta = struct.unpack('>hhh', data.read(6))
                    cow['position'] = [round(v * 4096) / 4096 + d / 4096
                                       for v, d in zip(cow['position'], delta)]
                if name != 'rel_entity_move':
                    cow['yaw_byte'], cow['pitch_byte'] = data.read(2)
                    cow['exact_rotation'] = None
                grounded = data.read(1)
                assert grounded in (b'\0', b'\1')
                cow['on_ground'] = grounded == b'\1'
                assert not data.read()
            elif name == 'entity_head_rotation':
                cow['head_byte'] = data.read(1)[0]
                assert not data.read()
            else:
                # Этот путь native sender не использует. Не угадываем relative flags.
                raise AssertionError(('Unexpected entity_teleport for cow', payload.hex()))
            event['state'] = copy.deepcopy(cow)


def assert_state(cow, expected, *, exact):
    position = snbt_vector(expected['Pos'], 3)
    rotation = snbt_vector(expected['Rotation'], 2)
    assert cow['position'] == position, ('position', cow, expected)
    assert [cow['yaw_byte'], cow['pitch_byte']] == list(map(angle_byte, rotation)), ('rotation bytes', cow, expected)
    assert cow['head_byte'] == angle_byte(rotation[0]), ('head', cow, expected)
    if exact:
        assert cow['exact_rotation'] == rotation, ('exact rotation', cow, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test-binary', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26026)
    args = parser.parse_args()
    assert args.port == 26026, 'Existing marker-only test entry point uses26026'
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    manifest = json.loads((args.data / 'sources.json').read_text())
    for name in ('protocol-26.1.json', 'entities-26.1.json'):
        entry = next(row for row in manifest['files'] + manifest['baseline_files'] if row['file'] == name)
        assert hashlib.sha256((args.data / name).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((args.data / 'protocol-26.1.json').read_text())
    entities = json.loads((args.data / 'entities-26.1.json').read_text())
    cow_type = next(row['id'] for row in entities if row['name'] == 'cow')
    fixture_path = Path(__file__).parent / 'testdata/mob-teleport-observations.json'
    fixture = json.loads(fixture_path.read_text())
    cases = [row for row in fixture['cases'] if row['source'] in ('console', 'player')]
    assert len(cases) == 26
    report = {'protocol': 775, 'target': 'core', 'port': args.port,
        'test_binary_sha256': hashlib.sha256(args.test_binary.read_bytes()).hexdigest(),
        'fixture_sha256': hashlib.sha256(fixture_path.read_bytes()).hexdigest(),
        'scope': '26 console/player cases; execute as/at are not implemented/tested here.',
        'limits': 'Network verifies position/body/head and published speed/ground; unit tests verify internal fall/teleport counters.',
        'cases': [], 'commands': []}
    args.output.mkdir(parents=True, exist_ok=False)
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', args.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-mob-tp-core-') as directory:
        root = Path(directory)
        (root / 'mcsa-gameplay-probe.marker').touch()
        log = args.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen([str(args.test_binary.resolve()), '--ignored', '--exact',
                'gameplay_probe::serve_java_26_3_probe', '--nocapture'], cwd=root,
                stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            sequence = 0
            def command(value):
                report['commands'].append(value)
                server.stdin.write(value + '\n')
                server.stdin.flush()
            def barrier(player=False):
                nonlocal sequence
                sequence += 1
                marker = f'MCSA_CORE_TP_{sequence:04d}'
                start = len(client.observed)
                if player:
                    client.command('say ' + marker)
                else:
                    command('say ' + marker)
                deadline = time.monotonic() + 10
                # Marker в system_chat подтверждает обработку обеих команд.
                while not any(marker.encode() in bytes.fromhex(row['payload_hex'])
                              for row in client.observed[start:] if row['name'] == 'system_chat'):
                    assert server.poll() is None and time.monotonic() < deadline, 'command barrier failed'
                    client.pump(.05)
                client.pump(.3)
            try:
                deadline = time.monotonic() + 120
                while 'MCSA gameplay probe ready on 26026' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.1)
                command('gamerule doMobSpawning false')
                command('gamerule randomTickSpeed 0')
                client = connect(args.port, protocol, lambda sock, proto: MobProbe(sock, proto, cow_type), 775)
                client.pump(20, lambda row: row['name'] == 'position')
                client.pump(.3)
                command('op McsaProbe')
                command('tp McsaProbe 0.5 64 0.5 35 -12')
                client.pump(5, lambda row: row.get('position') == [.5, 64., .5, 35., -12.])
                assert client.position == [.5, 64., .5, 35., -12.], client.position
                command('summon cow 3.5 64 0.5 {NoAI:1b,NoGravity:1b,Silent:1b,Tags:["mcsa_tp"],Rotation:[80f,20f]}')
                barrier()
                assert len(client.cows) == 1, client.cows
                entity = next(iter(client.cows))
                for case in cases:
                    client.stage = 'reset-' + case['name']
                    command(f'tp {SELECTOR} 3.5 64 0.5')
                    command(f'data merge entity {SELECTOR} {{Rotation:[80f,20f]}}')
                    barrier()
                    deadline = time.monotonic() + 5
                    while True:
                        try:
                            assert_state(client.cows[entity], case['before'], exact=False)
                            break
                        except AssertionError:
                            if time.monotonic() >= deadline:
                                raise
                            client.pump(.05)
                    before = copy.deepcopy(client.cows[entity])
                    client.stage = case['name']
                    offset = len(client.observed)
                    if case['source'] == 'player':
                        report['commands'].append('PLAYER: ' + case['command'])
                        client.command(case['command'])
                        barrier(player=True)
                    else:
                        command(case['command'])
                        barrier()
                    invalid = case['unchanged']
                    if not invalid:
                        deadline = time.monotonic() + 5
                        while True:
                            try:
                                assert_state(client.cows[entity], case['after'], exact=True)
                                assert client.cows[entity]['sync_count'] > before['sync_count']
                                break
                            except AssertionError:
                                if time.monotonic() >= deadline:
                                    raise
                                client.pump(.05)
                    after = copy.deepcopy(client.cows[entity])
                    assert_state(after, case['after'], exact=not invalid)
                    if invalid:
                        # NoAI fixture неподвижна. Вся опубликованная state должна сохраниться.
                        assert after == before, ('malformed mutation', case['name'], before, after)
                    else:
                        assert after['sync_count'] > before['sync_count'], 'No fresh authoritative position/rotation'
                    report['cases'].append({'name': case['name'], 'command': case['command'],
                        'before': before, 'after': after, 'packets': client.observed[offset:], 'passed': True})
            except BaseException as error:
                report['error'] = repr(error)
                raise
            finally:
                if client:
                    report['packets'] = client.observed
                    client.sock.close()
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
    print(json.dumps({'cases': len(report['cases']), 'exit_code': report['exit_code']}))


if __name__ == '__main__':
    main()
