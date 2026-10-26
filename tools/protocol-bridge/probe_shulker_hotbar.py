#!/usr/bin/env python3
"""Переносы хранилище→панель: собственный сетевой бот775/777, JAR непрозрачен."""
import argparse
import copy
import hashlib
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
from probe_container_session import Probe as BaseProbe
from probe_gameplay import connect
from smoke import read_varint, varint


class Probe(BaseProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.offhand = None
        self.full = 0
        self.sent = []

    def send(self, name, payload=b''):
        if name in ('window_click', 'block_place', 'close_window'):
            self.sent.append({'name': name, 'payload': payload.hex()})
        super().send(name, payload)

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        if name == 'window_items':
            self.full += 1
            if self.inventory['window'] == 0:
                self.offhand = self.inventory['slots'][45]
        if name == 'set_player_inventory':
            data = io.BytesIO(payload)
            slot = read_varint(data.read)
            value = self.slot(data)
            assert not data.read()
            if slot == 45:
                self.offhand = value
            self.observed.append({'name': name, 'payload': payload.hex()})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument('--binary', type=Path)
    target.add_argument('--jar', type=Path)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--version', type=int, choices=[775, 777], default=777)
    p.add_argument('--port', type=int, required=True)
    p.add_argument('--kinds', nargs='+', default=['shulker_box', 'chest', 'barrel'])
    p.add_argument('--modes', nargs='+')
    a = p.parse_args()
    assert 26200 <= a.port <= 26299 if a.binary else 26100 <= a.port <= 26199
    assert not a.jar or a.version == 777
    protocol = json.loads((a.data / ('protocol.json' if a.version == 777 else 'protocol-26.1.json')).read_text())
    items = a.data / 'items.json' if a.version == 777 else Path(__file__).parent.parent / 'data/items.json'
    names = {v['id']: v['name'] for v in json.loads(items.read_text())}
    item_ids = {name: item for item, name in names.items()}
    a.output.mkdir(parents=True, exist_ok=True)
    report = {'protocol': a.version, 'target': 'core' if a.binary else 'official', 'cases': [],
              'binary_sha256': hashlib.sha256((a.binary or a.jar).read_bytes()).hexdigest(),
              'probe_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'data_sha256': {f.name: hashlib.sha256(f.read_bytes()).hexdigest() for f in [a.data / ('protocol.json' if a.version == 777 else 'protocol-26.1.json'), items]}}
    if a.jar:
        assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
    with socket.socket() as check:
        check.bind(('127.0.0.1', a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-shulker-hotbar-') as directory:
        root = Path(directory)
        (root / 'config').mkdir()
        properties = (f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\n'
                      'enforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\n'
                      'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
                      'spawn-protection=0\nallow-flight=true\nmax-tick-time=0\n')
        (root / ('config/server.properties' if a.binary else 'server.properties')).write_text(properties)
        (root / 'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
        (root / 'config/skins.toml').write_text('mojang=false\nely=false\n')
        (root / 'eula.txt').write_text('eula=true\n')
        argv = [str(a.binary.resolve())] if a.binary else ['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx768M', '-jar', str(a.jar.resolve()), 'nogui']
        log = a.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(value):
                server.stdin.write(value + '\n')
                server.stdin.flush()
            def attach():
                c = connect(a.port, protocol, lambda s, p: Probe(s, p, a.version, names), a.version)
                c.pump(15, lambda r: r['name'] == 'position')
                c.pump(.5)
                return c
            try:
                deadline = time.monotonic() + 180
                while True:
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    try:
                        with socket.create_connection(('127.0.0.1', a.port), timeout=.2):
                            break
                    except OSError:
                        time.sleep(.2)
                if a.jar:
                    while 'Done (' not in log.read_text():
                        assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                        time.sleep(.1)
                client = attach()
                for cmd in ['fill -4 63 -4 4 63 4 stone', 'tp McsaProbe 0.5 64 0.5', 'gamemode survival McsaProbe']:
                    command(cmd)
                    client.pump(.25)
                sequence = 0
                def opened(fast=False):
                    nonlocal sequence
                    sequence += 1
                    client.send('block_place', b'\0' + struct.pack('>Q', 2 << 38 | 64) + b'\1' + struct.pack('>fff', .5, .5, .5) + b'\0\0' + varint(sequence))
                    if fast:
                        client.pump(3, lambda r: r['name'] == 'window_items')
                    else:
                        client.pump(.3)
                    assert client.inventory['window'] != 0 and len(client.inventory['slots']) == 63, client.inventory
                def close():
                    client.send('close_window', varint(client.inventory['window']))
                    client.pump(.15)
                def click(slot, button=0, mode=0, stale=False):
                    inv = client.inventory
                    inv['cursor'] = None
                    client.send('window_click', varint(inv['window']) + varint(-1 if stale else inv['revision']) + struct.pack('>hb', slot, button) + varint(mode) + b'\0\0')
                    client.pump(.15)
                def snapshot():
                    return copy.deepcopy({'slots': client.inventory['slots'], 'cursor': client.inventory['cursor']})
                def predicted_shift(source, updates):
                    inv = client.inventory
                    payload = varint(inv['window']) + varint(inv['revision']) + struct.pack('>hb', source, 0) + varint(1) + varint(len(updates))
                    for target, value in updates.items():
                        payload += struct.pack('>h', target)
                        payload += b'\0' if value is None else b'\1' + varint(item_ids[value['item']]) + varint(value['count']) + b'\0\0'
                        inv['slots'][target] = copy.deepcopy(value)
                    client.send('window_click', payload + b'\0')
                stone = {'item': 'stone', 'count': 12}
                occupied = {'item': 'dirt', 'count': 5}
                nested = {'item': 'red_shulker_box', 'count': 1, 'contents': [{'item': 'stone', 'count': 3}]}
                modes = ['pickup', 'shift', 'shift-main', 'shift-merge'] + [f'swap-{i}-{state}' for i in range(9) for state in ['empty', 'occupied']] + ['offhand', 'offhand-occupied', 'drag-left', 'drag-right', 'collect', 'stale', 'nested']
                if a.modes:
                    assert set(a.modes) <= set(modes + ['ground-pickup', 'shift-predicted', 'shift-stale', 'shift-burst', 'shift-last', 'shift-full', 'shift-nested'])
                    modes = a.modes
                for kind in a.kinds:
                    for reopen in [False, True]:
                        for mode in modes:
                            packet_start = len(client.observed)
                            action_start = len(client.sent)
                            if mode in ['nested', 'shift-nested'] and kind == 'shulker_box':
                                continue
                            if client.inventory and client.inventory['window']:
                                close()
                            for cmd in ['clear McsaProbe', 'item replace entity McsaProbe weapon.offhand with air', 'setblock 2 64 0 air', f'setblock 2 64 0 {kind}', 'item replace block 2 64 0 container.0 with stone 12']:
                                command(cmd)
                            if mode.startswith('swap') and mode.endswith('occupied'):
                                command(f'item replace entity McsaProbe hotbar.{mode.split("-")[1]} with dirt 5')
                            if mode == 'shift-main':
                                for hotbar in range(9):
                                    command(f'item replace entity McsaProbe hotbar.{hotbar} with dirt 64')
                            if mode == 'shift-merge':
                                command('item replace entity McsaProbe hotbar.8 with stone 60')
                            if mode == 'shift-full':
                                for group, size in [('hotbar', 9), ('inventory', 27)]:
                                    for own in range(size):
                                        command(f'item replace entity McsaProbe {group}.{own} with dirt 64')
                            if mode == 'shift-last':
                                command('item replace block 2 64 0 container.0 with air')
                                command('item replace block 2 64 0 container.26 with stone 12')
                            if mode == 'shift-burst':
                                for source in range(1, 27):
                                    command(f'item replace block 2 64 0 container.{source} with stone 12')
                            if mode == 'offhand-occupied':
                                command('item replace entity McsaProbe weapon.offhand with dirt 5')
                            if mode == 'collect':
                                command('item replace block 2 64 0 container.1 with stone 7')
                            if mode in ['nested', 'shift-nested']:
                                command('item replace block 2 64 0 container.0 with red_shulker_box[container=[{slot:0,item:{id:"minecraft:stone",count:3}}]]')
                            client.pump(.25)
                            fast = mode in ['shift-predicted', 'shift-stale', 'shift-burst', 'shift-last', 'shift-full', 'shift-nested']
                            opened(fast)
                            if reopen:
                                close()
                                opened(fast)
                            expected = [None] * 63
                            expected[0] = copy.deepcopy(stone)
                            cursor = None
                            if mode == 'ground-pickup':
                                command('summon item 0.5 64 0.5 {Item:{id:"minecraft:red_shulker_box",count:1,components:{"minecraft:container":[{slot:0,item:{id:"minecraft:stone",count:3}}]}},PickupDelay:0s,NoGravity:1b}')
                                client.pump(1)
                                expected[54] = nested
                            elif mode in ['pickup', 'nested']:
                                click(0)
                                click(54)
                                expected[0] = None
                                expected[54] = nested if mode == 'nested' else stone
                            elif mode.startswith('shift'):
                                before = client.full
                                if mode == 'shift-burst':
                                    expected[:27] = [copy.deepcopy(stone) for _ in range(27)]
                                    for source in range(27):
                                        updates = {source: None}
                                        expected[source] = None
                                        left = 12
                                        for target in range(62, 26, -1):
                                            count = expected[target]['count'] if expected[target] else 0
                                            moved = min(left, 64 - count)
                                            if moved:
                                                expected[target] = {'item': 'stone', 'count': count + moved}
                                                updates[target] = copy.deepcopy(expected[target])
                                                left -= moved
                                            if not left:
                                                break
                                        predicted_shift(source, updates)
                                    client.pump(.5)
                                elif mode == 'shift-predicted':
                                    predicted_shift(0, {0: None, 62: stone})
                                    client.pump(.5)
                                else:
                                    click(26 if mode == 'shift-last' else 0, mode=1, stale=mode == 'shift-stale')
                                if mode == 'shift-stale':
                                    report.setdefault('stale_full_sync', []).append(client.full > before)
                                expected[0] = None
                                if mode == 'shift-full':
                                    expected[0] = stone
                                    expected[27:] = [{'item': 'dirt', 'count': 64} for _ in range(36)]
                                elif mode == 'shift-burst':
                                    pass
                                elif mode == 'shift-nested':
                                    expected[62] = nested
                                elif mode == 'shift-main':
                                    expected[54:63] = [{'item': 'dirt', 'count': 64} for _ in range(9)]
                                    expected[53] = stone
                                elif mode == 'shift-merge':
                                    expected[62] = {'item': 'stone', 'count': 64}
                                    expected[61] = {'item': 'stone', 'count': 8}
                                else:
                                    expected[62] = stone
                            elif mode.startswith('swap'):
                                _, button, state = mode.split('-')
                                button = int(button)
                                click(0, button, 2)
                                expected[0] = occupied if state == 'occupied' else None
                                expected[54 + button] = stone
                            elif mode.startswith('offhand'):
                                click(0, 40, 2)
                                expected[0] = occupied if mode == 'offhand-occupied' else None
                            elif mode.startswith('drag'):
                                click(0)
                                offset = 0 if mode == 'drag-left' else 4
                                click(-999, offset, 5)
                                for target in [54, 55, 62]:
                                    click(target, offset + 1, 5)
                                click(-999, offset + 2, 5)
                                expected[0] = None
                                for target in [54, 55, 62]:
                                    expected[target] = {'item': 'stone', 'count': 4 if offset == 0 else 1}
                                cursor = None if offset == 0 else {'item': 'stone', 'count': 9}
                            elif mode == 'collect':
                                click(0)
                                click(54, mode=6)
                                click(54)
                                expected[0], expected[54] = None, {'item': 'stone', 'count': 19}
                            elif mode == 'stale':
                                before = client.full
                                click(0, 0, 0, stale=True)
                                expected[0] = None
                                cursor = stone
                                report.setdefault('stale_full_sync', []).append(client.full > before)
                            expected_snapshot = copy.deepcopy({'slots': expected, 'cursor': cursor})
                            immediate = snapshot()
                            # Закрытие возвращает курсор в инвентарь; перед ним
                            # кладём остаток в свободную ячейку панели.
                            if cursor:
                                click(61)
                                expected[61] = cursor
                            close()
                            opened()
                            reopened = snapshot()
                            expected_reopened = {'slots': expected, 'cursor': None}
                            row = {'kind': kind, 'mode': mode, 'after_reopen': reopen,
                                   'immediate': immediate, 'reopened': reopened,
                                   'expected': expected_snapshot, 'expected_reopened': expected_reopened,
                                   'pass': immediate == expected_snapshot and reopened == expected_reopened,
                                   'packets': client.observed[packet_start:], 'actions': client.sent[action_start:]}
                            if mode in ['stale', 'shift-stale']:
                                row['full_sync'] = report['stale_full_sync'][-1]
                                row['pass'] = row['pass'] and row['full_sync']
                            if mode.startswith('offhand'):
                                close()
                                client.sock.close()
                                time.sleep(.3)
                                client = attach()
                                row['offhand_relogin'] = client.offhand
                                row['pass'] = row['pass'] and client.offhand == stone
                                opened()
                            report['cases'].append(row)
                            print(json.dumps({k: row[k] for k in ['kind', 'mode', 'after_reopen', 'pass']}), flush=True)
                # Повторный вход получает сохранённую панель, а не предсказание.
                close()
                command('save-all')
                client.pump(.5)
                client.sock.close()
                time.sleep(.4)
                client = attach()
                report['relogin'] = snapshot()
                assert client.inventory['slots'][9:45] == expected[27:], client.inventory
                report['relogin_pass'] = True
            finally:
                if client:
                    report['packets'] = client.observed
                    client.sock.close()
                if server.poll() is None:
                    command('stop')
                    server.stdin.close()
                    try:
                        server.wait(timeout=40)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['exit_code'] = server.returncode
                report['passed'] = sum(c['pass'] for c in report['cases'])
                report['total'] = len(report['cases'])
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report[k] for k in ['passed', 'total', 'exit_code']}))
    assert report['passed'] == report['total'] and report['relogin_pass'] and report['exit_code'] == 0


if __name__ == '__main__':
    main()
