#!/usr/bin/env python3
"""Свежие восемь чёрных ящиков Java777: команды, лук и трезубец.

Критерии зафиксированы до записи: союзник friendlyFire=false остаётся с20HP,
без DamageEvent, отдачи, огня и эффектов; каждый положительный контроль
обязательно создаёт снаряд и ранит. Урон/RNG/сроки не сравниваются точно.
Судьба снаряда сохраняется отдельно; сравнивается факт наблюдённого пролёта
за цель. Это ограниченная проверка обычных незачарованных снарядов.
"""
import argparse
import copy
import hashlib
import io
import json
import math
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_gameplay import Client, connect
from probe_block_entities import exact, nbt
from probe_persistence_777 import player_metadata
from probe_use_actions_runtime import lpvec, use_slot
from smoke import read_varint, varint

JAR_SHA1 = '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
SCHEMA = 'team-projectiles-777-v3'
CASES = [weapon+'-'+mode for weapon in ('bow', 'trident')
         for mode in ('same-denied', 'same-allowed', 'different-denied')] + ['bow-self-denied','bow-self-allowed']
CRITERIA = {'version':SCHEMA, 'cases':CASES,
            'forbidden':'same-denied: initial/min/final HP20; no own DamageEvent, nonzero velocity, fire or effects',
            'control':'same-allowed/different-denied/self-allowed: initial HP20, minimum HP<20 and own DamageEvent with direct=projectile+1/cause=shooter+1, exactly one matching projectile spawn',
            'self':'self-denied has no presumed hit prohibition; input-v3 one authored tp within1s of observed spawn to forecast XZ/y65: position+=velocity, drag0.99/gravity0.05, descending crossing y66 within200ticks; observe10s; compare fresh official hit/health-reduced/sideeffects; self-allowed must genuinely hit',
            'backstop':'input-v3 wall z2; shooter z-5.5 and target z0.5 unchanged; require observed projectile-past-target as before',
            'comparison':'same boolean hit/fire/effect/velocity outcomes and observed projectile-past-target; no exact damage, RNG or tick comparison',
            'limits':'two stationary survival players; ordinary bow arrows and unenchanted thrown trident; no Flame/tipped/multishot/Piercing/Bedrock'}
# Отдельный ввод для проверки дальнего слежения; прежний v3 неизменен.
LONG_SCHEMA = 'team-projectiles-777-long-range-v1'
LONG_CRITERIA = copy.deepcopy(CRITERIA)
LONG_CRITERIA.update(version=LONG_SCHEMA,
    backstop='input long-range-v1 wall z24; same floor, two survival players and eight real shots; no inferred projectile positions',
    tracking='same-denied bow/trident: both observers must see exactly one matching spawn and a zero-velocity receipt; compare observed flight-past-target and in-ground metadata booleans for each observer; no guessed landing coordinates or exact RNG/tick/packet counts')


def criteria_for(schema):
    return {SCHEMA:CRITERIA, LONG_SCHEMA:LONG_CRITERIA}.get(schema)


NAMES = ('McsaArcher', 'McsaTarget')
# Собственная GUI-запись actual777-use-action-observations.json: release_use6.
# Внутреннее старое ядро использует5, но это НЕ wire-значение Java777.
RELEASE_USE = 6
TEAM_LAYOUT_FILE = Path(__file__).with_name('testdata')/'team-layout-777.json'
TEAM_LAYOUT_SHA256 = hashlib.sha256(TEAM_LAYOUT_FILE.read_bytes()).hexdigest()


def uint(data):
    value = read_varint(data.read)
    assert 0 <= value < 2**31, 'invalid unsigned integer'
    return value


def string(data):
    count = uint(data)
    assert count <= 32767
    return exact(data, count).decode('utf-8')


def end(data):
    assert not data.read(), 'trailing packet payload'


def finite(values):
    assert all(math.isfinite(v) for v in values), 'nonfinite capture'
    return list(values)


