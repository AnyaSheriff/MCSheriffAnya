#!/usr/bin/env python3
"""Один ORIGINAL777 pilot; JAR непрозрачен, gameplay/core не запускаются."""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import threading
import time
import traceback
import uuid

import record777 as base
from analyze777 import Decoder
from combat_ci import fetch
from noai_ambient_audit import sound_name
from combat_process import process_identity
from smoke import handshake, send, frame, varint, read_varint

ROOT = Path(__file__).resolve().parents[2]
CRITERIA = ROOT / 'tools/blackbox/ci/wind-origin-original.json'
CRITERIA_SHA256 = '87b357cd307e30f52e2021e743d38f89e4967ef1f6b968f014fdb705affcbaab'
ORDER = ['air', 'stone', 'obsidian', 'bedrock', 'barrier', 'air']
START = [100.5, 67.38627014995919, 100.5]
CELL = [100, 63, 100]
PORT = 26184
SOUNDS = ['minecraft:item.mace.smash_air', 'minecraft:item.mace.smash_ground',
          'minecraft:item.mace.smash_ground_heavy', 'minecraft:entity.wind_charge.wind_burst',
          'minecraft:entity.breeze.wind_burst']


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def decode(decoder, name, payload):
    stream = io.BytesIO(payload)
    result = decoder.read(stream, 'packet_' + name)
    assert not stream.read(), 'trailing bytes: ' + name
    return result


def inside(center):
    margin = 1 / 4096
    return all(math.isfinite(center[k]) and CELL[i] + margin <= center[k] <= CELL[i] + 1 - margin
               for i, k in enumerate(('x', 'y', 'z')))


def blocks(material):
    terrain = [(x, 59, z) for x in range(92, 110) for z in range(92, 110)]
    terrain += [(100, y, 102) for y in range(60, 64)] + [(100, 64, 104)]
    if material != 'air':
        terrain.append(tuple(CELL))
    return terrain


def feedback(motion, own_entity, name, value):
    if name == 'entity_velocity':
        return [value['velocity'][k] for k in ('x', 'y', 'z')] if value['entityId'] == own_entity else list(motion)
    if name == 'explosion' and value['playerKnockback'] is not None:
        return [motion[i] + value['playerKnockback'][k] for i,k in enumerate(('x','y','z'))]
    return list(motion)


def move(position, motion, terrain):
    """Собственная swept AABB только для явных full blocks этой площадки."""
    p, v = list(position), list(motion)
    box = [p[0] - .3, p[1], p[2] - .3, p[0] + .3, p[1] + 1.8, p[2] + .3]
    grounded = False
    for axis in (1, 0, 2):
        delta = v[axis]
        for block in terrain:
            others = [j for j in range(3) if j != axis]
            if not all(box[j + 3] > block[j] and box[j] < block[j] + 1 for j in others):
                continue
            if delta > 0 and box[axis + 3] <= block[axis]:
                delta = min(delta, block[axis] - box[axis + 3])
            elif delta < 0 and box[axis] >= block[axis] + 1:
                delta = max(delta, block[axis] + 1 - box[axis])
        if delta != v[axis]:
            grounded |= axis == 1 and v[axis] < 0
            v[axis] = 0.
        p[axis] += delta
        box[axis] += delta
        box[axis + 3] += delta
    v[1] = (v[1] - .08) * .98
    drag = .546 if grounded else .91
    v[0] *= drag
    v[2] *= drag
    return p, v, grounded


