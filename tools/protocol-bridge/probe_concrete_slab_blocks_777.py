#!/usr/bin/env python3
"""Selected sixteen-color slab states, ordinary placement and pickaxe drops777."""
import argparse,sys,os,json,hashlib,subprocess,socket,time,signal,struct,traceback
from pathlib import Path
from probe_gameplay import connect
from probe_white_concrete_slab import Probe,JAR_SHA1,HASHES,TARGET,FINISH_STATUS_777
from generate_tables import states

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--run',action='store_true');t=p.add_mutually_exclusive_group(required=True);t.add_argument('--jar',type=Path);t.add_argument('--test-binary',type=Path);p.add_argument('--binary-sha256');p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 if not a.run:print(json.dumps({'plan':'six explicit states + real lower placement + pickaxe mining for16 colors','port':26047 if a.jar else 26026}));return
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();jar=a.jar.resolve() if a.jar else None
 if jar:assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 else:assert a.binary_sha256 and hashlib.sha256(a.test_binary.read_bytes()).hexdigest()==a.binary_sha256
 for name,h in HASHES.items():assert hashlib.sha256((a.data/name).read_bytes()).hexdigest()==h
 protocol=json.loads((a.data/'protocol.json').read_text());items={x['name']:x['id'] for x in json.loads((a.data/'items.json').read_text())};blocks={x['name']:x for x in json.loads((a.data/'blocks.json').read_text())};entity_kind=next(x['id'] for x in json.loads((a.data/'entities.json').read_text()) if x['name']=='item')
 colors=['white','orange','magenta','light_blue','yellow','lime','pink','gray','light_gray','cyan','purple','blue','brown','green','red','black'];report={'scope':'selected family block states/place/mine raw777,noGUI','jar_sha1':JAR_SHA1 if jar else None,'binary_sha256':a.binary_sha256,'colors':{},'commands':[]};port=26047 if jar else 26026;server=None;client=None;log=out/'server.log'
 with socket.socket() as s:s.bind(('127.0.0.1',port))
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 class SurvivalProbe(Probe):
  def __init__(self,*args):super().__init__(*args);self.game_mode=None;self.mode_events=[]
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload)
   if name=='game_state_change' and payload and payload[0]==3:
    assert len(payload)==5
    mode=struct.unpack('>f',payload[1:])[0];assert mode in [0.,1.,2.,3.]
    self.game_mode=int(mode);self.mode_events.append({'mode':int(mode),'payload_hex':payload.hex()})
 def command(text,wait=.18):report['commands'].append(text);server.stdin.write(text+'\n');server.stdin.flush();client.pump(wait)
 try:
  with log.open('w') as output:
   if jar:argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui']
   else:
    for marker in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']:(world/marker).touch()
    argv=[str(a.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture']
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   until=time.monotonic()+100
   while ('Done (' if jar else 'MCSA gameplay probe ready') not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,p:SurvivalProbe(s,p,entity_kind),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
   for text in ['gamerule minecraft:spawn_mobs false','gamerule minecraft:random_tick_speed 0','fill -4 64 -4 5 64 4 stone','fill -10 65 -10 12 68 10 air','tp McsaProbe 0.5 65 0.5','clear McsaProbe','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(text)
   assert client.game_mode==0,('survival-not-confirmed',client.mode_events)
   report['survival_confirmed_by_packet']=True
   client.grounded_fixture=True;client.send('held_item_slot',struct.pack('>h',0))
   for color in colors:
    slab=color+'_concrete_slab';lookup={dict(props)['type']+'/'+dict(props)['waterlogged']:value for (name,props),value in states([blocks[slab]]).items()};case=report['colors'][color]={'name':slab,'states':{},'checks':[]}
    for properties,value in lookup.items():
     half,wet=properties.split('/');start=len(client.target_updates);command('setblock 2 65 0 '+slab+'[type='+half+',waterlogged='+wet+']')
     updates=client.target_updates[start:];assert updates and updates[-1]['state']==value,(color,properties,updates);case['states'][properties]=updates[-1];case['checks'].append('wire-state-'+properties)
    command('fill -10 65 -10 12 68 10 air');command('tp McsaProbe 0.5 65 0.5');command('clear McsaProbe');command('item replace entity McsaProbe weapon.mainhand with '+slab+' 1')
    assert client.game_mode==0
    start=len(client.target_updates);client.place((2,64,0),1,(.5,1.,.5));client.pump(.6);updates=client.target_updates[start:];assert updates and updates[-1]['state']==lookup['bottom/false'],(color,'place',updates);assert client.inventory[36] is None;case['placed']=updates[-1];case['checks'].append('ordinary-lower-placement-consumes-one-own-color')
    command('kill @e[type=item]');command('clear McsaProbe');command('item replace entity McsaProbe weapon.mainhand with diamond_pickaxe')
    start=len(client.events);client.dig(0);client.pump(2.);client.dig(FINISH_STATUS_777);client.pump(1.)
    selected=client.events[start:];assert any(e.get('position')==list(TARGET) and e.get('state')==0 for e in selected),(color,'not-air',selected)
    drops={e['entity']:e['slot'] for e in selected if e['name']=='entity_metadata' and e.get('slot')};assert list(drops.values())==[{'item':items[slab],'count':1}],(color,'drop',drops)
    case['drops']=drops;case['tool']=client.inventory.get(36);case['checks'].append('pickaxe-mining-air-and-one-own-color-drop')
    (out/'partial.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'color':color,'checks':len(case['checks']),'result':'PASS'}),flush=True)
 except BaseException as error:report['error']=repr(error);report['traceback']=traceback.format_exc()
 finally:
  if client:report['selected_events']=client.events;report['actions']=client.action_packets;report['gamemode_events']=client.mode_events;client.sock.close()
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
  with socket.socket() as s:report['port_free']=s.connect_ex(('127.0.0.1',port))!=0
  report['result']='PASS' if len(report['colors'])==16 and all(len(c['checks'])==8 for c in report['colors'].values()) and not report.get('error') and report.get('server_exit')==0 and report.get('group_gone') and report['port_free'] else 'FAIL';report['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();(out/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k in ['result','error','server_exit','group_gone','port_free']}));sys.exit(0 if report['result']=='PASS' else 1)

if __name__=='__main__':main()
