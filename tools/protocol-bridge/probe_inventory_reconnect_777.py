#!/usr/bin/env python3
"""Ограниченная A06: свободный рюкзак, курсор, сетки2×2/3×3 и сундук; FIN/RST.

Без полного рюкзака, подбора выпавшей стопки и Bedrock; один обычный рестарт.
Сторонний код/JAR не читается: сервер используется только как чёрный ящик.
"""
import argparse
from collections import Counter
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

from probe_gameplay import Client, connect
from probe_gameplay_inventory_777 import Probe as BaseProbe
from smoke import read_varint, varint

JAR_SHA1 = '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
CASES = [(kind, mode) for kind in ('personal', 'crafting', 'chest', 'workbench')
         for mode in ('close-fin', 'open-rst')]
STAGES = [kind+'-'+mode+'-'+phase for kind, mode in CASES for phase in ('before', 'after')] + ['workbench-open-rst-after-restart']
LIMITS = ['Free inventory space only; simple authored ground stacks decoded from metadata',
          'One normal final restart only; no crash restart, full inventory, other workstations, Bedrock, component or timing claim',
          'Graceful means explicit close_window then TCP FIN; abrupt means open-window TCP RST',
          'Second spectator accounts ground stacks; frozen world prevents pickup/physics during snapshots']


class Probe(BaseProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.drops_seen = set()
        self.ground_stacks = {}
        self.ground_events = []

    def send(self, name, payload=b''):
        # BaseProbe records selected packets; this probe records every play packet.
        self.wire.append({'direction':'out', 'name':name, 'payload':payload.hex()})
        Client.send(self, name, payload)

    def observe_packet(self, name, payload):
        start = len(self.wire)
        super().observe_packet(name, payload)
        del self.wire[start:]
        self.wire.append({'direction':'in', 'name':name, 'payload':payload.hex()})
        if name == 'spawn_entity':
            data = io.BytesIO(payload)
            entity = read_varint(data.read)
            assert len(data.read(16)) == 16
            if read_varint(data.read) == 72:
                self.drops_seen.add(entity)
                assert entity not in self.ground_stacks, 'duplicate item entity spawn'
                self.ground_stacks[entity] = None
                self.ground_events.append({'event':'spawn','entity':entity})
        elif name == 'entity_metadata':
            data = io.BytesIO(payload)
            entity = read_varint(data.read)
            if entity in self.ground_stacks:
                seen = set()
                while True:
                    raw = data.read(1)
                    assert raw, 'truncated item metadata'
                    index = raw[0]
                    if index == 255:
                        break
                    assert index not in seen, 'duplicate item metadata index'
                    seen.add(index)
                    kind = read_varint(data.read)
                    # Закреплённый protocol777 JSON: serializer7=item_stack/Slot.
                    # Raw official41889607814: 05 08 07 03 01 00 00 ff = stone3.
                    if index == 8:
                        assert kind == 7, 'item stack must use serializer7'
                        value = self.slot(data)
                        assert value and set(value)=={'item','count'}, 'only simple authored ground stacks'
                        assert value['item'] in ('stone','oak_planks','dirt') and value['count'] > 0
                        self.ground_stacks[entity] = value
                        self.ground_events.append({'event':'stack','entity':entity,'stack':copy.deepcopy(value)})
                    elif index == 0 and kind == 0:
                        assert len(data.read(1)) == 1, 'truncated item flags'
                    else:
                        raise AssertionError(('unexpected item metadata',index,kind))
                assert not data.read(), 'trailing item metadata'
        elif name == 'entity_destroy':
            data = io.BytesIO(payload)
            for _ in range(read_varint(data.read)):
                entity = read_varint(data.read)
                if entity in self.ground_stacks:
                    value = self.ground_stacks.pop(entity)
                    assert value is not None, 'item removed before its count was observed'
                    self.ground_events.append({'event':'destroy','entity':entity,'stack':value})
            assert not data.read()


def replay(packets, names):
    client = Probe.__new__(Probe)
    client.version = 777
    client.names = names
    client.inventory = None
    client.windows = []
    client.observed = []
    client.wire = []
    client.xp = None
    client.ground_items = set()
    client.drops_seen = set()
    client.ground_stacks = {}
    client.ground_events = []
    for packet in packets:
        if packet['direction'] == 'in':
            client.observe_packet(packet['name'], bytes.fromhex(packet['payload']))
    return client


def click_payload(window, revision, slot):
    # Реальный обычный левый щелчок, без локального предсказания.
    return varint(window)+varint(revision)+struct.pack('>hb', slot, 0)+b'\0\0\0'


def use_payload(x, y, z, sequence):
    packed = ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 0xfff)
    return b'\0'+struct.pack('>Q', packed)+b'\1'+struct.pack('>fff', .5, .5, .5)+b'\0\0'+varint(sequence)


