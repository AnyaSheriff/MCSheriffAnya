#!/usr/bin/env python3
"""Начальный gamemode: обычный вход777, способности, сохранение и runtime override.

Только непрозрачный официальный JAR или обычный бинарник; отдельные миры/порты.
Сохраняем лишь пакеты Login/Abilities собственных игроков, не реестры/чанки.
"""
import argparse
import hashlib
import io
import json
import socket
import struct
import subprocess
import sys
import time
from pathlib import Path

from probe_gameplay import Client, connect
from smoke import read_varint

PROTOCOL_SHA = '13a10695aa1ed3604ed674ce8fdc8a759f48118714eab98644e685a3d1960727'
JAR_SHA = '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
CASES = [('survival', 'survival'), ('creative', 'creative'), ('adventure', 'adventure'),
         ('spectator', 'spectator'), ('missing', None), ('number', '1'),
         ('uppercase', 'Creative'), ('unknown', 'invalid'), ('empty', ''),
         ('zero', '0'), ('two', '2'), ('three', '3'), ('out-of-range', '4'), ('negative', '-1')]


def exact(stream, size):
    value = stream.read(size)
    if len(value) != size:
        raise ValueError('truncated login')
    return value


def text(stream):
    size = read_varint(stream.read)
    if not 0 <= size <= 32767:
        raise ValueError('invalid string size')
    return exact(stream, size).decode('utf-8')


def login_mode(payload):
    """Поля до gamemode по закреплённой разрешённой JSON-схеме777."""
    stream = io.BytesIO(payload)
    exact(stream, 5)  # entityId, isHardcore
    count = read_varint(stream.read)
    if not 0 <= count <= 16:
        raise ValueError('unexpected dimension count')
    for _ in range(count):
        text(stream)
    for _ in range(3):  # maxPlayers, viewDistance, simulationDistance
        read_varint(stream.read)
    exact(stream, 3)  # reducedDebugInfo, enableRespawnScreen, doLimitedCrafting
    read_varint(stream.read)  # SpawnInfo.dimension
    text(stream)
    exact(stream, 8)  # hashedSeed
    mode = struct.unpack('>b', exact(stream, 1))[0]
    if mode not in (0, 1, 2, 3):
        raise ValueError('invalid game mode')
    return mode


def ability_values(payload):
    if len(payload) != 9:
        raise ValueError('unexpected abilities length')
    flags, fly, walk = struct.unpack('>Bff', payload)
    return {'flags': flags, 'fly': fly, 'walk': walk}


def observation_values(observation):
    packets = observation['packets']
    login = [bytes.fromhex(p['hex']) for p in packets if p['packet'] == 'login']
    abilities = [bytes.fromhex(p['hex']) for p in packets if p['packet'] == 'abilities']
    assert len(login) == 1 and abilities, 'actual Login/Abilities packet evidence required'
    mode, values = login_mode(login[0]), ability_values(abilities[0])
    assert mode == observation['mode'] and values == observation['abilities'], 'raw packet/summary mismatch'
    return mode, values


def compare_reports(candidate, reference, labels=None):
    assert candidate['status'] == reference['status'] == 'recorded'
    assert reference['official'] is True and reference['target_digest'] == JAR_SHA
    assert candidate['protocol_sha256'] == reference['protocol_sha256'] == PROTOCOL_SHA
    expected = {row['label']: row for row in reference['cases']}
    actual = {row['label']: row for row in candidate['cases']}
    assert actual and len(actual) == len(candidate['cases']) and len(expected) == len(reference['cases'])
    selected = set(expected) if labels is None else set(labels)
    assert set(actual) == selected and selected <= set(expected), 'missing/unexpected cases'
    checks = []
    for label, row in actual.items():
        old = expected[label]
        assert row['setting'] == old['setting'], label
        observations = ['fresh'] + (['saved_after_restart', 'fresh_after_restart', 'fresh_after_runtime_override'] if label == 'survival' else [])
        for key in observations:
            assert observation_values(row[key]) == observation_values(old[key]), (label, key)
            checks.append(label + '/' + key)
    return checks


