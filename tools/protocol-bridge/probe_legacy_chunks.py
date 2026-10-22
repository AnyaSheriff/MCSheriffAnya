#!/usr/bin/env python3
"""Запись только построенных нами секций 1.12.2; природный мир не выгружается."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_legacy_session import Probe
from smoke import handshake, read_varint, send


def sections(payload):
    data = io.BytesIO(payload)
    x, z, full = struct.unpack('>iiB', data.read(9))
    assert full in (0, 1)
    mask = read_varint(data.read)
    assert 0 <= mask <= 65535
    size = read_varint(data.read)
    packed = io.BytesIO(data.read(size))
    assert len(packed.getvalue()) == size
    result = []
    for y in range(16):
        if not mask & (1 << y):
            continue
        start = packed.tell()
        width = packed.read(1)[0]
        assert 4 <= width <= 8 or width == 13, width
        count = read_varint(packed.read)
        palette = [read_varint(packed.read) for _ in range(count)]
        assert (0 < count <= (1 << width)) if width <= 8 else count == 0
        length = read_varint(packed.read)
        assert length == width * 64
        words = [int.from_bytes(packed.read(8), 'big') for _ in range(length)]
        blocks = []
        for i in range(4096):
            at = i * width
            value = words[at // 64] >> (at % 64)
            if at % 64 + width > 64:
                value |= words[at // 64 + 1] << (64 - at % 64)
            value &= (1 << width) - 1
            blocks.append(palette[value] if palette else value)
        block_light, sky_light = packed.read(2048), packed.read(2048)
        assert len(block_light) == len(sky_light) == 2048
        result.append({'y': y, 'width': width, 'palette': palette, 'blocks': blocks,
                       'bytes': packed.getvalue()[start:packed.tell()].hex()})
    biomes = packed.read(256) if full else b''
    assert len(biomes) == (256 if full else 0) and not packed.read()
    # Не сохраняем NBT окружающего мира. Тестовые секции не содержат block entities.
    return x, z, bool(full), result


class ChunkProbe(Probe):
    def __init__(self, *args):
        super().__init__(*args)
        self.expected = None
        self.cases = []
        self.next_move = 0

    def send(self, name, body=b''):
        if name == 'position_look':
            self.next_move = time.monotonic() + .1
            return
        super().send(name, body)

    def tick(self):
        if self.position is not None and time.monotonic() >= self.next_move:
            super().send('position_look', struct.pack('>dddffB', *self.position, 0))
            self.next_move = time.monotonic() + .05

    def observe_packet(self, name, payload):
        if name == 'map_chunk':
            self.report.setdefault('chunk_headers', []).append(list(struct.unpack('>iiB', payload[:9])))
        if name != 'map_chunk' or self.expected is None or struct.unpack('>ii', payload[:8]) != (0, 0):
            return
        x, z, full, decoded = sections(payload)
        if not full:
            return
        for section in decoded:
            y = section['y']
            if y not in self.expected:
                continue
            differences = [(i, old, new) for i, (old, new) in enumerate(zip(self.expected[y], section['blocks'])) if old != new]
            assert not differences, ('authored blocks differ', y, differences[:20])
            self.cases.append({'section_y': y, 'width': section['width'], 'unique_states': len(set(section['blocks'])),
                               'section_hex': section['bytes'], 'expected_prefix': section['blocks'][:300],
                               'remaining_state': 16, 'has_sky_light': True})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--jar', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int, default=26023)
    p.add_argument('--snowy', action='store_true', help='Проверить metadata травы/подзола/мицелия под снегом')
    a = p.parse_args()
    with a.jar.open('rb') as f:
        assert hashlib.file_digest(f, 'sha1').hexdigest() == '886945bfb2b978778c3a0288fd7fab09d315b25f'
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    legacy = json.loads((a.data / 'legacy.json').read_text())['blocks']
    names = {b['id']: b['name'] for b in json.loads((a.data / 'blocks.json').read_text())}
    # Твёрдые стабильные блоки: без растений, падающего песка, redstone и NBT.
    allowed = {1, 3, 4, 5, 17, 20, 24, 35, 43, 44, 53, 67, 95, 98, 108, 109,
               114, 126, 128, 134, 135, 136, 156, 159, 160, 161, 162, 163,
               164, 168, 179, 180, 182, 201, 202, 203, 204, 205, 251, *range(235, 251)}
    states = sorted({int(k.split(':')[0]) * 16 + int(k.split(':')[1]) for k in legacy
                     if int(k.split(':')[0]) in allowed} - {16})
    assert len(states) >= 299, len(states)
    a.output.mkdir(parents=True, exist_ok=True)
    report = {'protocol': 340, 'checks': [], 'packets': [], 'cases': []}
    with socket.socket() as check:
        check.bind(('127.0.0.1', a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-chunks-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=FLAT\nlevel-seed=1\nview-distance=2\ngamemode=1\nspawn-protection=0\n')
        log = a.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui'], cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            sock = None
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                command('gamerule randomTickSpeed 0')
                command('gamerule doMobSpawning false')
                command('setworldspawn 0 64 0')
                sock = socket.create_connection(('127.0.0.1', a.port), timeout=10)
                handshake(sock, 340, a.port, 2)
                send(sock, b'\0\x09McsaProbe')
                client = ChunkProbe(sock, protocol, report)
                client.pump(20, lambda r: r['name'] == 'position')
                command('tp McsaProbe 8 180 8')
                client.pump(10, lambda r: r['name'] == 'position')
                client.pump(3)
                expected = {}
                if a.snowy:
                    command('fill 0 64 0 15 79 15 air')
                    command('fill 0 64 0 15 69 15 stone')
                    values = [16] * (6 * 256) + [0] * (10 * 256)
                    for row, (block, meta) in enumerate([(2, 0), (3, 2), (110, 0)]):
                        for column, (above, snow_meta) in enumerate([(0, 0), (78, 0), (80, 0)]):
                            x, z = 2 + column * 4, 2 + row * 4
                            command(f'setblock {x} 70 {z} {names[block]} {meta}')
                            values[6 * 256 + z * 16 + x] = block * 16 + meta
                            if above:
                                command(f'setblock {x} 71 {z} {names[above]} {snow_meta}')
                                values[7 * 256 + z * 16 + x] = above * 16 + snow_meta
                    expected[4] = values
                else:
                    for section_y, count in enumerate([1, 16, 17, 33, 65, 129, 300], start=4):
                        y = section_y * 16
                        command(f'fill 0 {y} 0 15 {y+15} 15 stone')
                        values = [16] * 4096
                        for i, state in enumerate(states[:count - 1]):
                            command(f'setblock {i%16} {y+i//256} {(i//16)%16} {names[state//16]} {state%16}')
                            values[i] = state
                        expected[section_y] = values
                command('say MCSA_CHUNKS_READY')
                deadline = time.monotonic() + 60
                while 'MCSA_CHUNKS_READY' not in log.read_text():
                    assert time.monotonic() < deadline
                    client.pump(.1)
                client.pump(1)
                command('tp McsaProbe 1000 65 1000')
                client.pump(10, lambda r: r['name'] == 'position')
                client.pump(2)
                client.expected = expected
                command('tp McsaProbe 8 180 8')
                client.pump(10, lambda r: r['name'] == 'position')
                client.pump(5)
                report['cases'] = client.cases
                assert len(client.cases) == (1 if a.snowy else 7), len(client.cases)
                widths = [c['width'] for c in client.cases]
                assert set(widths) == ({4} if a.snowy else {4, 5, 6, 7, 8, 13}), widths
                report['checks'].append('snowy-neighbours-nine-cases' if a.snowy else 'seven-authored-sections-all-palette-widths')
            except BaseException as error:
                report['error'] = str(error)
                raise
            finally:
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
    print(json.dumps({'checks': report['checks'], 'widths': [c['width'] for c in report['cases']]}))


if __name__ == '__main__':
    main()
