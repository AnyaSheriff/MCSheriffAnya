#!/usr/bin/env python3
"""Узкая проверка собственного первого Play776 настоящим клиентом в отдельном Xvfb.

Это воспроизведение кадров наших builder-ов, а не работающий сервер или gameplay.
"""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import time
import uuid

from live_client import CLIENT_SHA1, launch, stop
from probe_registry_client import (configuration_fixture, configuration_reply, exact,
                                   integer, packet, require_end, send, string)

# Закреплённый protocol.json776 SHA256 a6c9bd7240f74b94eb0d9b1497294e4cfaa4bb02c0023817c952429149e2d50c.
IDS = {'login': 49, 'abilities': 64, 'game_state_change': 38, 'update_view_position': 94,
       'map_chunk': 45, 'position': 72, 'entity_metadata': 99, 'window_items': 18,
       'set_slot': 20, 'set_cursor_item': 96, 'set_player_inventory': 108,
       'held_item_slot': 105, 'update_health': 104}
SEQUENCE = ['login', 'abilities', 'game_state_change', 'update_view_position'] + ['map_chunk'] * 9 + [
    'position', 'entity_metadata', 'window_items', 'set_slot', 'set_cursor_item',
    'set_player_inventory', 'held_item_slot', 'update_health']
PING = 0x4D435341


def fixture(raw):
    data = json.loads(raw)
    if (data.get('schema'), data.get('native_protocol'), data.get('target_protocol'),
            data.get('payload_protocol'), data.get('online_mode')) != (1, 775, 776, 776, False):
        raise ValueError('expected own offline775→776 fixture schema1')
    entries = data['packets']
    if [entry['name'] for entry in entries] != SEQUENCE:
        raise ValueError('unexpected first-Play packet sequence')
    bodies = [bytes.fromhex(entry['body_hex']) for entry in entries]
    if any(not body or len(body) > 8 * 1024 * 1024 for body in bodies):
        raise ValueError('invalid packet size')
    for name, body in zip(SEQUENCE, bodies):
        if integer(io.BytesIO(body)) != IDS[name]:
            raise ValueError('packet ID does not match name: ' + name)
    expected = tuple(data['expected_pose'][key] for key in ['x', 'y', 'z', 'yaw', 'pitch'])
    if not all(isinstance(value, (int, float)) and math.isfinite(value) for value in expected):
        raise ValueError('nonfinite pose')
    position = io.BytesIO(bodies[13]); integer(position)
    teleport = integer(position)
    values = struct.unpack('>ddddddffI', position.read())
    if teleport != data['teleport_id'] or values[:3] + values[6:8] != expected:
        raise ValueError('fixture pose/teleport mismatch')
    if values[3:6] != (0.0, 0.0, 0.0) or values[-1] != 0:
        raise ValueError('relative pose or velocity outside fixture scope')
    chunks = set()
    for body in bodies[4:13]:
        reader = io.BytesIO(body); integer(reader)
        chunks.add(struct.unpack('>ii', exact(reader, 8)))
    if chunks != {(x, z) for x in range(-1, 2) for z in range(-1, 2)}:
        raise ValueError('expected nine distinct spawn chunks')
    return data, bodies, expected


def read_play(body, teleport, pose):
    """Возвращает наблюдение; short ACK776 не содержит и не доказывает позу."""
    source = io.BytesIO(body)
    kind = integer(source)
    result = {'id': kind, 'body_hex': body.hex()}
    if kind == 0:
        value = integer(source); require_end(source, 'short teleport ACK776')
        if value != teleport:
            raise ValueError('unexpected teleport ACK')
        result['teleport_ack'] = value
    elif kind == 44:
        require_end(source, 'Player Loaded776'); result['loaded'] = True
    elif kind == 45:
        value = struct.unpack('>i', exact(source, 4))[0]; require_end(source, 'Pong776')
        if value != PING:
            raise ValueError('unexpected Pong')
        result['pong'] = value
    elif kind in (30, 31, 32, 33):
        position = pose[:3]
        rotation = pose[3:]
        supplied = []
        if kind in (30, 31):
            position = struct.unpack('>ddd', exact(source, 24))
            supplied.extend(position)
        if kind in (31, 32):
            rotation = struct.unpack('>ff', exact(source, 8))
            supplied.extend(rotation)
        flags = exact(source, 1)[0]; require_end(source, 'movement776')
        if flags & ~3 or not all(math.isfinite(value) for value in supplied):
            raise ValueError('invalid movement')
        result.update(pose=(*position, *rotation), ground=bool(flags & 1))
    return result


def play_complete(report, now, last_ground):
    return (last_ground is not None and 0 <= now - last_ground <= 1.5
            and all(report[key] for key in ['teleport_ack', 'player_loaded', 'post_play_pong', 'grounded']))


def next_play_packet(connection, deadline):
    # Не начинаем кадр заново после частичного чтения при socket.timeout.
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise TimeoutError('Play776 deadline expired')
    connection.settimeout(remaining)
    return packet(connection)


