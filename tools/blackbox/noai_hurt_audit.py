#!/usr/bin/env python3
"""Фиксированное hurt/control-наблюдение: валидность измерения, не паритет."""
import io
import json
import math
from pathlib import Path
from analyze777 import Decoder
from combat_ci import sha
from noai_ambient_audit import near, sound_name

SCENARIOS=[f'noai-cow-hurt-control-{i:02}' for i in range(1,9)]
SPAWN, ATTACK, END, RECORD = 10, 110, 410, 430
POSITIONS={'hurt':[3.5,0,1.5],'control':[7.5,0,1.5]}
SOUND='minecraft:entity.cow.ambient'
PACKETS={'spawn_entity','entity_metadata','damage_event','entity_destroy','entity_status',
         'entity_velocity','rel_entity_move','entity_move_look','entity_look','entity_teleport',
         'sync_entity_position','entity_head_rotation','sound_effect','entity_sound_effect','update_time'}


def expected_actions():
    return [{'after':SPAWN,'cmd':f'summon cow $({pos[0]},0,{pos[2]}) {{NoAI:1b,Age:0}}'} for pos in POSITIONS.values()]+[
        {'after':ATTACK,'attack':True,'which':0},{'after':END,'cmd':'kill @e[type=cow,distance=..48]'}]


def compatible_sound_position(wire, sample, origin):
    # SoundEffect — целые восьмые блока; наше отправление отсекает к нулю,
    # оригинал может округлять вниз. Относительное движение хранится с
    # шагом1/4096: только эта измеренная точность расширяет допустимый bin.
    uncertainty=sample['uncertainty']
    for encoded,relative,offset in zip(wire,sample['position'],origin):
        candidates={rounding((relative+offset+delta)*8)
                    for delta in (-uncertainty,0,uncertainty)
                    for rounding in (math.floor,math.trunc)}
        if encoded not in candidates:
            return False
    return True


def sound_position_candidates(cow,at):
    samples=sorted(cow['positions'],key=lambda row:row['at'])
    previous=[row for row in samples if row['at']<=at]
    candidates=previous[-1:]
    following=next((row for row in samples if row['at']>at),None)
    # Собственный tracker использует EVERY=3. Только ближайший следующий
    # измеренный снимок в этом окне допускает sound-before-movement порядок.
    # Больший сдвиг/пауза не угадываются по скорости: атрибуция отклоняется.
    if following is not None and following['at']-at<=3*.05+1e-9:
        candidates.append(following)
    return candidates


