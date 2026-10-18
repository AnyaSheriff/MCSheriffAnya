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
    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + (struct.pack('>dddff', *values) if self.version == 777 else b''))
    def observe_packet(self, name, payload):
        if name in ('spawn_entity', 'entity_metadata', 'respawn'):
            self.observed.append({'name': name, 'body': (varint(next(k for k,v in self.incoming.items() if v == name)) + payload).hex()})

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', type=int, choices=[775,777], required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26024)
    args = parser.parse_args()
    expected = {775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[args.version]
    assert hashlib.sha1(args.jar.read_bytes()).hexdigest() == expected
    sources = json.loads((args.data/'sources.json').read_text())
    for entry in sources['files'] + sources['baseline_files']:
        assert hashlib.sha256((args.data/entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((args.data/('protocol.json' if args.version == 777 else 'protocol-26.1.json')).read_text())
    entities = {v['name']:v['id'] for v in json.loads((args.data/('entities.json' if args.version == 777 else 'entities-26.1.json')).read_text())}
    with socket.socket() as check: check.bind(('127.0.0.1', args.port))
    args.output.mkdir(parents=True, exist_ok=True)
    report = {'version':args.version, 'jar_sha1':expected, 'entities':[], 'respawns':[]}
    with tempfile.TemporaryDirectory(prefix='mcsa-entity-probe-') as directory:
        root = Path(directory)
        (root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n')
        log = root/'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(args.jar.resolve()),'nogui'], cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(text):
                server.stdin.write(text+'\n'); server.stdin.flush()
            try:
                deadline = time.monotonic()+120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline, 'startup failed'
                    time.sleep(.1)
                client = connect(args.port, protocol, lambda s,p:Probe(s,p,args.version), args.version)
                client.pump(10, lambda r:r['name']=='position')
                command('tp McsaProbe 0.5 80 0.5')
                client.pump(5, lambda r:r.get('position',[])[:3]==[.5,80.,.5])
                command('fill -4 79 -4 4 79 4 minecraft:stone')
                client.pump(1)
                for name, nbt in CASES:
                    # Release notes26.3: BlockState Name/Properties → id/properties.
                    if args.version == 777 and name in ('falling_block','enderman','block_display'):
                        nbt = nbt.replace('Name:', 'id:').replace('Properties:', 'properties:')
                    start = len(client.observed)
                    command('summon '+name+' 2 80 2 '+nbt)
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
                    report['entities'].append({'type':name,'id':event['entity'],'packets':packets})
                for mode, dimension in [('survival','the_nether'),('creative','the_end'),('spectator','overworld'),('adventure','the_nether')]:
                    command('gamemode '+mode+' McsaProbe'); client.pump(.2)
                    start = len(client.observed)
                    command('execute in minecraft:'+dimension+' run tp McsaProbe 0.5 120 0.5')
                    client.pump(20, lambda r:r['name']=='respawn'); client.pump(1)
                    records = [p for p in client.observed[start:] if p['name']=='respawn']
                    assert len(records)==1
                    report['respawns'].append({'mode':mode,'dimension':dimension,**records[0]})
            except BaseException as error:
                report['error'] = str(error)
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
