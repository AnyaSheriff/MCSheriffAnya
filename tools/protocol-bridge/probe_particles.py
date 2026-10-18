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
    ('cloud', 'cloud'), ('block', 'block{block_state:"minecraft:bamboo_planks"}'),
    ('block_marker', 'block_marker{block_state:"minecraft:bamboo_planks"}'),
    ('falling_dust', 'falling_dust{block_state:"minecraft:bamboo_planks"}'),
    ('dust_pillar', 'dust_pillar{block_state:"minecraft:bamboo_planks"}'),
    ('block_crumble', 'block_crumble{block_state:"minecraft:bamboo_planks"}'),
    ('dust', 'dust{color:[1f,0.5f,0.25f],scale:1.5f}'),
    ('dust_color_transition', 'dust_color_transition{from_color:[1f,0f,0f],to_color:[0f,0f,1f],scale:1.5f}'),
    ('entity_effect', 'entity_effect{color:[1f,0.5f,0.25f,1f]}'),
    ('item', 'item{item:{id:"minecraft:red_wool",count:5}}'),
    ('item_damage', 'item{item:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:damage":7}}}'),
    ('sculk_charge', 'sculk_charge{roll:0.5f}'), ('shriek', 'shriek{delay:7}'),
    ('vibration_block', 'vibration{destination:{type:"block",pos:[2,80,2]},arrival_in_ticks:10}'),
    ('trail', 'trail{target:[2d,80d,2d],color:16711680,duration:10}'),
    ('tinted_leaves', 'tinted_leaves{color:16711680}'),
    ('dragon_breath', 'dragon_breath{power:0.5f}'),
    ('effect', 'effect{color:16711680,power:0.5f}'),
    ('instant_effect', 'instant_effect{color:16711680,power:0.5f}'),
    ('flash', 'flash{color:[1f,0.5f,0.25f,1f]}'),
]

class Probe(Client):
    def __init__(self, sock, protocol, version):
        super().__init__(sock, protocol)
        self.version = version
        self.observed = []
    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + (struct.pack('>dddff', *values) if self.version == 777 else b''))
    def observe_packet(self, name, payload):
        if name in ('world_particles', 'entity_metadata', 'spawn_entity'):
            self.observed.append({'name': name, 'body': (varint(next(k for k,v in self.incoming.items() if v == name)) + payload).hex()})

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', type=int, choices=[775,777], required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26025)
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
    report = {'version':args.version, 'jar_sha1':expected, 'particles':[], 'metadata':[]}
    with tempfile.TemporaryDirectory(prefix='mcsa-particle-probe-') as directory:
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
                for name, particle in CASES + [('cloud_zero','cloud'),('cloud_one','cloud'),('cloud_normal','cloud')]:
                    start = len(client.observed)
                    amount = 0 if name == 'cloud_zero' else 1 if name == 'cloud_one' else 7
                    mode = 'normal' if name == 'cloud_normal' else 'force'
                    cmd = f'particle {particle} 1 81 2 0.25 0.5 0.75 0.125 {amount} {mode} McsaProbe'
                    command(cmd)
                    client.pump(4, lambda r:r['name']=='world_particles')
                    records = [p for p in client.observed[start:] if p['name']=='world_particles']
                    assert len(records)==1, (name,records)
                    report['particles'].append({'case':name,'command':cmd,**records[0]})
                for name, nbt in [('area_effect_cloud','{Duration:2000,WaitTime:0,Radius:2f}'),
                                  ('cow','{NoAI:1b,Tags:["particle_probe"]}')]:
                    start = len(client.observed)
                    command('summon '+name+' 2 80 2 '+nbt)
                    event = client.pump(5, lambda r:r['name']=='spawn_entity' and r.get('entity_type')==entities[name])
                    client.pump(.2)
                    if name=='cow':
                        start=len(client.observed)
                        command('effect give @e[tag=particle_probe,limit=1] speed 100 1 false')
                        command('effect give @e[tag=particle_probe,limit=1] strength 100 1 false')
                        client.pump(1)
                    packets=[]
                    for p in client.observed[start:]:
                        raw=io.BytesIO(bytes.fromhex(p['body']));read_varint(raw.read)
                        if p['name']=='entity_metadata' and read_varint(raw.read)==event['entity']:packets.append(p)
                    assert packets
                    report['metadata'].append({'type':name,'id':event['entity'],'packets':packets})
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
    print(json.dumps({'particles':len(report['particles']),'metadata':len(report['metadata']),'exit_code':report['exit_code']}))

if __name__ == '__main__': main()
