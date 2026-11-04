#!/usr/bin/env python3
"""One selected ordinary white_concrete stonecut, opaque777, no table dump."""
import argparse,sys,os,json,hashlib,subprocess,socket,time,struct,signal,copy,io,traceback
from pathlib import Path
from probe_container_session import Probe as InventoryProbe
from probe_stonecutter_776 import selected_display
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,HASHES,pack_position
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
  print(json.dumps({'plan':'selected stonecut entry, exact named book entry, derived button, ordinary and shift crafting, invalid selections','port':26045 if args.jar else 26026,'scope':'raw777,notGUI,no recipe dump'}));return
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir()
 jar=args.jar.resolve() if args.jar else None;data=args.data
 if jar: assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 else: assert args.binary_sha256 and hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256
 for p,h in HASHES.items():assert hashlib.sha256((data/p).read_bytes()).hexdigest()==h
 protocol=json.loads((data/'protocol.json').read_text());names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())};items={n:i for i,n in names.items()};INPUT=items['white_concrete'];OUTPUT=items['white_concrete_slab']
 report={'scope':'selected ordinary raw777 stonecutter operation, noGUI','jar_sha1':JAR_SHA1 if jar else None,'binary_sha256':args.binary_sha256,'snapshots':{},'commands':[],'actions':[],'checks':[]};server=None;client=None;port=26045 if jar else 26026;log=out/'server.log'
 def uint(d,cap=65536):
  n=read_varint(d.read);assert 0<=n<=cap;return n
 def boolean(d):
  b=d.read(1);assert b in [b'\0',b'\1'];return b==b'\1'
 def ids(d):
  n=uint(d);assert n>0;return [uint(d) for _ in range(n-1)]
 class Probe(InventoryProbe):
  def __init__(self,*a):super().__init__(*a);self.stage='join';self.book=[];self.properties=[]
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload);d=io.BytesIO(payload)
   if name=='declare_recipes':
    report['selected_display']=selected_display(payload,INPUT,OUTPUT)
    report['selected_display']['scope']='only selected white_concrete→white_concrete_slab; unrelated prefix discarded'
    report['selected_display']['button_ordinal_for_input']=report['selected_display'].pop('button_ordinal_for_stone')
   elif name=='recipe_book_add' and self.stage=='named-reaward':
    # После точного recipegive ожидается один выбранный stonecut display.
    count=uint(d,8);assert count==1
    ident=uint(d);kind=uint(d);assert kind==3
    assert uint(d)==6;ingredients=ids(d)
    assert uint(d)==5;result=self.slot(d,template=True)
    assert uint(d)==4;station=uint(d)
    group=uint(d);category=uint(d);req=[ids(d) for _ in range(uint(d,8))] if boolean(d) else None
    flags=d.read(1)[0];replace=boolean(d);assert not d.read()
    self.book.append({'display_id':ident,'type':kind,'input_ids':ingredients,'result':result,'station_item':station,'group_raw':group,'category':category,'requirements':req,'flags':flags,'replace':replace})
   elif name=='craft_progress_bar':
    window=uint(d);prop,value=struct.unpack('>hh',d.read(4));assert not d.read();self.properties.append({'window':window,'property':prop,'value':value,'stage':self.stage})
 with socket.socket() as s:s.bind(('127.0.0.1',port))
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 def command(s):report['commands'].append(s);server.stdin.write(s+'\n');server.stdin.flush()
 def snapshot(k):report['snapshots'][k]=copy.deepcopy(client.inventory)
 def click(place,mode=0):
  inv=client.inventory;inv['cursor']=None;report['actions'].append({'action':'window_click','window':inv['window'],'slot':place,'mode':mode})
  client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',place,0)+varint(mode)+b'\0\0');client.pump(.4)
 def select(button):
  report['actions'].append({'action':'select','window':client.inventory['window'],'button':button});client.send('enchant_item',varint(client.inventory['window'])+varint(button));client.pump(.4)
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
   chosen=report['selected_display'];assert chosen['decoded_template']=={'item':OUTPUT,'count':2,'added':0,'removed':0};report['checks'].append('selected-update-recipes-template-is-two-slabs')
   for s in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','setblock 2 64 0 stonecutter','clear McsaProbe','give McsaProbe white_concrete 3']:command(s);client.pump(.3)
   candidate='minecraft:white_concrete_slab_from_white_concrete_stonecutting';command('recipe take McsaProbe '+candidate);client.pump(.3);client.stage='named-reaward';command('recipe give McsaProbe '+candidate);client.pump(.6);client.stage='menu'
   assert len(client.book)==1 and client.book[0]['result']=={'item':'white_concrete_slab','count':2};report['checks'].append('named-stonecut-recipe-accepted-and-selected-book-entry-reawarded')
   client.send('block_place',b'\0'+pack_position((2,64,0))+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(1));client.pump(.6);snapshot('opened')
   inv=client.inventory;assert len(inv['slots'])==38;places=[i for i,v in enumerate(inv['slots']) if i>=2 and v=={'item':'white_concrete','count':3}];assert len(places)==1;source=places[0]
   click(source);click(0);snapshot('input-three');assert client.inventory['slots'][0]=={'item':'white_concrete','count':3} and client.inventory['cursor'] is None
   before=copy.deepcopy(client.inventory);select(-1);snapshot('invalid-before-selection');assert client.inventory['slots']==before['slots'] and client.inventory['cursor']==before['cursor'];report['checks'].append('invalid-negative-selection-preserves-input-and-empty-output')
   client.stage='selected';select(chosen['button_ordinal_for_input']);snapshot('selected');assert client.inventory['slots'][1]=={'item':'white_concrete_slab','count':2};assert client.inventory['slots'][0]=={'item':'white_concrete','count':3};report['checks'].append('derived-valid-button-shows-two-without-consuming-input')
   before=copy.deepcopy(client.inventory)
   for invalid in [-1,65536]:select(invalid)
   snapshot('invalid-after-selection');assert client.inventory['slots']==before['slots'] and client.inventory['cursor']==before['cursor'];report['checks'].append('invalid-negative-and-large-buttons-preserve-selection-and-counts')
   client.stage='ordinary-take';click(1);snapshot('taken-once');assert client.inventory['cursor']=={'item':'white_concrete_slab','count':2} and client.inventory['slots'][0]=={'item':'white_concrete','count':2};report['checks'].append('ordinary-result-consumes-one-and-gives-two')
   click(source);snapshot('stored-first-two');assert client.inventory['cursor'] is None
   client.stage='shift-take';click(1,1);snapshot('shift-rest');inv=client.inventory;assert inv['slots'][0] is None and inv['slots'][1] is None and inv['cursor'] is None;assert sum(v['count'] for v in inv['slots'][2:] if v and v['item']=='white_concrete_slab')==6;assert not any(v and v['item']=='white_concrete' for v in inv['slots']);report['checks'].append('shift-consumes-remaining-two-and-total-output-is-six')
 except BaseException as e:report['error']=repr(e);report['traceback']=traceback.format_exc()
 finally:
  if client:report['selected_book_entries']=client.book;report['properties']=client.properties;report['packets']=client.observed;client.sock.close()
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