def audit_capture(raw,decoder,cow_type,sounds):
    errors=[]
    if raw.get('protocol')!=777 or len(raw.get('clients',[]))!=1:
        raise ValueError('one protocol777 client required')
    client=raw['clients'][0]
    if client.get('failure') or not client.get('events'):
        raise ValueError('failed/empty observer')
    start,origin=raw['start'],raw['origin']
    if len(raw.get('initial',[]))!=1 or type(raw['initial'][0].get('entity')) is not int:
        raise ValueError('single observer player identity required')
    player=raw['initial'][0]['entity']
    if [row.get('action') for row in raw.get('sent',[])]!=expected_actions():
        errors.append('fixed pair actions changed or missing')
    for row in raw.get('sent',[]):
        planned=start+row.get('action',{}).get('after',0)*.05
        if not math.isfinite(row.get('at',float('nan'))) or not planned-.001<=row['at']<=planned+.1:
            errors.append('command/attack delivery exceeds two ticks')
    cows={role:{'entity':None,'spawn':None,'metadata':[],'damage':[],'ambient':[],'hurt_sounds':[],
                'motion':[],'health':[],'removed':[],'statuses':[],'last_position':list(POSITIONS[role]),'positions':[]} for role in POSITIONS}
    identities={};ages=[];pending_sounds=[]
    for event in client['events']:
        name=event['name']
        if name not in PACKETS:
            continue
        at=event['at'];tick=(at-start)/.05
        if name=='update_time':
            data=bytes.fromhex(event['hex'])
            if len(data)<8:
                raise ValueError('truncated world age')
            ages.append({'at':at,'tick_nominal':tick,'age':int.from_bytes(data[:8],'big',signed=True)})
            continue
        stream=io.BytesIO(bytes.fromhex(event['hex']));value=decoder.read(stream,'packet_'+name)
        if stream.read():
            raise ValueError('trailing bytes: '+name)
        stamp={'at':at,'tick_nominal':tick,'packet':name,'value':value}
        if name=='spawn_entity' and value['type']==cow_type:
            relative=[value[key]-origin[index] for index,key in enumerate(('x','y','z'))]
            roles=[role for role,pos in POSITIONS.items() if near(relative,pos)]
            if len(roles)!=1 or not SPAWN<=tick<=SPAWN+2:
                errors.append('Cow spawn outside fixed position/two-tick delivery');continue
            role=roles[0]
            if cows[role]['entity'] is not None or value['entityId'] in identities:
                errors.append('duplicate Cow identity');continue
            cows[role]['entity']=value['entityId'];cows[role]['spawn']=stamp;identities[value['entityId']]=role
            cows[role]['positions'].append({'at':at,'position':relative,'uncertainty':0,'packet':'spawn_entity'})
            if role=='control' and (any(value.get(key,0) for key in ('yaw','pitch','headPitch')) or any(value.get('velocity',{}).values())):
                errors.append('control initial spawn velocity/rotation')
            continue
        if name=='entity_destroy':
            for entity in value['entityIds']:
                if entity in identities:
                    cows[identities[entity]]['removed'].append(stamp)
                    if tick<END-1:
                        errors.append('Cow removed before fixed pair ends')
        entity=value.get('entityId')
        role=identities.get(entity)
        if role is not None and SPAWN<=tick<END:
            cow=cows[role]
            if name=='entity_metadata':
                cow['metadata'].append(stamp)
                for entry in value['metadata']:
                    if entry['key']==9:
                        cow['health'].append({'at':at,'health':entry['value']})
                        if not math.isfinite(entry['value']) or entry['value']<=0:
                            errors.append(role+' Cow not alive')
                    if entry['key']==15 and not entry['value']&1:
                        errors.append(role+' NoAI disabled')
                    if entry['key']==4 and entry['value'] is True:
                        errors.append(role+' unexpectedly Silent')
                    if entry['key']==16 and entry['value'] is True:
                        errors.append(role+' unexpectedly baby')
            elif name=='damage_event':
                cow['damage'].append(stamp)
            elif name=='entity_status':
                cow['statuses'].append(stamp)
                if value.get('entityStatus')==3:
                    errors.append(role+' death status')
            elif name in {'entity_velocity','rel_entity_move','entity_move_look','entity_look','entity_teleport','sync_entity_position','entity_head_rotation'}:
                cow['motion'].append(stamp)
                if name in ('rel_entity_move','entity_move_look'):
                    for step in value['move']['steps']:
                        cow['last_position']=[a+b/4096 for a,b in zip(cow['last_position'],step['delta'])]
                elif name in ('entity_teleport','sync_entity_position'):
                    for pos in value.get('steps',[value]):
                        cow['last_position']=[pos[key]-origin[index]+(cow['last_position'][index]+origin[index] if value.get('flags',{}).get(key) else 0)
                                              for index,key in enumerate(('x','y','z'))]
                if name in ('rel_entity_move','entity_move_look','entity_teleport','sync_entity_position'):
                    cow['positions'].append({'at':at,'position':list(cow['last_position']),
                                             'uncertainty':1/4096 if name in ('rel_entity_move','entity_move_look') else 0,'packet':name})
                # Толчок/движение раненой коровы НЕ считаются ошибкой измерения.
                if role=='control':
                    if name=='entity_velocity' and any(value['velocity'].values()):
                        errors.append('control velocity')
                    if name in ('rel_entity_move','entity_move_look') and any(any(step['delta']) for step in value['move']['steps']):
                        errors.append('control relative movement')
                    if name in ('entity_teleport','sync_entity_position'):
                        for pos in value.get('steps',[value]):
                            if not near([pos[key]-origin[index] for index,key in enumerate(('x','y','z'))],POSITIONS['control']):
                                errors.append('control absolute movement')
                            if any(pos.get(key,0) for key in ('dx','dy','dz','yaw','pitch','headYaw')):
                                errors.append('control stepped velocity/rotation')
                        if any(value.get('flags',{}).values()) or any(value.get(key,0) for key in ('dx','dy','dz')):
                            errors.append('control relative teleport/velocity')
                    if any(value.get(key,0) for key in ('yaw','pitch','headYaw')):
                        errors.append('control rotation')
        if name in ('sound_effect','entity_sound_effect'):
            sound=sound_name(value['sound'],sounds)
            if sound not in (SOUND,'minecraft:entity.cow.hurt','minecraft:entity.cow.death') or not SPAWN<=tick<END:
                continue
            if name!='sound_effect':
                errors.append('unexpected entity-attached Cow sound');continue
            pending_sounds.append((sound,stamp))
    for sound,stamp in pending_sounds:
        value=stamp['value'];at=stamp['at']
        wire=[value[key] for key in ('x','y','z')]
        candidates={role:[sample for sample in sound_position_candidates(cow,at)
                          if compatible_sound_position(wire,sample,origin)]
                    for role,cow in cows.items()}
        roles=[role for role,samples in candidates.items() if samples]
        if len(roles)!=1:
            errors.append('Cow sound spatial attribution ambiguous' if roles else 'Cow sound spatial attribution unknown')
            continue
        role=roles[0];cow=cows[role]
        stamp['position_evidence']=candidates[role]
        if cow['entity'] is None or cow['spawn']['at']>at:
            errors.append('Cow sound before spawn')
        if value['soundCategory']!='neutral' or not near([value['volume']],[.4]) or not math.isfinite(value['pitch']) or not .8-1e-6<=value['pitch']<=1.2+1e-6:
            errors.append('adult Cow sound category/volume/pitch')
        if sound==SOUND:
            cow['ambient'].append(stamp)
        elif sound.endswith('.hurt'):
            cow['hurt_sounds'].append(stamp)
        else:
            errors.append('Cow death during pair')
    if not ages or ages[-1]['tick_nominal']<END:
        errors.append('capture ends before full pair observation')
    if any(b['age']<a['age'] for a,b in zip(ages,ages[1:])):
        errors.append('world age decreased during continuous pair')
    for role,cow in cows.items():
        initial=[entry for stamp in cow['metadata'] if stamp['tick_nominal']<=SPAWN+2 for entry in stamp['value']['metadata']]
        if cow['entity'] is None or not any(entry['key']==15 and entry['value']&1 for entry in initial):
            errors.append(role+' spawn/initial NoAI metadata missing')
        if not cow['health']:
            errors.append(role+' living health metadata missing')
        expected=1 if role=='hurt' else 0
        if len(cow['damage'])!=expected or len(cow['hurt_sounds'])!=expected:
            errors.append(role+' accepted damage/hurt sound count')
    hurt=cows['hurt']['damage']
    hit_tick=hurt[0]['tick_nominal'] if len(hurt)==1 else None
    if len(hurt)==1:
        hit=hurt[0]['value']
        # Actual777 native fixedyaw captures: PlayerAttack=34, причастные
        # entity IDs в DamageEvent передаются как id+1 на обеих сторонах.
        if hit.get('sourceTypeId')!=34 or hit.get('sourceCauseId')!=player+1 or hit.get('sourceDirectId')!=player+1:
            errors.append('accepted damage is not the observer fist attack')
        health=cows['hurt']['health']
        if health and not min(row['health'] for row in health)<health[0]['health']:
            errors.append('hurt Cow health did not decrease')
    control_health=cows['control']['health']
    if control_health and any(abs(row['health']-control_health[0]['health'])>1e-6 for row in control_health):
        errors.append('control health changed')
    if hit_tick is not None and not ATTACK<=hit_tick<=ATTACK+2:
        errors.append('accepted hurt outside attack delivery window')
    for role,cow in cows.items():
        times=[row['tick_nominal'] for row in cow['ambient']]
        after=[value for value in times if hit_tick is not None and value>=hit_tick]
        cow['intervoice_gaps_nominal_ticks']=[b-a for a,b in zip(times,times[1:])]
        cow['first_voice_after_actual_hurt_nominal_ticks']=after[0]-hit_tick if after else None
        cow['post_hurt_window_nominal_ticks']=END-hit_tick if hit_tick is not None else None
        cow['ambient_count']=len(times)
        cow['voices_before_actual_hurt']=sum(value<hit_tick for value in times) if hit_tick is not None else None
        cow['voices_after_actual_hurt']=len(after)
        before=[value for value in times if hit_tick is not None and value<hit_tick]
        cow['last_voice_before_actual_hurt_nominal_ticks']=hit_tick-before[-1] if before else None
    return {'errors':errors,'cows':cows,'world_age_samples':ages,'actual_hurt_nominal_tick':hit_tick,
            'window':{'spawn_nominal_tick':SPAWN,'attack_nominal_tick':ATTACK,'end_nominal_tick':END,
                      'warning':'Nominal ticks derive from packet arrival/50ms; preserve world-age anchors; no exact internal scheduling claim.'}}


