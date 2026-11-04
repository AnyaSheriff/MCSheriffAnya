#!/usr/bin/env python3
"""Проверка наших Configuration-пакетов непрозрачным официальным клиентом26.2/26.3.

JAR только исполняется; никакого чтения его кода или выгрузки датапаков.
Отдельные Xvfb, каталог игры и loopback-порт, вымышленный offline-профиль.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import shutil
import socket
import struct
import tempfile
import time
import uuid

from live_client import CLIENT_SHA1, launch, stop


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


# Configuration IDs подтверждены pinned protocol JSON. NBT проверяет сам клиент.
CONFIGURATIONS = {
    '26.2': {'protocol': 776, 'known': 14, 'tags': 13, 'registries': 25},
    '26.3': {'protocol': 777, 'known': 15, 'tags': 14, 'registries': None},
}


def read_string(stream):
    size = integer(stream)
    if size > 32767:
        raise ValueError('oversized configuration string')
    return exact(stream, size).decode('utf-8')


def require_end(stream, label):
    if stream.read(1):
        raise ValueError('trailing bytes: ' + label)


def known_packs(stream):
    count = integer(stream)
    if count > 1024:
        raise ValueError('oversized known-packs count')
    packs = [tuple(read_string(stream) for _ in range(3)) for _ in range(count)]
    require_end(stream, 'known packs')
    return packs


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client-dir', type=Path, required=True)
    parser.add_argument('--packets', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', choices=CONFIGURATIONS, default='26.3')
    parser.add_argument('--port', type=int, default=26012)
    parser.add_argument('--empty-known-packs', action='store_true')
    parser.add_argument('--play-packets', type=Path, help='наши14 Play-пакетов из export_chunk_client_probe')
    parser.add_argument('--particle-probe', action='store_true', help='четыре world_particles и эффекты трёх мобов')
    parser.add_argument('--entity-probe', action='store_true', help='два Respawn и три моба от настоящих builder-ов ядра')
    parser.add_argument('--extra-play-packets',type=Path,help='дополнительные наши подготовленные кадры Play после основного набора')
    args = parser.parse_args(argv)
    if not 1 <= args.port <= 65535:
        parser.error('--port must be between 1 and 65535')
    if args.version == '26.2' and any((args.play_packets, args.particle_probe, args.entity_probe, args.extra_play_packets)):
        parser.error('26.2 supports Configuration-only probing; Play is not verified yet')
    if (args.particle_probe or args.entity_probe or args.extra_play_packets) and not args.play_packets:
        parser.error('Play probe flags require --play-packets')
    if args.particle_probe and not args.entity_probe:
        parser.error('--particle-probe requires --entity-probe')
    return args


def configuration_fixture(raw, version, empty):
    config = CONFIGURATIONS[version]
    encoded = json.loads(raw)
    if not isinstance(encoded, list) or not 29 <= len(encoded) <= 128:
        raise ValueError('unexpected configuration packet count')
    bodies = [bytes.fromhex(body) for body in encoded]
    if any(not body or len(body) > 8 * 1024 * 1024 for body in bodies):
        raise ValueError('invalid configuration packet size')
    registry_count = len(bodies) - 4
    if config['registries'] is not None and registry_count != config['registries']:
        raise ValueError('unexpected registry count for ' + version)
    expected = [1, config['known']] + [7] * registry_count + [config['tags'], 3]
    if [integer(io.BytesIO(body)) for body in bodies] != expected or bodies[-1] != b'\x03':
        raise ValueError('configuration packet IDs/order do not match ' + version)
    advertised = io.BytesIO(bodies[1])
    integer(advertised)
    if known_packs(advertised) != [('minecraft', 'core', version)]:
        raise ValueError('fixture must offer the exact client core pack')
    if empty:
        bodies[1] = bytes(varint(config['known'])) + b'\x00'
    return bodies, registry_count


def configuration_reply(connection, report, expected_packs=None, timeout=60):
    """Ограниченное ожидание ответа; ACK с лишним payload успехом не считается."""
    deadline = time.monotonic() + timeout
    for _ in range(32):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('configuration reply deadline')
        connection.settimeout(remaining)
        response = packet(connection)
        kind = integer(response)
        report['received'].append(kind)
        if kind == 7:
            if expected_packs is None:
                raise ValueError('duplicate/out-of-order known-packs reply')
            packs = known_packs(response)
            report['known_packs'] = packs
            report['known_packs_count'] = len(packs)
            if packs != expected_packs:
                raise ValueError('unexpected known packs: ' + repr(packs))
            return
        if kind == 3:
            if expected_packs is not None:
                raise ValueError('finish ACK before known-packs reply')
            require_end(response, 'finish ACK')
            report['finish_ack'] = True
            return
    raise RuntimeError('no expected configuration reply within 32 packets')


def main(argv=None):
    args = parse_args(argv)
    cache = args.client_dir.resolve()
    raw = args.packets.read_bytes()
    bodies, registry_count = configuration_fixture(raw, args.version, args.empty_known_packs)
    args.output.mkdir(parents=True, exist_ok=True)
    report = {'client': args.version, 'protocol': CONFIGURATIONS[args.version]['protocol'],
              'client_sha1': CLIENT_SHA1[args.version], 'registry_count': registry_count,
              'fixture_sha256': hashlib.sha256(raw).hexdigest(),
              'empty_known_packs': args.empty_known_packs, 'finish_ack': False, 'received': []}
    with tempfile.TemporaryDirectory(prefix='mcsa-registry-client-') as directory, socket.socket() as listener:
        root = Path(directory)
        listener.bind(('127.0.0.1', args.port))
        listener.listen(4)
        listener.settimeout(120)
        identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
        with (args.output / 'client.log').open('w') as log:
            client = None
            try:
                client = launch(cache, root, args.port, log, version=args.version)
                with listener.accept()[0] as connection:
                    connection.settimeout(60)
                    handshake = packet(connection)
                    assert integer(handshake) == 0 and integer(handshake) == report['protocol']
                    exact(handshake, integer(handshake))
                    exact(handshake, 2)
                    assert integer(handshake) == 2
                    login = packet(connection)
                    assert integer(login) == 0
                    assert exact(login, integer(login)) == b'McsaProbe'
                    send(connection, b'\x02' + identity.bytes + string('McsaProbe') + b'\x00' + uuid.uuid4().bytes)
                    login_ack = packet(connection)
                    assert integer(login_ack) == 3
                    require_end(login_ack, 'Login ACK')
                    for body in bodies[:2]:
                        send(connection, body)
                    expected_packs = [] if args.empty_known_packs else [('minecraft', 'core', args.version)]
                    configuration_reply(connection, report, expected_packs)
                    for body in bodies[2:]:
                        send(connection, body)
                    configuration_reply(connection, report)
                    if args.play_packets:
                        play = [bytes.fromhex(b) for b in json.loads(args.play_packets.read_text())]
                        if args.entity_probe:
                            assert len(play) <= 128 and sum(b[0] == 0x2e for b in play) == 18
                            assert sum(b[0] == 0x54 for b in play) == 2
                            assert sum(b[0] == 1 for b in play) == 3
                            assert sum(b[0] == 0x65 for b in play) == 3
                        else:
                            assert len(play) in (14, 18) and sum(b[0] == 0x2e for b in play) == 9
                        if args.particle_probe:
                            assert args.entity_probe and sum(b[0] == 0x30 for b in play) == 4
                        expected_poses = {}
                        for body in play:
                            if body[0] == 0x49:
                                position = io.BytesIO(body); integer(position)
                                teleport = integer(position)
                                values = struct.unpack('>ddddddffI',position.read())
                                assert values[-1] == 0
                                expected_poses[teleport] = values[:3]+values[6:8]
                        acknowledgements = set()
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
                        if args.extra_play_packets:
                            extra=json.loads(args.extra_play_packets.read_text());assert len(extra)<=256
                            report['extra_fixture_sha256']=hashlib.sha256(args.extra_play_packets.read_bytes()).hexdigest()
                            report['extra_packets']=len(extra)
                            for value in extra:
                                body=bytes.fromhex(value);assert body and len(body)<=1024*1024
                                send(connection,body);time.sleep(.05)
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
                                teleport = integer(response)
                                pose = struct.unpack('>dddff', response.read())
                                assert teleport in expected_poses and pose == expected_poses[teleport], (teleport,pose)
                                acknowledgements.add(teleport)
                                report['teleport_ids'] = sorted(acknowledgements)
                                report['teleport_ack'] = acknowledgements == set(expected_poses)
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
                # live_client завершает только созданную им группу Xvfb+Java.
                if client is not None:
                    stop(client)
                    report['client_exit'] = client.returncode
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
