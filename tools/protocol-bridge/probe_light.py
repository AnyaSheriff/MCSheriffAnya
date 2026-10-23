#!/usr/bin/env python3
"""Свет от установленного/удалённого нами факела в двух официальных серверах.

Только серверные JAR как непрозрачные программы, без GUI, распаковки и отчётов.
Записываются только LightData тестового чанка; реестры/мир не сохраняются.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import socket
import subprocess
import tempfile
import time

from probe_gameplay import Client, connect, mapping
from smoke import read_varint, varint


def signed(value):
    return value - (1 << 32) if value & (1 << 31) else value


def decode(payload, version):
    data = io.BytesIO(payload)
    position = [signed(read_varint(data.read)), signed(read_varint(data.read))]
    masks = []
    for _ in range(4):
        count = read_varint(data.read)
        assert count <= 40
        if version in (774, 775, 776):
            words = [int.from_bytes(data.read(8), 'big') for _ in range(count)]
            mask = sum(word << (i * 64) for i, word in enumerate(words))
        else:
            mask = int.from_bytes(data.read(count), 'little')
        masks.append(mask)
    arrays = []
    for mask in masks[:2]:
        count = read_varint(data.read)
        assert count == mask.bit_count()
        values = []
        for _ in range(count):
            assert read_varint(data.read) == 2048
            value = data.read(2048)
            assert len(value) == 2048
            values.append(value)
        arrays.append(values)
    assert data.read() == b''
    assert not (masks[0] & masks[2] or masks[1] & masks[3])
    # Факел (-16,65,-16): чанк(-1,-1), секция8 в мире(-64..319), бит9 в свете.
    bit = 1 << 9
    level = None
    if masks[3] & bit:
        level = 0
    elif masks[1] & bit:
        index = (masks[1] & (bit - 1)).bit_count()
        level = arrays[1][index][128] & 15  # local y=1,z=0,x=0: nibble256.
    return {'chunk': position, 'masks': masks, 'torch_cell_level': level, 'block_peaks': [max(max(b & 15, b >> 4) for b in a) for a in arrays[1]], 'raw_masks_length':len(payload)}


class LightClient(Client):
    def __init__(self, sock, protocol, version, expected_section=None):
        super().__init__(sock, protocol)
        self.version = version
        self.lights = []
        self.light_headers = []
        self.chunk_ready = False
        self.expected_section = expected_section
        self.authored_sections = []
        self.block_entity_counts = []

    def send(self, name, payload=b''):
        if name == 'tick_end' and self.position is not None:
            super().send('position', struct.pack('>dddB', *self.position[:3], 0))
        super().send(name, payload)

    def confirm_teleport(self, teleport, values):
        payload = varint(teleport)
        if self.version == 777:
            payload += struct.pack('>dddff', *values)
        self.send('teleport_confirm', payload)

    def observe_packet(self, name, payload):
        if name == 'map_chunk' and struct.unpack('>ii', payload[:8]) == (-1, -1):
            self.chunk_ready = True
            data = io.BytesIO(payload[8:])
            for _ in range(read_varint(data.read)):
                read_varint(data.read)
                data.read(read_varint(data.read) * 8)
            sections = data.read(read_varint(data.read))
            if self.expected_section is not None:
                from probe_776_payloads import authored_section
                self.authored_sections.append(authored_section(sections, self.expected_section))
            entity_count = read_varint(data.read)
            self.block_entity_counts.append(entity_count)
            assert entity_count == 0, ('unexpected block entity in test chunk', entity_count)
            light = data.read()
            observed = decode(varint(-1) + varint(-1) + light, self.version)
            observed['source_packet'] = 'map_chunk'
            observed['light_hex'] = light.hex()
            self.lights.append(observed)
        if name == 'update_light':
            observed = decode(payload, self.version)
            self.light_headers.append({**observed, 'prefix':payload[:20].hex()})
            if observed['chunk'] == [-1, -1]:
                observed['source_packet'] = 'update_light'
                observed['light_hex'] = payload[10:].hex()
                observed['body_hex'] = (varint(mapping(self.protocol, 'play', 'toClient')[name]) + payload).hex()
                self.lights.append(observed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', type=int, choices=[775, 776, 777], required=True)
    parser.add_argument('--port', type=int, default=26015)
    parser.add_argument('--authored-section', action='store_true', help='только776: секция300 устойчивых общих состояний')
    import official_26_2_inputs
    official_26_2_inputs.arguments(parser)
    args = parser.parse_args()
    expected = {775: '97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51', 777: '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}
    provenance = None
    if args.version == 776:
        protocol, _, provenance = official_26_2_inputs.load(args)
        expected[776] = provenance['jar_sha1']
    else:
        assert args.data is not None
        with args.jar.open('rb') as source:
            assert hashlib.file_digest(source, 'sha1').hexdigest() == expected[args.version]
        path = args.data / ('protocol.json' if args.version == 777 else 'protocol-26.1.json')
        protocol = json.loads(path.read_text())
        manifest = json.loads((args.data / 'sources.json').read_text())
        assert hashlib.sha256(path.read_bytes()).hexdigest() == next(f['sha256'] for f in manifest['files'] + manifest['baseline_files'] if f['file'] == path.name)
    section_commands, expected_section = [], None
    if args.authored_section:
        assert args.version == 776
        from probe_776_payloads import authored_states
        section_commands, expected_section = authored_states(args.registry_cache)
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    args.output.mkdir(parents=True, exist_ok=True)
    report = {'protocol': args.version, 'jar_sha1': expected[args.version], 'cases': []}
    if provenance is not None: report['provenance'] = provenance
    report['commands'] = []
    with tempfile.TemporaryDirectory(prefix='mcsa-light-probe-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'white-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'gamemode=creative\nmax-players=2\n')
        log = args.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(args.jar.resolve()), 'nogui'],
                                      cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(value):
                report['commands'].append(value)
                server.stdin.write(value + '\n')
                server.stdin.flush()
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                command('gamerule minecraft:spawn_mobs false')
                command('gamerule minecraft:random_tick_speed 0')
                command('forceload add -16 -16')
                for value in section_commands:
                    command(value)
                command('say MCSA_776_LIGHT_READY')
                deadline = time.monotonic() + 45
                while 'MCSA_776_LIGHT_READY' not in log.read_text():
                    assert time.monotonic() < deadline, 'authored section commands incomplete'
                    time.sleep(.1)
                command('setblock -16 64 -16 minecraft:stone')
                for name, block, level in [('placed', 'torch', 14), ('removed', 'air', 0)]:
                    # Свежий вход проверяет фактический LightData полного чанка.
                    # Доставка инкрементальных обновлений — отдельная проверка.
                    command('setblock -16 65 -16 minecraft:' + block)
                    time.sleep(1)
                    client = connect(args.port, protocol, lambda sock, data: LightClient(sock, data, args.version, expected_section), version=args.version)
                    client.protocol = protocol
                    deadline = time.monotonic() + 15
                    while not any(r['torch_cell_level'] == level for r in client.lights):
                        assert time.monotonic() < deadline, f'no {name} light data'
                        client.pump(.2)
                    record = next(r for r in client.lights if r['torch_cell_level'] == level)
                    report['cases'].append({'case': name, **record})
                    if expected_section is not None:
                        assert client.authored_sections, 'authored section missing'
                        report.setdefault('authored_sections', []).append({'case': name, **client.authored_sections[-1]})
                    client.sock.close()
                    client = None
                    time.sleep(.5)
            except Exception as error:
                report['error'] = repr(error)
                if client:
                    report['authored_sections_before_failure'] = client.authored_sections
                    report['block_entity_counts'] = client.block_entity_counts
                    report['debug_lights'] = client.lights[-12:]
                    report['light_headers'] = client.light_headers[-30:]
                    report['packet_names'] = sorted(set(r['name'] for r in client.records))
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
                report['exit_code'] = server.returncode
                (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'version': args.version, 'cases': len(report['cases']), 'exit_code': report['exit_code']}))


if __name__ == '__main__':
    main()
