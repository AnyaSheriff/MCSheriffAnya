#!/usr/bin/env python3
"""Тот же настоящий TNTcart: Easy/Hard wear и положительный Peaceful-контроль."""
import copy
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import armor_sources_sample as armor
from thorns_natural_sample import health_packet,inventory_changes,uint

ROOT=armor.ROOT
DIFFICULTIES=['peaceful','easy','hard']
name=lambda difficulty:'armor-blast-difficulty-'+difficulty+'-777'


def prepare(output):
 output.mkdir(exist_ok=False)
 base=json.loads((ROOT/'tools/blackbox/scenarios'/ (armor.name('blast')+'.json')).read_text())
 for difficulty in DIFFICULTIES:
  s=copy.deepcopy(base);s['name']=name(difficulty);s['setup'].append('difficulty '+difficulty)
  raw=(json.dumps(s,ensure_ascii=False,indent=2)+'\n').encode();target=ROOT/'tools/blackbox/scenarios'/(s['name']+'.json');assert not target.exists();target.write_bytes(raw);(output/(s['name']+'.json')).write_bytes(raw)


def harmless(raw,items,entities):
 assert raw['protocol']==777 and raw['isolate_vitals'] and len(raw['clients'])==len(raw['initial'])==2
 for v in raw['initial']:assert v['health']==health_packet(v['health_receipt'])==20 and v['health_receipt']['at']<=raw['start']
 inv=[];receipts=[];state={};spawns=[[],[]];explosions=[[],[]];removes=[[],[]];primes=[[],[]]
 for observer,c in enumerate(raw['clients']):
  assert not c.get('failure') and c['events']
  for e in c['events']:
   assert e['name']!='damage_event','Peaceful actual explosion must cause no damage event'
   if e['name']=='update_health':assert health_packet(e)==20
   if e['name']=='spawn_entity':
    b=io.BytesIO(bytes.fromhex(e['hex']));entity=uint(b);assert len(b.read(16))==16;kind=uint(b);spawns[observer].append((e['at'],entity,kind))
   if e['name']=='entity_status':
    b=bytes.fromhex(e['hex']);assert len(b)==5;entity,status=struct.unpack('>iB',b);primes[observer].append((e['at'],entity,status))
   if e['name']=='entity_destroy':
    b=io.BytesIO(bytes.fromhex(e['hex']));n=uint(b);assert n<=65536;ids=[uint(b) for _ in range(n)];assert not b.read(1);removes[observer].append((e['at'],ids))
   if e['name']=='explosion':explosions[observer].append(e['at'])
   if observer==0:
    updates=inventory_changes(e)
    if updates:state.update(updates);inv.append((e['at'],state.copy()));receipts.append((e['at'],updates))
 clicks=[v for v in raw['sent'] if v['action'].get('window_click')];assert len(clicks)==10;rows=[]
 names=raw['clients'][0]['registry_names']['minecraft:enchantment']
 for i,((item,slot,damage,enchant),click) in enumerate(zip(armor.CASES['blast'],clicks)):
  begin=raw['start']+i*7.;end=begin+7.;times=[]
  for observer in [0,1]:
   events=[t for t in explosions[observer] if begin<=t<end];assert len(events)==1
   at=events[0];times.append(at);carts=[eid for t,eid,kind in spawns[observer] if begin<=t<at and kind==entities['tnt_minecart']];assert len(carts)==1
   # Реальный запал и удаление самой взорвавшейся вагонетки, не пустое ожидание.
   assert any(begin<=t<at and eid==carts[0] and status==70 for t,eid,status in primes[observer])
   assert any(at-.1<=t<end and carts[0] in ids for t,ids in removes[observer])
  at=min(times);moved=click['at'];assert at<moved<end
  first=next(s for t,s in reversed(inv) if t<at).get(slot);assert first and first['item']==items[item] and first['count']==1 and first.get('damage',0)==damage
  wanted={'minecraft:'+enchant:(3 if enchant=='unbreaking' else 4)} if enchant else {}
  assert {names[n]:v for n,v in first.get('enchantments',[])}==wanted
  final=next(s for t,s in reversed(inv) if t<end);assert final.get(slot) is None
  matches=[(n,s) for n,s in final.items() if s and s['item']==items[item]];assert len(matches)==1
  number,stack=matches[0];assert stack==first and number!=slot and any(moved<=t<end and updates.get(number)==stack for t,updates in receipts)
  rows.append({'trial':i,'item':item,'slot':slot,'enchantment':enchant,'initial_damage':damage,'final_damage':damage,'victim_damage':0.,'broken':[0,0],'source':None,'direct_roles':['no_damage','no_damage']})
 return rows


def inspect(raw,difficulty,items,entities):
 assert raw['scenario']==name(difficulty)
 assert any(c['command']=='difficulty '+difficulty for c in raw['commands'])
 if difficulty=='peaceful':return harmless(raw,items,entities)
 # Только имя сценария приводится к общей форме; байты/HP/слоты не меняются.
 view=dict(raw,scenario=armor.name('blast'))
 return [armor.normalize(r) for r in armor.inspect(view,'blast',items,entities)]


def analyze(output,recordings,data):
 items={v['name']:v['id'] for v in json.loads((data/'items.json').read_text())};entities={v['name']:v['id'] for v in json.loads((data/'entities.json').read_text())}
 expected=json.loads((ROOT/'tools/blackbox/ci/armor-difficulty-777.json').read_text())['expected'];result={}
 for d in DIFFICULTIES:
  result[d]={side:inspect(json.loads((recordings/(name(d)+'-'+side+'.json')).read_text()),d,items,entities) for side in ['official','ours']}
  for rows in result[d].values():assert rows==expected[d],d+' differs from frozen original'
 (output/'observations.json').write_text(json.dumps({'status':'strict-pass','sides':result},indent=2)+'\n')


def ci_run(output,work,data,binary,jar):
 prepare(output);shutil_pin=ROOT/'tools/blackbox/ci/armor-difficulty-777.json';(output/'expected-before-scale-fix.json').write_bytes(shutil_pin.read_bytes())
 command=[sys.executable,str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(work/'difficulty-recordings'),
  '--binary',str(binary),'--jar',str(jar),'--java','java','--official-port','26184','--core-port','26186','--parallel','--max-seconds','600','--isolate-vitals']+[name(d) for d in DIFFICULTIES]
 import shutil
 try:
  with (output/'recorder.log').open('w') as f:subprocess.run(command,stdout=f,stderr=subprocess.STDOUT,check=True,timeout=660)
  analyze(output,work/'difficulty-recordings',data)
  life=json.loads((work/'difficulty-recordings/lifecycle.json').read_text());assert not life['cleanup_errors'] and all(life['ports_free'].values())
 finally:
  for p in (work/'difficulty-recordings').glob('*.json'):shutil.copy2(p,output/p.name)
  for side in ['official','ours']:
   log=work/'difficulty-recordings'/side/'server.log'
   if log.exists():shutil.copy2(log,output/(side+'-server.log'))
