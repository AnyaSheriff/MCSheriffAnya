#!/usr/bin/env python3
# Площадки — обычные команды официального сервера. Содержимое JAR не читается.
# Запускать отдельно: один официальный сервер, куча не больше 2 GiB.
import argparse,sys,time,json,re
from pathlib import Path
sys.path.insert(0,str(Path('tools/measure').resolve()))
from genaudit_263_generate import Console
parser=argparse.ArgumentParser()
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--jar',type=Path,required=True)
parser.add_argument('--java',default='/usr/lib/jvm/java-25-openjdk/bin/java')
parser.add_argument('--port',type=int,choices=range(26100,26200),default=26172)
parser.add_argument('--poplar-population',action='store_true',help='Дополнительно сохранить четыре окна естественного пятнистого леса')
parser.add_argument('--dappled-plots',action='store_true',help='Вместо крон тёмного дуба измерить 128 изолированных деревьев естественного селектора')
parser.add_argument('--spruce-plots',action='store_true',help='Проверить 32 изолированных spruce, используемых естественным селектором')
a=parser.parse_args()
r=a.output.resolve()
if r.is_relative_to(Path(__file__).resolve().parents[2]):
 raise ValueError('Нужен свежий каталог вне репозитория')
r.mkdir(parents=True,exist_ok=False)
(r/'eula.txt').write_text('eula=true\n')
(r/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nlevel-seed=1792932773345722119\nonline-mode=false\nview-distance=2\nsimulation-distance=2\npause-when-empty-seconds=0\nmax-tick-time=-1\n')
s=Console([a.java,'-Xms256M','-Xmx2G','-XX:ActiveProcessorCount=2','-jar',str(a.jar.resolve()),'nogui'],r,r'Done \(')
plots={}
try:
 reply=s.command('seed')
 if '1792932773345722119' not in reply:
  raise ValueError('Сервер не подтвердил семя')
 (r/'observed-seed.json').write_text(json.dumps({'seed':1792932773345722119,'console_reply':reply})+'\n')
 s.command('gamerule minecraft:random_tick_speed 0')
 s.command('gamerule minecraft:spawn_mobs false')
 if a.spruce_plots:
  points=[]
  s.command('forceload add 6144 6144 6239 6335')
  time.sleep(10)
  for i in range(4):
   for j in range(8):
    x,z=6156+i*24,6156+j*24
    s.command(f'fill {x-9} 199 {z-9} {x+9} 199 {z+9} minecraft:grass_block')
    reply=s.command(f'place feature minecraft:spruce {x} 200 {z}')
    if 'Unknown' in reply or 'Invalid' in reply or "Can't find" in reply:raise ValueError(reply)
    points.append([x,z])
  plots['spruce_configured']={'points':points,'base_y':200}
  (r/'plots.json').write_text(json.dumps(plots,indent=2)+'\n')
  s.command('tick sprint 100')
  time.sleep(3)
  s.command('save-all flush')
  sys.exit(0)
 if a.dappled_plots:
  points=[]
  replies=[]
  for base in (4608,4864):
   s.command(f'forceload add {base} 4608 {base+191} 4799')
   time.sleep(10)
   for i in range(8):
    for j in range(8):
     x,z=base+12+i*24,4608+12+j*24
     s.command(f'fill {x-9} 199 {z-9} {x+9} 199 {z+9} minecraft:grass_block')
     s.command(f'fillbiome {x-9} 192 {z-9} {x+9} 224 {z+9} minecraft:dappled_forest')
     reply=s.command(f'place feature minecraft:trees_dappled_forest {x} 200 {z}')
     if 'Unknown' in reply or 'Invalid' in reply or "Can't find" in reply:raise ValueError(reply)
     points.append([x,z]);replies.append(reply)
   s.command('save-all flush')
   s.command('forceload remove all')
  plots['dappled_selector']={'points':points,'base_y':200}
  (r/'plots.json').write_text(json.dumps(plots,indent=2)+'\n')
  (r/'placement-replies.json').write_text(json.dumps(replies,indent=2)+'\n')
  s.command('tick sprint 100')
  time.sleep(3)
  s.command('save-all flush')
  sys.exit(0)
 s.command('forceload add 4096 4096 4223 4303')
 s.command('forceload add 4352 4096 4463 4207')
 time.sleep(10)
 for species,feature,base,nx,nz in [('dark_oak','dark_oak',4096,4,8),('spruce_leaf_litter','spruce_leaf_litter',4352,4,4)]:
  points=[]
  for i in range(nx):
   for j in range(nz):
    x,z=base+12+i*24,4096+12+j*24
    s.command(f'fill {x-9} 199 {z-9} {x+9} 199 {z+9} minecraft:grass_block')
    reply=s.command(f'place feature minecraft:{feature} {x} 200 {z}')
    if 'Unknown' in reply or 'Invalid' in reply or "Can't find" in reply:print(reply,flush=True)
    points.append([x,z])
  plots[species]={'points':points,'base_y':200}
 (r/'plots.json').write_text(json.dumps(plots,indent=2)+'\n')
 s.command('tick sprint 100')
 time.sleep(3)
 s.command('save-all flush')
 if a.poplar_population:
  plan={'seed':1792932773345722119,'windows':{'overworld':[],'the_nether':[],'the_end':[]},
        'method':'Four locate biome dappled_forest queries from +/-8192; zero random ticks'}
  s.command('forceload remove all')
  for x,z in [(-8192,-8192),(-8192,8192),(8192,-8192),(8192,8192)]:
   reply=s.command(f'execute positioned {x} 64 {z} run locate biome minecraft:dappled_forest')
   match=re.search(r'\[(-?\d+), (?:~|-?\d+), (-?\d+)\]',reply)
   if not match:raise ValueError(reply)
   bx,bz=map(int,match.groups());cx,cz=bx//16,bz//16
   bounds=[cx-4,cz-4,cx+4,cz+4];plan['windows']['overworld'].append(bounds)
   s.command(f'forceload add {(cx-4)*16} {(cz-4)*16} {(cx+4)*16+15} {(cz+4)*16+15}')
   time.sleep(15)
   s.command('save-all flush')
   s.command('forceload remove all')
  (r/'poplar-plan.json').write_text(json.dumps(plan,indent=2)+'\n')
finally:s.close()
print('Stopped',s.process.returncode,flush=True)
