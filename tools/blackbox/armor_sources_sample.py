#!/usr/bin/env python3
"""Настоящие стрелы, взрыв вагонетки и падение: броня/чары/элитры777."""
import argparse
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
import struct
from thorns_natural_sample import damage,health_packet,inventory_changes,uint

ROOT=Path(__file__).resolve().parents[2]
CASES={
 'arrow':[('diamond_chestplate',6,0,None),('diamond_chestplate',6,0,'projectile_protection'),('elytra',6,0,None),('elytra',6,431,None),('elytra',6,431,'unbreaking')]*2,
 'blast':[('diamond_chestplate',6,0,None),('diamond_chestplate',6,0,'blast_protection'),('elytra',6,0,None),('elytra',6,431,None),('elytra',6,431,'unbreaking')]*2,
 'fall':[('diamond_boots',8,0,None),('diamond_boots',8,0,'feather_falling'),('elytra',6,0,None),('elytra',6,431,None)]*2,
}
CASES['arrow-break']=[('diamond_chestplate',6,527,None),('diamond_chestplate',6,527,'projectile_protection')]*2
CASES['blast-break']=[('diamond_chestplate',6,527,None),('diamond_chestplate',6,527,'blast_protection')]*2
INTERVAL={'arrow':72,'blast':140,'fall':90,'arrow-break':72,'blast-break':140}
name=lambda kind:'armor-'+kind+'-real-observe-777'