class State:
    """Один и тот же строгий декодер для live и повторного чтения raw."""
    def __init__(self, kinds):
        self.kinds = kinds
        self.entity = None
        self.health = None
        self.teams = {}
        self.team_receipts = []
        self.events = []
        self.inventory = {}
        self.positions = {}

    def accept(self, packet):
        assert packet.get('direction') in ('in', 'out')
        payload = bytes.fromhex(packet['payload'])
        data = io.BytesIO(payload)
        name = packet['name']
        event = {'name':name}
        if packet['direction'] == 'out':
            if name == 'use_item':
                hand, sequence = uint(data), uint(data)
                rotation = finite(struct.unpack('>ff', exact(data,8)))
                assert hand == 0 and rotation in ([0.0,0.0],[0.0,-90.0])
                end(data)
                event.update(hand=hand, sequence=sequence)
            elif name == 'block_dig':
                status = uint(data)
                exact(data,9)
                uint(data)
                end(data)
                event['status'] = status
            elif name == 'teleport_confirm':
                event['teleport'] = uint(data)
                event['position'] = finite(struct.unpack('>dddff',exact(data,32)))
                end(data)
            else:
                return
            self.events.append(event)
            return
        if name == 'login':
            self.entity = struct.unpack('>i',exact(data,4))[0]
            return
        if name == 'update_health':
            health = struct.unpack('>f', exact(data,4))[0]
            food = uint(data)
            saturation = struct.unpack('>f', exact(data,4))[0]
            finite([health,saturation])
            assert 0 <= health <= 20 and 0 <= food <= 20
            end(data)
            self.health = health
            event['health'] = health
        elif name == 'damage_event':
            entity, source, cause, direct = [uint(data) for _ in range(4)]
            present = exact(data,1)
            assert present in (b'\0',b'\1')
            if present == b'\1': finite(struct.unpack('>ddd', exact(data,24)))
            end(data)
            if entity != self.entity: return
            event.update(source=source,cause=cause,direct=direct)
        elif name == 'entity_velocity':
            entity = uint(data)
            velocity = finite(lpvec(data))
            end(data)
            if entity != self.entity: return
            event['velocity'] = velocity
        elif name == 'entity_effect':
            entity, effect, amplifier, duration = [uint(data) for _ in range(4)]
            flags = exact(data,1)[0]
            end(data)
            if entity != self.entity: return
            event.update(effect=effect,amplifier=amplifier,duration=duration,flags=flags)
        elif name == 'entity_metadata':
            entity = uint(data)
            if entity != self.entity: return
            _, fields = player_metadata(payload)
            if 0 not in fields: return
            event['fire'] = bool(fields[0] & 1)
        elif name == 'teams':
            team, mode = string(data), exact(data,1)[0]
            assert mode <= 4
            if mode in (0,2):
                # Собственный team-layout-777.json/actions.rs: JSON оставил
                # старый порядок775. Оригинал777: name/prefix/suffix,
                # visibility/collision, optional color, flags.
                nbt(data); nbt(data); nbt(data)
                visibility,collision=uint(data),uint(data)
                assert visibility<=3 and collision<=3
                has_color=exact(data,1)
                assert has_color in (b'\0',b'\1')
                if has_color==b'\1': assert uint(data)<=15
                flags = exact(data,1)[0]
                assert flags & ~3 == 0
                if mode == 0:
                    assert team not in self.teams, 'duplicate team'
                    self.teams[team] = {'flags':flags,'members':[]}
                else:
                    assert team in self.teams
                    self.teams[team]['flags'] = flags
            if mode in (0,3,4):
                count = uint(data)
                assert count <= 32
                members = [string(data) for _ in range(count)]
                assert team in self.teams
                for member in members:
                    if mode == 4:
                        self.teams[team]['members'].remove(member)
                    else:
                        for value in self.teams.values():
                            if member in value['members']: value['members'].remove(member)
                        self.teams[team]['members'].append(member)
            if mode == 1:
                assert team in self.teams
                del self.teams[team]
            end(data)
            self.team_receipts.append({'team':team,'mode':mode})
            return
        elif name == 'spawn_entity':
            entity = uint(data)
            exact(data,16)
            kind = uint(data)
            position = finite(struct.unpack('>ddd',exact(data,24)))
            velocity = finite(lpvec(data))
            exact(data,3); uint(data); end(data)
            if kind not in self.kinds.values(): return
            self.positions[entity] = position
            event.update(entity=entity,kind=kind,position=position,velocity=velocity)
        elif name == 'sync_entity_position':
            entity, kind = uint(data), uint(data)
            assert kind in (0,1)
            if kind == 0:
                positions = [finite(struct.unpack('>ddd',exact(data,24)))]
            else:
                count = uint(data)
                assert 0 < count <= 64
                positions = []
                for _ in range(count):
                    positions.append(finite(struct.unpack('>ddd',exact(data,24))))
                    uint(data)
            finite(struct.unpack('>ff',exact(data,8)))
            assert exact(data,1) in (b'\0',b'\1')
            end(data)
            if entity not in self.positions: return
            self.positions[entity] = positions[-1]
            event.update(entity=entity,positions=positions)
        elif name in ('rel_entity_move','entity_move_look'):
            entity = uint(data)
            # Собственный movement.rs: opaque entityDelta777, младший бит
            # ground, остальные — число шагов; ноль означает один шаг.
            properties = uint(data)
            count = properties >> 1
            assert count <= 1024
            deltas = []
            for _ in range(count or 1):
                if count: uint(data)
                deltas.append(struct.unpack('>hhh',exact(data,6)))
            if name == 'entity_move_look': exact(data,2)
            end(data)
            if entity not in self.positions: return
            positions = []
            for delta in deltas:
                position = [old + change/4096 for old,change in zip(self.positions[entity],delta)]
                self.positions[entity] = position
                positions.append(position)
            event.update(entity=entity,positions=positions)
        elif name == 'entity_destroy':
            count = uint(data)
            assert count <= 10000
            entities = [uint(data) for _ in range(count)]
            end(data)
            event['entities'] = [entity for entity in entities if entity in self.positions]
        elif name in ('window_items','set_slot','set_player_inventory'):
            if name == 'window_items':
                window, revision, count = uint(data), uint(data), uint(data)
                assert count <= 128
                slots = {i:use_slot(data) for i in range(count)}
                use_slot(data)
                if window == 0: self.inventory = slots
            else:
                if name == 'set_slot':
                    window, revision = uint(data), uint(data)
                    slot = struct.unpack('>h',exact(data,2))[0]
                else:
                    window, slot = 0, uint(data)
                    slot = slot+36 if slot < 9 else slot
                stack = use_slot(data)
                if window == 0: self.inventory[slot] = stack
            end(data)
            return
        else:
            return
        self.events.append(event)


