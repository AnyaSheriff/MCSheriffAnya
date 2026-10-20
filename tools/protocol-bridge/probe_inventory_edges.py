#!/usr/bin/env python3
"""Собственные наблюдения брони и lifetime transaction ACK; JAR не распаковывается."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_legacy_inventory import InventoryProbe, slot as legacy_slot
from probe_container_session import Probe as ModernProbe
from probe_gameplay import connect
from smoke import handshake, read_varint, send, varint


class ModernEdges(ModernProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.stage = 'join'
        self.entities = []

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        if name in ('spawn_entity', 'entity_metadata', 'entity_destroy', 'collect'):
            self.entities.append({'stage': self.stage, 'name': name, 'payload': payload.hex()})


def dropped_items(events, legacy, client):
    """Только наши выпавшие предметы; остальные сущности не декодируем."""
    spawned = {}
    result = {}
    for row in events:
        data = io.BytesIO(bytes.fromhex(row['payload']))
        if row['name'] == 'spawn_entity':
            entity = read_varint(data.read); data.read(16)
            kind = data.read(1)[0] if legacy else read_varint(data.read)
            if kind == (2 if legacy else 71 if client.version == 775 else 72):
                spawned[entity] = row['stage']
        elif row['name'] == 'entity_metadata':
            entity = read_varint(data.read)
            if entity not in spawned:
                continue
            while True:
                key = data.read(1)[0]
                if key == 255: break
                kind = data.read(1)[0] if legacy else read_varint(data.read)
                if kind == (5 if legacy else 7):
                    value = legacy_slot(data) if legacy else client.slot(data)
                    # Повторная metadata той же сущности не означает новый дроп.
                    result[entity] = {'stage': spawned[entity], 'entity': entity, 'stack': value}
                elif kind in ([0, 6] if legacy else [0, 8]): data.read(1)
                elif kind == 1: read_varint(data.read)
                elif kind == (2 if legacy else 3): data.read(4)
                elif kind in ([3, 4] if legacy else [4]): data.read(read_varint(data.read))
                elif not legacy and kind == 6: assert data.read(1) == b'\0'
                else: raise AssertionError(('Unexpected authored item metadata', kind, row))
            assert not data.read()
    return list(result.values())


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--version', type=int, choices=[340, 775, 777], required=True)
    p.add_argument('--jar', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int)
    p.add_argument('--repeat', type=int, choices=range(1, 17), default=1)
    p.add_argument('--offhand-merge', action='store_true')
    a = p.parse_args()
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    hashes = {340: '886945bfb2b978778c3a0288fd7fab09d315b25f',
              775: '97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',
              777: '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == hashes[a.version]
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files'] + manifest.get('baseline_files', []):
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / ('protocol-26.1.json' if a.version == 775 else 'protocol.json')).read_text())
    legacy = a.version == 340
    port = a.port or (26023 if legacy else 26024)
    names = None
    if not legacy:
        items = a.data / 'items.json' if a.version == 777 else Path(__file__).resolve().parents[2] / 'tools/data/items.json'
        names = {row['id']: row['name'] for row in json.loads(items.read_text())}
    report = {'protocol': a.version, 'jar_sha1': hashes[a.version], 'snapshots': [], 'lifecycle': [], 'packets': [], 'commands': []}
    a.output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as check:
        check.bind(('127.0.0.1', port))
    with tempfile.TemporaryDirectory(prefix='mcsa-inventory-edges-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\n')
        java = '/usr/lib/jvm/java-8-openjdk/jre/bin/java' if legacy else '/usr/lib/jvm/java-25-openjdk/bin/java'
        log = a.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen([java, '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui'], cwd=root,
                                      stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            transaction = 0

            def command(value):
                report['commands'].append(value)
                server.stdin.write(value + '\n'); server.stdin.flush()

            def click(stage, index, button=0, mode=0, expected=None):
                nonlocal transaction
                transaction += 1
                client.stage = stage
                if legacy:
                    client.click(transaction, index, button, mode, expected)
                else:
                    client.send('window_click', b'\0' + varint(client.inventory['revision'])
                                + struct.pack('>hb', index, button) + varint(mode) + b'\0\0')
                client.pump(.35)
                return transaction

            def ack(ident):
                client.send('transaction', struct.pack('>BhB', 0, ident, 1)); client.pump(.15)

            def snapshot(stage):
                if legacy:
                    ident = click(stage + '-resync', 8, expected={'item': 264, 'count': 64, 'damage': 0})
                    assert client.transactions[-1]['transaction'] == ident and not client.transactions[-1]['accepted']
                    report['snapshots'].append({'stage': stage, 'slots': list(client.slots), 'cursor': client.cursor})
                    ack(ident)
                else:
                    client.send('window_click', b'\0' + varint(-1) + struct.pack('>hb', 8, 0) + b'\0\0\0')
                    client.pump(.35)
                    report['snapshots'].append({'stage': stage, 'slots': list(client.inventory['slots']), 'cursor': client.inventory['cursor']})

            try:
                deadline = time.monotonic() + 150
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.1)
                for rule in (['doMobSpawning false', 'randomTickSpeed 0', 'keepInventory true'] if legacy else
                             ['minecraft:spawn_mobs false', 'minecraft:random_tick_speed 0', 'minecraft:keep_inventory true']):
                    command('gamerule ' + rule)
                if legacy:
                    sock = socket.create_connection(('127.0.0.1', port), timeout=10)
                    handshake(sock, 340, port, 2); send(sock, b'\0\x09McsaProbe')
                    client = InventoryProbe(sock, protocol, report)
                else:
                    client = connect(port, protocol, lambda s, p: ModernEdges(s, p, a.version, names), a.version)
                client.pump(20, lambda r: r['name'] == 'position'); client.pump(1)
                command('gamemode survival McsaProbe'); client.pump(.3)
                cases = [
                    ('empty-helmet-hotbar-stack', False, False, 8, 0, False),
                    ('occupied-helmet-hotbar-stack', True, False, 8, 0, False),
                    ('occupied-helmet-full-inventory-stack', True, True, 8, 0, False),
                    ('occupied-helmet-full-inventory-single', True, True, 1, 0, False),
                    ('oversized-pumpkin-helmet-full-inventory-stack', True, True, 8, 0, True),
                ]
                if not legacy:
                    cases.extend([
                        ('empty-helmet-offhand-stack', False, False, 8, 40, False),
                        ('occupied-helmet-offhand-stack', True, False, 8, 40, False),
                        ('occupied-helmet-offhand-full-inventory-stack', True, True, 8, 40, False),
                    ])
                if a.offhand_merge:
                    assert not legacy
                    cases = []
                for stage, occupied, full, amount, button, oversized in cases:
                    client.stage = stage + '-setup'
                    command('clear McsaProbe')
                    command('kill @e[type=item]'); client.pump(.2)
                    def replace(place, item, count):
                        command(f'replaceitem entity McsaProbe slot.{place} minecraft:{item} {count}' if legacy else
                                f'item replace entity McsaProbe {place} with {item} {count}')
                    if full:
                        for place in [f'hotbar.{n}' for n in range(9)] + [f'inventory.{n}' for n in range(27)]:
                            replace(place, 'bedrock', 64)
                    pumpkin = 'pumpkin' if legacy else 'carved_pumpkin'
                    if occupied: replace('armor.head', pumpkin if oversized else 'iron_helmet', 8 if oversized else 1)
                    replace('weapon.offhand' if button == 40 else 'hotbar.0', pumpkin, amount)
                    client.pump(.5)
                    click(stage, 5, button, 2)
                    snapshot(stage)
                    for repeat in range(1, a.repeat):
                        repeated = f'{stage}-repeat-{repeat}'
                        click(repeated, 5, button, 2)
                        snapshot(repeated)
                if a.offhand_merge:
                    client.stage = 'full-offhand-give'
                    command('clear McsaProbe'); command('kill @e[type=item]'); client.pump(.2)
                    for place in [f'hotbar.{n}' for n in range(9)] + [f'inventory.{n}' for n in range(27)]:
                        command(f'item replace entity McsaProbe {place} with bedrock 64')
                    command('item replace entity McsaProbe weapon.offhand with carved_pumpkin 7'); client.pump(.5)
                    command('give McsaProbe carved_pumpkin 1'); client.pump(.5)
                    snapshot(client.stage)
                if legacy:
                    command('clear McsaProbe'); client.pump(.3)
                    for transition in ['close-window', 'death-respawn']:
                        rejected = click(transition + '-reject', 8, expected={'item': 264, 'count': 64, 'damage': 0})
                        assert client.transactions[-1]['transaction'] == rejected and not client.transactions[-1]['accepted']
                        before = len(client.transactions)
                        if transition == 'close-window':
                            client.send('close_window', b'\0'); client.pump(.3)
                        else:
                            command('kill McsaProbe'); client.pump(.7)
                            client.send('client_command', b'\0')
                            client.pump(5, lambda r: r['name'] == 'respawn'); client.pump(.7)
                        after = click(transition + '-without-ack', 8)
                        unlocked = len(client.transactions) > before
                        row = {'transition': transition, 'rejected': rejected, 'click_without_ack': after,
                               'unlocked_without_ack': unlocked, 'responses': client.transactions[before:]}
                        ack(rejected)
                        after_ack = click(transition + '-after-old-ack', 8)
                        row['old_ack_then_click_accepted'] = client.transactions[-1]['transaction'] == after_ack and client.transactions[-1]['accepted']
                        report['lifecycle'].append(row)
            except BaseException as error:
                report['error'] = repr(error)
                raise
            finally:
                if client:
                    if not legacy:
                        report['packets'] = client.observed
                        report['entities'] = client.entities
                    client.sock.close()
                if server.poll() is None:
                    command('stop'); server.stdin.close()
                    try: server.wait(timeout=30)
                    except subprocess.TimeoutExpired: server.kill(); server.wait()
                report['exit_code'] = server.returncode
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
                if client:
                    report['dropped_items'] = dropped_items(report.get('entities', []), legacy, client)
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'protocol': a.version, 'snapshots': len(report['snapshots']), 'lifecycle': report['lifecycle']}))


if __name__ == '__main__':
    main()
