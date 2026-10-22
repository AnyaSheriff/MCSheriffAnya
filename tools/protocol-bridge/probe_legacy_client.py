#!/usr/bin/env python3
"""Настоящий клиент1.12.2 читает экспорт нашего мира и подтверждает3телепорта."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import select
import socket
import struct
import subprocess
import tempfile
import time

from live_client import stop
from live_legacy_client import launch
from observe import frame
from probe_gameplay import mapping
from probe_legacy_session import string
from smoke import read_varint, send, varint


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--client-dir', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--fixture', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int, default=26025)
    p.add_argument('--cow-fixture', type=Path,
                   help='Проверить пустую inline metadata cow, затем наблюдённые поля')
    a = p.parse_args()
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    fixture = json.loads(a.fixture.read_text()); assert fixture['protocol'] == 340
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    outgoing = mapping(protocol, 'play', 'toClient')
    incoming = {v: k for k, v in mapping(protocol, 'play', 'toServer').items()}
    report = {'protocol': 340, 'fixture_sha256': hashlib.sha256(a.fixture.read_bytes()).hexdigest(), 'checks': [], 'packets': []}
    a.output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as listener, tempfile.TemporaryDirectory(prefix='mcsa-legacy-client-') as directory:
        listener.bind(('127.0.0.1', a.port)); listener.listen(2); listener.settimeout(60)
        root = Path(directory)
        log = a.output / 'client.log'
        with log.open('w') as output:
            client = launch(a.client_dir.resolve(), root, a.port, output,
                            pause_on_lost_focus=not bool(a.cow_fixture))
            sock = None
            try:
                sock, _ = listener.accept(); sock.settimeout(10)
                handshake = io.BytesIO(frame(sock))
                assert read_varint(handshake.read) == 0
                assert read_varint(handshake.read) == 340
                string(handshake); handshake.read(2)
                assert read_varint(handshake.read) == 2 and not handshake.read()
                login = io.BytesIO(frame(sock)); assert read_varint(login.read) == 0
                assert string(login) == 'McsaProbe' and not login.read()
                report['checks'].append('actual-client-handshake-login')
                send(sock, bytes.fromhex(fixture['login'])); send(sock, bytes.fromhex(fixture['join']))

                def packet(name, body):
                    send(sock, varint(outgoing[name]) + body)

                def phase(index, chunks, respawn=None):
                    if respawn:
                        send(sock, bytes.fromhex(fixture[respawn]))
                    send(sock, bytes.fromhex(fixture['abilities']))
                    packet('spawn_position', struct.pack('>Q', 64 << 26))
                    packet('update_time', struct.pack('>qq', 0, -6000))
                    packet('update_health', struct.pack('>f', 20) + varint(20) + struct.pack('>f', 5))
                    for chunk in fixture[chunks]:
                        send(sock, bytes.fromhex(chunk))
                    send(sock, bytes.fromhex(fixture['teleports'][index-1]))
                    token = 0x1020304000 + index
                    packet('keep_alive', struct.pack('>q', token))
                    ack = alive = moved = False
                    deadline = time.monotonic() + 30
                    while time.monotonic() < deadline:
                        assert client.poll() is None, 'client exited'
                        if not select.select([sock], [], [], .1)[0]:
                            continue
                        data = io.BytesIO(frame(sock)); number = read_varint(data.read)
                        body = data.read(); name = incoming.get(number, f'unknown-{number}')
                        record = {'phase': index, 'name': name, 'bytes': len(body), 'body_hex': (varint(number) + body).hex()}
                        parsed = io.BytesIO(body)
                        if name == 'teleport_confirm':
                            ident = read_varint(parsed.read)
                            assert not parsed.read() and ident == index, ident
                            ack = True; record['teleport'] = ident
                        elif name == 'keep_alive':
                            ident, = struct.unpack('>q', body)
                            assert ident == token
                            alive = True; record['token'] = ident
                        elif name in ('position', 'position_look'):
                            xyz = struct.unpack('>ddd', body[:24])
                            if ack and all(abs(v-e) < .001 for v, e in zip(xyz, fixture['position'][:3])):
                                moved = True; record['position'] = xyz
                        report['packets'].append(record)
                        if ack and alive and moved:
                            break
                    assert ack and alive and moved, (index, ack, alive, moved)
                    report['checks'].extend([f'phase{index}-teleport-ack', f'phase{index}-keepalive-after-chunks', f'phase{index}-position-matches-world'])

                phase(1, 'sky_chunks')
                if a.cow_fixture:
                    cows = json.loads(a.cow_fixture.read_text())
                    assert cows['protocol'] == 340
                    report['cow_fixture_sha256'] = hashlib.sha256(a.cow_fixture.read_bytes()).hexdigest()
                    report['cow_render_review'] = 'pending: screenshots require visual inspection'
                    authored = []

                    def capture(label):
                        # Только Java из нашей отдельной process session Xvfb.
                        private = None
                        for candidate in Path('/proc').iterdir():
                            if not candidate.name.isdigit():
                                continue
                            try:
                                if os.getsid(int(candidate.name)) != client.pid:
                                    continue
                                if not (candidate / 'exe').resolve().name.startswith('java'):
                                    continue
                                pairs = (candidate / 'environ').read_bytes().split(b'\0')
                                private = {k.decode(): v.decode() for item in pairs
                                           if b'=' in item for k, v in [item.split(b'=', 1)]
                                           if k in (b'DISPLAY', b'XAUTHORITY')}
                                if private.get('DISPLAY'):
                                    break
                            except (OSError, ProcessLookupError):
                                continue
                        assert private and private.get('DISPLAY'), 'Private Xvfb display not found'
                        environment = os.environ.copy()
                        for key in ('DISPLAY', 'XAUTHORITY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET'):
                            environment.pop(key, None)
                        environment.update(private)
                        path = a.output / (label + '.png')
                        subprocess.run(['import', '-window', 'root', str(path.resolve())],
                                       env=environment, check=True, timeout=10,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                        report.setdefault('screenshots', []).append(
                            {'stage': label, 'file': path.name,
                             'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})

                    def barrier(label, index):
                        token = 0x2030405000 + index
                        packet('keep_alive', struct.pack('>q', token))
                        deadline = time.monotonic() + 10
                        received = False
                        while time.monotonic() < deadline:
                            assert client.poll() is None, 'client exited during cow phase'
                            if not select.select([sock], [], [], .05)[0]:
                                continue
                            data = io.BytesIO(frame(sock))
                            number = read_varint(data.read)
                            body = data.read()
                            name = incoming.get(number, f'unknown-{number}')
                            report['packets'].append({'phase': label, 'name': name,
                                                      'bytes': len(body),
                                                      'body_hex': (varint(number) + body).hex()})
                            if name == 'keep_alive' and struct.unpack('>q', body)[0] == token:
                                received = True
                                break
                        assert received, label
                        time.sleep(.8)
                        capture(label)
                        report['checks'].append(label + '-keepalive-after-packets')

                    barrier('cow-baseline', 1)
                    x, y, z = fixture['position'][:3]
                    for stage, dx in [('adult-spawn', -1.2), ('baby-spawn', 1.2)]:
                        event = next(e for e in cows['events'] if e['stage'] == stage
                                     and e['name'] == 'spawn_entity_living')
                        ident = event['entity']
                        body = (varint(ident) + bytes.fromhex(event['uuid']) + varint(event['kind'])
                                + struct.pack('>dddBBBhhh', x + dx, y, z + 4,
                                              0, 0, 0, 0, 0, 0) + b'\xff')
                        packet('spawn_entity_living', body)
                        report.setdefault('cow_outgoing', []).append(
                            {'stage': 'empty-inline', 'name': 'spawn_entity_living',
                             'payload': body.hex()})
                        authored.append(event)
                    barrier('cow-empty-inline', 2)
                    for event in authored:
                        body = varint(event['entity']) + bytes.fromhex(event['metadata'])
                        packet('entity_metadata', body)
                        report['cow_outgoing'].append(
                            {'stage': 'observed-metadata', 'name': 'entity_metadata',
                             'payload': body.hex()})
                    barrier('cow-observed-metadata', 3)
                    adult = authored[0]['entity']
                    for name, body in [
                        ('entity_head_rotation', varint(adult) + b'\x40'),
                        ('rel_entity_move', varint(adult) + struct.pack('>hhhB', 2048, 0, 0, 1)),
                    ]:
                        packet(name, body)
                        report['cow_outgoing'].append(
                            {'stage': 'head-relative', 'name': name, 'payload': body.hex()})
                    barrier('cow-head-relative', 4)
                    body = varint(len(authored)) + b''.join(varint(e['entity']) for e in authored)
                    packet('entity_destroy', body)
                    report['cow_outgoing'].append(
                        {'stage': 'remove', 'name': 'entity_destroy', 'payload': body.hex()})
                    barrier('cow-removed', 5)
                phase(2, 'dark_chunks', 'respawn_nether')
                phase(3, 'sky_chunks', 'respawn_overworld')
                # Даём renderer обработать уже принятый мир; ошибка decoder/GL
                # должна остаться в логе, а не скрыться мгновенным завершением.
                time.sleep(1)
                assert client.poll() is None
                text = log.read_text(errors='replace')
                for marker in ['DecoderException', 'OutOfMemoryError', 'Unexpected error', 'Reported exception thrown!', 'UnsatisfiedLinkError']:
                    assert marker not in text, marker
            except BaseException as error:
                report['error'] = str(error)
                raise
            finally:
                if sock: sock.close()
                stop(client)
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'checks': report['checks']}))


if __name__ == '__main__':
    main()
