#!/usr/bin/env python3
"""Только внешняя подготовка и анализ; запуска серверов, Cargo или CI здесь нет."""
import argparse,collections,copy,hashlib,io,json,math,struct,sys
from pathlib import Path
BASE=Path(__file__).parent
CRITERIA_SHA256='24e96f7b79c34111d6239c80bb16cb04f3c602895147e93ee881f3a366ea1cbe'
GROUPS=('offhand-commandloaded','mainhand-natural','offhand-natural')
ROTATION=[math.pi,math.pi/9]
read=lambda p:json.loads(p.read_text())
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
def cases(group):
 assert group in GROUPS
 natural=group!='offhand-commandloaded';hand=int(group!='mainhand-natural');duration=104 if natural else 56
 return [dict(trial=i,hand=hand,natural=natural,control=i in [0,1,130,131],damage=0 if i in [0,1,130,131] else 464,unbreaking=0 if i in [0,1,130,131] else 3,start=i*duration,duration=duration,fire=i*duration+(76 if natural else 24)) for i in range(132)]
def scenario(group,repo):
 s=read(repo/'tools/blackbox/scenarios/pvp-thorns-control-777.json');actions=[]
 def swap(t):actions.append(dict(after=t,window_click=dict(inventory=True,slot=36,mode=2,button=40)))
 def menu(t):actions.extend([dict(after=t,click=[5,0,3]),dict(after=t+6,close=True)])
 for c in cases(group):
  t=c['start'];actions.extend([dict(after=t,cmd='kill @e[type=arrow]'),dict(after=t,cmd='clear $(bot)'),dict(after=t+1,cmd='tp $(bot) $(3.5,0,3.5) 0 -20')])
  ench=[]
  if c['unbreaking']:ench.append('unbreaking:3')
  if c['natural']:ench.append('multishot:1')
  components=['damage='+str(c['damage'])]
  if ench:components.append('enchantments={'+','.join(ench)+'}')
  if not c['natural']:components.append('charged_projectiles=[{id:"minecraft:arrow",count:1},{id:"minecraft:arrow",count:1},{id:"minecraft:arrow",count:1}]')
  else:actions.append(dict(after=t+2,cmd='item replace entity $(bot) inventory.0 with minecraft:arrow 3'))
  actions.append(dict(after=t+2,cmd='item replace entity $(bot) weapon.'+('offhand' if c['hand'] else 'mainhand')+' with minecraft:crossbow['+','.join(components)+']'))
  if c['hand']:swap(t+4)
  menu(t+10)
  if c['hand']:swap(t+18)
  if c['natural']:
   actions.extend([dict(after=t+24,look=ROTATION,use=True,hand=c['hand']),dict(after=t+54,release=True)])
   if c['hand']:swap(t+56)
   menu(t+62)
   if c['hand']:swap(t+70)
  actions.append(dict(after=c['fire'],look=ROTATION,use=True,hand=c['hand']))
  if c['hand']:swap(c['fire']+8)
  menu(c['fire']+14)
 s.update(name='crossbow-ub-'+group+'-777',about='PRECAPTURE bounded original-first UBIII; no runtime proof yet',actions=actions,record_ticks=132*cases(group)[0]['duration'],random=[],track_projectiles=True,blocks=[dict(at=[5,0,3],state='minecraft:chest[facing=west,type=single,waterlogged=false]')])
 s['setup']=[v for v in s['setup'] if not v.startswith('tp ')]+['tp $(bot) $(3.5,0,3.5) 0 -20','tp $(bot2) $(0.5,0,3.5) 90 0'];return s

