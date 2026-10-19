#!/usr/bin/env python3
"""Обычный вход, телепорт и возрождение на официальном1.12.2; JAR не распаковывается."""
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
import uuid
import zlib
from observe import frame
from probe_gameplay import mapping
from smoke import handshake, read_varint, send, varint


def string(data):
    size = read_varint(data.read)
    assert 0 <= size <= 32767
    value = data.read(size)
    assert len(value) == size
    return value.decode()


class Probe:
    def __init__(self, sock, protocol, report):
        self.sock, self.protocol, self.report = sock, protocol, report
        self.threshold = None
        self.phase = 'login'
        self.position = None

    def send(self, name, body=b''):
        packet = varint(mapping(self.protocol, 'play', 'toServer')[name]) + body
        if self.threshold is not None:
            packet = varint(len(packet)) + zlib.compress(packet) if len(packet) >= self.threshold else b'\0' + packet
        send(self.sock, packet)

    def receive(self):
        raw = frame(self.sock)
        if self.threshold is not None:
            data = io.BytesIO(raw)
            size = read_varint(data.read)
            assert 0 <= size <= 2097151
            compressed = data.read()
            if size:
                assert size >= self.threshold
                decoder = zlib.decompressobj()
                raw = decoder.decompress(compressed, size + 1)
                assert len(raw) == size and decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
            else:
                raw = compressed
                assert len(raw) < self.threshold
        data = io.BytesIO(raw)
        packet_id = read_varint(data.read)
        names = {v: k for k, v in mapping(self.protocol, self.phase, 'toClient').items()}
        name = names.get(packet_id, f'unknown-{packet_id}')
        payload = data.read()
        record = {'phase': self.phase, 'name': name, 'id': packet_id, 'size': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}
        data = io.BytesIO(payload)
        if self.phase == 'login':
            record['payload'] = payload.hex()
            if name == 'compress':
                self.threshold = read_varint(data.read)
                assert self.threshold >= 0
                record['threshold'] = self.threshold
            elif name == 'success':
                ident, username = string(data), string(data)
                expected = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
                assert ident == str(expected) and username == 'McsaProbe'
                record.update(uuid=ident, username=username)
                self.phase = 'play'
            else:
                raise AssertionError(f'Unexpected login packet {name}')
            assert not data.read()
        elif name in ('login', 'respawn', 'position', 'abilities', 'game_state_change'):
            record['payload'] = payload.hex()
            if name == 'login':
                entity, mode, dimension, difficulty, maximum = struct.unpack('>iBiBB', data.read(11))
                record.update(entity=entity, mode=mode, dimension=dimension, difficulty=difficulty, maximum=maximum, level_type=string(data), reduced_debug=bool(data.read(1)[0]))
                self.send('settings', b'\x05en_us\x02\x00\x01\x7f\x01')
            elif name == 'respawn':
                dimension, difficulty, mode = struct.unpack('>iBB', data.read(6))
                record.update(dimension=dimension, difficulty=difficulty, mode=mode, level_type=string(data))
            elif name == 'position':
                values = list(struct.unpack('>dddff', data.read(32)))
                flags = data.read(1)[0]
                teleport = read_varint(data.read)
                assert flags & ~31 == 0
                if flags:
                    assert self.position is not None
                    values = [v + self.position[i] if flags & (1 << i) else v for i, v in enumerate(values)]
                self.position = values
                record.update(values=values, flags=flags, teleport=teleport)
                self.send('teleport_confirm', varint(teleport))
                self.send('position_look', struct.pack('>dddffB', *values, 0))
            elif name == 'abilities':
                flags, flying, walking = struct.unpack('>Bff', data.read(9))
                record.update(flags=flags, flying=flying, walking=walking)
            else:
                reason, value = struct.unpack('>Bf', data.read(5))
                record.update(reason=reason, value=value)
            assert not data.read(), name
        elif name == 'keep_alive':
            self.send('keep_alive', payload)
        elif name == 'kick_disconnect':
            raise AssertionError('Server disconnected test player')
        self.report['packets'].append(record)
        return record

    def pump(self, seconds, predicate=None):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([self.sock], [], [], .05)[0]:
                record = self.receive()
                if predicate and predicate(record):
                    return record
        assert predicate is None, 'Expected packet not received'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26023)
    parser.add_argument('--compression', type=int, choices=[-1, 256], required=True)
    args = parser.parse_args()
    assert hashlib.sha1(args.jar.read_bytes()).hexdigest() == '886945bfb2b978778c3a0288fd7fab09d315b25f'
    manifest = json.loads((args.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((args.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((args.data / 'protocol.json').read_text())
    report = {'protocol': 340, 'compression': args.compression, 'jar_sha1': '886945bfb2b978778c3a0288fd7fab09d315b25f', 'checks': [], 'packets': []}
    args.output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-session-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nnetwork-compression-threshold={args.compression}\nlevel-type=FLAT\nlevel-seed=1\nview-distance=2\ngamemode=1\nspawn-protection=0\n')
        with (args.output / 'server.log').open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G', '-jar', str(args.jar.resolve()), 'nogui'], cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            sock = None
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in (args.output / 'server.log').read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'Startup failed'
                    time.sleep(.1)
                sock = socket.create_connection(('127.0.0.1', args.port), timeout=10)
                handshake(sock, 340, args.port, 2)
                send(sock, b'\0\x09McsaProbe')
                client = Probe(sock, protocol, report)
                client.pump(20, lambda r: r['name'] == 'position')
                client.pump(1)
                assert (client.threshold is None) == (args.compression < 0)
                report['checks'].append('offline-login-join-teleport-ack')
                command('tp McsaProbe 4.5 65 -2.5 90 10')
                position = client.pump(5, lambda r: r['name'] == 'position')
                assert position['values'] == [4.5, 65, -2.5, 90, 10]
                report['checks'].append('absolute-teleport')
                command('gamemode survival McsaProbe')
                mode = client.pump(5, lambda r: r['name'] == 'game_state_change' and r.get('reason') == 3)
                assert mode['value'] == 0
                client.pump(.5)
                command('kill McsaProbe')
                client.pump(1)
                client.send('client_command', b'\0')
                respawn = client.pump(5, lambda r: r['name'] == 'respawn')
                assert respawn['dimension'] == 0 and respawn['mode'] == 0
                client.pump(5, lambda r: r['name'] == 'position')
                client.pump(1)
                report['checks'].append('survival-death-respawn-teleport-ack')
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
                (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'checks': report['checks'], 'exit_code': report['exit_code']}))


if __name__ == '__main__':
    main()
