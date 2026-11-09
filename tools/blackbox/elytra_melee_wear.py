#!/usr/bin/env python3
"""Элитры в нагрудном слоте: износ от настоящего удара и независимый inventory echo777."""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
import struct
from thorns_natural_sample import damage,health_packet,inventory_changes

ROOT=Path(__file__).resolve().parents[2]
NAME='pvp-elytra-melee-wear-observe-777'
CASES=[('diamond_chestplate',0,0),('elytra',0,0),('elytra',431,0),('elytra',431,3)]*2
INTERVAL=32


def prepare(output):
    output.mkdir(parents=True,exist_ok=False)
    scenario=json.loads((ROOT/'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text());actions=[]
    for i,(item,initial,level) in enumerate(CASES):
        t=i*INTERVAL
        for player in ['$(bot)','$(bot2)']:
            actions.extend([{'after':t,'cmd':'clear '+player},{'after':t,'cmd':'effect give '+player+' minecraft:instant_health 1 10'}])
        actions.extend([{'after':t+2,'cmd':'tp $(bot) $(3.5,0,3.5) 180 0'}, {'after':t+2,'cmd':'tp $(bot2) $(3.5,0,1.5) 0 0'}])
        specification='minecraft:'+item+'[damage='+str(initial)+(',enchantments={unbreaking:3}' if level else '')+']'
        actions.extend([{'after':t+4,'cmd':'item replace entity $(bot) armor.chest with '+specification},
                        {'after':t+12,'attack':True,'bot':2},
                        {'after':t+20,'window_click':{'inventory':True,'slot':6,'mode':1}}])
    scenario.update(name=NAME,about='Обычные кулаки, контрольный нагрудник и элитры; shiftclick после удара даёт серверный предмет в инвентаре. Discovery не объявляет PASS.',actions=actions,record_ticks=len(CASES)*INTERVAL,random=[])
    scenario['setup'].insert(2,'effect clear $(bot)');scenario['setup'].insert(3,'effect clear $(bot2)')
    raw=(json.dumps(scenario,ensure_ascii=False,indent=2)+'\n').encode();target=ROOT/'tools/blackbox/scenarios'/f'{NAME}.json';assert not target.exists();target.write_bytes(raw);(output/'scenario.json').write_bytes(raw)
    criteria={'schema':'elytra-melee777-v1','cases':CASES,'interval':INTERVAL,'scenario_sha256':hashlib.sha256(raw).hexdigest(),
              'positive':'HP20 each from actual packets before every real bare-fist hit; one correctly attributed player_attack for each observer; attackerHP20; exact initial chest item/damage/enchantment. Shiftclick after hit must produce server inventory item outside chest, or a real observed break with both statuses and explicit empty chest. No inference from missing packets.',
              'control':'Fresh diamond_chestplate loses1. Victim HP arrays must match across both sides. No equalization of independent RNG.',
              'discovery':'Record actual Elytra item/wear/break behavior BEFORE changing gameplay. Not a full damage-source/equippable-component/flight proof.'}
    (output/'criteria-before-recording.json').write_text(json.dumps(criteria,ensure_ascii=False,indent=2)+'\n')


def inspect(raw,items):
    assert raw['scenario']==NAME and raw['protocol']==777 and raw['isolate_vitals']
    assert len(raw['initial'])==len(raw['clients'])==2
    victim,attacker=[v['entity'] for v in raw['initial']]
    for initial in raw['initial']:assert initial['health']==health_packet(initial['health_receipt'])==20
    health=[[(raw['start']-.001,20.)],[(raw['start']-.001,20.)]];hits=[[],[]];breaks=[[],[]];inventory=[];state={};receipts=[]
    for observer,client in enumerate(raw['clients']):
        assert not client.get('failure') and client['events']
        for event in client['preroll']+client['events']:
            if event['name']=='update_health':health[observer].append((event['at'],health_packet(event)))
            if event['name']=='damage_event':hits[observer].append(damage(event,client['registry_names']['minecraft:damage_type']))
            if event['name']=='entity_status':
                payload=bytes.fromhex(event['hex']);assert len(payload)==5;entity,status=struct.unpack('>iB',payload)
                if entity==victim and status==50:breaks[observer].append(event['at'])
            if observer==0:
                changes=inventory_changes(event)
                if changes:
                    state.update(changes);inventory.append((event['at'],state.copy()));receipts.append((event['at'],changes))
    attacks=[e for e in raw['sent'] if e['action'].get('attack')];clicks=[e for e in raw['sent'] if e['action'].get('window_click')];assert len(attacks)==len(clicks)==len(CASES)
    registry=raw['clients'][0]['registry_names']['minecraft:enchantment'];rows=[]
    for i,((item,initial,level),attack,click) in enumerate(zip(CASES,attacks,clicks)):
        start=raw['start']+i*INTERVAL*.05;end=start+INTERVAL*.05;at=attack['at'];moved=click['at']
        assert attack['action']=={'after':i*INTERVAL+12,'attack':True,'bot':2}
        assert click['action']=={'after':i*INTERVAL+20,'window_click':{'inventory':True,'slot':6,'mode':1}}
        before=[next(h for t,h in reversed(v) if t<at) for v in health];after=[next(h for t,h in reversed(v) if t<end) for v in health]
        assert before==[20.,20.] and 0<after[0]<20 and after[1]==20.,(i,before,after)
        first=next(s for t,s in reversed(inventory) if t<at).get(6);assert first and first['item']==items[item] and first.get('damage',0)==initial
        assert {registry[n]:v for n,v in first.get('enchantments',[])}==({'minecraft:unbreaking':3} if level else {})
        for observer in [0,1]:
            events=[e for e in hits[observer] if start<=e['at']<end];assert len(events)==1
            event=events[0];assert event['entity']==victim and event['kind']=='minecraft:player_attack' and event['cause']==event['direct']==attacker+1
        final=next(s for t,s in reversed(inventory) if t<end);assert final.get(6) is None,(i,'chest did not move',final.get(6))
        matches=[(slot,stack) for slot,stack in final.items() if stack and stack['item']==items[item]];broken=[sum(start<=t<end for t in v) for v in breaks]
        if matches:
            assert len(matches)==1 and broken==[0,0]
            slot,stack=matches[0];assert slot!=6 and any(moved<=t<end and c.get(slot)==stack for t,c in receipts),'No post-hit authoritative item echo'
            final_damage=stack.get('damage',0);assert final_damage>=initial
        else:
            assert broken==[1,1] and any(at<=t<moved and 6 in c and c[6] is None for t,c in receipts),'Missing item without actual breaking receipt'
            final_damage=None
        if item=='diamond_chestplate':assert final_damage==1
        rows.append({'trial':i,'item':item,'initial_damage':initial,'unbreaking':level,'final_damage':final_damage,'broken_by_observer':broken,'victim_damage':20-after[0],'post_hit_item_echo':bool(matches)})
    return rows


def validate(rows):
    assert len(rows)==len(CASES)
    for i,(row,(item,initial,level)) in enumerate(zip(rows,CASES)):
        assert (row['trial'],row['item'],row['initial_damage'],row['unbreaking'])==(i,item,initial,level)
        assert row['final_damage']==(1 if item=='diamond_chestplate' else initial),row
        assert row['broken_by_observer']==[0,0] and row['post_hit_item_echo'],row


def analyze(output,recordings,data,strict=False):
    items={v['name']:v['id'] for v in json.loads((data/'items.json').read_text())};sides={s:inspect(json.loads((recordings/f'{NAME}-{s}.json').read_text()),items) for s in ['official','ours']}
    assert [r['victim_damage'] for r in sides['official']]==[r['victim_damage'] for r in sides['ours']]
    result={'status':'recorded-not-yet-validated' if strict else 'discovery-no-parity-claim','sides':sides};(output/'observations.json').write_text(json.dumps(result,indent=2)+'\n');
    if strict:
        for rows in sides.values():validate(rows)
        result['status']='strict-pass';(output/'observations.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


def ci_run(output):
    assert os.environ.get('GITHUB_ACTIONS')=='true'
    import combat_ci
    from thorns_multi_sample import ci_run as prior_controls
    output.mkdir(exist_ok=False)
    work=Path(os.environ['RUNNER_TEMP'])/'mcsa-elytra-melee';work.mkdir(exist_ok=False)
    data=work/'data';data.mkdir()
    pin=json.loads((ROOT/'tools/blackbox/ci/protection-edges-777.json').read_text())
    combat_ci.fetch(pin['official_jar'],work/'server.jar')
    for entry in pin['json_files']:
        if entry['file'] in ['protocol.json','items.json','blocks.json','entities.json']:combat_ci.fetch(entry,data/entry['file'])
    probe=output/'elytra';prepare(probe)
    # Нормативный результат заморожен после исходного оригинала, до Rust-правки.
    shutil.copy2(ROOT/'tools/blackbox/ci/elytra-melee-777.json',probe/'expected-before-candidate.json')
    binary=ROOT/'target/release/mcsheriffanya'
    manifest={'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
              'runtime_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'status':'running'}
    command=[sys.executable,str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),'--output',str(work/'recordings'),
        '--binary',str(binary),'--jar',str(work/'server.jar'),'--java','java',
        '--official-port','26190','--core-port','26192','--parallel','--max-seconds','240','--isolate-vitals',NAME]
    try:
        with (probe/'recorder.log').open('w') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=300)
        analyze(probe,work/'recordings',data,True)
        lifecycle=json.loads((work/'recordings/lifecycle.json').read_text());assert not lifecycle['cleanup_errors'] and all(lifecycle['ports_free'].values())
        # Старые точные HP/fire/Thorns и natural/UB/multi проверки не ослабляются.
        names=['pvp-fire-protection-control-777','pvp-fire-protection-iv-777','pvp-thorns-control-777','pvp-thorns-break-777']
        old=output/'old-four';old.mkdir()
        second=command[:-1]+names;second[second.index('--output')+1]=str(work/'old-four-recordings')
        with (old/'recorder.log').open('w') as log:subprocess.run(second,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=300)
        from protection_edges_ci import semantics
        result=semantics(work/'old-four-recordings',data)
        (old/'semantics.json').write_text(json.dumps(result,indent=2)+'\n');assert result['status']=='verified-semantic-controls',result
        lifecycle=json.loads((work/'old-four-recordings/lifecycle.json').read_text());assert not lifecycle['cleanup_errors'] and all(lifecycle['ports_free'].values())
        prior_controls(output/'thorns-regressions')
        manifest['status']='strict-pass'
    finally:
        for directory,target in [('recordings',probe),('old-four-recordings',output/'old-four')]:
            target.mkdir(exist_ok=True)
            for p in (work/directory).glob('*.json'):shutil.copy2(p,target/p.name)
            for side in ['official','ours']:
                log=work/directory/side/'server.log'
                if log.exists():shutil.copy2(log,target/(side+'-server.log'))
        if manifest['status']=='running':manifest['status']='failed'
        (output/'run.json').write_text(json.dumps(manifest,indent=2)+'\n')


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['prepare','analyze','ci']);p.add_argument('--output',type=Path,required=True);p.add_argument('--recordings',type=Path);p.add_argument('--data',type=Path);p.add_argument('--strict',action='store_true');args=p.parse_args()
    if args.mode=='prepare':prepare(args.output)
    elif args.mode=='ci':ci_run(args.output)
    else:analyze(args.output,args.recordings,args.data,args.strict)
