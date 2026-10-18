#!/usr/bin/env python3
"""Проверка наших Configuration-пакетов непрозрачным официальным клиентом26.3.

JAR только исполняется; никакого чтения его кода или выгрузки датапаков.
Отдельные Xvfb, каталог игры и loopback-порт, вымышленный offline-профиль.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import time
import uuid


def varint(value):
    out = bytearray()
    while value >= 128:
        out.append((value & 127) | 128)
        value >>= 7
    out.append(value)
    return bytes(out)


def exact(stream, size):
    out = bytearray()
    while len(out) < size:
        part = stream.recv(size - len(out)) if isinstance(stream, socket.socket) else stream.read(size - len(out))
        if not part:
            raise EOFError('client closed the connection')
        out.extend(part)
    return bytes(out)


def integer(stream):
    value = 0
    for shift in range(0, 35, 7):
        byte = exact(stream, 1)[0]
        value |= (byte & 127) << shift
        if byte < 128:
            return value
    raise ValueError('oversized VarInt')


def packet(stream):
    size = integer(stream)
    if not 1 <= size <= 8 * 1024 * 1024:
        raise ValueError('oversized packet')
    return io.BytesIO(exact(stream, size))


def string(value):
    value = value.encode()
    return varint(len(value)) + value


def send(stream, body):
    stream.sendall(varint(len(body)) + body)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-dir', type=Path, required=True)
    parser.add_argument('--packets', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26012)
    parser.add_argument('--empty-known-packs', action='store_true')
    parser.add_argument('--play-packets', type=Path, help='наши14 Play-пакетов из export_chunk_client_probe')
    args = parser.parse_args()
    cache = args.client_dir.resolve()
    with (cache / 'client.jar').open('rb') as source:
        assert hashlib.file_digest(source, 'sha1').hexdigest() == 'e877b6a07acd633fb3bb475002175cec036e7b87'
    bodies = [bytes.fromhex(body) for body in json.loads(args.packets.read_text())]
    registry_count = sum(body[0] == 7 for body in bodies)
    assert registry_count >= 25 and len(bodies) == registry_count + 4
    assert bodies[0][0] == 1 and bodies[1][0] == 15 and bodies[-1] == b'\x03'
    if args.empty_known_packs:
        bodies[1] = b'\x0f\x00'
    args.output.mkdir(parents=True, exist_ok=True)
    report = {'client': '26.3', 'protocol': 777, 'registry_count': registry_count,
              'fixture_sha256': hashlib.sha256(args.packets.read_bytes()).hexdigest(),
              'empty_known_packs': args.empty_known_packs, 'finish_ack': False, 'received': []}
    with tempfile.TemporaryDirectory(prefix='mcsa-registry-client-') as directory, socket.socket() as listener:
        root = Path(directory)
        (root / 'options.txt').write_text('onboardAccessibility:false\nskipMultiplayerWarning:true\nnarrator:0\n')
        listener.bind(('127.0.0.1', args.port))
        listener.listen(4)
        listener.settimeout(120)
        identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
        command = ['xvfb-run', '-a', '-s', '-screen 0 854x480x24',
                   '/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-XX:StackShadowPages=32',
                   '--enable-native-access=ALL-UNNAMED', '--add-exports', 'java.base/jdk.internal.misc=ALL-UNNAMED',
                   '-Djava.library.path=' + str(root / 'natives/java'),
                   '-Djna.tmpdir=' + str(root / 'natives/jna'),
                   '-Dorg.lwjgl.system.SharedLibraryExtractPath=' + str(root / 'natives/lwjgl'),
                   '-Dio.netty.native.workdir=' + str(root / 'natives/netty'),
                   '-Dminecraft.launcher.brand=mcsa-probe', '-Dminecraft.launcher.version=1',
                   '-cp', (cache / 'classpath.txt').read_text().strip(), 'net.minecraft.client.main.Main',
                   '--username', 'McsaProbe', '--version', '26.3', '--gameDir', str(root),
                   '--assetsDir', str(cache / 'assets'), '--assetIndex', '34', '--uuid', identity.hex,
                   '--accessToken', '0', '--clientId', '0', '--xuid', '0', '--versionType', 'release',
                   '--width', '854', '--height', '480', '--quickPlayMultiplayer', f'127.0.0.1:{args.port}']
        environment = os.environ.copy()
        for key in ['WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'DISPLAY', 'XAUTHORITY', 'DBUS_SESSION_BUS_ADDRESS']:
            environment.pop(key, None)
        (root / 'runtime').mkdir(mode=0o700)
        environment.update(XDG_SESSION_TYPE='x11', XDG_RUNTIME_DIR=str(root / 'runtime'), ALSOFT_DRIVERS='null')
        with (args.output / 'client.log').open('w') as log:
            client = subprocess.Popen(command, cwd=root, stdin=subprocess.DEVNULL, stdout=log,
                                      stderr=subprocess.STDOUT, start_new_session=True, env=environment)
            try:
                with listener.accept()[0] as connection:
                    connection.settimeout(60)
                    handshake = packet(connection)
                    assert integer(handshake) == 0 and integer(handshake) == 777
                    exact(handshake, integer(handshake))
                    exact(handshake, 2)
                    assert integer(handshake) == 2
                    login = packet(connection)
                    assert integer(login) == 0
                    assert exact(login, integer(login)) == b'McsaProbe'
                    send(connection, b'\x02' + identity.bytes + string('McsaProbe') + b'\x00' + uuid.uuid4().bytes)
                    assert integer(packet(connection)) == 3
                    for body in bodies[:2]:
                        send(connection, body)
                    for _ in range(32):
                        response = packet(connection)
                        kind = integer(response)
                        report['received'].append(kind)
                        if kind == 7:
                            report['known_packs_count'] = integer(response)
                            if args.empty_known_packs:
                                assert report['known_packs_count'] == 0
                            break
                    else:
                        raise RuntimeError('no known-packs reply')
                    for body in bodies[2:]:
                        send(connection, body)
                    for _ in range(32):
                        response = packet(connection)
                        kind = integer(response)
                        report['received'].append(kind)
                        if kind == 3:
                            report['finish_ack'] = True
                            break
                    assert report['finish_ack'], 'client did not accept configuration'
                    if args.play_packets:
                        play = [bytes.fromhex(b) for b in json.loads(args.play_packets.read_text())]
                        assert len(play) in (14, 18) and sum(b[0] == 0x2e for b in play) == 9
                        report['play_fixture_sha256'] = hashlib.sha256(args.play_packets.read_bytes()).hexdigest()
                        report['play_received'] = []
                        report['teleport_ack'] = False
                        report['player_loaded'] = False
                        report['post_play_pong'] = False
                        report['sent_play'] = []
                        for body in play:
                            send(connection, body)
                            report['sent_play'].append(body[0])
                            time.sleep(.15)
                        # Ping после всех пакетов требует ответа уже после их обработки.
                        send(connection, b'\x3e' + struct.pack('>i', 0x4d435341))
                        # ID закреплены в candidate protocol.json; проверяем не только
                        # TCP-приём, а закрытие клиентом экрана загрузки и подтверждение позы.
                        deadline = time.monotonic() + 30
                        while time.monotonic() < deadline:
                            response = packet(connection)
                            kind = integer(response)
                            report['play_received'].append(kind)
                            if kind == 0:
                                assert integer(response) == 1
                                pose = struct.unpack('>dddff', response.read())
                                assert pose == (-8.5, 67.0, -8.5, 0.0, 0.0), pose
                                report['teleport_ack'] = True
                            if kind == 44:
                                report['player_loaded'] = True
                            if kind == 45:
                                assert struct.unpack('>i', response.read())[0] == 0x4d435341
                                report['post_play_pong'] = True
                            if report['teleport_ack'] and report['player_loaded'] and report['post_play_pong']:
                                break
                        assert report['teleport_ack'] and report['player_loaded'] and report['post_play_pong'], 'client did not finish play exchange'
                        time.sleep(1)
                        assert not list((root / 'debug').glob('disconnect-*.txt')), 'client produced a disconnect report'
                        if (root / 'logs/latest.log').exists():
                            messages = (root / 'logs/latest.log').read_text()
                            assert 'Failed to handle packet' not in messages and 'Failed to load chunk' not in messages, 'client rejected play data'
                    message = b'Registry probe complete'
                    send(connection, b'\x20\x08' + struct.pack('>H', len(message)) + message)
                    time.sleep(.5)
            except Exception as error:
                report['error'] = repr(error)
                raise
            finally:
                # Завершаем только собственную группу Xvfb+Java по сохранённому PID.
                try:
                    os.killpg(client.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    client.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(client.pid, signal.SIGKILL)
                    client.wait()
                for folder in ['crash-reports', 'debug']:
                    for diagnostic in (root / folder).rglob('*.txt'):
                        if diagnostic.stat().st_size <= 1024 * 1024:
                            destination = args.output / 'diagnostics' / diagnostic.relative_to(root)
                            destination.parent.mkdir(parents=True, exist_ok=True)
                            shutil.copyfile(diagnostic, destination)
                if (root / 'logs/latest.log').exists():
                    shutil.copyfile(root / 'logs/latest.log', args.output / 'latest.log')
                (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