def ground_snapshot(client):
    assert all(value is not None for value in client.ground_stacks.values()), 'missing ground stack metadata'
    return sorted((copy.deepcopy(value) for value in client.ground_stacks.values()),
                  key=lambda value:(value['item'],value['count']))


def totals(inv, ground=()):
    assert inv and isinstance(inv['slots'], list)
    # Личный инвентарь и верстак имеют46слотов; слот0 — только preview.
    values = inv['slots'][1:] if len(inv['slots']) == 46 else inv['slots']
    result = Counter()
    for value in values + [inv['cursor']] + list(ground):
        if value:
            assert set(value) == {'item', 'count'} and type(value['count']) is int and value['count'] > 0
            result[value['item']] += value['count']
    return dict(sorted(result.items()))


def validate(report, target):
    if (report.get('protocol') != 777 or report.get('target') != target
            or report.get('status') != 'PASS' or report.get('exit_code') != 0
            or report.get('behavior_failures') or 'error' in report):
        raise ValueError('failed or incomplete report')
    if report.get('server_exits') != [0,0]:
        raise ValueError('missing two clean server exits')
    if set(report.get('server_log_sha256', {})) != {'server.log','server-1.log'} or any(len(value)!=64 for value in report['server_log_sha256'].values()):
        raise ValueError('missing two server log hashes')
    if target == 'official' and report.get('jar_sha1') != JAR_SHA1:
        raise ValueError('wrong official executable')
    rows = report.get('snapshots', [])
    if [row.get('stage') for row in rows] != STAGES:
        raise ValueError('missing, duplicate or reordered stages')
    sessions = report.get('sessions', [])
    expected_sessions = STAGES
    if [s.get('name') for s in sessions] != expected_sessions:
        raise ValueError('missing or reordered sessions')
    names = {int(k):v for k,v in report['item_names'].items()}
    observer = report.get('observer', [])
    if not any(p['direction']=='in' and p['name']=='login' for p in observer):
        raise ValueError('missing live observer')
    witnessed = replay(observer, names)
    if report.get('ground_events') != witnessed.ground_events:
        raise ValueError('ground event history not derived from raw observer')
    restart_observer = report.get('restart_observer', [])
    if not any(p['direction']=='in' and p['name']=='login' for p in restart_observer):
        raise ValueError('missing restarted live observer')
    if report.get('restart_ground_events') != replay(restart_observer, names).ground_events:
        raise ValueError('restart ground history not derived from raw observer')
    observer_indices = [0,0]
    original_observer = observer
    for row_number, (row, session) in enumerate(zip(rows, sessions)):
        epoch = 1 if row_number==16 else 0
        if row.get('observer_epoch') != epoch:
            raise ValueError('wrong observer epoch across restart')
        observer = restart_observer if epoch else original_observer
        index = row.get('packet_index')
        packets = session.get('packets', [])
        if not any(p['direction']=='in' and p['name']=='login' for p in packets):
            raise ValueError('no real reconnect login')
        initial = next((p for p in packets if p['direction']=='in' and p['name']=='window_items'), None)
        restored = replay([initial], names).inventory if initial else None
        if not restored or restored['window'] != 0 or len(restored['slots']) != 46 or restored['cursor'] is not None:
            raise ValueError('missing initial personal inventory with empty cursor')
        if type(index) is not int or not 0 < index <= len(packets):
            raise ValueError('invalid snapshot packet range')
        measured = replay(packets[:index], names)
        if measured.inventory != row.get('inventory'):
            raise ValueError('snapshot not derived from incoming packets')
        inv = measured.inventory
        kind = row['stage'].split('-')[0]
        expected_size = 63 if kind=='chest' else 46
        if not inv or len(inv['slots']) != expected_size or (inv['window']==0) != (kind in ('personal','crafting')):
            raise ValueError('wrong window scope')
        if kind in ('chest','workbench'):
            uses = [p for p in packets[:index] if p['direction']=='out' and p['name']=='block_place']
            if len(uses) != 1 or not bytes.fromhex(uses[0]['payload']).startswith(use_payload(2,64,0,1)[:-1]):
                raise ValueError('missing real container block use')
            if not any(p['direction']=='in' and p['name']=='open_window' for p in packets[:index]):
                raise ValueError('missing container open response')
        if not row['stage'].endswith('-before') and inv['cursor'] is not None:
            raise ValueError('cursor persisted across reconnect')
        boundary = row.get('observer_packet_index')
        if type(boundary) is not int or not observer_indices[epoch] <= boundary <= len(observer):
            raise ValueError('invalid observer snapshot packet range')
        observer_indices[epoch] = boundary
        ground = ground_snapshot(replay(observer[:boundary], names))
        if row.get('ground') != ground:
            raise ValueError('ground snapshot not derived from observer packets')
        if ('-before' in row['stage'] or '-close-fin-' in row['stage']) and ground:
            raise ValueError('unexpected ground stack before disconnect or after explicit close')
        if row.get('totals') != totals(inv, ground):
            raise ValueError('wrong item accounting')
        outgoing = [p['name'] for p in packets if p['direction']=='out']
        if row['stage'].endswith('-before'):
            if 'window_click' not in outgoing:
                raise ValueError('no real pickup click')
            mode = 'close-fin' if '-close-fin-' in row['stage'] else 'open-rst'
            if session.get('disconnect') != mode:
                raise ValueError('missing disconnect receipt')
            if ('close_window' in outgoing) != (mode == 'close-fin'):
                raise ValueError('wrong graceful/abrupt coverage')
            clicks = []
            for packet in packets:
                if packet['direction']=='out' and packet['name']=='window_click':
                    data = io.BytesIO(bytes.fromhex(packet['payload']))
                    read_varint(data.read); read_varint(data.read)
                    place, button = struct.unpack('>hb', data.read(3))
                    if button != 0 or data.read() != b'\0\0\0':
                        raise ValueError('not a real unpredicted normal pickup click')
                    clicks.append(place)
            kind = row['stage'].split('-')[0]
            if clicks != ([0] if kind=='chest' else [11,1,10] if kind=='workbench' else [10,1,9] if kind=='crafting' else [9]):
                raise ValueError('missing, repeated or wrong action sequence')
        else:
            if not any(p['direction']=='in' and p['name']=='window_items' for p in packets):
                raise ValueError('missing restored authoritative inventory')
    observer = original_observer
    for before, after in zip(rows[:16:2], rows[1:16:2]):
        kind = before['stage'].split('-')[0]
        if '-close-fin-' in before['stage']:
            prefix = replay(observer[:before['observer_packet_index']], names).ground_events
            events = replay(observer[:after['observer_packet_index']], names).ground_events[len(prefix):]
            if any(event['event']=='spawn' for event in events):
                raise ValueError('explicit close transiently dropped an item')
        expected = {'stone':3} if kind=='personal' else {'oak_planks':2, 'stone':3} if kind in ('crafting','workbench') else {'dirt':5, 'stone':7}
        if before['totals'] != expected or after['totals'] != expected:
            raise ValueError('lost or duplicated items')
        if before['inventory']['cursor'] != {'item':'stone', 'count':7 if kind=='chest' else 3}:
            raise ValueError('cursor was not authoritatively populated')
        if kind in ('crafting','workbench') and before['inventory']['slots'][1] != {'item':'oak_planks', 'count':2}:
            raise ValueError('crafting inputs were not really placed')
        if kind=='chest':
            if before['inventory']['slots'][1] != {'item':'dirt', 'count':5} or after['inventory']['slots'][:27] != before['inventory']['slots'][:27]:
                raise ValueError('chest residual changed')
    before_restart, after_restart = rows[-2:]
    if before_restart['ground'] != [{'item':'oak_planks','count':2},{'item':'stone','count':3}]:
        raise ValueError('final restart fixture must contain observed ground planks2 and stone3')
    if after_restart['totals'] != {'oak_planks':2,'stone':3}:
        raise ValueError('restart lost or duplicated items')
    for field in ('ground','totals'):
        if before_restart[field] != after_restart[field]:
            raise ValueError('restart changed '+field)
    for field in ('slots','cursor'):
        if before_restart['inventory'][field] != after_restart['inventory'][field]:
            raise ValueError('restart changed inventory '+field)
    return rows