def validate_case(c,e):
 """В analyze сюда поступают только разобранные raw; входы unit tests синтетические."""
 expected={'minecraft:unbreaking':3} if c['unbreaking'] else {}
 if c['natural']:expected['minecraft:multishot']=1
 initial,loaded,final=e['initial'],e['loaded'],e['final']
 for stack in [initial,loaded]:
  assert stack and stack['item']=='crossbow' and stack['count']==1 and stack.get('damage',0)==c['damage']
  assert stack['named_enchantments']==expected
 assert loaded.get('projectiles')==[dict(item='arrow',count=1)]*3
 assert e['no_break_before_fire']
 assert e['actual_hands']==[c['hand']]*(2 if c['natural'] else 1)
 assert e['actual_yaw_pitch']==[(0.,-20.)]*len(e['actual_hands'])
 assert e['menu_count']==(3 if c['natural'] else 2) and e['actual_ack'] and e['old_ids_gone_before_first_use']
 if c['hand']:assert e['offhand_echo_before_fire'] and (not c['natural'] or e['offhand_echo_before_load'])
 if c['natural']:
  assert not initial.get('projectiles') and e['ammo']==[3,2,2]
  assert e['release_status']==6 and 1.25<=e['held_seconds']<=2.0 and e['no_spawn_during_loading']
 else:assert initial['projectiles']==loaded['projectiles'] and e['ammo']==[None,None]
 assert e['emitted_ids'][0]==e['emitted_ids'][1];n=len(e['emitted_ids'][0]);assert 1<=n<=3
 assert all(kind=='arrow' for _,kind in e['emitted_ids'][0]);status=48 if c['hand'] else 47
 if final:
  assert final['item']=='crossbow' and final['count']==1 and not final.get('projectiles')
  assert final['named_enchantments']==expected and final.get('damage',0)==(3 if c['control'] else 464)
  assert n==3 and e['break_status']==[[],[]]
 else:assert not c['control'] and e['break_status']==[[status],[status]]
 if c['control']:assert n==3 and final;category='plain_fresh_control'
 else:category=['one_arrow_then_break','two_arrows_then_break','three_arrows_then_break' if final is None else 'three_arrows_survived'][n-1]
 return category

def statistics(rows):
 assert sha(BASE/'criteria.json')==CRITERIA_SHA256
 fixed=read(BASE/'criteria.json');assert len(rows)==132
 assert [r['trial'] for r in rows if r['control']]==[0,1,130,131]
 assert all(r['category']=='plain_fresh_control' for r in rows if r['control'])
 sample=[r for r in rows if not r['control']];assert len(sample)==128
 counts={label:sum(r['category']==label for r in sample) for label in fixed['categories']};assert sum(counts.values())==128
 failures=[label for label,n in counts.items() if not fixed['categories'][label]['inclusive_count_bounds'][0]<=n<=fixed['categories'][label]['inclusive_count_bounds'][1]]
 return dict(N=128,counts=counts,statistical_pass=not failures,failures=failures,criteria_sha256=sha(BASE/'criteria.json'))