def audit(folder,data):
    decoder=Decoder(json.loads((data/'protocol.json').read_text()))
    cow=next(value['id'] for value in json.loads((data/'entities.json').read_text()) if value['name']=='cow')
    sounds=json.loads((folder/'sound-observations.json').read_text())['sounds']
    captures={};errors=[]
    for scenario in SCENARIOS:
        for side in ('official','ours'):
            path=folder/f'{scenario}-{side}.json';raw=json.loads(path.read_text())
            if raw.get('scenario')!=scenario:
                raise ValueError('scenario identity mismatch')
            report=audit_capture(raw,decoder,cow,sounds);report['raw_sha256']=sha(path)
            captures[f'{scenario}/{side}']=report
            errors.extend(f'{scenario}/{side}: {error}' for error in report['errors'])
    summary={side:{role:[{'scenario':name, 'first_post_hurt_gap_nominal_ticks':captures[f'{name}/{side}']['cows'][role]['first_voice_after_actual_hurt_nominal_ticks'],
                         'post_hurt_window_nominal_ticks':captures[f'{name}/{side}']['cows'][role]['post_hurt_window_nominal_ticks'],
                         'intervoice_gaps_nominal_ticks':captures[f'{name}/{side}']['cows'][role]['intervoice_gaps_nominal_ticks']} for name in SCENARIOS]
                   for role in POSITIONS} for side in ('official','ours')}
    return {'status':'observation-invalid' if errors else 'observations-valid','errors':errors,'captures':captures,'observations_by_side':summary,
            'claim':'Eight fixed nonlethal hurt/control NoAI Cow pairs on each side. Reports observations and censoring only; no parity, exact reset constant or RNG/timing/count law claim. No comparison tolerances changed.'}
