#!/usr/bin/env python3
"""Живая проверка776 по закреплённому JSON: собственные кадры, сжатие, ответы и движение."""
import argparse
import collections
import io
import json
import math
import socket
import struct
import time
import zlib
from pathlib import Path
from probe_gameplay import mapping
from smoke import varint, read_varint


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--port', type=int, required=True)
    p.add_argument('--protocol', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--seconds', type=int, default=310)
    a = p.parse_args()
    assert 26100 <= a.port <= 26199
    protocol = json.loads(a.protocol.read_text())
    names = {s: {d: mapping(protocol, s, d) for d in ('toServer', 'toClient')} for s in ('login', 'configuration', 'play')}
    incoming = {s: {v: k for k, v in names[s]['toClient'].items()} for s in names}
    report = dict(version=776, status='failed', packets=collections.Counter(), sent=collections.Counter(), moves=0, jumps=0)
    with socket.create_connection(('127.0.0.1', a.port), timeout=10) as sock:
        threshold = -1
        def send(body):
            data = body
            if threshold >= 0:
                data = varint(len(body)) + zlib.compress(body) if len(body) >= threshold else b'\0' + body
            sock.sendall(varint(len(data)) + data)
        def named(state, name, body=b''):
            report['sent'][name] += 1
            send(varint(names[state]['toServer'][name]) + body)
        def exact(n):
            out = bytearray()
            while len(out) < n:
                part = sock.recv(n - len(out))
                if not part:
                    raise EOFError('server disconnected')
                out.extend(part)
            return bytes(out)
        def read(state):
            n = read_varint(lambda n: exact(n))
            assert 0 < n <= 2097151
            data = exact(n)
            if threshold >= 0:
                stream = io.BytesIO(data)
                size = read_varint(stream.read)
                data = zlib.decompress(stream.read()) if size else stream.read()
                assert not size or len(data) == size
            stream = io.BytesIO(data)
            number = read_varint(stream.read)
            name = incoming[state][number]
            report['packets'][state + ':' + name] += 1
            assert name not in ('disconnect', 'kick_disconnect', 'start_configuration'), name
            return name, stream.read()
        host = b'localhost'
        send(b'\0' + varint(776) + varint(len(host)) + host + struct.pack('>H', a.port) + b'\2')
        named('login', 'login_start', b'\7Live776' + bytes(16))
        while True:
            name, payload = read('login')
            if name == 'compress':
                threshold = read_varint(io.BytesIO(payload).read)
            elif name == 'success':
                named('login', 'login_acknowledged')
                break
            else:
                raise AssertionError(name)
        named('configuration', 'settings', b'\5en_us\2\0\1\x7f\1\0\1\0')
        while True:
            name, payload = read('configuration')
            if name == 'finish_configuration':
                named('configuration', name)
                break
            if name in ('keep_alive', 'select_known_packs'):
                named('configuration', name, payload)
            if name == 'ping':
                named('configuration', 'pong', payload)
        sock.settimeout(10)
        start = time.monotonic()
        next_move = start
        pose = None
        # Ответы разбираются непрерывно; socket timeout не отменяет начатое чтение кадра.
        import select
        while time.monotonic() - start < a.seconds:
            now = time.monotonic()
            if pose and now >= next_move:
                phase = now - start
                x, y, z = pose
                jump = max(0, math.sin(phase * 3)) * 1.2
                named('play', 'position_look', struct.pack('>dddffB', x + math.sin(phase) * 2, y + jump, z + math.cos(phase) * 2, 0, 0, int(jump == 0)))
                report['moves'] += 1
                report['jumps'] += int(jump > 0)
                next_move = now + .05
            if not select.select([sock], [], [], .02)[0]:
                continue
            name, payload = read('play')
            if name == 'keep_alive':
                named('play', name, payload)
            elif name == 'login':
                named('play', 'player_loaded')
            elif name == 'position':
                stream = io.BytesIO(payload)
                teleport = read_varint(stream.read)
                values = struct.unpack('>ddddddffI', stream.read())
                pose = values[:3]
                named('play', 'teleport_confirm', varint(teleport))
            elif name == 'chunk_batch_finished':
                named('play', 'chunk_batch_received', struct.pack('>f', 64))
        report.update(status='passed', seconds=time.monotonic() - start, compression_threshold=threshold)
        assert report['sent']['keep_alive'] > 30 and report['moves'] > 100 and report['jumps'] > 100
    a.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ('sent', 'packets')}))


if __name__ == '__main__':
    main()
