#!/usr/bin/env python3
"""Проверка всех ID палитр собственным recorder777; экспорт Rust остаётся непрозрачным пакетом."""
import argparse
import gzip
from functools import lru_cache
import io
import json
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'blackbox'))
from record777 import Recorder
from analyze777 import Decoder
from probe_gameplay import connect, mapping
from probe_registry_client import packet, integer, send, string
from smoke import read_varint


def exact(data, count):
    result = data.read(count)
    if len(result) != count:
        raise ValueError('truncated palette')
    return result


@lru_cache(maxsize=65536)
def valid_indices(word, width, size):
    mask = (1 << width) - 1
    return all((word >> shift) & mask < size for shift in range(0, (64 // width) * width, width))


def palette(data, count, minimum, maximum, direct, known):
    width = exact(data, 1)[0]
    if width == 0:
        ids = [read_varint(data.read)]
        if ids[0] not in known:
            raise ValueError(f'unknown palette id {ids[0]}')
        return len(ids)
    indirect = minimum <= width <= maximum
    if not indirect and (width != direct or width <= maximum):
        raise ValueError(f'invalid palette width {width}')
    ids = []
    if indirect:
        size = read_varint(data.read)
        if not 1 <= size <= 1 << width:
            raise ValueError('invalid palette size')
        ids = [read_varint(data.read) for _ in range(size)]
        for value in ids:
            if value not in known:
                raise ValueError(f'unknown palette id {value}')
    words = exact(data, ((count + 64 // width - 1) // (64 // width)) * 8)
    # Все записи палитры, включая неиспользованные, проверены выше.
    # Прямой контейнер не имеет списка: проверяем каждую его ячейку.
    if indirect and len(ids) != 1 << width:
        for (word,) in struct.iter_unpack('>Q', words):
            if not valid_indices(word, width, len(ids)):
                raise ValueError('invalid palette index')
    if not indirect:
        left = count
        mask = (1 << width) - 1
        for (word,) in struct.iter_unpack('>Q', words):
            for shift in range(min(left, 64 // width)):
                value = (word >> (shift * width)) & mask
                if value not in known:
                    raise ValueError(f'unknown direct id {value}')
            left -= min(left, 64 // width)
    return len(ids) if indirect else count


def chunk(payload, biomes, blocks):
    data = io.BytesIO(payload)
    x, z = struct.unpack('>ii', exact(data, 8))
    for _ in range(read_varint(data.read)):
        read_varint(data.read)
        exact(data, read_varint(data.read) * 8)
    sections = io.BytesIO(exact(data, read_varint(data.read)))
    ids = 0
    for _ in range(24):
        solid, fluid = struct.unpack('>HH', exact(sections, 4))
        assert solid <= 4096 and fluid <= 4096
        ids += palette(sections, 4096, 4, 8, 16, blocks)
        ids += palette(sections, 64, 1, 3, max(1, (len(biomes) - 1).bit_length()), range(len(biomes)))
    assert not sections.read(1), 'trailing section bytes'
    return (x, z), ids


class CheckedRecorder(Recorder):
    def __init__(self, sock, protocol, registries, blocks):
        super().__init__(sock, protocol)
        self.registry_names = registries
        self.blocks = blocks
        self.chunks = set()
        self.palette_ids = 0
        self.done = False

    def observe_packet(self, name, payload):
        if name in ('kick_disconnect', 'disconnect'):
            raise ValueError(f'disconnected: {payload!r}')
        if name == 'map_chunk':
            pos, count = chunk(payload, self.registry_names['minecraft:worldgen/biome'], self.blocks)
            self.chunks.add(pos)
            self.palette_ids += count
        if name == 'keep_alive' and payload == b'PALTEST!':
            self.done = True
        super().observe_packet(name, payload)


def configuration(protocol, registries):
    decoder = Decoder(protocol)
    decoder.types.update(protocol['configuration']['toClient']['types'])
    registry_id = mapping(protocol, 'configuration', 'toClient')['registry_data']
    def receive(sock, kind, payload):
        if kind == registry_id:
            data = io.BytesIO(payload)
            row = decoder.read(data, 'packet_registry_data')
            assert not data.read(1)
            registries[row['id']] = [entry['key'] for entry in row['entries']]
        return False
    return receive


def replay(folder, protocol, blocks, port, expected=12):
    report = []
    for path in sorted(folder.glob('*.frames.gz')):
        errors = []
        with socket.socket() as listener:
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(('127.0.0.1', port))
            listener.listen(1)
            def serve():
                try:
                    with listener.accept()[0] as peer:
                        peer.settimeout(120)
                        packet(peer)
                        packet(peer)
                        send(peer, b'\x02' + bytes(16) + string('PaletteProbe') + b'\0')
                        packet(peer)
                        packet(peer)
                        known = mapping(protocol, 'configuration', 'toClient')['select_known_packs']
                        send(peer, bytes([known]) + b'\x01' + string('minecraft') + string('core') + string('26.3'))
                        packet(peer)
                        peer.sendall((folder / 'configuration.frames').read_bytes())
                        send(peer, b'\x03')
                        packet(peer)
                        with gzip.open(path, 'rb') as source:
                            while data := source.read(65536):
                                peer.sendall(data)
                        keep = mapping(protocol, 'play', 'toClient')['keep_alive']
                        send(peer, bytes([keep]) + b'PALTEST!')
                        while peer.recv(65536):
                            pass
                except Exception as error:
                    errors.append(repr(error))
            worker = threading.Thread(target=serve)
            worker.start()
            registries = {}
            client = connect(port, protocol, lambda s, p: CheckedRecorder(s, p, registries, blocks),
                             username='PaletteProbe', configuration_handler=configuration(protocol, registries))
            try:
                client.pump(120, lambda _: client.done)
                assert client.done and len(client.chunks) >= 1089
                report.append(dict(fixture=path.name, chunks=len(client.chunks), palette_ids=client.palette_ids,
                                   biomes=len(registries['minecraft:worldgen/biome'])))
            finally:
                client.sock.close()
                worker.join(timeout=10)
            assert not worker.is_alive() and not errors, errors
        print(json.dumps(report[-1]), flush=True)
    assert len(report) == expected
    return report


def live(binary, world, protocol, blocks, port, output):
    with tempfile.TemporaryDirectory(prefix='s26-b777-live-') as directory:
        root = Path(directory)
        shutil.copytree(world, root / 'world')
        (root / 'config').mkdir()
        (root / 'config/server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\n'
            'network-compression-threshold=-1\nlevel-seed=1792855987123998292\nview-distance=2\nsimulation-distance=2\ngamemode=creative\n')
        (root / 'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
        with (output / 'live.log').open('w') as log:
            server = subprocess.Popen([str(binary.resolve())], cwd=root, stdin=subprocess.PIPE, stdout=log, stderr=log, text=True)
            client = None
            try:
                deadline = time.monotonic() + 120
                while 'Готово (' not in (output / 'live.log').read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                registries = {}
                client = connect(port, protocol, lambda s, p: CheckedRecorder(s, p, registries, blocks),
                                 username='PaletteProbe', configuration_handler=configuration(protocol, registries))
                client.pump(30, lambda _: client.position is not None)
                server.stdin.write('tp PaletteProbe 56 140 -40\n')
                server.stdin.flush()
                client.pump(60, lambda _: (3, -3) in client.chunks)
                assert (3, -3) in client.chunks
                assert len(registries['minecraft:worldgen/biome']) == 67
                return dict(chunks=len(client.chunks), palette_ids=client.palette_ids, biomes=67, reported_chunk_loaded=True)
            finally:
                if client:
                    client.sock.close()
                if server.poll() is None:
                    server.stdin.write('stop\n')
                    server.stdin.flush()
                    try:
                        server.wait(timeout=60)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26184)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--world', type=Path)
    args = parser.parse_args()
    assert 26100 <= args.port <= 26199
    args.output.mkdir(parents=True, exist_ok=True)
    protocol = json.loads((args.data / 'protocol.json').read_text())
    blocks = {state for block in json.loads((args.data / 'blocks.json').read_text())
              for state in range(block['minStateId'], block['maxStateId'] + 1)}
    report = {'replay': replay(args.fixtures, protocol, blocks, args.port)}
    if args.binary:
        assert args.world
        report['live'] = live(args.binary, args.world, protocol, blocks, args.port, args.output)
    (args.output / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
