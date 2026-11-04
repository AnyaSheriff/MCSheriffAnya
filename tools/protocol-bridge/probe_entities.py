#!/usr/bin/env python3
"""Наши команды и пакеты официального сервера; без чтения JAR/датапаков."""
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
from probe_gameplay import Client, connect
from smoke import read_varint, varint

CASES = [
    ('armor_stand', '{NoGravity:1b,ShowArms:1b,Small:1b,Pose:{Head:[10f,20f,30f]}}'),
    ('cow', '{NoAI:1b,Age:-24000,CustomName:"MCSA cow",CustomNameVisible:1b}'),
    ('slime', '{NoAI:1b,Size:3}'),
    ('magma_cube', '{NoAI:1b,Size:2}'),
    ('falling_block', '{NoGravity:1b,Time:1,BlockState:{Name:"minecraft:oak_log",Properties:{axis:"x"}}}'),
    ('item', '{NoGravity:1b,Item:{id:"minecraft:red_wool",count:5}}'),
    ('arrow', '{NoGravity:1b,Motion:[0.01d,0d,0d]}'),
    ('painting', '{variant:"minecraft:kebab",facing:2b}'),
    ('wolf', '{NoAI:1b,Owner:[I;1,2,3,4],Sitting:1b}'),
    ('enderman', '{NoAI:1b,carriedBlockState:{Name:"minecraft:bamboo_planks"}}'),
    ('block_display', '{block_state:{Name:"minecraft:bamboo_planks"}}'),
]

