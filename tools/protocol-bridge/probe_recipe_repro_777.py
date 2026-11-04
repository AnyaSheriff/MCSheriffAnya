#!/usr/bin/env python3
"""Чтение всех пакетов рецептов при входе бота в отдельную копию проблемного мира."""
import argparse
import hashlib
import io
import socket
import zlib
import json
import shutil
import subprocess
import time
import uuid
from pathlib import Path
from probe_gameplay import Client, connect
from recipe_book_decoder import Decoder
from smoke import read_varint, varint


class CompressedSocket:
    def __init__(self, sock):
        self.sock = sock
        self.threshold = -1
        self.login = True
        self.buffer = b''

    def fileno(self):
        return self.sock.fileno()

    def close(self):
        self.sock.close()

    def sendall(self, framed):
        data = io.BytesIO(framed)
        length = read_varint(data.read)
        body = data.read()
        assert len(body) == length
        if self.threshold >= 0:
            body = (varint(len(body)) + zlib.compress(body)
                    if len(body) >= self.threshold else b'\0' + body)
        self.sock.sendall(varint(len(body)) + body)

    def recv(self, size):
        while not self.buffer:
            length = read_varint(self.sock.recv)
            body = b''
            while len(body) < length:
                part = self.sock.recv(length - len(body))
                if not part:
                    raise EOFError('truncated compressed frame')
                body += part
            if self.threshold >= 0:
                data = io.BytesIO(body)
                plain = read_varint(data.read)
                body = zlib.decompress(data.read()) if plain else data.read()
                assert not plain or len(body) == plain
            if self.login:
                data = io.BytesIO(body)
                ident = read_varint(data.read)
                if ident == 3:
                    self.threshold = read_varint(data.read)
                    continue
                if ident == 2:
                    self.login = False
            self.buffer = varint(len(body)) + body
        result, self.buffer = self.buffer[:size], self.buffer[size:]
        return result


def compressed_connect(*args, **kwargs):
    original = socket.create_connection
    socket.create_connection = lambda *a, **kw: CompressedSocket(original(*a, **kw))
    try:
        return connect(*args, **kwargs)
    finally:
        socket.create_connection = original


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--repro', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    root = args.output.resolve() / 'server'
    root.mkdir()
    shutil.copytree(args.repro / 'world', root / 'world')
    shutil.copytree(args.repro / 'config', root / 'config')
    props = root / 'config/server.properties'
    props.write_text(props.read_text() + '\nserver-ip=127.0.0.1\nserver-port=26291\nonline-mode=false\nnetwork-compression-threshold=256\n')
    settings = root / 'config/mcsa.properties'
    settings.write_text(settings.read_text() + '\nauth-enabled=false\noffline-license-mode=disabled\nsend-error-reports=false\nbedrock-port=19191\n')
    (root / 'config/skins.toml').write_text('mojang=false\nely=false\n')
    name = 'SheriffAnya'
    raw = bytearray(hashlib.md5(('OfflinePlayer:' + name).encode()).digest())
    raw[6] = (raw[6] & 15) | 48
    raw[8] = (raw[8] & 63) | 128
    offline = uuid.UUID(bytes=bytes(raw)).hex
    owner = '16d3c22147764fe380af6f2b651b501b'
    for old, new in [(owner, offline), (str(uuid.UUID(owner)), str(uuid.UUID(offline)))]:
        for source in list((root / 'world').rglob(old + '*')):
            shutil.copyfile(source, source.with_name(source.name.replace(old, new)))
    protocol = json.loads((args.data / 'protocol.json').read_text())
    names = {v['id']: v['name'] for v in json.loads((args.data / 'items.json').read_text())}
    report = {'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'packets': [], 'commands': []}

    class Probe(Client):
        def observe_packet(self, packet_name, payload):
            if packet_name == 'recipe_book_add':
                path = args.output / f'recipe-{len(report["packets"])}.bin'
                path.write_bytes(payload)
                try:
                    rows, replace = Decoder(payload, names).book()
                except Exception:
                    report['bad_packet'] = path.name
                    raise
                report['packets'].append({'entries': len(rows), 'replace': replace})

    log = args.output / 'server.log'
    process = subprocess.Popen([str(args.binary.resolve())], cwd=root, stdin=subprocess.PIPE,
                               stdout=log.open('w'), stderr=subprocess.STDOUT, text=True)
    client = None
    def command(text):
        report['commands'].append(text)
        process.stdin.write(text + '\n')
        process.stdin.flush()
    try:
        until = time.monotonic() + 100
        while 'Готово (' not in log.read_text():
            assert process.poll() is None and time.monotonic() < until, log.read_text()[-2000:]
            time.sleep(.1)
        client = compressed_connect(26291, protocol, Probe, login_uuid=bytes.fromhex(owner), username=name)
        client.pump(35)
        before = len(report['packets'])
        command('give SheriffAnya iron_ingot 1')
        client.pump(3)
        command('give SheriffAnya poplar_log 1')
        client.pump(3)
        report['natural_packets_after_join'] = len(report['packets']) - before
        command('recipe give SheriffAnya *')
        client.pump(5)
        command('tp SheriffAnya 136 65 152')
        client.pump(10)
        command('save-all')
        client.pump(3)
        report['connected_seconds'] = 59
        report['compression_threshold'] = client.sock.threshold
        report['no_disconnect'] = True
        client.sock.close()
        client = None
        command('stop')
        process.wait(timeout=40)
        report['first_exit'] = process.returncode
        offset = len(log.read_text())
        process = subprocess.Popen([str(args.binary.resolve())], cwd=root, stdin=subprocess.PIPE,
                                   stdout=log.open('a'), stderr=subprocess.STDOUT, text=True)
        until = time.monotonic() + 100
        while 'Готово (' not in log.read_text()[offset:]:
            assert process.poll() is None and time.monotonic() < until
            time.sleep(.1)
        client = compressed_connect(26291, protocol, Probe, login_uuid=bytes.fromhex(owner), username=name)
        client.pump(20)
        command('tp SheriffAnya 136 65 152')
        client.pump(5)
        report['restart_loaded_chunk'] = True
    except BaseException as error:
        report['error'] = repr(error)
        raise
    finally:
        if client:
            client.sock.close()
        if process.poll() is None:
            command('stop')
            try:
                process.wait(timeout=40)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=10)
        report['server_exit'] = process.returncode
        report['chunk_errors'] = log.read_text().count('Чанк 8 9 не записан')
        region = root / 'world/region/r.0.0.mcs'
        contents = region.read_bytes()
        entry = contents[8 + (8+9*32)*8:16 + (8+9*32)*8]
        report['chunk_8_9_region_entry'] = entry.hex()
        report['chunk_8_9_saved'] = entry != bytes(8)
        (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report))
    assert report['server_exit'] == 0
    assert report['chunk_errors'] == 0
    assert report['chunk_8_9_saved'] and report['restart_loaded_chunk']
    assert report['natural_packets_after_join'] > 0
    assert report['compression_threshold'] == 256


if __name__ == '__main__':
    main()