def inspect(group,raw,data,repo):
 sys.path.insert(0,str(repo/'tools/protocol-bridge'));sys.path.insert(0,str(repo/'tools/blackbox'))
 from probe_crossbow_session import Probe
 from probe_player_actions_runtime import menu_slot
 from smoke import read_varint
 from crossbow_nearbreak_ci import check_raw_controls
 check_raw_controls(raw,read(data/'protocol.json'),repo)
 expected=scenario(group,repo);assert raw['scenario']==expected['name'] and raw.get('causal_menu_readiness') is True
 assert raw.get('capture_side') in ['official','ours'], 'new explicit capture-side receipt required'
 assert [s['action'] for s in raw['sent']]==sorted(expected['actions'],key=lambda a:a['after'])
 names={r['id']:r['name'] for r in read(data/'items.json')};entities={r['id']:r['name'] for r in read(data/'entities.json')};parser=object.__new__(Probe);parser.names=names;parser.version=777
 menus=[];changes=[];state={};spawns=[[],[]];statuses=[[],[]];destroyed=[{},{}];teleports={};owner=raw['initial'][0]['entity'];enchant_names=raw['clients'][0]['registry_names']['minecraft:enchantment']
 def named(stack):
  if stack is None:return None
  return dict(**stack,named_enchantments={enchant_names[k]:v for k,v in stack.get('enchantments',[])})
 for viewer,client in enumerate(raw['clients']):
  for event in client['events']:
   b=io.BytesIO(bytes.fromhex(event['hex']));name=event['name'];updated={}
   if name=='window_items' and viewer==0:
    window,revision,n=[read_varint(b.read) for _ in range(3)];assert n<=100;slots=[parser.slot(b) for _ in range(n)];assert parser.slot(b) is None and not b.read()
    if window:assert n==63 and slots[:27]==[None]*27;menus.append((event['at'],window,slots))
    else:assert n==46;updated=dict(enumerate(slots))
   elif name=='set_slot' and viewer==0:
    window,revision=[read_varint(b.read) for _ in range(2)];slot=struct.unpack('>h',b.read(2))[0];stack=parser.slot(b);assert not b.read()
    if window==0:updated={slot:stack}
   elif name=='set_player_inventory' and viewer==0:
    slot=menu_slot(read_varint(b.read));updated={slot:parser.slot(b)};assert not b.read()
   elif name=='spawn_entity':
    entity=read_varint(b.read);assert len(b.read(16))==16;kind=entities[read_varint(b.read)]
    if kind in ['arrow','spectral_arrow','firework_rocket']:spawns[viewer].append((event['at'],entity,kind))
   elif name=='entity_status':
    entity,status=struct.unpack('>iB',bytes.fromhex(event['hex']))
    if entity==owner:statuses[viewer].append((event['at'],status))
   elif name=='entity_destroy':
    for _ in range(read_varint(b.read)):destroyed[viewer][read_varint(b.read)]=event['at']
    assert not b.read()
   elif name=='position' and viewer==0:
    teleport=read_varint(b.read);xyz=list(struct.unpack('>ddd',b.read(24)));b.read(24);yaw,pitch=struct.unpack('>ff',b.read(8));assert struct.unpack('>I',b.read(4))[0]==0 and not b.read();teleports[teleport]=(event['at'],xyz+[yaw,pitch])
   if updated:state.update(updated);changes.append((event['at'],updated,state.copy()))
 wires=raw['clients'][0]['wire_sent'];wireuse=[w for w in wires if w['name']=='use_item'];releases=[w for w in wires if w['name']=='block_dig'];acks={}
 for w in wires:
  if w['name']=='teleport_confirm':
   b=io.BytesIO(bytes.fromhex(w['hex']));tid=read_varint(b.read);pose=list(struct.unpack('>dddff',b.read(32)));assert not b.read();acks[tid]=(w['at'],pose)
 swaps=[w for w in wires if w['name']=='window_click'];assert len(swaps)==(0 if group=='mainhand-natural' else (5 if group=='offhand-natural' else 3)*132)
 for w in swaps:
  b=io.BytesIO(bytes.fromhex(w['hex']));assert read_varint(b.read)==0;read_varint(b.read);assert struct.unpack('>hb',b.read(3))==(36,40) and read_varint(b.read)==2 and b.read()==b'\0\0'
 cleanups=[c for c in raw['commands'] if c.get('via')=='OWN isolated RCON' and c.get('command')=='kill @e[type=arrow]'];tp=[c['fresh_tp_ack'] for c in raw['commands'] if c.get('command','').startswith('tp blackbox ') and c.get('fresh_tp_ack',{}).get('at',0)>=raw['start']];assert len(cleanups)==len(tp)==132
 rows=[]
 for c,cleanup,ack in zip(cases(group),cleanups,tp):
  start=raw['start']+c['start']*.05;end=start+c['duration']*.05
  uses=[w for w in wireuse if start<=w['at']<end];assert len(uses)==(2 if c['natural'] else 1);fire=uses[-1]['at'];hands=[];rotations=[]
  for w in uses:
   b=io.BytesIO(bytes.fromhex(w['hex']));hands.append(read_varint(b.read));assert read_varint(b.read)>0;rotations.append(struct.unpack('>ff',b.read(8)));assert not b.read()
  snapshots=[(at,window,slots) for at,window,slots in menus if start<=at<end];assert len(snapshots)==(3 if c['natural'] else 2)
  for at,window,slots in snapshots:
   assert all(stack is None for slot,stack in enumerate(slots) if slot!=54 and not(c['natural'] and slot==27))
   if c['natural']:assert slots[27]==dict(item='arrow',count=3 if len([m for m in snapshots if m[0]<at])==0 else 2)
   readiness=[r for r in raw['commands'] if r.get('readiness')=='actual-open-window-and-full-items' and r['received_at']==at and r['window']==window];assert len(readiness)==1
   opens=[e for e in raw['clients'][0]['events'] if e['name']=='open_window' and start<=e['at']<=at and read_varint(io.BytesIO(bytes.fromhex(e['hex'])).read)==window];assert opens
   closes=[w for w in wires if w['name']=='close_window' and at<w['at']<end and read_varint(io.BytesIO(bytes.fromhex(w['hex'])).read)==window];assert len(closes)==1
  assert snapshots[0][0]<uses[0]['at'] and snapshots[-1][0]>fire
  if c['natural']:assert uses[0]['at']<snapshots[1][0]<fire
  before,loaded,final=[named(snapshots[k][2][54]) for k in [0,-2 if c['natural'] else 0,-1]]
  def echo(after,use,stack):
   received=[row for row in changes if start+after*.05<=row[0]<use and 45 in row[1]]
   return bool(received and named(received[-1][2].get(45))==stack and received[-1][2].get(36) is None)
  assert ack['id'] in teleports and ack['id'] in acks
  server_at,server_pose=teleports[ack['id']];wire_at,wire_pose=acks[ack['id']]
  actual_ack=start<=server_at<=wire_at<=ack['at']<snapshots[0][0] and server_pose==wire_pose==ack['pose'] and wire_pose[:3]==[103.5,-59.,103.5]
  assert cleanup['old_arrow_ids_still_tracked']==[[],[]]
  oldgone=cleanup['issued_at']<=cleanup['all_old_ids_gone_at']<uses[0]['at']
  for viewer,ids in enumerate(cleanup['old_arrow_ids_before']):oldgone &= all(entity in destroyed[viewer] and destroyed[viewer][entity]<uses[0]['at'] for entity in ids)
  release=[w for w in releases if start<=w['at']<fire];release_status=None;held=None
  if c['natural']:
   assert len(release)==1;payload=bytes.fromhex(release[0]['hex']);assert payload==bytes([6])+bytes(8)+b'\0\0';release_status=6;held=release[0]['at']-uses[0]['at'];assert release[0]['at']<snapshots[1][0]
  else:assert not release
  assert all(not(start<=at<fire and 47<=status<=52) for side in statuses for at,status in side), 'premature break status before fire'
  e=dict(no_break_before_fire=all(not(start<=at<fire and 47<=status<=52) for side in statuses for at,status in side),initial=before,loaded=loaded,final=final,actual_hands=hands,actual_yaw_pitch=rotations,menu_count=len(snapshots),actual_ack=actual_ack,old_ids_gone_before_first_use=oldgone,offhand_echo_before_fire=echo(70 if c['natural'] else 18,fire,loaded) if c['hand'] else False,offhand_echo_before_load=echo(18,uses[0]['at'],before) if c['hand'] and c['natural'] else False,ammo=[(slots[27]['count'] if slots[27] and slots[27]['item']=='arrow' else None) for _,_,slots in snapshots],release_status=release_status,held_seconds=held,no_spawn_during_loading=all(not(uses[0]['at']<=at<fire) for side in spawns for at,_,_ in side) if c['natural'] else None,emitted_ids=[sorted((entity,kind) for at,entity,kind in side if fire<=at<end) for side in spawns],break_status=[[status for at,status in side if fire<=at<end and 47<=status<=52] for side in statuses])
  rows.append(dict(trial=c['trial'],control=c['control'],category=validate_case(c,e),evidence=e))
 assert len(wireuse)==132*(2 if cases(group)[0]['natural'] else 1)
 auxiliary=[[dict(at=at,status=status) for at,status in side if status==55] for side in statuses]
 return dict(status='bounded-observation-not-full-parity',rows=rows,statistics=statistics(rows),auxiliary_swap55=auxiliary,auxiliary_nonbreak_statuses=[[dict(at=at,status=status) for at,status in side if not 47<=status<=52] for side in statuses],auxiliary_known_mismatch_not_masked=True)