class Probe(Client):
    def __init__(self, sock, protocol, kinds):
        super().__init__(sock,protocol)
        self.wire = []
        self.state = State(kinds)

    def send(self,name,payload=b''):
        packet = {'direction':'out','name':name,'payload':payload.hex(),'time':time.monotonic()}
        self.wire.append(packet)
        self.state.accept(packet)
        super().send(name,payload)

    def observe_packet(self,name,payload):
        packet = {'direction':'in','name':name,'payload':payload.hex(),'time':time.monotonic()}
        self.wire.append(packet)
        self.state.accept(packet)


def replay(packets,kinds):
    result = State(kinds)
    for packet in packets: result.accept(packet)
    return result


def team_assert(state,case):
    assert set(state.teams) == {'mcsa_a','mcsa_b'}, 'missing team acknowledgements'
    a,b = state.teams['mcsa_a'],state.teams['mcsa_b']
    mode = case.split('-',1)[1]
    assert bool(a['flags'] & 1) == mode.endswith('allowed'), 'wrong acknowledged friendlyFire'
    assert NAMES[0] in a['members']
    assert NAMES[1] in (b if mode == 'different-denied' else a)['members'], 'wrong acknowledged membership'


def predict_self_aim(position,velocity,*,max_ticks=200):
    """Только авторский ввод TP, не доказательство попадания/физики сервера.

    Wiki Arrow и собственный arrow.rs: drag0.99/gravity0.05. Начальные
    величины берём из реального spawn, включая квантование lpVec3.
    Paired41996104828: TP под текущий y107 дал оставшийся driftX0.318,
    поэтому заранее целимся в расчётное пересечение тела на y66.
    """
    assert len(position)==len(velocity)==3
    position,velocity=finite(position),finite(velocity)
    assert type(max_ticks) is int and 1<=max_ticks<=200, 'invalid prediction budget'
    assert 65<position[1]<68 and 2<velocity[1]<4, 'invalid self launch height/upward speed'
    assert 2<math.sqrt(sum(v*v for v in velocity))<4, 'invalid self launch speed'
    assert abs(velocity[0])<.25 and abs(velocity[2])<.25, 'invalid self launch horizontal speed'
    for tick in range(1,max_ticks+1):
        after=[p+v for p,v in zip(position,velocity)]
        if velocity[1]<0 and position[1]>=66 and after[1]<=66:
            fraction=(position[1]-66)/(position[1]-after[1])
            crossing=[p+(q-p)*fraction for p,q in zip(position,after)]
            assert -7<crossing[0]<7 and -15<crossing[2]<1.5, 'self aim outside authored clear floor'
            return {'tick':tick,'crossing':crossing}
        position=after
        velocity=[velocity[0]*.99,velocity[1]*.99-.05,velocity[2]*.99]
    raise AssertionError('self prediction exceeded200ticks')


