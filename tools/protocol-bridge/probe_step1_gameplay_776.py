#!/usr/bin/env python3
"""Одна частная сессия776/775: выбранные игровые действия, без public admission."""
import argparse
import hashlib
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

    def send(self, name, payload=b''):
        if name in {'attack', 'use_entity', 'client_command', 'window_click', 'block_place', 'teleport_confirm'}:
            self.sent.append({'name': name, 'id': self.outgoing[name], 'hex': payload.hex()})
        super().send(name, payload)

    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport))
        self.acks.append(teleport)

    def observe_packet(self, name, payload):
        assert len(self.raw) < 12000
        row = {'name': name, 'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}
        if name in {'login', 'position', 'spawn_entity', 'entity_metadata', 'update_health',
                    'death_combat_event', 'respawn', 'set_slot', 'set_player_inventory',
                    'window_items', 'block_change', 'acknowledge_player_digging'}:
            row['hex'] = payload.hex()
        self.raw.append(row)
        LightClient.observe_packet(self, name, payload)
        if name == 'spawn_entity':
            import io
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
    args = parser.parse_args()
    assert not args.output.exists(), 'preserve earlier evidence'
    assert hashlib.sha256(args.protocol.read_bytes()).hexdigest() == PROTOCOL_SHA
    protocols = {776: json.loads(args.protocol.read_text()), 775: json.loads(args.native_protocol.read_text())}
    items = {v['name']: v['id'] for v in table(args.cache, 'items')}
    entity_item = next(v['id'] for v in table(args.cache, 'entities') if v['name'] == 'item')
    native_entities = re.findall(r'\("([^"]+)",', (Path(__file__).resolve().parent / 'src/entity_layouts.rs').read_text())
    native_item = native_entities.index('item')
    with socket.socket() as check:
        check.bind(('127.0.0.1', PORT))
    args.output.mkdir(parents=True)
    report = {'primary': 775, 'client': 776, 'peer': 775 if args.scenario == 'all' else None, 'public_admission': False,
              'checks': [], 'commands': [], 'status': 'failed', 'scenario': args.scenario,
              'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'binary_sha256': hashlib.sha256(args.test_binary.read_bytes()).hexdigest()}
    clients = []
    server = None
    with tempfile.TemporaryDirectory(prefix='mcsa-step1-gameplay-') as directory:
        root = Path(directory)
        (root / 'mcsa-gameplay-probe.marker').touch()
        with (args.output / 'server.log').open('w') as log:
            try:
                server = subprocess.Popen([str(args.test_binary.resolve()), '--ignored', '--exact',
                    'gameplay_probe::serve_java_26_2_probe', '--nocapture'], cwd=root,
                    stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
                deadline = time.monotonic() + 30
                while 'MCSA gameplay probe ready' not in (args.output / 'server.log').read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup timeout'
                    time.sleep(.1)

                def login(name, version):
                    probe = connect(PORT, protocols[version],
                        lambda s, p: GameplayProbe(s, p, entity_item if version == 776 else native_item, version),
                        version=version, username=name)
                    clients.append(probe)
                    probe.pump(20, lambda _: probe.position is not None and 'window_items' in probe.seen)
                    probe.pump(.5)
                    assert probe.position and probe.entity is not None and 'map_chunk' in probe.seen
                    return probe

                client = login('McsaProbe', 776)
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
                             'gamemode survival McsaProbe', 'clear McsaProbe']:
                    command(text)
                client.grounded_fixture = True
                if args.scenario == 'all':
                    peer = login('McsaPeer', 775)
                    command('tp McsaPeer 0.5 65 1.5')
                    command('gamemode survival McsaPeer')
                    peer.grounded_fixture = True
                    pump(1.)
                    assert [peer.entity, 156] in client.spawns, client.spawns
                    assert [client.entity, 155] in peer.spawns, peer.spawns
                    report['checks'].append('mixed775-player-spawns-metadata-movement')
                    client.send('attack', varint(peer.entity))
                    client.pump(.3)
                    peer.pump(5, lambda _: peer.health is not None and 0 < peer.health < 20)
                    assert peer.health is not None and 0 < peer.health < 20, peer.health
                    report['pvp_776_to_775'] = peer.health
                    peer.send('attack', varint(client.entity))
                    peer.pump(.3)
                    client.pump(5, lambda _: client.health is not None and 0 < client.health < 20)
                    assert client.health is not None and 0 < client.health < 20, client.health
                    report['pvp_775_to_776'] = client.health
                    report['checks'].append('mixed775-pvp-both-directions-health-decreases')
                if args.scenario != 'relog-light':
                    command('give McsaProbe stone 2')
                    client.send('held_item_slot', struct.pack('>h', 0))
                    client.place((2, 64, 0), 1, (.5, 1., .5))
                    pump(1.)
                    stone_state = next(v['defaultState'] for v in table(args.cache, 'blocks') if v['name'] == 'stone')
                    assert client.target_updates[-1]['state'] == stone_state, client.target_updates
                    assert client.inventory.get(36) == {'item': items['stone'], 'count': 1}, client.inventory
                    report['checks'].append('survival-placement-state-and-consumption')
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
                    command('clear McsaProbe')
                    command('give McsaProbe diamond 3')
                    for slot in (36, 37):
                        client.send('window_click', varint(0) + varint(-1) + struct.pack('>hb', slot, 0) + bytes([0, 0, 0]))
                        pump(.5)
                    assert client.inventory.get(36) is None
                    assert client.inventory.get(37) == {'item': items['diamond'], 'count': 3}, client.inventory
                    report['checks'].append('authoritative-inventory-pickup-deposit')
                else:
                    command('give McsaProbe diamond 3')
                client.sock.close()
                time.sleep(.7)
                client = login('McsaProbe', 776)
                expected_slot = 36 if args.scenario == 'relog-light' else 37
                assert client.inventory.get(expected_slot) == {'item': items['diamond'], 'count': 3}, client.inventory
                report['checks'].append('relogin-preserves-authoritative-inventory')
                command('setblock -16 64 -16 stone')
                command('setblock -16 65 -16 torch')
                pump(1.)
                assert any(v['chunk'] == [-1, -1] and v['torch_cell_level'] == 14 for v in client.lights), [(v['chunk'], v['torch_cell_level']) for v in client.lights]
                report['checks'].append('light-masks-arrays-and-torch-level14')
                report['status'] = 'pass'
            except BaseException as error:
                report['error'] = f'{type(error).__name__}: {error}'
                raise
            finally:
                report['sessions'] = [{'version': c.version, 'records': c.raw, 'sent': c.sent, 'acks': c.acks,
                                       'lights': c.lights, 'inventory': c.inventory} for c in clients]
                for c in clients:
                    c.sock.close()
                if server is not None:
                    report['shutdown'] = stop_server(server)
                    report['server_exit'] = server.returncode
                    if server.returncode != 0:
                        report['status'] = 'failed'
                with socket.socket() as check:
                    check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    check.bind(('127.0.0.1', PORT))
                (args.output / 'result.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    assert report['status'] == 'pass', report.get('error')
    print(json.dumps({'checks': len(report['checks']), 'status': report['status']}))


if __name__ == '__main__':
    main()
