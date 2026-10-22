#!/usr/bin/env python3
"""Узкая обычная проба white_concrete_slab на официальном777; JAR непрозрачен.

План по умолчанию. --execute только после выделения окна серверу.
Сохраняются только собственные блоки/предметы, не чанки/реестры/мир.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import signal
import socket
import struct
import subprocess
import tempfile
import time

from generate_tables import states
from probe_gameplay import Client, connect
from smoke import read_varint, varint

JAR_SHA1 = '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
HASHES = {
    'protocol.json': '13a10695aa1ed3604ed674ce8fdc8a759f48118714eab98644e685a3d1960727',
    'blocks.json': '776312e51207352734304a293d9300f6c69b4efa40f00ac574c99d1ca7889873',
    'items.json': 'f0d0e447dd0311b7322f4548971d6ebf6074b5842fd4c7b4e3d8698e00b5edcb',
    'entities.json': '33c2d7569e10cb98d86240e8135396702edb92c98e6ccb1c18bf6a481f22e3d5',
}
BLOCK = 'white_concrete_slab'
TARGET = (2, 65, 0)
# Наш actual777 trace: после START0 клиент посылает3 перед air/drop;
# старый775 FINISH2 нельзя переносить по одному VarInt schema без enum.
FINISH_STATUS_777 = 3


def exact(data, size):
    value = data.read(size)
    if len(value) != size:
        raise ValueError('truncated payload')
    return value


def uint(data):
    value = read_varint(data.read)
    if not 0 <= value <= 0x7fffffff:
        raise ValueError('invalid nonnegative VarInt')
    return value


def slot(data):
    count = uint(data)
    if not count:
        return None
    assert count <= 127, 'unexpected count'
    item, added, removed = uint(data), uint(data), uint(data)
    assert added <= 1 and removed == 0, 'unobserved slab/tool components'
    result = {'item': item, 'count': count}
    if added:
        assert uint(data) == 3, 'only normal pickaxe damage supported'
        result['damage'] = uint(data)
    return result


def pack_position(pos):
    x, y, z = pos
    assert -(1 << 25) <= x < (1 << 25) and -(1 << 25) <= z < (1 << 25)
    assert -2048 <= y < 2048
    return struct.pack('>Q', ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 0xfff))


def signed(value, bits):
    return value - (1 << bits) if value & (1 << (bits - 1)) else value


def changes(name, payload):
    data = io.BytesIO(payload)
    result = []
    if name == 'block_change':
        packed = int.from_bytes(exact(data, 8), 'big')
        result.append(([signed(packed >> 38, 26), signed(packed & 4095, 12),
                        signed((packed >> 12) & 0x3ffffff, 26)], uint(data)))
    elif name == 'multi_block_change':
        packed = int.from_bytes(exact(data, 8), 'big')
        x, z, y = signed(packed >> 42, 22), signed((packed >> 20) & 0x3fffff, 22), signed(packed & 0xfffff, 20)
        count = uint(data)
        assert count <= 4096
        for _ in range(count):
            value = uint(data)
            result.append(([x * 16 + ((value >> 8) & 15), y * 16 + (value & 15),
                            z * 16 + ((value >> 4) & 15)], value >> 12))
    else:
        raise ValueError('not a block update')
    assert not data.read(), 'trailing block bytes'
    return result


def item_metadata(payload):
    data = io.BytesIO(payload)
    entity, fields, result = uint(data), set(), None
    while True:
        index = exact(data, 1)[0]
        if index == 255:
            break
        assert index not in fields and len(fields) < 9
        fields.add(index)
        kind = uint(data)
        if (index, kind) in ((0, 0), (3, 8), (4, 8), (5, 8)):
            value = exact(data, 1)[0]
            assert kind != 8 or value <= 1
        elif (index, kind) in ((1, 1), (6, 20), (7, 1)):
            uint(data)
        elif (index, kind) == (2, 6):
            assert exact(data, 1) == b'\0', 'named drops outside probe scope'
        elif (index, kind) == (8, 7):
            result = slot(data)
        else:
            raise AssertionError(f'unknown item metadata {index}/{kind}')
    assert not data.read(), 'trailing metadata bytes'
    return entity, result


class Probe(Client):
    def __init__(self, sock, protocol, item_entity):
        super().__init__(sock, protocol)
        self.item_entity = item_entity
        self.events = []
        self.item_ids = set()
        self.inventory = {}
        self.target_updates = []
        self.sequence = 0
        self.grounded_fixture = False
        self.sent_counts = {}
        self.action_packets = []

    def send(self, name, payload=b''):
        self.sent_counts[name] = self.sent_counts.get(name, 0) + 1
        if name in ('block_dig', 'player_loaded'):
            self.action_packets.append({'name': name, 'packet_id': self.outgoing[name],
                'payload_hex': payload.hex(), 'client_pose': self.position,
                'play_loaded': self.loaded, 'tick_end_sent': self.sent_counts.get('tick_end', 0)})
        # Наш LightClient уже использует Position перед TickEnd. Здесь grounded
        # только для явной сухой каменной площадки; это TCP fixture, не GUI.
        if name == 'tick_end' and self.grounded_fixture:
            assert self.position[:3] == [.5, 65., .5], 'fixture pose changed'
            self.sent_counts['position'] = self.sent_counts.get('position', 0) + 1
            super().send('position', struct.pack('>dddB', *self.position[:3], 1))
        super().send(name, payload)

    def observe_packet(self, name, payload):
        assert len(self.records) < 20000 and len(self.events) < 2000, 'record limit'
        data = io.BytesIO(payload)
        event = None
        if name in ('block_change', 'multi_block_change'):
            for position, state in changes(name, payload):
                if tuple(position) == TARGET:
                    event = {'name': name, 'position': position, 'state': state}
                    self.target_updates.append(event.copy())
        elif name == 'spawn_entity':
            entity = uint(data)
            exact(data, 16)
            if uint(data) == self.item_entity:
                assert len(self.item_ids) < 32
                self.item_ids.add(entity)
                event = {'name': name, 'entity': entity, 'position': list(struct.unpack('>ddd', exact(data, 24)))}
        elif name == 'entity_metadata':
            entity = uint(data)
            if entity in self.item_ids:
                entity, value = item_metadata(payload)
                event = {'name': name, 'entity': entity, 'slot': value}
        elif name in ('set_slot', 'set_player_inventory', 'window_items'):
            if name == 'window_items':
                window, revision, size = uint(data), uint(data), uint(data)
                assert size <= 100
                entries = {i: slot(data) for i in range(size)}
                slot(data)
                if window == 0:
                    self.inventory = entries
            else:
                if name == 'set_slot':
                    window, revision = uint(data), uint(data)
                    index = struct.unpack('>h', exact(data, 2))[0]
                else:
                    window, index = 0, uint(data)
                    index = index + 36 if index < 9 else index
                value = slot(data)
                if window == 0:
                    self.inventory[index] = value
            assert not data.read(), 'trailing inventory bytes'
            event = {'name': name, 'inventory': dict(self.inventory)}
        elif name == 'acknowledge_player_digging':
            event = {'name': name, 'sequence': uint(data)}
            assert not data.read()
        elif name == 'collect':
            event = {'name': name, 'entity': uint(data), 'collector': uint(data), 'count': uint(data)}
            assert not data.read()
        if event:
            event['payload_hex'] = payload.hex()
            self.events.append(event)

    def place(self, pos, face, cursor):
        self.sequence += 1
        self.send('block_place', b'\0' + pack_position(pos) + varint(face) + struct.pack('>fff', *cursor) + b'\0\0' + varint(self.sequence))

    def dig(self, status):
        self.sequence += 1
        self.send('block_dig', varint(status) + pack_position(TARGET) + b'\1' + varint(self.sequence))


def inputs(data):
    result = {}
    for name, expected in HASHES.items():
        raw = (data / name).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == expected, name
        result[name] = json.loads(raw)
    block = next(row for row in result['blocks.json'] if row['name'] == BLOCK)
    item = next(row for row in result['items.json'] if row['name'] == BLOCK)
    assert (block['id'], item['id'], block['minStateId'], block['maxStateId']) == (800, 721, 18307, 18312)
    lookup = {dict(props)['type'] + '/' + dict(props)['waterlogged']: value
              for (name, props), value in states([block]).items()}
    result['states'] = lookup
    return result


def prepare_output(path, execute):
    # Реальные результаты не перезаписываем даже после неуспешной пробы.
    path.mkdir(parents=True, exist_ok=not execute)


def terminate_signal(signum, frame):
    raise SystemExit(128 + signum)


def cleanup_evidence(pid, port):
    evidence = {'remaining_own_group': None, 'port_released': False, 'errors': []}
    try:
        os.killpg(pid, 0)
        evidence['remaining_own_group'] = True
    except ProcessLookupError:
        evidence['remaining_own_group'] = False
    except OSError as error:
        evidence['errors'].append(repr(error))
    try:
        with socket.socket() as check:
            check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            check.bind(('127.0.0.1', port))
        evidence['port_released'] = True
    except OSError as error:
        evidence['errors'].append(repr(error))
    return evidence


def server_properties(port):
    return (f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\n'
            'enforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'gamemode=creative\nspawn-protection=0\nmax-players=1\n')


def diagnostic_commands(label):
    assert label in ('slab-before', 'slab-after', 'stone-before', 'stone-after')
    commands = ['data get entity McsaProbe ' + field
                for field in ('Pos', 'Rotation', 'OnGround', 'SelectedItem')]
    commands += ['execute if entity @a[name=McsaProbe,gamemode=' + mode + '] run say DIAG_' + label + '_' + mode
                 for mode in ('survival', 'creative', 'adventure', 'spectator')]
    commands += ['execute if block 2 65 0 minecraft:' + block + ' run say DIAG_' + label + '_' + name
                 for name, block in [('air', 'air'), ('slab', 'white_concrete_slab[type=bottom,waterlogged=false]'), ('stone', 'stone')]]
    return commands


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26030, choices=[26030])
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--diagnostic-dig', action='store_true', help='узкие команды состояния и stone control; исходные slab assertions сохраняются')
    args = parser.parse_args()
    tables = inputs(args.data)
    with args.jar.open('rb') as source:
        assert hashlib.file_digest(source, 'sha1').hexdigest() == JAR_SHA1
    report = {'protocol': 777, 'sources': HASHES, 'jar_sha1': JAR_SHA1,
              'states': tables['states'], 'checks': [], 'status': 'plan-only',
              'dig_finish_status': FINISH_STATUS_777,
              'probe_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'limits': ['crafting/stonecutting not observed', 'no save/reload coverage',
                         'only white concrete slab, unenchanted tool, authored area']}
    prepare_output(args.output, args.execute)
    if not args.execute:
        (args.output / 'plan.json').write_text(json.dumps(report, indent=2))
        print('Plan only; no sockets, Java, or server launched.')
        return
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    cpus = sorted(os.sched_getaffinity(0))[:2]
    assert len(cpus) == 2
    protocol = tables['protocol.json']
    item_entity = next(v['id'] for v in tables['entities.json'] if v['name'] == 'item')
    tool = next(v['id'] for v in tables['items.json'] if v['name'] == 'diamond_pickaxe')
    block = next(v for v in tables['blocks.json'] if v['name'] == BLOCK)
    assert str(tool) in block['harvestTools']
    client = None
    with tempfile.TemporaryDirectory(prefix='mcsa-white-concrete-slab-777-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(server_properties(args.port))
        log = args.output / 'server.log'
        with log.open('w') as out:
            server = subprocess.Popen(['taskset', '-c', ','.join(map(str, cpus)),
                '/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(args.jar.resolve()), 'nogui'],
                cwd=root, stdin=subprocess.PIPE, stdout=out, stderr=subprocess.STDOUT,
                text=True, start_new_session=True)
            report['cpu_affinity'] = cpus
            deadline = time.monotonic() + 300
            def command(text):
                assert time.monotonic() < deadline and server.poll() is None
                server.stdin.write(text + '\n')
                server.stdin.flush()
            marker = 0
            def barrier(text, duration=.25):
                nonlocal marker
                marker += 1
                token = f'MCSA_SLAB_{marker:03d}_DONE'
                command(text)
                command('say ' + token)
                end = time.monotonic() + 8
                while not re.search(r'\[Server\] ' + token + r'\s*$', log.read_text(), re.M):
                    assert time.monotonic() < end and server.poll() is None, 'command barrier timeout'
                    client.pump(.05)
                client.pump(duration)
            def target(spec):
                barrier('setblock 2 65 0 minecraft:air')
                start = len(client.target_updates)
                barrier('setblock 2 65 0 minecraft:' + spec)
                return client.target_updates[start:]
            def diagnose(label):
                first_line = len(log.read_text().splitlines())
                for text in diagnostic_commands(label):
                    barrier(text, .05)
                lines = log.read_text().splitlines()[first_line:]
                assert len(lines) <= 200, 'diagnostic output limit'
                report.setdefault('diagnostics', []).append({'label': label,
                    'commands': diagnostic_commands(label), 'server_log_lines': lines,
                    'client_pose': client.position, 'sent_counts': dict(client.sent_counts)})
            def check(label, **evidence):
                report['checks'].append({'case': label, **evidence})
            def expect_state(label, expected, action):
                start = len(client.target_updates)
                action()
                client.pump(.6)
                updates = client.target_updates[start:]
                assert updates and updates[-1]['state'] == expected, (label, updates)
                check(label, updates=updates, inventory=dict(client.inventory))
            previous_term = signal.getsignal(signal.SIGTERM)
            try:
                signal.signal(signal.SIGTERM, terminate_signal)
                end = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < end, 'startup timeout'
                    time.sleep(.1)
                client = connect(args.port, protocol, lambda s, p: Probe(s, p, item_entity))
                client.pump(10, lambda r: r['name'] == 'position')
                for text in ['gamerule minecraft:spawn_mobs false', 'gamerule minecraft:random_tick_speed 0',
                             'fill -4 64 -4 5 64 4 stone', 'fill -4 65 -4 5 68 4 air',
                             'tp McsaProbe 0.5 65 0.5', 'clear McsaProbe', 'gamemode survival McsaProbe']:
                    barrier(text)
                assert client.position[:3] == [.5, 65., .5]
                client.send('held_item_slot', struct.pack('>h', 0))
                for state, expected in tables['states'].items():
                    kind, wet = state.split('/')
                    updates = target(f'{BLOCK}[type={kind},waterlogged={wet}]')
                    assert any(row['state'] == expected for row in updates), (state, updates)
                    check('explicit-state-' + state, updates=updates)
                target('air')
                barrier('give McsaProbe minecraft:white_concrete_slab 65')
                stacks = [v['count'] for v in client.inventory.values() if v and v['item'] == 721]
                assert sorted(stacks) == [1, 64], stacks
                check('ordinary-stack-64-plus-1', stacks=stacks)
                barrier('clear McsaProbe')
                barrier('item replace entity McsaProbe weapon.mainhand with minecraft:white_concrete_slab 2')
                expect_state('place-bottom', tables['states']['bottom/false'],
                             lambda: client.place((2, 64, 0), 1, (.5, 1., .5)))
                assert client.inventory[36] == {'item': 721, 'count': 1}, client.inventory
                expect_state('merge-double', tables['states']['double/false'],
                             lambda: client.place(TARGET, 1, (.5, .5, .5)))
                assert client.inventory[36] is None
                target('air')
                barrier('setblock 2 66 0 minecraft:stone')
                barrier('item replace entity McsaProbe weapon.mainhand with minecraft:white_concrete_slab')
                expect_state('place-top', tables['states']['top/false'],
                             lambda: client.place((2, 66, 0), 0, (.5, 0., .5)))
                barrier('setblock 2 66 0 minecraft:air')
                target('water[level=0]')
                barrier('item replace entity McsaProbe weapon.mainhand with minecraft:white_concrete_slab')
                expect_state('place-waterlogged-bottom', tables['states']['bottom/true'],
                             lambda: client.place((2, 64, 0), 1, (.5, 1., .5)))
                barrier('fill -10 65 -10 12 68 10 minecraft:air')
                barrier('tp McsaProbe 0.5 65 0.5')
                assert client.position[:3] == [.5, 65., .5]
                client.grounded_fixture = True
                client.pump(.25)
                report['dig_fixture'] = {'dry_reset': [-10, 65, -10, 12, 68, 10],
                    'position': [.5, 65., .5], 'on_ground': True,
                    'limitations': 'constructed TCP movement; water and grounded changed together; no isolated cause claim'}
                for kind, expected_count, tool_name, delay in [('bottom', 1, 'diamond_pickaxe', 2.),
                        ('double', 2, 'diamond_pickaxe', 2.), ('bottom', 0, 'air', 12.)]:
                    barrier('kill @e[type=minecraft:item]')
                    barrier('clear McsaProbe')
                    target(f'{BLOCK}[type={kind},waterlogged=false]')
                    barrier('item replace entity McsaProbe weapon.mainhand with minecraft:' + tool_name)
                    if args.diagnostic_dig and kind == 'bottom' and tool_name == 'diamond_pickaxe':
                        diagnose('slab-before')
                    start = len(client.events)
                    client.dig(0)
                    client.pump(delay)
                    client.dig(FINISH_STATUS_777)
                    client.pump(1)
                    events = client.events[start:]
                    if args.diagnostic_dig and kind == 'bottom' and tool_name == 'diamond_pickaxe':
                        diagnose('slab-after')
                        target('stone')
                        diagnose('stone-before')
                        stone_start = len(client.events)
                        client.dig(0)
                        client.pump(2.)
                        client.dig(FINISH_STATUS_777)
                        client.pump(1.)
                        report['stone_control_events'] = client.events[stone_start:]
                        diagnose('stone-after')
                    assert any(e.get('position') == list(TARGET) and e.get('state') == 0 for e in events), events
                    drops = [e for e in events if e['name'] == 'entity_metadata' and e.get('slot')]
                    by_entity = {e['entity']: e['slot'] for e in drops}
                    assert sum(v['count'] for v in by_entity.values() if v['item'] == 721) == expected_count, by_entity
                    assert all(v['item'] == 721 for v in by_entity.values())
                    check('dig-' + kind + '-' + tool_name, drops=by_entity, observation_seconds=delay + 1)
                report['status'] = 'pass'
            except BaseException as error:
                report['status'] = 'failed'
                report['error'] = repr(error)
                raise
            finally:
                # Во время cleanup повторный SIGTERM не должен оставить ребёнка.
                signal.signal(signal.SIGTERM, signal.SIG_IGN)
                cleanup_errors = []
                try:
                    if client:
                        report['events'] = client.events
                        report['sent_counts'] = client.sent_counts
                        report['action_packets'] = client.action_packets
                        client.sock.close()
                    if server.poll() is None:
                        try:
                            server.stdin.write('stop\n'); server.stdin.flush()
                            server.wait(timeout=30)
                        except (OSError, subprocess.TimeoutExpired):
                            os.killpg(server.pid, signal.SIGTERM)
                            try:
                                server.wait(timeout=10)
                            except subprocess.TimeoutExpired:
                                os.killpg(server.pid, signal.SIGKILL)
                                server.wait(timeout=10)
                except BaseException as cleanup_error:
                    cleanup_errors.append(repr(cleanup_error))
                    try:
                        os.killpg(server.pid, signal.SIGKILL)
                        server.wait(timeout=10)
                    except ProcessLookupError:
                        pass
                    except BaseException as last_error:
                        cleanup_errors.append(repr(last_error))
                report['server_exit'] = server.returncode
                report['cleanup'] = cleanup_evidence(server.pid, args.port)
                report['cleanup']['errors'].extend(cleanup_errors)
                cleanup_ok = (not report['cleanup']['remaining_own_group']
                              and report['cleanup']['port_released']
                              and not report['cleanup']['errors'])
                if report['status'] == 'pass' and (not cleanup_ok or server.returncode != 0):
                    report['status'] = 'failed-cleanup'
                try:
                    (args.output / 'result.json').write_text(json.dumps(report, indent=2))
                finally:
                    signal.signal(signal.SIGTERM, previous_term)
    assert report['status'] == 'pass', report.get('cleanup')
    print(json.dumps({'status': report['status'], 'checks': len(report['checks'])}))


if __name__ == '__main__':
    main()