def world_setup_commands(backstop=2):
    assert type(backstop) is int and backstop in (2,24), 'unregistered backstop input'
    # core41990613917 появился на y63: будущий полy64 пересёк голову.
    # Защита на время авторского строительства не меняет режим выстрелов.
    return [f'gamemode creative {name}' for name in NAMES]+[
        'gamerule minecraft:spawn_mobs false','gamerule minecraft:natural_health_regeneration false',
        'difficulty normal','weather clear','time set noon',
        'fill -8 64 -16 8 64 32 stone','fill -8 65 -16 8 72 32 air',
        f'fill -8 65 {backstop} 8 69 {backstop} stone','team add mcsa_a','team add mcsa_b']


def case_setup_commands(case):
    weapon,mode=case.split('-',1)
    # Сначала безопасное положение, затем лечение и выживание. В неудачной
    # записи core41990613917 обратный порядок дал удушье внутри пола доTP.
    return [f'gamemode creative {name}' for name in NAMES]+[
        'kill @e[type=arrow]','kill @e[type=trident]','kill @e[type=item]',
        f'clear {NAMES[0]}',f'clear {NAMES[1]}',
        f'team join mcsa_a {NAMES[0]}',
        f'team join {"mcsa_b" if mode=="different-denied" else "mcsa_a"} {NAMES[1]}',
        f'team modify mcsa_a friendlyFire {str(mode.endswith("allowed")).lower()}',
        f'tp {NAMES[0]} 0.5 65 -5.5 0 {"-90" if mode.startswith("self-") else "0"}',
        f'tp {NAMES[1]} {"6.5" if mode.startswith("self-") else "0.5"} 65 0.5 180 0',
        *[f'effect clear {name}' for name in NAMES],
        *[f'effect give {name} instant_health 1 8 true' for name in NAMES],
        *[f'gamemode survival {name}' for name in NAMES],
        f'give {NAMES[0]} {weapon}', *([f'give {NAMES[0]} arrow 3'] if weapon=='bow' else [])]