class ModeClient(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.mode = None
        self.abilities = None
        self.raw = []

    def observe_packet(self, name, payload):
        if name == 'login':
            self.mode = login_mode(payload)
            self.raw.append({'packet': name, 'hex': payload.hex()})
        elif name == 'abilities':
            self.abilities = ability_values(payload)
            self.raw.append({'packet': name, 'hex': payload.hex()})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar', type=Path)
    target.add_argument('--binary', type=Path)
    parser.add_argument('--protocol', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26240)
    parser.add_argument('--cases', default=','.join(label for label, _ in CASES))
    parser.add_argument('--reference', type=Path)
    args = parser.parse_args()
    if not 26200 <= args.port <= 26298:
        parser.error('isolated port26200–26298 required')
    requested = args.cases.split(',')
    if not requested or len(set(requested)) != len(requested) or set(requested) - {k for k, _ in CASES}:
        parser.error('unknown/duplicate cases')
    protocol_bytes = args.protocol.read_bytes()
    assert hashlib.sha256(protocol_bytes).hexdigest() == PROTOCOL_SHA
    protocol = json.loads(protocol_bytes)
    target = (args.jar or args.binary).resolve()
    with target.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha1' if args.jar else 'sha256').hexdigest()
    if args.jar:
        assert digest == JAR_SHA
    for port in (args.port, args.port + 1):
        with socket.socket() as check:
            check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            check.bind(('127.0.0.1', port))
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = {'status': 'running', 'official': bool(args.jar), 'target_digest': digest,
              'protocol_sha256': PROTOCOL_SHA, 'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'cases': [], 'scope': 'Java777 startup modes/saved/runtime override'}
    process = None
    clients = []
    handle = None
    rcon = None
    root = None
    settings_file = None

    def flush():
        (out / 'result.json').write_text(json.dumps(report, indent=2) + '\n')

    def launch(index):
        nonlocal process, handle, rcon
        log = root / f'process-{index}.log'
        handle = log.open('w')
        command = (['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx768M', '-XX:ActiveProcessorCount=2',
                    '-jar', str(target), 'nogui'] if args.jar else [str(target)])
        process = subprocess.Popen(command, cwd=root, stdin=subprocess.DEVNULL if args.jar else subprocess.PIPE,
                                   stdout=handle, stderr=subprocess.STDOUT, text=True)
        report.setdefault('processes', []).append({'pid': process.pid, 'root': str(root), 'log': str(log)})
        deadline = time.monotonic() + 90
        while True:
            assert process.poll() is None and time.monotonic() < deadline, log.read_text()[-1500:]
            if args.jar:
                ready = 'Done (' in log.read_text()
            else:
                ready = 'Готово (' in log.read_text()
            if ready:
                break
            time.sleep(.1)
        if args.jar:
            sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'measure'))
            from pregen import Rcon
            rcon = Rcon(args.port + 1, 'local-startup-probe')

    def command(value):
        report.setdefault('commands', []).append(value)
        if rcon:
            rcon.run(value)
        else:
            process.stdin.write(value + '\n')
            process.stdin.flush()
        for client in clients:
            client.pump(.4)

    def join(name):
        client = connect(args.port, protocol, ModeClient, username=name,
                         login_uuid=hashlib.md5(('OfflinePlayer:' + name).encode()).digest())
        clients.append(client)
        client.pump(15, lambda _: client.mode is not None and client.abilities is not None and client.position is not None)
        client.pump(.1)
        observation = {'username': name, 'mode': client.mode, 'abilities': client.abilities, 'packets': list(client.raw)}
        return observation

    def close_clients():
        for client in clients:
            client.sock.close()
        clients.clear()

    def stop():
        nonlocal process, handle, rcon
        close_clients()
        if process and process.poll() is None:
            if args.jar:
                process.terminate()
            else:
                process.stdin.write('stop\n')
                process.stdin.flush()
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
                raise RuntimeError('isolated process shutdown timeout')
            report.setdefault('exit_codes', []).append(process.returncode)
            assert process.returncode in (0, 143), process.returncode
        if rcon:
            rcon.sock.close()
            rcon = None
        if handle:
            handle.close()
            handle = None
        process = None

    try:
        for label, mode in CASES:
            if label not in requested:
                continue
            root = out / label
            root.mkdir()
            (root / 'eula.txt').write_text('eula=true\n')
            if not args.jar:
                (root / 'config').mkdir()
                (root / 'config/skins.toml').write_text('mojang=false\nely=false\n')
                (root / 'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
            settings_file = root / ('server.properties' if args.jar else 'config/server.properties')
            properties = (f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\nenforce-whitelist=false\nenforce-secure-profile=false\n'
                          'network-compression-threshold=-1\nview-distance=2\nsimulation-distance=2\n'
                          'level-type=minecraft:flat\nlevel-seed=1\nspawn-protection=0\nmax-tick-time=0\n'
                          f'enable-rcon=true\nrcon.port={args.port+1}\nrcon.password=local-startup-probe\n')
            settings_file.write_text(properties + ('' if mode is None else f'gamemode={mode}\n'))
            launch(0)
            item = {'label': label, 'setting': mode, 'fresh': join('ModeProbe')}
            report['cases'].append(item)
            if label == 'survival':
                command('gamemode adventure ModeProbe')
                close_clients()
                command('save-all flush')
                stop()
                # Этот же UUID сохраняет adventure при новом default creative.
                settings_file.write_text(properties + 'gamemode=creative\n')
                launch(1)
                item['saved_after_restart'] = join('ModeProbe')
                item['fresh_after_restart'] = join('ModeFresh')
                command('defaultgamemode spectator')
                item['fresh_after_runtime_override'] = join('ModeLater')
            stop()
            flush()
            print(label, 'mode', item['fresh']['mode'], 'flags', item['fresh']['abilities']['flags'], flush=True)
        report['status'] = 'recorded'
        if args.reference:
            reference = json.loads(args.reference.read_text())
            assert set(requested) <= {r['label'] for r in reference['cases']}
            assert {r['label'] for r in report['cases']} == set(requested)
            report['compared'] = compare_reports(report, reference, requested)
            report['reference_sha256'] = hashlib.sha256(args.reference.read_bytes()).hexdigest()
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        try:
            stop()
        finally:
            flush()


if __name__ == '__main__':
    main()