class PilotClient(base.Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.decoder = Decoder(protocol)
        self.events, self.outbound, self.acks, self.moves = [], [], [], []
        self.motion = [0., 0., 0.]
        self.entity, self.health, self.failure = None, None, None
        self.registries = {}
        self.lock = threading.RLock()
        self.running = True
        self.terrain = blocks('air')

    def send(self, name, payload=b'', *, explicit=False):
        # Client.pump не управляет physics grid. Ответы KeepAlive/TPACK остаются.
        if name == 'tick_end' and not explicit:
            return
        with self.lock:
            super().send(name, payload)
            self.outbound.append(dict(at=time.monotonic(), name=name, hex=payload.hex()))

    def confirm_teleport(self, teleport, values):
        super().confirm_teleport(teleport, values)
        self.acks.append(dict(at=time.monotonic(), id=teleport, pose=list(values), motion=list(self.motion)))

    def observe_packet(self, name, payload):
        with self.lock:
            self.events.append(dict(at=time.monotonic(), name=name, hex=payload.hex()))
            if name == 'login':
                self.entity = struct.unpack('>i', payload[:4])[0]
            elif name == 'update_health':
                self.health = struct.unpack('>f', payload[:4])[0]
            elif name == 'position':
                data = io.BytesIO(payload)
                read_varint(data.read)
                values = struct.unpack('>ddddddffI', data.read())
                flags = values[-1]
                assert not flags & 256, 'unhandled velocity rotation'
                self.motion = [values[3+i] + (self.motion[i] if flags & (32 << i) else 0) for i in range(3)]
            elif name == 'entity_velocity':
                value = decode(self.decoder, name, payload)
                self.motion = feedback(self.motion, self.entity, name, value)
            elif name == 'explosion':
                value = decode(self.decoder, name, payload)
                if value['playerKnockback'] is not None:
                    # Explosion несёт добавочный импульс; EntityVelocity выше
                    # заменяет motion. Обрабатываем в фактическом wire order.
                    self.motion = feedback(self.motion, self.entity, name, value)

    def start(self):
        def worker():
            try:
                while self.running:
                    self.pump(.05)
            except BaseException as error:
                if self.running:
                    self.failure = repr(error)
        self.thread = threading.Thread(target=worker, daemon=True)
        self.thread.start()

    def step(self):
        with self.lock:
            assert self.position is not None and not self.failure, self.failure
            pos, self.motion, grounded = move(self.position[:3], self.motion, self.terrain)
            assert 92.3 <= pos[0] <= 109.7 and 92.3 <= pos[2] <= 109.7, 'left prepared terrain'
            self.position[:3] = pos
            wire = struct.pack('>dddffB', *self.position, int(grounded))
            self.send('position_look', wire)
            self.moves.append(dict(at=time.monotonic(), position=pos, motion=list(self.motion), onGround=grounded))

    def close(self):
        self.running = False
        self.sock.close()
        if hasattr(self, 'thread'):
            self.thread.join(2)

    def snapshot(self):
        with self.lock:
            return dict(entity=self.entity, events=list(self.events), outbound=list(self.outbound),
                        acks=list(self.acks), moves=list(self.moves), failure=self.failure,
                        registries=self.registries)


def peer_positions(events, decoder, entity):
    """Реконструкция только действительных tracker packets777, не local pose."""
    pos = None
    rows = []
    for event in events:
        name = event['name']
        if name not in {'spawn_entity', 'sync_entity_position', 'entity_teleport', 'rel_entity_move', 'entity_move_look'}:
            continue
        value = decode(decoder, name, bytes.fromhex(event['hex']))
        if value['entityId'] != entity:
            continue
        if name in {'spawn_entity', 'sync_entity_position', 'entity_teleport'}:
            if name == 'sync_entity_position' and value['positionType'] == 'stepped':
                for step in value['steps']:
                    pos = [step[k] for k in ('x', 'y', 'z')]
                    rows.append(dict(at=event['at'], position=list(pos), source=name, tickOffset=step['tickOffset']))
                continue
            if name == 'entity_teleport' and any(value['flags'].values()):
                raise ValueError('relative peer teleport requires separate proof')
            pos = [value[k] for k in ('x', 'y', 'z')]
        else:
            assert pos is not None, 'delta without peer base'
            for step in value['move']['steps']:
                pos = [pos[i] + step['delta'][i] / 4096 for i in range(3)]
                rows.append(dict(at=event['at'], position=list(pos), source=name, onGround=value['move']['onGround']))
            continue
        rows.append(dict(at=event['at'], position=list(pos), source=name, onGround=value.get('onGround')))
    return rows


def audit(raw, protocol, items, block_states):
    """Без поправок stimulus/критериев после original. Ошибка = stop."""
    decoder = Decoder(protocol)
    clients, trial = raw['clients'], raw['trial']
    assert raw['protocol'] == 777 and len(clients) == 2
    assert all(c['events'] and not c['failure'] for c in clients), 'INCONCLUSIVE_CONNECTION'
    actor, victim = (c['entity'] for c in clients)
    attack, end = trial['attack_at'], trial['end_at']
    events = [[dict(e, value=decode(decoder, e['name'], bytes.fromhex(e['hex'])))
               for e in c['events'] if e['name'] in {'damage_event', 'sound_effect', 'entity_sound_effect',
                  'explosion', 'update_time', 'update_health', 'window_items', 'entity_effect', 'remove_entity_effect', 'world_particles',
                  'game_state_change', 'abilities', 'entity_velocity', 'block_change', 'multi_block_change'}]
              for c in clients]
    for c in clients:
        assert any(e['name'] == 'login' for e in c['events']), 'INCONCLUSIVE_LOGIN'
        assert any(e['name'] == 'system_chat' and trial['echo'].encode().hex() in e['hex'] and e['at'] > end for e in c['events']), 'INCONCLUSIVE_LIVE'
    acks = [a for a in clients[0]['acks'] if trial['start_tp_at'] <= a['at'] < attack]
    assert len(acks) == 1 and acks[0]['pose'][:3] == START and acks[0]['motion'] == [0, 0, 0], 'INCONCLUSIVE_ACK'
    tp_packets = [e for e in clients[0]['events'] if e['name'] == 'position'
                  and trial['start_tp_at'] <= e['at'] <= acks[0]['at']]
    assert len(tp_packets) == 1, 'INCONCLUSIVE_TP_PACKET'
    tp = io.BytesIO(bytes.fromhex(tp_packets[0]['hex']))
    teleport_id = read_varint(tp.read)
    values = struct.unpack('>ddddddffI', tp.read())
    assert teleport_id == acks[0]['id'] and list(values[:3]) == START and list(values[3:6]) == [0,0,0] and values[-1] == 0, 'INCONCLUSIVE_TP_FLAGS'
    ack_wire = [e for e in clients[0]['outbound'] if e['name'] == 'teleport_confirm'
                and trial['start_tp_at'] <= e['at'] < attack]
    expected_ack = varint(teleport_id) + struct.pack('>dddff', *acks[0]['pose'])
    assert len(ack_wire) == 1 and ack_wire[0]['hex'] == expected_ack.hex(), 'INCONCLUSIVE_TP_WIRE_ACK'
    assert not [e for e in clients[0]['events'] if e['name'] == 'position' and acks[0]['at'] < e['at'] <= end], 'INCONCLUSIVE_CORRECTION'
    wires = [e for e in clients[0]['outbound'] if e['name'] == 'attack' and trial['start_tp_at'] <= e['at'] <= end]
    assert len(wires) == 1 and wires[0]['hex'] == varint(victim).hex(), 'INCONCLUSIVE_ATTACK'
    moves = [e for e in clients[0]['moves'] if acks[0]['at'] <= e['at'] < attack]
    assert len(moves) == 10 and all(not m['onGround'] and m['position'][1] > 64 for m in moves), 'INCONCLUSIVE_FALL'
    actual_moves = [e for e in clients[0]['outbound'] if e['name'] == 'position_look' and acks[0]['at'] <= e['at'] < attack]
    assert len(actual_moves) == 10, 'INCONCLUSIVE_MOVEMENT_WIRE'
    for recorded, sent in zip(moves, actual_moves):
        x,y,z,yaw,pitch,ground = struct.unpack('>dddffB', bytes.fromhex(sent['hex']))
        assert [x,y,z] == recorded['position'] and bool(ground) == recorded['onGround'], 'INCONCLUSIVE_LOCAL_ONLY_POSE'
    assert attack - actual_moves[-1]['at'] < .025, 'INCONCLUSIVE_DELAYED_ATTACK'
    assert START[1] - moves[-1]['position'][1] > 1.5
    assert 64.02 <= moves[-1]['position'][1] <= 64.06
    wire = clients[0]['outbound']
    index = wire.index(wires[0])
    assert wire[index-1]['name'] == 'position_look' and wire[index+1]['name'] == 'tick_end', 'INCONCLUSIVE_WIRE_ORDER'
    peers = peer_positions(clients[1]['events'], decoder, actor)
    # Доставка tracker может прийти после Attack, но только до следующего
    # движения: не используем поздний landing pose как pre-hit evidence.
    nextmove = next(e['at'] for e in clients[0]['moves'] if e['at'] > attack)
    bound = [p for p in peers if moves[-1]['at'] - .025 <= p['at'] < nextmove]
    assert any(100.49 <= p['position'][0] <= 100.51 and 64.02 <= p['position'][1] <= 64.06
               and 100.49 <= p['position'][2] <= 100.51 and not p.get('onGround', False) for p in bound), 'INCONCLUSIVE_SERVER_POSE'
    damages = [e for e in events[1] if e['name'] == 'damage_event' and attack <= e['at'] <= attack + 1
               and e['value']['entityId'] == victim]
    names = clients[1]['registries']['minecraft:damage_type']
    assert len([e for e in damages if names[e['value']['sourceTypeId']] == 'minecraft:mace_smash'
                and e['value']['sourceCauseId'] == actor+1 and e['value']['sourceDirectId'] == actor+1]) == 1, 'INCONCLUSIVE_SMASH_SOURCE'
    sounds = [sound_name(e['value']['sound'], raw['sounds']) for e in events[1]
              if e['name'] in {'sound_effect', 'entity_sound_effect'} and attack <= e['at'] <= attack+1]
    assert any(s in SOUNDS[:3] for s in sounds), 'INCONCLUSIVE_SMASH_SOUND'
    hp = [(e['at'], e['value']['health']) for e in events[1] if e['name'] == 'update_health']
    pre = [h for at, h in hp if trial['reset_at'] <= at < attack]
    post = [h for at, h in hp if attack <= at < attack+1]
    assert pre and pre[-1] == 200 and post and 0 < post[0] < 200 and all(h > 0 for at,h in hp if at >= trial['reset_at']), 'INCONCLUSIVE_HP'
    ages = [[(e['at'], e['value']['age']) for e in side if e['name'] == 'update_time'] for side in events]
    for side in ages:
        before = [age for at,age in side if at < attack]
        after = [age for at,age in side if attack < at <= end]
        reset = [age for at,age in side if trial['quiet_at'] <= at < attack]
        assert before and after and after[-1] - before[-1] >= 60, 'INCONCLUSIVE_WINDOW'
        assert len(reset) >= 3 and reset[-1] - reset[0] >= 40, 'INCONCLUSIVE_COOLDOWN'
    menus = [e['value'] for e in events[0] if e['name'] == 'window_items' and trial['reset_at'] <= e['at'] < attack]
    assert menus and len(menus[-1]['items']) == 63 and menus[-1]['carriedItem']['itemCount'] == 0, 'INCONCLUSIVE_MENU'
    occupied = [s for s in menus[-1]['items'] if s['itemCount']]
    assert len(occupied) == 1 and occupied[0]['itemCount'] == 1 and items[occupied[0]['itemId']] == 'mace', 'INCONCLUSIVE_ITEM'
    enchantments = [c['data']['enchantments'] for c in occupied[0]['components'] if c['type'] == 'enchantments']
    ench_names = clients[0]['registries']['minecraft:enchantment']
    assert len(enchantments) == 1 and [(ench_names[x['id']], x['level']) for x in enchantments[0]] == [('minecraft:wind_burst', 1)], 'INCONCLUSIVE_ENCHANT'
    assert trial['victim_inventory_receipt']['response'].rstrip().endswith('[]'), 'INCONCLUSIVE_VICTIM_EQUIPMENT'
    for side in events:
        active = {}
        for e in side:
            if e['at'] > attack:
                break
            if e['name'] == 'entity_effect':
                active[(e['value']['entityId'], e['value']['effectId'])] = e
            elif e['name'] == 'remove_entity_effect':
                active.pop((e['value']['entityId'], e['value']['effectId']), None)
        assert not active, 'INCONCLUSIVE_ACTIVE_EFFECT'
    assert not [e for side in events for e in side if e['name'] == 'entity_effect' and trial['quiet_at'] <= e['at'] <= end], 'INCONCLUSIVE_EFFECT'
    for index, side in enumerate(events):
        modes = [e['value']['gameMode'] for e in side if e['name'] == 'game_state_change'
                 and e['value']['reason'] == 'change_game_mode' and e['at'] < attack]
        ability = [e['value']['flags'] for e in side if e['name'] == 'abilities' and e['at'] < attack]
        assert modes and modes[-1] == 0 and ability and not ability[-1] & 6, 'INCONCLUSIVE_MODE_FLIGHT'
        changes = []
        for e in side:
            if not trial['cell_change_at'] <= e['at'] < attack:
                continue
            if e['name'] == 'block_change' and [e['value']['location'][k] for k in ('x','y','z')] == CELL:
                changes.append(e['value']['type'])
            if e['name'] == 'multi_block_change':
                chunk = e['value']['chunkCoordinates']
                for entry in e['value']['records']:
                    pos = [chunk['x']*16 + ((entry >> 8)&15), chunk['y']*16 + (entry&15), chunk['z']*16 + ((entry >> 4)&15)]
                    if pos == CELL:
                        changes.append(entry >> 12)
        assert changes and changes[-1] == block_states[trial['material']], 'INCONCLUSIVE_CELL_STATE'
    explosions = [e for e in events[0] if e['name'] == 'explosion' and attack <= e['at'] <= end]
    observer_explosions = [[e for e in side if e['name'] == 'explosion' and attack <= e['at'] <= end] for side in events]
    origin_bound = bool(explosions) and all(inside(e['value']['center']) for e in explosions)
    wind_sounds = [sound_name(e['value']['sound'], raw['sounds']) for side in events for e in side
                   if e['name'] in {'sound_effect', 'entity_sound_effect'} and attack <= e['at'] <= end
                   and sound_name(e['value']['sound'], raw['sounds']) in SOUNDS[3:]]
    wind_particles = [e for side in events for e in side if e['name'] == 'world_particles' and attack <= e['at'] <= end
                      and 'gust' in e['value']['particle']['type']]
    feedback = [e for e in events[0] if e['name'] == 'entity_velocity' and attack <= e['at'] <= end
                and e['value']['entityId'] == actor]
    lift_feedback = any(e['value']['velocity']['y'] > .1 for e in feedback)
    if not explosions and (any(observer_explosions) or wind_sounds or wind_particles or lift_feedback):
        effect = 'PARTIAL_EFFECT_OR_NOTIFICATION_AMBIGUITY'
    elif explosions:
        effect = 'EVENT_PRESENT_COUNT_ZERO' if all(e['value']['blockCount'] == 0 for e in explosions) else 'EVENT_PRESENT_COUNT_NONZERO'
    else:
        effect = 'QUALIFIED_NO_WIND_OBSERVED_CONDITIONAL_UNDERFOOT_ONLY'
    if trial['index'] in (0, 5):
        assert explosions and all(sound_name(e['value']['sound'], raw['sounds']) in SOUNDS[3:] for e in explosions), 'INCONCLUSIVE_AIR_CONTROL'
    return dict(status='qualified-observation', material=trial['material'], explosion_present=bool(explosions),
                explosions=explosions, explosions_by_observer=observer_explosions, smash_sounds=sounds, peer_pose=bound,
                A_origin='EVENT_PRESENT_C_INSIDE_U' if origin_bound else 'INCONCLUSIVE_ORIGIN_GEOMETRY',
                B_effect=effect, wind_particles=wind_particles, feedback=feedback,
                note='N1; absent event has no measured centre; no named immunity/tag policy claim')


class Experiment:
    def __init__(self, control, clients, output, stone_id):
        self.control, self.clients, self.output = control, clients, output
        self.stone_id = stone_id
        self.commands = []
        self.sequence = 0
        self.sounds = {}
        self.calibration = []
        self.current_trial = None

    def command(self, text):
        row = dict(at=time.monotonic(), command=text)
        self.commands.append(row)
        row['response'] = self.control.run(text)
        return row

    def frames(self, count):
        deadline = time.monotonic()
        for _ in range(count):
            for c in self.clients:
                c.step()
                c.send('tick_end', explicit=True)
            deadline += .05
            time.sleep(max(0, deadline - time.monotonic()))

    def echo(self, token):
        self.command('tellraw @a ' + json.dumps(dict(text=token)))
        self.frames(4)
        assert all(any(e['name'] == 'system_chat' and token.encode().hex() in e['hex']
                       for e in c.events) for c in self.clients), 'missing unique live echo'

    def teleport(self, index, position):
        client = self.clients[index]
        before = len(client.acks)
        row = self.command('tp ' + ('blackboxb' if index else 'blackbox') + ' ' +
                           ' '.join(map(str, position)) + ' 0 0')
        limit = time.monotonic() + 2
        while len(client.acks) == before and time.monotonic() < limit:
            time.sleep(.005)
        assert len(client.acks) == before + 1 and client.acks[-1]['pose'][:3] == position, 'fresh TP ACK'
        return row

    def prepare(self):
        self.command('gamerule spawn_mobs false')
        self.command('gamerule natural_health_regeneration false')
        self.command('difficulty normal')
        self.command('kill @e[type=!player]')
        self.command('forceload add 92 92 109 109')
        self.command('fill 92 59 92 109 100 109 air')
        self.command('fill 92 59 92 109 59 109 stone')
        self.command('fill 100 60 102 100 63 102 stone')
        self.command('setblock 100 64 104 chest[facing=north]')
        for name in ('blackbox', 'blackboxb'):
            self.command('gamemode survival ' + name)
            self.command('attribute ' + name + ' minecraft:max_health base set 200')
            self.command('effect give ' + name + ' instant_health 1 10')
            self.command('effect clear ' + name)
            self.command('clear ' + name)
        for i in range(2):
            self.teleport(i, [100.5, 64., 102.5])
        self.frames(5)
        # Имена звуков проверяются в777 обычным consumable Slot, без static IDs.
        for name in SOUNDS:
            before = len(self.clients[0].events)
            command_receipt = self.command('item replace entity blackbox weapon.mainhand with minecraft:stone[consumable={sound:"' + name + '"}]')
            self.frames(4)
            observed = []
            for event in self.clients[0].events[before:]:
                if event['name'] not in ('set_slot', 'set_player_inventory'):
                    continue
                value = decode(self.clients[0].decoder, event['name'], bytes.fromhex(event['hex']))
                slot = value.get('item', value.get('contents', {}))
                for component in slot.get('components', []):
                    if component['type'] == 'consumable':
                        holder = component['data']['sound']
                        assert set(holder) == {'soundId'}, 'sound provenance not numeric'
                        assert slot['itemCount'] == 1 and slot['itemId'] == self.stone_id, 'sound provenance wrong stone slot'
                        assert event['at'] >= command_receipt['at'], 'stale sound slot'
                        self.sounds[str(holder['soundId'])] = name
                        self.calibration.append(dict(requested_name=name, command_receipt=command_receipt,
                                                     actual_event=event, decoded_slot=slot, numeric_holder=holder))
                        observed.append(event)
            assert observed, '777 sound calibration missing: ' + name
        write(self.output / 'sound-provenance.json', dict(name_candidates_source='our existing sounds.json names only; numeric IDs independently observed on this original777 before trials', name_candidates_sha256=sha(ROOT / 'tools/blackbox/data/sounds.json'), sounds=self.sounds, calibration=self.calibration, clients=[c.snapshot() for c in self.clients]))
        self.command('clear blackbox')
        # Только значимые свободные клетки тела/траектории, без подмены U.
        checks = {(100,y,100) for y in range(64,72)} | {(100,y,z) for y in range(64,67) for z in (101,102)}
        for x,y,z in sorted(checks):
            token = f'wind_geometry_{x}_{y}_{z}'
            receipt = self.command(f'execute if block {x} {y} {z} minecraft:air run say {token}')
            assert token in receipt['response'], 'uncleared fall/body cell'

    def trial(self, index, material):
        actor, victim = self.clients
        row = dict(index=index, material=material, reset_at=time.monotonic())
        self.current_trial = row
        write(self.output / f'trial-{index}-start.json', row)
        self.command('kill @e[type=!player]')
        for name in ('blackbox', 'blackboxb'):
            self.command('clear ' + name)
            self.command('effect clear ' + name)
            self.command('damage ' + name + ' 1 minecraft:generic')
            self.command('effect give ' + name + ' instant_health 1 10')
        for i in range(2):
            self.teleport(i, [100.5, 64., 102.5])
        # Изменение U подтверждается реальным block-change и named if-block.
        self.command('setblock 100 63 100 glass')
        self.frames(2)
        changed_at = time.monotonic()
        self.command('setblock 100 63 100 ' + material)
        for c in self.clients:
            c.terrain = blocks(material)
        self.command('item replace entity blackbox weapon.mainhand with minecraft:mace[enchantments={wind_burst:1}]')
        actor.send('held_item_slot', struct.pack('>h', 0))
        self.frames(4)
        token = 'wind_cell_' + str(index)
        check = self.command('execute if block 100 63 100 minecraft:' + material + ' run say ' + token)
        assert token in check['response'], 'varied named block not confirmed'
        for c in self.clients:
            changes = [r for r in c.records if r['name'] == 'block_change' and r.get('position') == CELL]
            assert changes, 'actual varied block receipt missing'
        row['cell_change_at'] = changed_at
        # Full menu echo includes complete player inventory (chest itself empty).
        before = len(actor.events)
        self.sequence += 1
        actor.send('block_place', b'\0' + base.pack_position([100, 64, 104]) + varint(2) +
                   struct.pack('>fff', .5, .5, .5) + b'\0\0' + varint(self.sequence))
        self.frames(8)
        menus = [e for e in actor.events[before:] if e['name'] == 'window_items']
        assert menus, 'fresh full inventory menu missing'
        menu = decode(actor.decoder, 'window_items', bytes.fromhex(menus[-1]['hex']))
        actor.send('close_window', varint(menu['windowId']))
        self.command('effect clear blackbox')
        self.command('effect clear blackboxb')
        row['victim_inventory_receipt'] = self.command('data get entity blackboxb Inventory')
        assert row['victim_inventory_receipt']['response'].rstrip().endswith('[]'), 'victim inventory/armor/offhand not empty'
        row['quiet_at'] = time.monotonic()
        # 80 real cadence frames give >=40 world ticks via independently checked age.
        self.frames(80)
        assert victim.health == 200, 'actual pre-hit HP not 200'
        assert all(c.health and c.health > 0 and not c.failure for c in self.clients)
        row['start_tp_at'] = self.teleport(0, START)['at']
        start = time.monotonic()
        for tick in range(10):
            time.sleep(max(0, start + tick * .05 - time.monotonic()))
            # Attack immediately follows move10, before its TickEnd.
            with actor.lock:
                actor.step()
                if tick == 9:
                    row['attack_at'] = time.monotonic()
                    actor.send('attack', varint(victim.entity))
                actor.send('tick_end', explicit=True)
            victim.step()
            victim.send('tick_end', explicit=True)
        time.sleep(max(0, start + .5 - time.monotonic()))
        self.frames(80)
        row['end_at'] = time.monotonic()
        row['echo'] = 'wind_live_' + str(index) + '_' + uuid.uuid4().hex
        self.echo(row['echo'])
        return dict(protocol=777, trial=row, sounds=self.sounds, commands=list(self.commands),
                    clients=[c.snapshot() for c in self.clients])


def status(port):
    with socket.create_connection(('127.0.0.1', port), timeout=5) as sock:
        handshake(sock, 777, port, 1)
        send(sock, b'\0')
        packet = frame(sock)
        assert read_varint(packet.read) == 0
        result = json.loads(packet.read(read_varint(packet.read)))
        assert result['version'] == dict(name='26.3', protocol=777), result
        return result


def run(output):
    assert os.environ.get('GITHUB_ACTIONS') == 'true', 'GitHub-only: no local server'
    assert os.environ.get('GITHUB_RUN_ATTEMPT') == '1', 'no retry'
    assert sha(CRITERIA) == CRITERIA_SHA256, 'pre-capture criteria changed'
    output.mkdir(parents=True, exist_ok=False)
    work = Path(os.environ['RUNNER_TEMP']) / 'mcsa-wind-origin-original'
    work.mkdir(exist_ok=False)
    data, folder = work / 'data', work / 'official'
    data.mkdir(); folder.mkdir()
    pin = json.loads((ROOT / 'tools/blackbox/ci/sources-777.json').read_text())
    jar = folder / 'server.jar'
    fetch(pin['official_jar'], jar)
    for entry in pin['json_files']:
        if entry['file'] in ('protocol.json', 'items.json', 'blocks.json', 'entities.json'):
            fetch(entry, data / entry['file'])
    protocol = json.loads((data / 'protocol.json').read_text())
    items = {x['id']:x['name'] for x in json.loads((data / 'items.json').read_text())}
    block_states = {x['name']:x['defaultState'] for x in json.loads((data / 'blocks.json').read_text())}
    manifest = dict(source=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                    tree=subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], text=True).strip(),
                    jar_sha256=sha(jar), criteria_sha256=sha(CRITERIA),
                    data_sha256={p.name:sha(p) for p in data.iterdir()},
                    status='running', trials=[], scope='ORIGINAL777 ONLY; maximum six WB-I attacks; no retry')
    assert manifest['source'] == os.environ['GITHUB_SHA']
    own = [Path(__file__), CRITERIA, ROOT / '.github/workflows/wind-origin-original.yml']
    # All our Python imports are copied for later source replay; no foreign source.
    own += list((ROOT / 'tools/blackbox').glob('*.py')) + list((ROOT / 'tools/protocol-bridge').glob('*.py'))
    source_dir = output / 'source'; source_dir.mkdir()
    manifest['helpers'] = {}
    for path in set(own):
        relative = path.relative_to(ROOT)
        target = source_dir / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(path.read_bytes())
        manifest['helpers'][str(relative)] = sha(path)
    for p in data.iterdir():
        (output / p.name).write_bytes(p.read_bytes())
    write(output / 'before.json', manifest)
    (folder / 'eula.txt').write_text('eula=true\n')
    (folder / 'server.properties').write_text(
        f'server-ip=127.0.0.1\nserver-port={PORT}\nenable-rcon=true\nrcon.port={PORT+1}\n'
        'rcon.password=local-blackbox\nonline-mode=false\nenforce-secure-profile=false\nwhite-list=false\n'
        'network-compression-threshold=-1\nlevel-type=minecraft:flat\n'
        'generator-settings={"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":3}],"biome":"minecraft:plains"}\n'
        'level-seed=1\ngenerate-structures=false\nspawn-protection=0\nview-distance=2\nsimulation-distance=2\n'
        'allow-flight=true\ngamemode=creative\nmax-tick-time=0\n')
    (output / 'server.properties').write_bytes((folder / 'server.properties').read_bytes())
    ops = []
    for name in ('blackbox', 'blackboxb'):
        ident = bytearray(hashlib.md5(('OfflinePlayer:' + name).encode()).digest())
        ident[6] = (ident[6] & 15) | 48; ident[8] = (ident[8] & 63) | 128
        ops.append(dict(uuid=str(uuid.UUID(bytes=bytes(ident))), name=name, level=4))
    write(folder / 'ops.json', ops)
    lifecycle = dict(servers=[], cleanup=[], cleanup_errors=[], ports_free={})
    def save():
        write(output / 'lifecycle.json', lifecycle)
    clients, servers, experiment = [], [], None
    handlers = {sig:signal.signal(sig, base.interrupt_run) for sig in (signal.SIGALRM, signal.SIGTERM, signal.SIGINT)}
    signal.alarm(600)
    try:
        assert all(base.ports_free([PORT, PORT+1]).values()), 'isolated ports occupied'
        log = (output / 'official-server.log').open('w')
        proc = subprocess.Popen(['java', '-Xmx1G', '-XX:ActiveProcessorCount=2', '-jar', str(jar), 'nogui'],
                                cwd=folder, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
                                start_new_session=True)
        servers.append((proc, log, None, PORT))
        lifecycle['servers'].append(dict(pid=proc.pid, identity=process_identity(proc.pid), port=PORT))
        save()
        limit = time.monotonic() + 120
        while True:
            assert proc.poll() is None, 'original exited during startup'
            try:
                control = base.Rcon(PORT+1, 'local-blackbox')
                break
            except OSError:
                if time.monotonic() > limit:
                    raise TimeoutError('startup')
                time.sleep(.2)
        servers[-1] = (proc, log, control, PORT)
        manifest['actual_status'] = status(PORT)
        for name in ('blackbox', 'blackboxb'):
            registries, config_raw = {}, []
            def configuration(sock, packet_id, payload):
                config_raw.append(dict(id=packet_id, hex=payload.hex()))
                table = protocol['configuration']['toClient']['types']['packet'][1][0]['type'][1]['mappings']
                if table.get(hex(packet_id), table.get(f'0x{packet_id:02x}')) == 'registry_data':
                    decoder = Decoder(protocol)
                    decoder.types.update(protocol['configuration']['toClient']['types'])
                    row = decode(decoder, 'registry_data', payload)
                    registries[row['id']] = [entry['key'] for entry in row['entries']]
                return False
            client = base.connect(PORT, protocol, PilotClient, version=777, username=name, configuration_handler=configuration)
            clients.append(client)
            client.registries = registries
            write(output / (name + '-configuration.json'), config_raw)
            client.pump(15, lambda row:row['name'] == 'position')
            client.start()
        experiment = Experiment(control, clients, output, next(k for k,v in items.items() if v == 'stone'))
        experiment.prepare()
        for index, material in enumerate(ORDER):
            raw = experiment.trial(index, material)
            path = output / ('trial-' + str(index) + '-' + material + '.json')
            write(path, raw)  # First immutable capture is saved BEFORE any qualification.
            result = audit(raw, protocol, items, block_states)
            manifest['trials'].append(dict(index=index, material=material, raw_sha256=sha(path), result=result))
            write(output / 'progress.json', manifest)
        manifest['status'] = 'six-qualified-observations-no-policy-change'
    except BaseException as error:
        manifest['status'] = 'INCONCLUSIVE_STOP_NO_RETRY'
        manifest['error'] = repr(error)
        (output / 'failure.txt').write_text(traceback.format_exc())
    finally:
        signal.alarm(0)
        # Preserve even partial setup/fall/error; no discarded trial or retry.
        write(output / 'final-streams.json', dict(clients=[c.snapshot() for c in clients],
             commands=experiment.commands if experiment else [],
             partial_trial=experiment.current_trial if experiment else None))
        for c in clients:
            c.close()
        base.cleanup_servers(servers, lifecycle, save, [PORT, PORT+1])
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
        cleanup_ok = (len(lifecycle['cleanup']) == 1 and not lifecycle['cleanup_errors'] and
                      all(r.get('exit_code') == 0 and r.get('group_gone') for r in lifecycle['cleanup']) and
                      all(lifecycle['ports_free'].values()))
        manifest['cleanup_ok'] = cleanup_ok
        manifest['evidence_sha256'] = {str(p.relative_to(output)):sha(p) for p in output.rglob('*') if p.is_file()}
        write(output / 'after.json', manifest)
    assert manifest['status'] == 'six-qualified-observations-no-policy-change' and manifest['cleanup_ok'], manifest.get('error', manifest['status'])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    run(parser.parse_args().output)