def summarize(report,row):
    case = row['case']
    kinds = report['entity_kinds']
    measured = []
    before = []
    for role in ('shooter','victim'):
        packets = report['sessions'][role]['packets']
        start, stop = row['ranges'][role]
        assert type(start) is int and type(stop) is int and 0 < start < stop <= len(packets), 'invalid raw shot range'
        times=[p['time'] for p in packets[start:stop]]
        assert all(isinstance(t,(int,float)) and math.isfinite(t) for t in times)
        assert times==sorted(times), 'nonmonotonic capture'
        assert times[-1]-times[0] >= (10 if '-self-' in case else 3), 'short shot observation'
        state = replay(packets[:start],kinds)
        assert state.entity is not None and any(p['direction']=='in' and p['name']=='login' for p in packets[:start]), 'no real login'
        team_assert(state,case)
        before.append(copy.deepcopy(state))
        state.events = []
        for packet in packets[start:stop]: state.accept(packet)
        measured.append(state)
    assert before[0].entity != before[1].entity, "players must have distinct observed entity IDs"
    shooter,victim = measured
    target_index = 0 if '-self-' in case else 1
    victim = measured[target_index]
    assert before[target_index].health == 20, 'target initial HP must be20'
    shots = [event for event in shooter.events if event['name']=='use_item']
    releases = [event for event in shooter.events if event['name']=='block_dig' and event['status']==RELEASE_USE]
    assert len(shots)==len(releases)==1, 'missing or duplicate real use/release'
    kind = kinds['arrow' if case.startswith('bow-') else 'trident']
    spawns = [event for event in shooter.events if event['name']=='spawn_entity' and event['kind']==kind]
    assert len(spawns)==1, 'shot must spawn exactly one matching projectile'
    projectile = spawns[0]['entity']
    positions = [spawns[0]['position']]
    positions += [p for event in shooter.events if event.get('entity')==projectile for p in event.get('positions',[])]
    if '-self-' in case:
        aim=row.get('self_aim')
        assert isinstance(aim,dict) and aim.get('projectile')==projectile, 'missing self aim evidence'
        forecast=predict_self_aim(spawns[0]['position'],spawns[0]['velocity'])
        assert aim.get('forecast')==forecast, 'self aim forecast not derived from raw spawn'
        at=forecast['crossing']
        target=aim['player_position']
        assert target==[at[0],65,at[2]], 'self aim player position differs'
        receipts=[event['position'] for event in shooter.events if event['name']=='teleport_confirm']
        assert len(receipts)==1 and all(abs(a-b)<1e-5 for a,b in zip(receipts[0],target+[0,-90])), 'missing exact self teleport acknowledgement'
        start,stop=row['ranges']['shooter']
        packets=report['sessions']['shooter']['packets'][start:stop]
        spawn_packet=next(packet for packet in packets if packet['direction']=='in' and packet['name']=='spawn_entity'
                          and uint(io.BytesIO(bytes.fromhex(packet['payload'])))==projectile)
        teleport_packet=next(packet for packet in packets if packet['direction']=='out' and packet['name']=='teleport_confirm')
        assert 0<=teleport_packet['time']-spawn_packet['time']<=1, 'self teleport was not early'
        own_damage=[packet for packet in packets if packet['direction']=='in' and packet['name']=='damage_event'
                    and uint(io.BytesIO(bytes.fromhex(packet['payload'])))==before[0].entity]
        assert all(teleport_packet['time']<packet['time'] for packet in own_damage), 'self teleport must precede actual damage'
    health = [20] + [event['health'] for event in victim.events if event['name']=='update_health']
    damage_events = [event for event in victim.events if event['name']=='damage_event']
    assert all(event['direct']==projectile+1 and event['cause']==before[0].entity+1
               for event in damage_events), 'unrelated damage invalidates shot control'
    value = {'initial_health':20,'minimum_health':min(health),'final_health':victim.health,
            # Закреплённый damage_event хранит cause/direct как ID+1;
            # постороннее повреждение не заменяет положительный выстрел.
            'hit':any(event['name']=='damage_event' and event['direct']==projectile+1
                      and event['cause']==before[0].entity+1 for event in victim.events),
            'damage_events':damage_events,
            'velocity':any(event['name']=='entity_velocity' and any(abs(v)>.001 for v in event['velocity']) for event in victim.events),
            'fire':any(event.get('fire',False) for event in victim.events),
            'effects':sorted({event['effect'] for event in victim.events if event['name']=='entity_effect'}),
            'projectile':{'spawn':spawns[0], 'positions':positions,
                          'past_target':any(p[2]>1.0 for p in positions),
                          'destroyed':any(projectile in event.get('entities',[]) for event in shooter.events)}}


    if report['schema'] == LONG_SCHEMA:
        value['tracking'] = tracking_receipts(report, row, projectile)
    return value


