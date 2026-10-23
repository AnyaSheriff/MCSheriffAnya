#!/usr/bin/env python3
"""Частная776: порталы, книга/крафт и парольный Configuration-барьер."""
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
import traceback

from probe_step1_gameplay_776 import GameplayProbe
from probe_gameplay import connect
from probe_core_776 import PORT, PROTOCOL_SHA, stop_server
from probe_776_payloads import table
from probe_white_concrete_slab import uint
from probe_registry_client import read_string
from smoke import varint, send


def boolean(d):
    b = d.read(1)
    assert b in (b'\0', b'\1')
    return b == b'\1'


def idset(d):
    n = uint(d)
    assert n <= 65537
    return {'tag': read_string(d)} if n == 0 else {'items': [uint(d) for _ in range(n - 1)]}


def display(d, depth=0, version=776):
    assert depth < 32
    kind = uint(d)
    if version in (771,772,773,774): kind=(0,1,4,5,6,8,9,10)[kind]
    r = {'type': kind}
    if kind in (0, 1):
        pass
    elif kind == 2:
        r['base'] = display(d, depth + 1, version)
    elif kind == 4:
        r['item'] = uint(d)
    elif kind == 5:
        item, count = uint(d), uint(d)
        if version in (771,772,773,774): item,count=count,item
        assert uint(d) == uint(d) == 0
        r['stack'] = {'item': item, 'count': count}
    elif kind == 6:
        r['set'] = {'tag':read_string(d)} if version in (771,772,773,774) else idset(d)
    elif kind in (7, 9):
        r['parts'] = [display(d, depth + 1, version) for _ in range(2)]
    elif kind == 10:
        count = uint(d)
        assert count <= 4096
        r['parts'] = [display(d, depth + 1, version) for _ in range(count)]
    else:
        raise ValueError(('unobserved display', kind))
    return r


def recipe(d, version=776):
    kind = uint(d)
    r = {'type': kind}
    if kind in (0, 1):
        if kind == 1:
            r['width'], r['height'] = uint(d), uint(d)
        count = uint(d)
        assert count <= 9
        r['ingredients'] = [display(d, version=version) for _ in range(count)]
        count = 2
    else:
        count = {2: 4, 3: 3, 4: 5}[kind]
    r['parts'] = [display(d, version=version) for _ in range(count)]
    if kind == 2:
        r['duration'], r['xp'] = uint(d), struct.unpack('>f', d.read(4))[0]
    return r


