#!/usr/bin/env python3
"""Сетевой smoke: текущий клиент допускается, будущие версии не включены случайно."""
import argparse
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time


def varint(value):
    value &= 0xffffffff
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
    raise ValueError('oversized VarInt')


def send(sock, body):
    sock.sendall(varint(len(body)) + body)


def frame(sock):
    remaining = read_varint(sock.recv)
    assert 0 < remaining < 32768
    data = bytearray()
    while remaining:
        part = sock.recv(remaining)
        if not part:
            raise EOFError('truncated frame')
        data.extend(part)
        remaining -= len(part)
    return io.BytesIO(data)


def handshake(sock, protocol, port, state):
    host = b'localhost'
    send(sock, b'\0' + varint(protocol) + varint(len(host)) + host + struct.pack('>H', port) + varint(state))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--port', type=int, default=25573)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', args.port))
    checks = []
    with tempfile.TemporaryDirectory(prefix='mcsa-protocol-smoke-') as folder:
        root = Path(folder)
        (root / 'config').mkdir()
        (root / 'config/server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'network-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\n'
            'view-distance=2\nsimulation-distance=2\n')
        (root / 'config/mcsa.properties').write_text('auth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
        log = root / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen([binary], cwd=root, stdin=subprocess.PIPE, stdout=output,
                                      stderr=subprocess.STDOUT, text=True)
            try:
                deadline = time.monotonic() + 30
                while 'Готово (' not in log.read_text():
                    assert server.poll() is None, log.read_text()
                    assert time.monotonic() < deadline, 'startup timeout'
                    time.sleep(.05)
                for protocol in [47, 340, 775, 776, 777, 778]:
                    with socket.create_connection(('127.0.0.1', args.port), timeout=5) as sock:
                        handshake(sock, protocol, args.port, 1)
                        send(sock, b'\0')
                        response = frame(sock)
                        assert read_varint(response.read) == 0
                        length = read_varint(response.read)
                        status = json.loads(response.read(length))
                        assert status['version'] == {'name': '26.1.2', 'protocol': 775}, status
                        checks.append(f'status-{protocol}-native775')
                for protocol in [47, 340, 774, 776, 777, 778]:
                    with socket.create_connection(('127.0.0.1', args.port), timeout=5) as sock:
                        handshake(sock, protocol, args.port, 2)
                        response = frame(sock)
                        assert read_varint(response.read) == 0
                        length = read_varint(response.read)
                        reason = response.read(length).decode()
                        assert '26.1.2' in reason and '775' in reason, reason
                        checks.append(f'login-{protocol}-rejected')
                with socket.create_connection(('127.0.0.1', args.port), timeout=5) as sock:
                    handshake(sock, 775, args.port, 2)
                    name = b'BridgeTest'
                    send(sock, b'\0' + varint(len(name)) + name + bytes([42]) * 16)
                    response = frame(sock)
                    assert read_varint(response.read) == 2, 'native LoginSuccess expected'
                    checks.append('login-775-success')
                server.stdin.write('stop\n')
                server.stdin.flush()
                server.wait(timeout=30)
                assert server.returncode == 0 and 'Мир сохранён' in log.read_text()
            finally:
                if server.poll() is None:
                    server.terminate()
                    try:
                        server.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
    print(json.dumps({'passed': len(checks), 'checks': checks}))


if __name__ == '__main__':
    main()
