#!/usr/bin/env python3
"""Конечная ORIGINAL777 ground-калибровка; никаких боя/Wind/core/build."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import copy
import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import signal
import socket
import struct
import subprocess
import threading
import time

import record777 as base
from analyze777 import Decoder
from combat_ci import fetch
from combat_process import process_identity
from smoke import handshake, send, frame, varint, read_varint

ROOT = Path(__file__).resolve().parents[2]
CRITERIA = ROOT / 'tools/blackbox/ci/tracker-ground-offset-777.json'
PLAN = ROOT / 'tools/blackbox/ci/tracker-ground-offset-777-plan.md'
CRITERIA_SHA = 'a65fed98f2a03b2117e80d68c5ce3a0f4016feb25488312bf77926fcf2a1feb5'
PORT = 26180
ACTOR = [100.5, 64., 100.5]
OBSERVER = [100.5, 64., 106.5]
COORDINATE_PACKETS = {'spawn_entity', 'sync_entity_position', 'entity_teleport', 'rel_entity_move', 'entity_move_look'}
FORBIDDEN_OUT = {'attack', 'use_entity', 'use_item', 'block_place', 'block_dig', 'arm_animation'}
FORBIDDEN_IN = {'explosion', 'damage_event', 'entity_effect'}


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def criteria():
    assert sha(CRITERIA) == CRITERIA_SHA, 'frozen criteria changed'
    result = json.loads(CRITERIA.read_text())
    assert sha(PLAN) == result['plan_sha256'], 'frozen plan changed'
    return result


def decoded(decoder, event):
    stream = io.BytesIO(bytes.fromhex(event['hex']))
    value = decoder.read(stream, 'packet_' + event['name'])
    assert not stream.read(), 'trailing bytes: ' + event['name']
    return value


def parse_pos(response, player='blackbox'):
    prefix = player + ' has the following entity data: '
    assert response.startswith(prefix), 'wrong authority player/response'
    number = r'[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?[dD]?'
    match = re.fullmatch(r'\[\s*(' + number + r')\s*,\s*(' + number + r')\s*,\s*(' + number + r')\s*\]', response[len(prefix):])
    assert match, 'authority Pos must be exactly three finite doubles'
    result = [float(x.rstrip('dD')) for x in match.groups()]
    assert all(math.isfinite(x) for x in result), 'nonfinite authority Pos'
    return result


def parse_ground(response, player='blackbox'):
    prefix = player + ' has the following entity data: '
    assert response.startswith(prefix) and response[len(prefix):] in ('0b', '1b'), 'wrong authority ground receipt'
    return response[len(prefix):] == '1b'


def same(a, b, tolerance=1e-9):
    return len(a) == len(b) and all(abs(x-y) <= tolerance for x, y in zip(a, b))


def geometry_checks():
    values=[(x,63,100,'stone') for x in range(100,103)]+[(100,63,106,'stone')]
    return values+[(x,y,z,'air') for x in range(100,103) for y in (64,65) for z in (100,106)]


def schedule(fixed):
    rows, queries = [], []
    current = list(ACTOR)
    for phase in fixed['schedule']['phases']:
        start = len(rows)
        for offset in range(phase['frames']):
            move = None
            if phase['kind'] == 'hold':
                assert current[0] == phase['x'], 'phase endpoint mismatch'
                if offset % phase['report_every'] == 0:
                    move = list(current)
            elif offset in phase['move_offsets']:
                index = phase['move_offsets'].index(offset) + 1
                current = [phase['start_x'] + phase['step']*index, 64., 100.5]
                move = list(current)
            frame_id = len(rows)
            row = dict(frame=frame_id, phase=phase['name'], phase_offset=offset,
                       actor_move=move, expected_current=list(current), phase_kind=phase['kind'])
            if offset in phase['authority_frames']:
                row['query'] = len(queries)
                queries.append(dict(index=len(queries), frame=frame_id, phase=phase['name'],
                                    phase_kind=phase['kind'], expected_hold=list(current) if phase['kind']=='hold' else None,
                                    phase_start_frame=start, phase_end_frame=start+phase['frames']))
            rows.append(row)
    assert len(rows) == fixed['schedule']['total_frames'] == 412 and len(queries) == 11
    return rows, queries


def phase_echo(events, decoder, token, issued_at):
    matches = []
    for index, event in enumerate(events):
        if event['name'] != 'profileless_chat' or event['at'] < issued_at:
            continue
        value = decoded(decoder, event)
        if value['message'] != token:
            continue
        assert value['name'] == 'Rcon' and value['type'] == {'chatType': 4} and value['target'] is None, 'wrong phase echo type/speaker'
        matches.append(dict(index=index, at=event['at'], value=value, raw=event))
    assert len(matches) == 1, 'missing/duplicate/stale exact phase echo'
    return matches[0]


def age_bracket(events, index, decoder):
    before = [decoded(decoder, e)['age'] for e in events[:index] if e['name'] == 'update_time']
    after = [decoded(decoder, e)['age'] for e in events[index+1:] if e['name'] == 'update_time']
    return dict(before=before[-1] if before else None, after=after[0] if after else None,
                scope='ordered same-stream emission bracket; NOT pose sample/execute time')


def tracker_steps(events, decoder, entity):
    position = None
    result = []
    for index, event in enumerate(events):
        if event['name'] not in COORDINATE_PACKETS:
            continue
        value = decoded(decoder, event)
        if value['entityId'] != entity:
            continue
        name = event['name']
        offsets = []
        positions = []
        if name == 'sync_entity_position' and value['positionType'] == 'stepped':
            for step in value['steps']:
                position = [step[k] for k in ('x', 'y', 'z')]
                offsets.append(step['tickOffset'])
                positions.append(list(position))
        elif name in ('spawn_entity', 'sync_entity_position', 'entity_teleport'):
            if name == 'entity_teleport':
                assert not any(value['flags'].values()), 'relative peer TP lacks independent base proof'
            position = [value[k] for k in ('x', 'y', 'z')]
            positions.append(list(position))
        else:
            assert position is not None, 'peer delta without absolute base'
            for step in value['move']['steps']:
                position = [position[i] + step['delta'][i]/4096 for i in range(3)]
                offsets.append(step.get('tickOffset', 0))
                positions.append(list(position))
        assert positions and all(all(math.isfinite(x) for x in p) for p in positions), 'empty/nonfinite peer coordinate packet'
        result.append(dict(index=index, at=event['at'], name=name, positions=positions,
                           offsets=offsets, onGround=value.get('onGround', value.get('move', {}).get('onGround')),
                           age_bracket=age_bracket(events, index, decoder), raw=event))
    return result


def emission_gap_observation(previous, current):
    left = current['age_bracket']['before']
    right = previous['age_bracket']['after']
    offset_sum = sum(current['offsets']) if current['offsets'] else None
    lower = max(0, left-right) if left is not None and right is not None else None
    excluded = lower is not None and offset_sum is not None and lower > offset_sum
    return dict(previous_event_index=previous['index'], current_event_index=current['index'],
                current_offsets=current['offsets'], offset_sum=offset_sum,
                emission_gap_lower_ticks=lower, excludes_elapsed_emission_gap=excluded,
                scope='does not identify pose sample timing/interpolation/current authority')


class TraceClient(base.Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.decoder = Decoder(protocol)
        self.events, self.outbound, self.acks = [], [], []
        self.entity, self.health, self.failure = None, None, None
        self.lock = threading.RLock()
        self.running = True

    def send(self, name, payload=b'', *, explicit=False):
        if name == 'tick_end' and not explicit:
            return
        with self.lock:
            issued_at=time.monotonic()
            super().send(name, payload)
            self.outbound.append(dict(issued_at=issued_at, at=time.monotonic(), name=name, hex=payload.hex()))

    def observe_packet(self, name, payload):
        with self.lock:
            self.events.append(dict(at=time.monotonic(), name=name, hex=payload.hex()))
            if name == 'login':
                self.entity = struct.unpack('>i', payload[:4])[0]
            elif name == 'update_health':
                self.health = struct.unpack('>f', payload[:4])[0]

    def confirm_teleport(self, ident, values):
        with self.lock:
            super().confirm_teleport(ident, values)
            self.acks.append(dict(at=time.monotonic(), id=ident, pose=list(values)))

    def move(self, position):
        with self.lock:
            assert not self.failure
            self.position = list(position) + [0., 0.]
            self.send('position_look', struct.pack('>dddffB', *self.position, 1))

    def start(self):
        def receive():
            try:
                while self.running:
                    self.pump(.05)
            except BaseException as error:
                if self.running:
                    self.failure = repr(error)
        self.thread = threading.Thread(target=receive, daemon=True)
        self.thread.start()

    def snapshot(self):
        with self.lock:
            return dict(entity=self.entity, health=self.health, events=list(self.events),
                        outbound=list(self.outbound), acks=list(self.acks), failure=self.failure)

    def close(self):
        self.running = False
        self.sock.close()
        if hasattr(self, 'thread'):
            self.thread.join(2)


class Capture:
    def __init__(self, clients, control, fixed, output, prefix):
        self.clients, self.control, self.fixed, self.output, self.prefix = clients, control, fixed, output, prefix
        self.commands, self.queries, self.frames = [], [], []
        self.lock = threading.RLock()
        self.executor = ThreadPoolExecutor(max_workers=1)
        self.futures = []

    def command(self, text):
        row = dict(command=text, at=time.monotonic())
        with self.lock:
            self.commands.append(row)
        result = self.control.run(text)
        with self.lock:
            row.update(response=result, ended_at=time.monotonic())
        return copy.deepcopy(row)

    def idle(self, count):
        began = time.monotonic()
        for n in range(count):
            time.sleep(max(0, began+n*.05-time.monotonic()))
            for c in self.clients:
                c.send('tick_end', explicit=True)

    def teleport(self, i, position):
        c = self.clients[i]
        count = len(c.acks)
        row = self.command('tp ' + ('blackboxb' if i else 'blackbox') + ' ' + ' '.join(map(str, position)) + ' 0 0')
        deadline = time.monotonic()+2
        while len(c.acks) == count and time.monotonic() < deadline:
            time.sleep(.005)
        assert len(c.acks) == count+1 and c.acks[-1]['pose'][:3] == position, 'fresh initial TP/ACK'
        return row

    def check_block(self, x, y, z, material, number):
        token = self.prefix + '_geometry_' + str(number)
        row = self.command(f'execute if block {x} {y} {z} minecraft:{material} run say {token}')
        self.idle(4)
        row['echoes'] = [phase_echo(c.snapshot()['events'], c.decoder, token, row['at']) for c in self.clients]
        return row

    def prepare(self):
        for text in ('gamerule spawn_mobs false', 'gamerule natural_health_regeneration false',
                     'difficulty normal', 'weather clear', 'kill @e[type=!player]', 'forceload add 92 92 124 124'):
            self.command(text)
        self.idle(20)
        self.command('fill 92 64 92 124 84 124 air')
        self.command('fill 92 63 92 124 63 124 stone')
        self.initial_teleports = [self.teleport(0, ACTOR), self.teleport(1, OBSERVER)]
        for name in ('blackbox', 'blackboxb'):
            for text in ('gamemode survival ' + name, 'clear ' + name, 'effect clear ' + name):
                self.command(text)
        self.idle(10)
        for i,c in enumerate(self.clients):
            c.move(ACTOR if i == 0 else OBSERVER)
        checks = geometry_checks()
        self.geometry = [self.check_block(*pos, number=i) for i,pos in enumerate(checks)]
        for name in ('blackbox','blackboxb'):
            inv = self.command('data get entity ' + name + ' Inventory')
            equipment = self.command('data get entity ' + name + ' equipment')
            assert inv['response'].endswith('[]') and ('Found no elements matching equipment' in equipment['response'] or equipment['response'].endswith('{}')), 'noncombat clients must be empty/naked'

    def sample(self, index):
        with self.lock:
            self.queries[index]['started_at'] = time.monotonic()
        before_token = self.prefix + '_q' + str(index) + '_before'
        after_token = self.prefix + '_q' + str(index) + '_after'
        values = dict(before_token=before_token, after_token=after_token)
        try:
            values['before'] = self.command('say ' + before_token)
            values['Pos'] = self.command('data get entity blackbox Pos')
            values['OnGround'] = self.command('data get entity blackbox OnGround')
            values['after'] = self.command('say ' + after_token)
            values['ended_at'] = time.monotonic()
        except BaseException as error:
            values['error'] = repr(error)
            raise
        finally:
            with self.lock:
                self.queries[index].update(values)

    def run(self):
        rows, query_plans = schedule(self.fixed)
        self.queries = [dict(q) for q in query_plans]
        self.start = time.monotonic()+.2
        for row in rows:
            planned = self.start+row['frame']*.05
            time.sleep(max(0, planned-time.monotonic()))
            observed = dict(row, scheduled_at=planned, entered_at=time.monotonic())
            if row['actor_move'] is not None:
                self.clients[0].move(row['actor_move'])
            if row['frame'] % 20 == 0:
                self.clients[1].move(OBSERVER)
            for c in self.clients:
                c.send('tick_end', explicit=True)
            self.frames.append(observed)
            if 'query' in row:
                with self.lock:
                    self.queries[row['query']].update(requested_at=time.monotonic(), scheduled_at=planned)
                self.futures.append(self.executor.submit(self.sample, row['query']))
        time.sleep(max(0, self.start+len(rows)*.05-time.monotonic()))
        self.end = time.monotonic()
        for future in self.futures:
            future.result(timeout=2)
        self.final_echo_token = self.prefix + '_final_live'
        self.final_echo = self.command('say ' + self.final_echo_token)
        time.sleep(self.fixed['authority']['post_schedule_echo_drain_seconds'])
        return self.snapshot()

    def snapshot(self):
        with self.lock:
            result = dict(protocol=777, prefix=self.prefix, scope='NONCOMBAT_GROUND_ONLY_NOT_WIND_POSE_PROOF',
                          commands=copy.deepcopy(self.commands), queries=copy.deepcopy(self.queries),
                          frames=copy.deepcopy(self.frames), clients=[c.snapshot() for c in self.clients],
                          initial_teleports=getattr(self,'initial_teleports',[]), geometry=getattr(self,'geometry',[]))
            for field in ('start','end','final_echo_token','final_echo'):
                if hasattr(self,field):
                    result[field] = copy.deepcopy(getattr(self,field))
            return result


def analyze(raw, protocol, fixed):
    assert raw['protocol'] == 777 and raw['scope'] == 'NONCOMBAT_GROUND_ONLY_NOT_WIND_POSE_PROOF'
    rows, query_plans = schedule(fixed)
    clients = raw['clients']
    assert len(clients) == 2 and all(c['events'] and not c['failure'] and c['health'] > 0 for c in clients), 'incomplete/live clients'
    decoder = Decoder(protocol)
    start, end = raw['start'], raw['end']
    assert len(raw['frames']) == len(rows), 'partial finite schedule'
    for planned, observed in zip(rows, raw['frames']):
        assert all(observed[k] == v for k,v in planned.items()), 'schedule/phase changed'
        assert abs(observed['scheduled_at']-(start+planned['frame']*.05)) < 1e-7
        assert 0 <= observed['entered_at']-observed['scheduled_at'] <= .025, 'cadence frame late'
    assert raw['final_echo_token']==raw['prefix']+'_final_live', 'wrong final phase token'
    geometry=geometry_checks()
    assert len(raw['geometry'])==len(geometry), 'geometry receipt count'
    for i,(receipt,(x,y,z,material)) in enumerate(zip(raw['geometry'],geometry)):
        token=raw['prefix']+'_geometry_'+str(i)
        assert receipt['command']==f'execute if block {x} {y} {z} minecraft:{material} run say {token}', 'geometry command changed'
        for client in clients:
            phase_echo(client['events'],decoder,token,receipt['at'])
    for name in ('blackbox','blackboxb'):
        inventory=[c for c in raw['commands'] if c['command']=='data get entity '+name+' Inventory' and c['at']<start]
        equipment=[c for c in raw['commands'] if c['command']=='data get entity '+name+' equipment' and c['at']<start]
        assert len(inventory)==len(equipment)==1 and inventory[0]['response'].endswith('[]'), 'missing empty inventory receipt'
        text=equipment[0]['response']
        assert 'Found no elements matching equipment' in text or text.endswith('{}'), 'equipped noncombat player'
    actual_wires = []
    final_echoes = []
    for index, client in enumerate(clients):
        assert any(e['name']=='login' for e in client['events']), 'actual Login missing'
        assert not [e for e in client['outbound'] if e['name'] in FORBIDDEN_OUT], 'NONCOMBAT outbound violation'
        assert not [e for e in client['events'] if e['name'] in FORBIDDEN_IN], 'unexpected Wind/damage/effect'
        assert not [e for e in client['events'] if e['name']=='position' and start <= e['at'] <= end], 'unplanned correction/TP'
        initial = raw['initial_teleports'][index]
        acks = [e for e in client['acks'] if initial['at'] <= e['at'] < start]
        assert len(acks)==1 and same(acks[0]['pose'][:3], ACTOR if index==0 else OBSERVER), 'fresh initial ACK pose'
        tps = [e for e in client['events'] if e['name']=='position' and initial['at'] <= e['at'] < start]
        assert len(tps)==1, 'initial TP count'
        value = decoded(decoder, tps[0])
        assert not any(value['flags'].values()) and all(value[k]==0 for k in ('dx','dy','dz')), 'TP flags/velocity unresolved'
        assert value['teleportId']==acks[0]['id'] and same([value[k] for k in ('x','y','z')], acks[0]['pose'][:3]), 'TP/ACK mismatch'
        wire_acks = [e for e in client['outbound'] if e['name']=='teleport_confirm' and initial['at'] <= e['at'] < start]
        expected_ack = varint(value['teleportId'])+struct.pack('>dddff',*acks[0]['pose'])
        assert len(wire_acks)==1 and bytes.fromhex(wire_acks[0]['hex'])==expected_ack, 'actual wire ACK mismatch'
        packets = [e for e in client['outbound'] if e['name']=='position_look' and start <= e['at'] <= end]
        expected = [(r['frame'],r['actor_move']) for r in rows if r['actor_move'] is not None] if index==0 else [(r['frame'],OBSERVER) for r in rows if r['frame']%20==0]
        assert len(packets)==len(expected), 'missing/extra movement wire'
        moves = []
        for event,(frame_id, position) in zip(packets, expected):
            x,y,z,yaw,pitch,ground = struct.unpack('>dddffB',bytes.fromhex(event['hex']))
            assert same([x,y,z],position) and yaw==pitch==0 and ground==1, 'unplanned ground movement'
            assert 0 <= event['issued_at']-(start+frame_id*.05) <= .025, 'movement cadence'
            moves.append(dict(event=event,frame=frame_id,position=[x,y,z]))
        ticks=[e for e in client['outbound'] if e['name']=='tick_end' and start <= e['at'] <= end]
        assert len(ticks)==len(rows), 'TickEnd grid changed'
        actual_wires.append(moves)
        ages=[decoded(decoder,e)['age'] for e in client['events'] if e['name']=='update_time' and start <= e['at'] <= end]
        assert ages and ages==sorted(ages) and ages[-1]-ages[0]>=200, 'actual world age progress missing'
        modes=[decoded(decoder,e) for e in client['events'] if e['name']=='game_state_change' and e['at']<start]
        assert [v['gameMode'] for v in modes if v['reason']=='change_game_mode'][-1]==0, 'not Survival'
        abilities=[decoded(decoder,e)['flags'] for e in client['events'] if e['name']=='abilities' and e['at']<start]
        assert abilities and not abilities[-1]&6, 'flight capability/state'
        final_echoes.append(phase_echo(client['events'],decoder,raw['final_echo_token'],raw['final_echo']['at']))
    authority=[]
    assert len(raw['queries'])==len(query_plans)==11, 'authority sample count'
    for query, plan in zip(raw['queries'], query_plans):
        assert all(query[k]==v for k,v in plan.items()) and not query.get('error'), 'authority plan/error'
        assert 0 <= query['started_at']-query['scheduled_at'] <= .1, 'authority worker started late'
        assert 0 <= query['ended_at']-query['started_at'] <= 1., 'authority sample duration'
        phase_begin=start+plan['phase_start_frame']*.05
        phase_end=start+plan['phase_end_frame']*.05
        assert phase_begin <= query['started_at'] <= query['ended_at'] <= phase_end, 'authority outside frozen phase'
        assert query['before_token']==raw['prefix']+'_q'+str(plan['index'])+'_before' and query['after_token']==raw['prefix']+'_q'+str(plan['index'])+'_after', 'wrong authority phase token'
        ordered = [query[k] for k in ('before','Pos','OnGround','after')]
        assert all(a['ended_at'] <= b['at'] for a,b in zip(ordered,ordered[1:])), 'RCON command order'
        assert query['Pos']['command']=='data get entity blackbox Pos' and query['OnGround']['command']=='data get entity blackbox OnGround', 'wrong authority command'
        position=parse_pos(query['Pos']['response'])
        assert parse_ground(query['OnGround']['response']), 'authoritative nonground state'
        submitted=[ACTOR]+[m['position'] for m in actual_wires[0] if m['event']['issued_at'] <= query['Pos']['ended_at']]
        assert any(same(position,p) for p in submitted), 'authoritative Pos not on actual submitted path'
        if plan['phase_kind']=='hold':
            assert same(position,plan['expected_hold']), 'held authoritative Pos mismatch'
        echoes=[]
        for client in clients:
            before=phase_echo(client['events'],decoder,query['before_token'],query['before']['at'])
            after=phase_echo(client['events'],decoder,query['after_token'],query['after']['at'])
            assert before['index'] < after['index'], 'phase echo order'
            echoes.append(dict(before=before,after=after,
                               before_age=age_bracket(client['events'],before['index'],decoder),
                               after_age=age_bracket(client['events'],after['index'],decoder)))
        authority.append(dict(index=plan['index'],phase=plan['phase'],phase_kind=plan['phase_kind'],
                              actual_Pos=position,actual_OnGround=True,echoes=echoes,
                              command_interval=[query['Pos']['at'],query['Pos']['ended_at']],
                              note='Pos and OnGround separately read; no atomic pose/current tracker claim'))
    tracked=tracker_steps(clients[1]['events'],decoder,clients[0]['entity'])
    measured=[p for p in tracked if start <= p['at'] <= end]
    assert measured, 'no observer coordinate packets'
    for packet in measured:
        submitted=[ACTOR]+[m['position'] for m in actual_wires[0] if m['event']['issued_at'] <= packet['at']]
        assert all(any(same(position,p,1/4096) for p in submitted) for position in packet['positions']), 'unexplained peer coordinate endpoint'
    held=[]
    for sample in authority:
        if sample['phase_kind']!='hold':
            continue
        echo=sample['echoes'][1]
        before=[p for p in tracked if p['index'] < echo['before']['index']]
        after=[p for p in tracked if p['index'] < echo['after']['index']]
        assert before and after, 'no peer base before held authority'
        agrees=all(same(p[-1]['positions'][-1],sample['actual_Pos'],1/4096) for p in (before,after))
        held.append(dict(authority_index=sample['index'],agrees=agrees,
                         before_endpoint=before[-1],after_endpoint=after[-1],
                         note='held ground endpoint only, NOT moving/falling/combat timing proof'))
    gaps=[emission_gap_observation(a,b) for a,b in zip(measured,measured[1:])]
    excluded=[g for g in gaps if g['excludes_elapsed_emission_gap']]
    return dict(status='COMPLETE_NONCOMBAT_OBSERVATIONS_OFFSET_SEMANTICS_UNRESOLVED',
                authority=authority,held_endpoint_observations=held,tracker_packets=measured,
                emission_gap_observations=gaps,
                elapsed_coordinate_emission_gap_hypothesis='EXCLUDED_BY_ORDERED_AGE_BRACKET' if excluded else 'UNRESOLVED',
                exclusion_receipts=excluded,
                interpolation_vs_sample_timing='UNIDENTIFIABLE_FROM_THIS_SERVER_ONLY_GROUND_EXPERIMENT',
                future_nonzero_Wind_pose_eligibility=False,ground_is_not_fall_or_combat_proof=True,
                automatic_tag_policy_change=False,final_echoes=final_echoes)


def status(port):
    with socket.create_connection(('127.0.0.1',port),timeout=5) as sock:
        handshake(sock,777,port,1)
        send(sock,b'\0')
        packet=frame(sock)
        assert read_varint(packet.read)==0
        value=json.loads(packet.read(read_varint(packet.read)))
        assert value['version']=={'name':'26.3','protocol':777}
        return value


def ready(proc,log):
    deadline=time.monotonic()+120
    errors=[]
    while time.monotonic()<deadline:
        assert proc.poll() is None,'original exited during startup'
        done=[s for s in log.read_text(errors='replace').splitlines() if '[Server thread/INFO]: Done (' in s]
        if done:
            try:
                return dict(done=done[-1],status=status(PORT),earlier_status_errors=errors)
            except (OSError,EOFError,ValueError,AssertionError) as error:
                errors.append(repr(error))
        time.sleep(.1)
    raise TimeoutError('Done + native777 status not ready: '+repr(errors))


def run(output):
    assert os.environ.get('GITHUB_ACTIONS')=='true','GitHub-only; no local server'
    assert os.environ.get('GITHUB_RUN_ATTEMPT')=='1','no automatic retry'
    fixed=criteria()
    output.mkdir(parents=True,exist_ok=False)
    source=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    manifest=dict(source=source,tree=subprocess.check_output(['git','rev-parse','HEAD^{tree}'],text=True).strip(),
                  base='8b9a87182b7f540ff56d1afa45775078882739b4',jar_sha256=None,
                  criteria_sha256=sha(CRITERIA),plan_sha256=sha(PLAN),data_sha256={},
                  status='running',stage='source-frozen',scope='NONCOMBAT_GROUND_ONLY_NO_WIND_OR_CORE',helpers={})
    lifecycle=dict(servers=[],cleanup=[],cleanup_errors=[],ports_free={})
    def save():write(output/'lifecycle.json',lifecycle)
    servers,clients,capture=[],[],None
    handlers={sig:signal.signal(sig,base.interrupt_run) for sig in (signal.SIGALRM,signal.SIGTERM,signal.SIGINT)}
    signal.alarm(300)
    try:
        assert source==os.environ['GITHUB_SHA']
        pin_path=ROOT/'tools/blackbox/ci/sources-777.json'
        own=[Path(__file__),CRITERIA,PLAN,pin_path,ROOT/'.github/workflows/tracker-ground-offset.yml']
        own+=list((ROOT/'tools/blackbox').glob('*.py'))+list((ROOT/'tools/protocol-bridge').glob('*.py'))
        for path in set(own):
            name=path.relative_to(ROOT);target=output/'source'/name
            target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(path.read_bytes())
            manifest['helpers'][str(name)]=sha(path)
        write(output/'source-before-download.json',manifest)
        manifest['stage']='fetch-inputs'
        work=Path(os.environ['RUNNER_TEMP'])/'mcsa-tracker-ground-offset';work.mkdir(exist_ok=False)
        folder=work/'original';folder.mkdir()
        data=work/'data';data.mkdir()
        pin=json.loads(pin_path.read_text());jar=folder/'server.jar'
        fetch(pin['official_jar'],jar)
        manifest['jar_sha256']=sha(jar)
        for entry in pin['json_files']:
            if entry['file'] in ('protocol.json','blocks.json'):
                fetch(entry,data/entry['file'])
                (output/entry['file']).write_bytes((data/entry['file']).read_bytes())
                manifest['data_sha256'][entry['file']]=sha(data/entry['file'])
        protocol=json.loads((data/'protocol.json').read_text())
        manifest['stage']='server-config'
        (folder/'eula.txt').write_text('eula=true\n')
        (folder/'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={PORT}\nenable-rcon=true\nrcon.port={PORT+1}\n'
            'rcon.password=local-blackbox\nonline-mode=false\nenforce-secure-profile=false\nwhite-list=false\n'
            'network-compression-threshold=-1\nlevel-type=minecraft:flat\n'
            'generator-settings={"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":3}],"biome":"minecraft:plains"}\n'
            'level-seed=1\ngenerate-structures=false\nspawn-protection=0\nview-distance=3\nsimulation-distance=3\n'
            'allow-flight=true\ngamemode=creative\nmax-tick-time=0\n')
        (output/'server.properties').write_bytes((folder/'server.properties').read_bytes())
        write(output/'before.json',manifest)
        assert all(base.ports_free([PORT,PORT+1]).values()),'isolated ports occupied'
        log=(output/'original-server.log').open('w')
        argv=['java','-Xmx1G','-XX:ActiveProcessorCount=2','-jar',str(jar),'nogui']
        proc=subprocess.Popen(argv,cwd=folder,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        servers.append((proc,log,None,PORT))
        lifecycle['servers'].append(dict(pid=proc.pid,port=PORT,argv=argv,identity=process_identity(proc.pid)));save()
        manifest['stage']='startup'
        manifest['startup']=ready(proc,output/'original-server.log')
        control=base.Rcon(PORT+1,'local-blackbox');servers[-1]=(proc,log,control,PORT)
        manifest['stage']='clients'
        for name in ('blackbox','blackboxb'):
            configuration=[]
            def observe_config(sock,packet_id,payload):
                configuration.append(dict(id=packet_id,hex=payload.hex()))
                return False
            c=base.connect(PORT,protocol,TraceClient,version=777,username=name,configuration_handler=observe_config)
            clients.append(c)
            write(output/(name+'-configuration.json'),configuration)
            c.pump(15,lambda row:row['name']=='position');c.start()
        capture=Capture(clients,control,fixed,output,'tracker_'+os.environ['GITHUB_RUN_ID'])
        manifest['stage']='prepare-ground'
        capture.prepare()
        manifest['stage']='fixed-schedule'
        raw=capture.run()
        path=output/'raw.json';write(path,raw)
        manifest['raw_frozen_sha256']=sha(path)
        manifest['stage']='analyze'
        result=analyze(raw,protocol,fixed)
        assert sha(path)==manifest['raw_frozen_sha256'],'raw changed during analysis'
        write(output/'observations.json',result)
        manifest['status']=result['status'];manifest['stage']='complete'
    except BaseException as error:
        import traceback
        manifest['status']='TECHNICAL_INCONCLUSIVE_STOP_NO_RETRY'
        manifest['error']=repr(error)
        (output/'failure.txt').write_text(traceback.format_exc())
    finally:
        signal.alarm(0)
        if capture:
            write(output/'final-partial-streams.json',capture.snapshot())
            capture.executor.shutdown(wait=False,cancel_futures=True)
        else:
            write(output/'final-partial-streams.json',dict(clients=[c.snapshot() for c in clients]))
        for c in clients:c.close()
        base.cleanup_servers(servers,lifecycle,save,[PORT,PORT+1])
        for sig,handler in handlers.items():signal.signal(sig,handler)
        manifest['cleanup_ok']=(len(lifecycle['cleanup'])==1 and not lifecycle['cleanup_errors'] and
                                all(r.get('exit_code')==0 and r.get('group_gone') for r in lifecycle['cleanup']) and
                                all(lifecycle['ports_free'].values()))
        manifest['evidence_sha256']={str(p.relative_to(output)):sha(p) for p in output.rglob('*') if p.is_file()}
        write(output/'after.json',manifest)
    assert manifest['status']=='COMPLETE_NONCOMBAT_OBSERVATIONS_OFFSET_SEMANTICS_UNRESOLVED' and manifest['cleanup_ok'],manifest.get('error',manifest['status'])


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    run(parser.parse_args().output)
