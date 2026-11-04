#!/usr/bin/env python3
"""Ограниченная A06: свободный рюкзак, курсор, сетка2×2 и сундук; FIN/RST.

Без полного рюкзака, восстановления выпавшей стопки, рестарта и Bedrock.
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
CASES = [(kind, mode) for kind in ('personal', 'crafting', 'chest')
         for mode in ('close-fin', 'open-rst')]
STAGES = [kind+'-'+mode+'-'+phase for kind, mode in CASES for phase in ('before', 'after')]
LIMITS = ['Free inventory space only; no ground stack recovery/count decoder',
          'No restart, full inventory, workstation, Bedrock, component or timing claim',
          'Graceful means explicit close_window then TCP FIN; abrupt means open-window TCP RST',
          'Ground items are forbidden and watched by a second spectator; any item spawn fails']


class Probe(BaseProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.drops_seen = set()

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


def totals(inv):
    assert inv and isinstance(inv['slots'], list)
    # Окно0: слот0 — только preview, не принадлежащая игроку стопка.
    values = inv['slots'][1:] if inv['window'] == 0 else inv['slots']
    result = Counter()
    for value in values + [inv['cursor']]:
        if value:
            assert set(value) == {'item', 'count'} and type(value['count']) is int and value['count'] > 0
            result[value['item']] += value['count']
    return dict(sorted(result.items()))


def validate(report, target):
    if (report.get('protocol') != 777 or report.get('target') != target
            or report.get('status') != 'PASS' or report.get('exit_code') != 0
            or report.get('behavior_failures') or 'error' in report):
        raise ValueError('failed or incomplete report')
    if target == 'official' and report.get('jar_sha1') != JAR_SHA1:
        raise ValueError('wrong official executable')
    rows = report.get('snapshots', [])
    if [row.get('stage') for row in rows] != STAGES:
        raise ValueError('missing, duplicate or reordered stages')
    sessions = report.get('sessions', [])
    expected_sessions = [kind+'-'+mode+'-'+phase for kind, mode in CASES for phase in ('before', 'after')]
    if [s.get('name') for s in sessions] != expected_sessions:
        raise ValueError('missing or reordered sessions')
    names = {int(k):v for k,v in report['item_names'].items()}
    observer = report.get('observer', [])
    if not any(p['direction']=='in' and p['name']=='login' for p in observer):
        raise ValueError('missing live observer')
    if replay(observer, names).drops_seen:
        raise ValueError('item dropped while actor disconnected')
    for row, session in zip(rows, sessions):
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
        if measured.inventory != row.get('inventory') or measured.drops_seen:
            raise ValueError('snapshot not derived from incoming packets')
        inv = measured.inventory
        expected_size = 63 if row['stage'].startswith('chest-') else 46
        if not inv or len(inv['slots']) != expected_size or (inv['window']==0) != (expected_size==46):
            raise ValueError('wrong window scope')
        if row['stage'].endswith('-after') and inv['cursor'] is not None:
            raise ValueError('cursor persisted across reconnect')
        if row.get('totals') != totals(inv):
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
            if clicks != ([0] if kind=='chest' else [10,1,9] if kind=='crafting' else [9]):
                raise ValueError('missing, repeated or wrong action sequence')
        else:
            if not any(p['direction']=='in' and p['name']=='window_items' for p in packets):
                raise ValueError('missing restored authoritative inventory')
    for before, after in zip(rows[::2], rows[1::2]):
        kind = before['stage'].split('-')[0]
        expected = {'stone':3} if kind=='personal' else {'oak_planks':2, 'stone':3} if kind=='crafting' else {'dirt':5, 'stone':7}
        if before['totals'] != expected or after['totals'] != expected:
            raise ValueError('lost or duplicated items')
        if before['inventory']['cursor'] != {'item':'stone', 'count':7 if kind=='chest' else 3}:
            raise ValueError('cursor was not authoritatively populated')
        if kind=='crafting' and before['inventory']['slots'][1] != {'item':'oak_planks', 'count':2}:
            raise ValueError('crafting inputs were not really placed')
        if kind=='chest':
            if before['inventory']['slots'][1] != {'item':'dirt', 'count':5} or after['inventory']['slots'][:27] != before['inventory']['slots'][:27]:
                raise ValueError('chest residual changed')
    return rows


def compare(official, ours):
    left, right = validate(official, 'official'), validate(ours, 'core')
    differences = []
    for a, b in zip(left, right):
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
              'commands':[], 'observer':[], 'behavior_failures':[]}
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
        log = args.output/'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            def pump(seconds=.4):
                deadline = time.monotonic()+seconds
                while time.monotonic() < deadline:
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
                assert not actor.drops_seen and not observer.drops_seen, 'unexpected ground item'
                state = copy.deepcopy(actor.inventory)
                report['snapshots'].append({'stage':stage, 'packet_index':len(actor.wire), 'inventory':state, 'totals':totals(state)})
            def open_chest(sequence):
                actor.send('block_place', use_payload(2,64,0,sequence))
                pump(.7)
                assert actor.inventory and actor.inventory['window'] != 0 and len(actor.inventory['slots'])==63
            try:
                deadline = time.monotonic()+120
                while ('Done (' if args.jar else 'Готово (') not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline, 'server readiness timeout'
                    time.sleep(.1)
                command('gamerule minecraft:spawn_mobs false')
                command('gamerule minecraft:random_tick_speed 0')
                command('fill -2 63 -2 10 63 2 stone')
                observer = connect(args.port, protocol, lambda s,p:Probe(s,p,777,names), username='McsaWitness', login_uuid=bytes([38])*16)
                report['observer'] = observer.wire
                observer.pump(10, lambda packet:packet['name']=='position')
                command('gamemode spectator McsaWitness')
                command('tp McsaWitness 8.5 65 0.5')
                for number, (kind, mode) in enumerate(CASES):
                    label = kind+'-'+mode
                    actor = join(label+'-before')
                    command('tp McsaProbe 0.5 64 0.5')
                    command('gamemode survival McsaProbe')
                    command('clear McsaProbe')
                    if kind=='chest':
                        command('setblock 2 64 0 chest')
                        command('item replace block 2 64 0 container.0 with stone 7')
                        command('item replace block 2 64 0 container.1 with dirt 5')
                        open_chest(number*2+1)
                        click(0)
                    else:
                        command('item replace entity McsaProbe inventory.0 with stone 3')
                        if kind=='crafting':
                            command('item replace entity McsaProbe inventory.1 with oak_planks 2')
                            click(10); click(1)
                        click(9)
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
                    if kind=='chest':
                        open_chest(number*2+2)
                    snap(label+'-after')
                    actor.sock.shutdown(socket.SHUT_RDWR); actor.sock.close(); actor = None
                    pump(.8)
                report['status'] = 'PASS'
            except BaseException as error:
                report['error'] = repr(error)
                report['status'] = 'ERROR'
                raise
            finally:
                for client in (actor, observer):
                    if client:
                        client.sock.close()
                if server.poll() is None:
                    try:
                        server.stdin.write('stop\n'); server.stdin.flush(); server.stdin.close()
                        server.wait(timeout=30)
                    except (OSError, subprocess.TimeoutExpired):
                        server.kill(); server.wait()
                report['exit_code'] = server.returncode
                if server.returncode != 0:
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
