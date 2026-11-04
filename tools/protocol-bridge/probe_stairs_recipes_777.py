#!/usr/bin/env python3
"""Выбранные рецепты ступеней бетона/шерсти (777): показ рецептов, ручной верстак, камнерез.
Официальный JAR как непрозрачный; сохраняются только выбранные показы и слоты."""
import argparse,sys,os,json,hashlib,subprocess,socket,time,struct,signal,copy,io,traceback
from pathlib import Path
from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,HASHES,pack_position
from probe_registry_client import read_string
from smoke import varint,read_varint
from probe_stonecutter_776 import selected_display
COLORS=['white','orange','magenta','light_blue','yellow','lime','pink','gray','light_gray','cyan','purple','blue','brown','green','red','black']
def main():
 p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--port',type=int,default=26147);a=p.parse_args()
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();jar=a.jar.resolve();data=a.data
 assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 for n,h in HASHES.items():assert hashlib.sha256((data/n).read_bytes()).hexdigest()==h
 protocol=json.loads((data/'protocol.json').read_text());names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())};items={v:k for k,v in names.items()}
 def uint(d,cap=65536):
  n=read_varint(d.read);assert 0<=n<=cap,n;return n
 def boolean(d):
  b=d.read(1);assert b in [b'\0',b'\1'];return b==b'\1'
 def idset(d):
  n=uint(d);return {'tag':read_string(d)} if n==0 else {'items':[uint(d) for _ in range(n-1)]}
 target=lambda n:n.endswith('_stairs') and (n.endswith('wool_stairs') or n.endswith('concrete_stairs'))
 class Probe(InventoryProbe):
  def __init__(self,*x):super().__init__(*x);self.stage='join';self.selected=[];self.cuts={};self.game_mode=None
  def slotdisplay(self,d,depth=0):
   kind=uint(d,10);r={'type':kind}
   if kind in [0,1]:pass
   elif kind==2:r['base']=self.slotdisplay(d,depth+1)
   elif kind==4:r['item']=uint(d)
   elif kind==5:r['stack']=self.slot(d,template=True)
   elif kind==6:r['set']=idset(d)
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
   if name=='game_state_change' and payload and payload[0]==3:self.game_mode=int(struct.unpack('>f',payload[1:])[0])
   if name=='declare_recipes':
    for c in COLORS:
     for fam in ['concrete','wool']:
      try:
       t=selected_display(payload,items[c+'_'+fam],items[c+'_'+fam+'_stairs']);t['button_ordinal_for_input']=t.pop('button_ordinal_for_stone');self.cuts[c+'_'+fam]=t
      except BaseException as e:self.cuts.setdefault('missing',[]).append([c+'_'+fam,repr(e)[:80]])
   if self.stage=='join' or name!='recipe_book_add':return
   d=io.BytesIO(payload);sel=[]
   for _ in range(uint(d,256)):
    ident=uint(d);display=self.display(d);group=uint(d);category=uint(d,12)
    req=[idset(d) for _ in range(uint(d,32))] if boolean(d) else None
    flags=d.read(1)
    res=display['result'];item=res.get('stack',{}).get('item') or res.get('item')
    item=names.get(item,item) if isinstance(item,int) else item
    if isinstance(item,str) and target(item):sel.append({'display':display,'group_raw':group,'category':category,'requirements':req,'flags':flags[0],'stage':self.stage,'result_name':item})
   self.selected+=sel
 rep={'jar_sha1':JAR_SHA1,'steps':{},'commands':[]};log=out/'server.log';port=a.port;server=None;client=None;seq=0
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 def command(t,w=.18):rep['commands'].append(t);server.stdin.write(t+'\n');server.stdin.flush();client.pump(w)
 def close():
  if client.inventory and client.inventory['window']:client.send('close_window',varint(client.inventory['window']));client.pump(.12)
 def open_at(at):
  nonlocal seq
  close();seq+=1;client.send('block_place',b'\0'+pack_position(at)+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(seq));client.pump(.3)
 def click(place,button=0):
  inv=client.inventory;inv['cursor']=None;client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',place,button)+b'\0\0\0');client.pump(.18)
 try:
  with log.open('w') as o:
   server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui'],cwd=world,stdin=subprocess.PIPE,stdout=o,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   until=time.monotonic()+100
   while 'Done (' not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,pr:Probe(s,pr,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
   for t in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','setblock 2 64 0 crafting_table','setblock 2 64 2 stonecutter','clear McsaProbe','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(t)
   assert client.game_mode==0
   rep['cuts']=client.cuts
   for fam in ['concrete','wool']:
    for color in COLORS:
     base=color+'_'+fam;stairs=base+'_stairs'
     close();command('clear McsaProbe');client.stage=color+'/'+fam+'/natural';start=len(client.selected);command('give McsaProbe '+base+' 6',.4)
     rep['steps'][base+'/natural']=client.selected[start:]
     for name in [stairs,stairs+'_from_'+base+'_stonecutting']:
      client.stage=color+'/'+fam+'/take';command('recipe take McsaProbe minecraft:'+name);client.stage=color+'/'+fam+'/named/'+name;start=len(client.selected);command('recipe give McsaProbe minecraft:'+name,.3)
      rep['steps'][base+'/named/'+name]=client.selected[start:]
     if color not in ('white','red'):continue
     command('clear McsaProbe');command('give McsaProbe '+base+' 12',.4);client.stage='craft';open_at((2,64,0))
     for label,grid in [('left',[1,4,5,7,8,9]),('right',[3,5,6,7,8,9])]:
      src=next(i for i,v in enumerate(client.inventory['slots']) if i>=10 and v and v['item']==base);click(src)
      for g in grid:click(g,1)
      click(src);rep['steps'][base+'/craft-'+label]={'result':copy.deepcopy(client.inventory['slots'][0])}
      if client.inventory['slots'][0]:
       click(0);rep['steps'][base+'/craft-'+label]['cursor']=copy.deepcopy(client.inventory['cursor']);click(38)
       for g in range(1,10):
        if client.inventory['slots'][g]:click(g);click(38)
     close();client.stage='cut';open_at((2,64,2))
     places=[i for i,v in enumerate(client.inventory['slots']) if i>=2 and v and v['item']==base]
     if places:
      source=places[0];click(source)
      click(0);cut=client.cuts.get(base)
      if cut:
       client.send('enchant_item',varint(client.inventory['window'])+varint(cut['button_ordinal_for_input']));client.pump(.2)
       rep['steps'][base+'/cut']={'slots':copy.deepcopy(client.inventory['slots'][:3])}
 except BaseException as e:rep['error']=repr(e);rep['traceback']=traceback.format_exc()
 finally:
  if client:client.sock.close()
  if server and server.poll() is None:
   try:server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=25)
   except BaseException:os.killpg(server.pid,signal.SIGTERM)
  (out/'result.json').write_text(json.dumps(rep,indent=1)+'\n');print('done',rep.get('error'))
if __name__=='__main__':main()