def compare(official, ours):
    left, right = validate(official, 'official'), validate(ours, 'core')
    differences = []
    for a, b in zip(left, right):
        if a['ground'] != b['ground']:
            differences.append({'stage':a['stage'], 'field':'ground'})
        for field in ('slots', 'cursor'):
            if a['inventory'][field] != b['inventory'][field]:
                differences.append({'stage':a['stage'], 'field':field})
    return differences


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar', type=Path)
    target.add_argument('--binary', type=Path)
    parser.add_argument('--java', default='/usr/lib/jvm/java-25-openjdk/bin/java')
    parser.add_argument('--data', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--expected', type=Path)
    parser.add_argument('--port', type=int, required=True)
    args = parser.parse_args()
    assert 26000 <= args.port <= 26299
    assert args.jar or args.expected, 'core requires a fresh official report'
    expected = json.loads(args.expected.read_text()) if args.expected else None
    if expected is not None:
        validate(expected, 'official')
    executable = (args.jar or args.binary).resolve()
    if args.jar:
        assert hashlib.sha1(executable.read_bytes()).hexdigest() == JAR_SHA1
    manifest = json.loads((args.data/'sources.json').read_text())
    for entry in manifest['files'] + manifest.get('baseline_files', []):
        assert hashlib.sha256((args.data/entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((args.data/'protocol.json').read_text())
    names = {item['id']:item['name'] for item in json.loads((args.data/'items.json').read_text())}
    report = {'protocol':777, 'target':'official' if args.jar else 'core',
              'target_sha256':hashlib.sha256(executable.read_bytes()).hexdigest(),
              'jar_sha1':JAR_SHA1 if args.jar else None, 'status':'INCOMPLETE',
              'limits':LIMITS, 'item_names':names, 'sessions':[], 'snapshots':[],
              'commands':[], 'observer':[], 'ground_events':[], 'restart_observer':[],
              'restart_ground_events':[], 'server_exits':[], 'server_log_sha256':{}, 'behavior_failures':[]}
    if args.expected:
        report['expected_sha256'] = hashlib.sha256(args.expected.read_bytes()).hexdigest()
    args.output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as check:
        check.bind(('127.0.0.1', args.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-reconnect-777-') as directory:
        root = Path(directory)
        properties = f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\n'
        (root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(properties)
        if args.binary:
            (root/'config').mkdir()
            (root/'config/server.properties').write_text(properties)
            (root/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
            (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
        argv = [args.java, '-Xmx1G', '-jar', str(executable), 'nogui'] if args.jar else [str(executable)]
        actor = observer = None
        restart_output = None
        epoch = 0
        stopped = set()
        log = args.output/'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            def stop_server():
                if server.pid in stopped:
                    return
                if server.poll() is None:
                    try:
                        server.stdin.write('stop\n'); server.stdin.flush(); server.stdin.close()
                        server.wait(timeout=30)
                    except (OSError, subprocess.TimeoutExpired):
                        server.kill(); server.wait()
                report['server_exits'].append(server.returncode)
                stopped.add(server.pid)
            def ready(path):
                deadline = time.monotonic()+120
                while ('Done (' if args.jar else 'Готово (') not in path.read_text():
                    assert server.poll() is None and time.monotonic()<deadline, 'server readiness timeout'
                    time.sleep(.1)
            def pump(seconds=.4):
                deadline = time.monotonic()+seconds
                while time.monotonic() < deadline:
                    if not actor and not observer:
                        time.sleep(.05)
                    for client in (actor, observer):
                        if client:
                            client.pump(.08)
            def command(value):
                report['commands'].append(value)
                server.stdin.write(value+'\n'); server.stdin.flush()
                pump()
            def join(label):
                client = connect(args.port, protocol, lambda s,p:Probe(s,p,777,names))
                report['sessions'].append({'name':label, 'packets':client.wire})
                try:
                    client.pump(10, lambda packet:packet['name']=='position')
                    client.pump(.5)
                    assert client.inventory and client.inventory['window']==0 and client.inventory['cursor'] is None
                except BaseException:
                    client.sock.close()
                    raise
                return client
            def click(slot):
                inv = actor.inventory
                actor.send('window_click', click_payload(inv['window'], inv['revision'], slot))
                pump(.6)
            def snap(stage):
                pump(.4)
                state = copy.deepcopy(actor.inventory)
                ground = ground_snapshot(observer)
                report['snapshots'].append({'stage':stage, 'packet_index':len(actor.wire),
                                            'observer_epoch':epoch, 'observer_packet_index':len(observer.wire),
                                            'inventory':state, 'ground':ground, 'totals':totals(state,ground)})
            def open_container(sequence, size):
                actor.send('block_place', use_payload(2,64,0,sequence))
                pump(.7)
                assert actor.inventory and actor.inventory['window'] != 0 and len(actor.inventory['slots'])==size
            try:
                ready(log)
                command('gamerule minecraft:spawn_mobs false')
                command('gamerule minecraft:random_tick_speed 0')
                observer = connect(args.port, protocol, lambda s,p:Probe(s,p,777,names), username='McsaWitness', login_uuid=bytes([38])*16)
                report['observer'] = observer.wire
                report['ground_events'] = observer.ground_events
                observer.pump(10, lambda packet:packet['name']=='position')
                command('gamemode spectator McsaWitness')
                command('tp McsaWitness 8.5 65 0.5')
                for number, (kind, mode) in enumerate(CASES):
                    label = kind+'-'+mode
                    actor = join(label+'-before')
                    command('tp McsaProbe 0.5 64 0.5')
                    command('gamemode survival McsaProbe')
                    # Чанки уже загружены реальным игроком, до входа fill не работал.
                    command('fill -2 63 -2 10 63 2 stone')
                    command('tick unfreeze')
                    command('kill @e[type=item]')
                    pump(.5)
                    assert not observer.ground_stacks, 'previous case ground cleanup incomplete'
                    command('tick freeze')
                    command('clear McsaProbe')
                    if kind=='chest':
                        command('setblock 2 64 0 chest')
                        command('item replace block 2 64 0 container.0 with stone 7')
                        command('item replace block 2 64 0 container.1 with dirt 5')
                        open_container(number*2+1,63)
                        click(0)
                    else:
                        command('item replace entity McsaProbe inventory.0 with stone 3')
                        if kind in ('crafting','workbench'):
                            command('item replace entity McsaProbe inventory.1 with oak_planks 2')
                            if kind=='workbench':
                                # Удаляем остаток предыдущей фикстуры до замены блока.
                                if mode=='close-fin':
                                    command('item replace block 2 64 0 container.1 with air')
                                command('setblock 2 64 0 crafting_table')
                                open_container(number*2+1,46)
                            click(11 if kind=='workbench' else 10); click(1)
                        click(10 if kind=='workbench' else 9)
                    snap(label+'-before')
                    session = report['sessions'][-1]
                    session['disconnect'] = mode
                    if mode=='close-fin':
                        actor.send('close_window', varint(actor.inventory['window']))
                        pump(.5)
                        actor.sock.shutdown(socket.SHUT_RDWR)
                    else:
                        actor.sock.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack('ii',1,0))
                    actor.sock.close(); actor = None
                    pump(1.2)
                    actor = join(label+'-after')
                    pump(.5)
                    if kind in ('chest','workbench'):
                        open_container(number*2+2,63 if kind=='chest' else 46)
                    snap(label+'-after')
                    actor.sock.shutdown(socket.SHUT_RDWR); actor.sock.close(); actor = None
                    pump(.8)
                # Последняя ground-фикстура не очищается: обычный stop/start того же мира.
                observer.sock.close(); observer = None
                stop_server()
                assert report['server_exits']==[0], 'first server did not stop cleanly'
                epoch = 1
                restart_log = args.output/'server-1.log'
                restart_output = restart_log.open('w')
                server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=restart_output, stderr=subprocess.STDOUT, text=True)
                ready(restart_log)
                command('tick freeze')
                observer = connect(args.port, protocol, lambda s,p:Probe(s,p,777,names), username='McsaWitness', login_uuid=bytes([38])*16)
                report['restart_observer'] = observer.wire
                report['restart_ground_events'] = observer.ground_events
                observer.pump(10, lambda packet:packet['name']=='position')
                command('gamemode spectator McsaWitness')
                command('tp McsaWitness 8.5 65 0.5')
                actor = join(STAGES[-1])
                pump(.5)
                open_container(17,46)
                snap(STAGES[-1])
                report['status'] = 'PASS'
            except BaseException as error:
                report['error'] = repr(error)
                report['status'] = 'ERROR'
                raise
            finally:
                for client in (actor, observer):
                    if client:
                        client.sock.close()
                stop_server()
                if restart_output:
                    restart_output.close()
                output.flush()
                for filename in ('server.log','server-1.log'):
                    path = args.output/filename
                    if path.exists():
                        report['server_log_sha256'][filename] = hashlib.sha256(path.read_bytes()).hexdigest()
                report['exit_code'] = server.returncode
                if report['server_exits'] != [0,0]:
                    report['status'] = 'ERROR'
                if report['status']=='PASS':
                    try:
                        validate(report, report['target'])
                        if expected is not None:
                            report['behavior_failures'] = compare(expected, report)
                            if report['behavior_failures']:
                                report['status'] = 'FAIL'
                    except (ValueError, AssertionError, KeyError) as error:
                        report['error'] = repr(error)
                        report['status'] = 'ERROR'
                (args.output/'result.json').write_text(json.dumps(report, indent=2)+'\n')
    if report['status'] != 'PASS':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
