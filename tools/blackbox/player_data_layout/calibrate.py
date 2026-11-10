#!/usr/bin/env python3
"""Один ORIGINAL-only finite command layout capture; никакого core/Cargo."""
import tarfile
import argparse,hashlib,io,json,os,shutil,socket,struct,subprocess,sys,time,traceback
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/blackbox'))
import record777 as own
import combat_ci
import signal
from rcon_transport import IsolatedRcon
from combat_process import stop_process,ports_free,process_identity
from slots import SlotDecoder
from probe_player_actions_runtime import menu_slot
BASE=Path(__file__).parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def read(p):return json.loads(p.read_text())
def write(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
STATES=[
 dict(id='empty',selected=0,commands=['clear blackbox']),
 dict(id='plain',selected=0,commands=['clear blackbox','item replace entity blackbox weapon.mainhand with minecraft:iron_ingot 3','item replace entity blackbox weapon.offhand with minecraft:gold_ingot 5']),
 dict(id='complex',selected=2,commands=['clear blackbox','item replace entity blackbox hotbar.2 with minecraft:crossbow[damage=7,enchantments={unbreaking:3},custom_data={mcsa_layout:1,marker:"LAYOUT_FIXED"}]','item replace entity blackbox weapon.offhand with minecraft:gold_ingot 5'])]
QUERIES=['data get entity blackbox','data get entity blackbox Inventory','data get entity blackbox SelectedItem','data get entity blackbox equipment','data get entity blackbox equipment.offhand','data get entity blackbox MCSA_MISSING_LAYOUT_FIELD','data get entity blackbox Pos','data get entity blackbox OnGround','data get entity blackbox Inventory[0]','data get entity blackbox SelectedItem.count','data get entity blackbox SelectedItem.count 2','data get entity @a Pos']
def preflight(output):
 assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('GITHUB_RUN_ATTEMPT')=='1'
 assert not output.exists()
 pin=read(BASE/'pins.json')
 for name,h in pin['source_files'].items():assert sha(ROOT/name)==h
 assert sha(BASE/'criteria.json')==pin['criteria_sha256']
 return pin
class RawClient(own.Recorder):
 def __init__(self,sock,protocol):
  super().__init__(sock,protocol);self.recording=True
def inventory_proof(events,items):
 parser=SlotDecoder(items)
 slots={};seed=False;receipts=[]
 for e in events:
  b=io.BytesIO(bytes.fromhex(e['hex']));name=e['name']
  if name=='window_items':
   window=own.read_varint(b.read);revision=own.read_varint(b.read);n=own.read_varint(b.read);values=[parser.slot(b) for _ in range(n)];cursor=parser.slot(b);assert not b.read()
   if window==0:assert n==46;slots=dict(enumerate(values));seed=True;receipts.append(dict(at=e['at'],kind='full46',revision=revision,cursor=cursor))
  elif name=='set_player_inventory':
   slot=menu_slot(own.read_varint(b.read));value=parser.slot(b);assert not b.read()
   if seed:slots[slot]=value;receipts.append(dict(at=e['at'],kind='slot',slot=slot,value=value))
  elif name=='set_slot':
   window=own.read_varint(b.read);revision=own.read_varint(b.read);slot=struct.unpack('>h',b.read(2))[0];value=parser.slot(b);assert not b.read()
   if window==0 and seed:slots[slot]=value;receipts.append(dict(at=e['at'],kind='slot',slot=slot,value=value))
 assert seed and len(slots)==46,'missing actual full46 inventory seed'
 return dict(slots=[slots[i] for i in range(46)],receipts=receipts)
def equipment(events,items):
 parser=SlotDecoder(items);out=[]
 for e in events:
  if e['name']!='entity_equipment':continue
  b=io.BytesIO(bytes.fromhex(e['hex']));entity=own.read_varint(b.read);slots=[]
  while True:
   tag=b.read(1);assert tag;slot=tag[0];slots.append(dict(slot=slot&127,item=parser.slot(b)))
   if not slot&128:break
  assert not b.read();out.append(dict(entity=entity,slots=slots,at=e['at'],hex=e['hex']))
 return out
def barrier(clients,rcon,token):
 assert token.isascii() and token.replace('_','').isalnum()
 issued=time.monotonic();response=rcon.run('say '+token);deadline=time.monotonic()+5
 while time.monotonic()<deadline:
  found=[[e for e in c.events if e['at']>=issued and e['name'] in ['system_chat','profileless_chat'] and token.encode() in bytes.fromhex(e['hex'])] for c in clients]
  if all(found):return dict(token=token,issued=issued,response=response,receipts=[x[0] for x in found])
  assert all(c.failure is None for c in clients);time.sleep(.01)
 raise AssertionError('both actual barrier receipts required')
def enchantments(configuration,protocol):
 from analyze777 import Decoder
 d=Decoder(protocol);d.types.update(protocol['configuration']['toClient']['types'])
 mapping=protocol['configuration']['toClient']['types']['packet'][1][0]['type'][1]['mappings']
 for packet in configuration:
  if (mapping.get(hex(packet['id'])) or mapping.get(f"0x{packet['id']:02x}"))=='registry_data':
   b=io.BytesIO(bytes.fromhex(packet['hex']));row=d.read(b,'packet_registry_data');assert not b.read()
   if row['id']=='minecraft:enchantment':return [entry['key'] for entry in row['entries']]
 raise AssertionError('missing original enchantment registry')
def assert_unbreaking_three(stack,names):
 assert stack.get('enchantments')==[[names.index('minecraft:unbreaking'),3]],'exact unbreakingIII required'
def finalize_manifest(output):
 write(output/'files.json',{str(p.relative_to(output)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in output.rglob('*') if p.is_file() and p!=output/'files.json'})
def assert_ready(control,done,status):
 assert control is not None and done and status['version']['protocol']==777 and '26.3' in status['version']['name'],'original startup not ready'
def status_query(port):
 with socket.create_connection(('127.0.0.1',port),timeout=3) as sock:
  body=own.varint(0)+own.varint(777)+own.varint(9)+b'localhost'+struct.pack('>H',port)+own.varint(1)
  sock.sendall(own.varint(len(body))+body+b'\x01\x00')
  f=sock.makefile('rb');size=own.read_varint(f.read);assert 0<size<2*1024*1024
  b=io.BytesIO(f.read(size));assert own.read_varint(b.read)==0
  n=own.read_varint(b.read);data=b.read(n);assert len(data)==n and not b.read()
  return json.loads(data.decode('utf8','strict'))
def semantic_startup(candidate,done,status_reader):
 # После actual response никаких повторов: contradiction/frame failure завершают capture.
 assert done
 candidate.run('list')
 status=status_reader();assert_ready(candidate,done,status);return status
def validate_setup_reply(command,response):
 expected={
  'gamerule spawn_mobs false':'Game rule spawn_mobs is now set to false',
  'gamerule natural_health_regeneration false':'Game rule natural_health_regeneration is now set to false',
 }
 if command in expected:assert response==expected[command],'setup gamerule rejected'
 else:assert response and not response.startswith(('Unknown','Incorrect','Invalid')),'setup failed'
class CleanupAdapter:
 def __init__(self,control,errors):self.control=control;self.errors=errors
 def run(self,text):
  assert text=='stop'
  try:return self.control.stop_only()
  except (AssertionError,EOFError,OSError) as error:self.errors.append(repr(error));return ''
def imported_own_files():
 result=[]
 for module in list(sys.modules.values()):
  f=getattr(module,'__file__',None)
  if f:
   path=Path(f).resolve()
   if path.suffix=='.py' and path.is_relative_to(ROOT):result.append(str(path.relative_to(ROOT)))
 return sorted(set(result))
def capture(output):
 pin=preflight(output);signal.signal(signal.SIGALRM,lambda *_: (_ for _ in ()).throw(TimeoutError('finite capture deadline')));signal.alarm(300);output.mkdir();work=output/'original';work.mkdir();clients=[];configurations=[];startup_attempts=[];cleanup_control=None;proc=None;control=None;log=None
 result=dict(status='INVALID',source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),original_only=True,core_executed=False,compiled=False);commands=[];states=[];life={}
 try:
  for port in [26230,26231]:
   with socket.socket() as check:check.bind(('127.0.0.1',port))
  source=read(ROOT/'tools/blackbox/ci/protection-edges-777.json');data=output/'data';data.mkdir()
  combat_ci.fetch(source['official_jar'],work/'server.jar')
  for e in source['json_files']:
   if e['file'] in ['protocol.json','items.json','entities.json','blocks.json']:combat_ci.fetch(e,data/e['file'])
  (work/'eula.txt').write_text('eula=true\n')
  (work/'server.properties').write_text('server-ip=127.0.0.1\nserver-port=26230\nenable-rcon=true\nrcon.port=26231\nrcon.password=local-layout-fixture\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\ngenerate-structures=false\nspawn-protection=0\nallow-flight=true\nview-distance=2\nsimulation-distance=2\nmax-tick-time=0\n')
  log=(work/'server.log').open('w');argv=['java','-Xmx1G','-XX:ActiveProcessorCount=2','-jar','server.jar','nogui']
  proc=subprocess.Popen(argv,cwd=work,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);life['identity']=process_identity(proc.pid);life['argv']=argv
  deadline=time.monotonic()+120;startup_status=None;startup_done=False
  while time.monotonic()<deadline:
   assert proc.poll() is None
   startup_done='Done (' in (work/'server.log').read_text()
   if startup_done:break
   time.sleep(.1)
  assert startup_done,'Done readiness deadline'
  candidate=IsolatedRcon(26231,'local-layout-fixture');cleanup_control=candidate
  try:
   startup_status=semantic_startup(candidate,startup_done,lambda:status_query(26230))
  except BaseException as error:
   startup_attempts.append(dict(error=repr(error),calls=candidate.calls));raise
  control=candidate
  write(output/'startup-status.json',startup_status)
  protocol=read(data/'protocol.json');items=read(data/'items.json')
  for name in ['blackbox','blackboxb']:
   configuration=[]
   def receive_configuration(sock,packet_id,payload):
    configuration.append(dict(id=packet_id,hex=payload.hex()));return False
   c=own.connect(26230,protocol,lambda s,p:RawClient(s,p),version=777,username=name,configuration_handler=receive_configuration);c.start();clients.append(c);configurations.append(configuration)
  nonce='RUN'+os.environ['GITHUB_RUN_ID']
  def command(text):
   row=dict(command=text,issued=time.monotonic());row['response']=control.run(text);row['completed']=time.monotonic();commands.append(row);return row
  for text in ['gamerule natural_health_regeneration false','gamerule spawn_mobs false','gamemode creative blackbox','gamemode creative blackboxb','tp blackbox 0.5 65 0.5','tp blackboxb 2.5 65 0.5']:validate_setup_reply(text,command(text)['response'])
  for state in STATES:
   clients[0].send('held_item_slot',struct.pack('>h',state['selected']))
   for text in state['commands']:command(text)
   begin=barrier(clients,control,nonce+'_'+state['id'].upper()+'_READY')
   readiness_deadline=time.monotonic()+3
   while True:
    before=inventory_proof(clients[0].events,items)
    observed={}
    for packet in equipment(clients[1].events,items):
     if packet['entity']==clients[0].entity:
      for item in packet['slots']:observed[item['slot']]=item['item']
    if state['id']=='empty' or (observed.get(0)==before['slots'][36+state['selected']] and observed.get(1)==before['slots'][45]):break
    assert time.monotonic()<readiness_deadline,'missing positive observer equipment fixture'
    time.sleep(.01)
   names={x['name']:x['id'] for x in items}
   slots=before['slots'];selected=36+state['selected']
   if state['id']=='empty':assert slots[selected] is None and slots[45] is None
   elif state['id']=='plain':
    assert slots[selected]['item']=='iron_ingot' and slots[selected]['count']==3 and slots[45]['item']=='gold_ingot' and slots[45]['count']==5
   else:
    assert slots[selected]['item']=='crossbow' and slots[selected]['count']==1 and slots[selected].get('damage')==7 and slots[selected].get('enchantments')
    assert_unbreaking_three(slots[selected],enchantments(configurations[0],protocol))
    assert slots[45]['item']=='gold_ingot' and slots[45]['count']==5
    assert slots[selected]['custom_data']=={'mcsa_layout':{'type':3,'value':1},'marker':{'type':8,'value':'LAYOUT_FIXED'}}
   replies=[command(q) for q in QUERIES]
   end=barrier(clients,control,nonce+'_'+state['id'].upper()+'_END')
   states.append(dict(id=state['id'],selected=state['selected'],begin=begin,end=end,before=before,after=inventory_proof(clients[0].events,items),queries=replies,equipment=[equipment(c.events,items) for c in clients]))
  result.update(status='COMPLETE-layout-observation-not-parity',states=states)
 except BaseException:
  (output/'error.txt').write_text(traceback.format_exc());raise
 finally:
  signal.alarm(0)
  write(output/'commands.json',commands)
  write(output/'startup-attempts.json',startup_attempts)
  write(output/'configuration.json',configurations)
  write(output/'pins.json',pin)
  shutil.copy2(BASE/'criteria.json',output/'criteria.json')
  write(output/'raw-clients.json',[dict(entity=c.entity,events=c.events,wire_sent=c.wire_sent,failure=c.failure) for c in clients])
  for c in clients:c.close()
  if proc is not None:
   life['cleanup_errors']=[];life['cleanup']=stop_process(proc,CleanupAdapter(cleanup_control,life['cleanup_errors']) if cleanup_control else None,identity=life['identity'])
  write(output/'raw-rcon-frames.json',cleanup_control.calls if cleanup_control else [])
  used=imported_own_files();missing=[n for n in used if n not in pin['source_files']]
  if missing:result.update(status='INVALID',unfrozen_imports=missing)
  write(output/'own-import-closure.json',used)
  if log:log.close()
  life['ports_free']=ports_free([26230,26231])
  if not all(life['ports_free'].values()) or (proc is not None and (life['cleanup']['exit_code']!=0 or not life['cleanup']['group_gone'])):result.update(status='INVALID',cleanup_failed=True)
  with tarfile.open(output/'own-source.tar.gz','w:gz') as archive:
   for n in sorted(set([*pin['source_files'],'tools/blackbox/player_data_layout/criteria.json','tools/blackbox/player_data_layout/pins.json','.github/workflows/player-data-layout-original.yml'])):archive.add(ROOT/n,arcname=n)
  write(output/'lifecycle.json',life);write(output/'result.json',result)
  finalize_manifest(output)
 if result['status'].startswith('COMPLETE'):
  assert all(life['ports_free'].values()) and life['cleanup']['group_gone']
  first=output/'FIRST-ORIGINAL';first.mkdir(exist_ok=False)
  for n in ['result.json','commands.json','raw-clients.json','lifecycle.json','raw-rcon-frames.json','configuration.json','criteria.json','pins.json','own-source.tar.gz']:shutil.copy2(output/n,first/n)
  finalize_manifest(output)
 else:raise RuntimeError('technical INVALID retained; stop without retry')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();capture(a.output.resolve())
