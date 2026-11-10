#!/usr/bin/env python3
"""Строгая проверка реальных ведьминых зелий; не проверяет timing/RNG."""
import io, json, math
from pathlib import Path
from analyze777 import Decoder
from thorns_natural_sample import health_packet

KINDS = ('near','far','wall','creative','noai')
NAMES = [f'witch-bounded-{kind}-{i}' for kind in KINDS for i in range(1, 4 if kind in ('near','far') else 2)]

def require(condition, message):
    if not condition: raise ValueError(message)

def inspect(raw, protocol, entities, name):
    require(raw.get('protocol')==777 and raw.get('scenario')==name and raw.get('isolate_vitals') is True,'recording identity')
    require(len(raw.get('clients',[]))==1 and len(raw.get('initial',[]))==1,'one player required')
    c=raw['clients'][0];init=raw['initial'][0];require(not c.get('failure') and c.get('events'),'failed or empty client')
    require(init.get('health_receipt') and init.get('health')==health_packet(init['health_receipt'])==20.,'actual HP20 missing')
    player=init['entity'];start=raw['start'];origin=raw['origin'];sent=raw['sent'];kind=name.split('-')[2]
    require(all(abs(init['position'][j]-(origin[j]+v))<1e-5 for j,v in enumerate((.5,0.,.5))), 'initial position missing')
    scenario=json.loads((Path(__file__).resolve().parents[2]/'tools/blackbox/scenarios'/f'{name}.json').read_text())
    require([x['action'] for x in sent]==scenario['actions'], 'actual action sequence')
    require(len(sent)==2 and [x['action']['after'] for x in sent]==[10,610 if kind=='noai' else 410], 'action schedule')
    moves=c.get('moves',[])
    require(len(moves)>=2 and max(m['at'] for m in moves)>=start+(scenario['actions'][-1]['after']-25)*.05 and all(all(abs(m['position'][k]-(origin[j]+v))<1e-5 for j,(k,v) in enumerate((('x',.5),('y',0.),('z',.5)))) for m in moves),'actual player movement missing or left position')
    for s in sent:
        planned=start+s['action']['after']*.05
        require(math.isfinite(s['at']) and planned-.001<=s['at']<=planned+.1,'late action')
    require(not any(e['name'] in ('attack','use_entity') for e in c['outbound']),'unexpected player attack')
    dec=Decoder(protocol); mode=None;mode_after=[];witch=None;positions=[];moving=None;metadata=[];projectiles={};potion_data={};effects=[];damages=[];destroy=[];world_events=[];health=[];ages=[]
    for e in c['preroll']+c['events']:
        n=e['name'];t=(e['at']-start)/.05
        if n=='update_time':ages.append(t);continue
        if n not in ('game_state_change','spawn_entity','entity_metadata','entity_destroy','entity_effect','remove_entity_effect','update_health','damage_event','world_event','rel_entity_move','entity_move_look','sync_entity_position','entity_teleport'):continue
        stream=io.BytesIO(bytes.fromhex(e['hex']));v=dec.read(stream,'packet_'+n);require(not stream.read(),f'{n}: trailing bytes')
        if n=='game_state_change' and v['reason']=='change_game_mode':
            if t<0:mode=v['gameMode']
            else:mode_after.append((t,v['gameMode']))
        elif n=='spawn_entity' and 0<=t<=620 and v['type']==entities['witch']:
            require(witch is None,'more than one witch');witch=v['entityId'];moving=[v['x'],v['y'],v['z']];positions.append((t,moving[:]))
            require(8<=t<=13 and abs(v['y']-origin[1])<1e-5,'witch spawn/height')
            expected_x=9.5 if kind=='far' else 2.5 if kind=='near' else 7.5
            require(abs(v['x']-origin[0]-expected_x)<1e-5 and abs(v['z']-origin[2]-.5)<1e-5,'witch spawn position')
        elif n=='spawn_entity' and v['type'] in (entities['splash_potion'],entities['lingering_potion']) and t>=0:
            require(v['entityId'] not in projectiles,'duplicate projectile');projectiles[v['entityId']]={'tick':t,'owner':v['objectData'],'type':v['type']}
        elif n=='entity_metadata' and v['entityId']==witch and t>=0:metadata.extend((t,x) for x in v['metadata'])
        elif n=='entity_metadata' and v['entityId'] in projectiles:
            for x in v['metadata']:
                if x['key']!=8:continue
                item=x['value'];require(item['itemCount']==1 and item['itemId']==entities['splash_item'],'potion item')
                parts=[p for p in item['components'] if p['type']=='potion_contents'];require(len(parts)==1,'potion metadata missing')
                contents=parts[0]['data']
                require(contents.get('customColor') is None and not contents.get('customEffects',[]) and contents.get('customName') is None,'unexpected custom potion')
                potion_data[v['entityId']]=contents['potionId']
        elif n=='entity_destroy':destroy.append((t,v['entityIds']))
        elif n=='entity_effect' and v['entityId']==player:effects.append((t,v))
        elif n=='update_health':health.append((t,v))
        elif n=='damage_event' and v['entityId']==player:damages.append((t,v))
        elif n=='world_event' and v['effectId'] in (2002,2007):world_events.append((t,v))
        elif witch is not None and v.get('entityId')==witch and moving is not None:
            if n in ('rel_entity_move','entity_move_look'):
                for step in v['move']['steps']:
                    moving=[x+delta/4096 for x,delta in zip(moving,step['delta'])];positions.append((t,moving[:]))
            elif n=='sync_entity_position':
                for step in v.get('steps',[v]):
                    require(all(k in step for k in ('x','y','z')),'missing position');moving=[step[k] for k in ('x','y','z')];positions.append((t,moving[:]))
            elif n=='entity_teleport':
                require(not any(v.get('flags',{}).values()),'relative teleport not supported in control')
                moving=[v[k] for k in ('x','y','z')];positions.append((t,moving[:]))
    end=610 if kind=='noai' else 410
    require(mode==(1. if kind=='creative' else 0.),f'actual mode missing/differs: {mode}')
    require(all(v==mode for t,v in mode_after if t<end), 'mode changed during observation')
    require(any(t<0 and v['effectId']==10 and v['amplifier']==4 and v['duration']>0 for t,v in effects), 'Resistance V setup packet missing')
    require(witch is not None and len(positions)>0,'witch absent')
    require(ages and max(ages)>=end-15,'full window missing')
    require(not any(witch in ids and t<end-1 for t,ids in destroy),'witch removed early')
    receipt=init['health_receipt']
    ready=[e for e in c['preroll'] if e['name']=='update_health']
    require(receipt['at']<start and ready and ready[-1]['hex']==receipt['hex'] and abs(ready[-1]['at']-receipt['at'])<.01, 'HP receipt not final preroll health')
    require(not any(v['health']<=0 for t,v in health if ready[-1]['at']<=start+t*.05 and t<end),'player died after readiness')
    require(any(x['key']==9 and x['value']>0 for _,x in metadata),'witch HP metadata missing')
    require(not any(x['key']==9 and x['value']<=0 and t<end-1 for t,x in metadata),'witch died during observation')
    if kind=='noai': require(any(x['key']==15 and x['value']&1 for _,x in metadata),'NoAI metadata missing')
    if kind=='wall':
        # Стена x=origin+3, z=origin-8..+9, y=origin..+5.
        require(all(p[0]>=origin[0]+4.25 and origin[2]-7.75<=p[2]<=origin[2]+8.75 and abs(p[1]-origin[1])<2 for _,p in positions),'wall bypass/out of sight')
    else:
        require(all(abs(p[1]-origin[1])<2 for _,p in positions),'witch left floor level')
        if kind in ('near','far'):
            # Для положительной сцены нужен гарантированный общий пол.
            limit=12 if kind=='far' else 8
            require(all(origin[0]-7.5<=p[0]<=origin[0]+limit+.5 and origin[2]-7.5<=p[2]<=origin[2]+8.5 for _,p in positions),'witch left prepared floor')
    require(all(v['owner']==witch and v['type']==entities['splash_potion'] and ident in potion_data for ident,v in projectiles.items()),'projectile owner/type/metadata')
    require(all(t>=10 and t<end for t in [v['tick'] for v in projectiles.values()]),'projectile outside window')
    drinking=sorted((t,x['value']) for t,x in metadata if x['key']==17)
    for projectile in projectiles.values():
        preceding=[v for t,v in drinking if t<=projectile['tick']]
        require(not preceding or preceding[-1] is not True, 'potion spawned during observed drinking')
    valid_impacts=[]
    used_events=set()
    for ident,v in projectiles.items():
        removals=[t for t,ids in destroy if ident in ids and t>=v['tick']]
        require(len(removals)<=1,'duplicate projectile removal')
        if not removals:continue
        t=removals[0]
        choices=[(abs(at-t),i) for i,(at,ev) in enumerate(world_events) if i not in used_events and abs(at-t)<=2 and (ev['effectId']==2007 if potion_data[ident]==26 else ev['effectId']==2002)]
        if choices:
            _,selected=min(choices);used_events.add(selected);valid_impacts.append((ident,t,potion_data[ident]))
    effect_positive=any(v['effectId']==18 and any(p==28 and abs(t-impact)<=2 for _,impact,p in valid_impacts) for t,v in effects)
    harming=any(v['sourceTypeId']==23 and v['sourceCauseId']==witch+1 and any(p==26 and v['sourceDirectId']==ident+1 and abs(t-impact)<=2 for ident,impact,p in valid_impacts) for t,v in damages)
    if kind in ('wall','creative','noai'):require(not projectiles,'control spawned potion')
    return {'case':name,'witch':witch,'player':player,'mode':mode,'projectiles':projectiles,'potion_ids':potion_data,'impacts':valid_impacts,'poison_effect':effect_positive,'harming_cause_direct':harming,'observed_drinking':any(value is True for _,value in drinking),'positions':len(positions),'last_age_tick':max(ages)}