def capture(client, output):
    # Только DISPLAY/XAUTHORITY Java в собственной сессии; значения не журналируем.
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
            private = {k.decode(): v.decode() for item in pairs if b'=' in item
                       for k, v in [item.split(b'=', 1)] if k in (b'DISPLAY', b'XAUTHORITY')}
            if private.get('DISPLAY'):
                break
        except (OSError, ProcessLookupError):
            continue
    if not private or not private.get('DISPLAY'):
        raise RuntimeError('own private display unavailable')
    environment = os.environ.copy()
    for key in ('DISPLAY', 'XAUTHORITY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET'):
        environment.pop(key, None)
    environment.update(private)
    subprocess.run(['import', '-window', 'root', str(output)], env=environment,
                   check=True, timeout=10, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return hashlib.sha256(output.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ['client-dir', 'configuration', 'play', 'output']:
        parser.add_argument('--' + key, type=Path, required=True)
    parser.add_argument('--port', type=int, default=26014)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error('invalid port')
    raw = args.play.read_bytes()
    data, bodies, expected = fixture(raw)
    config_raw = args.configuration.read_bytes()
    config, count = configuration_fixture(config_raw, '26.2', True)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    report = {'client': '26.2', 'protocol': 776, 'client_sha1': CLIENT_SHA1['26.2'],
              'configuration_sha256': hashlib.sha256(config_raw).hexdigest(),
              'play_sha256': hashlib.sha256(raw).hexdigest(), 'registry_count': count,
              'received': [], 'finish_ack': False, 'play_received': [],
              'teleport_ack': False, 'player_loaded': False, 'post_play_pong': False,
              'grounded': False, 'visual_review': 'pending', 'runtime_server': False}
    with tempfile.TemporaryDirectory(prefix='mcsa-initial776-') as folder:
        root = Path(folder)
        client = None
        try:
            with socket.socket() as listener, (args.output / 'client.log').open('w') as log:
                listener.bind(('127.0.0.1', args.port)); listener.listen(1); listener.settimeout(120)
                client = launch(args.client_dir.resolve(), root, args.port, log, version='26.2')
                with listener.accept()[0] as connection:
                    connection.settimeout(60)
                    handshake = packet(connection)
                    if integer(handshake) != 0 or integer(handshake) != 776:
                        raise ValueError('not a776 login')
                    exact(handshake, integer(handshake)); exact(handshake, 2)
                    if integer(handshake) != 2:
                        raise ValueError('not Login state')
                    require_end(handshake, 'Handshake')
                    login = packet(connection)
                    if integer(login) != 0 or exact(login, integer(login)) != b'McsaProbe':
                        raise ValueError('unexpected Login Start')
                    exact(login, 16); require_end(login, 'Login Start')
                    identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
                    send(connection, b'\x02' + identity.bytes + string('McsaProbe') + b'\x00' + uuid.uuid4().bytes)
                    ack = packet(connection)
                    if integer(ack) != 3:
                        raise ValueError('missing Login ACK')
                    require_end(ack, 'Login ACK')
                    for body in config[:2]:
                        send(connection, body)
                    configuration_reply(connection, report, [])
                    for body in config[2:]:
                        send(connection, body)
                    configuration_reply(connection, report)
                    for body in bodies:
                        send(connection, body); time.sleep(.05)
                    send(connection, b'\x3d' + struct.pack('>i', PING))
                    deadline = time.monotonic() + 45
                    pose = (None,) * 5
                    ground_start = last_ground = None
                    ground_samples = 0
                    for _ in range(2048):
                        now = time.monotonic()
                        if now >= deadline:
                            break
                        response = next_play_packet(connection, deadline)
                        now = time.monotonic()
                        observed = read_play(response.read(), data['teleport_id'], pose)
                        report['play_received'].append(observed)
                        report['teleport_ack'] |= 'teleport_ack' in observed
                        report['player_loaded'] |= observed.get('loaded', False)
                        report['post_play_pong'] |= 'pong' in observed
                        if 'pose' in observed:
                            pose = observed['pose']
                            good = observed['ground'] and all(x is not None and abs(x - y) < 0.02 for x, y in zip(pose, expected))
                            if good:
                                if last_ground is None or now - last_ground > 1.5:
                                    ground_start, ground_samples = now, 0
                                last_ground = now; ground_samples += 1
                                report['grounded'] = now - ground_start >= 2 and ground_samples >= 3
                            else:
                                ground_start = last_ground = None; ground_samples = 0; report['grounded'] = False
                        if play_complete(report, now, last_ground):
                            break
                    if not play_complete(report, time.monotonic(), last_ground):
                        raise RuntimeError('incomplete Play exchange or unstable camera')
                    report['final_pose'] = pose
                    report['ground_samples'] = ground_samples
                    report['ground_span_seconds'] = last_ground - ground_start
                    report['screenshot_sha256'] = capture(client, args.output / 'initial-play.png')
                    if list((root / 'debug').glob('disconnect-*.txt')) or list((root / 'crash-reports').glob('*.txt')):
                        raise RuntimeError('client produced an unexpected diagnostic')
                    if (root / 'logs/latest.log').exists():
                        messages = (root / 'logs/latest.log').read_text()
                        if 'Failed to handle packet' in messages or 'Failed to load chunk' in messages:
                            raise RuntimeError('client rejected own Play data')
                    message = b'Initial Play776 probe complete'
                    send(connection, b'\x20\x08' + struct.pack('>H', len(message)) + message)
                    time.sleep(.5)
        except Exception as error:
            report['error'] = repr(error)
            raise
        finally:
            if client is not None:
                stop(client); report['client_exit'] = client.returncode
            if (root / 'logs/latest.log').exists():
                shutil.copyfile(root / 'logs/latest.log', args.output / 'latest.log')
            for directory in ['debug', 'crash-reports']:
                for source in (root / directory).rglob('*.txt'):
                    if source.stat().st_size <= 1024 * 1024:
                        destination = args.output / 'diagnostics' / source.relative_to(root)
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(source, destination)
            (args.output / 'result.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
    print(json.dumps({key: report[key] for key in ['finish_ack', 'teleport_ack', 'player_loaded', 'post_play_pong', 'grounded']}))


if __name__ == '__main__':
    main()
