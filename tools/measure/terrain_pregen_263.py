#!/usr/bin/env python3
# Свой мир и обычные команды: генерация выбранных чанков, затем сохранение.
# У оригинала JAR остаётся непрозрачным, читаются лишь сохранённые регионы.
import argparse, json, re, subprocess, time
from pathlib import Path


def main():
    p=argparse.ArgumentParser();p.add_argument('--kind',choices=['official','ours'],required=True);p.add_argument('--executable',type=Path,required=True);p.add_argument('--plan',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--port',type=int,required=True);p.add_argument('--biome');a=p.parse_args()
    official=a.kind=='official';assert (26100<=a.port<=26199) if official else (26200<=a.port<=26299)
    plan=json.loads(a.plan.read_text());root=a.output.resolve();root.mkdir(parents=True,exist_ok=False);config=root if official else root/'config';config.mkdir(exist_ok=True)
    (config/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nlevel-type=minecraft:normal\nlevel-seed={plan["seed"]}\nview-distance=2\nsimulation-distance=2\nmax-tick-time=0\npause-when-empty-seconds=0\n')
    if official:
        (root/'eula.txt').write_text('eula=true\n');command=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx768M','-jar',str(a.executable.resolve()),'nogui']
    else:
        (config/'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n');(config/'skins.toml').write_text('mojang=false\nely=false\n');command=[str(a.executable.resolve())]
    logpath=root/'server.log'
    with logpath.open('w') as log:
        server=subprocess.Popen(command,cwd=root,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
        def cmd(s):server.stdin.write(s+'\n');server.stdin.flush()
        def wait(marker,seconds=240):
            deadline=time.monotonic()+seconds
            while marker not in logpath.read_text():
                assert server.poll() is None and time.monotonic()<deadline,marker
                time.sleep(.2)
        try:
            wait('Done (' if official else 'Сервер запущен',120)
            cmd('gamerule random_tick_speed 0')
            if a.biome:
                assert official and re.fullmatch('[a-z_]+',a.biome)
                cmd('locate biome minecraft:'+a.biome)
                deadline=time.monotonic()+120
                while True:
                    found=re.search(r'is at \[(-?\d+), [^,]+, (-?\d+)\]',logpath.read_text())
                    if found:break
                    assert server.poll() is None and time.monotonic()<deadline,'locate biome failed'
                    time.sleep(.2)
                plan['source']='Official 26.3 locate biome '+a.biome+' followed by an 8x8 chunk census window'
                plan['located_xz']=[int(found[1]),int(found[2])]
                cx,cz=int(found[1])//16,int(found[2])//16
                plan['chunks']=[[x,z] for x in range(cx-4,cx+4) for z in range(cz-4,cz+4)]
                (root/'observed-plan.json').write_text(json.dumps(plan,indent=2))
            for cx,cz in plan['chunks']:cmd(f'forceload add {cx*16} {cz*16}')
            cmd('say TERRAIN_GENERATION_DONE');wait('TERRAIN_GENERATION_DONE')
            # Оригинал складывает чанки асинхронно после forceload.
            if official:time.sleep(15)
            cmd('save-all flush');cmd('say TERRAIN_SAVE_DONE');wait('TERRAIN_SAVE_DONE')
        finally:
            if server.poll() is None:
                cmd('stop')
                try:server.wait(timeout=45)
                except subprocess.TimeoutExpired:server.terminate();server.wait(timeout=15)
    assert server.returncode==0,server.returncode
    print(json.dumps({'kind':a.kind,'seed':plan['seed'],'chunks':len(plan['chunks']),'exit_code':server.returncode}))

if __name__=='__main__':main()
