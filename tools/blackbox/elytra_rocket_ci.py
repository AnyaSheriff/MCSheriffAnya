#!/usr/bin/env python3
"""Звук, взрыв и привязка ракеты элитр по пакетам с двух серверов 26.3."""
import argparse
import io
import json
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
from probe_gameplay import Client, connect, packed
from probe_wool_steps_777 import exact, uint31
from smoke import varint


NAME = 'ElytraRocket'
PORT = 27864
ROCKET = 'minecraft:firework_rocket[fireworks={flight_duration:1,explosions:[{shape:"small_ball",colors:[I;16711680]}]}]'


class Watcher(Client):
    def __init__(self, sock, protocol, rocket_type):
        super().__init__(sock, protocol)
        self.rocket_type = rocket_type
        self.rockets = set()
        self.player_id = None
        self.events = []

    def observe_packet(self, name, payload):
        data = io.BytesIO(payload)
        if name == 'login':
            self.player_id = struct.unpack('>i', exact(data, 4))[0]
            return
        if name == 'spawn_entity':
            entity = uint31(data)
            exact(data, 16)
            if uint31(data) != self.rocket_type:
                return
            self.rockets.add(entity)
        elif name == 'entity_metadata':
            entity = uint31(data)
            if entity not in self.rockets:
                return
        elif name == 'entity_status':
            entity = struct.unpack('>i', exact(data, 4))[0]
            if entity not in self.rockets:
                return
        elif name == 'entity_destroy':
            entity = next((eid for eid in (uint31(data) for _ in range(uint31(data)))
                           if eid in self.rockets), None)
            if entity is None:
                return
        elif name in ('sound_effect', 'update_health'):
            entity = None
        else:
            return
        self.events.append({'packet': name, 'id': entity, 'hex': payload.hex()})


def observe(label, command, protocol, rocket_type, output, root):
    reference = label == 'reference'
    log = output / f'{label}.log'
    config = root / 'config'
    config.mkdir(parents=True)
    (root / 'eula.txt').write_text('eula=true\n')
    settings = (
        f'server-ip=127.0.0.1\nserver-port={PORT}\nonline-mode=false\nwhite-list=false\n'
        'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n'
        'level-seed=1\ngenerate-structures=false\nview-distance=2\nsimulation-distance=2\n'
        'gamemode=survival\nspawn-protection=0\ndifficulty=peaceful\n'
    )
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
    record = {'phases': {}, 'commands': []}

    def console(*lines):
        record['commands'].extend(lines)
        process.stdin.write('\n'.join(lines) + '\n')
        process.stdin.flush()

    def await_log(word, seconds):
        end = time.monotonic() + seconds
        while word not in log.read_text(errors='replace'):
            if process.poll() is not None or time.monotonic() >= end:
                raise RuntimeError(f'Ответ консоли не получен. {log}')
            time.sleep(.05)

    try:
        await_log('Done (' if reference else 'Готово (', 60)
        client = connect(PORT, protocol, lambda sock, schema: Watcher(sock, schema, rocket_type), username=NAME)
        client.pump(12, lambda row: row['name'] == 'position')
        console('gamerule spawn_mobs false', 'gamerule natural_health_regeneration false',
                f'gamemode survival {NAME}', 'fill -2 98 -2 3 98 3 stone',
                f'tp {NAME} 0.5 99 0.5 0 20', f'item replace entity {NAME} armor.chest with minecraft:elytra',
                f'item replace entity {NAME} weapon.mainhand with minecraft:firework_rocket 2', 'say ROCKET_FLOOR')
        await_log('ROCKET_FLOOR', 12)
        client.pump(.9)
        start = len(client.events)
        client.send('block_place', b'\0' + packed(0, 98, 1) + b'\1'
                    + struct.pack('>fff', .5, 1., .5) + b'\0\0\1')
        client.pump(2.5)
        record['phases']['floor'] = client.events[start:]

        console(f'tp {NAME} 0.5 155 0.5 0 20', f'item replace entity {NAME} weapon.mainhand with {ROCKET} 2',
                'say ROCKET_AIR')
        await_log('ROCKET_AIR', 12)
        client.pump(.6)
        if client.position[:3] != [.5, 155., .5]:
            raise ValueError('Игрок не оказался в точке начала полёта')
        client.send('position_look', struct.pack('>dddffB', .5, 154.8, .5, 0., 20., 0))
        client.send('entity_action', varint(client.player_id) + b'\6\0')
        client.pump(.35)
        start = len(client.events)
        client.send('use_item', b'\0\2' + struct.pack('>ff', 0., 20.))
        client.pump(2.6)
        record['phases']['elytra'] = client.events[start:]
    finally:
        if client:
            record['player_id'] = client.player_id
            client.sock.close()
        if process.poll() is None:
            try:
                console('stop')
                process.wait(timeout=20)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
        contents = log.read_text(errors='replace')
        stopped = contents.rpartition('Stopping server')[2]
        saved = ('Saving worlds' in stopped and stopped.count('Saving chunks for level') == 3
                 if reference else 'Мир сохранён' in contents)
        record.update(exit=process.returncode, saved=saved)
        (output / f'{label}-packets.json').write_text(json.dumps(record, indent=2) + '\n')
    return record


