#!/usr/bin/env python3
"""Один ordinary777 автокрафтер: свои импульсы, слоты и выходной сундук."""
import argparse,io,sys,os,json,hashlib,subprocess,socket,time,struct,signal,copy,traceback
from pathlib import Path
from probe_container_session import Probe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,HASHES,pack_position,item_metadata,signed
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
  print(json.dumps({'plan':'own crafter without receiver; unique item stack six; selected success/empty/invalid world events','port':26046 if args.jar else 26026,'scope':'raw777,noGUI,own selected slots only'}));return
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir()
 jar=args.jar.resolve() if args.jar else None;data=args.data
 if jar: assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 else: assert args.binary_sha256 and hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256
 for p,h in HASHES.items():assert hashlib.sha256((data/p).read_bytes()).hexdigest()==h
 protocol=json.loads((data/'protocol.json').read_text());names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())}
 report={'scope':'selected raw777 crafter spill and failure events, unique item metadata, noGUI','jar_sha1':JAR_SHA1 if jar else None,'binary_sha256':args.binary_sha256,'snapshots':{},'world_events':[],'item_drops':[],'commands':[],'actions':[],'checks':[]};server=None;client=None;port=26046 if jar else 26026;log=out/'server.log';sequence=0
 entity_kind=next(v['id'] for v in json.loads((data/'entities.json').read_text()) if v['name']=='item')
 item_ids=set();began=time.monotonic()
 class ObservedProbe(Probe):
  def observe_packet(self,name,payload):
   if name=='world_event':
    assert len(payload)==17
    effect,packed,eventdata,global_=struct.unpack('>iQi?',payload)
    at=[signed(packed>>38,26),signed(packed&4095,12),signed((packed>>12)&0x3ffffff,26)]
    if at==[2,64,0]:
     assert len(report['world_events'])<16
     report['world_events'].append(dict(effect_id=effect,position=at,data=eventdata,**{'global':global_},elapsed_seconds=round(time.monotonic()-began,6),payload_hex=payload.hex()))
   elif name=='spawn_entity':
    d=io.BytesIO(payload);entity=read_varint(d.read);d.read(16);kind=read_varint(d.read)
    at=list(struct.unpack('>ddd',d.read(24)))
    if kind==entity_kind and all(abs(v-e)<4 for v,e in zip(at,[2,64,0])): item_ids.add(entity)
   elif name=='entity_metadata':
    d=io.BytesIO(payload);entity=read_varint(d.read)
    if entity in item_ids:
     entity,value=item_metadata(payload)
     if value is not None:
      assert len(report['item_drops'])<16
      report['item_drops'].append({'entity':entity,'slot':value,'elapsed_seconds':round(time.monotonic()-began,6)})
   return super().observe_packet(name,payload)
 with socket.socket() as s:s.bind(('127.0.0.1',port))
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 def command(s):report['commands'].append(s);server.stdin.write(s+'\n');server.stdin.flush()
 def snapshot(k):report['snapshots'][k]=copy.deepcopy(client.inventory)
 def close():
  if client.inventory and client.inventory['window']:
   client.send('close_window',varint(client.inventory['window']));client.pump(.2)
 def open_at(at):
  nonlocal sequence
  close();sequence+=1;report['actions'].append({'action':'open','at':at,'sequence':sequence})
  client.send('block_place',b'\0'+pack_position(at)+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(sequence));client.pump(.5)
 def pulse(label):
  report['actions'].append({'action':'pulse','label':label,'power_at':[3,64,0]});command('setblock 3 64 0 redstone_block');client.pump(.8);command('setblock 3 64 0 air');client.pump(.3)
 def click(slot):
  inv=client.inventory;inv['cursor']=None;report['actions'].append({'action':'click','window':inv['window'],'slot':slot})
  client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',slot,0)+b'\0\0\0');client.pump(.4)
 def crafter(label,pattern,count):
  snapshot(label);inv=client.inventory;assert len(inv['slots'])==46,('actual-crafter-slots',len(inv['slots']))
  expected=[{'item':'white_concrete','count':count} if i in pattern and count else None for i in range(9)]
  assert inv['slots'][:9]==expected,(label,inv)
  expected_result={'item':'white_concrete_slab','count':6} if count and pattern==[0,1,2] else None
  assert inv['slots'][45]==expected_result,(label,'preview',inv)
 try:
  with log.open('w') as output:
   if jar: argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui']
   else:
    for name in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']: (world/name).touch()
    argv=[str(args.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture']
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   until=time.monotonic()+100
   while ('Done (' if jar else 'MCSA gameplay probe ready') not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,p:ObservedProbe(s,p,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(.7)
   for text in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','setblock 2 64 -1 air','setblock 2 64 0 crafter[orientation=north_up,triggered=false,crafting=false]','clear McsaProbe']:
    command(text);client.pump(.3)
   for slot in [0,1,2]:command(f'item replace block 2 64 0 container.{slot} with white_concrete 1');client.pump(.2)
   open_at((2,64,0));crafter('before-spill',[0,1,2],1);report['checks'].append('own-crafter-row-one-each-preview-six-without-receiver')
   pulse('no-receiver');crafter('after-spill',[],0)
   matching=[r for r in report['item_drops'] if r['slot']['item']==next(i for i,n in names.items() if n=='white_concrete_slab')]
   by_entity={r['entity']:r['slot'] for r in matching}
   assert list(by_entity.values())==[{'item':next(i for i,n in names.items() if n=='white_concrete_slab'),'count':6}],report['item_drops']
   assert report['world_events'],report
   report['checks'].append('actual-pulse-consumes-inputs-and-spawns-one-stack-six-with-selected-events')
   pulse('empty-after-spill');crafter('empty-after-spill',[],0)
   assert [(e['effect_id'],e['data']) for e in report['world_events']]==[(1049,0),(2010,2),(1050,0)],report['world_events']
   report['checks'].append('empty-crafter-emits-selected-failure-event-without-new-output')
   for slot in [0,3,6]:command(f'item replace block 2 64 0 container.{slot} with white_concrete 1');client.pump(.2)
   crafter('invalid-before',[0,3,6],1);pulse('invalid-vertical');crafter('invalid-after',[0,3,6],1)
   assert [(e['effect_id'],e['data']) for e in report['world_events']]==[(1049,0),(2010,2),(1050,0),(1050,0)],report['world_events']
   assert len({r['entity'] for r in report['item_drops']})==1,report['item_drops']
   report['checks'].append('invalid-crafter-preserves-input-and-emits-selected-failure-without-new-output')
 except BaseException as e:report['error']=repr(e);report['traceback']=traceback.format_exc()
 finally:
  if client:report['packets']=client.observed;client.sock.close()
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
  report['result']='PASS' if len(report['checks'])==4 and not report.get('error') and report.get('server_exit')==0 and report.get('group_gone') and report['port_free'] else 'FAIL'
  report['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();(out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k in ['result','checks','error','server_exit','group_gone','port_free']}));sys.exit(0 if report['result']=='PASS' else 1)

if __name__=='__main__':main()
