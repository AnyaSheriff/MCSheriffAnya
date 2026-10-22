#!/usr/bin/env python3
"""Selected wool slab behavior on official777 or a hash-pinned guarded core; --run opts in."""
import argparse,copy,hashlib,io,json,os,signal,socket,struct,subprocess,sys,time,traceback
from pathlib import Path
sys.dont_write_bytecode=True

def main():
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('--run',action='store_true');p.add_argument('--repo',type=Path,default=Path('/home/lenox/.local/share/mcsa-java-26.3'))
 p.add_argument('--data',type=Path,default=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/candidate-json'))
 target=p.add_mutually_exclusive_group(required=True);target.add_argument('--jar',type=Path);target.add_argument('--test-binary',type=Path)
 p.add_argument('--binary-sha256');p.add_argument('--output',type=Path,required=True);p.add_argument('--port',type=int)
 p.add_argument('--phase',choices=['recipes','tools','both'],default='both');p.add_argument('--colors',default='white,orange,magenta,light_blue,yellow,lime,pink,gray,light_gray,cyan,purple,blue,brown,green,red,black')
 a=p.parse_args();a.port=a.port or (26049 if a.jar else 26026);assert 1024<=a.port<=65535 and a.port not in [25565,25566,25567]
 helpers=Path(__file__).resolve().parent if Path(__file__).resolve().parent.name=='protocol-bridge' else (a.repo/'tools/protocol-bridge').resolve();sys.path.insert(0,str(helpers))
 from probe_container_session import Probe as InventoryProbe
 from probe_gameplay import connect
 from probe_white_concrete_slab import Probe as BlockProbe,JAR_SHA1,HASHES,TARGET,FINISH_STATUS_777,pack_position
 from probe_registry_client import read_string
 from probe_stonecutter_776 import selected_display
 from generate_tables import states
 from smoke import read_varint,varint
 colors=a.colors.split(',');allowed=['white','orange','magenta','light_blue','yellow','lime','pink','gray','light_gray','cyan','purple','blue','brown','green','red','black']
 assert colors and len(set(colors))==len(colors) and all(c in allowed for c in colors)
 for name,pin in HASHES.items():assert hashlib.sha256((a.data/name).read_bytes()).hexdigest()==pin,name
 if a.jar:assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==JAR_SHA1
 else:assert a.binary_sha256 and hashlib.sha256(a.test_binary.read_bytes()).hexdigest()==a.binary_sha256
 if a.test_binary:assert a.port==26026,'guarded core harness has fixed port26026'
 protocol=json.loads((a.data/'protocol.json').read_text());item_rows=json.loads((a.data/'items.json').read_text());names={r['id']:r['name'] for r in item_rows};items={v:k for k,v in names.items()};limits={r['name']:r['stackSize'] for r in item_rows}
 blocks={r['name']:r for r in json.loads((a.data/'blocks.json').read_text())};family={c+'_wool_slab' for c in colors};slab_ids={items[n] for n in family}
 plan={'scope':'selected wool slab777 observations; explicit server profile, no concrete recipe/drop inference','phase':a.phase,'colors':colors,'port':a.port,'input_pins':HASHES,'jar_sha1':JAR_SHA1 if a.jar else None,'binary_sha256':a.binary_sha256,'state_count':len(colors)*6,'tool_cases':len(colors)*6,'tools':['hand','shears','diamond_pickaxe'],'halves':['bottom','double'],'server_will_start':bool(a.run)}
 if not a.run:print(json.dumps(plan,ensure_ascii=False,indent=2));return
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();log=out/'server.log'
 report={'plan':plan,'recipes':{},'blocks':{},'commands':[],'limits':['NoGUI','manual crafting runs only directly decoded singleton ingredient displays','unknown display/tag/form recorded as inconclusive, never guessed','tool duration is a bounded hold, not measured vanilla break-time parity']}
 server=None;client=None;sequence=0
 def save(): (out/'partial.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
 def uint(d,cap=65536):
  n=read_varint(d.read);assert 0<=n<=cap,n;return n
 def boolean(d):
  b=d.read(1);assert b in [b'\0',b'\1'];return b==b'\1'
 def idset(d):
  n=uint(d);return {'tag':read_string(d)} if n==0 else {'items':[uint(d) for _ in range(n-1)]}
 class RecipeProbe(InventoryProbe):
  def __init__(self,*args):super().__init__(*args);self.stage='join';self.selected=[];self.cuts={};self.mode=None;self.mode_events=[]
  def slotdisplay(self,d,depth=0):
   assert depth<16;k=uint(d,10);r={'type':k}
   if k in [0,1]:pass
   elif k==2:r['base']=self.slotdisplay(d,depth+1)
   elif k==3:r['base']=self.slotdisplay(d,depth+1);r['component']=uint(d)
   elif k==4:r['item']=uint(d)
   elif k==5:r['stack']=self.slot(d,template=True)
   elif k==6:r['set']=idset(d)
   elif k in [7,9]:r['parts']=[self.slotdisplay(d,depth+1) for _ in range(2)]
   elif k==10:r['parts']=[self.slotdisplay(d,depth+1) for _ in range(uint(d,256))]
   else:raise ValueError(('unsupported observed slot display',k))
   return r
  def display(self,d):
   k=uint(d,4);r={'type':k}
   if k in [0,1]:
    if k==1:r['width']=uint(d,3);r['height']=uint(d,3)
    r['ingredients']=[self.slotdisplay(d) for _ in range(uint(d,9))];r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   elif k==2:
    r['ingredient']=self.slotdisplay(d);r['fuel']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d);r['duration']=uint(d);r['xp']=struct.unpack('>f',d.read(4))[0]
   elif k==3:r['ingredient']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   else:r['template']=self.slotdisplay(d);r['base']=self.slotdisplay(d);r['addition']=self.slotdisplay(d);r['result']=self.slotdisplay(d);r['station']=self.slotdisplay(d)
   return r
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload)
   if name=='game_state_change' and payload and payload[0]==3:
    assert len(payload)==5
    mode=struct.unpack('>f',payload[1:])[0];assert mode in [0.,1.,2.,3.]
    self.mode=int(mode);self.mode_events.append({'mode':self.mode,'payload_hex':payload.hex()})
   if name=='declare_recipes':
    for color in colors:
     slab=color+'_wool_slab'
     try:self.cuts[slab]={'status':'present','entry':selected_display(payload,items[color+'_wool'],items[slab])}
     except ValueError as error:
      self.cuts[slab]={'status':'absent' if str(error)=='selected stone→stone_slab recipe absent' else 'inconclusive','detail':str(error)}
   if name!='recipe_book_add' or self.stage=='join':return
   d=io.BytesIO(payload);chosen=[]
   for _ in range(uint(d,65536)):
    ident=uint(d);display=self.display(d);group=uint(d);category=uint(d,12);requirements=[idset(d) for _ in range(uint(d,32))] if boolean(d) else None
    flag=d.read(1);assert len(flag)==1 and flag[0]&~3==0
    result=display['result'];target=result.get('stack',{}).get('item',names.get(result.get('item')))
    if target in family:chosen.append({'display_id':ident,'display':display,'group_raw':group,'category':category,'requirements':requirements,'flags':flag[0],'stage':self.stage})
   replace=boolean(d);assert not d.read()
   for row in chosen:row['replace']=replace;self.selected.append(row)
 class ToolProbe(BlockProbe):
  def __init__(self,*args):super().__init__(*args);self.mode=None;self.mode_events=[]
  def observe_packet(self,name,payload):
   super().observe_packet(name,payload)
   if name=='game_state_change' and payload and payload[0]==3:
    assert len(payload)==5
    mode=struct.unpack('>f',payload[1:])[0];assert mode in [0.,1.,2.,3.]
    self.mode=int(mode);self.mode_events.append({'mode':self.mode,'payload_hex':payload.hex()})
 def command(text,wait=.2):
  report['commands'].append(text);server.stdin.write(text+'\n');server.stdin.flush()
  if client:client.pump(wait)
  else:time.sleep(wait)
 def close():
  if client.inventory and client.inventory['window']:client.send('close_window',varint(client.inventory['window']));client.pump(.2)
 def open_at(pos):
  nonlocal sequence
  close();sequence+=1;client.send('block_place',b'\0'+pack_position(pos)+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(sequence));client.pump(.4)
 def click(place,button=0):
  inv=client.inventory;inv['cursor']=None;client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',place,button)+b'\0\0\0');client.pump(.2)
 def target_entry(row):return row['display']['result'].get('stack',{}).get('item',names.get(row['display']['result'].get('item')))
 try:
  with socket.socket() as check:check.bind(('127.0.0.1',a.port))
  (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
  with log.open('w') as output:
   if a.jar:argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(a.jar.resolve()),'nogui'];ready='Done ('
   else:
    for marker in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']:(world/marker).touch()
    argv=[str(a.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture'];ready='MCSA gameplay probe ready'
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   deadline=time.monotonic()+100
   while ready not in log.read_text():assert server.poll() is None and time.monotonic()<deadline;time.sleep(.1)
   if a.phase in ['recipes','both']:
    client=connect(a.port,protocol,lambda s,p:RecipeProbe(s,p,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
    for text in ['gamerule minecraft:spawn_mobs false','fill -4 64 -4 5 64 4 stone','tp McsaProbe 0.5 65 0.5','setblock 2 65 0 crafting_table','setblock 2 65 2 stonecutter','clear McsaProbe','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(text)
    assert client.mode==0,client.mode_events
    report['recipe_survival_mode_events']=copy.deepcopy(client.mode_events)
    client.stage='initial-take';command('recipe take McsaProbe *')
    discovery=[];report['selected_recipe_discovery']=discovery
    for color in colors:
     slab=color+'_wool_slab';base=color+'_wool';case=report['recipes'][color]={'input':base,'target':slab,'selected_entries':[r for r in discovery if target_entry(r)==slab],'stonecut_declaration':client.cuts.get(slab,{'status':'not-received'})}
     close();command('clear McsaProbe');client.stage=color+'/take-all';command('recipe take McsaProbe *');client.stage=color+'/natural';command('give McsaProbe '+base+' 1',.7)
     case['natural_family_entries']=[r for r in client.selected if r['stage']==client.stage]
     # Candidate names are hypotheses until actual named reaward/console feedback.
     case['candidate_named_checks']=[]
     for candidate in [slab,slab+'_from_'+base+'_stonecutting']:
      client.stage=color+'/take-candidate';command('recipe take McsaProbe minecraft:'+candidate)
      client.stage=color+'/named/'+candidate;start=len(client.selected);command('recipe give McsaProbe minecraft:'+candidate,.4)
      case['candidate_named_checks'].append({'candidate':'minecraft:'+candidate,'selected_reaward':copy.deepcopy(client.selected[start:]),'accepted_selected_target':any(target_entry(r)==slab for r in client.selected[start:])})
     # Union only actually emitted target entries. No blanket give* packet decoding.
     observed={r['display_id']:r for r in client.selected if target_entry(r)==slab}
     case['selected_entries']=copy.deepcopy(list(observed.values()))
     discovery.extend(case['selected_entries'])
     case['presence']='observed' if observed else 'not-observed-by-input-or-candidate-names'
     crafting=[r for r in case['selected_entries'] if r['display']['type'] in [0,1]]
     case['craft_operations']=[]
     for row in crafting:
      d=row['display'];operation={'display_id':row['display_id'],'display':d};case['craft_operations'].append(operation)
      supplied=[]
      for ingredient in d['ingredients']:
       if ingredient['type']==0:supplied.append(None)
       elif ingredient['type']==4:supplied.append(ingredient['item'])
       elif ingredient['type']==6 and len(ingredient.get('set',{}).get('items',[]))==1:supplied.append(ingredient['set']['items'][0])
       else:operation['status']='not-executed-unsupported-ingredient';break
      else:
       close();command('clear McsaProbe')
       for item in set(supplied)-{None}:command('give McsaProbe '+names[item]+' '+str(supplied.count(item)))
       open_at((2,65,0));assert client.inventory['window']!=0
       for number,item in enumerate(supplied):
        if item is None:continue
        source=next(i for i,v in enumerate(client.inventory['slots']) if i>=10 and v and v['item']==names[item]);click(source)
        cell=1+(number//d['width']*3+number%d['width'] if d['type']==1 else number);click(cell,1);click(source)
       operation['ready']=copy.deepcopy(client.inventory)
       observed=client.inventory['slots'][0];operation['observed_output']=observed
       expected=d['result'].get('stack')
       if expected is not None:assert observed==expected,('observed-display-vs-manual-output',slab,observed,expected)
       if observed:
        click(0);operation['taken']=copy.deepcopy(client.inventory);assert client.inventory['cursor']==observed
       operation['status']='observed'
     cut=case['stonecut_declaration']
     if cut['status']=='present':
      close();command('clear McsaProbe');command('give McsaProbe '+base+' 1');open_at((2,65,2))
      source=next(i for i,v in enumerate(client.inventory['slots']) if i>=2 and v and v['item']==base);click(source);click(0)
      entry=cut['entry'];client.send('enchant_item',varint(client.inventory['window'])+varint(entry['button_ordinal_for_stone']));client.pump(.4)
      case['stonecut_ready']=copy.deepcopy(client.inventory);observed=client.inventory['slots'][1]
      if observed:click(1)
      case['stonecut_taken']=copy.deepcopy(client.inventory)
     save();print(json.dumps({'phase':'recipes','color':color,'selected':len(case['selected_entries']),'cut':cut['status']}),flush=True)
    close();client.sock.close();client=None;time.sleep(.4)
   if a.phase in ['tools','both']:
    entity=next(r['id'] for r in json.loads((a.data/'entities.json').read_text()) if r['name']=='item')
    client=connect(a.port,protocol,lambda s,p:ToolProbe(s,p,entity),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
    for text in ['gamerule minecraft:spawn_mobs false','gamerule minecraft:random_tick_speed 0','fill -4 64 -4 5 64 4 stone','fill -10 65 -10 12 68 10 air','tp McsaProbe 0.5 65 0.5','clear McsaProbe','gamemode creative McsaProbe','gamemode survival McsaProbe']:command(text)
    assert client.mode==0,client.mode_events;report['tools_survival_mode_events']=copy.deepcopy(client.mode_events);client.grounded_fixture=True;client.send('held_item_slot',struct.pack('>h',0))
    for color in colors:
     slab=color+'_wool_slab';lookup={dict(props)['type']+'/'+dict(props)['waterlogged']:state for (_,props),state in states([blocks[slab]]).items()};case=report['blocks'][color]={'target':slab,'states':{},'tools':[]}
     for key,state in lookup.items():
      half,wet=key.split('/');start=len(client.target_updates);command('setblock 2 65 0 '+slab+'[type='+half+',waterlogged='+wet+']');updates=client.target_updates[start:];assert updates and updates[-1]['state']==state;case['states'][key]=updates[-1]
     command('fill -10 65 -10 12 68 10 air');command('tp McsaProbe 0.5 65 0.5');command('clear McsaProbe');command('item replace entity McsaProbe weapon.mainhand with '+slab+' 1')
     start=len(client.target_updates);client.place((2,64,0),1,(.5,1.,.5));client.pump(.6);case['placement']={'updates':client.target_updates[start:],'held':client.inventory.get(36)}
     assert case['placement']['updates'] and case['placement']['updates'][-1]['state']==lookup['bottom/false']
     assert client.inventory.get(36) is None
     for half in ['bottom','double']:
      for tool in ['hand','shears','diamond_pickaxe']:
       assert client.mode==0,client.mode_events
       command('kill @e[type=item]');client.item_ids.clear();command('clear McsaProbe');command('setblock 2 65 0 '+slab+'[type='+half+',waterlogged=false]')
       if tool!='hand':command('item replace entity McsaProbe weapon.mainhand with '+tool)
       before=copy.deepcopy(client.inventory.get(36));start=len(client.events);action_start=len(client.action_packets)
       client.dig(0);client.pump(2.);client.dig(FINISH_STATUS_777);client.pump(.7)
       selected=client.events[start:];drops={r['entity']:r['slot'] for r in selected if r['name']=='entity_metadata' and r.get('slot')}
       air=any(r.get('position')==list(TARGET) and r.get('state')==0 for r in selected)
       case['tools'].append({'half':half,'tool':tool,'held_before':before,'held_after':copy.deepcopy(client.inventory.get(36)),'air_observed':air,'status':'observed' if air else 'inconclusive-no-air-within-hold','drops':drops,'spawns':[r for r in selected if r['name']=='spawn_entity'],'block_updates':[r for r in selected if r.get('position')==list(TARGET) and 'state' in r],'drop_metadata':[r for r in selected if r['name']=='entity_metadata' and r.get('slot')],'actions':client.action_packets[action_start:]})
     save();print(json.dumps({'phase':'tools','color':color,'cases':len(case['tools'])}),flush=True)
 except BaseException as error:report['error']=repr(error);report['traceback']=traceback.format_exc()
 finally:
  if client:
   try:client.sock.close()
   except OSError:pass
  if server:
   if server.poll() is None:
    try:server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=25)
    except BaseException:
     try:os.killpg(server.pid,signal.SIGTERM)
     except ProcessLookupError:pass
     try:server.wait(timeout=5)
     except subprocess.TimeoutExpired:
      try:os.killpg(server.pid,signal.SIGKILL)
      except ProcessLookupError:pass
      server.wait(timeout=5)
   report['server_exit']=server.returncode
   try:os.killpg(server.pid,0);report['group_gone']=False
   except ProcessLookupError:report['group_gone']=True
  with socket.socket() as check:report['port_free']=check.connect_ex(('127.0.0.1',a.port))!=0
  report['runner_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
  report['own_helper_sha256']={str(Path(m.__file__).relative_to(helpers)):hashlib.sha256(Path(m.__file__).read_bytes()).hexdigest() for m in list(sys.modules.values()) if getattr(m,'__file__',None) and Path(m.__file__).suffix=='.py' and Path(m.__file__).is_relative_to(helpers)}
  complete=(a.phase=='tools' or len(report['recipes'])==len(colors)) and (a.phase=='recipes' or len(report['blocks'])==len(colors))
  report['result']='OBSERVATION_COMPLETE' if complete and not report.get('error') and report.get('server_exit')==0 and report.get('group_gone') and report['port_free'] else 'FAIL'
  (out/'result.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(json.dumps({k:report.get(k) for k in ['result','error','server_exit','group_gone','port_free']}))
 return 0 if report['result']=='OBSERVATION_COMPLETE' else 1
if __name__=='__main__':sys.exit(main())
