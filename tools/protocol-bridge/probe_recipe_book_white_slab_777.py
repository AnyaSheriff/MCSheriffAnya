#!/usr/bin/env python3
"""Selected ordinary777 slab unlock/autofill; opaque JAR, no recipe dump."""
import argparse,sys,os,json,hashlib,subprocess,socket,time,struct,signal,copy,io,traceback
from pathlib import Path
from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,HASHES,pack_position
from probe_registry_client import read_string
from smoke import varint,read_varint

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--run',action='store_true')
 target=parser.add_mutually_exclusive_group(required=True)
 target.add_argument('--jar',type=Path)
 target.add_argument('--test-binary',type=Path)
 parser.add_argument('--binary-sha256')
 parser.add_argument('--data',type=Path,required=True)
 parser.add_argument('--output',type=Path,required=True)
 args=parser.parse_args()
 if not args.run:
  print(json.dumps({'plan':'selected slab unlock, named identity, ghost, once/all autofill, exact consumption','port':26044 if args.jar else 26026,'scope':'ordinary official777 raw packets; noGUI/noRust'}));return
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir()
 jar=args.jar.resolve() if args.jar else None;data=args.data
 if jar: assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 else: assert args.binary_sha256 and hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256
 for p,h in HASHES.items():assert hashlib.sha256((data/p).read_bytes()).hexdigest()==h
 protocol=json.loads((data/'protocol.json').read_text());names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())}
 SLAB=next(i for i,n in names.items() if n=='white_concrete_slab')
 def uint(d,cap=65536):
  n=read_varint(d.read);assert 0<=n<=cap,n;return n
 def boolean(d):
  b=d.read(1);assert b in [b'\0',b'\1'];return b==b'\1'
 def idset(d):
  n=uint(d)
  return {'tag':read_string(d)} if n==0 else {'items':[uint(d) for _ in range(n-1)]}
 class Probe(InventoryProbe):
  def __init__(self,*a):super().__init__(*a);self.stage='join';self.selected=[];self.ghosts=[]
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
   result=r['result'];return result.get('item')==SLAB or result.get('stack',{}).get('item')=='white_concrete_slab'
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload)
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
 report={'scope':'selected ordinary raw777 recipebook unlock/autofill, noGUI','jar_sha1':JAR_SHA1 if jar else None,'binary_sha256':args.binary_sha256,'snapshots':{},'commands':[],'requests':[],'checks':[]};server=None;client=None;port=26044 if jar else 26026;log=out/'server.log'
 with socket.socket() as s:s.bind(('127.0.0.1',port))
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 def command(s):report['commands'].append(s);server.stdin.write(s+'\n');server.stdin.flush()
 def snapshot(k):report['snapshots'][k]=copy.deepcopy(client.inventory)
 def request(all):
  ident=[r for r in client.selected if r['display']['type']==1][-1]['display_id'];window=client.inventory['window'];report['requests'].append({'window':window,'display_id':ident,'make_all':all,'stage':client.stage})
  client.send('craft_recipe_request',varint(window)+varint(ident)+bytes([all]));client.pump(.7)
 def click(slot):
  inv=client.inventory;inv['cursor']=None;client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',slot,0)+b'\0\0\0');client.pump(.4)
 try:
  with log.open('w') as output:
   if jar: argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui']
   else:
    for name in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']: (world/name).touch()
    argv=[str(args.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture']
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   until=time.monotonic()+100
   while ('Done (' if jar else 'MCSA gameplay probe ready') not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,p:Probe(s,p,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(.7)
   for s in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','setblock 2 64 0 crafting_table','clear McsaProbe','recipe take McsaProbe *']:command(s);client.pump(.3)
   client.stage='natural-one-concrete';command('give McsaProbe white_concrete 1');client.pump(1)
   crafts=[r for r in client.selected if r['display']['type']==1];assert len(crafts)==1,client.selected
   row=crafts[-1];assert row['display']['type']==1 and row['display']['width']==3 and row['display']['height']==1
   report['checks'].append('one-white-concrete-unlocks-selected-shaped-slab-display')
   client.stage='named-take-give';command('recipe take McsaProbe minecraft:white_concrete_slab');client.pump(.3);command('recipe give McsaProbe minecraft:white_concrete_slab');client.pump(.6)
   crafts=[r for r in client.selected if r['display']['type']==1];assert len(crafts)==2 and crafts[0]['display_id']==crafts[1]['display_id'];report['checks'].append('named-recipe-reaward-same-session-display-id')
   client.send('block_place',b'\0'+pack_position((2,64,0))+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(1));client.pump(.7);snapshot('opened-one-input')
   client.stage='insufficient-one';request(False);snapshot('insufficient-one')
   assert client.ghosts and client.inventory['slots'][37]=={'item':'white_concrete','count':1};assert all(client.inventory['slots'][i] is None for i in range(10));report['checks'].append('insufficient-input-produces-selected-ghost-without-loss')
   command('give McsaProbe white_concrete 8');client.pump(.6)
   client.stage='autofill-once';request(False);snapshot('autofill-once')
   slots=client.inventory['slots'];assert slots[0]=={'item':'white_concrete_slab','count':6};assert [i for i in range(1,10) if slots[i]]==[4,5,6];assert all(slots[i]=={'item':'white_concrete','count':1} for i in [4,5,6]);assert slots[37]=={'item':'white_concrete','count':6};report['checks'].append('ordinary-autofill-centers-row-one-each-with-six-left')
   client.stage='autofill-repeat';request(False);snapshot('autofill-repeat');slots=client.inventory['slots'];assert all(slots[i]=={'item':'white_concrete','count':2} for i in [4,5,6]) and slots[37]=={'item':'white_concrete','count':3};report['checks'].append('repeated-normal-autofill-increments-one-to-two')
   client.stage='autofill-all';request(True);snapshot('autofill-all');slots=client.inventory['slots'];assert all(slots[i]=={'item':'white_concrete','count':3} for i in [4,5,6]) and slots[37] is None;report['checks'].append('make-all-increments-two-to-three-from-nine-inputs')
   for take,label in [(1,'first-result'),(2,'second-result'),(3,'third-result')]:
    click(0);snapshot(label);assert client.inventory['cursor']=={'item':'white_concrete_slab','count':6*take}
    if take<3: assert all(client.inventory['slots'][i]=={'item':'white_concrete','count':3-take} for i in [4,5,6])
    else: assert all(client.inventory['slots'][i] is None for i in range(10))
   report['checks'].append('three-takes-consume-nine-and-produce-eighteen')
 except BaseException as e:report['error']=repr(e);report['traceback']=traceback.format_exc()
 finally:
  if client:report['selected_entries']=client.selected;report['selected_ghosts']=client.ghosts;report['packets']=client.observed;client.sock.close()
  if server:
   if server.poll() is None:
    try:command('stop');server.stdin.close();server.wait(timeout=25)
    except BaseException:
     os.killpg(server.pid,signal.SIGTERM)
     try:server.wait(timeout=5)
     except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=5)
   report['server_exit']=server.returncode
   try:os.killpg(server.pid,0);report['group_gone']=False
   except ProcessLookupError:report['group_gone']=True
  with socket.socket() as s:report['port_free']=s.connect_ex(('127.0.0.1',port))!=0
  report['result']='PASS' if len(report['checks'])==7 and not report.get('error') and report.get('server_exit')==0 and report.get('group_gone') and report['port_free'] else 'FAIL'
  report['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();(out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k in ['result','checks','error','server_exit','group_gone','port_free']}));sys.exit(0 if report['result']=='PASS' else 1)

if __name__=='__main__':main()
