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


def connect(port, protocol, client_type=Client, version=777, *, login_uuid=None, username="McsaProbe", configuration_handler=None, view_distance=2):
    if login_uuid is not None and (not isinstance(login_uuid, bytes) or len(login_uuid) != 16):
        raise ValueError('login_uuid must contain16 opaque bytes')
    sock = socket.create_connection(('127.0.0.1', port), timeout=10)
    try:
        handshake(sock, version, port, 2)
        name = username.encode('ascii')
        assert 1 <= len(name) <= 16 and username.replace('_', '').isalnum()
        send(sock, b'\0' + varint(len(name)) + name + (bytes([37]) * 16 if login_uuid is None else login_uuid))
        packet = io.BytesIO(frame(sock))
        assert read_varint(packet.read) == 2
        send(sock, b'\x03')
        assert 2 <= view_distance <= 32
        send(sock, b'\x00\x05en_us' + bytes([view_distance]) + b'\x00\x01\x7f\x01\x00\x01\x00')
        known_packs = mapping(protocol, "configuration", "toClient")["select_known_packs"]
        for _ in range(160):
            packet = io.BytesIO(frame(sock))
            packet_id = read_varint(packet.read)
            payload = packet.read()
            if packet_id == known_packs:
                send(sock, b'\x07' + payload)
            elif packet_id == 4:
                send(sock, b'\x04' + payload)
            elif configuration_handler and configuration_handler(sock, packet_id, payload):
                continue
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


# Отдельная сетевая проверка очередей/света, без чтения кода оригинала.
import math
import shutil
import sys
import uuid
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parents[2]
JAVA = '/usr/lib/jvm/java-25-openjdk/bin/java'

def string(text):
    data = text.encode()
    return varint(len(data)) + data

def packed(x, y, z):
    return struct.pack('>Q', ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 4095))

