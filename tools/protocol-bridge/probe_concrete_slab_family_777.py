#!/usr/bin/env python3
"""Selected ordinary777 slab unlock/autofill; opaque JAR, no recipe dump."""
import argparse,sys,os,json,hashlib,subprocess,socket,time,struct,signal,copy,io,traceback
from pathlib import Path
from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,HASHES,pack_position
from probe_registry_client import read_string
from smoke import varint,read_varint
from probe_stonecutter_776 import selected_display

def main():
 parser=argparse.ArgumentParser(description='Selected concrete slab family recipes; plan unless --run')
 parser.add_argument('--run',action='store_true')
 target=parser.add_mutually_exclusive_group(required=True);target.add_argument('--jar',type=Path);target.add_argument('--test-binary',type=Path)
 parser.add_argument('--binary-sha256');parser.add_argument('--data',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
 args=parser.parse_args()
 if not args.run:print(json.dumps({'plan':'16 own-color unlock/named recipes/manual3→6/cut1→2; mixed+vertical refusal','port':26047 if args.jar else 26026}));return
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir()
 jar=args.jar.resolve() if args.jar else None;data=args.data
 if jar:assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 else:assert args.binary_sha256 and hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256
 for p,h in HASHES.items():assert hashlib.sha256((data/p).read_bytes()).hexdigest()==h
 protocol=json.loads((data/'protocol.json').read_text());names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())};items={v:k for k,v in names.items()}
 colors=['white','orange','magenta','light_blue','yellow','lime','pink','gray','light_gray','cyan','purple','blue','brown','green','red','black']
 family_names=[c+'_concrete_slab' for c in colors];slab_ids={items[n] for n in family_names}
 def uint(d,cap=65536):
  n=read_varint(d.read);assert 0<=n<=cap,n;return n
 def boolean(d):
  b=d.read(1);assert b in [b'\0',b'\1'];return b==b'\1'
 def idset(d):
  n=uint(d)
  return {'tag':read_string(d)} if n==0 else {'items':[uint(d) for _ in range(n-1)]}
 class Probe(InventoryProbe):
  def __init__(self,*a):super().__init__(*a);self.stage='join';self.selected=[];self.ghosts=[];self.cuts={};self.game_mode=None;self.mode_events=[]
  def slotdisplay(self,d,depth=0):
   assert depth<16
   kind=uint(d,10);r={'type':kind}
   if kind in [0,1]:pass
   elif kind==2:r['base']=self.slotdisplay(d,depth+1)
   elif kind==4:r['item']=uint(d)
   elif kind==5:r['stack']=self.slot(d,template=True)
   elif kind==6:r['set']=idset(d)
   elif kind in [7,9]:r['parts']=[self.slotdisplay(d,depth+1) for _ in range(2)]
   elif kind==10:r['parts']=[self.slotdisplay(d,depth+1) for _ in range(uint(d,256))]
   else:raise ValueError(('unobserved slot display',kind))
   return r
  def display(self,d):
   kind=uint(d,4);r={'type':kind}
   if kind in [0,1]:
    if kind==1:r['width']=uint(d,3);r['height']=uint(d,3)
    r['ingredients']=[self.slotdisplay(d) for _ in range(uint(d,9))]
    r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   elif kind==2:
    r['ingredient']=self.slotdisplay(d);r['fuel']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d);r['duration']=uint(d);r['xp']=struct.unpack('>f',d.read(4))[0]
   elif kind==3:r['ingredient']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   elif kind==4:
    r['template']=self.slotdisplay(d);r['base']=self.slotdisplay(d);r['addition']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   return r
  def is_slab(self,r):
   result=r['result'];return result.get('item') in slab_ids or result.get('stack',{}).get('item') in family_names
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload)
   if name=='game_state_change' and payload and payload[0]==3:
    assert len(payload)==5
    mode=struct.unpack('>f',payload[1:])[0];assert mode in [0.,1.,2.,3.]
    self.game_mode=int(mode);self.mode_events.append({'mode':int(mode),'payload_hex':payload.hex()})
   if name=='declare_recipes':
    for slab in family_names:
     target=selected_display(payload,items[slab.removesuffix('_slab')],items[slab]);target['button_ordinal_for_input']=target.pop('button_ordinal_for_stone');self.cuts[slab]=target
   if self.stage=='join':return
   d=io.BytesIO(payload)
   if name=='recipe_book_add':
    selected=[]
    for _ in range(uint(d,256)):
     ident=uint(d);display=self.display(d);group=uint(d);category=uint(d,12)
     req=[idset(d) for _ in range(uint(d,32))] if boolean(d) else None
     flags=d.read(1);assert len(flags)==1 and flags[0]&~3==0
     if self.is_slab(display):selected.append({'display_id':ident,'display':display,'group_raw':group,'category':category,'requirements':req,'flags':flags[0],'stage':self.stage})
    replace=boolean(d);assert not d.read()
    for row in selected:row['replace']=replace;self.selected.append(row)
   elif name=='craft_recipe_response':
    window=uint(d);display=self.display(d);assert not d.read()
    if self.is_slab(display):self.ghosts.append({'window':window,'display':display,'stage':self.stage})
 report={'scope':'selected ordinary777 sixteen-color concrete slabs, noGUI','jar_sha1':JAR_SHA1 if jar else None,'binary_sha256':args.binary_sha256,'colors':{},'negative_cases':{},'commands':[],'actions':[]};server=None;client=None;port=26047 if jar else 26026;log=out/'server.log';sequence=0
 with socket.socket() as s:s.bind(('127.0.0.1',port))
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 def command(text,wait=.18):report['commands'].append(text);server.stdin.write(text+'\n');server.stdin.flush();client.pump(wait)
 def close():
  if client.inventory and client.inventory['window']:client.send('close_window',varint(client.inventory['window']));client.pump(.12)
 def open_at(at):
  nonlocal sequence
  close();sequence+=1;client.send('block_place',b'\0'+pack_position(at)+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(sequence));client.pump(.3)
 def click(place,button=0):
  inv=client.inventory;inv['cursor']=None;client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',place,button)+b'\0\0\0');client.pump(.18)
 def snap(case,label):case['snapshots'][label]=copy.deepcopy(client.inventory)
 def selected(stage,slab,kind):return [r for r in client.selected if r['stage']==stage and r['display']['type']==kind and r['display']['result'].get('stack',{}).get('item')==slab]
 def saved(): (out/'partial.json').write_text(json.dumps(report,indent=2)+'\n')
 try:
  with log.open('w') as output:
   if jar:argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui']
   else:
    for marker in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']:(world/marker).touch()
    argv=[str(args.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture']
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   until=time.monotonic()+100
   while ('Done (' if jar else 'MCSA gameplay probe ready') not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,p:Probe(s,p,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
   for text in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','setblock 2 64 0 crafting_table','setblock 2 64 2 stonecutter','clear McsaProbe','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(text)
   assert client.game_mode==0,('survival-not-confirmed',client.mode_events)
   report['survival_confirmed_by_packet']=True
   for color,slab in zip(colors,family_names):
    assert client.game_mode==0
    base=color+'_concrete';case=report['colors'][color]={'base':base,'slab':slab,'checks':[],'snapshots':{},'selected_cut':client.cuts[slab]}
    close();command('clear McsaProbe');client.stage=color+'/natural';command('give McsaProbe '+base+' 4',.5)
    for kind,count in [(1,6),(3,2)]:
     rows=selected(client.stage,slab,kind);assert len(rows)==1,(color,'natural',kind,client.selected);assert rows[0]['display']['result']['stack']=={'item':slab,'count':count}
    unrelated=[r for r in client.selected if r['stage']==client.stage and r['display']['result'].get('stack',{}).get('item')!=slab];assert not unrelated,(color,'unrelated-family-unlock',unrelated)
    case['natural_entries']=[r for r in client.selected if r['stage']==client.stage];case['checks'].append('matching-color-unlocks-shaped-and-cut-only')
    for name,kind in [(slab,1),(slab+'_from_'+base+'_stonecutting',3)]:
     client.stage=color+'/take';command('recipe take McsaProbe minecraft:'+name)
     client.stage=color+'/named/'+str(kind);command('recipe give McsaProbe minecraft:'+name,.3)
     rows=selected(client.stage,slab,kind);assert len(rows)==1,(color,name,client.selected);case.setdefault('named_entries',[]).append({'name':'minecraft:'+name,'entry':rows[0]});case['checks'].append('exact-named-'+str(kind)+'-accepted-selected-entry')
    client.stage=color+'/craft';open_at((2,64,0));assert client.inventory['slots'][37]=={'item':base,'count':4}
    click(37)
    for place in [1,2,3]:click(place,1)
    click(37);snap(case,'craft-ready');assert client.inventory['slots'][0]=={'item':slab,'count':6};assert client.inventory['slots'][37]=={'item':base,'count':1}
    click(0);snap(case,'craft-taken');assert all(client.inventory['slots'][i] is None for i in range(4));assert client.inventory['cursor']=={'item':slab,'count':6}
    click(38);case['checks'].append('manual-three-same-color-produces-six-with-exact-consumption')
    client.stage=color+'/cut';open_at((2,64,2));places=[i for i,v in enumerate(client.inventory['slots']) if i>=2 and v=={'item':base,'count':1}];assert len(places)==1;source=places[0]
    click(source);click(0);button=case['selected_cut']['button_ordinal_for_input'];report['actions'].append({'color':color,'select_cut':button});client.send('enchant_item',varint(client.inventory['window'])+varint(button));client.pump(.2)
    snap(case,'cut-ready');assert client.inventory['slots'][0]=={'item':base,'count':1} and client.inventory['slots'][1]=={'item':slab,'count':2}
    click(1);snap(case,'cut-taken');assert client.inventory['slots'][0] is None and client.inventory['slots'][1] is None and client.inventory['cursor']=={'item':slab,'count':2}
    click(source);snap(case,'stored-total-eight');assert sum(v['count'] for v in client.inventory['slots'][2:] if v and v['item']==slab)==8;assert not any(v and v['item']==base for v in client.inventory['slots']);case['checks'].append('stonecut-one-matching-color-produces-two-total-eight')
    print(json.dumps({'color':color,'result':'PASS','checks':len(case['checks']),'named':[x['name'] for x in case['named_entries']]}),flush=True);saved()
   for label,grid in [('mixed',[(1,'white_concrete'),(2,'red_concrete'),(3,'white_concrete')]),('vertical',[(1,'blue_concrete'),(4,'blue_concrete'),(7,'blue_concrete')])]:
    close();command('clear McsaProbe');case=report['negative_cases'][label]={'snapshots':{}}
    groups={base:[place for place,item in grid if item==base] for _,base in grid}
    for base,places in groups.items():command('give McsaProbe '+base+' '+str(len(places)))
    open_at((2,64,0))
    for base,places in groups.items():
     source=next(i for i,v in enumerate(client.inventory['slots']) if i>=10 and v=={'item':base,'count':len(places)})
     click(source)
     for place in places:click(place,1)
     assert client.inventory['cursor'] is None
    snap(case,'ready');expected={place:{'item':base,'count':1} for place,base in grid};assert {i:v for i,v in enumerate(client.inventory['slots'][1:10],1) if v}==expected
    assert client.inventory['slots'][0] is None;before=copy.deepcopy(client.inventory);click(0);snap(case,'refused');assert client.inventory['slots']==before['slots'] and client.inventory['cursor'] is None;case['result']='PASS'
 except BaseException as error:report['error']=repr(error);report['traceback']=traceback.format_exc()
 finally:
  if client:report['packets']=client.observed;report['gamemode_events']=client.mode_events;client.sock.close()
  if server:
   if server.poll() is None:
    try:server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=25)
    except BaseException:
     os.killpg(server.pid,signal.SIGTERM)
     try:server.wait(timeout=5)
     except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=5)
   report['server_exit']=server.returncode
   try:os.killpg(server.pid,0);report['group_gone']=False
   except ProcessLookupError:report['group_gone']=True
  with socket.socket() as check:report['port_free']=check.connect_ex(('127.0.0.1',port))!=0
  report['result']='PASS' if len(report['colors'])==16 and all(len(c['checks'])==5 for c in report['colors'].values()) and len(report['negative_cases'])==2 and all(c.get('result')=='PASS' for c in report['negative_cases'].values()) and not report.get('error') and report.get('server_exit')==0 and report.get('group_gone') and report['port_free'] else 'FAIL'
  report['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();(out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k in ['result','error','server_exit','group_gone','port_free']}));sys.exit(0 if report['result']=='PASS' else 1)

if __name__=='__main__':main()
