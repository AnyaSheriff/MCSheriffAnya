#!/usr/bin/env python3
"""Отдельная обычная команда /tp на official775; только наш временный мир.

Запускать лишь в выделенном MC-окне, без Cargo/других probes. Это сбор
наблюдений, а не заранее объявленная проверка всех гипотез об углах.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import tempfile
import time

JAR_SHA1 = '97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51'
SELECTOR = '@e[type=cow,tag=mcsa_tp,limit=1]'
RESET = '{Pos:[3.5d,64d,0.5d],Rotation:[80f,20f],Motion:[0d,0d,0d]}'
FIELDS = ('Pos', 'Rotation', 'Motion', 'OnGround')
# Все относительные/clamp ответы оставляем наблюдениями до ручного review.
CASES = [
    ('absolute90', 'console', '3.5 64 0.5 90 10'),
    ('absolute180', 'console', '-6.5 64 0.5 180 0'),
    ('absolute450', 'console', '3.5 64 4.5 450 -30'),
    ('absolute_minus450', 'console', '3.5 64 4.5 -450 30'),
    ('pitch95', 'console', '3.5 64 0.5 0 95'),
    ('pitch_minus95', 'console', '3.5 64 0.5 0 -95'),
    ('pitch450', 'console', '3.5 64 0.5 0 450'),
    ('pitch_minus450', 'console', '3.5 64 0.5 0 -450'),
    ('pitch270', 'console', '3.5 64 0.5 0 270'),
    ('pitch_minus270', 'console', '3.5 64 0.5 0 -270'),
    ('pitch360', 'console', '3.5 64 0.5 0 360'),
    ('pitch_minus360', 'console', '3.5 64 0.5 0 -360'),
    ('console_relative', 'console', '3.5 64 0.5 ~10 ~5'),
    ('console_relative_zero', 'console', '3.5 64 0.5 ~ ~'),
    ('player_relative', 'player', '3.5 64 0.5 ~10 ~5'),
    ('player_relative_zero', 'player', '3.5 64 0.5 ~ ~'),
    ('player_relative_pitch_high', 'player', '3.5 64 0.5 ~ ~120'),
    ('player_relative_pitch_low', 'player', '3.5 64 0.5 ~ ~-120'),
    ('execute_as', 'as', '3.5 64 0.5 ~10 ~5'),
    ('execute_at', 'at', '3.5 64 0.5 ~10 ~5'),
    ('execute_as_at', 'as_at', '3.5 64 0.5 ~10 ~5'),
    ('omitted', 'console', '5.5 64 1.5'),
    ('bad_yaw_nan', 'console', '5.5 64 1.5 NaN 10'),
    ('bad_pitch_inf', 'console', '5.5 64 1.5 10 Infinity'),
    ('bad_relative', 'console', '5.5 64 1.5 ~bad 10'),
    ('bad_relative_nan', 'console', '5.5 64 1.5 10 ~NaN'),
    ('bad_position', 'console', 'bad 64 1.5 10 20'),
    ('missing_pitch', 'console', '5.5 64 1.5 10'),
    ('extra_argument', 'console', '5.5 64 1.5 10 20 extra'),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26028)
    parser.add_argument('--case', action='append', default=[], help='Run only named cases; repeatable')
    args = parser.parse_args()
    assert set(args.case) <= {row[0] for row in CASES}, 'unknown case'
    assert args.port not in (25565, 25566, 25567), 'use isolated probe port'
    # Только хеш файла; содержимое бинарника не исследуется.
    assert hashlib.sha1(args.jar.read_bytes()).hexdigest() == JAR_SHA1
    manifest = json.loads((args.data / 'sources.json').read_text())
    entry = next(row for row in manifest['baseline_files'] + manifest['files']
                 if row['file'] == 'protocol-26.1.json')
    protocol_file = args.data / entry['file']
    assert hashlib.sha256(protocol_file.read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads(protocol_file.read_text())
    assert protocol['play']['toServer']['types']['packet_chat_command'] == [
        'container', [{'name': 'command', 'type': 'string'}]]
    sys.path.insert(0, str(args.repo / 'tools/protocol-bridge'))
    from probe_gameplay import Client, connect
    from smoke import varint

    class Probe(Client):
        def __init__(self, sock, proto):
            super().__init__(sock, proto)
            self.observed = []
        def confirm_teleport(self, teleport, values):
            self.send('teleport_confirm', varint(teleport))
        def observe_packet(self, name, payload):
            if name in ('spawn_entity', 'entity_metadata', 'sync_entity_position',
                        'entity_teleport', 'entity_head_rotation', 'entity_move_look',
                        'rel_entity_move',
                        'entity_look', 'system_chat', 'disconnect'):
                self.observed.append({'name': name, 'payload_hex': payload.hex()})
        def command(self, command):
            raw = command.encode()
            self.send('chat_command', varint(len(raw)) + raw)

    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', args.port))
    args.output.mkdir(parents=True, exist_ok=False)
    report = {'version': 775, 'jar_sha1': JAR_SHA1,
              'protocol_sha256': entry['sha256'], 'cases': [],
              'note': 'Observations only; relative/clamp semantics need review.'}
    with tempfile.TemporaryDirectory(prefix='mcsa-tp775-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'white-list=false\nenforce-secure-profile=false\n'
            'network-compression-threshold=-1\nlevel-type=minecraft:flat\n'
            'level-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'gamemode=creative\nspawn-protection=0\n')
        log = root / 'server.log'
        with log.open('w') as output:
            cpus = sorted(os.sched_getaffinity(0))[:2]
            server = subprocess.Popen([
                'taskset', '-c', ','.join(map(str, cpus)),
                '/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G',
                '-jar', str(args.jar.resolve()), 'nogui'], cwd=root,
                stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            sequence = 0
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            def barrier():
                nonlocal sequence
                sequence += 1
                marker = f'MCSA_TP_BARRIER_{sequence:04d}'
                command('say ' + marker)
                deadline = time.monotonic() + 10
                while marker not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'console barrier failed'
                    client.pump(.05)
            def state():
                result = {}
                for field in FIELDS:
                    offset = len(log.read_text())
                    command(f'data get entity {SELECTOR} {field}')
                    barrier()
                    text = log.read_text()[offset:]
                    # Сохраняем точный SNBT; не eval и не сторонний NBT parser.
                    rows = re.findall(r'has the following entity data: (.+)', text)
                    assert len(rows) == 1, f'no unique {field} result: {text}'
                    result[field] = rows[0].strip()
                return result
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.1)
                client = connect(args.port, protocol, Probe, 775)
                client.pump(10, lambda row: row['name'] == 'position')
                command('op McsaProbe')
                command('fill -8 63 -8 8 63 8 minecraft:bedrock')
                command('tp McsaProbe 0.5 64 0.5 35 -12')
                client.pump(5, lambda row: row.get('position') == [.5, 64., .5, 35., -12.])
                assert client.position == [.5, 64., .5, 35., -12.]
                command('summon cow 3.5 64 0.5 {NoAI:1b,NoGravity:1b,Silent:1b,Tags:["mcsa_tp"],Rotation:[80f,20f]}')
                client.pump(.5)
                barrier()
                for name, source, arguments in CASES:
                    if args.case and name not in args.case:
                        continue
                    command(f'data merge entity {SELECTOR} {RESET}')
                    barrier()
                    client.pump(.15)
                    before = state()
                    assert before['Rotation'] == '[80.0f, 20.0f]', before
                    action = f'tp {SELECTOR} {arguments}'
                    prefix = {'as': 'execute as McsaProbe run ',
                              'at': 'execute at McsaProbe run ',
                              'as_at': 'execute as McsaProbe at @s run '}.get(source, '')
                    offset = len(log.read_text())
                    packets = len(client.observed)
                    if source == 'player':
                        # Порядок пакетов одного игрока: marker подтверждает обработку /tp.
                        client.command(action)
                        marker = 'MCSA_PLAYER_COMPLETED_' + name
                        client.command('say ' + marker)
                        deadline = time.monotonic() + 10
                        while marker not in log.read_text()[offset:]:
                            assert time.monotonic() < deadline, 'player command barrier failed'
                            client.pump(.05)
                    else:
                        command(prefix + action)
                        barrier()
                    client.pump(.2)
                    command_log = log.read_text()[offset:]
                    after = state()
                    report['cases'].append({'name': name, 'source': source,
                        'command': prefix + action, 'player_rotation': client.position[3:],
                        'before': before, 'after': after,
                        'unchanged': before == after, 'command_log': command_log,
                        'packets': client.observed[packets:]})
            except BaseException as error:
                report['error'] = repr(error)
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
                report['exit_code'] = server.returncode
                (args.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
                (args.output / 'server.log').write_text(log.read_text())
    print(json.dumps({'cases': len(report['cases']), 'exit_code': report['exit_code']}))


if __name__ == '__main__':
    main()
