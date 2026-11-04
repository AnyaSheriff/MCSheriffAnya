#!/usr/bin/env python3
"""Заряд, выстрел и подбор стрел: обычные команды и пакеты без чтения реализации."""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from smoke import read_varint, varint


class Probe(InventoryProbe):
    def __init__(self, sock, protocol, version, names, entities):
        super().__init__(sock, protocol, version, names)
        self.entities = entities
        self.spawns = []

    def slot(self, data, template=False, depth=0):
        assert depth < 16
        if template:
            item, count = read_varint(data.read), read_varint(data.read)
        else:
            count = read_varint(data.read)
            if not count:
                return None
            item = read_varint(data.read)
        added, removed = read_varint(data.read), read_varint(data.read)
        assert 0 <= added <= 8 and removed == 0
        result = {'item': self.names[item], 'count': count}
        for _ in range(added):
            kind = read_varint(data.read)
            if kind == 3:
                result['damage'] = read_varint(data.read)
            elif kind == 13:
                result['enchantments'] = [[read_varint(data.read), read_varint(data.read)] for _ in range(read_varint(data.read))]
            elif kind == 22:
                assert data.read(2) == b'\x0a\0'
                result['intangible'] = True
            elif kind == (53 if self.version == 777 else 51):
                assert data.read(1) == b'\1'
                result['potion'] = read_varint(data.read)
                assert data.read(1) == b'\0' and read_varint(data.read) == 0 and data.read(1) == b'\0'
            elif kind == (51 if self.version == 777 else 49):
                size = read_varint(data.read)
                assert 0 <= size <= 16
                result['projectiles'] = [self.slot(data, True, depth + 1) for _ in range(size)]
            else:
                raise AssertionError(f'unexpected component {kind}')
        return result

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        if name == 'spawn_entity':
            data = io.BytesIO(payload)
            entity = read_varint(data.read)
            data.read(16)
            kind = self.entities[read_varint(data.read)]
            if kind in ('arrow', 'spectral_arrow', 'firework_rocket'):
                position = struct.unpack('>ddd', data.read(24))
                first = data.read(1)[0]
                velocity = [0.0, 0.0, 0.0]
                if first:
                    rest = data.read(5)
                    packed = first | rest[0] << 8 | int.from_bytes(rest[1:], 'big') << 16
                    scale = (packed & 3) + ((read_varint(data.read) << 2) if packed & 4 else 0)
                    velocity = [(((packed >> (3 + 15 * i)) & 32767) / 32766 * 2 - 1) * scale for i in range(3)]
                self.spawns.append({'id': entity, 'kind': kind, 'position': position, 'velocity': velocity, 'payload': payload.hex()})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', type=int, choices=[775, 777], required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--jar', type=Path)
    source.add_argument('--test-binary', type=Path)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, required=True)
    a = parser.parse_args()
    if a.test_binary:
        assert a.port == 26026
    sources = json.loads((a.data / 'sources.json').read_text())
    for entry in sources['files'] + sources['baseline_files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / ('protocol.json' if a.version == 777 else 'protocol-26.1.json')).read_text())
    item_table = a.data / 'items.json' if a.version == 777 else Path(__file__).resolve().parents[2] / 'tools/data/items.json'
    names = {v['id']: v['name'] for v in json.loads(item_table.read_text())}
    entities = {v['id']: v['name'] for v in json.loads((a.data / ('entities.json' if a.version == 777 else 'entities-26.1.json')).read_text())}
    fixture = json.loads((Path(__file__).parent / 'testdata/crossbow-observations.json').read_text())
    cases = fixture['cases']
    report = {'version': a.version, 'gameplay_version': '26.1.2' if a.test_binary or a.version == 775 else '26.3', 'cases': [], 'commands': cases, 'items_sha256': hashlib.sha256(item_table.read_bytes()).hexdigest()}
    if a.jar:
        expected = {775: '97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51', 777: '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
        assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == expected
        report['jar_sha1'] = expected
    else:
        report['test_binary_sha256'] = hashlib.sha256(a.test_binary.read_bytes()).hexdigest()
    with socket.socket() as check:
        check.bind(('127.0.0.1', a.port))
    a.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='mcsa-crossbow-session-') as directory:
        root = Path(directory)
        if a.jar:
            (root / 'eula.txt').write_text('eula=true\n')
            (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n')
            argv = ['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui']
            ready = 'Done ('
        else:
            (root / 'mcsa-gameplay-probe.marker').touch()
            (root / 'config').mkdir()
            (root / 'world/playerdata').mkdir(parents=True)
            (root / 'skins').mkdir()
            argv = [str(a.test_binary.resolve()), '--ignored', '--exact', 'gameplay_probe::serve_java_26_3_probe', '--nocapture']
            ready = 'MCSA gameplay probe ready'
        log = root / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            try:
                deadline = time.monotonic() + 120
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.1)
                client = connect(a.port, protocol, lambda s, p: Probe(s, p, a.version, names, entities), a.version)
                client.pump(10, lambda r: r['name'] == 'position')
                client.pump(1)
                for cmd in ['fill -4 63 -4 4 63 8 stone', 'fill -4 64 5 4 68 5 stone', 'setblock 2 64 0 chest', 'gamemode survival McsaProbe']:
                    command(cmd)
                    client.pump(.3)
                sequence = 0
                def snapshot():
                    nonlocal sequence
                    sequence += 1
                    client.send('block_place', b'\0' + struct.pack('>Q', 2 << 38 | 64) + b'\1' + struct.pack('>fff', .5, .5, .5) + b'\0\0' + varint(sequence))
                    client.pump(.5)
                    assert client.windows and client.inventory['window'] == client.windows[-1]['window'], 'chest did not open'
                    result = json.loads(json.dumps(client.inventory['slots'][27:]))
                    client.send('close_window', varint(client.inventory['window']))
                    client.pump(.2)
                    return result
                for case in cases:
                    for kind in ['arrow', 'spectral_arrow', 'firework_rocket', 'item']:
                        command(f'kill @e[type={kind}]')
                    command('clear McsaProbe')
                    command('tp McsaProbe 0.5 64 0.5 0 0')
                    command('item replace entity McsaProbe weapon.mainhand with ' + case['command'])
                    client.pump(.5)
                    current = {'name': case['name'], 'before': snapshot()}
                    report['cases'].append(current)
                    crossbow = current['before'][27]
                    assert crossbow == case['before'], 'charged contents differ from official recording'
                    assert crossbow and crossbow['item'] == 'crossbow' and crossbow.get('projectiles'), 'charged contents lost before firing'
                    start = len(client.spawns)
                    sequence += 1
                    client.send('use_item', b'\0' + varint(sequence) + struct.pack('>ff', 0, 0))
                    client.pump(1.5)
                    current['spawns'] = client.spawns[start:]
                    current['after_shot'] = snapshot()
                    assert len(current['spawns']) == len(crossbow['projectiles']), 'wrong projectile count'
                    assert not current['after_shot'][27].get('projectiles'), 'crossbow still charged'
                    command('tp McsaProbe 0.5 64 4.0 0 0')
                    client.pump(2)
                    command('tp McsaProbe 0.5 64 0.5 0 0')
                    client.pump(.3)
                    current['after_pickup'] = snapshot()
                    assert current['after_shot'][27] == case['after_shot'], 'wrong wear or unloaded data'
                    pickup = sorted([v for v in current['after_pickup'] if v and v['item'] != 'crossbow'], key=lambda v: json.dumps(v, sort_keys=True))
                    assert pickup == case['pickup'], 'wrong pickup types/counts/components'
                    assert sorted(v['kind'] for v in current['spawns']) == sorted(case['spawns']), 'wrong projectile types'
                    expected_speed = 1.6 if 'firework_rocket' in case['spawns'] else 3.15
                    angles = {'crossbow_two_multishot': [-10, 10], 'crossbow_four_multishot': [-10/3, 10/3, -10, 10]}.get(case['name'], [0] * len(case['spawns']))
                    # В ядре стрелы и ракеты рассылаются разными очередями:
                    # порядок создания проверяем по id, порядок пакетов не скрываем.
                    current['spawn_order_matches'] = [v['kind'] for v in current['spawns']] == case['spawns']
                    for spawn, expected_angle in zip(sorted(current['spawns'], key=lambda v: v['id']), angles):
                        x, y, z = spawn['velocity']
                        assert abs(math.sqrt(x*x+y*y+z*z) - expected_speed) < .15, 'wrong projectile speed'
                        angle = math.degrees(math.atan2(-x, z))
                        assert abs(angle - expected_angle) < 2, 'wrong projectile spread'


                assert 'Перевод Java-пакета:' not in log.read_text(), 'translation error'
            except BaseException as error:
                report['error'] = str(error)
                raise
            finally:
                if client:
                    report['packets'] = client.observed
                    client.sock.close()
                if server.poll() is None:
                    command('stop')
                    server.stdin.close()
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['exit_code'] = server.returncode
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
                (a.output / 'server.log').write_text(log.read_text())
    assert report['exit_code'] == 0
    print(json.dumps({'cases': len(report['cases']), 'exit_code': report['exit_code']}))


if __name__ == '__main__':
    main()
