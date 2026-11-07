#!/usr/bin/env python3
"""Общий own-code разбор наблюдений777: HP, владелец урона, чары и броня."""
import io
import math
import struct
from defense_ci import uint, finished, exact, use_slot, player_metadata
from probe_player_actions_runtime import menu_slot


def inventory_changes(event):
    """Только player inventory; неизвестный компонент/хвост запрещает PASS."""
    data = io.BytesIO(bytes.fromhex(event['hex']))
    if event['name'] == 'window_items':
        window, revision, count = uint(data), uint(data), uint(data)
        assert window == 0 and count <= 100
        slots = {i: use_slot(data) for i in range(count)}
        use_slot(data)
    elif event['name'] == 'set_slot':
        window, revision = uint(data), uint(data)
        index = struct.unpack('>h', exact(data, 2))[0]
        assert window == 0
        slots = {index: use_slot(data)}
    elif event['name'] == 'set_player_inventory':
        slots = {menu_slot(uint(data)): use_slot(data)}
    else:
        return {}
    finished(data)
    return slots


def health_packet(event):
    data = io.BytesIO(bytes.fromhex(event['hex']))
    hp = struct.unpack('>f', exact(data, 4))[0]
    food = uint(data)
    saturation = struct.unpack('>f', exact(data, 4))[0]
    finished(data)
    assert math.isfinite(hp) and 0 < hp <= 20 and 0 <= food <= 20 and math.isfinite(saturation)
    return hp


def observe(raw, names, inventory=False):
    name = raw['scenario']
    assert name in names and raw['protocol'] == 777 and raw['isolate_vitals'] is True
    assert len(raw['initial']) == len(raw['clients']) == 2
    for initial in raw['initial']:
        receipt = initial.get('health_receipt')
        assert receipt and receipt['name'] == 'update_health' and receipt['at'] <= raw['start']
        assert health_packet(receipt) == initial['health'] == 20, 'initial HP not observed'
    assert all(c['events'] and not c.get('failure') for c in raw['clients'])
    attacks = [a for a in raw['sent'] if a['action'].get('attack') and a['action'].get('bot') == 2]
    assert attacks, 'no actual authored attack'
    victim, attacker = [r['entity'] for r in raw['initial']]
    health = [[20.0], [20.0]]
    damage, effects, removed, burning, chest, breaks = [], [], [], [], [], []
    observer_breaks = [[], []]
    observer_break_indices = [[], []]
    for side, client in enumerate(raw['clients']):
        for event_index, event in enumerate(client['events']):
            data = io.BytesIO(bytes.fromhex(event['hex']))
            tick = (event['at'] - raw['start']) * 20
            if event['name'] == 'update_health':
                hp = health_packet(event)
                if hp != health[side][-1]:
                    health[side].append(hp)
            elif event['name'] == 'damage_event':
                entity, source, cause, direct = [uint(data) for _ in range(4)]
                present = exact(data, 1)
                assert present in (b'\0', b'\1')
                if present == b'\1': exact(data, 24)
                finished(data)
                if entity == (victim if side == 0 else attacker):
                    damage.append({'side': side, 'source': source, 'cause': cause,
                                   'direct': direct, 'tick': tick, 'index': event_index})
            elif event['name'] == 'entity_effect':
                entity, effect, amplifier, duration = [uint(data) for _ in range(4)]
                flags = exact(data, 1)[0]; finished(data)
                if entity == (victim if side == 0 else attacker):
                    effects.append({'side': side, 'id': effect, 'amplifier': amplifier,
                                    'duration': duration, 'flags': flags, 'tick': tick})
            elif event['name'] == 'remove_entity_effect':
                entity, effect = uint(data), uint(data); finished(data)
                if entity == (victim if side == 0 else attacker):
                    removed.append({'side': side, 'id': effect, 'tick': tick})
            elif event['name'] == 'entity_metadata' and uint(data) == victim:
                _, fields = player_metadata(bytes.fromhex(event['hex']))
                if 0 in fields: burning.append({'side': side, 'burning': bool(fields[0] & 1), 'tick': tick})
            elif event['name'] == 'entity_status':
                entity = struct.unpack('>i', exact(data, 4))[0]
                status = exact(data, 1)[0]; finished(data)
                if entity == victim and status == 50:
                    observer_breaks[side].append(tick)
                    observer_break_indices[side].append(event_index)
                    if side == 0: breaks.append(tick)
            if side == 0 and inventory:
                slots = inventory_changes(event)
                if 6 in slots: chest.append({'tick': tick, 'stack': slots[6]})
    return dict(name=name, victim=victim, attacker=attacker, attacks=attacks,
                health=health, damage=damage, effects=effects, removed=removed,
                burning=burning, chest=chest, breaks=breaks, observer_breaks=observer_breaks,
                observer_break_indices=observer_break_indices)
