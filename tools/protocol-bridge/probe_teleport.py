#!/usr/bin/env python3
"""ConfirmTeleportation26.3: наблюдение только собственного временного игрока.

Сохраняем LoginPlay и ответ на tp (не реестры/датапаки). JAR только запускается.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import socket
import struct
import subprocess
import tempfile
import time

from probe_gameplay import Client, connect
from smoke import varint


class Probe(Client):
    def __init__(self, *args):
        super().__init__(*args)
        self.pending = None

    def confirm_teleport(self, teleport, values):
        self.pending = (teleport, values)

    def acknowledge(self, id_offset=0, x_offset=0, yaw_offset=0):
        teleport, values = self.pending
        values = list(values)
        values[0] += x_offset
        values[3] += yaw_offset
        payload = varint(teleport + id_offset) + struct.pack('>dddff', *values)
        self.send('teleport_confirm', payload)
        return payload


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--jar', required=True, type=Path)
    parser.add_argument('--data', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--port', type=int, default=26003)
    args = parser.parse_args()
    with args.jar.open('rb') as stream:
        assert hashlib.file_digest(stream, 'sha1').hexdigest() == '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
    protocol_bytes = (args.data / 'protocol.json').read_bytes()
    manifest = json.loads((args.data / 'sources.json').read_text())
    expected = next(f['sha256'] for f in manifest['files'] if f['file'] == 'protocol.json')
    assert hashlib.sha256(protocol_bytes).hexdigest() == expected
    protocol = json.loads(protocol_bytes)
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    report = {'source_commit': manifest['commit'], 'cases': []}
    with tempfile.TemporaryDirectory(prefix='mcsa-teleport-26.3-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'white-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'gamemode=creative\nmax-players=2\n')
        log = root / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(args.jar.resolve()), 'nogui'],
                                      cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            def query(field):
                start = len(log.read_text())
                command('data get entity McsaProbe ' + field)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    client.pump(.1)
                    found = re.search(r'McsaProbe has the following entity data: (\[[^\n]*\])', log.read_text()[start:])
                    if found:
                        return [float(v.strip().rstrip('df')) for v in found[1][1:-1].split(',')]
                raise TimeoutError('no data get result: ' + field)
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                client = connect(args.port, protocol, Probe)
                client.pump(10, lambda r: r['name'] == 'position')
                report['play_login_hex'] = client.login_body.hex()
                report['initial_confirmation_hex'] = client.acknowledge().hex()
                client.pump(.3)
                for label, id_offset, x_offset, yaw_offset in [
                    ('matching', 0, 0, 0), ('wrong-id', -1, 0, 0),
                    ('different-position', 0, 7, 0), ('different-yaw', 0, 0, 17),
                ]:
                    command('tp McsaProbe 10 81 10 30 0')
                    client.pump(5, lambda r: r['name'] == 'position')
                    expected = list(client.pending[1])
                    client.acknowledge(id_offset, x_offset, yaw_offset)
                    client.pump(.3)
                    case = {'case': label, 'position_after_ack': query('Pos'), 'rotation_after_ack': query('Rotation')}
                    client.send('position_look', struct.pack('>dddffB', 10.25, 81, 10, 45, 0, 0))
                    client.pump(.3)
                    case['position_after_move'] = query('Pos')
                    case['rotation_after_move'] = query('Rotation')
                    accepted = list(expected)
                    if not id_offset:
                        accepted[0] += x_offset
                        accepted[3] += yaw_offset
                    assert case['position_after_ack'] == accepted[:3], case
                    assert case['rotation_after_ack'] == accepted[3:], case
                    assert case['position_after_move'] == (expected[:3] if id_offset else [10.25, 81., 10.]), case
                    assert case['rotation_after_move'] == [45., 0.], case
                    report['cases'].append(case)
                    if id_offset:
                        client.acknowledge()
                        client.pump(.3)
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
    print(json.dumps(report))


if __name__ == '__main__':
    main()
