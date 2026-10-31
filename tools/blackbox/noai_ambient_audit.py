#!/usr/bin/env python3
"""Ограниченное доказательство голоса NoAI Cow, не timing/RNG parity."""
import io
import json
import math
from pathlib import Path
from analyze777 import Decoder
from combat_ci import sha, write_json

SCENARIOS = ['noai-cow-ambient', 'noai-cow-ambient-silent']
WINDOWS = [(30, 630), (650, 1250), (1270, 1870)]
POSITION = [4.5, 0, 3.5]
# Это предел доставки команды/появления, не допуск случайного звука.
SPAWN_LATENCY_TICKS = 2
SOUND = 'minecraft:entity.cow.ambient'
PACKETS = {'spawn_entity', 'entity_metadata', 'entity_velocity', 'rel_entity_move',
           'entity_move_look', 'entity_look', 'entity_teleport', 'sync_entity_position',
           'entity_head_rotation', 'sound_effect', 'entity_sound_effect', 'entity_destroy'}


def near(actual, expected):
    return len(actual) == len(expected) and all(math.isfinite(a) and abs(a-b) < 1e-6 for a,b in zip(actual, expected))


def audit_capture(raw, decoder, cow_type, sounds, silent):
    errors, observations = [], []
    windows = [{'start_tick':a, 'end_tick':b, 'spawns':[], 'sounds':[], 'metadata':[], 'metadata_packets':[]} for a,b in WINDOWS]
    if raw.get('protocol') != 777 or len(raw.get('clients', [])) != 1:
        raise ValueError('one protocol777 client required')
    client = raw['clients'][0]
    if client.get('failure') or not client.get('events'):
        raise ValueError('failed/empty capture')
    origin, start = raw['origin'], raw['start']
    # Последний update_time доказывает, что клиент слушал до конца окна.
    if max(e['at'] for e in client['events'] if e['name']=='update_time') < start + WINDOWS[-1][1]*.05:
        errors.append('capture ends before final 600-tick window ends')
    if any(e.get('name') in ('attack','use_entity') for e in client.get('outbound',[])):
        errors.append('unexpected outbound interaction/attack')
    expected_actions = []
    for a,b in WINDOWS:
        expected_actions.extend([{'after':a,'cmd':'summon cow $(4.5,0,3.5) {NoAI:1b,Age:0'+(',Silent:1b' if silent else '')+'}'},
                                 {'after':b,'cmd':'kill @e[type=cow,distance=..48]'}])
    for sent in raw.get('sent',[]):
        planned=start+sent.get('action',{}).get('after',0)*.05
        if not math.isfinite(sent.get('at',float('nan'))) or not planned-.001 <= sent['at'] <= planned+SPAWN_LATENCY_TICKS*.05:
            errors.append('scheduled command delivery late/missing timestamp')
    if [e.get('action') for e in raw.get('sent', [])] != expected_actions:
        errors.append('sent actions differ from fixed bounded series')
    cows = {}; alive = set()
    for event in client['events']:
        name = event['name']
        if name not in PACKETS:
            continue
        stream = io.BytesIO(bytes.fromhex(event['hex']))
        value = decoder.read(stream, 'packet_'+name)
        if stream.read():
            raise ValueError('trailing bytes: '+name)
        tick = (event['at']-start)/.05
        stamp = {'at':event['at'], 'tick_float':tick, 'packet':name, 'value':value}
        if name == 'spawn_entity' and value['type'] == cow_type:
            choices = [i for i,(a,b) in enumerate(WINDOWS) if a <= tick <= a+SPAWN_LATENCY_TICKS]
            if len(choices)!=1:
                errors.append('cow spawn outside declared window'); continue
            i=choices[0];cows[value['entityId']]=i;alive.add(value['entityId'])
            windows[i]['spawns'].append(stamp)
            if not near([value[k]-origin[j] for j,k in enumerate(('x','y','z'))], POSITION):
                errors.append('cow spawn position')
            if any(value.get(k,0) for k in ('yaw','pitch','headPitch')) or any(value['velocity'].values()):
                errors.append('cow spawn rotation/velocity')
        if name=='entity_destroy':
            for removed in value['entityIds']:
                alive.discard(removed)
                if removed in cows and tick < WINDOWS[cows[removed]][1]-1:
                    errors.append('Cow removed before observation window ends')
        entity = value.get('entityId')
        if entity in cows:
            i=cows[entity];observations.append(stamp)
            if name=='entity_metadata':
                windows[i]['metadata'].extend(value['metadata'])
                windows[i]['metadata_packets'].append(stamp)
            if name in ('rel_entity_move','entity_move_look'):
                if any(any(step['delta']) for step in value['move']['steps']):
                    errors.append('cow relative movement')
            if name=='entity_velocity' and any(value['velocity'].values()):
                errors.append('cow velocity')
            if name in ('entity_teleport','sync_entity_position'):
                positions=value.get('steps',[value])
                for pos in positions:
                    if any(pos.get(k,0) for k in ('dx','dy','dz','yaw','pitch','headYaw')):
                        errors.append('cow stepped rotation/velocity')
                    if not near([pos[k]-origin[j] for j,k in enumerate(('x','y','z'))],POSITION):
                        errors.append('cow absolute movement')
                if any(value.get(k,0) for k in ('dx','dy','dz')) or any(value.get('flags',{}).values()):
                    errors.append('cow teleport velocity/relative flags')
            if name in ('entity_move_look','entity_look','entity_teleport','sync_entity_position','entity_head_rotation'):
                if any(value.get(k,0) for k in ('yaw','pitch','headYaw')):
                    errors.append('cow rotation')
        if name in ('sound_effect','entity_sound_effect'):
            holder=value['sound'];sound=sounds.get(str(holder.get('soundId')),holder.get('sound',{}).get('soundName'))
            if sound != SOUND:
                continue
            if name!='sound_effect':
                errors.append('unexpected entity-attached ambient transport');continue
            choices=[i for i,(a,b) in enumerate(WINDOWS) if a-1 <= tick < b+1]
            if len(choices)!=1:
                errors.append('ambient outside declared window');continue
            i=choices[0];windows[i]['sounds'].append(stamp)
            w=windows[i]
            prior=[p for p in w['metadata_packets'] if p['at']<=event['at']]
            if not any(p['at']<=event['at'] and p['value']['entityId'] in alive for p in w['spawns']) or not any(
                    m['key']==15 and m['value']&1 for p in prior for m in p['value']['metadata']):
                errors.append('ambient before Cow spawn/NoAI metadata')
            if silent and not any(m['key']==4 and m['value'] is True for p in prior for m in p['value']['metadata']):
                errors.append('ambient before Silent metadata')
            if not near([value[k]/8-origin[j] for j,k in enumerate(('x','y','z'))],POSITION):
                errors.append('ambient position')
            # Измеренные взрослые Cow: volume0.4, возрастной pitch0.8..1.2.
            if not near([value['volume']],[.4]) or not math.isfinite(value['pitch']) or not .8-1e-6 <= value['pitch'] <= 1.2+1e-6:
                errors.append('ambient volume/pitch')
            if value['soundCategory'] != 'neutral':
                errors.append('ambient category')
    for w in windows:
        if len(w['spawns'])!=1:
            errors.append('each window requires exactly one Cow spawn')
        initial=[m for p in w['metadata_packets'] if p['at'] <= start+(w['start_tick']+SPAWN_LATENCY_TICKS)*.05 for m in p['value']['metadata']]
        if not any(m['key']==15 and m['value']&1 for m in initial):
            errors.append('NoAI metadata missing')
        if any(m['key']==15 and not m['value']&1 for m in w['metadata']):
            errors.append('NoAI metadata disabled')
        if silent and any(m['key']==4 and m['value'] is False for m in w['metadata']):
            errors.append('Silent metadata disabled')
        if not silent and any(m['key']==4 and m['value'] is True for m in w['metadata']):
            errors.append('positive Cow unexpectedly Silent')
        if silent and not any(m['key']==4 and m['value'] is True for m in initial):
            errors.append('Silent metadata missing')
        if any(m['key']==16 and m['value'] is True for m in w['metadata']):
            errors.append('unexpected baby Cow')
        w['sound_count']=len(w['sounds'])
        w['spawn_latency_ticks']=[p['tick_float']-w['start_tick'] for p in w['spawns']]
        w['nominal_observed_ticks_after_spawn']=[w['end_tick']-p['tick_float'] for p in w['spawns']]
    count=sum(w['sound_count'] for w in windows)
    if silent and count:
        errors.append('Silent Cow emitted ambient')
    return {'errors':errors, 'windows':windows, 'sound_count':count, 'cow_packets':observations}


