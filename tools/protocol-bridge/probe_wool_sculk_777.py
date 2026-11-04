#!/usr/bin/env python3
"""Selected ordinary777 sculk discovery; no server unless --run, no fire claims."""
import argparse,hashlib,json,os,signal,socket,struct,subprocess,sys,time,traceback
from pathlib import Path
HELPERS=Path(__file__).resolve().parent
sys.path.insert(0,str(HELPERS))
from probe_gameplay import Client,connect
from probe_white_concrete_slab import JAR_SHA1,HASHES,changes,pack_position,FINISH_STATUS_777
from generate_tables import states
from smoke import varint
PORT=26050
SHAPES=[dict(label=half+'-'+str(wet).lower(),half=half,wet=wet) for half in ['bottom','top','double'] for wet in [False,True]]
TYPES=[dict(label='stone',block='stone'),dict(label='full-wool',block='white_wool')]+[dict(x,block='white_wool_slab') for x in SHAPES]
LINES=[dict(name='upper',source=[2,65,0],sensor=[6,66,0],barrier=[3,65,0],center_intersection_y=65.75),dict(name='lower',source=[2,64,0],sensor=[6,65,0],barrier=[5,65,0],center_intersection_y=65.25)]
class Inconclusive(Exception):pass
class Probe(Client):
 def __init__(self,*a):super().__init__(*a);self.events=[];self.tracked=set();self.mode=None;self.modes=[];self.hold=False;self.sequence=0;self.actions=[]
 def send(self,name,payload=b''):
  if name=='tick_end' and self.hold and self.position:super().send('position',struct.pack('>dddB',*self.position[:3],1))
  if name in ['block_place','block_dig']:self.actions.append(dict(name=name,payload_hex=payload.hex(),pose=self.position,at=time.monotonic()))
  super().send(name,payload)
 def observe_packet(self,name,payload):
  assert len(self.records)<120000 and len(self.events)<20000,'record bound'
  if name in ['block_change','multi_block_change']:
   for pos,state in changes(name,payload):
    if tuple(pos) in self.tracked:self.events.append(dict(name=name,position=pos,state=state,payload_hex=payload.hex(),at=time.monotonic()))
  if name=='game_state_change' and payload[:1]==b'\3':
   assert len(payload)==5;self.mode=struct.unpack('>f',payload[1:])[0];self.modes.append(dict(mode=self.mode,payload_hex=payload.hex()))
 def place(self,pos,face=1,cursor=(.5,.5,.5)):
  self.sequence+=1;self.send('block_place',b'\0'+pack_position(pos)+varint(face)+struct.pack('>fff',*cursor)+b'\0\0'+varint(self.sequence))
 def dig(self,pos,status):
  self.sequence+=1;self.send('block_dig',varint(status)+pack_position(pos)+b'\1'+varint(self.sequence))
