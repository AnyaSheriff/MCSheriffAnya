#!/usr/bin/env python3
"""Наблюдение Login/Configuration официального сервера без чтения JAR/таблиц мира.

Старые номера пакетов используются как проверяемые гипотезы. Сохраняются только
ID, размеры и хеши пакетов; содержимое реестров/датапаков не выгружается.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time

from smoke import varint, read_varint, send, handshake


def frame(sock):
    length = read_varint(sock.recv)
    if not 0 < length <= 8 * 1024 * 1024:
        raise ValueError('invalid packet length')
    data = bytearray()
    while len(data) < length:
        part = sock.recv(length - len(data))
        if not part:
            raise EOFError('truncated packet')
        data.extend(part)
    return bytes(data)


def observe(port, protocol, records):
    with socket.create_connection(('127.0.0.1', port), timeout=10) as sock:
        handshake(sock, protocol, port, 1)
        send(sock, b'\0')
        packet = io.BytesIO(frame(sock))
        assert read_varint(packet.read) == 0
        status = json.loads(packet.read(read_varint(packet.read)))
        records.append({'phase': 'status', 'version': status['version']})
    with socket.create_connection(('127.0.0.1', port), timeout=10) as sock:
        handshake(sock, protocol, port, 2)
        name = b'McsaProbe'
        send(sock, b'\0' + varint(len(name)) + name + bytes([37]) * 16)
        phase = 'login'
        for _ in range(160):
            raw = frame(sock)
            packet = io.BytesIO(raw)
            packet_id = read_varint(packet.read)
            payload = packet.read()
            records.append({'phase': phase, 'id': packet_id, 'size': len(payload),
                            'sha256': hashlib.sha256(payload).hexdigest()})
            if phase == 'login':
                if packet_id != 2:
                    raise ValueError(f'LoginSuccess hypothesis failed: id={packet_id}')
                send(sock, b'\x03')
                phase = 'configuration'
                send(sock, b'\x00\x05en_us\x02\x00\x01\x7f\x01\x00\x01\x00')
            elif phase == 'configuration':
                if packet_id in (14, 15):
                    # Проверяем гипотезу Known Packs по полной структуре, не одному ID.
                    candidate = io.BytesIO(payload)
                    try:
                        count = read_varint(candidate.read)
                        assert 0 <= count <= 16
                        packs = []
                        for _ in range(count):
                            names = []
                            for _ in range(3):
                                length = read_varint(candidate.read)
                                assert 0 <= length <= 256
                                name = candidate.read(length)
                                assert len(name) == length
                                names.append(name.decode('utf-8'))
                            packs.append(names)
                        assert not candidate.read()
                        assert any(pack[:2] == ['minecraft', 'core'] for pack in packs)
                    except (AssertionError, ValueError, EOFError, UnicodeError):
                        continue
                    records[-1]['known_packs_hypothesis'] = packs
                    send(sock, b'\x07' + payload)
                elif packet_id == 3 and not payload:
                    send(sock, b'\x03')
                    phase = 'play'
                elif packet_id == 4 and len(payload) == 8:
                    send(sock, b'\x04' + payload)
            else:
                return records
        raise ValueError('packet budget exhausted')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--jar', required=True)
    parser.add_argument('--sha1', required=True)
    parser.add_argument('--java', default='/usr/lib/jvm/java-25-openjdk/bin/java')
    parser.add_argument('--protocol', type=int, default=777)
    parser.add_argument('--port', type=int, default=26003)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    jar = Path(args.jar).resolve()
    with jar.open('rb') as source:
        assert hashlib.file_digest(source, 'sha1').hexdigest() == args.sha1
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', args.port))
    report = {'jar_sha1': args.sha1, 'handshake_protocol': args.protocol,
              'scope': 'wire observation only; no gameplay compatibility claim'}
    with tempfile.TemporaryDirectory(prefix='mcsa-vanilla-version-probe-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'enforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'spawn-protection=0\nmax-players=2\n')
        log = root / 'probe.log'
        with log.open('w') as output:
            server = subprocess.Popen([args.java, '-Xmx1G', '-jar', str(jar), 'nogui'],
                                      cwd=root, stdin=subprocess.PIPE, stdout=output,
                                      stderr=subprocess.STDOUT, text=True)
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None, 'official server exited during startup'
                    assert time.monotonic() < deadline, 'official server startup timeout'
                    time.sleep(.1)
                report['packets'] = []
                observe(args.port, args.protocol, report['packets'])
            except Exception as error:
                report['error'] = str(error)
                raise
            finally:
                if server.poll() is None:
                    server.stdin.write('stop\n')
                    server.stdin.flush()
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['server_exit'] = server.returncode
                destination = Path(args.output)
                destination.write_text(json.dumps(report, indent=2) + '\n')
                destination.with_suffix('.log').write_text(log.read_text())
    print(json.dumps(report))


if __name__ == '__main__':
    main()