def tracking_receipts(report, row, projectile):
    """Только фактически полученные пакеты. Нулевая скорость не сообщает место."""
    result = {}
    for role in ('shooter','victim'):
        packets=report['sessions'][role]['packets']
        start,stop=row['ranges'][role]
        state=replay(packets[:start],report['entity_kinds'])
        receipts=[];spawns=0
        for index in range(start,stop):
            packet=packets[index];before=len(state.events)
            state.accept(packet)
            if packet['direction']!='in': continue
            record={'packet_index':index,'time':packet['time'],'name':packet['name']}
            data=io.BytesIO(bytes.fromhex(packet['payload']))
            if packet['name']=='entity_velocity':
                entity=uint(data)
                if entity!=projectile: continue
                record['velocity']=finite(lpvec(data));end(data)
            elif packet['name']=='entity_metadata':
                entity=uint(data)
                if entity!=projectile: continue
                _,fields=player_metadata(bytes.fromhex(packet['payload']))
                if 10 not in fields: continue
                assert type(fields[10]) is bool, 'arrow in-ground must be boolean'
                record['in_ground']=fields[10]
            else:
                events=[event for event in state.events[before:] if event.get('entity')==projectile]
                if not events: continue
                event=events[-1]
                if event['name']=='spawn_entity':
                    spawns+=1;record['positions']=[event['position']]
                elif 'positions' in event: record['positions']=event['positions']
                else: continue
            if projectile in state.positions:
                record['last_received_position']=state.positions[projectile].copy()
            receipts.append(record)
        actual_positions=[p for receipt in receipts for p in receipt.get('positions',[])]
        result[role]={'spawns':spawns,
            'zero_velocity':any('velocity' in r and all(v==0 for v in r['velocity']) for r in receipts),
            'in_ground':any(r.get('in_ground') is True for r in receipts),
            'past_target':any(p[2]>1 for p in actual_positions),
            'coordinate_updates':sum(r['name']!='spawn_entity' and 'positions' in r for r in receipts),
            'receipts':receipts}
    return result


def behavior_failures(report):
    failures=[]
    for row in report['cases']:
        value=row['summary']
        if row['case'].endswith('same-denied'):
            if value['minimum_health'] != 20 or value['final_health'] != 20 or any(value[key] for key in ('damage_events','hit','velocity','fire','effects')):
                failures.append(row['case']+': forbidden hit or side effect')
        elif not row['case'].endswith('self-denied') and not(value['minimum_health'] < 20 and value['hit']):
            failures.append(row['case']+': positive control did not genuinely hit')
        if value['fire'] or value['effects']:
            failures.append(row['case']+': plain projectile produced fire/effect')
    return failures


def validate(report,target,require_pass=True):
    criteria=criteria_for(report.get('schema'))
    assert criteria is not None and report.get('criteria')==criteria, 'wrong criteria'
    assert report.get('target')==target and report.get('protocol')==777
    assert report.get('team_layout_sha256')==TEAM_LAYOUT_SHA256, 'missing pinned own team-layout evidence'
    assert report.get('server_exit')==0 and 'error' not in report, 'incomplete or crashed capture'
    assert len(report.get('server_log_sha256',''))==64
    assert len(report.get('target_sha256',''))==64
    if target=='official': assert report.get('jar_sha1')==JAR_SHA1
    assert [row['case'] for row in report.get('cases',[])]==CASES, 'missing/reordered/duplicate cases'
    assert set(report.get('sessions',{}))=={'shooter','victim'}
    assert report['sessions']['shooter']['name']==NAMES[0] and report['sessions']['victim']['name']==NAMES[1]
    assert report['sessions']['shooter']['uuid']!=report['sessions']['victim']['uuid']
    previous={'shooter':0,'victim':0}
    for row in report['cases']:
        for role in previous:
            assert row['ranges'][role][0] >= previous[role], 'overlapping shot ranges'
            previous[role]=row['ranges'][role][1]
        assert row['summary']==summarize(report,row), 'summary not derived from raw'
        if report['schema']==LONG_SCHEMA and row['case'].endswith('same-denied'):
            for role in ('shooter','victim'):
                tracking=row['summary']['tracking'][role]
                assert tracking['spawns']==1 and tracking['zero_velocity'], 'missing observed projectile spawn/stop control'

    failures=behavior_failures(report)
    assert report.get('behavior_failures')==failures, 'forged semantic failures'
    if require_pass:
        assert report.get('status')=='PASS' and not failures, 'semantic failure'
    else:
        assert report.get('status') in ('PASS','FAIL')


