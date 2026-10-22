#!/usr/bin/env python3
"""Соломенная кровать, подушки, карты строений, пластинка Bounce 26.3 (777): рецепты книги, камнерез, выпадения и размеры стопок.
Официальный JAR как непрозрачный; сохраняются только выбранные показы и слоты.
Выпадения — /loot give (сетевой наблюдаемый результат), рецепты — recipe_book_add."""
import argparse,os,json,hashlib,subprocess,time,struct,signal,io,re,traceback
from pathlib import Path
from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,HASHES
from probe_registry_client import read_string
from smoke import read_varint
from probe_stonecutter_776 import plain_template

WANT=re.compile(r'straw|cushion|_map|bounce|hay')
WOOD=['straw_bed', 'music_disc_bounce', 'white_cushion', 'orange_cushion', 'magenta_cushion', 'light_blue_cushion', 'yellow_cushion', 'lime_cushion', 'pink_cushion', 'gray_cushion', 'light_gray_cushion', 'cyan_cushion', 'purple_cushion', 'blue_cushion', 'brown_cushion', 'green_cushion', 'red_cushion', 'black_cushion', 'ocean_monument_map', 'woodland_mansion_map', 'buried_trial_chambers_map', 'jungle_pyramid_map', 'swamp_hut_map', 'desert_village_map', 'plains_village_map', 'savanna_village_map', 'snowy_village_map', 'taiga_village_map', 'buried_treasure_map', 'buried_ancient_city_map', 'buried_mineshaft_map', 'desert_pyramid_map', 'abandoned_camp_map', 'warm_ocean_ruins_map']
MINERAL=[]
# блоки для проверки выпадений: (метка, запись setblock)
DROPS=[('straw_bed_head','straw_bed[part=head]'),('straw_bed_foot','straw_bed[part=foot]'),('white_bed','white_bed[part=head]'),('hay_block','hay_block')]
DROPS+=[]
for n in MINERAL:
    if n.endswith('_slab'):DROPS+=[(n,n),(n+'_double',n+'[type=double]')]
    else:DROPS.append((n,n))
TOOLS=[('hand',None),('pickaxe','iron_pickaxe'),('axe','iron_axe'),('shears','shears')]