def verdict(record):
    def launch_sound(payload):
        data = io.BytesIO(bytes.fromhex(payload))
        sound = uint31(data)
        # В протоколе один звук может прийти по номеру реестра или по имени.
        return sound == 637 or (sound == 0 and exact(data, uint31(data))
                                == b'minecraft:entity.firework_rocket.launch')

    def rocket_sequence(rows, attached):
        spawns = [row['id'] for row in rows if row['packet'] == 'spawn_entity']
        if len(spawns) != 1:
            return False
        rocket = spawns[0]
        attach = b'\x09\x13' + varint(record['player_id'] + 1) + b'\xff'
        metadata = [bytes.fromhex(row['hex']) for row in rows
                    if row['packet'] == 'entity_metadata' and row['id'] == rocket]
        sounds = [row for row in rows if row['packet'] == 'sound_effect'
                  and launch_sound(row['hex'])]
        blast = [i for i, row in enumerate(rows) if row['packet'] == 'entity_status'
                 and row['id'] == rocket and bytes.fromhex(row['hex'])[-1] == 17]
        removes = [i for i, row in enumerate(rows) if row['packet'] == 'entity_destroy' and row['id'] == rocket]
        return (bool(metadata) and any(data.endswith(attach) for data in metadata) == attached
                and len(sounds) == 1 and len(blast) == len(removes) == 1 and blast[0] < removes[0])

    if record['exit'] != 0 or not record['saved']:
        return {'save_and_stop': False}
    phases = record['phases']
    return {'save_and_stop': True, 'floor_launch': rocket_sequence(phases['floor'], False),
            'attached_launch_and_blast': rocket_sequence(phases['elytra'], True),
            'attached_star_damage': any(row['packet'] == 'update_health'
                and struct.unpack('>f', bytes.fromhex(row['hex'])[:4])[0] == 13.0 for row in phases['elytra'])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--build-receipt', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('Парная проверка новой сборки выполняется в CI')
    binary = args.binary.resolve(strict=True)
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    build = json.loads(args.build_receipt.read_text())
    if head != os.environ.get('GITHUB_SHA') or head != build['source'] or sha(binary) != build['sha256'] or binary.stat().st_size != build['bytes']:
        raise ValueError('Сборка ядра не соответствует коммиту')
    args.output.mkdir(parents=True, exist_ok=False)
    pins = json.loads(SOURCES.read_text())
    with tempfile.TemporaryDirectory(prefix='mcsa-elytra-rocket-') as temp:
        root = Path(temp)
        jar = root / 'official.jar'
        fetch(pins['official_jar'], jar)
        for pin in pins['json_files']:
            if pin['file'] in {'entities.json', 'protocol.json'}:
                fetch(pin, root / pin['file'])
        protocol = json.loads((root / 'protocol.json').read_text())
        kind = next(row['id'] for row in json.loads((root / 'entities.json').read_text())
                    if row['name'] == 'firework_rocket')
        cases = [('reference', ['java', '-Xmx1G', '-jar', str(jar), 'nogui']), ('candidate', [str(binary)])]
        receipt = {'source': head, 'binary_sha256': build['sha256'], 'reference_sha256': sha(jar)}
        for label, command in cases:
            receipt[label] = verdict(observe(label, command, protocol, kind, args.output, root / label))
        (args.output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
        if not all(check for label, _ in cases for check in receipt[label].values()):
            raise ValueError('Проверьте звук, пакеты привязки или взрыва ракеты и сохранение мира')


if __name__ == '__main__':
    main()
