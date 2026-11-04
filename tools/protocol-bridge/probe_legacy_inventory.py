#!/usr/bin/env python3
"""Щелчки и transaction ACK на отдельном официальном340; только свои предметы."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_legacy_session import Probe
from smoke import handshake, read_varint, send, varint


def slot(data):
    item = struct.unpack('>h', data.read(2))[0]
    if item == -1:
        return None
    count, damage = struct.unpack('>Bh', data.read(3))
    assert data.read(1) == b'\0', 'This probe only authors plain stacks'
    return {'item': item, 'count': count, 'damage': damage}


def stack(value):
    return b'\xff\xff' if value is None else struct.pack('>hBhB', value['item'], value['count'], value['damage'], 0)


class InventoryProbe(Probe):
    def __init__(self, *args):
        super().__init__(*args)
        self.stage = 'join'
        self.slots = None
        self.cursor = None
        self.transactions = []

    def observe_packet(self, name, payload):
        if name in ('spawn_entity', 'entity_metadata', 'entity_velocity', 'rel_entity_move',
                    'entity_move_look', 'entity_teleport', 'entity_destroy', 'collect'):
            self.report.setdefault('entities', []).append({'stage': self.stage, 'name': name, 'payload': payload.hex()})
        data = io.BytesIO(payload)
        row = {'stage': self.stage, 'name': name}
        if name == 'window_items':
            window, count = struct.unpack('>Bh', data.read(3))
            values = [slot(data) for _ in range(count)]
            if window == 0:
                self.slots = list(values)
            row.update(window=window, slots=values)
        elif name == 'set_slot':
            window, index = struct.unpack('>bh', data.read(3))
            value = slot(data)
            if window == -1 and index == -1:
                self.cursor = value
            elif window in (0, -2) and self.slots is not None and 0 <= index < len(self.slots):
                self.slots[index] = value
            row.update(window=window, slot=index, value=value)
        elif name == 'transaction':
            window, transaction, accepted = struct.unpack('>BhB', data.read(4))
            assert accepted in (0, 1)
            row.update(window=window, transaction=transaction, accepted=bool(accepted))
            self.transactions.append(row)
        else:
            return
        assert not data.read()
        row['payload'] = payload.hex()
        self.report.setdefault('inventory', []).append(row)

    def click(self, transaction, index, button, mode, expected):
        body = struct.pack('>Bhb h', 0, index, button, transaction) + varint(mode) + stack(expected)
        self.report.setdefault('actions', []).append({'stage': self.stage, 'transaction': transaction,
            'slot': index, 'button': button, 'mode': mode, 'expected': expected, 'payload': body.hex()})
        self.send('window_click', body)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar', type=Path)
    target.add_argument('--test-binary', type=Path)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int)
    p.add_argument('--modes', action='store_true')
    p.add_argument('--entities', action='store_true', help='Observe authored dropped-item motion and pickup')
    p.add_argument('--skip-drops', action='store_true', help='Separate inventory checks from item entity presentation')
    p.add_argument('--item', choices=['stone', 'bedrock'], default='stone')
    a = p.parse_args()
    a.port = a.port or (26027 if a.test_binary else 26023)
    if a.test_binary: assert a.port == 26027
    else: assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == '886945bfb2b978778c3a0288fd7fab09d315b25f'
    sources = json.loads((a.data / 'sources.json').read_text())
    for row in sources['files']:
        assert hashlib.sha256((a.data / row['file']).read_bytes()).hexdigest() == row['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    report = {'protocol': 340, 'target': 'core' if a.test_binary else 'official', 'item': a.item, 'skipped': ['drop-one','drop-stack'] if a.skip_drops else [], 'checks': [], 'packets': []}
    a.output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as check:
        check.bind(('127.0.0.1', a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-inventory-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=FLAT\nlevel-seed=1\nview-distance=2\ngamemode=0\nspawn-protection=0\n')
        log = a.output / 'server.log'
        with log.open('w') as output:
            if a.test_binary:
                (root / 'mcsa-gameplay-probe.marker').touch()
                argv = [str(a.test_binary.resolve()), '--ignored', '--exact', 'gameplay_probe::serve_java_1_12_2_probe', '--nocapture']
            else:
                argv = ['/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui']
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            sock = None

            def command(value):
                server.stdin.write(value + '\n'); server.stdin.flush()

            try:
                deadline = time.monotonic() + 120
                ready = 'MCSA gameplay probe ready on 26027' if a.test_binary else 'Done ('
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                command('gamerule doMobSpawning false')
                command('gamerule randomTickSpeed 0')
                sock = socket.create_connection(('127.0.0.1', a.port), timeout=10)
                handshake(sock, 340, a.port, 2); send(sock, b'\0\x09McsaProbe')
                client = InventoryProbe(sock, protocol, report)
                client.pump(20, lambda r: r['name'] == 'position'); client.pump(1)
                command('gamemode survival McsaProbe')
                command(f'item replace entity McsaProbe hotbar.0 with minecraft:{a.item} 8' if a.test_binary else f'replaceitem entity McsaProbe slot.hotbar.0 minecraft:{a.item} 8')
                client.pump(1)
                stone = {'item': 1 if a.item == 'stone' else 7, 'count': 8, 'damage': 0}
                assert client.slots[36] == stone
                for stage, transaction, index, expected in [('pickup', 1, 36, stone), ('place', 2, 37, None)]:
                    client.stage = stage; client.click(transaction, index, 0, 0, expected)
                    client.pump(5, lambda r: r['name'] == 'transaction')
                    assert client.transactions[-1]['transaction'] == transaction and client.transactions[-1]['accepted']
                    client.pump(.2)
                    report['checks'].append(stage + '-accepted')
                client.stage = 'wrong-prediction'
                client.click(3, 37, 0, 0, {'item': 264, 'count': 64, 'damage': 0})
                client.pump(5, lambda r: r['name'] == 'transaction'); client.pump(.5)
                assert client.transactions[-1]['transaction'] == 3 and not client.transactions[-1]['accepted']
                assert client.slots[37] is None and client.cursor == stone
                report['checks'].append('wrong-prediction-still-applies-server-click-and-resyncs')
                before = len(client.transactions)
                client.stage = 'blocked-before-ack'; client.click(4, 38, 0, 0, None); client.pump(.5)
                assert len(client.transactions) == before
                report['checks'].append('click-blocked-until-transaction-ack')
                client.stage = 'wrong-ack-id'; client.send('transaction', struct.pack('>BhB', 0, 2, 1))
                client.click(5, 38, 0, 0, None); client.pump(.5)
                assert len(client.transactions) == before
                report['checks'].append('wrong-ack-id-keeps-clicks-blocked')
                client.stage = 'matching-ack-false'; client.send('transaction', struct.pack('>BhB', 0, 3, 0))
                client.click(6, 38, 0, 0, None); client.pump(.5)
                false_unlocked = len(client.transactions) > before
                report['matching_ack_false_unlocked'] = false_unlocked
                if not false_unlocked:
                    client.stage = 'matching-ack-true'; client.send('transaction', struct.pack('>BhB', 0, 3, 1))
                    client.click(7, 38, 0, 0, None); client.pump(5, lambda r: r['name'] == 'transaction')
                assert client.transactions[-1]['accepted']
                report['checks'].append('matching-ack-unlocks-clicks')
                if a.modes:
                    next_transaction = 20
                    def action(stage, index, button, mode, expected):
                        nonlocal next_transaction
                        client.stage = stage; transaction = next_transaction; next_transaction += 1
                        client.click(transaction, index, button, mode, expected)
                        client.pump(5, lambda r: r['name'] == 'transaction'); client.pump(.1)
                        ack = client.transactions[-1]
                        assert ack['transaction'] == transaction and ack['accepted'], (stage, ack)
                        return transaction

                    def resync():
                        nonlocal next_transaction
                        client.stage += '-observe'; transaction = next_transaction; next_transaction += 1
                        # Камень нельзя надеть на голову: неверное предсказание
                        # вынуждает показать авторитетный снимок, не меняя вещи.
                        client.click(transaction, 5, 0, 0, {'item': 264, 'count': 64, 'damage': 0})
                        client.pump(5, lambda r: r['name'] == 'transaction'); client.pump(.2)
                        assert not client.transactions[-1]['accepted']
                        report.setdefault('snapshots', []).append({'stage': client.stage,
                            'slots': list(client.slots), 'cursor': client.cursor})
                        client.send('transaction', struct.pack('>BhB', 0, transaction, 1))

                    for name, button, mode, expected in [('right-pickup', 1, 0, stone),
                            ('shift', 0, 1, None), ('hotbar-swap', 1, 2, None),
                            ('drop-one', 0, 4, None), ('drop-stack', 1, 4, None),
                            ('creative-clone', 2, 3, None), ('double-click', 0, 6, None)]:
                        if a.skip_drops and mode == 4: continue
                        client.send('close_window', b'\0'); client.pump(.2)
                        command('clear McsaProbe'); command('kill @e[type=item]'); client.pump(.2)
                        command('gamemode creative McsaProbe' if mode == 3 else 'gamemode survival McsaProbe')
                        command(f'item replace entity McsaProbe hotbar.0 with minecraft:{a.item} 8' if a.test_binary else f'replaceitem entity McsaProbe slot.hotbar.0 minecraft:{a.item} 8')
                        client.pump(.5)
                        action(name, 36, button, mode, expected); resync()
                        report['checks'].append(name + '-observed')

                    for name, base in [('left-drag', 0), ('right-drag', 4), ('creative-drag', 8), ('right-place', -1), ('double-collect', -2)]:
                        client.send('close_window', b'\0'); client.pump(.2)
                        command('clear McsaProbe'); command('kill @e[type=item]'); client.pump(.2)
                        command('gamemode creative McsaProbe' if base == 8 else 'gamemode survival McsaProbe')
                        command(f'item replace entity McsaProbe hotbar.0 with minecraft:{a.item} 8' if a.test_binary else f'replaceitem entity McsaProbe slot.hotbar.0 minecraft:{a.item} 8')
                        client.pump(.5)
                        action(name + '-pickup', 36, int(base == -2), 0, stone)
                        if base >= 0:
                            for index, button in [(-999, base), (37, base+1), (38, base+1), (-999, base+2)]:
                                action(name, index, button, 5, None)
                        else:
                            action(name, 37, 1 if base == -1 else 0, 0 if base == -1 else 6, None)
                        resync(); report['checks'].append(name + '-observed')

                if a.entities:
                    assert not a.test_binary, 'Entity observations currently use the official340 only'
                    command('clear McsaProbe'); command('kill @e[type=item]')
                    command('gamemode survival McsaProbe'); client.pump(.3)
                    x,y,z=client.position[:3]
                    client.stage='item-no-gravity'
                    command(f'summon item {x+2} {y+0.5} {z} {{Item:{{id:"minecraft:bedrock",Count:3b}},PickupDelay:0s,NoGravity:1b,Motion:[0.01d,0.0d,0.0d]}}')
                    client.pump(.7)
                    assert any(r['stage']==client.stage and r['name']=='spawn_entity' for r in report.get('entities',[]))
                    report['checks'].append('item-spawn-motion-metadata-observed')
                    client.stage='item-pickup'
                    command(f'tp McsaProbe {x+2.2} {y} {z}');client.pump(1.5)
                    assert any(r['stage']==client.stage and r['name']=='collect' for r in report['entities'])
                    report['checks'].append('item-pickup-observed')
            except BaseException as error:
                report['error'] = str(error); raise
            finally:
                if sock: sock.close()
                if server.poll() is None:
                    command('stop'); server.stdin.close()
                    try: server.wait(timeout=30)
                    except subprocess.TimeoutExpired: server.kill(); server.wait()
                report['exit_code'] = server.returncode
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    if a.test_binary and a.modes:
        fixture = json.loads((Path(__file__).parent / 'testdata/legacy-inventory.json').read_text())
        expected = {v['stage']: v for v in fixture['snapshots']}
        try:
            for snapshot in report['snapshots']:
                assert snapshot == expected[snapshot['stage']], snapshot
            report['snapshot_comparison'] = 'PASS'
        except AssertionError as error:
            report['error'] = str(error)
            raise
        finally:
            (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'checks': report['checks'], 'matching_ack_false_unlocked': report['matching_ack_false_unlocked']}))


if __name__ == '__main__':
    main()