class Probe(Client):
    def __init__(self, sock, protocol, version):
        super().__init__(sock, protocol)
        self.version = version
        self.observed = []
        self.capture_motion = False
    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + (struct.pack('>dddff', *values) if self.version == 777 else b''))
    def observe_packet(self, name, payload):
        if name in ('spawn_entity', 'entity_metadata', 'respawn') or (self.capture_motion and name in ('rel_entity_move', 'entity_move_look', 'sync_entity_position')):
            self.observed.append({'name': name, 'body': (varint(next(k for k,v in self.incoming.items() if v == name)) + payload).hex()})

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', type=int, choices=[775,776,777], required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26024)
    parser.add_argument('--respawns-only', action='store_true',
                        help='Join до/после смены режима и Respawn; без summon/block probes')
    parser.add_argument('--ordinary-only', action='store_true', help='только776: cow/slime/magma metadata и короткое движение, без Respawn')
    import official_26_2_inputs
    official_26_2_inputs.arguments(parser)
    args = parser.parse_args()
    assert not (args.respawns_only and args.ordinary_only)
    assert not args.ordinary_only or args.version == 776
    if args.version == 776:
        assert args.respawns_only or args.ordinary_only, '776 entity/block probes require separate verification; use --respawns-only'
        protocol, entity_rows, provenance = official_26_2_inputs.load(args)
        expected = provenance['jar_sha1']
    else:
        assert args.data is not None, '--data required for775/777'
        expected = {775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[args.version]
        assert hashlib.sha1(args.jar.read_bytes()).hexdigest() == expected
        sources = json.loads((args.data/'sources.json').read_text())
        for entry in sources['files'] + sources['baseline_files']:
            assert hashlib.sha256((args.data/entry['file']).read_bytes()).hexdigest() == entry['sha256']
        protocol = json.loads((args.data/('protocol.json' if args.version == 777 else 'protocol-26.1.json')).read_text())
        entity_rows = json.loads((args.data/('entities.json' if args.version == 777 else 'entities-26.1.json')).read_text())
        provenance = None
    entities = {v['name']:v['id'] for v in entity_rows}
    with socket.socket() as check: check.bind(('127.0.0.1', args.port))
    args.output.mkdir(parents=True, exist_ok=True)
    report = {'version':args.version, 'jar_sha1':expected, 'entities':[], 'respawns':[], 'commands':[]}
    if provenance is not None: report['provenance'] = provenance
    with tempfile.TemporaryDirectory(prefix='mcsa-entity-probe-') as directory:
        root = Path(directory)
        (root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n')
        log = root/'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(args.jar.resolve()),'nogui'], cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(text):
                report['commands'].append(text)
                server.stdin.write(text+'\n'); server.stdin.flush()
            try:
                deadline = time.monotonic()+120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline, 'startup failed'
                    time.sleep(.1)
                command('gamerule minecraft:spawn_mobs false')
                client = connect(args.port, protocol, lambda s,p:Probe(s,p,args.version), args.version)
                client.capture_motion = args.ordinary_only
                client.pump(10, lambda r:r['name']=='position')
                if not args.ordinary_only: report['login_initial_hex'] = client.login_body.hex()
                if args.respawns_only:
                    command('gamemode survival McsaProbe'); client.pump(.5)
                    client.sock.close(); client = None
                    time.sleep(.5)
                    client = connect(args.port, protocol, lambda s,p:Probe(s,p,args.version), args.version)
                    client.pump(10, lambda r:r['name']=='position')
                    report['login_after_switch_hex'] = client.login_body.hex()
                command('tp McsaProbe 0.5 80 0.5')
                client.pump(5, lambda r:r.get('position',[])[:3]==[.5,80.,.5])
                command('fill -4 79 -4 4 79 4 minecraft:stone')
                client.pump(1)
                cases = CASES
                if args.ordinary_only:
                    cases = [
                        ('cow', '{Tags:["mcsa_776_cow"],NoAI:1b,NoGravity:1b,Age:-24000,CustomName:"MCSA cow",CustomNameVisible:1b}'),
                        ('slime', '{NoAI:1b,NoGravity:1b,Size:3}'),
                        ('magma_cube', '{NoAI:1b,NoGravity:1b,Size:2}'),
                    ]
                for name, nbt in ([] if args.respawns_only else cases):
                    # Release notes26.3: BlockState Name/Properties → id/properties.
                    if args.version == 777 and name in ('falling_block','enderman','block_display'):
                        nbt = nbt.replace('Name:', 'id:').replace('Properties:', 'properties:')
                    start = len(client.observed)
                    x = {'cow':2, 'slime':5, 'magma_cube':8}[name] if args.ordinary_only else 2
                    command('summon '+name+f' {x} 80 2 '+nbt)
                    event = client.pump(5, lambda r:r['name']=='spawn_entity' and r.get('entity_type')==entities[name])
                    client.pump(.5)
                    packets = []
                    for packet in client.observed[start:]:
                        data = io.BytesIO(bytes.fromhex(packet['body'])); read_varint(data.read)
                        if read_varint(data.read)==event['entity']: packets.append(packet)
                    assert any(p['name']=='spawn_entity' for p in packets)
                    if name == 'falling_block':
                        raw = io.BytesIO(bytes.fromhex(next(p['body'] for p in packets if p['name']=='spawn_entity')))
                        read_varint(raw.read); read_varint(raw.read); raw.read(16); read_varint(raw.read); raw.read(24)
                        first = raw.read(1)[0]
                        if first:
                            raw.read(5)
                            if first & 4: read_varint(raw.read)
                        raw.read(3)
                        from generate_tables import states
                        blocks = json.loads((args.data/'blocks.json').read_text()) if args.version == 777 else json.loads((Path(__file__).resolve().parents[1]/'data/blocks.json').read_text())
                        assert read_varint(raw.read) == states(blocks)[('oak_log',(('axis','x'),))], 'falling block did not retain requested oak_log[axis=x]'
                    entry = {'type':name,'id':event['entity'],'packets':packets}
                    if args.ordinary_only: entry['spawn_position'] = event['position']
                    report['entities'].append(entry)
                    if args.ordinary_only:
                        from probe_776_payloads import metadata
                        fields = {}
                        for packet in packets:
                            if packet['name'] == 'entity_metadata':
                                entity, values = metadata(bytes.fromhex(packet['body']))
                                assert entity == event['entity']
                                fields.update(values)
                        assert fields, 'no authored metadata'
                        entry['metadata_fields'] = fields
                        if name == 'cow':
                            assert fields[16] == {'serializer':8, 'value':1}, fields
                            assert fields[2]['value'] == 'MCSA cow', fields
                        else:
                            assert fields[18] == {'serializer':1, 'value':4 if name == 'slime' else 3}, fields
                            assert not any(fields.get(i, {}).get('serializer') == 1 for i in [16,17]), fields
                if args.ordinary_only:
                    from probe_776_payloads import motion
                    cow = next(row['id'] for row in report['entities'] if row['type'] == 'cow')
                    position = next(row['spawn_position'].copy() for row in report['entities'] if row['type'] == 'cow')
                    report['motions'] = []
                    for x, yaw in [(2.25,30), (2.5,60), (2.0,0)]:
                        start = len(client.observed)
                        text = f'data merge entity @e[tag=mcsa_776_cow,limit=1] {{Pos:[{x}d,80d,2d],Rotation:[{yaw}f,15f]}}'
                        command(text); client.pump(1)
                        packets = []
                        for packet in client.observed[start:]:
                            if packet['name'] not in ('rel_entity_move','entity_move_look','sync_entity_position'): continue
                            decoded = motion(packet['name'], bytes.fromhex(packet['body']))
                            if decoded['entity'] != cow: continue
                            packets.append({**packet, 'decoded':decoded})
                            if 'delta' in decoded: position = [a+b/4096 for a,b in zip(position,decoded['delta'])]
                            else: position = decoded['position']
                        entry = {'command':text, 'expected_position':[x,80.,2.], 'position':position.copy(), 'packets':packets}
                        report['motions'].append(entry)
                        assert packets and position == [x,80.,2.], entry
                    assert any(p['name']=='entity_move_look' for row in report['motions'] for p in row['packets']), 'no relative rotation observation'
                for mode, dimension in ([] if args.ordinary_only else [('survival','the_nether'),('creative','the_end'),('spectator','overworld'),('adventure','the_nether')]):
                    command('gamemode '+mode+' McsaProbe'); client.pump(.2)
                    start = len(client.observed)
                    command('execute in minecraft:'+dimension+' run tp McsaProbe 0.5 120 0.5')
                    client.pump(20, lambda r:r['name']=='respawn'); client.pump(1)
                    records = [p for p in client.observed[start:] if p['name']=='respawn']
                    assert len(records)==1
                    report['respawns'].append({'mode':mode,'dimension':dimension,**records[0]})
            except BaseException as error:
                report['error'] = str(error)
                if args.ordinary_only and client:
                    report['failed_observed_packets'] = client.observed[-80:]
                raise
            finally:
                if client: client.sock.close()
                if server.poll() is None:
                    command('stop')
                    try: server.wait(timeout=30)
                    except subprocess.TimeoutExpired: server.kill(); server.wait()
                report['exit_code'] = server.returncode
                (args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
                (args.output/'server.log').write_text(log.read_text())
    print(json.dumps({'entities':len(report['entities']),'respawns':len(report['respawns']),'exit_code':report['exit_code']}))

if __name__ == '__main__': main()
