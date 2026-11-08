"""Регрессия: near-break UnbreakingIII+forcedThornsVII; только собственные packets777."""
from pathlib import Path
import hashlib
import io
import json
import struct
import sys

REPO=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(REPO/'tools/blackbox'))
from thorns_natural_sample import damage,health_packet,inventory_changes

NAME='pvp-thorns-break-unbreaking-observe-777'
COUNT=24


def prepare(root):
    root.mkdir(exist_ok=False)
    base=json.loads((REPO/'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text());actions=[]
    for i in range(COUNT):
        tick=i*32
        for who in ['$(bot)','$(bot2)']:actions.append({'after':tick,'cmd':'effect give '+who+' minecraft:instant_health 1 10'})
        actions.extend([{'after':tick+2,'cmd':'tp $(bot) $(3.5,0,3.5) 180 0'},
                        {'after':tick+2,'cmd':'tp $(bot2) $(3.5,0,1.5) 0 0'},
                        {'after':tick+4,'cmd':'item replace entity $(bot) armor.chest with minecraft:diamond_chestplate[damage=527,enchantments={thorns:7,unbreaking:3}]'},
                        {'after':tick+12,'attack':True,'bot':2}])
    base.update(name=NAME,about='Near-break chestplate/UnbreakingIII/forcedThornsVII. Броня, сломанная входящим ударом, не отражает; пережившая его отражает.',actions=actions,record_ticks=COUNT*32,random=[])
    raw=(json.dumps(base,ensure_ascii=False,indent=2)+'\n').encode();target=REPO/'tools/blackbox/scenarios'/f'{NAME}.json';assert not target.exists();target.write_bytes(raw);(root/'scenario.json').write_bytes(raw)
    (root/'criteria-before-recording.json').write_text(json.dumps({'trials':COUNT,'scenario_sha256':hashlib.sha256(raw).hexdigest(),'scope':'regression from paired24 original/core observations; forcedVII reflects iff armor did not break before incoming DamageEvent; HP20/real hit/correct cause/both observers/fresh damage527 ThornsVII UnbreakingIII required. Classify damage, final armor and break event ordering.'},indent=2)+'\n')


def analyze(root,recordings,data):
    items={x['name']:x['id'] for x in json.loads((data/'items.json').read_text())};out={}
    for side in ['official','ours']:
        raw=json.loads((recordings/f'{NAME}-{side}.json').read_text());assert raw['protocol']==777 and raw['isolate_vitals'];victim,attacker=[x['entity'] for x in raw['initial']]
        health=[[(raw['start']-.01,20.)],[(raw['start']-.01,20.)]];gear=[];hits=[[],[]];breaks=[[],[]]
        for observer,client in enumerate(raw['clients']):
            assert client['events'] and not client.get('failure');assert raw['initial'][observer]['health']==health_packet(raw['initial'][observer]['health_receipt'])==20
            for index,event in enumerate(client['events']):
                if event['name']=='update_health':health[observer].append((event['at'],health_packet(event)))
                if event['name']=='damage_event':
                    row=damage(event,client['registry_names']['minecraft:damage_type']);row['index']=index;hits[observer].append(row)
                if event['name']=='entity_status':
                    data=bytes.fromhex(event['hex']);assert len(data)==5
                    entity,status=struct.unpack('>iB',data)
                    if entity==victim and status==50:breaks[observer].append({'at':event['at'],'index':index})
                if observer==0:
                    slots=inventory_changes(event)
                    if 6 in slots:gear.append((event['at'],slots[6]))
        attacks=[x for x in raw['sent'] if x['action'].get('attack')];assert len(attacks)==COUNT;rows=[]
        names=raw['clients'][0]['registry_names']['minecraft:enchantment']
        for i,attack in enumerate(attacks):
            start=raw['start']+i*1.6;end=start+1.6;at=attack['at']
            before=[next(hp for t,hp in reversed(v) if t<at) for v in health];after=[next(hp for t,hp in reversed(v) if t<end) for v in health];assert before==[20.,20.] and 0<after[0]<20
            initial=next(s for t,s in reversed(gear) if t<at);final=next(s for t,s in reversed(gear) if t<end)
            assert initial and initial['item']==items['diamond_chestplate'] and initial['damage']==527
            assert {names[n]:v for n,v in initial['enchantments']}=={'minecraft:thorns':7,'minecraft:unbreaking':3}
            reflected=[];ordering=[]
            for observer in [0,1]:
                all_hits=[e for e in hits[observer] if start<=e['at']<end]
                incoming=[e for e in all_hits if e['entity']==victim and e['kind']=='minecraft:player_attack' and e['cause']==e['direct']==attacker+1]
                returned=[e for e in all_hits if e['entity']==attacker and e['kind']=='minecraft:thorns' and e['cause']==e['direct']==victim+1]
                broken=[e for e in breaks[observer] if start<=e['at']<end]
                assert len(incoming)==1 and len(returned)<=1 and len(all_hits)==len(incoming)+len(returned)
                assert len(broken)==int(final is None)
                reflected.append(bool(returned));ordering.append({'incoming_index':incoming[0]['index'],'reflection_index':returned[0]['index'] if returned else None,'break_index':broken[0]['index'] if broken else None})
            assert reflected[0]==reflected[1] and ((1<=20-after[1]<=5) if reflected[0] else after[1]==20)
            rows.append({'trial':i,'victim_hp':after[0],'attacker_hp':after[1],'reflection':reflected[0],'broken':final is None,'final_armor':final,'event_order':ordering})
        out[side]={'trials':rows,'reflections':sum(r['reflection'] for r in rows),'broken':sum(r['broken'] for r in rows),'bare_damage_with_reflection':sum(r['reflection'] and r['victim_hp']==19. for r in rows)}
    for value in out.values():
        check_semantics(value['trials'])
        assert 0<value['reflections']<COUNT, 'both armor-survived and armor-broke-first cohorts must be observed'
    (root/'observations.json').write_text(json.dumps({'status':'verified-unbreaking-break-semantic-controls','sides':out},indent=2)+'\n')
    print({s:{k:v for k,v in d.items() if k!='trials'} for s,d in out.items()})


def check_semantics(rows):
    for row in rows:
        owner=row['event_order'][0]
        broke_first=owner['break_index'] is not None and owner['break_index']<owner['incoming_index']
        assert row['reflection']==(not broke_first), ('reflection disagrees with actual incoming armor break',row)
        assert row['victim_hp']==(19. if broke_first else 19.304000854492188), ('armor mitigation disagrees with break order',row)


if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=['prepare','analyze']);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--recordings',type=Path);parser.add_argument('--data',type=Path);args=parser.parse_args()
    if args.mode=='prepare':prepare(args.output)
    else:analyze(args.output,args.recordings,args.data)