def state_name(case):
 name=case['block']
 return name if 'half' not in case else name+'[type='+case['half']+',waterlogged='+str(case['wet']).lower()+']'
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--run',action='store_true');target=ap.add_mutually_exclusive_group();target.add_argument('--jar',type=Path);target.add_argument('--test-binary',type=Path);ap.add_argument('--binary-sha256');ap.add_argument('--data',type=Path,default=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/candidate-json'));ap.add_argument('--output',type=Path);a=ap.parse_args()
 if not a.jar and not a.test_binary:a.jar=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/server.jar')
 port=PORT if a.jar else 26026
 report=dict(scope='white wool selected emission16/occlusion20; discovery not calibration/fire/fullfamily',port=port,jar_sha1=JAR_SHA1 if a.jar else None,binary_sha256=a.binary_sha256,pins=HASHES,emission_types=TYPES,lines=LINES,cases=[],commands=[],limits=['no fire observation','normal sculk_sensor has no facing; no calibrated sensor','center-ray geometry is fixture design, actual event origin not independently measured','absence needs working controls; no exact timing parity','wet double placement records actual merge state, does not assume water retained'])
 if a.jar:assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==JAR_SHA1
 else:assert a.binary_sha256 and hashlib.sha256(a.test_binary.read_bytes()).hexdigest()==a.binary_sha256
 for name,h in HASHES.items():assert hashlib.sha256((a.data/name).read_bytes()).hexdigest()==h,name
 report['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();report['helper_sha256']={name:hashlib.sha256((HELPERS/name).read_bytes()).hexdigest() for name in ['probe_gameplay.py','probe_white_concrete_slab.py','generate_tables.py','smoke.py','observe.py']}
 if not a.run:print(json.dumps(dict(report,mode='plan-only',plan=['16 actual place/break source trials, no intervening wool','20 lever trials: 2 lines ×(9 barriers + no action)','mandatory stone/air/fullwool controls; quiet setup baseline','selected sensor phase/power and source actualstate only']),indent=2));return
 assert a.output,'--output required for --run';out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();(out/'runner.py').write_bytes(Path(__file__).read_bytes())
 blocks=json.loads((a.data/'blocks.json').read_text());protocol=json.loads((a.data/'protocol.json').read_text());lookup=states(blocks);props_by_id={value:dict(props) for (name,props),value in lookup.items() if name=='sculk_sensor'};sensor_ids=set(props_by_id)
 all_states={value:dict(name=name,properties=dict(props)) for (name,props),value in lookup.items() if name in ['stone','white_wool','white_wool_slab','air','water','lever','sculk_sensor']}
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=3\nsimulation-distance=3\ngamemode=survival\nspawn-protection=0\n')
 with socket.socket() as sock:sock.bind(('127.0.0.1',port))
 server=client=None;log=out/'server.log';deadline=time.monotonic()+650
 def save(): (out/'partial.json').write_text(json.dumps(report,indent=2)+'\n')
 def command(text,wait=.15):
  assert time.monotonic()<deadline,'global650s bound';report['commands'].append(dict(text=text,at=time.monotonic()));server.stdin.write(text+'\n');server.stdin.flush();client.pump(wait)
 def query(pos,spec,token):
  command('execute if block '+' '.join(map(str,pos))+' '+spec+' run say '+token)
  assert token in log.read_text(),('selected-state-query-not-confirmed',pos,spec,token)
 def active(events,sensor):
  rows=[dict(e,properties=props_by_id[e['state']]) for e in events if e['position']==sensor and e['state'] in sensor_ids and props_by_id[e['state']]['sculk_sensor_phase']=='active']
  assert all(int(e['properties']['power'])>0 for e in rows),'active sensor with zero power'
  return rows
 def setup(source,sensor,barrier=None,barrier_spec='air',source_spec='air'):
  client.tracked={tuple(source),tuple(sensor)}|({tuple(barrier)} if barrier else set())
  for text in ['fill 1 64 -2 9 68 2 air','fill -2 63 -2 10 63 2 stone','setblock 0 64 0 stone','kill @e[type=item]','clear McsaProbe','tp McsaProbe 0.5 65 0.5']:command(text)
  # Local supports: source floor/side, sensor floor. They are stone, never wool occluders.
  command('setblock '+' '.join(map(str,[source[0],source[1]-1,source[2]]))+' stone')
  if not source_spec.startswith('lever'):
   for neighbor in [[1,65,0],[3,65,0],[2,65,-1],[2,65,1]]:command('setblock '+' '.join(map(str,neighbor))+' stone')
  command('setblock '+' '.join(map(str,source))+' '+source_spec)
  if barrier:command('setblock '+' '.join(map(str,barrier))+' '+barrier_spec)
  command('setblock '+' '.join(map(str,[sensor[0],sensor[1]-1,sensor[2]]))+' stone')
  command('setblock '+' '.join(map(str,sensor))+' sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]')
  client.pump(3.5);query(sensor,'sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]','MCSA_READY_'+str(len(report['cases'])))
  query(source,source_spec,'MCSA_SOURCE_READY_'+str(len(report['cases'])))
  if barrier:query(barrier,barrier_spec,'MCSA_BARRIER_READY_'+str(len(report['cases'])))
  start=len(client.events);client.pump(.7)
  if active(client.events[start:],sensor):raise Inconclusive('background sensor activation during no-action baseline')
  assert client.mode==0,'Survival changed'
 def trial(label,source,sensor,action,initial,extra):
  start=len(client.events);action();client.pump(3.5);events=client.events[start:];activation=active(events,sensor)
  case=dict(label=label,source=source,sensor=sensor,initial=initial,**extra,selected_events=events,active=bool(activation),activations=activation,observation_seconds=5.5 if extra.get('action')=='break' else 3.5,outcome='active' if activation else 'no-activation-within-observation-window')
  command('data get block '+' '.join(map(str,sensor))+' last_vibration_frequency')
  case['final_source_updates']=[dict(e,decoded=all_states.get(e['state'])) for e in events if e['position']==source];report['cases'].append(case);save();print(json.dumps(dict(label=label,outcome=case['outcome'])),flush=True);return case
 try:
  with log.open('w') as output:
   if a.jar:argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(a.jar.resolve()),'nogui'];ready='Done ('
   else:
    for marker in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']:(world/marker).touch()
    argv=[str(a.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture'];ready='MCSA gameplay probe ready'
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   until=time.monotonic()+100
   while ready not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,Probe,777);client.pump(15,lambda r:r['name']=='position');client.pump(.3)
   for text in ['gamerule minecraft:spawn_mobs false','gamerule minecraft:random_tick_speed 0','weather clear','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(text)
   assert client.mode==0;client.hold=True;client.send('held_item_slot',struct.pack('>h',0));report['survival_confirmed_by_packet']=True
   source=[2,65,0];sensor=[6,65,0]
   for kind in TYPES:
    for action_name in ['place','break']:
     spec=state_name(kind)
     if action_name=='break':initial=spec
     elif kind.get('half')=='double':initial='white_wool_slab[type=bottom,waterlogged='+str(kind['wet']).lower()+']'
     else:initial='water' if kind.get('wet') else 'air'
     setup(source,sensor,source_spec=initial)
     if action_name=='place':
      command('item replace entity McsaProbe weapon.mainhand with '+kind['block']+' 1')
      # Reset after command/item setup; inventory changes are not the tested event.
      client.pump(.3);query(sensor,'sculk_sensor[power=0,sculk_sensor_phase=inactive]','MCSA_PREACTION_'+str(len(report['cases'])))
      def act():
       if kind.get('half')=='double':client.place(source,1,(.5,1.,.5))
       elif kind.get('half')=='top':client.place([3,65,0],4,(0.,.75,.5))
       else:client.place([2,64,0],1,(.5,1.,.5))
     else:
      command('item replace entity McsaProbe weapon.mainhand with diamond_pickaxe')
      def act():client.dig(source,0);client.pump(2.);client.dig(source,FINISH_STATUS_777)
     case=trial('emission/'+kind['label']+'/'+action_name,source,sensor,act,initial,dict(kind='emission',action=action_name,material=kind))
     updates=case['final_source_updates'];assert updates,'actual source action produced no selected state'
     final=updates[-1]['decoded'];assert final,('unrecognized actual state',updates)
     if action_name=='place':
      assert final['name']==kind['block'];assert final['properties'].get('type')==kind.get('half'),('wrong placed shape',case)
      if 'half' in kind:assert final['properties']['waterlogged']==str(kind['wet'] and kind['half']!='double').lower(),('wrong placed waterlogged',case)
     else:assert final['name'] in ['air','water'],('break-not-complete',case)
   for line in LINES:
    for kind in [dict(label='air',block='air')]+TYPES+[dict(label='no-event',block='air')]:
     source=line['source'];sensor=line['sensor'];spec=state_name(kind);setup(source,sensor,line['barrier'],spec,'lever[face=floor,facing=north,powered=false]')
     query(source,'lever[face=floor,facing=north,powered=false]','MCSA_LEVER_READY_'+str(len(report['cases'])))
     if kind['label']=='no-event':act=lambda:None
     else:act=lambda:client.place(source,1,(.5,.5,.5))
     case=trial('occlusion/'+line['name']+'/'+kind['label'],source,sensor,act,'lever[powered=false]',dict(kind='occlusion',barrier=kind,geometry=line,source_gameevent_origin='unmeasured; coordinates record block centers only'))
     if kind['label']!='no-event':
      query(source,'lever[powered=true]','MCSA_LEVER_USED_'+str(len(report['cases'])))
      assert case['final_source_updates'] and case['final_source_updates'][-1]['decoded']['properties']['powered']=='true','lever packet did not confirm action'
   controls=[]
   for case in report['cases']:
    expected=None
    if case['kind']=='emission' and case['material']['label']=='stone':expected=True
    if case['kind']=='emission' and case['material']['label']=='full-wool':expected=False
    if case['kind']=='occlusion' and case['barrier']['label'] in ['air','stone']:expected=True
    if case['kind']=='occlusion' and case['barrier']['label'] in ['full-wool','no-event']:expected=False
    if expected is not None:controls.append(dict(label=case['label'],expected_active=expected,observed_active=case['active'],matched=expected==case['active']))
   report['controls']=controls
   if not all(x['matched'] for x in controls):raise Inconclusive('discriminating controls did not match; do not infer slab quiet/shape behavior')
   assert len(report['cases'])==36;report['result']='OBSERVED'
 except Inconclusive as error:report.update(result='INCONCLUSIVE',error=str(error),traceback=traceback.format_exc())
 except BaseException as error:report.update(result='FAIL',error=repr(error),traceback=traceback.format_exc())
 finally:
  if client:report['mode_events']=client.modes;report['actions']=client.actions;client.sock.close()
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
  with socket.socket() as sock:report['port_free']=sock.connect_ex(('127.0.0.1',port))!=0
  if not(report.get('server_exit')==0 and report.get('group_gone') and report['port_free']):report['result']='FAIL'
  (out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report.get(k) for k in ['result','error','server_exit','group_gone','port_free']}))
 if report['result']!='OBSERVED':sys.exit(1)
if __name__=='__main__':main()
