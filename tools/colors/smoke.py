#!/usr/bin/env python3
"""Наш клиент Java и отдельные pipe/PTY серверы; только временные миры/игроки."""
import argparse
import io
import json
import os
from pathlib import Path
import pty
import select
import socket
import struct
import subprocess
import tempfile
import threading
import time


def varint(value):
    result = bytearray()
    while value > 127:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def read_varint(read):
    value = 0
    for shift in range(0, 35, 7):
        byte = read(1)
        if not byte:
            raise EOFError('missing VarInt')
        value |= (byte[0] & 127) << shift
        if byte[0] < 128:
            return value
    raise ValueError('invalid VarInt')


def string(text):
    data = text.encode()
    return varint(len(data)) + data


def send(sock, packet_id, data=b''):
    body = varint(packet_id) + data
    sock.sendall(varint(len(body)) + body)


def frame(sock):
    length = read_varint(sock.recv)
    assert 0 < length < 8 * 1024 * 1024
    data = bytearray()
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            raise EOFError('truncated frame')
        data.extend(chunk)
    data = io.BytesIO(data)
    return read_varint(data.read), data.read()


def nbt_string(data):
    length = struct.unpack('>H', data.read(2))[0]
    value = data.read(length).decode('utf-8', 'surrogatepass')
    return value.encode('utf-16', 'surrogatepass').decode('utf-16')


def nbt(data, kind=None):
    if kind is None:
        kind = data.read(1)[0]
    if kind == 1:
        return data.read(1)[0] != 0
    if kind == 8:
        return nbt_string(data)
    if kind == 9:
        child = data.read(1)[0]
        count = struct.unpack('>i', data.read(4))[0]
        assert 0 <= count < 1024
        return [nbt(data, child) for _ in range(count)]
    if kind == 10:
        fields = {}
        while True:
            kind = data.read(1)[0]
            if kind == 0:
                return fields
            key = nbt_string(data)
            fields[key] = nbt(data, kind)
    raise AssertionError(f'unexpected NBT {kind}')


def content(component):
    if isinstance(component, str):
        return component
    return component.get('text', '') + ''.join(content(child) for child in component.get('extra', []))


def children(component):
    return [component] if isinstance(component, str) else [component] + component.get('extra', [])


class Client:
    def __init__(self, port):
        self.sock = socket.create_connection(('127.0.0.1', port), timeout=8)
        send(self.sock, 0, varint(775) + string('localhost') + struct.pack('>H', port) + varint(2))
        send(self.sock, 0, string('ColorProbe') + bytes([42]) * 16)
        assert frame(self.sock)[0] == 2
        send(self.sock, 3)
        while True:
            packet, payload = frame(self.sock)
            if packet == 14:
                send(self.sock, 7, payload)
            elif packet == 3:
                send(self.sock, 3)
                break
        self.messages = []
        self.positioned = False
        self.pump(lambda: self.positioned)

    def pump(self, done):
        deadline = time.monotonic() + 10
        while not done():
            assert time.monotonic() < deadline, 'client timeout'
            if not select.select([self.sock], [], [], .05)[0]:
                send(self.sock, 0x0d)
                continue
            packet, payload = frame(self.sock)
            if packet == 0x48:
                teleport = read_varint(io.BytesIO(payload).read)
                send(self.sock, 0, varint(teleport))
                send(self.sock, 0x2c)
                self.positioned = True
            elif packet == 0x2c:
                send(self.sock, 0x1c, payload)
            elif packet == 0x0b:
                send(self.sock, 0x0b, struct.pack('>f', 64))
            elif packet == 0x79:
                self.messages.append(nbt(io.BytesIO(payload)))

    def message(self, expected):
        self.pump(lambda: any(content(c) == expected for c in self.messages))
        return next(c for c in self.messages if content(c) == expected)

    def chat(self, text):
        # protocol775: text, timestamp/salt, optional signature, offset,
        # acknowledged bitmap and checksum. No borrowed protocol-library code.
        send(self.sock, 9, string(text) + struct.pack('>qq', int(time.time() * 1000), 0) + bytes(6))


