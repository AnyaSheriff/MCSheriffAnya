#!/usr/bin/env python3
"""Наблюдаем только данные явно поставленных блоков, без записи мира/реестров."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import time
from probe_gameplay import Client, connect
from smoke import read_varint, varint

BLOCKS = ['furnace', 'chest', 'trapped_chest', 'ender_chest', 'jukebox', 'dispenser',
          'dropper', 'oak_sign', 'oak_hanging_sign', 'spawner', 'piston', 'brewing_stand',
          'enchanting_table', 'end_portal', 'beacon', 'skeleton_skull', 'daylight_detector',
          'hopper', 'comparator', 'white_banner', 'structure_block', 'end_gateway',
          'command_block', 'shulker_box', 'white_bed', 'conduit', 'barrel', 'smoker',
          'blast_furnace', 'lectern', 'bell', 'jigsaw', 'campfire', 'beehive', 'sculk_sensor',
          'calibrated_sculk_sensor', 'sculk_catalyst', 'sculk_shrieker', 'chiseled_bookshelf',
          'oak_shelf', 'suspicious_sand', 'decorated_pot', 'crafter', 'trial_spawner', 'vault']
# Названия блоков, а не предположенные ID реестра.


def exact(data, n):
    assert 0 <= n <= 8 * 1024 * 1024
    value = data.read(n)
    assert len(value) == n
    return value


def nbt(data, kind=None, depth=0):
    assert depth < 64
    if kind is None:
        kind = exact(data, 1)[0]
    if kind == 0:
        return None
    if kind in (1, 2, 3, 4, 5, 6):
        return struct.unpack({1:'>b',2:'>h',3:'>i',4:'>q',5:'>f',6:'>d'}[kind], exact(data, {1:1,2:2,3:4,4:8,5:4,6:8}[kind]))[0]
    if kind == 8:
        return exact(data, int.from_bytes(exact(data, 2), 'big')).decode('utf-8', errors='replace')
    if kind == 10:
        fields = {}
        while (child := exact(data, 1)[0]):
            key = nbt(data, 8, depth + 1)
            assert key not in fields
            fields[key] = nbt(data, child, depth + 1)
        return fields
    if kind == 9:
        child = exact(data, 1)[0]
        count = int.from_bytes(exact(data, 4), 'big', signed=True)
        assert 0 <= count <= 65536
        return [nbt(data, child, depth + 1) for _ in range(count)]
    if kind in (7, 11, 12):
        count = int.from_bytes(exact(data, 4), 'big', signed=True)
        exact(data, count * {7:1,11:4,12:8}[kind])
        return {'array_count':count}
    raise ValueError('NBT type ' + str(kind))


class Probe(Client):
    def __init__(self, sock, protocol, version):
        super().__init__(sock, protocol)
        self.version = version
        self.entities = None

    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + (struct.pack('>dddff', *values) if self.version == 777 else b''))

    def observe_packet(self, name, payload):
        if name != 'map_chunk' or payload[:8] != struct.pack('>ii', -1, -1):
            return
        data = io.BytesIO(payload[8:])
        for _ in range(read_varint(data.read)):
            read_varint(data.read)
            exact(data, read_varint(data.read) * 8)
        exact(data, read_varint(data.read))  # не сохраняем секции/генерацию мира
        entities = []
        for _ in range(read_varint(data.read)):
            xz = exact(data, 1)[0]
            y = int.from_bytes(exact(data, 2), 'big', signed=True)
            kind = read_varint(data.read)
            start = data.tell()
            value = nbt(data)
            end = data.tell()
            if y != 65:
                continue
            index = (xz & 15) * 16 + (xz >> 4)
            assert index < len(BLOCKS)
            entities.append({'block':BLOCKS[index], 'kind':kind, 'position':[-16 + (xz >> 4),y,-16 + (xz & 15)],
                             'nbt':value, 'nbt_hex':data.getvalue()[start:end].hex()})
        self.entities = entities


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True);p.add_argument('--version',type=int,choices=[775,777],required=True)
    p.add_argument('--port',type=int,default=26018)
    args=p.parse_args(); args.output.mkdir(parents=True,exist_ok=True)
    expected={775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}
    with args.jar.open('rb') as f: assert hashlib.file_digest(f,'sha1').hexdigest()==expected[args.version]
    path=args.data/('protocol.json' if args.version==777 else 'protocol-26.1.json')
    manifest=json.loads((args.data/'sources.json').read_text())
    assert hashlib.sha256(path.read_bytes()).hexdigest()==next(f['sha256'] for f in manifest['files']+manifest['baseline_files'] if f['file']==path.name)
    protocol=json.loads(path.read_text());report={'version':args.version,'jar_sha1':expected[args.version]}
    with tempfile.TemporaryDirectory(prefix='mcsa-block-entities-') as directory:
        root=Path(directory);(root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\n')
        log=args.output/'server.log'
        with log.open('w') as output:
            server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(args.jar.resolve()),'nogui'],cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
            client=None
            def command(value): server.stdin.write(value+'\n');server.stdin.flush()
            try:
                deadline=time.monotonic()+120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(.1)
                command('forceload add -16 -16')
                command('fill -16 64 -16 -1 64 -1 minecraft:stone')
                command('fill -16 66 -16 -1 66 -1 minecraft:stone')
                for i,block in enumerate(BLOCKS):
                    x,z=-16+i%16,-16+i//16
                    suffix='[attached=true]' if block=='oak_hanging_sign' else ''
                    if block=='oak_sign': suffix='{front_text:{messages:["MCSA probe","","",""]}}'
                    command(f'setblock {x} 65 {z} minecraft:{block}'+suffix)
                time.sleep(2)
                index = BLOCKS.index('white_bed')
                command(f'execute if block {-16+index%16} 65 {-16+index//16} minecraft:white_bed run say MCSA_BED_PRESENT')
                command(f'execute unless block {-16+index%16} 65 {-16+index//16} minecraft:white_bed run say MCSA_BED_MISSING')
                deadline=time.monotonic()+5
                while 'MCSA_BED_' not in log.read_text():
                    assert time.monotonic()<deadline
                    time.sleep(.05)
                report['bed_block_present']='MCSA_BED_PRESENT' in log.read_text()
                assert report['bed_block_present'], 'bed block was removed: cannot infer its BE absence'
                client=connect(args.port,protocol,lambda sock,p:Probe(sock,p,args.version),version=args.version)
                deadline=time.monotonic()+15
                while client.entities is None:
                    assert time.monotonic()<deadline
                    client.pump(.2)
                report['login_initial_hex']=client.login_body.hex()
                first_entities=client.entities
                command('gamemode survival McsaProbe')
                client.pump(.5)
                client.sock.close(); client=None
                time.sleep(.5)
                client=connect(args.port,protocol,lambda sock,p:Probe(sock,p,args.version),version=args.version)
                deadline=time.monotonic()+15
                while client.login_body is None:
                    assert time.monotonic()<deadline
                    client.pump(.2)
                report['login_after_switch_hex']=client.login_body.hex()
                client.entities=first_entities
                report['entities']=client.entities
                report['missing']=[name for name in BLOCKS if name not in {e['block'] for e in client.entities}]
            finally:
                if client: client.sock.close()
                if server.poll() is None:
                    command('stop')
                    try: server.wait(timeout=30)
                    except subprocess.TimeoutExpired: server.kill();server.wait()
                report['exit_code']=server.returncode
                (args.output/'result.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n')
    assert report['exit_code']==0
    print(json.dumps({'version':args.version,'entities':len(report['entities']),'missing':report['missing']}))

if __name__=='__main__':main()
