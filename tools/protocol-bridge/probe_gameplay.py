#!/usr/bin/env python3
"""Проверка JSON26.3 через обычные команды и пакеты официального сервера.

Только собственный тестовый игрок/мир. Никаких отчётов, распаковки JAR,
выгрузок реестров или чтения сторонних реализаций.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import select
import socket
import struct
import subprocess
import tempfile
import time

from smoke import varint, read_varint, send, handshake
from observe import frame


def mapping(protocol, phase, direction):
    return {name: int(key, 16) for key, name in
            protocol[phase][direction]['types']['packet'][1][0]['type'][1]['mappings'].items()}


class Client:
    def __init__(self, sock, protocol):
        self.sock = sock
        self.incoming = {value: key for key, value in mapping(protocol, 'play', 'toClient').items()}
        self.outgoing = mapping(protocol, 'play', 'toServer')
        self.records = []
        self.last_tick = 0
        self.position = None
        self.loaded = False
        self.login_body = None

    def observe_packet(self, name, payload):
        pass

    def send(self, name, payload=b''):
        send(self.sock, varint(self.outgoing[name]) + payload)

    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + struct.pack('>dddff', *values))

    def pump(self, duration, predicate=None):
        deadline = time.monotonic() + duration
        start = len(self.records)
        while time.monotonic() < deadline:
            if self.loaded and time.monotonic() - self.last_tick > .05:
                self.send('tick_end')
                self.last_tick = time.monotonic()
            if not select.select([self.sock], [], [], .05)[0]:
                continue
            packet = io.BytesIO(frame(self.sock))
            packet_id = read_varint(packet.read)
            payload = packet.read()
            name = self.incoming.get(packet_id, f'unknown-{packet_id}')
            self.observe_packet(name, payload)
            data = io.BytesIO(payload)
            record = {'name': name, 'id': packet_id, 'size': len(payload)}
            if name == 'keep_alive':
                self.send('keep_alive', payload)
            elif name == 'login':
                self.login_body = varint(packet_id) + payload
                self.send('player_loaded')
                self.loaded = True
            elif name == 'position':
                teleport = read_varint(data.read)
                x, y, z, dx, dy, dz, yaw, pitch, flags = struct.unpack('>ddddddffI', data.read())
                # Команда tp без поворота присылает относительные yaw/pitch.
                values = [x, y, z, yaw, pitch]
                assert not (flags & 31) or self.position is not None, 'relative initial position'
                if self.position is not None:
                    values = [value + self.position[i] if flags & (1 << i) else value
                              for i, value in enumerate(values)]
                self.position = values
                self.confirm_teleport(teleport, values)
                record['position'] = self.position
            elif name == 'chunk_batch_finished':
                self.send('chunk_batch_received', struct.pack('>f', 64))
            elif name == 'block_change':
                packed = int.from_bytes(data.read(8), 'big')
                x, z, y = packed >> 38, (packed >> 12) & 0x3ffffff, packed & 0xfff
                x = x - (1 << 26) if x >= (1 << 25) else x
                z = z - (1 << 26) if z >= (1 << 25) else z
                y = y - (1 << 12) if y >= (1 << 11) else y
                record.update(position=[x, y, z], state=read_varint(data.read))
            elif name in ('set_player_inventory', 'set_slot'):
                if name == 'set_slot':
                    record['window'] = read_varint(data.read)
                    record['revision'] = read_varint(data.read)
                    slot = struct.unpack('>h', data.read(2))[0]
                else:
                    slot = read_varint(data.read)
                count = read_varint(data.read)
                record.update(slot=slot, count=count)
                if count:
                    record['item'] = read_varint(data.read)
            elif name == 'spawn_entity':
                entity = read_varint(data.read)
                data.read(16)
                entity_type = read_varint(data.read)
                record.update(entity=entity, entity_type=entity_type,
                              position=list(struct.unpack('>ddd', data.read(24))))
            elif name in ('rel_entity_move', 'entity_move_look', 'sync_entity_position', 'entity_teleport'):
                record['entity'] = read_varint(data.read)
                record['movement_hex'] = data.read().hex()
            elif name == 'kick_disconnect':
                raise RuntimeError('official server disconnected test client')
            self.records.append(record)
            if predicate and predicate(record):
                return record
        if predicate:
            raise TimeoutError('expected packet not received; last names: ' + str([r['name'] for r in self.records[start:]][-12:]))
        return None


def connect(port, protocol, client_type=Client, version=777, *, login_uuid=None):
    if login_uuid is not None and (not isinstance(login_uuid, bytes) or len(login_uuid) != 16):
        raise ValueError('login_uuid must contain16 opaque bytes')
    sock = socket.create_connection(('127.0.0.1', port), timeout=10)
    try:
        handshake(sock, version, port, 2)
        name = b'McsaProbe'
        send(sock, b'\0' + varint(len(name)) + name + (bytes([37]) * 16 if login_uuid is None else login_uuid))
        packet = io.BytesIO(frame(sock))
        assert read_varint(packet.read) == 2
        send(sock, b'\x03')
        send(sock, b'\x00\x05en_us\x02\x00\x01\x7f\x01\x00\x01\x00')
        known_packs = mapping(protocol, "configuration", "toClient")["select_known_packs"]
        for _ in range(160):
            packet = io.BytesIO(frame(sock))
            packet_id = read_varint(packet.read)
            payload = packet.read()
            if packet_id == known_packs:
                send(sock, b'\x07' + payload)
            elif packet_id == 4:
                send(sock, b'\x04' + payload)
            elif packet_id == 3:
                send(sock, b'\x03')
                return client_type(sock, protocol)
        raise RuntimeError('configuration packet budget exhausted')
    except BaseException:
        sock.close()
        raise


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26003)
    args = parser.parse_args()
    with args.jar.open('rb') as source:
        assert hashlib.file_digest(source, 'sha1').hexdigest() == '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
    manifest = json.loads((args.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((args.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((args.data / 'protocol.json').read_text())
    blocks = {v['name']: v for v in json.loads((args.data / 'blocks.json').read_text())}
    items = {v['name']: v for v in json.loads((args.data / 'items.json').read_text())}
    entities = {v['name']: v for v in json.loads((args.data / 'entities.json').read_text())}
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    report = {'checks': [], 'movements': [], 'source_commit': manifest['commit']}
    with tempfile.TemporaryDirectory(prefix='mcsa-gameplay-26.3-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'enforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'gamemode=creative\nspawn-protection=0\nmax-players=2\n')
        log = root / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(args.jar.resolve()), 'nogui'],
                                      cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.1)
                client = connect(args.port, protocol)
                client.pump(10, lambda r: r['name'] == 'position')
                command('tp McsaProbe 0.5 81 0.5')
                client.pump(5, lambda r: r.get('position', [])[:3] == [.5, 81., .5])
                command('setblock 1 79 1 minecraft:dirt')
                client.pump(1)
                report['checks'].append({'login': 'play-loaded-teleport-ack'})
                for block in ['stone', 'oak_log', 'poplar_log', 'poplar_stairs', 'white_wool_slab', 'cinnabar', 'sulfur', 'red_shrub']:
                    expected = blocks[block]['defaultState']
                    command('setblock 1 80 1 minecraft:' + block)
                    event = client.pump(4, lambda r: r['name'] == 'block_change' and r.get('position') == [1, 80, 1])
                    assert event['state'] == expected, (block, expected, event)
                    report['checks'].append({'block': block, 'state': expected})
                for item in ['stone', 'diamond_sword', 'poplar_log', 'white_wool_slab', 'cinnabar', 'sulfur']:
                    command('item replace entity McsaProbe hotbar.0 with minecraft:' + item)
                    event = client.pump(4, lambda r: r['name'] in ('set_player_inventory', 'set_slot') and r.get('item') == items[item]['id'])
                    report['checks'].append({'item': item, 'id': event['item']})
                command('summon armor_stand 2 80 2 {Tags:["mcsa_probe"],NoGravity:1b,Invulnerable:1b}')
                event = client.pump(4, lambda r: r['name'] == 'spawn_entity' and r.get('entity_type') == entities['armor_stand']['id'])
                entity = event['entity']
                client.pump(.3)
                for x in [2.01, 2.1, 3.1, 3.11, 3., 2.]:
                    start = len(client.records)
                    command('data merge entity @e[tag=mcsa_probe,limit=1] {Pos:[' + str(x) + 'd,80d,2d]}')
                    client.pump(.4)
                    report['movements'].append({'commanded_position': [x, 80, 2],
                        'packets': [r for r in client.records[start:] if r.get('entity') == entity and 'movement_hex' in r]})
            except Exception as error:
                report['error'] = str(error)
                raise
            finally:
                if client:
                    client.sock.close()
                if server.poll() is None:
                    command('stop')
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['server_exit'] = server.returncode
                args.output.write_text(json.dumps(report, indent=2) + '\n')
                args.output.with_suffix('.log').write_text(log.read_text())
    print(json.dumps({'passed': len(report['checks']), 'movement_samples': len(report['movements'])}))


if __name__ == '__main__':
    main()