class Probe(GameplayProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.books, self.ghosts, self.dimensions = [], [], []

    def move_to(self, values):
        # Следующий tick_end должен повторять новое место, а не старый teleport.
        self.position = list(values) + list(self.position[3:])
        payload = struct.pack('>dddB', *values, 1)
        self.sent.append(dict(name='position', id=self.outgoing['position'], hex=payload.hex()))
        self.send('position', payload)

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        d = io.BytesIO(payload)
        if name == 'recipe_book_add':
            count = uint(d)
            assert count <= 65536
            entries = []
            for _ in range(count):
                ident, value = uint(d), recipe(d,self.version)
                group, category = uint(d), uint(d)
                requirements = [idset(d) for _ in range(uint(d))] if boolean(d) else None
                flags = d.read(1)
                assert len(flags) == 1 and flags[0] & ~3 == 0
                entries.append(dict(id=ident, display=value, group=group, category=category,
                                    requirements=requirements, flags=flags[0]))
            replace = boolean(d)
            assert not d.read()
            self.books.append(dict(entries=entries, replace=replace))
        elif name == 'craft_recipe_response':
            self.ghosts.append(dict(window=uint(d), display=recipe(d,self.version)))
            assert not d.read()
        elif name == 'respawn':
            dimension_type = uint(d)
            dimension = read_string(d)
            self.dimensions.append(dict(type=dimension_type, name=dimension))
        if name in ('recipe_book_add', 'recipe_book_remove', 'craft_recipe_response'):
            self.raw[-1]['hex'] = payload.hex()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('test-binary', 'protocol', 'cache', 'output'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--primary', type=int, choices=(775, 777), required=True)
    p.add_argument('--password', action='store_true')
    p.add_argument('--end', action='store_true', help='Дополнительная проба портала Края; не новое условие допуска')
    p.add_argument('--client',type=int,choices=(771,772,773,774,776),default=776)
    a = p.parse_args()
    port=26128 if a.client==771 else PORT
    assert not (a.end and a.password)
    assert hashlib.sha256(a.protocol.read_bytes()).hexdigest() == (PROTOCOL_SHA if a.client==776 else '11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be' if a.client==771 else '11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be' if a.client==772 else '0f34fce39ca160cf9856013befd52bdc67cf30687306f47dacd007693d5a2d70' if a.client==773 else '4108833ded28ee21786e4905d0a07b3a06164a3842a8d6bab98096fd3c2e28ab')
    protocol = json.loads(a.protocol.read_text())
    items = {v['name']: v['id'] for v in table(a.cache, 'items')}
    entity = next(v['id'] for v in table(a.cache, 'entities') if v['name'] == 'item')
    with socket.socket() as s:
        s.bind(('127.0.0.1', port))
    a.output.mkdir(parents=True, exist_ok=False)
    report = dict(primary=a.primary, client=a.client, public_admission=False, checks=[], commands=[],
                  status='failed', runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  binary_sha256=hashlib.sha256(a.test_binary.read_bytes()).hexdigest(), auth_prompts=[])
    client = server = None
    with tempfile.TemporaryDirectory(prefix='mcsa-step1-dimensions-') as folder:
        root = Path(folder)
        (root / 'mcsa-gameplay-probe.marker').touch()
        if a.primary == 777:
            (root / 'mcsa-catalog-preview263.marker').touch()
        if a.password:
            (root / 'mcsa-password-probe.marker').touch()
        test = 'serve_java_1_21_6_probe' if a.client==771 else 'serve_java_1_21_8_probe' if a.client==772 else 'serve_java_1_21_9_probe' if a.client==773 else 'serve_java_1_21_11_probe' if a.client in (771,772,773,774) else ('serve_java_26_2_primary_26_3_probe' if a.primary == 777 else 'serve_java_26_2_probe')
        log_path = a.output / 'server.log'
        with log_path.open('w') as log:
            try:
                server = subprocess.Popen([str(a.test_binary.resolve()), '--ignored', '--exact',
                    'gameplay_probe::' + test, '--nocapture'], cwd=root, stdin=subprocess.PIPE,
                    stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
                deadline = time.monotonic() + 30
                while 'MCSA gameplay probe ready' not in log_path.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                attempt = 0
                def auth(sock, packet_id, payload):
                    nonlocal attempt
                    if packet_id == 0x11:
                        assert not payload
                        report['checks'].append('password-clears-dialog-before-finish')
                        return True
                    if packet_id != 0x12:
                        return False
                    assert not list((root / 'world/playerdata').iterdir())
                    assert 'вошёл с номером сущности' not in log_path.read_text()
                    registered = b'mcsheriffanya:login' in payload
                    report['auth_prompts'].append(dict(registered=registered, sha256=hashlib.sha256(payload).hexdigest()))
                    # Собственные тестовые пароли в артефакты не записываются.
                    password = 'wrong' if registered and attempt == 0 else 'fixture-password'
                    attempt += 1
                    action = 'login' if registered else 'register'
                    nbt = bytearray(b'\x0a')
                    fields = {'password': password}
                    if not registered:
                        fields['repeat'] = password
                    for key, value in fields.items():
                        nbt += b'\x08' + struct.pack('>H', len(key)) + key.encode() + struct.pack('>H', len(value)) + value.encode()
                    nbt += b'\0'
                    action = ('mcsheriffanya:' + action).encode()
                    send(sock, b'\x08' + varint(len(action)) + action + varint(len(nbt)) + nbt)
                    return True
                def login():
                    c = connect(port, protocol, lambda s, pr: Probe(s, pr, entity, a.client), a.client,
                                configuration_handler=auth if a.password else None)
                    c.pump(20, lambda _: c.position is not None and 'window_items' in c.seen)
                    c.pump(.5)
                    return c
                client = login()
                report['checks'].append('login-configuration-play')
                marker = 0
                def command(text):
                    nonlocal marker
                    marker += 1
                    token = f'MCSA_CORE_DIG_{marker}'
                    report['commands'].append(text)
                    server.stdin.write(text + '\nsay ' + token + '\n')
                    server.stdin.flush()
                    client.pump(10, lambda _: token in client.markers)
                    client.pump(.3)
                if a.password:
                    report['checks'].append('password-register-before-player-created')
                    client.sock.close()
                    client = None
                    deadline = time.monotonic() + 10
                    while 'McsaProbe вышел' not in log_path.read_text():
                        assert time.monotonic() < deadline
                        time.sleep(.1)
                    # Для повторного входа барьер сравнивается с предыдущим срезом.
                    # Файлы игрока теперь существуют; отдельный callback проверяет
                    # отсутствие новой join-записи до верного пароля.
                    old_auth = auth
                    previous_join_count = log_path.read_text().count('вошёл с номером сущности')
                    def relog_auth(sock, packet_id, payload):
                        if packet_id == 0x12:
                            assert log_path.read_text().count('вошёл с номером сущности') == previous_join_count
                            # Не читаем существующий файл игрока ради проверки пароля.
                            registered = b'mcsheriffanya:login' in payload
                            assert registered
                            report['auth_prompts'].append(dict(registered=True, sha256=hashlib.sha256(payload).hexdigest()))
                            password = 'wrong' if len(report['auth_prompts']) == 2 else 'fixture-password'
                            key, value = b'password', password.encode()
                            nbt = b'\x0a\x08' + struct.pack('>H', len(key)) + key + struct.pack('>H', len(value)) + value + b'\0'
                            action = b'mcsheriffanya:login'
                            send(sock, b'\x08' + varint(len(action)) + action + varint(len(nbt)) + nbt)
                            return True
                        return old_auth(sock, packet_id, payload)
                    auth = relog_auth
                    client = login()
                    assert len(report['auth_prompts']) == 3
                    report['checks'].append('password-relog-wrong-rejected-correct-enters')
                else:
                    for text in ['gamerule doMobSpawning false', 'gamemode creative McsaProbe',
                                 'fill -4 64 -4 5 64 4 stone', 'tp McsaProbe 0.5 65 0.5']:
                        command(text)
                    client.grounded_fixture = True
                    command('recipe take McsaProbe *')
                    before = len(client.books)
                    command('recipe give McsaProbe *')
                    assert len(client.books) > before and sum(len(b['entries']) for b in client.books[before:]) > 500
                    report['checks'].append('complete-primary-recipe-book-translates-and-decodes')
                    command('recipe take McsaProbe *')
                    command('recipe give McsaProbe minecraft:oak_planks')
                    chosen = [r for b in client.books[-1:] for r in b['entries'] if r['display']['parts'][0].get('stack', {}).get('item') == items['oak_planks']]
                    assert len(chosen) == 1, client.books[-1:]
                    ident = chosen[0]['id']
                    command('clear McsaProbe')
                    client.send('craft_recipe_request', varint(0) + varint(ident) + b'\0')
                    client.pump(5, lambda _: bool(client.ghosts))
                    assert client.ghosts[-1]['window'] == 0
                    command('give McsaProbe oak_log 2')
                    client.send('craft_recipe_request', varint(0) + varint(ident) + b'\0')
                    client.pump(1)
                    assert client.inventory.get(0) == dict(item=items['oak_planks'], count=4), client.inventory
                    assert sum(v['count'] for v in client.inventory.values() if v and v['item'] == items['oak_log']) == 2
                    client.send('window_click', varint(0) + varint(-1) + struct.pack('>hb', 0, 0) + bytes([0, 0, 0]))
                    client.pump(1)
                    assert client.cursor == dict(item=items['oak_planks'], count=4), client.cursor
                    assert sum(v['count'] for v in client.inventory.values() if v and v['item'] == items['oak_log']) == 1
                    report['crafted_inventory'], report['crafted_cursor'] = dict(client.inventory), client.cursor
                    report['checks'].append('recipe-ghost-autofill-result-and-consumption')
                    client.send('window_click', varint(0) + varint(-1) + struct.pack('>hb', 37, 0) + bytes([0, 0, 0]))
                    client.pump(.5)
                    command('fill 1 65 -1 4 69 -1 obsidian')
                    command('fill 2 66 -1 3 68 -1 nether_portal[axis=x]')
                    command('tp McsaProbe 2.5 66 0.5')
                    client.move_to([2.5, 66., -.5])
                    client.pump(20, lambda _: len(client.dimensions) >= 1)
                    assert client.dimensions[-1]['name'] == 'minecraft:the_nether', client.dimensions
                    client.pump(1)
                    destination = list(client.position[:3])
                    # Выйти из портала и войти снова обычным движением.
                    away = [destination[0], destination[1], destination[2] + 3]
                    client.grounded_fixture = False
                    client.move_to(away); client.pump(.5)
                    client.move_to(destination)
                    client.pump(20, lambda _: len(client.dimensions) >= 2)
                    assert client.dimensions[-1]['name'] == 'minecraft:overworld', client.dimensions
                    command('say portal-roundtrip-complete')
                    client.pump(1)
                    assert 'перешёл в minecraft:overworld' in log_path.read_text()
                    assert len(client.acks) >= 3
                    report['checks'].append('nether-portal-out-and-back-respawn-chunks-short-ack')
                    if a.end:
                        command('op McsaProbe')
                        # Авторский проём; перенос выполняется обычным движением, не execute-in/tp.
                        client.grounded_fixture = True
                        command('fill 9 64 9 13 64 13 stone')
                        command('fill 10 65 10 12 65 12 end_portal')
                        command('tp McsaProbe 11.5 66 11.5')
                        client.move_to([11.5, 65.5, 11.5])
                        client.pump(20, lambda _: len(client.dimensions) >= 3)
                        assert client.dimensions[-1]['name'] == 'minecraft:the_end', client.dimensions
                        command('say end-outbound-complete')
                        client.pump(1)
                        destination = list(client.position[:3])
                        # Авторим блок от имени игрока в его измерении;
                        # игрок входит в портал собственным пакетом позиции.
                        x, y, z = [int(v // 1) for v in destination]
                        command(f'execute as McsaProbe run setblock {x+2} {y} {z} end_portal')
                        client.grounded_fixture = False
                        client.move_to([destination[0], destination[1], destination[2] + 1])
                        client.pump(.5)
                        client.move_to([x + 2.5, y + .5, z + .5])
                        client.pump(20, lambda _: len(client.dimensions) >= 4)
                        assert client.dimensions[-1]['name'] == 'minecraft:overworld', client.dimensions
                        command('say end-roundtrip-complete')
                        client.pump(1)
                        log_text = log_path.read_text()
                        assert 'перешёл в minecraft:the_end' in log_text
                        assert log_text.count('перешёл в minecraft:overworld') >= 2
                        assert len(client.acks) >= 5
                        respawns = [i for i, row in enumerate(client.raw) if row['name'] == 'respawn']
                        assert len(respawns) == 4
                        for start, end in zip(respawns, respawns[1:] + [len(client.raw)]):
                            assert {'map_chunk', 'position'} <= {r['name'] for r in client.raw[start:end]}
                        report['checks'].append('end-portal-out-and-back-respawn-chunks-short-ack')
                report['status'] = 'pass'
            except BaseException as error:
                report['error'], report['traceback'] = repr(error), traceback.format_exc()
            finally:
                if client:
                    report['packets'], report['books'], report['ghosts'], report['dimensions'] = client.raw, client.books, client.ghosts, client.dimensions
                    report['sent'], report['acks'] = client.sent, client.acks
                    client.sock.close()
                if server:
                    stop_server(server)
                    report['server_exit'] = server.returncode
    (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report.get(k) for k in ('status', 'checks', 'error', 'server_exit')}))
    if report['status'] != 'pass':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