def compare(official,actual):
    validate(official,'official')
    validate(actual,'core',False)
    assert official['schema']==actual['schema'] and official['criteria']==actual['criteria'], 'cannot compare different input profiles'
    failures=[]
    for a,b in zip(official['cases'],actual['cases']):
        for key in ('hit','velocity','fire','effects'):
            if a['summary'][key]!=b['summary'][key]: failures.append(a['case']+': '+key+' differs')
        if (a['summary']['minimum_health']<20) != (b['summary']['minimum_health']<20):
            failures.append(a['case']+': health-reduced differs')
        if a['summary']['projectile']['past_target'] != b['summary']['projectile']['past_target']:
            failures.append(a['case']+': observed flight past target differs')
        if official['schema']==LONG_SCHEMA and a['case'].endswith('same-denied'):
            for role in ('shooter','victim'):
                for key in ('past_target','zero_velocity','in_ground'):
                    if a['summary']['tracking'][role][key]!=b['summary']['tracking'][role][key]:
                        failures.append(a['case']+': '+role+' tracking '+key+' differs')

    return failures


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    group=parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--jar',type=Path);group.add_argument('--binary',type=Path)
    parser.add_argument('--java',default='/usr/lib/jvm/java-25-openjdk/bin/java')
    parser.add_argument('--data',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--port',type=int,required=True);parser.add_argument('--expected',type=Path)
    parser.add_argument('--observe-only',action='store_true')
    parser.add_argument('--long-range',action='store_true',help='Отдельный закреплённый ввод: стенкаz24 и оба наблюдателя; default v3 z2')
    args=parser.parse_args()
    assert 26000 <= args.port <= 26299
    assert not args.observe_only or args.binary, 'observe-only is solely for preserved core baseline'
    assert args.jar or args.expected, 'core requires fresh official oracle'
    expected=json.loads(args.expected.read_text()) if args.expected else None
    schema=LONG_SCHEMA if args.long_range else SCHEMA
    criteria=criteria_for(schema)
    if expected:
        validate(expected,'official')
        assert expected['schema']==schema, 'expected capture uses different input profile'
    manifest=json.loads(Path(__file__).with_name('candidate-sources-26.3.json').read_text())
    data_hashes={}
    for entry in manifest['files']:
        if entry['file'] in ('protocol.json','entities.json','items.json'):
            value=hashlib.sha256((args.data/entry['file']).read_bytes()).hexdigest()
            assert value==entry['sha256']
            data_hashes[entry['file']]=value
    protocol=json.loads((args.data/'protocol.json').read_text())
    entities=json.loads((args.data/'entities.json').read_text())
    kinds={name:next(row['id'] for row in entities if row['name']==name) for name in ('arrow','trident')}
    executable=(args.jar or args.binary).resolve()
    if args.jar: assert hashlib.sha1(executable.read_bytes()).hexdigest()==JAR_SHA1
    args.output.mkdir(parents=True,exist_ok=True)
    assert not (args.output/'result.json').exists(), 'refuse to overwrite existing evidence'
    report={'schema':schema,'criteria':criteria,'protocol':777,'target':'official' if args.jar else 'core',
            'target_sha256':hashlib.sha256(executable.read_bytes()).hexdigest(),
            'jar_sha1':JAR_SHA1 if args.jar else None,'data_sha256':data_hashes,'entity_kinds':kinds,
            'team_layout_sha256':TEAM_LAYOUT_SHA256,
            'status':'INCOMPLETE','sessions':{},'cases':[],'commands':[],'behavior_failures':[]}
    if args.expected: report['expected_sha256']=hashlib.sha256(args.expected.read_bytes()).hexdigest()
    # Критерии записаны до запуска сервера и не подстраиваются по результату.
    (args.output/'criteria.json').write_text(json.dumps(criteria,indent=2)+'\n')
    with socket.socket() as check: check.bind(('127.0.0.1',args.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-team-projectiles-777-') as directory:
        root=Path(directory)
        props=f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=flat\nlevel-seed=1\nview-distance=3\nsimulation-distance=3\nspawn-protection=0\n'
        (root/'eula.txt').write_text('eula=true\n');(root/'server.properties').write_text(props)
        if args.binary:
            (root/'config').mkdir();(root/'config/server.properties').write_text(props)
            (root/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
            (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
        argv=[args.java,'-Xmx1G','-jar',str(executable),'nogui'] if args.jar else [str(executable)]
        clients={}
        log=args.output/'server.log'
        with log.open('w') as stream:
            server=subprocess.Popen(argv,cwd=root,stdin=subprocess.PIPE,stdout=stream,stderr=subprocess.STDOUT,text=True)
            def pump(seconds=.4):
                until=time.monotonic()+seconds
                while time.monotonic()<until:
                    assert server.poll() is None, 'server exited during capture'
                    if not clients: time.sleep(.05)
                    for client in clients.values(): client.pump(.06)
            def command(value):
                report['commands'].append(value)
                server.stdin.write(value+'\n');server.stdin.flush();pump(.25)
            def wait(predicate,message,seconds=8):
                until=time.monotonic()+seconds
                while not predicate():
                    assert time.monotonic()<until,message
                    pump(.15)
            try:
                deadline=time.monotonic()+120
                while ('Done (' if args.jar else 'Готово (') not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline,'server readiness timeout'
                    time.sleep(.1)
                for role,name,uuid in [('shooter',NAMES[0],bytes([61])*16),('victim',NAMES[1],bytes([62])*16)]:
                    client=connect(args.port,protocol,lambda s,p:Probe(s,p,kinds),username=name,login_uuid=uuid)
                    clients[role]=client
                    report['sessions'][role]={'name':name,'uuid':uuid.hex(),'packets':client.wire}
                    wait(lambda:client.position is not None,'initial player position')
                for cmd in world_setup_commands(24 if args.long_range else 2):
                    command(cmd)
                for index,case in enumerate(CASES):
                    weapon,mode=case.split('-',1)
                    for cmd in case_setup_commands(case):
                        command(cmd)
                    shooter,victim=clients['shooter'],clients['victim']
                    for client in clients.values(): client.send('held_item_slot',struct.pack('>h',0))
                    pump(.8)
                    for client in clients.values(): team_assert(client.state,case)
                    wait(lambda:victim.state.health==20 and shooter.state.health==20,'players not restored to20HP')
                    row={'case':case,'ranges':{role:[len(client.wire),None] for role,client in clients.items()}}
                    shot_events=len(shooter.state.events)
                    shooter.send('use_item',varint(0)+varint(index*2+1)+struct.pack('>ff',0,-90 if mode.startswith('self-') else 0))
                    pump(1.25)
                    shooter.send('block_dig',varint(RELEASE_USE)+b'\0'*9+varint(index*2+2))
                    if mode.startswith('self-'):
                        deadline=time.monotonic()+10
                        relocated=False
                        while time.monotonic()<deadline:
                            pump(.10)
                            spawns=[event for event in shooter.state.events[shot_events:]
                                    if event['name']=='spawn_entity' and event['kind']==kinds['arrow']]
                            assert len(spawns)<=1, 'duplicate self-shot spawn'
                            if not spawns: continue
                            if not relocated:
                                forecast=predict_self_aim(spawns[0]['position'],spawns[0]['velocity'])
                                at=forecast['crossing']
                                command(f'tp {NAMES[0]} {at[0]:.6f} 65 {at[2]:.6f} 0 -90')
                                row['self_aim']={'projectile':spawns[0]['entity'],'forecast':forecast,'player_position':[at[0],65,at[2]]}
                                relocated=True
                        assert relocated, 'self-shot spawn not observed in bounded aim window'
                    else:
                        pump(3)
                    for role,client in clients.items(): row['ranges'][role][1]=len(client.wire)
                    row['summary']=summarize(report,row)
                    report['cases'].append(row)
                    print(json.dumps({'case':case,'health':row['summary']['minimum_health'],'hit':row['summary']['hit']}),flush=True)
            except BaseException as error:
                report['error']=type(error).__name__+': '+str(error)
            finally:
                for client in clients.values(): client.sock.close()
                if server.poll() is None:
                    try:
                        server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=30)
                    except (OSError,subprocess.TimeoutExpired): server.kill();server.wait()
                report['server_exit']=server.returncode
        report['server_log_sha256']=hashlib.sha256(log.read_bytes()).hexdigest()
    if 'error' not in report:
        report['behavior_failures']=behavior_failures(report)
        report['status']='FAIL' if report['behavior_failures'] else 'PASS'
        try:
            validate(report,report['target'],False)
            if expected: report['comparison_failures']=compare(expected,report)
            if report.get('comparison_failures'): report['status']='FAIL'
        except BaseException as error:
            report['error']=type(error).__name__+': '+str(error);report['status']='INCOMPLETE'
    (args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({key:report.get(key) for key in ('status','error','behavior_failures','comparison_failures','server_exit')}))
    # Baseline failures сохраняются и остаются FAIL; exit0 лишь позволяет CI
    # собрать их доказательства. Незавершённая запись не получает этот выход.
    if report['status']!='PASS' and not(args.observe_only and report['status']=='FAIL'):
        raise SystemExit(1)


if __name__=='__main__': main()
