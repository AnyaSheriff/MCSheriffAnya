#!/usr/bin/env python3
"""Парная 26.3 проверка направления радостного гаста по сетевым пакетам."""
import argparse
import io
import json
import math
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import time

from combat_ci import ROOT, SOURCES, fetch, sha

sys.path.insert(0, str(ROOT / 'tools/protocol-bridge'))
from probe_gameplay import Client, connect
from probe_wool_steps_777 import decode_movement, exact, uint31


NAME = 'HappyFacingProbe'
PORT = 27864
MOTION = {'rel_entity_move', 'entity_move_look', 'sync_entity_position'}


def angle_byte(degrees):
    return math.floor(degrees * 256 / 360) % 256


class Probe(Client):
    def __init__(self, sock, protocol, entity_type):
        super().__init__(sock, protocol)
        self.entity_type = entity_type
        self.actor = None
        self.actor_position = None
        self.yaw = self.head = 0
        self.samples = []

    def observe_packet(self, name, payload):
        if name not in MOTION | {'spawn_entity', 'entity_look', 'entity_head_rotation'}:
            return
        data = io.BytesIO(payload)
        entity = uint31(data)
        if name == 'spawn_entity':
            exact(data, 16)
            if uint31(data) != self.entity_type:
                return
            if self.actor is not None:
                raise ValueError('Появилось больше одного тестового гаста')
            self.actor = entity
            self.actor_position = struct.unpack('>ddd', exact(data, 24))
            # В 777 перед ориентацией стоит переменная длина lpVec3.
            first = exact(data, 1)[0]
            if first:
                exact(data, 5)
                if first & 4:
                    uint31(data)
            _, self.yaw, self.head = exact(data, 3)
            uint31(data)
            if data.read():
                raise ValueError('Хвост в пакете появления')
        elif entity != self.actor:
            return
        elif name in MOTION:
            _, steps = decode_movement(name, data)
            for step in steps:
                if 'position' in step:
                    self.actor_position = step['position']
                else:
                    self.actor_position = [round(v * 4096) / 4096 + d / 4096
                                           for v, d in zip(self.actor_position, step['delta'])]
            if name == 'entity_move_look':
                self.yaw = payload[-2]
            if name == 'sync_entity_position':
                self.yaw = angle_byte(struct.unpack('>f', payload[-9:-5])[0])
        elif name == 'entity_look':
            # В наблюдаемом пакете 777 флаг земли стоит перед углами.
            ground, self.yaw, _ = exact(data, 3)
            if ground > 1 or data.read():
                raise ValueError('Неверный флаг земли в пакете поворота')
        else:
            self.head = exact(data, 1)[0]
        if not all(map(math.isfinite, self.actor_position)):
            raise ValueError('Не конечные координаты гаста')
        self.samples.append({'packet': name, 'hex': payload.hex(), 'position': self.actor_position,
                             'yaw': self.yaw, 'head': self.head})