class Probe(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.chunks = {}
        self.responses = {}
        self.latest_chunk = None
        self.login_at = None
        self.center = None
        self.kick = None
        self.wanted_echo = None
        self.echoes = []
        self.entity = None
        self.position_packets = []

    def confirm_teleport(self, teleport, values):
        assert all(math.isfinite(value) for value in values), (teleport, values, self.position_packets[-2:])
        super().confirm_teleport(teleport, values)

    def command(self, text):
        self.send('chat_command', string(text))

    def observe_packet(self, name, payload):
        now = time.monotonic()
        if name == 'position':
            self.position_packets.append(payload.hex())
        if name == 'login':
            self.login_at = now
        elif name == 'update_view_position':
            data = io.BytesIO(payload)
            self.center = (read_varint(data.read), read_varint(data.read))
            self.center = tuple(v - 2**32 if v >= 2**31 else v for v in self.center)
        elif name == 'map_chunk':
            place = struct.unpack('>ii', payload[:8])
            self.chunks[place] = now
            self.latest_chunk = now
        elif name == 'tab_complete':
            self.responses[read_varint(io.BytesIO(payload).read)] = now
        elif name == 'kick_disconnect':
            self.kick = payload.hex()
        if self.wanted_echo:
            kind, at, sequence, start = self.wanted_echo
            if kind in ('dig', 'place') and name == 'block_change':
                data = io.BytesIO(payload)
                value = int.from_bytes(data.read(8), 'big')
                x, z, y = value >> 38, (value >> 12) & 0x3ffffff, value & 4095
                if x >= 1 << 25: x -= 1 << 26
                if z >= 1 << 25: z -= 1 << 26
                if y >= 1 << 11: y -= 1 << 12
                state = read_varint(data.read)
                if (x, y, z) == at and ((kind == 'dig' and state == 0) or (kind == 'place' and state != 0)):
                    self.echoes.append(dict(kind=kind, ms=(now - start) * 1000))
                    self.wanted_echo = None
            elif kind == 'hit' and name in ('damage_event', 'hurt_animation', 'entity_metadata'):
                received = read_varint(io.BytesIO(payload).read)
                if received == at:
                    self.echoes.append(dict(kind=kind, ms=(now - start) * 1000))
                    self.wanted_echo = None

    def settle(self, seconds=45):
        start = time.monotonic()
        while time.monotonic() - start < seconds:
            self.pump(.1)
            if self.chunks and self.latest_chunk and time.monotonic() - self.latest_chunk > 2:
                return
        raise TimeoutError('view did not finish')


def measure(port, protocol, name, join_only=False):
    client = connect(port, protocol, client_type=Probe, username=name, view_distance=10)
    try:
        client.settle()
        assert client.position is not None
        center = client.center
        join = dict(chunks=len(client.chunks), center=center, first_ms=(min(client.chunks.values())-client.login_at)*1000,
                    ms=(max(client.chunks.values()) - client.login_at) * 1000)
        print(name, port, 'join', join, flush=True)
        if join_only: return dict(join=join)
        stages = []
        for label, speed, distance in [('walking', 4.3, 32), ('flying', 16., 64)]:
            client.command('gamemode creative')
            client.send('abilities', b'\x00' if label == 'walking' else b'\x02')
            client.pump(.2)
            x, _, z, yaw, pitch = client.position
            if label == 'walking':
                # Настоящая ходьба по опоре, без сохранённого флага полёта.
                left, right, cross = math.floor(x) - 2, math.ceil(x + distance) + 2, math.floor(z)
                client.command(f'fill {left} 80 {cross-2} {right} 80 {cross+2} stone')
                client.command(f'fill {left} 81 {cross-2} {right} 84 {cross+2} air')
            height = 81 if label == 'walking' else 90
            client.command(f'tp @s {x} {height} {z} 0 0')
            client.pump(.5)
            client.settle()
            x, y, z, yaw, pitch = client.position
            old = set(client.chunks)
            start = time.monotonic()
            steps = round(distance / speed / .05)
            # Движение и Client Tick End образуют один кадр; второго таймера нет.
            client.loaded = False
            client.send('tick_end')
            try:
                for index in range(1, steps + 1):
                    nx = x + distance * index / steps
                    assert all(math.isfinite(v) for v in (nx, y, z, yaw, pitch)), (client.position, client.position_packets[-2:])
                    client.send('position_look', struct.pack('>dddffB', nx, y, z, yaw, pitch, int(label == 'walking')))
                    client.send('tick_end')
                    client.position = [nx, y, z, yaw, pitch]
                    client.pump(max(0, start + index * .05 - time.monotonic()))
            finally:
                client.loaded = True
                client.last_tick = time.monotonic()
            end_move = time.monotonic()
            client.settle()
            new = [at for place, at in client.chunks.items() if place not in old]
            assert new and client.center == (math.floor((x + distance) / 16), math.floor(z / 16)), (label, client.center, client.position)
            stages.append(dict(mode=label, speed=speed, blocks=distance, new_chunks=len(new),
                movement_ms=(end_move-start)*1000, last_chunk_after_stop_ms=(max(new)-end_move)*1000 if new else None,
                total_ms=(max(new)-start)*1000 if new else None))
        print(name, port, 'movement', stages, flush=True)
        # Добыча/установка во время холодной догрузки новой полосы.
        client.command('fill 398 80 -2 404 80 2 stone')
        client.command('fill 398 81 -2 404 84 2 air')
        client.command('tp @s 400.5 81 0.5 0 0')
        client.pump(.2)
        client.command('item replace entity @s weapon.mainhand with stone 64')
        client.command('summon cow 402 81 0 {NoAI:1b,Health:200f,attributes:[{id:"minecraft:max_health",base:200.0}]}')
        client.pump(.6)
        cows = [r['entity'] for r in client.records if r['name'] == 'spawn_entity' and abs(r.get('position', [0])[0] - 402) < 1]
        for index in range(10):
            pos = (400, 81, 1)
            client.command('setblock 400 81 1 stone')
            client.pump(.1)
            sequence = index * 2 + 1
            client.wanted_echo = ('dig', pos, sequence, time.monotonic())
            # START_DESTROY_BLOCK =0 в обоих проверяемых wire-профилях.
            client.send('block_dig', b'\x00' + packed(*pos) + b'\x01' + varint(sequence))
            client.pump(.2)
            sequence += 1
            client.wanted_echo = ('place', pos, sequence, time.monotonic())
            client.send('block_place', b'\x00' + packed(400, 80, 1) + b'\x01' + struct.pack('>fff', .5, 1., .5) + b'\x00\x00' + varint(sequence))
            client.pump(.2)
            cows = [r['entity'] for r in client.records if r['name'] == 'spawn_entity' and abs(r.get('position', [0])[0] - 402) < 1]
            if cows:
                client.wanted_echo = ('hit', cows[-1], 0, time.monotonic())
                client.send('attack', varint(cows[-1]))
                client.pump(.6)
        print(name, port, 'echoes', client.echoes, flush=True)
        return dict(join=join, movement=stages, echoes=client.echoes)
    finally:
        client.sock.close()


def completion_limits(port, protocol):
    rows = []
    for count, unit in [(32_500, 'a'), (32_501, 'a'), (16_250, '😀'), (16_251, '😀')]:
        client = connect(port, protocol, client_type=Probe, username='TabProbe', view_distance=2)
        try:
            client.pump(1)
            text = unit * count
            start = time.monotonic()
            client.send('tab_complete', varint(42) + string(text))
            try:
                client.pump(2, lambda r: 42 in client.responses)
                outcome = 'response'
            except (RuntimeError, EOFError, ConnectionError, TimeoutError):
                outcome = 'disconnect' if client.kick else 'no-response'
            rows.append(dict(utf16=len(text.encode('utf-16-le'))//2, bytes=len(text.encode()),
                             outcome=outcome, ms=(time.monotonic()-start)*1000, kick=client.kick))
        finally:
            client.sock.close()
    return rows


def review_streaming_main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--official-port', type=int, default=26120)
    parser.add_argument('--ours-port', type=int, default=26220)
    parser.add_argument('--ours-only', action='store_true')
    parser.add_argument('--attach', action='store_true')
    parser.add_argument('--join-only', action='store_true')
    args = parser.parse_args()
    assert 26100 <= args.official_port <= 26199 and 26200 <= args.ours_port <= 26299
    args.output.mkdir(parents=True, exist_ok=False)
    protocol = json.loads((args.data / 'protocol.json').read_text())
    children = []
    logs = []
    result = dict(binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  jar_sha256=hashlib.sha256(args.jar.read_bytes()).hexdigest())
    try:
        for label, port in ([] if args.attach else [('ours', args.ours_port)] if args.ours_only else [('official', args.official_port), ('ours', args.ours_port)]):
            folder = args.output / label
            folder.mkdir()
            config = folder if label == 'official' else folder / 'config'
            config.mkdir(exist_ok=True)
            (config / 'server.properties').write_text(
                f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nenforce-secure-profile=false\n'
                'network-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=10\nsimulation-distance=4\n'
                'generator-settings={"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":3}],"biome":"minecraft:plains"}\n'
                'generate-structures=false\nspawn-protection=0\nallow-flight=true\ngamemode=creative\nmax-tick-time=0\n')
            operators = []
            for name in ('StreamProbe', 'TabProbe'):
                raw = bytearray(hashlib.md5(('OfflinePlayer:' + name).encode()).digest())
                raw[6] = (raw[6] & 15) | 48
                raw[8] = (raw[8] & 63) | 128
                operators.append(dict(uuid=str(uuid.UUID(bytes=bytes(raw))), name=name, level=4))
            (folder / 'ops.json').write_text(json.dumps(operators))
            if label == 'official':
                shutil.copyfile(args.jar, folder / 'server.jar')
                (folder / 'eula.txt').write_text('eula=true\n')
                command = [JAVA, '-Xms128M', '-Xmx1G', '-jar', 'server.jar', 'nogui']
            else:
                (config / 'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
                (config / 'skins.toml').write_text('mojang=false\nely=false\n')
                command = [str(args.binary.resolve())]
            log = (folder / 'server.log').open('w'); logs.append(log)
            process = subprocess.Popen(command, cwd=folder, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
            children.append(process)
            (folder / 'pid').write_text(str(process.pid))
            deadline = time.monotonic() + 150
            while True:
                assert process.poll() is None, (folder / 'server.log').read_text()[-1600:]
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=.3): break
                except OSError:
                    assert time.monotonic() < deadline
                    time.sleep(.3)
        def one(side):
            label, port = side
            measurements = measure(port, protocol, 'StreamProbe', args.join_only)
            if not args.join_only: measurements['completion_limits'] = completion_limits(port, protocol)
            (args.output / f'{label}.json').write_text(json.dumps(measurements, indent=2) + '\n')
            return label, measurements
        with ThreadPoolExecutor(max_workers=2) as pool:
            for label, measurements in pool.map(one, [('ours', args.ours_port)] if args.ours_only else [('official', args.official_port), ('ours', args.ours_port)]):
                result[label] = measurements
        (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2), flush=True)
    finally:
        for process in children:
            if process.poll() is None: process.terminate()
        for process in children:
            try: process.wait(timeout=30)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
        for log in logs: log.close()

if __name__ == '__main__':
    if '--review-streaming' in sys.argv:
        sys.argv.remove('--review-streaming')
        review_streaming_main()
    else:
        main()
