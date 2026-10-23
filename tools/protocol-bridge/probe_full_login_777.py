#!/usr/bin/env python3
"""Одна обычная сессия777: вход, PvP, смерть, действия, повторный вход и диск.
Оригинал запускается отдельной копией, без stdin; команды идут через RCON.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import time

from probe_gameplay import connect
from probe_catalog_preview_777 import CatalogProbe
from probe_white_concrete_slab import inputs, JAR_SHA1, FINISH_STATUS_777, TARGET
from probe_core_777_stone import validate_stone
from smoke import varint
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'measure'))
from pregen import Rcon


class Probe(CatalogProbe):
    def __init__(self, sock, protocol, item):
        super().__init__(sock, protocol, item)
        self.health = None
        self.entity = None
        self.seen = []

    def send(self, name, payload=b''):
        if name == 'tick_end' and self.grounded_fixture:
            super().send('position', struct.pack('>dddB', *self.position[:3], 1))
            self.grounded_fixture = False
            try:
                return super().send(name, payload)
            finally:
                self.grounded_fixture = True
        return super().send(name, payload)

    def observe_packet(self, name, payload):
        self.seen.append(name)
        if name == 'update_health':
            self.health = struct.unpack('>f', payload[:4])[0]
        elif name == 'login':
            self.entity = struct.unpack('>i', payload[:4])[0]
        elif name == 'respawn':
            self.send('player_loaded')
        super().observe_packet(name, payload)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    target = p.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar', type=Path)
    target.add_argument('--binary', type=Path)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--client-version', type=int, choices=(775,777), default=777)
    p.add_argument('--native-protocol', type=Path)
    p.add_argument('--migrate', action='store_true')
    p.add_argument('--resume', action='store_true')
    p.add_argument('--port', type=int, default=26182)
    a = p.parse_args()
    assert 26100 <= a.port <= 26198
    for port in (a.port, a.port + 1):
        with socket.socket() as s:
            s.bind(('127.0.0.1', port))
    tables = inputs(a.data)
    protocol = tables['protocol.json']
    names = {v['name']: v['id'] for v in tables['items.json']}
    protocol777 = protocol
    names777 = dict(names)
    if a.client_version == 775 or a.migrate:
        assert a.native_protocol and not a.jar
        protocol775 = json.loads(a.native_protocol.read_text())
    if a.client_version == 775:
        protocol = protocol775
        names = {v['name']: v['id'] for v in json.loads((Path(__file__).resolve().parents[1] / 'data/items.json').read_text())}
        names['white_concrete_slab'] = names['quartz_slab']
        view = next(v for v in json.loads((Path(__file__).resolve().parents[2] / 'src/gameplay_catalog/client-775-views.json').read_text())['entries'] if v['name']=='white_concrete_slab')
        tables['states'] = {key: view['states'][i] for i,key in enumerate(['top/true','top/false','bottom/true','bottom/false','double/true','double/false'])}
    item = next(v['id'] for v in tables['entities.json'] if v['name'] == 'item')
    native_entity_names=re.findall(r'\("([^"]+)",',(Path(__file__).resolve().parent/'src/entity_layouts.rs').read_text())
    native_item=native_entity_names.index('item')
    a.output.mkdir(parents=True, exist_ok=a.resume)
    root = a.output.resolve() / 'server'
    root.mkdir(exist_ok=a.resume)
    report = {'checks': [], 'protocol': a.client_version, 'peer_protocol': 777, 'official': bool(a.jar), 'commands': [],
              'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    if a.native_protocol:
        report['native_protocol_sha256']=hashlib.sha256(a.native_protocol.read_bytes()).hexdigest()
    if a.resume:
        report['previous_attempt'] = json.loads((a.output/'result.json').read_text())
        history=a.output/f'attempt-{len(list(a.output.glob("attempt-*")))+1}'
        history.mkdir()
        for source in list(a.output.glob('server-*.log'))+[a.output/'result.json']:
            if source.is_file():shutil.copyfile(source,history/source.name)
        if a.migrate and (root/'world/mcsa-world.json').is_file():
            current=json.loads((root/'world/mcsa-world.json').read_text())
            assert current['gameplay_version']=='26.3'
            shutil.move(root/'world',history/'world')
            shutil.copytree(a.output/'original-775-world',root/'world')
    if a.jar:
        assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == JAR_SHA1
        shutil.copyfile(a.jar, root / 'server.jar')
        argv = ['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', 'server.jar', 'nogui']
    else:
        argv = [str(a.binary.resolve())]
        report['binary_sha256'] = hashlib.sha256(a.binary.read_bytes()).hexdigest()
        (root / 'config').mkdir(exist_ok=a.resume)
        (root / 'config/skins.toml').write_text('mojang=false\nely=false\n')
        (root / 'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
    (root / 'eula.txt').write_text('eula=true\n')
    properties_path = root / ('server.properties' if a.jar else 'config/server.properties')
    properties_path.write_text(
        f'server-ip=127.0.0.1\nserver-port={a.port}\nenable-rcon=true\nrcon.port={a.port+1}\n'
        'rcon.password=local-login-probe\nonline-mode=false\nwhite-list=false\nenforce-whitelist=false\n'
        'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n'
        'level-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\nmax-tick-time=0\n')
    client = peer = server = rcon = None
    logs = []
    cpus = sorted(os.sched_getaffinity(0))[:2]
    os.sched_setaffinity(0, cpus)

    def launch(index):
        nonlocal server, rcon
        log = a.output / f'server-{index}.log'
        assert f'server-port={a.port}\n' in properties_path.read_text()
        output = log.open('w')
        logs.append(output)
        server = subprocess.Popen(argv, cwd=root, stdin=subprocess.DEVNULL if a.jar else subprocess.PIPE,
                                  stdout=output, stderr=subprocess.STDOUT, text=True)
        report.setdefault('pids', []).append(server.pid)
        deadline = time.monotonic() + 120
        while True:
            assert server.poll() is None and time.monotonic() < deadline, log.read_text()[-2500:]
            try:
                if a.jar:
                    if 'Done (' not in log.read_text():
                        time.sleep(.2)
                        continue
                    rcon = Rcon(a.port + 1, 'local-login-probe')
                else:
                    with socket.create_connection(('127.0.0.1', a.port), timeout=.2):
                        pass
                break
            except OSError:
                time.sleep(.2)

    def pump(seconds=.3):
        if client:
            client.pump(seconds)
        if peer:
            peer.pump(.1)

    marker = 0

    def cmd(value):
        nonlocal marker
        report['commands'].append(value)
        if a.jar:
            rcon.run(value)
        else:
            marker += 1
            token = f'MCSA_CORE_DIG_{marker}'
            server.stdin.write(value + '\nsay ' + token + '\n')
            server.stdin.flush()
            if client:
                client.pump(10, lambda row: token in client.markers)
        pump()

    def login(username='McsaProbe', version=None):
        version = version or (777 if username=='McsaPeer' else a.client_version)
        selected = protocol775 if version==775 else protocol777
        class VersionProbe(Probe):
            def confirm_teleport(self, teleport, values):
                if version==775:
                    self.send('teleport_confirm', varint(teleport))
                else:
                    super().confirm_teleport(teleport, values)
        result = connect(a.port, selected, lambda s, pr: VersionProbe(s, pr, native_item if version==775 else item), version=version, username=username,
                         login_uuid=bytes([37 if username == 'McsaProbe' else 38]) * 16)
        result.pump(15, lambda row: row['name'] == 'position')
        if 'window_items' not in result.seen:
            result.pump(30, lambda row: row['name'] == 'window_items')
        result.pump(.5)
        assert 'window_items' in result.seen, 'initial inventory did not arrive'
        assert result.position is not None and result.entity is not None
        return result

    def stop():
        nonlocal server, rcon
        if server and server.poll() is None:
            if a.jar:
                os.kill(server.pid, 15)
            else:
                server.stdin.write('stop\n')
                server.stdin.flush()
            try:
                server.wait(timeout=30)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
                raise AssertionError('server shutdown timeout')
        if server:
            report.setdefault('exit_codes', []).append(server.returncode)
        rcon = None

    try:
        if a.migrate:
            config = root / 'config/mcsa.properties'
            config.write_text(config.read_text().replace('java-version=26.3','java-version=26.1.2'))
            if not (a.resume and (a.output / 'original-775-world').is_dir()):
                launch(0)
                client = login(version=775)
                cmd('forceload add 0 0')
                cmd('setblock 1 65 1 diamond_block')
                cmd('give McsaProbe diamond 7')
                client.sock.close(); client=None
                time.sleep(.7)
                stop()
                shutil.copytree(root / 'world', a.output / 'original-775-world')
            original = {str(p.relative_to(root/'world')): hashlib.sha256(p.read_bytes()).hexdigest() for p in (root/'world').rglob('*') if p.is_file()}
            migration = subprocess.run([str(a.binary.resolve()), '--upgrade-world-26.3', str(root/'world')], cwd=root, capture_output=True, text=True, timeout=120)
            report['migration'] = {'exit': migration.returncode, 'stdout': migration.stdout, 'stderr': migration.stderr, 'source_hashes': original}
            assert migration.returncode == 0, report['migration']
            manifest = json.loads((root/'world/mcsa-world.json').read_text())
            backup = root/'backups/world'/manifest['snapshot_id']/'world'
            assert all(hashlib.sha256((backup/p).read_bytes()).hexdigest()==h for p,h in original.items())
            report['checks'].append('explicit-offline-upgrade-real775-world-verified-entire-backup')
            downgrade = subprocess.run([str(a.binary.resolve())], cwd=root, input='', capture_output=True, text=True, timeout=15)
            report['downgrade']={'exit':downgrade.returncode,'stdout':downgrade.stdout,'stderr':downgrade.stderr}
            assert downgrade.returncode != 0 and '25565' not in downgrade.stdout
            report['checks'].append('downgrade-refused-before-listeners')
            config.write_text(config.read_text().replace('java-version=26.1.2','java-version=26.3'))
        launch(1)
        client = login()
        if a.migrate:
            expected = names['diamond']
            assert any(v and v['item']==expected and v['count']==7 for v in client.inventory.values()), client.inventory
            report['checks'].append('upgraded-world-preserves-player-diamonds7')
            log=a.output/'server-1.log'
            offset=len(log.read_text())
            cmd('setblock 1 65 1 diamond_block')
            assert 'Could not set the block' in log.read_text()[offset:],log.read_text()[offset:]
            report['checks'].append('upgraded-world-preserves-existing-diamond-block')
        report['checks'].append(f'public{a.client_version}-login-configuration-play-chunks')
        for value in ['gamerule doMobSpawning false', 'forceload add -24 -24 23 23',
                      'fill -4 64 -4 5 64 4 stone', 'fill -4 65 -4 5 68 4 air',
                      'tp McsaProbe 0.5 65 0.5', 'gamemode survival McsaProbe', 'clear McsaProbe']:
            cmd(value)
        client.grounded_fixture = True
        peer = login('McsaPeer')
        if a.client_version==775:
            views=json.loads((Path(__file__).resolve().parents[2]/'src/gameplay_catalog/client-775-views.json').read_text())['entries']
            blocks777={v['name']:v for v in tables['blocks.json']}
            for name in ['poplar_log','sulfur_stairs','cinnabar','sulfur_spike','straw_bed']:
                view=next(v for v in views if v['name']==name)
                block=blocks777[name]
                cmd(f'setblock 2 65 0 {name}')
                assert client.target_updates[-1]['state']==view['states'][block['defaultState']-block['minStateId']], (name,client.target_updates[-1])
            report['checks'].append('775-sees-five-new-block-families-as-explicit-substitutes')
            # Номера типов берём из закреплённых собственных таблиц775, без чужого протокольного кода.
            entity_names=re.findall(r'\("([^"]+)",',(Path(__file__).resolve().parent/'src/entity_layouts.rs').read_text())
            for name,base,x in [('poplar_boat','oak_boat',3),('poplar_chest_boat','oak_chest_boat',4),('cushion','armor_stand',-3),('sulfur_cube','slime',-4)]:
                start=len(client.records)
                cmd(f'summon {name} {x} 65 2')
                assert any(r.get('entity_type')==entity_names.index(base) for r in client.records[start:]), (name,client.records[start:])
            report['checks'].append('775-receives-four-new-entity-substitute-spawns-and-metadata')
            cushion=next(r['entity'] for r in reversed(client.records) if r.get('entity_type')==entity_names.index('armor_stand'))
            cmd('tp McsaProbe -2.5 65 1.5')
            client.pump(1.)
            before=len(client.records)
            client.send('use_entity',varint(cushion)+b'\x00\x00\x00')
            client.pump(10,lambda r:r['name']=='set_passengers')
            peer.pump(.2)
            assert any(r['name']=='set_passengers' for r in client.records[before:]), client.records[before:]
            cmd('tp McsaPeer -2.5 65 2.5')
            before=len(peer.events)
            peer.send('attack',varint(cushion))
            peer.pump(10,lambda r:any(e.get('slot',{}).get('item')==names777['white_cushion'] for e in peer.events[before:] if e.get('slot')))
            client.pump(.3)
            assert any(e.get('slot',{}).get('item')==names777['white_cushion'] for e in peer.events[before:] if e.get('slot')),peer.events[before:]
            report['checks'].append('775-sits-on-substitute-777-breaks-real-cushion-and-drops-real-item')
            for name in ['poplar_log','cinnabar']:
                cmd(f'give McsaProbe {name} 2')
                view=next(v for v in views if v['name']==name)
                assert any(v and v['item']==view['item'] and v['count']==2 for v in client.inventory.values()), (name,client.inventory)
            report['checks'].append('775-inventory-shows-new-wood-and-mineral-items')
        cmd('tp McsaPeer 0.5 65 2')
        cmd('gamemode survival McsaPeer')
        peer.grounded_fixture = True
        pump(1.)
        cmd('tp McsaProbe 0.5 65 0.5')
        pump(1.)
        cmd('clear McsaProbe')
        report['pvp_poses'] = [client.position, peer.position]
        assert peer.health == 20, peer.health
        client.send('attack', varint(peer.entity))
        client.pump(.7)
        peer.pump(10., lambda row: row['name'] == 'update_health' and peer.health < 20)
        report['pvp_health'] = peer.health
        if peer.health is not None and 0 < peer.health < 20:
            report['checks'].append('survival-player-attack-reduces-peer-health')
        else:
            report.setdefault('failures', []).append('survival-player-attack-reduces-peer-health')
        peer.sock.close()
        peer = None
        cmd('kill McsaProbe')
        assert client.health == 0 and 'death_combat_event' in client.seen, (client.health, client.seen[-30:])
        client.grounded_fixture = False
        client.send('client_command', b'\0')
        client.pump(10, lambda row: row['name'] == 'respawn')
        client.pump(30, lambda row: row['name'] == 'position')
        pump(1.)
        assert client.health == 20, client.health
        report['checks'].append('death-event-respawn-health20-teleport-ack')
        cmd('tp McsaProbe 0.5 65 0.5')
        client.grounded_fixture = True
        cmd('clear McsaProbe')
        cmd('setblock 2 65 0 air')
        cmd('give McsaProbe white_concrete_slab 2')
        client.send('held_item_slot', struct.pack('>h', 0))
        client.place((2, 64, 0), 1, (.5, 1., .5))
        pump(.5)
        assert client.target_updates[-1]['state'] == tables['states']['bottom/false'], (client.target_updates[-1],tables['states'])
        assert client.inventory.get(36) == {'item': names['white_concrete_slab'], 'count': 1}
        report['placement'] = {'update': client.target_updates[-1], 'held': client.inventory.get(36)}
        report['checks'].append('survival-new-block-placement-consumes-one')
        cmd('clear McsaProbe')
        cmd('setblock 2 65 0 stone')
        cmd('give McsaProbe diamond_pickaxe')
        start = len(client.events)
        client.dig(0)
        pump(2.)
        client.dig(2 if a.client_version==775 else FINISH_STATUS_777)
        pump(.8)
        report['mining'] = {'drops': validate_stone(client.events[start:], client.inventory, names['diamond_pickaxe'], names['cobblestone']),
                            'tool': client.inventory.get(36)}
        report['checks'].append('survival-mining-drop-and-tool-damage1')
        cmd('clear McsaProbe')
        cmd('give McsaProbe white_concrete_slab 3')
        # Обычный щелчок: предметы проходят через курсор и возвращаются в инвентарь.
        for place in (36, 37):
            client.send('window_click', b'\0' + varint(-1) + struct.pack('>hb', place, 0) + b'\0\0\0')
            pump(.3)
        assert client.inventory.get(36) is None
        assert client.inventory.get(37) == {'item': names['white_concrete_slab'], 'count': 3}
        report['checks'].append('inventory-authoritative-click-moves-stack')
        cmd('setblock 2 65 0 white_concrete_slab[type=top,waterlogged=false]')
        client.sock.shutdown(socket.SHUT_RDWR)
        client.sock.close()
        client = None
        time.sleep(1.)
        client = login()
        assert client.inventory.get(37) == {'item': names['white_concrete_slab'], 'count': 3}, client.inventory
        report['reconnected_inventory'] = client.inventory
        report['checks'].append('logout-login-preserves-new-items')
        if a.client_version==775:
            client.sock.close(); client=None
            time.sleep(.7)
            client=login(version=777)
            assert client.inventory.get(37)=={'item': names777['white_concrete_slab'], 'count':3}, client.inventory
            report['checks'].append('777-relogin-proves775-clicks-kept-real-new-item')
            client.sock.close();client=None
            time.sleep(.7)
            client=login()
        if not a.jar:
            client.sock.close()
            client = None
            time.sleep(.5)
            stop()
            manifest = json.loads((root / 'world/mcsa-world.json').read_text())
            assert manifest['gameplay_version'] == '26.3'
            assert list((root / 'world').rglob('*.mcs'))
            assert list((root / 'world/playerdata').glob('*.mcrw'))
            report['manifest'] = manifest
            report['saved_files'] = [{'path': str(path.relative_to(root)), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                                      'bytes': path.stat().st_size} for path in sorted((root / 'world').rglob('*'))
                                     if path.is_file() and path.suffix in ('.mcs', '.mcrw')]
            launch(2)
            client = login()
            assert client.inventory.get(37) == {'item': names['white_concrete_slab'], 'count': 3}, client.inventory
            cmd('forceload add 0 0')
            start = len(client.target_updates)
            log = a.output / 'server-2.log'
            offset = len(log.read_text())
            cmd('setblock 2 65 0 white_concrete_slab[type=top,waterlogged=false]')
            assert 'Could not set the block' in log.read_text()[offset:], log.read_text()[offset:]
            assert len(client.target_updates) == start, 'saved block was rewritten'
            report['checks'].append('restart-restores-profile-player-and-native-regions')
        report['status'] = 'partial' if report.get('failures') else 'pass'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = repr(error)
        raise
    finally:
        for connection in (client, peer):
            if connection:
                report.setdefault('packets', []).extend(connection.seen)
                connection.sock.close()
        stop()
        for output in logs:
            output.close()
        (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'checks': len(report['checks']), 'status': report['status']}))


if __name__ == '__main__':
    main()