def negative_raw_controls(group,raw,data,repo):
 """Будущие повреждения настоящего raw; до первого original они не выполнялись."""
 result=[]
 for label in ['wire-hand','wire-use','wire-ACK','full-menu','observer-spawn','observer-break','oldID-pending']+(['wire-release'] if 'natural' in group else []):
  if label=='observer-break' and not any(e['name']=='entity_status' and 47<=struct.unpack('>iB',bytes.fromhex(e['hex']))[1]<=52 for e in raw['clients'][1]['events']):
   result.append(dict(mutation=label,status='NOT-APPLICABLE-no-positive-break-event',rejected=None));continue
  v=copy.deepcopy(raw)
  if label=='wire-hand':
   packet=next(w for w in v['clients'][0]['wire_sent'] if w['name']=='use_item');payload=bytes.fromhex(packet['hex']);packet['hex']=(bytes([payload[0]^1])+payload[1:]).hex()
  elif label in ['wire-use','wire-ACK','wire-release']:
   name={'wire-use':'use_item','wire-ACK':'teleport_confirm','wire-release':'block_dig'}[label];v['clients'][0]['wire_sent']=[w for w in v['clients'][0]['wire_sent'] if w['name']!=name]
  elif label=='full-menu':v['clients'][0]['events']=[e for e in v['clients'][0]['events'] if e['name']!='window_items']
  elif label=='observer-spawn':v['clients'][1]['events']=[e for e in v['clients'][1]['events'] if e['name']!='spawn_entity']
  elif label=='observer-break':v['clients'][1]['events']=[e for e in v['clients'][1]['events'] if e['name']!='entity_status']
  elif label=='oldID-pending':next(c for c in v['commands'] if c.get('via')=='OWN isolated RCON' and c.get('command')=='kill @e[type=arrow]')['old_arrow_ids_still_tracked']=[[123],[]]
  try:
   checked=inspect(group,v,data,repo)
  except (AssertionError,ValueError,EOFError):result.append(dict(mutation=label,rejected=True,kind='receipt' if label=='oldID-pending' else 'raw-packet'))
  else:
   raise AssertionError('negative raw accepted: '+label)
 return result

