#!/usr/bin/env python3
# Чёрный ящик: собственные серверы/площадки, JAR только запускается.
import argparse, hashlib, io, json, socket, subprocess, sys, time
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'protocol-bridge'))
from probe_gameplay import Client, connect
from smoke import read_varint

COVERS=['stone','oak_planks','stone_slab[type=bottom]','stone_slab[type=top]',
        'stone_slab[type=double]','stone_slab[type=top,waterlogged=true]',
        'snow[layers=1]','snow[layers=2]','snow[layers=8]','glass','poplar_planks','water']

class CoverClient(Client):
    def __init__(self,*args):
        super().__init__(*args)
        self.block_updates=[]
    def observe_packet(self,name,payload):
        if name=='multi_block_change':
            data=io.BytesIO(payload);packed=int.from_bytes(data.read(8),'big')
            def signed(n,b):return n-(1<<b) if n&(1<<(b-1)) else n
            cx,cz,sy=signed(packed>>42,22),signed((packed>>20)&((1<<22)-1),22),signed(packed&((1<<20)-1),20)
            for _ in range(read_varint(data.read)):
                record=read_varint(data.read)
                self.block_updates.append(([cx*16+((record>>8)&15),sy*16+(record&15),cz*16+((record>>4)&15)],record>>12))
        elif name=='block_change':
            data=io.BytesIO(payload);packed=int.from_bytes(data.read(8),'big')
            def signed(n,b):return n-(1<<b) if n&(1<<(b-1)) else n
            p=[signed(packed>>38,26),signed(packed&4095,12),signed((packed>>12)&((1<<26)-1),26)]
            self.block_updates.append((p,read_varint(data.read)))

def run(args):
    official=args.kind=='official';assert (26100<=args.port<=26199) if official else (26200<=args.port<=26299)
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
    config=root if official else root/'config';config.mkdir(exist_ok=True)
    (config/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nenforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=4\nsimulation-distance=4\ngamemode=creative\nspawn-protection=0\nmax-tick-time=0\nallow-flight=true\n')
    if official:
        (root/'eula.txt').write_text('eula=true\n')
        command=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx768M','-jar',str(args.executable.resolve()),'nogui']
    else:
        (config/'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\npause-when-empty-seconds=0\n')
        (config/'skins.toml').write_text('mojang=false\nely=false\n')
        command=[str(args.executable.resolve())]
    logpath=root/'server.log';client=None;result={'kind':args.kind,'executable_sha256':hashlib.sha256(args.executable.read_bytes()).hexdigest(),'covers':{}}
    with logpath.open('w') as log:
        server=subprocess.Popen(command,cwd=root,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
        def cmd(s):server.stdin.write(s+'\n');server.stdin.flush()
        try:
            deadline=time.monotonic()+180
            while True:
                assert server.poll() is None and time.monotonic()<deadline,'startup failed'
                try:
                    with socket.create_connection(('127.0.0.1',args.port),timeout=.2):
                        if ('Done (' if official else 'Готово (') in logpath.read_text():break
                except OSError:time.sleep(.2)
            protocol=json.loads((args.data/'protocol.json').read_text())
            client=connect(args.port,protocol,client_type=CoverClient);client.pump(20,lambda r:r['name']=='position')
            cmd('tp McsaProbe 8.5 66 16.5');client.pump(3)
            cmd('gamerule random_tick_speed 0');cmd('gamerule advance_weather false');cmd('weather clear');cmd('time set noon')
            cmd('fill -2 63 -2 19 66 38 air');cmd('fill -2 63 -2 19 63 38 stone')
            for i,cover in enumerate(COVERS):
                for j in range(8):
                    x,z=j*2,i*3
                    cmd(f'setblock {x} 64 {z} grass_block');cmd(f'setblock {x} 65 {z} {cover}')
            client.pump(2);start=len(client.records);client.block_updates.clear()
            cmd('gamerule random_tick_speed 4096');client.pump(5);cmd('gamerule random_tick_speed 0');client.pump(.5)
            for i,cover in enumerate(COVERS):
                for j in range(8):
                    for block in ['dirt','grass_block']:
                        cmd(f'execute if block {j*2} 64 {i*3} {block} run say COVER_{i}_{j}_{block}')
            client.pump(2)
            text=logpath.read_text()
            for i,cover in enumerate(COVERS):
                result['covers'][cover]={block:sum(f'COVER_{i}_{j}_{block}' in text for j in range(8)) for block in ['dirt','grass_block']}
                assert sum(result['covers'][cover].values())==8,(cover,result['covers'][cover])
            dirt=next(b['defaultState'] for b in json.loads((args.data/'blocks.json').read_text()) if b['name']=='dirt')
            result['client_dirt_positions']=sorted({tuple(p) for p,state in client.block_updates if p[1]==64 and state==dirt})
            (root/'packets.json').write_text(json.dumps(client.records[start:],indent=2))
        finally:
            if client:client.sock.close()
            if server.poll() is None:
                cmd('stop')
                try:server.wait(timeout=30)
                except subprocess.TimeoutExpired:server.terminate();server.wait(timeout=15)
            result['exit_code']=server.returncode
            (root/'result.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result,indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--kind',choices=['official','ours'],required=True);p.add_argument('--executable',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--port',type=int,required=True);p.add_argument('--output',type=Path,required=True);run(p.parse_args())
