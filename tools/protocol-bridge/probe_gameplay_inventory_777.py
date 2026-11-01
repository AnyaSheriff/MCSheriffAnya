#!/usr/bin/env python3
"""Свежие Shift-клики брони, stale state и XP после смерти на777. JAR не распаковывается."""
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
from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from smoke import varint, read_varint



class Probe(InventoryProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.wire = []
        self.xp = None

    def send(self, name, payload=b''):
        self.wire.append({'direction': 'out', 'name': name, 'payload': payload.hex()})
        super().send(name, payload)

    def observe_packet(self, name, payload):
        if name in ('window_items', 'set_slot', 'set_player_inventory', 'set_cursor_item',
                    'experience', 'respawn', 'death_combat_event', 'update_health'):
            self.wire.append({'direction': 'in', 'name': name, 'payload': payload.hex()})
        super().observe_packet(name, payload)
        data = io.BytesIO(payload)
        if name == 'set_player_inventory':
            slot = read_varint(data.read)
            value = self.slot(data)
            assert not data.read()
            # Протокольный индекс PlayerInventory -> индекс окна 0.
            place = slot + 36 if slot < 9 else 44 - slot if 36 <= slot <= 39 else 45 if slot == 40 else slot
            if self.inventory:
                self.inventory['slots'][place] = value
        elif name == 'experience':
            progress = struct.unpack('>f', data.read(4))[0]
            self.xp = {'progress': progress, 'level': read_varint(data.read), 'total': read_varint(data.read)}
            assert not data.read()


def click_payload(revision, place, mode=1):
    # Без предсказанных изменений; сервер возвращает авторитетные слоты.
    return b'\0' + varint(revision) + struct.pack('>hb', place, 0) + varint(mode) + b'\0\0'


def assert_slots(client, expected):
    if getattr(client, 'observe_only', False):
        return
    slots = [None] * 46
    for place, item in expected.items():
        slots[place] = {'item': item, 'count': 1}
    assert client.inventory['window'] == 0 and client.inventory['slots'] == slots, client.inventory
    assert client.inventory['cursor'] is None, client.inventory

def main():
    p = argparse.ArgumentParser(description=__doc__)
    target=p.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar',type=Path)
    target.add_argument('--test-binary',type=Path)
    target.add_argument('--binary',type=Path)
    p.add_argument('--version', type=int, choices=[777], default=777)
    p.add_argument('--observe-only', action='store_true', help='Сохранять снимки без утверждений о поведении')
    p.add_argument('--java', default='/usr/lib/jvm/java-25-openjdk/bin/java')
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int)
    a = p.parse_args()
    expected = {775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
    a.port=a.port or (26226 if a.binary else 26026 if a.test_binary else 26024)
    assert 26000 <= a.port <= 26299, 'isolated probe ports only'
    if a.test_binary: assert a.port==26026
    elif a.binary: assert 26200<=a.port<=26299
    else: assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == expected
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files'] + manifest['baseline_files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / ('protocol.json' if a.version == 777 else 'protocol-26.1.json')).read_text())
    items = a.data / 'items.json' if a.version == 777 else Path(__file__).resolve().parents[2] / 'tools/data/items.json'
    names = {v['id']:v['name'] for v in json.loads(items.read_text())}
    report = {'protocol':a.version,'target':'core' if a.test_binary or a.binary else 'official','jar_sha1':expected if a.jar else None,'snapshots':[], 'commands':[], 'command_events':[], 'checks':[], 'observe_only':a.observe_only, 'checks_kind':'observed-actions-only' if a.observe_only else 'asserted-invariants', 'behavior_failures':[], 'target_sha256':hashlib.sha256((a.jar or a.binary or a.test_binary).read_bytes()).hexdigest()}
    a.output.mkdir(parents=True,exist_ok=True)
    with socket.socket() as check: check.bind(('127.0.0.1',a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-inventory-modes-') as directory:
        root=Path(directory)
        (root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=flat\nlevel-seed=1\nview-distance=2\n')
        log=a.output/'server.log'
        with log.open('w') as output:
            if a.binary:
                (root/'config').mkdir()
                (root/'config/server.properties').write_text((root/'server.properties').read_text())
                (root/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
                (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
                argv=[str(a.binary.resolve())]
            elif a.test_binary:
                (root/'mcsa-gameplay-probe.marker').touch()
                argv=[str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::serve_java_26_3_probe','--nocapture']
            else: argv=[a.java,'-Xmx1G','-jar',str(a.jar.resolve()),'nogui']
            server=subprocess.Popen(argv,cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
            client=None
            def command(value):
                report['commands'].append(value)
                report['command_events'].append({'command':value, 'packet_index':len(client.wire) if client else 0})
                server.stdin.write(value+'\n');server.stdin.flush()
            try:
                deadline=time.monotonic()+120
                ready='Готово (' if a.binary else 'MCSA gameplay probe ready' if a.test_binary else 'Done ('
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(.1)
                command('gamerule minecraft:spawn_mobs false');command('gamerule minecraft:random_tick_speed 0')
                client=connect(a.port,protocol,lambda s,p:Probe(s,p,a.version,names),a.version)
                client.observe_only = a.observe_only
                def check(condition, message):
                    if not condition:
                        report['behavior_failures'].append(str(message))
                        if not a.observe_only:
                            raise AssertionError(message)
                client.pump(10,lambda r:r['name']=='position');client.pump(1)
                assert client.inventory and client.inventory['window']==0, 'initial inventory missing'
                def setup(*commands):
                    for value in commands:
                        command(value)
                        client.pump(.35)
                setup('fill -2 63 -2 2 63 2 stone', 'tp McsaProbe 0.5 64 0.5',
                      'gamemode survival McsaProbe', 'gamerule minecraft:keep_inventory true')
                def snapshot(stage):
                    report['snapshots'].append({'stage':stage, 'packet_index':len(client.wire), 'inventory':json.loads(json.dumps(client.inventory)), 'xp':client.xp})
                def click(place, revision=None, mode=1):
                    client.send('window_click', click_payload(client.inventory['revision'] if revision is None else revision, place, mode))
                    client.pump(.4)
                for item, armor in [('iron_helmet',5), ('iron_chestplate',6), ('iron_leggings',7), ('iron_boots',8), ('elytra',6), ('carved_pumpkin',5), ('shield',45)]:
                    setup('clear McsaProbe', 'item replace entity McsaProbe hotbar.0 with '+item)
                    assert_slots(client, {36:item})
                    click(36)
                    assert_slots(client, {armor:item})
                    snapshot(item+'-equipped')
                    report['checks'].append(item+'-shift-equips')
                    setup('item replace entity McsaProbe hotbar.0 with '+item)
                    click(36)
                    assert_slots(client, {armor:item,9:item})
                    snapshot(item+'-occupied')
                    report['checks'].append(item+'-occupied-moves-main')
                    click(armor)
                    assert_slots(client, {9:item,10:item})
                    snapshot(item+'-unequipped')
                    report['checks'].append(item+'-shift-unequips')
                for source, slot in [('inventory.0',9), ('hotbar.0',36)]:
                    setup('clear McsaProbe', f'item replace entity McsaProbe {source} with carved_pumpkin 8')
                    snapshot('stack8-'+source+'-before')
                    click(slot)
                    snapshot('stack8-'+source+'-after')
                setup('clear McsaProbe', 'item replace entity McsaProbe weapon.offhand with iron_helmet')
                snapshot('offhand-helmet-before')
                click(45)
                snapshot('offhand-helmet-after')
                setup('clear McsaProbe', 'item replace entity McsaProbe inventory.0 with iron_helmet')
                assert_slots(client, {9:'iron_helmet'})
                click(9)
                assert_slots(client, {5:'iron_helmet'})
                report['checks'].append('main-inventory-shift-equips')
                setup('clear McsaProbe')
                for slot in range(9):
                    setup(f'item replace entity McsaProbe hotbar.{slot} with stone')
                setup('item replace entity McsaProbe inventory.0 with iron_helmet')
                click(9)
                assert_slots(client, {5:'iron_helmet', **{36+i:'stone' for i in range(9)}})
                report['checks'].append('full-hotbar-shift-equips-free-armor')
                setup('clear McsaProbe', 'item replace entity McsaProbe hotbar.0 with iron_boots', 'item replace entity McsaProbe hotbar.1 with iron_boots')
                click(36); click(37); click(8)
                assert_slots(client, {9:'iron_boots',10:'iron_boots'})
                # Пустой слот и устаревший state: действие ничего не переносит.
                before = len(client.wire)
                click(5, revision=-1, mode=0)
                check(any(r['direction']=='in' and r['name']=='window_items' for r in client.wire[before:]), 'stale state lacks full resync')
                assert_slots(client, {9:'iron_boots',10:'iron_boots'})
                report['checks'].append('stale-state-full-resync')
                setup('experience set McsaProbe 17 levels', 'experience set McsaProbe 3 points')
                check(client.xp and client.xp['level']==17 and client.xp['progress']>0, client.xp)
                xp = dict(client.xp) if client.xp else None
                snapshot('before-death')
                setup('kill McsaProbe')
                start = len(client.wire)
                client.send('client_command', b'\0')
                client.pump(5, lambda r:r['name']=='respawn'); client.pump(1)
                check(any(r['name']=='respawn' for r in client.wire[start:]), 'no respawn')
                check(any(r['name']=='experience' for r in client.wire[start:]), 'no XP packet after respawn')
                check(client.xp==xp, (xp,client.xp))
                snapshot('after-respawn')
                report['checks'].append('keep-inventory-respawn-restores-xp-packet')
            except BaseException as error: report['error']=str(error);raise
            finally:
                if client: report['packets']=client.wire;client.sock.close()
                if server.poll() is None:
                    command('stop');server.stdin.close()
                    try:server.wait(timeout=30)
                    except subprocess.TimeoutExpired:server.kill();server.wait()
                report['exit_code']=server.returncode
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0
    print(json.dumps({'protocol':a.version,'checks':report['checks']}))


if __name__=='__main__':main()