class Server:
    def __init__(self, binary, port, root, mode, terminal=False, no_color=False, dumb=False):
        config = root / 'config'
        config.mkdir()
        (config / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\n'
            'network-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\n'
            'view-distance=2\nsimulation-distance=2\n')
        (config / 'mcsa.properties').write_text(
            f'console-colors={mode}\nauth-enabled=false\nauth-premium=false\n'
            'bedrock-port=0\nprefetch-chunks=0\n')
        (config / 'skins.toml').write_text('mojang=false\nely=false\n')
        env = dict(os.environ)
        env.pop('NO_COLOR', None)
        env['TERM'] = 'dumb' if dumb else 'xterm-256color'
        if no_color:
            env['NO_COLOR'] = '1'
        self.master = None
        self.data = bytearray()
        if terminal:
            self.master, slave = pty.openpty()
            self.proc = subprocess.Popen([binary], cwd=root, env=env, stdin=slave, stdout=slave, stderr=slave)
            os.close(slave)
            self.reader_fd = self.master
        else:
            self.proc = subprocess.Popen([binary], cwd=root, env=env, stdin=subprocess.PIPE,
                                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            self.reader_fd = self.proc.stdout.fileno()
        self.thread = threading.Thread(target=self.capture, daemon=True)
        self.thread.start()
        try:
            self.wait_for('Готово (')
        except BaseException:
            self.proc.terminate()
            self.proc.wait(timeout=10)
            self.thread.join(timeout=2)
            if self.master is not None:
                os.close(self.master)
            raise

    def capture(self):
        try:
            while True:
                chunk = os.read(self.reader_fd, 65536)
                if not chunk:
                    return
                self.data.extend(chunk)
        except OSError:
            pass  # PTY returns EIO after the slave closes.

    def text(self):
        return bytes(self.data).decode(errors='replace')

    def wait_for(self, text):
        deadline = time.monotonic() + 30
        while text not in self.text():
            assert self.proc.poll() is None, self.text()
            assert time.monotonic() < deadline, self.text()
            time.sleep(.03)

    def command(self, command):
        data = (command + '\n').encode()
        if self.master is None:
            self.proc.stdin.write(data)
            self.proc.stdin.flush()
        else:
            os.write(self.master, data)

    def stop(self):
        if self.proc.poll() is None:
            self.command('stop')
            self.proc.wait(timeout=30)
        self.thread.join(timeout=2)
        if self.master is not None:
            os.close(self.master)
        assert self.proc.returncode == 0 and 'Мир сохранён' in self.text(), self.text()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--port', type=int, default=26005)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    checks = []
    for mode, terminal, no_color, dumb, enabled in [
        ('always', False, False, False, True), ('never', False, False, False, False),
        ('auto', False, False, False, False), ('auto', True, False, False, True),
        ('auto', True, True, False, False), ('auto', True, False, True, False),
    ]:
        with tempfile.TemporaryDirectory(prefix='mcsa-colors-') as folder:
            server = Server(binary, args.port, Path(folder), mode, terminal, no_color, dumb)
            try:
                if mode == 'always':
                    client = Client(args.port)
                    try:
                        cases = [('&a&lЗелёный&r обычный', 'Зелёный обычный', '#55ff55', 'Зелёный'),
                                 ('&#12AbEfHEX', 'HEX', '#12abef', 'HEX'),
                                 ('§bГолубой', 'Голубой', '#55ffff', 'Голубой'),
                                 ('&&a буквально', '&a буквально', None, None)]
                        for source, plain, color, colored in cases:
                            server.command('say ' + source)
                            component = client.message('[Server] ' + plain)
                            if color:
                                assert any(isinstance(c, dict) and c.get('color') == color and c.get('text') == colored
                                           for c in children(component)), component
                            if colored == 'Зелёный':
                                runs = children(component)
                                assert any(isinstance(c, dict) and c.get('text') == 'Зелёный' and c.get('bold') for c in runs)
                                assert any(isinstance(c, dict) and c.get('text') == ' обычный' and c.get('bold') is False for c in runs)
                            checks.append('java-say-' + plain)
                        client.chat('&cПривет 😀')
                        component = client.message('<ColorProbe> Привет 😀')
                        assert any(isinstance(c, dict) and c.get('color') == '#ff5555' and c.get('text') == 'Привет 😀'
                                   for c in children(component)), component
                        checks.append('java-player-chat')
                    finally:
                        client.sock.close()
                server.command('say &aCOLOR_READY&r')
                server.wait_for('COLOR_READY')
                time.sleep(.3)
            finally:
                server.stop()
            log = server.text()
            assert ('\x1b[92mINFO' in log) == enabled, log
            assert ('\x1b[38;2;85;255;85mCOLOR_READY' in log) == enabled, log
            assert log.count(']: [Server] COLOR_READY') == (0 if enabled else 1), log
            checks.append(f'console-{mode}-tty{terminal}-no_color{no_color}-dumb{dumb}')
    print(json.dumps({'passed': len(checks), 'checks': checks}, ensure_ascii=False))


if __name__ == '__main__':
    main()