def prepare(output,kinds=None):
 output.mkdir(exist_ok=False,parents=True)
 for kind in (kinds or list(CASES)):
  cases=CASES[kind];family=kind.split('-')[0]
  folder=output/kind;folder.mkdir()
  s=json.loads((ROOT/'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text());actions=[]
  s['setup'].insert(2,'effect clear $(bot)');s['setup'].insert(3,'effect clear $(bot2)')
  s['setup'].extend(['fill $(-10,-5,-10) $(16,-1,10) obsidian','tp $(bot) $(3.5,0,3.5) 180 0','tp $(bot2) $(-4.5,0,-4.5) 0 0'])
  if family=='blast':s['setup'].extend(['setblock $(9,-1,3) redstone_block','setblock $(9,0,3) activator_rail[shape=east_west,powered=true]'])
  for i,(item,slot,initial,enchant) in enumerate(cases):
   t=i*INTERVAL[kind]
   for who in ['$(bot)','$(bot2)']:actions.extend([{'after':t,'cmd':'clear '+who},{'after':t,'cmd':'effect give '+who+' minecraft:instant_health 1 10'}])
   actions.extend([{'after':t+2,'cmd':'tp $(bot) $(3.5,0,3.5) 180 0'}, {'after':t+2,'cmd':'tp $(bot2) '+('$(3.5,0,-0.5)' if family=='arrow' else '$(-4.5,0,-4.5)')+' 0 0'}])
   spec='minecraft:'+item+'[damage='+str(initial)+(',enchantments={'+enchant+':'+('3' if enchant=='unbreaking' else '4')+'}' if enchant else '')+']'
   actions.append({'after':t+4,'cmd':'item replace entity $(bot) armor.'+('feet' if slot==8 else 'chest')+' with '+spec})
   if family=='arrow':
    actions.extend([{'after':t+4,'cmd':'give $(bot2) bow 1'},{'after':t+4,'cmd':'give $(bot2) arrow 5'}, {'after':t+10,'use':True,'bot':2},{'after':t+26,'release':True,'bot':2}]);shift=54
   elif family=='blast':actions.append({'after':t+12,'cmd':'summon tnt_minecart $(9.5,0,3.5)'});shift=118
   else:actions.append({'after':t+12,'cmd':'tp $(bot) $(3.5,12,3.5) 180 0'});shift=60
   actions.append({'after':t+shift,'window_click':{'inventory':True,'slot':slot,'mode':1}})
  s.update(name=name(kind),about='Discovery: реальный '+kind+', обычная/защитная броня и элитры. Серверный inventory echo после урона; HP/источник обязательны. Не готовый parity PASS.',actions=actions,record_ticks=len(cases)*INTERVAL[kind],random=[])
  s['cleanup'].extend(['kill @e[type=tnt_minecart,distance=..64]','kill @e[type=arrow,distance=..64]'])
  data=(json.dumps(s,ensure_ascii=False,indent=2)+'\n').encode();p=ROOT/'tools/blackbox/scenarios'/(name(kind)+'.json');assert not p.exists();p.write_bytes(data);(folder/'scenario.json').write_bytes(data)
  criteria={'schema':'armor-sources777-v1','kind':kind,'cases':cases,'interval':INTERVAL[kind],'scenario_sha256':hashlib.sha256(data).hexdigest(),
   'positive':'Actual initial HP20 both and before each action; one causal damage event received by both observers, HP loss on victim, attackerHP20. Real release/arrow spawn, real minecart/explosion or actual client fall; not /damage. Exact pre-hit item/damage/enchantment/count1. Explicit post-shift server item echo/count1/preserved enchantments or real break event BOTH observers plus empty armor slot.',
   'discovery':'No expected Elytra/armor result invented before opaque original observation. No equality of independent random trajectories assumed. Preserve actual HP/source/wear and freeze normative criteria only after baseline.'}
  (folder/'criteria-before-recording.json').write_text(json.dumps(criteria,indent=2)+'\n')


def inspect(raw,kind,items,entities):
 family=kind.split('-')[0]
 assert raw['scenario']==name(kind) and raw['protocol']==777 and raw['isolate_vitals'] is True
 assert len(raw['initial'])==len(raw['clients'])==2
 victim,attacker=[v['entity'] for v in raw['initial']]
 for v in raw['initial']:assert v['health']==health_packet(v['health_receipt'])==20 and v['health_receipt']['at']<=raw['start']
 health=[[(raw['start']-.001,20.)],[(raw['start']-.001,20.)]];hits=[[],[]];breaks=[[],[]];inv=[];state={};receipts=[];spawns=[[],[]]
 for observer,c in enumerate(raw['clients']):
  assert not c.get('failure') and c['events']
  for event in c['preroll']+c['events']:
   if event['name']=='update_health':health[observer].append((event['at'],health_packet(event)))
   if event['name']=='damage_event':hits[observer].append(damage(event,c['registry_names']['minecraft:damage_type']))
   if event['name']=='entity_status':
    b=bytes.fromhex(event['hex']);assert len(b)==5;entity,status=struct.unpack('>iB',b)
    if entity==victim and status==(52 if family=='fall' else 50):breaks[observer].append(event['at'])
   if event['name']=='spawn_entity':
    b=io.BytesIO(bytes.fromhex(event['hex']));entity=uint(b);assert len(b.read(16))==16;typ=uint(b);spawns[observer].append((event['at'],entity,typ))
   if observer==0:
    updates=inventory_changes(event)
    if updates:state.update(updates);inv.append((event['at'],state.copy()));receipts.append((event['at'],updates))
 clicks=[v for v in raw['sent'] if v['action'].get('window_click')];assert len(clicks)==len(CASES[kind])
 names=raw['clients'][0]['registry_names']['minecraft:enchantment'];rows=[]
 for i,((item,slot,initial,enchant),click) in enumerate(zip(CASES[kind],clicks)):
  begin=raw['start']+i*INTERVAL[kind]*.05;end=begin+INTERVAL[kind]*.05;moved=click['at']
  events=[[v for v in h if begin<=v['at']<end] for h in hits];assert all(len(v)==1 for v in events),(kind,i,events)
  at=min(v[0]['at'] for v in events)
  before=[next(hp for t,hp in reversed(h) if t<at) for h in health];after=[next(hp for t,hp in reversed(h) if t<end) for h in health]
  assert before==[20.,20.] and 0<after[0]<20 and after[1]==20.,(kind,i,before,after)
  assert moved>at
  expected='minecraft:'+{'arrow':'arrow','fall':'fall','blast':'explosion'}[family]
  roles=[]
  for observer in [0,1]:
   event=events[observer][0];assert event['entity']==victim and event['kind']==expected,(kind,i,event)
   if family=='arrow':
    arrow_ids=[eid for t,eid,typ in spawns[observer] if begin<=t<event['at'] and typ==entities['arrow']]
    assert len(arrow_ids)==1 and event['direct']==arrow_ids[0]+1 and event['cause']==attacker+1
    roles.append('arrow')
   elif family=='fall':
    assert event['cause']==event['direct']==0;roles.append('none')
   else:
    carts=[eid for t,eid,typ in spawns[observer] if begin<=t<event['at'] and typ==entities['tnt_minecart']]
    assert len(carts)==1 and event['cause']==0 and event['direct'] in [0,carts[0]+1]
    roles.append('minecart' if event['direct']==carts[0]+1 else 'missing')
  initial_stack=next(s for t,s in reversed(inv) if t<at).get(slot)
  assert initial_stack and initial_stack['item']==items[item] and initial_stack['count']==1 and initial_stack.get('damage',0)==initial
  wanted={('minecraft:'+enchant):(3 if enchant=='unbreaking' else 4)} if enchant else {}
  assert {names[n]:v for n,v in initial_stack.get('enchantments',[])}==wanted
  final=next(s for t,s in reversed(inv) if t<end);assert final.get(slot) is None
  matches=[(number,s) for number,s in final.items() if s and s['item']==items[item]];broken=[sum(begin<=t<end for t in h) for h in breaks]
  if matches:
   assert len(matches)==1 and broken==[0,0]
   number,stack=matches[0];assert stack['count']==1 and stack.get('enchantments',[])==initial_stack.get('enchantments',[])
   assert number!=slot and any(moved<=t<end and updates.get(number)==stack for t,updates in receipts),'No actual post-hit inventory echo'
   final_damage=stack.get('damage',0);assert final_damage>=initial
  else:
   assert broken==[1,1] and any(at<=t<moved and slot in updates and updates[slot] is None for t,updates in receipts)
   final_damage=None
  if family=='arrow':
   actions=[v['action'] for v in raw['sent'] if begin<=v['at']<end]
   assert {'after':i*INTERVAL[kind]+10,'use':True,'bot':2} in actions and {'after':i*INTERVAL[kind]+26,'release':True,'bot':2} in actions
  if family=='fall':
   moves=[v['position']['y'] for v in raw['clients'][0]['moves'] if begin<=v['at']<at]
   assert moves and max(moves)-min(moves)>8.,'Actual falling client trajectory required'
  rows.append({'trial':i,'item':item,'slot':slot,'enchantment':enchant,'initial_damage':initial,'final_damage':final_damage,'victim_damage':20-after[0],'broken':broken,'source':events[0][0]['kind'],'cause':events[0][0]['cause'],'direct':events[0][0]['direct'],'direct_roles':roles})
 return rows


def normalize(row):
 return {k:v for k,v in row.items() if k not in ['cause','direct']}


def validate(kind,rows,expected):
 assert len(rows)==len(CASES[kind])
 assert [normalize(r) for r in rows]==expected[kind],kind+' differs from frozen original outcomes'


def analyze(output,recordings,data,kinds=None,strict=False):
 items={v['name']:v['id'] for v in json.loads((data/'items.json').read_text())};entities={v['name']:v['id'] for v in json.loads((data/'entities.json').read_text())};result={}
 for kind in (kinds or list(CASES)):
  result[kind]={side:inspect(json.loads((recordings/(name(kind)+'-'+side+'.json')).read_text()),kind,items,entities) for side in ['official','ours']}
 result={'status':'recorded-not-yet-validated' if strict else 'discovery-only','sides':result}
 (output/'observations.json').write_text(json.dumps(result,indent=2)+'\n')
 if strict:
  expected=json.loads((ROOT/'tools/blackbox/ci/armor-sources-777.json').read_text())['expected']
  for kind,sides in result['sides'].items():
   for rows in sides.values():validate(kind,rows,expected)
  result['status']='strict-pass';(output/'observations.json').write_text(json.dumps(result,indent=2)+'\n')
 print({k:{s:len(r) for s,r in v.items()} for k,v in result['sides'].items()})


def ci_run(output):
 assert os.environ.get('GITHUB_ACTIONS')=='true'
 import combat_ci
 from elytra_melee_wear import ci_run as prior_controls
 output.mkdir(exist_ok=False)
 work=Path(os.environ['RUNNER_TEMP'])/'mcsa-armor-sources';work.mkdir(exist_ok=False)
 data=work/'data';data.mkdir();pin=json.loads((ROOT/'tools/blackbox/ci/protection-edges-777.json').read_text())
 combat_ci.fetch(pin['official_jar'],work/'server.jar')
 for entry in pin['json_files']:
  combat_ci.fetch(entry,data/entry['file'])
 probe=output/'sources';prepare(probe);shutil.copy2(ROOT/'tools/blackbox/ci/armor-sources-777.json',probe/'expected-before-candidate.json')
 binary=ROOT/'target/release/mcsheriffanya'
 manifest={'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'runtime_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'status':'running'}
 command=[sys.executable,str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(work/'recordings'),
  '--binary',str(binary),'--jar',str(work/'server.jar'),'--java','java','--official-port','26184','--core-port','26186',
  '--parallel','--max-seconds','700','--isolate-vitals']
 try:
  with (probe/'recorder.log').open('w') as f:subprocess.run(command+[name(k) for k in CASES],stdout=f,stderr=subprocess.STDOUT,check=True,timeout=760)
  analyze(probe,work/'recordings',data,strict=True)
  lifecycle=json.loads((work/'recordings/lifecycle.json').read_text());assert not lifecycle['cleanup_errors'] and all(lifecycle['ports_free'].values())
  legacy=output/'legacy-bows';legacy.mkdir();oldnames=['pvp-bow','pvp-bow-power','pvp-bow-punch','minecart-tnt-rail']
  second=command.copy();second[second.index('--output')+1]=str(work/'legacy-recordings')
  with (legacy/'recorder.log').open('w') as f:subprocess.run(second+oldnames,stdout=f,stderr=subprocess.STDOUT,check=True,timeout=760)
  with (legacy/'comparison.log').open('w') as f:subprocess.run([sys.executable,str(ROOT/'tools/blackbox/analyze777.py'),str(work/'legacy-recordings'),'--protocol',str(data/'protocol.json')],stdout=f,stderr=subprocess.STDOUT,check=True,timeout=120)
  rows=json.loads((work/'legacy-recordings/comparison-summary.json').read_text());assert len(rows)==4 and all(r['status']=='match' and not r['warnings'] and not any(r['decode_errors']) for r in rows),rows
  lifecycle=json.loads((work/'legacy-recordings/lifecycle.json').read_text());assert not lifecycle['cleanup_errors'] and all(lifecycle['ports_free'].values())
  prior_controls(output/'prior-combat')
  manifest['status']='strict-pass'
 finally:
  for source,target in [('recordings',probe),('legacy-recordings',output/'legacy-bows')]:
   target.mkdir(exist_ok=True)
   for p in (work/source).glob('*.json'):shutil.copy2(p,target/p.name)
   for p in (work/source).glob('*comparison*.txt'):shutil.copy2(p,target/p.name)
   for side in ['official','ours']:
    log=work/source/side/'server.log'
    if log.exists():shutil.copy2(log,target/(side+'-server.log'))
  for p in data.glob('*.json'):shutil.copy2(p,output/p.name)
  if manifest['status']=='running':manifest['status']='failed'
  (output/'run.json').write_text(json.dumps(manifest,indent=2)+'\n')


if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('mode',choices=['prepare','analyze','ci']);p.add_argument('--output',type=Path,required=True);p.add_argument('--recordings',type=Path);p.add_argument('--data',type=Path);p.add_argument('--only',choices=list(CASES),action='append');p.add_argument('--strict',action='store_true');a=p.parse_args()
 if a.mode=='prepare':prepare(a.output,a.only)
 elif a.mode=='ci':ci_run(a.output)
 else:analyze(a.output,a.recordings,a.data,a.only,a.strict)