def report_auxiliary(original,candidate):
 return dict(status='OPEN-known-swap55-mismatch-not-full-parity',original_counts=[len(v) for v in original['auxiliary_swap55']],candidate_counts=[len(v) for v in candidate['auxiliary_swap55']],original_raw_auxiliary=original['auxiliary_swap55'],candidate_raw_auxiliary=candidate['auxiliary_swap55'])

def freeze_original(group,raw_path,data,repo,output,capture_receipt=None):
 assert not output.exists(), 'first original pin is immutable'
 raw_bytes=raw_path.read_bytes();raw=json.loads(raw_bytes);raw_digest=hashlib.sha256(raw_bytes).hexdigest();assert raw.get('capture_side')=='official'
 assert capture_receipt and capture_receipt['source'] and capture_receipt['gameplay_base']==read(BASE/'source-pins.json')['source']
 assert capture_receipt['core_tree']==read(BASE/'source-pins.json')['core_tree']
 assert capture_receipt['requested_capture_sides']==['official'] and capture_receipt['raw_sha256']==raw_digest
 assert capture_receipt['criteria_sha256']==sha(BASE/'criteria.json') and capture_receipt['recorder_sha256']==read(BASE/'source-pins.json')['proposed_recorder_sha256']
 assert capture_receipt['jar_sha1']=='33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
 expected_scenario=hashlib.sha256((json.dumps(scenario(group,repo),ensure_ascii=False,indent=2)+'\n').encode()).hexdigest()
 assert capture_receipt['scenario_sha256']==expected_scenario
 assert capture_receipt['lifecycle_clean'] is True and capture_receipt['lock_sha256'] and capture_receipt['runtime_sha256']
 assert capture_receipt['plugins_active'] is False
 result=inspect(group,raw,data,repo)
 output.mkdir(exist_ok=False)
 (output/'original-raw.json').write_bytes(raw_bytes)
 (output/'criteria.json').write_bytes((BASE/'criteria.json').read_bytes())
 write(output/'observation.json',result)
 write(output/'original-first-pin.json',dict(group=group,source_before_candidate=read(BASE/'source-pins.json')['source'],raw_sha256=raw_digest,capture_source=capture_receipt['source'],criteria_sha256=sha(BASE/'criteria.json'),scenario_sha256=hashlib.sha256((json.dumps(scenario(group,repo),ensure_ascii=False,indent=2)+'\n').encode()).hexdigest(),first_complete_original=True,statistical_pass=result['statistics']['statistical_pass'],no_retry_for_green=True,historical_mainhand_PRIMARY_unchanged=True))
 return result

def prepare(output,repo):
 output.mkdir(exist_ok=False)
 pins=read(BASE/'source-pins.json')
 for name,h in pins['files'].items():assert sha(repo/name)==h or (name=='tools/blackbox/record777.py' and sha(repo/name)==read(BASE/'source-pins.json')['proposed_recorder_sha256'])
 for group in GROUPS:
  p=output/group;p.mkdir();write(p/'scenario.json',scenario(group,repo));write(p/'cases.json',cases(group))
 write(output/'manifest.json',dict(source=pins['source'],criteria_sha256=sha(BASE/'criteria.json'),scenario_sha256={g:sha(output/g/'scenario.json') for g in GROUPS},cases_sha256={g:sha(output/g/'cases.json') for g in GROUPS},status='PRECAPTURE-ONLY-NO-SERVERS'))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['prepare','analyze']);p.add_argument('--repo',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--group',choices=GROUPS);p.add_argument('--raw',type=Path);p.add_argument('--data',type=Path);a=p.parse_args()
 if a.mode=='prepare':prepare(a.output,a.repo)
 else:
  assert a.group and a.raw and a.data;write(a.output,inspect(a.group,read(a.raw),a.data,a.repo))
