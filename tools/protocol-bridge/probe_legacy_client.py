#!/usr/bin/env python3
"""Настоящий клиент1.12.2 читает экспорт нашего мира и подтверждает3телепорта."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import select
import socket
import struct
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
    a = p.parse_args()
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
            client = launch(a.client_dir.resolve(), root, a.port, output)
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
                        record = {'phase': index, 'name': name, 'bytes': len(body)}
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
