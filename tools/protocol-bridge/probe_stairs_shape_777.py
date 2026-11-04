"""Форма ступеней 26.3: 1280 расстановок (512 пар + 768 троек) через setblock/execute if block на официальном сервере.
Запуск: из пустой папки с подпапкой world, JAR как непрозрачный; результат shapes.json -> src/gameplay_catalog/stairs-shape-26.3.json."""
import subprocess,itertools,json,time,sys
W='world'
open(W+'/eula.txt','w').write('eula=true\n')
open(W+'/server.properties','w').write('server-ip=127.0.0.1\nserver-port=26148\nonline-mode=false\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\nspawn-monsters=false\n')
F=['north','south','west','east'];D={'north':(0,-1),'south':(0,1),'west':(-1,0),'east':(1,0)}
S=['straight','inner_left','inner_right','outer_left','outer_right']
cmds=[];keys=[]
A=(0,70,0)
def st(name,f,h):return f'{name}[facing={f},half={h}]'
def sb(p,s):cmds.append(f'setblock {p[0]} {p[1]} {p[2]} {s}')
def clear():
    for dx in (-1,0,1):
        for dz in (-1,0,1):sb((dx,70,dz),'air')
def query(key):
    for s in S:cmds.append(f'execute if block 0 70 0 white_wool_stairs[shape={s}] run say R|{key}|{s}')
# set 1: pair
for fa,fb,d,ha,hb,tb in itertools.product(F,F,F,['bottom','top'],['bottom','top'],['white_wool_stairs','oak_stairs']):
    clear();sb(A,st('white_wool_stairs',fa,ha));dx,dz=D[d];sb((dx,70,dz),st(tb,fb,hb));query(f'P,{fa},{fb},{d},{ha},{hb},{tb}')
# set 2: triple, all same half bottom, B and C different positions
for fa,pb,fb,pc,fc in itertools.product(F,F,F,F,F):
    if pb==pc:continue
    clear();sb(A,st('white_wool_stairs',fa,'bottom'));sb((D[pb][0],70,D[pb][1]),st('white_wool_stairs',fb,'bottom'));sb((D[pc][0],70,D[pc][1]),st('white_wool_stairs',fc,'bottom'));query(f'T,{fa},{pb},{fb},{pc},{fc}')
cmds.append('stop')
p=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar','/home/lenox/.local/share/mcsa-java-26.3-probe/server.jar','nogui'],cwd=W,stdin=subprocess.PIPE,stdout=open('server.log','w'),stderr=subprocess.STDOUT,text=True,start_new_session=True)
while 'Done (' not in open('server.log').read():time.sleep(.3)
p.stdin.write('forceload add -2 -2 2 2\n');p.stdin.flush();time.sleep(4);p.stdin.write('fill -2 69 -2 2 69 2 stone\n')
for c in cmds:p.stdin.write(c+'\n')
p.stdin.flush();p.wait(timeout=900)
res={}
for line in open('server.log'):
    if 'R|' in line:
        k,s=line.strip().split('R|',1)[1].split('|');res.setdefault(k,[]).append(s)
json.dump(res,open('shapes.json','w'))
print(len(res),sum(len(v)!=1 for v in res.values()))