def observe(label, command, protocol, entity_type, output, root):
    reference = label == 'reference'
    log = output / f'{label}.log'
    record = {'phase': {}, 'commands': []}
    config = root / 'config'
    config.mkdir(parents=True)
    (root / 'eula.txt').write_text('eula=true\n')
    settings = (f'server-ip=127.0.0.1\nserver-port={PORT}\nonline-mode=false\nwhite-list=false\n'
        'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n'
        'level-seed=1\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n')
    (root / 'server.properties').write_text(settings)
    (config / 'server.properties').write_text(settings)
    (config / 'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\n'
        'auth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    (config / 'skins.toml').write_text('mojang=false\nely=false\n')
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', PORT))
    with log.open('w') as stream:
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=stream, stderr=subprocess.STDOUT,
                                   cwd=root, text=True)
    client = None

    def console(lines):
        record['commands'].extend(lines)
        process.stdin.write('\n'.join(lines) + '\n')
        process.stdin.flush()

    def await_log(word, seconds):
        end = time.monotonic() + seconds
        while word not in log.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() >= end:
                raise RuntimeError(f'Ответ консоли не получен. {log}')
            time.sleep(.05)

    def stage(name, seconds, *lines):
        start = len(client.samples)
        token = f'HAPPY_GHAST_{name}_BARRIER'
        console([*lines, 'say ' + token])
        await_log(token, 12)
        client.pump(seconds)
        if client.actor is None:
            raise RuntimeError('Тестовый гаст не появился у клиента')
        record['phase'][name] = {'position': client.actor_position, 'yaw': client.yaw, 'head': client.head,
                                  'packets': client.samples[start:]}

    def teleport(x, y, z):
        return f'tp {NAME} {x:.8f} {y:.8f} {z:.8f}'

    try:
        await_log('Done (' if reference else 'Готово (', 60)
        client = connect(PORT, protocol, lambda sock, p: Probe(sock, p, entity_type), username=NAME)
        client.pump(8, lambda row: row['name'] == 'position')
        stage('summon', .8, 'tick freeze', 'gamerule spawn_mobs false', f'gamemode creative {NAME}',
              teleport(9.5, 100, .5), f'give {NAME} minecraft:snowball 1',
              'summon minecraft:happy_ghast 0.5 100.0 0.5 {Silent:1b,Rotation:[0f,0f]}', 'tick step 3')
        stage('east', 2.6, 'tick step 38')
        x, y, z = client.actor_position
        stage('west', 3.4, teleport(x - 9, y, z), 'tick step 58')
        x, y, z = client.actor_position
        stage('hold', 1.8, teleport(x, y + 4.01, z), 'tick step 20')
        x, y, z = client.actor_position
        stage('diagonal', 3.4, teleport(x + 7, y, z + 4), 'tick step 60')
        x, y, z = client.actor_position
        stage('cardinal', 1.8, teleport(x, y + 4.01, z), 'tick step 20')
    finally:
        if client:
            client.sock.close()
        if process.poll() is None:
            try:
                console(['stop'])
                process.wait(timeout=20)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
        contents = log.read_text(errors='replace')
        stopped = contents.rpartition('Stopping server')[2]
        saved = 'Saving worlds' in stopped and stopped.count('Saving chunks for level') == 3 if reference else 'Мир сохранён' in contents
        record.update(exit=process.returncode, saved=saved)
        (output / f'{label}-packets.json').write_text(json.dumps(record, indent=2) + '\n')
    return record


def verdict(record):
    steps = record['phase']
    east = steps['east']
    west = steps['west']
    diagonal = steps['diagonal']
    cardinal = steps['cardinal']
    diagonal_facing = any(s['yaw'] == s['head'] and 205 <= s['yaw'] <= 221 for s in diagonal['packets'])
    return (record['exit'] == 0 and record['saved']
        and east['position'][0] - steps['summon']['position'][0] > .4
        and east['position'][0] - west['position'][0] > .4
        and east['yaw'] == east['head'] == 192 and west['yaw'] == west['head'] == 64
        and diagonal_facing
        and cardinal['yaw'] == cardinal['head'] == 192
        and math.dist(diagonal['position'], cardinal['position']) < .15)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--build-receipt', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--java', default='java')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('Парная проверка новой сборки выполняется в CI')
    build = json.loads(args.build_receipt.read_text())
    binary = args.binary.resolve(strict=True)
    head = subprocess.check_output(['git','rev-parse','HEAD'], cwd=ROOT, text=True).strip()
    if (head != os.environ.get('GITHUB_SHA') or head != build['source']
            or sha(binary) != build['sha256'] or binary.stat().st_size != build['bytes']):
        raise ValueError('Сборка ядра не соответствует коммиту')
    args.output.mkdir(parents=True, exist_ok=False)
    pins = json.loads(SOURCES.read_text())
    with tempfile.TemporaryDirectory(prefix='mcsa-happy-ghast-') as temp:
        root = Path(temp)
        jar = root / 'official.jar'
        fetch(pins['official_jar'], jar)
        recorded = {'source': head, 'binary_sha256': build['sha256'], 'reference_sha256': sha(jar)}
        for pin in pins['json_files']:
            if pin['file'] in {'entities.json', 'protocol.json'}:
                fetch(pin, root / pin['file'])
        protocol = json.loads((root / 'protocol.json').read_text())
        entity_type = next(e['id'] for e in json.loads((root / 'entities.json').read_text()) if e['name'] == 'happy_ghast')
        for label, cmd in [('reference', [args.java, '-Xmx1G', '-jar', str(jar), 'nogui']),
                           ('candidate', [str(binary)])]:
            recorded[label] = verdict(observe(label, cmd, protocol, entity_type, args.output, root / label))
        (args.output / 'receipt.json').write_text(json.dumps(recorded, indent=2) + '\n')
    if not (recorded['reference'] and recorded['candidate']):
        raise ValueError('Поворот, положение или сохранение мира расходятся. Смотрите пакеты двух прогонов.')


if __name__ == '__main__':
    main()