def main():
 p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--port',type=int,default=26150);p.add_argument('--leaves',type=int,default=0);a=p.parse_args()
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();jar=a.jar.resolve();data=a.data
 assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 for n,h in HASHES.items():assert hashlib.sha256((data/n).read_bytes()).hexdigest()==h
 protocol=json.loads((data/'protocol.json').read_text());names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())}
 def uint(d,cap=1<<20):
  n=read_varint(d.read);assert 0<=n<=cap,n;return n
 def boolean(d):
  b=d.read(1);assert b in [b'\0',b'\1'];return b==b'\1'
 def idset(d):
  n=uint(d);return {'tag':read_string(d)} if n==0 else {'items':[names.get(uint(d)) for _ in range(n-1)]}
 class Probe(InventoryProbe):
  def __init__(self,*x):super().__init__(*x);self.stage='join';self.book=[];self.cuts=[]
  def slotdisplay(self,d,depth=0):
   kind=uint(d,10);r={'type':kind}
   if kind in [0,1]:pass
   elif kind==2:r['base']=self.slotdisplay(d,depth+1)
   elif kind==4:r['item']=names.get(uint(d))
   elif kind==5:
    t=plain_template(d);t['item']=names.get(t['item']);r['stack']=t
   elif kind==3:r['source']=self.slotdisplay(d,depth+1);r['component']=uint(d)
   elif kind==6:r['set']=idset(d)
   elif kind==8:
    r['parts']=[self.slotdisplay(d,depth+1) for _ in range(2)];pat=uint(d);assert pat!=0,'inline trim pattern';r['pattern']=pat
   elif kind in [7,9]:r['parts']=[self.slotdisplay(d,depth+1) for _ in range(2)]
   elif kind==10:r['parts']=[self.slotdisplay(d,depth+1) for _ in range(uint(d,256))]
   else:raise ValueError(('slot display',kind))
   return r
  def display(self,d):
   kind=uint(d,4);r={'type':kind}
   if kind in [0,1]:
    if kind==1:r['width']=uint(d,3);r['height']=uint(d,3)
    r['ingredients']=[self.slotdisplay(d) for _ in range(uint(d,9))];r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   elif kind==2:
    r['ingredient']=self.slotdisplay(d);r['fuel']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d);r['duration']=uint(d);r['xp']=struct.unpack('>f',d.read(4))[0]
   elif kind==3:r['ingredient']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   elif kind==4:
    for k in ['template','base','addition','result','station']:r[k]=self.slotdisplay(d)
   return r
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload)
   if name=='declare_recipes':
    d=io.BytesIO(payload)
    for _ in range(uint(d,4096)):
     read_string(d)
     for _ in range(uint(d)):uint(d)
    for row in range(uint(d)):
     n=uint(d)
     if n==0:inputs={'tag':read_string(d)}
     else:inputs={'items':[names.get(uint(d)) for _ in range(n-1)]}
     kind=uint(d,10)
     if kind==4:res={'item':names.get(uint(d)),'count':1}
     elif kind==5:
      res=plain_template(d);res['item']=names.get(res['item'])
     else:raise ValueError(('stonecutter display',kind))
     if WANT.search(json.dumps([inputs,res])):self.cuts.append({'row':row,'inputs':inputs,'result':res})
   if name!='recipe_book_add':return
   d=io.BytesIO(payload)
   for _ in range(uint(d,2048)):
    ident=uint(d);display=self.display(d);group=uint(d);category=uint(d,12)
    req=[idset(d) for _ in range(uint(d,32))] if boolean(d) else None
    flags=d.read(1)
    if WANT.search(json.dumps(display)):self.book.append({'display':display,'group_raw':group,'category':category,'requirements':req,'flags':flags[0],'stage':self.stage})
 rep={'jar_sha1':JAR_SHA1,'drops':{},'stacks':{},'commands':[]};log=out/'server.log';port=a.port;server=None;client=None
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 def command(t,w=.12):
  server.stdin.write(t+'\n');server.stdin.flush();client.pump(w)
 def held():
  return [(i,v['item'],v['count']) for i,v in enumerate(client.inventory['slots']) if v and i>=9]
 try:
  with log.open('w') as o:
   server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui'],cwd=world,stdin=subprocess.PIPE,stdout=o,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   rep['server_pid']=server.pid
   until=time.monotonic()+100
   while 'Done (' not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,pr:Probe(s,pr,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
   for t in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','clear McsaProbe']:command(t)
   if a.leaves:
    for leaf in ['yellow_poplar_leaves','orange_poplar_leaves','red_poplar_leaves','oak_leaves']:
     command('clear McsaProbe',.1);command(f'setblock 2 64 2 {leaf}[persistent=false] replace',.2)
     server.stdin.write('execute as McsaProbe run loot give @s mine 2 64 2 mainhand\n'*a.leaves);server.stdin.flush();client.pump(6)
     tot={}
     for (_,i2,c) in held():tot[i2]=tot.get(i2,0)+c
     rep['drops']['leaves%d/%s'%(a.leaves,leaf)]=tot
   else:
      client.stage='recipes'
      for n in WOOD+MINERAL:command('recipe give McsaProbe minecraft:'+n,.15)
      rep['book']=client.book;rep['cuts']=client.cuts
      # размеры стопок: 64 штуки каждого предмета
      for n in WOOD+MINERAL:
       command('clear McsaProbe',.05);command(f'give McsaProbe {n} 64',.15)
       rep['stacks'][n]=[(c) for (_,i,c) in held() if i==n]
      command('clear McsaProbe',.05)
      # выпадения через loot
      for label,spec in DROPS:
       command(f'setblock 2 64 2 {spec} replace',.1)
       rep['drops'][label]={}
       for tname,tool in TOOLS:
        command('clear McsaProbe',.05)
        if tool:command(f'item replace entity McsaProbe weapon.mainhand with {tool}',.08)
        command('execute as McsaProbe run loot give @s mine 2 64 2 mainhand',.12)
        rep['drops'][label][tname]=sorted([(i,c) for (_,i,c) in held() if i!=tool])
      command('setblock 2 64 2 air',.05)
      # случайные выпадения листвы: 300 бросков рукой
      for leaf in []:
       command('clear McsaProbe',.05);command(f'setblock 2 64 2 {leaf}[persistent=false] replace',.1)
       tot={}
       for _ in range(300):command('execute as McsaProbe run loot give @s mine 2 64 2 mainhand',.02)
       client.pump(.5)
       for (_,i,c) in held():tot[i]=tot.get(i,0)+c
       rep['drops']['leaves300/'+leaf]=tot
 except BaseException as e:rep['error']=repr(e);rep['traceback']=traceback.format_exc()
 finally:
  if client:client.sock.close()
  if server and server.poll() is None:
   try:server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=25)
   except BaseException:os.killpg(server.pid,signal.SIGTERM)
  (out/'result.json').write_text(json.dumps(rep,indent=1)+'\n');print('done',rep.get('error'))
if __name__=='__main__':main()