def audit(folder, data):
    decoder=Decoder(json.loads((data/'protocol.json').read_text()))
    cow=next(e['id'] for e in json.loads((data/'entities.json').read_text()) if e['name']=='cow')
    sounds=json.loads((folder/'sound-observations.json').read_text())['sounds']
    if SOUND not in sounds.values():
        raise ValueError('ambient sound registry identity unobserved')
    captures={};errors=[]
    for scenario in SCENARIOS:
        for side in ('official','ours'):
            path=folder/f'{scenario}-{side}.json';raw=json.loads(path.read_text())
            if raw.get('scenario')!=scenario:
                raise ValueError('scenario identity mismatch')
            report=audit_capture(raw,decoder,cow,sounds,scenario.endswith('-silent'))
            report['raw_sha256']=sha(path);captures[f'{scenario}/{side}']=report
            errors.extend(f'{scenario}/{side}: {e}' for e in report['errors'])
    positive=[captures[f'{SCENARIOS[0]}/{side}']['sound_count'] for side in ('official','ours')]
    status='failed' if errors else 'inconclusive' if positive[0]==0 else 'failed' if positive[1]==0 else 'scoped-pass'
    return {'status':status,'errors':errors,'captures':captures,'max_spawn_delivery_ticks':SPAWN_LATENCY_TICKS,
            'claim':'NoAI adult Cow ambient presence, Silent suppression and no observed movement/rotation in three fixed nominal600-tick command windows (spawn delivery at most2ticks, actual lengths reported) only; no timing/count/frequency/RNG parity claim.'}


if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('folder',type=Path);p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();report=audit(a.folder,a.data);write_json(a.output,report)
    raise SystemExit(0 if report['status']=='scoped-pass' else 1)
