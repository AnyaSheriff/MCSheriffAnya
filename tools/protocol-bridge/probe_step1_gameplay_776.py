#!/usr/bin/env python3
"""Частная сессия776 на основных775/777: выбранные действия, без public admission."""
import argparse
import hashlib
import io
import json
import re
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_core_776 import PORT, PROTOCOL_SHA, stop_server
from probe_776_payloads import table
from probe_full_login_777 import Probe
from probe_gameplay import connect
from probe_light import LightClient
from smoke import varint
from probe_white_concrete_slab import slot, uint, inputs, pack_position


class GameplayProbe(Probe):
    def __init__(self, sock, protocol, item, version):
        super().__init__(sock, protocol, item)
        self.version = version
        self.acks = []
        self.raw = []
        self.lights = []
        self.protocol = protocol
        self.chunk_ready = False
        self.expected_section = None
        self.authored_sections = []
        self.block_entity_counts = []
        self.light_headers = []
        self.spawns = []
        self.sent = []
        self.health_events = []
        self.window = None
        self.menu = None
        self.slots = {}
        self.cursor = None
        self.revision = 0

    def send(self, name, payload=b''):
        if name in {'attack', 'use_entity', 'client_command', 'window_click', 'close_window', 'block_place', 'block_dig', 'teleport_confirm'}:
            self.sent.append({'name': name, 'id': self.outgoing[name], 'hex': payload.hex()})
        super().send(name, payload)

    def confirm_teleport(self, teleport, values):
        body = varint(teleport)
        if self.version == 777:
            body += struct.pack('>dddff', *values)
        self.send('teleport_confirm', body)
        self.acks.append(teleport)

    def observe_packet(self, name, payload):
        assert len(self.raw) < 12000
        row = {'name': name, 'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}
        if name in {'login', 'position', 'spawn_entity', 'entity_metadata', 'update_health',
                    'death_combat_event', 'respawn', 'set_slot', 'set_player_inventory',
                    'window_items', 'set_cursor_item', 'open_window', 'close_window', 'block_action',
                    'block_change', 'acknowledge_player_digging'}:
            row['hex'] = payload.hex()
        self.raw.append(row)
        data = io.BytesIO(payload)
        if name == 'update_health':
            self.health_events.append(struct.unpack('>f', payload[:4])[0])
        elif name == 'open_window':
            self.window, self.menu = uint(data), uint(data)
            self.slots, self.cursor = {}, None
        elif name == 'window_items':
            window, revision, size = uint(data), uint(data), uint(data)
            entries = {i: slot(data) for i in range(size)}
            cursor = slot(data)
            if window == self.window:
                self.slots, self.cursor, self.revision = entries, cursor, revision
        elif name == 'set_slot':
            window, revision = uint(data), uint(data)
            index = struct.unpack('>h', data.read(2))[0]
            value = slot(data)
            if window == self.window:
                self.slots[index], self.revision = value, revision
        elif name == 'set_cursor_item':
            self.cursor = slot(data)
        LightClient.observe_packet(self, name, payload)
        if name == 'spawn_entity':
            from smoke import read_varint
            data = io.BytesIO(payload)
            entity = read_varint(data.read)
            data.read(16)
            self.spawns.append([entity, read_varint(data.read)])
        super().observe_packet(name, payload)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('test-binary', 'protocol', 'native-protocol', 'cache', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--scenario', choices=('all', 'lifecycle', 'relog-light'), default='all')
    parser.add_argument('--primary', type=int, choices=(775,777), default=775)
    parser.add_argument('--peer', type=int, choices=(775,777), default=775)
    parser.add_argument('--peer-protocol', type=Path)
    parser.add_argument('--restart', action='store_true')
    parser.add_argument('--client', type=int, choices=(769,770,771,772,773,774,776), default=776)
    args = parser.parse_args()
    port=26126 if args.client==769 else 26127 if args.client==770 else 26128 if args.client==771 else PORT
    assert not args.output.exists(), 'preserve earlier evidence'
    assert hashlib.sha256(args.protocol.read_bytes()).hexdigest() == (PROTOCOL_SHA if args.client==776 else 'e324c6be98f732a10ddc23539d95b35ad7918b0e2dcc2debf2331c8937564443' if args.client==769 else '8695c1cd011af37d214cc2139fc9bf6e7b445ce45305fbfa28c93f47563cb988' if args.client==770 else '11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be' if args.client==771 else '11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be' if args.client==772 else '0f34fce39ca160cf9856013befd52bdc67cf30687306f47dacd007693d5a2d70' if args.client==773 else '4108833ded28ee21786e4905d0a07b3a06164a3842a8d6bab98096fd3c2e28ab')
    protocols = {args.client: json.loads(args.protocol.read_text()), 775: json.loads(args.native_protocol.read_text())}
    if args.peer == 777:
        assert args.peer_protocol
        assert args.peer_protocol.name == 'protocol.json'
        peer_tables = inputs(args.peer_protocol.parent)
        protocols[777] = peer_tables['protocol.json']
    items = {v['name']: v['id'] for v in table(args.cache, 'items')}
    entity_item = next(v['id'] for v in table(args.cache, 'entities') if v['name'] == 'item')
    native_entities = re.findall(r'\("([^"]+)",', (Path(__file__).resolve().parent / 'src/entity_layouts.rs').read_text())
    native_item = native_entities.index('item')
    player_wire = {775: native_entities.index('player'),
                   args.client: next(v['id'] for v in table(args.cache, 'entities') if v['name'] == 'player')}
    item_wire = {775: native_item, args.client: entity_item}
    if args.peer == 777:
        peer_entities = {v['name']: v['id'] for v in peer_tables['entities.json']}
        player_wire[777], item_wire[777] = peer_entities['player'], peer_entities['item']
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', port))
    args.output.mkdir(parents=True)
    report = {'primary': args.primary, 'client': args.client, 'peer': args.peer if args.scenario == 'all' else None, 'public_admission': False,
              'checks': [], 'commands': [], 'status': 'failed', 'scenario': args.scenario,
              'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'binary_sha256': hashlib.sha256(args.test_binary.read_bytes()).hexdigest()}
    clients = []
    server = None
    with tempfile.TemporaryDirectory(prefix='mcsa-step1-gameplay-') as directory:
        root = Path(directory)
        (root / 'mcsa-gameplay-probe.marker').touch()
        if args.restart:
            (root / 'mcsa-persistence-probe.marker').touch()
        if args.primary == 777:
            (root / 'mcsa-catalog-preview263.marker').touch()
        test = 'serve_java_1_21_4_probe' if args.client==769 else 'serve_java_1_21_5_probe' if args.client==770 else 'serve_java_1_21_6_probe' if args.client==771 else 'serve_java_1_21_8_probe' if args.client==772 else 'serve_java_1_21_9_probe' if args.client==773 else 'serve_java_1_21_11_probe' if args.client in (769,770,771,772,773,774) else ('serve_java_26_2_primary_26_3_probe' if args.primary == 777 else 'serve_java_26_2_probe')
        with (args.output / 'server.log').open('w') as log:
            try:
                def start_server():
                    offset = len((args.output / 'server.log').read_text())
                    process = subprocess.Popen([str(args.test_binary.resolve()), '--ignored', '--exact',
                        'gameplay_probe::' + test, '--nocapture'], cwd=root,
                        stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
                    deadline = time.monotonic() + 30
                    while 'MCSA gameplay probe ready' not in (args.output / 'server.log').read_text()[offset:]:
                        assert process.poll() is None and time.monotonic() < deadline, 'startup timeout'
                        time.sleep(.1)
                    return process
                server = start_server()

                def login(name, version):
                    probe = connect(port, protocols[version],
                        lambda s, p: GameplayProbe(s, p, item_wire[version], version),
                        version=version, username=name)
                    clients.append(probe)
                    probe.pump(20, lambda _: probe.position is not None and 'window_items' in probe.seen)
                    probe.pump(.5)
                    assert probe.position and probe.entity is not None and 'map_chunk' in probe.seen
                    return probe

                client = login('McsaProbe', args.client)
                report['checks'].append('private-login-configuration-chunks-inventory')
                marker = 0

                def pump(seconds=.3):
                    client.pump(seconds)
                    if peer is not None:
                        peer.pump(.1)

                def command(text):
                    nonlocal marker
                    marker += 1
                    token = f'MCSA_CORE_DIG_{marker}'
                    report['commands'].append(text)
                    server.stdin.write(text + '\nsay ' + token + '\n')
                    server.stdin.flush()
                    client.pump(5, lambda _: token in client.markers)
                    assert token in client.markers, 'command barrier missing'
                    pump()

                peer = None
                for text in ['gamerule doMobSpawning false', 'fill -4 64 -4 5 64 4 stone',
                             'fill -4 65 -4 5 68 4 air', 'tp McsaProbe 0.5 65 0.5',
                             'gamemode survival McsaProbe', 'clear McsaProbe', 'gamerule naturalRegeneration false']:
                    command(text)
                client.grounded_fixture = True
                if args.scenario == 'all':
                    peer = login('McsaPeer', args.peer)
                    command('tp McsaPeer 0.5 65 1.5')
                    command('gamemode survival McsaPeer')
                    peer.grounded_fixture = True
                    pump(1.)
                    assert [peer.entity, player_wire[args.client]] in client.spawns, client.spawns
                    assert [client.entity, player_wire[args.peer]] in peer.spawns, peer.spawns
                    report['checks'].append(f'mixed{args.peer}-player-spawns-metadata-movement')
                    before = len(peer.health_events)
                    client.send('use_entity' if args.client in (769,770,771,772,773,774) else 'attack', varint(peer.entity)+(b'\x01\x00' if args.client in (769,770,771,772,773,774) else b''))
                    client.pump(.3)
                    peer.pump(5, lambda _: peer.health is not None and 0 < peer.health < 20)
                    assert any(0 < h < 20 for h in peer.health_events[before:]), peer.health_events
                    report[f'pvp_{args.client}_to_{args.peer}'] = peer.health_events[before:]
                    before = len(client.health_events)
                    peer.send('attack', varint(client.entity))
                    peer.pump(.3)
                    client.pump(5, lambda _: client.health is not None and 0 < client.health < 20)
                    assert any(0 < h < 20 for h in client.health_events[before:]), client.health_events
                    report[f'pvp_{args.peer}_to_{args.client}'] = client.health_events[before:]
                    report['checks'].append(f'mixed{args.peer}-pvp-both-directions-health-decreases')
                if args.scenario != 'relog-light':
                    command('give McsaProbe stone 2')
                    client.send('held_item_slot', struct.pack('>h', 0))
                    client.place((2, 64, 0), 1, (.5, 1., .5))
                    pump(1.)
                    stone_state = next(v['defaultState'] for v in table(args.cache, 'blocks') if v['name'] == 'stone')
                    assert client.target_updates[-1]['state'] == stone_state, client.target_updates
                    assert client.inventory.get(36) == {'item': items['stone'], 'count': 1}, client.inventory
                    report['checks'].append('survival-placement-state-and-consumption')
                    command('clear McsaProbe')
                    command('give McsaProbe iron_pickaxe')
                    command('setblock 2 65 0 stone')
                    start = len(client.target_updates)
                    event_start = len(client.events)
                    client.send('block_dig', varint(0) + pack_position((2, 65, 0)) + b'\x01' + varint(21))
                    pump(1.5)
                    client.send('block_dig', varint(2) + pack_position((2, 65, 0)) + b'\x01' + varint(22))
                    pump(1.)
                    assert any(v['state'] == 0 for v in client.target_updates[start:]), client.target_updates[start:]
                    assert client.inventory.get(36) == {'item': items['iron_pickaxe'], 'count': 1, 'damage': 1}, client.inventory
                    drops = {e['entity']: e['slot'] for e in client.events[event_start:] if e['name'] == 'entity_metadata' and e.get('slot')}
                    assert list(drops.values()) == [{'item': items['cobblestone'], 'count': 1}], drops
                    report['mining_drops'] = drops
                    report['checks'].append('mining-start0-finish2-air-cobblestone-drop-tool-damage')
                    before = len(client.acks)
                    command('kill McsaProbe')
                    assert client.health == 0 and 'death_combat_event' in client.seen
                    client.grounded_fixture = False
                    client.send('client_command', b'\0')
                    client.pump(20, lambda _: client.health == 20 and len(client.acks) > before)
                    assert client.health == 20 and len(client.acks) > before
                    report['checks'].append('death-respawn-health20-teleport-ack')
                    command('tp McsaProbe 0.5 65 0.5')
                    client.grounded_fixture = True
                    # После смерти оставшиеся предметы могут подбираться между
                    # щелчками. Убираем их до очистки инвентаря, с command barrier.
                    command('kill @e[type=item]')
                    command('clear McsaProbe')
                    command('give McsaProbe diamond 3')
                    for slot in (36, 37):
                        client.send('window_click', varint(0) + varint(-1) + struct.pack('>hb', slot, 0) + bytes([0, 0, 0]))
                        pump(.5)
                    assert client.inventory.get(36) is None
                    assert client.inventory.get(37) == {'item': items['diamond'], 'count': 3}, client.inventory
                    report['checks'].append('authoritative-inventory-pickup-deposit')
                    command('setblock 2 65 0 chest')
                    client.place((2, 65, 0), 1, (.5, .5, .5))
                    client.pump(5, lambda _: client.window is not None and len(client.slots) == 63)
                    assert client.window is not None and client.menu == 2 and len(client.slots) == 63, (client.window, client.menu, client.slots)
                    window = client.window
                    def click(index):
                        client.send('window_click', varint(window) + varint(-1) + struct.pack('>hb', index, 0) + bytes([0, 0, 0]))
                        pump(.5)
                    click(55)
                    assert client.cursor == {'item': items['diamond'], 'count': 3}, client.cursor
                    click(0)
                    assert client.cursor is None and client.slots[0] == {'item': items['diamond'], 'count': 3}, client.slots
                    client.send('close_window', varint(window))
                    pump(.5)
                    client.window = None
                    client.place((2, 65, 0), 1, (.5, .5, .5))
                    client.pump(5, lambda _: client.window is not None and client.slots.get(0) is not None)
                    assert client.slots.get(0) == {'item': items['diamond'], 'count': 3}, client.slots
                    window = client.window
                    click(0)
                    click(55)
                    client.send('close_window', varint(client.window))
                    pump(.5)
                    assert client.cursor is None
                    report['checks'].append('chest-open-deposit-close-reopen-withdraw-close')
                else:
                    command('give McsaProbe diamond 3')
                client.sock.close()
                time.sleep(.7)
                client = login('McsaProbe', args.client)
                expected_slot = 36 if args.scenario == 'relog-light' else 37
                assert client.inventory.get(expected_slot) == {'item': items['diamond'], 'count': 3}, client.inventory
                report['checks'].append('relogin-preserves-authoritative-inventory')
                command('setblock -16 64 -16 stone')
                start = len(client.lights)
                command('setblock -16 65 -16 torch')
                pump(1.)
                assert any(v['source_packet'] == 'update_light' and v['torch_cell_level'] == 14 for v in client.lights[start:]), [(v['chunk'], v['torch_cell_level']) for v in client.lights[start:]]
                start = len(client.lights)
                command('setblock -16 65 -16 air')
                pump(1.)
                assert any(v['source_packet'] == 'update_light' and v['torch_cell_level'] == 0 for v in client.lights[start:])
                report['checks'].append('dynamic-light-update-torch14-removal0-masks-arrays')
                if args.restart:
                    assert args.scenario == 'all'
                    client.place((2, 65, 0), 1, (.5, .5, .5))
                    client.pump(5, lambda _: client.window is not None and len(client.slots) == 63)
                    window = client.window
                    # Три алмаза остаются в сундуке через полный перезапуск процесса.
                    click(55)
                    click(0)
                    assert client.slots[0] == {'item': items['diamond'], 'count': 3}
                    client.send('close_window', varint(window))
                    pump(.5)
                    for c in clients:
                        c.sock.close()
                    time.sleep(1.)
                    report['first_shutdown'] = stop_server(server)
                    report['first_server_exit'] = server.returncode
                    assert server.returncode == 0
                    assert 'MCSA persistence probe saved world' in (args.output / 'server.log').read_text()
                    saved = sorted(p for p in (root / 'world').rglob('*') if p.is_file())
                    report['saved_files'] = [{'path': str(p.relative_to(root)), 'bytes': p.stat().st_size,
                                              'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in saved]
                    server = start_server()
                    peer = None
                    client = login('McsaProbe', args.client)
                    assert client.inventory.get(37) is None, client.inventory
                    client.window = None
                    client.place((2, 65, 0), 1, (.5, .5, .5))
                    client.pump(5, lambda _: client.window is not None and len(client.slots) == 63)
                    assert client.slots[0] == {'item': items['diamond'], 'count': 3}, client.slots
                    window = client.window
                    click(0)
                    click(55)
                    client.send('close_window', varint(window))
                    pump(.5)
                    assert client.inventory.get(37) == {'item': items['diamond'], 'count': 3}, client.inventory
                    report['checks'].append('process-restart-player-and-chest-world-persistence')
                report['status'] = 'pass'
            except BaseException as error:
                report['error'] = f'{type(error).__name__}: {error}'
                raise
            finally:
                report['sessions'] = [{'version': c.version, 'records': c.raw, 'sent': c.sent, 'acks': c.acks,
                                       'lights': c.lights, 'inventory': c.inventory, 'health_events': c.health_events} for c in clients]
                for c in clients:
                    c.sock.close()
                if server is not None:
                    report['shutdown'] = stop_server(server)
                    report['server_exit'] = server.returncode
                    if server.returncode != 0:
                        report['status'] = 'failed'
                with socket.socket() as check:
                    check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    check.bind(('127.0.0.1', port))
                (args.output / 'result.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    assert report['status'] == 'pass', report.get('error')
    print(json.dumps({'checks': len(report['checks']), 'status': report['status']}))


if __name__ == '__main__':
    main()
