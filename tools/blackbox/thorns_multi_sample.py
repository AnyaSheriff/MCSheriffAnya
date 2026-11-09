#!/usr/bin/env python3
"""Несколько деталей с ШипамиVII: реальный урон и согласие обоих наблюдателей777."""
import argparse
import hashlib
import io
import json
import os
import shutil
import subprocess
from pathlib import Path
import sys
from thorns_natural_sample import damage,health_packet,inventory_changes,uint

ROOT=Path(__file__).resolve().parents[2]
NAME='pvp-thorns-multi-armor-observe-777'
PIECES=[1,2,4]*8
GEAR=[('head','diamond_helmet',5),('chest','diamond_chestplate',6),('legs','diamond_leggings',7),('feet','diamond_boots',8)]


def prepare(output):
    output.mkdir(exist_ok=False,parents=True)
    base=json.loads((ROOT/'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text());actions=[]
    for i,count in enumerate(PIECES):
        tick=i*32
        for who in ['$(bot)','$(bot2)']:actions.append({'after':tick,'cmd':'effect give '+who+' minecraft:instant_health 1 10'})
        actions.extend([{'after':tick+2,'cmd':'tp $(bot) $(3.5,0,3.5) 180 0'}, {'after':tick+2,'cmd':'tp $(bot2) $(3.5,0,1.5) 0 0'}])
        for index,(slot,item,_) in enumerate(GEAR):
            value='minecraft:'+item+'[damage=0,enchantments={thorns:7}]' if index<count else 'minecraft:air'
            actions.append({'after':tick+4,'cmd':'item replace entity $(bot) armor.'+slot+' with '+value})
        actions.append({'after':tick+12,'attack':True,'bot':2})
    base['setup'].insert(2,'effect clear $(bot)');base['setup'].insert(3,'effect clear $(bot2)')
    base.update(name=NAME,about='1/2/4 здоровых деталей ThornsVII; голый атакующий. HP20 и правильный causal damage обязательны; RNG между серверами не приравнивается.',actions=actions,record_ticks=len(PIECES)*32,random=[])
    raw=(json.dumps(base,ensure_ascii=False,indent=2)+'\n').encode();target=ROOT/'tools/blackbox/scenarios'/f'{NAME}.json';assert not target.exists();target.write_bytes(raw)
    (output/'scenario.json').write_bytes(raw)
    criteria={'schema':'multi-thorns777-v1','pieces':PIECES,'scenario_sha256':hashlib.sha256(raw).hexdigest(),'positive':'bothHP20 before genuine bare-fist hit; fresh exact armor/ThornsVII; one incoming attributed player_attack both observers; victimHP falls; no failure/decode gaps','reflection':'forced7: at least one thorns return, correct owner/direct, attackerHP falls1..5*pieces; counts must agree between observers; fresh single-piece control exactly1. Discovery writes disagreements, strict validation rejects them. No equality of random damage/count across independent servers. All armor wear captured.'}
    (output/'criteria-before-recording.json').write_text(json.dumps(criteria,ensure_ascii=False,indent=2)+'\n')


def inspect(raw,items):
    assert raw['protocol']==777 and raw['isolate_vitals'];victim,attacker=[v['entity'] for v in raw['initial']]
    health=[[(raw['start']-.001,20.)],[(raw['start']-.001,20.)]];gear=[[],[]];hits=[[],[]];animations=[[],[]]
    for observer,client in enumerate(raw['clients']):
        assert not client.get('failure') and raw['initial'][observer]['health']==health_packet(raw['initial'][observer]['health_receipt'])==20
        state={s:None for _,_,s in GEAR}
        for event in client['preroll']+client['events']:
            if event['name']=='update_health':health[observer].append((event['at'],health_packet(event)))
            if event['name']=='damage_event':hits[observer].append(damage(event,client['registry_names']['minecraft:damage_type']))
            if event['name']=='hurt_animation':animations[observer].append((event['at'],uint(io.BytesIO(bytes.fromhex(event['hex'])))))
            updates=inventory_changes(event)
            if any(s in updates for _,_,s in GEAR):
                state.update({s:updates[s] for _,_,s in GEAR if s in updates});gear[observer].append((event['at'],state.copy()))
    attacks=[e for e in raw['sent'] if e['action'].get('attack')];assert len(attacks)==len(PIECES)
    names=raw['clients'][0]['registry_names']['minecraft:enchantment'];rows=[]
    for i,(attack,count) in enumerate(zip(attacks,PIECES)):
        start=raw['start']+i*1.6;end=start+1.6;at=attack['at']
        before=[next(hp for t,hp in reversed(v) if t<at) for v in health];after=[next(hp for t,hp in reversed(v) if t<end) for v in health]
        assert before==[20.,20.] and 0<after[0]<20 and 0<after[1]<20,(i,before,after)
        initial=next(s for t,s in reversed(gear[0]) if t<at);final=next(s for t,s in reversed(gear[0]) if t<end)
        for index,(_,item,slot) in enumerate(GEAR):
            stack=initial[slot]
            if index<count:
                assert stack and stack['item']==items[item] and stack.get('damage',0)==0
                assert {names[n]:v for n,v in stack.get('enchantments',[])}=={'minecraft:thorns':7}
                assert final[slot] and final[slot]['item']==items[item]
            else:assert stack is None
        reflected=[]
        for observer in [0,1]:
            events=[e for e in hits[observer] if start<=e['at']<end]
            incoming=[e for e in events if e['entity']==victim and e['kind']=='minecraft:player_attack' and e['cause']==e['direct']==attacker+1]
            returned=[e for e in events if e['entity']==attacker and e['kind']=='minecraft:thorns' and e['cause']==e['direct']==victim+1]
            assert len(incoming)==1 and len(events)==len(incoming)+len(returned) and 1<=len(returned)<=count,(i,observer,events)
            reflected.append(len(returned))
        if count==1:assert reflected==[1,1]
        assert 1<=20-after[1]<=count*5
        rows.append({'trial':i,'pieces':count,'victim_damage':20-after[0],'reflected_damage':20-after[1],'reflections_by_observer':reflected,'observers_agree':reflected[0]==reflected[1],'attacker_hurt_animations':sum(start<=t<end and entity==attacker for t,entity in animations[1]),'wear':{s:final[s]['damage'] for _,_,s in GEAR[:count]}})
    return {'trials':rows,'observer_mismatches':sum(not r['observers_agree'] for r in rows)}



def validate(rows):
    # Усилено после исходных paired24: у оригинала всегда один возврат/анимация,
    # даже когда следующий бросок больше и здоровье получает только разницу.
    for r in rows:
        assert r['reflections_by_observer']==[1,1],r
        assert r['attacker_hurt_animations']==1,r
        assert 1<=r['reflected_damage']<=5,r
        assert len(r['wear'])==r['pieces'] and all(v==3 for v in r['wear'].values()),r


def analyze(output,recordings,data,strict=False):
    items={v['name']:v['id'] for v in json.loads((data/'items.json').read_text())};out={}
    for side in ['official','ours']:out[side]=inspect(json.loads((recordings/f'{NAME}-{side}.json').read_text()),items)
    assert [r['victim_damage'] for r in out['official']['trials']]==[r['victim_damage'] for r in out['ours']['trials']]
    result={'status':'recorded-not-yet-validated' if strict else 'discovery-no-parity-claim','sides':out}
    (output/'observations.json').write_text(json.dumps(result,indent=2)+'\n')
    if strict:
        for row in out.values():validate(row['trials'])
        result['status']='strict-pass'
        (output/'observations.json').write_text(json.dumps(result,indent=2)+'\n')
    print({side:row['observer_mismatches'] for side,row in out.items()})


def ci_run(output):
    assert os.environ.get('GITHUB_ACTIONS')=='true'
    import combat_ci
    from thorns_natural_sample import ci_run as single_controls
    output.mkdir(exist_ok=False)
    single_controls(output/'single-controls')
    work=Path(os.environ['RUNNER_TEMP'])/'mcsa-multi-thorns';work.mkdir(exist_ok=False)
    data=work/'data';data.mkdir()
    pin=json.loads((ROOT/'tools/blackbox/ci/protection-edges-777.json').read_text())
    combat_ci.fetch(pin['official_jar'],work/'server.jar')
    for entry in pin['json_files']:
        if entry['file'] in ['protocol.json','items.json','blocks.json','entities.json']:combat_ci.fetch(entry,data/entry['file'])
    multi=output/'multi';prepare(multi)
    command=[sys.executable,str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(work/'recordings'),
        '--binary',str(ROOT/'target/release/mcsheriffanya'),'--jar',str(work/'server.jar'),'--java','java',
        '--official-port','26180','--core-port','26182','--parallel','--max-seconds','600','--isolate-vitals',NAME]
    try:
        with (multi/'recorder.log').open('w') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=660)
        analyze(multi,work/'recordings',data,True)
        lifecycle=json.loads((work/'recordings/lifecycle.json').read_text());assert not lifecycle['cleanup_errors'] and all(lifecycle['ports_free'].values())
    finally:
        for p in (work/'recordings').glob('*.json'):shutil.copy2(p,multi/p.name)
        for side in ['official','ours']:
            source=work/'recordings'/side/'server.log'
            if source.exists():shutil.copy2(source,multi/(side+'-server.log'))
    (output/'run.json').write_text(json.dumps({'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
        'runtime_sha256':hashlib.sha256((ROOT/'target/release/mcsheriffanya').read_bytes()).hexdigest(),
        'multi_status':'strict-pass','single_controls':'see independently preserved child evidence'},indent=2)+'\n')


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['prepare','analyze','ci']);p.add_argument('--output',type=Path,required=True);p.add_argument('--recordings',type=Path);p.add_argument('--data',type=Path);p.add_argument('--strict',action='store_true');a=p.parse_args()
    if a.mode=='prepare':prepare(a.output)
    elif a.mode=='ci':ci_run(a.output)
    else:analyze(a.output,a.recordings,a.data,a.strict)