def audit_pair(recordings, protocol, entity_rows, item_rows):
    entities={x['name']:x['id'] for x in entity_rows};entities['splash_item']=next(x['id'] for x in item_rows if x['name']=='splash_potion')
    sides={}
    invalid={'official':[],'ours':[]}
    for side in ('official','ours'):
        rows=[]
        for name in NAMES:
            try:
                raw=json.loads((Path(recordings)/f'{name}-{side}.json').read_text());rows.append(inspect(raw,protocol,entities,name))
            except Exception as error:
                invalid[side].append({'case':name,'error':repr(error)})
        sides[side]=rows
    if invalid['official'] or invalid['ours']:
        return {'status':'inconclusive-official-invalid' if invalid['official'] else 'candidate-invalid', 'invalid':invalid,'sides':sides,'limits':['invalid recordings are never counted as absent events']}
    missing = {}
    for side,rows in sides.items():
        missing[side]=[]
        for kind in ('near','far'):
            subset=[r for r in rows if r['case'].startswith(f'witch-bounded-{kind}-')]
            if not any(r['poison_effect'] for r in subset): missing[side].append(kind+': no real poison impact/effect')
            if not any(r['harming_cause_direct'] for r in subset): missing[side].append(kind+': no harming damage with both owners')
        if not any(r['observed_drinking'] for r in rows if r['case'].startswith('witch-bounded-near-')): missing[side].append('near: no actual drinking control')
    status = 'inconclusive-official-control' if missing['official'] else 'candidate-fail' if missing['ours'] else 'strict-bounded-pass'
    return {'status':status,'missing':missing,'sides':sides,'limits':['no distribution/timing or complete AI parity','death messages not covered']}
