#!/usr/bin/env python3
"""Шесть опытов с FireProtection: подлинные HP/экипировка/огонь обоим клиентам777."""
import io
import json
from combat_wire import health_packet, inventory_changes
from defense_ci import exact, finished, player_metadata, uint, use_slot
from thorns_natural_sample import damage
from fire_stacking_qualifier import qualify

GEAR = [('diamond_helmet', 5, 5), ('diamond_chestplate', 6, 4),
        ('diamond_leggings', 7, 3), ('diamond_boots', 8, 2)]


def equipment(event):
    if event['name'] != 'entity_equipment':
        return None
    source = io.BytesIO(bytes.fromhex(event['hex']))
    entity = uint(source)
    slots = {}
    while True:
        flag = exact(source, 1)[0]
        slot = flag & 127
        assert slot <= 5 and slot not in slots, 'invalid/duplicate equipment slot'
        slots[slot] = use_slot(source)
        if not flag & 128:
            break
    finished(source)
    return entity, slots


def stack_matches(stack, item, enchantments, names):
    assert stack and stack['item'] == item and stack['count'] == 1
    assert stack.get('damage', 0) == 0
    pairs = stack.get('enchantments', [])
    assert all(type(n) is int and 0 <= n < len(names) for n, _ in pairs)
    actual = {names[n]: level for n, level in pairs}
    assert len(actual) == len(pairs) and actual == enchantments


def inspect(raw, case, items):
    assert raw['scenario'] == case['name'] and raw['protocol'] == 777
    assert raw['isolate_vitals'] is True and len(raw['clients']) == len(raw['initial']) == 2
    victim, attacker = [v['entity'] for v in raw['initial']]
    start = raw['start']
    attacks = [r for r in raw['sent'] if r['action'].get('attack')]
    assert len(attacks) == 1 and attacks[0]['action'] == {'after': 20, 'attack': True, 'bot': 2}
    at = attacks[0]['at']
    assert start + .95 <= at <= start + 1.25, 'late/missing authored attack'
    # Recorder отправляет физику до record_ticks+13 включительно. Запас здесь
    # только для времени получения, не для изменения допустимой длительности огня.
    end = start + 7.35
    evidence = {'start_at': start, 'attack_at': at, 'end_at': end, 'victim': victim, 'attacker': attacker,
                'initial_hp': [], 'gear_verified': [], 'health': [[], []],
                'damage': [[], []], 'burning': [[], []]}
    inventories, seen = [], []
    for observer, client in enumerate(raw['clients']):
        assert client['events'] and not client.get('failure')
        initial = raw['initial'][observer]
        receipt = initial['health_receipt']
        assert receipt['name'] == 'update_health' and receipt['at'] <= start
        hp = health_packet(receipt)
        assert initial['health'] == hp == 20
        evidence['initial_hp'].append(hp)
        evidence['health'][observer].append({'at': receipt['at'], 'hp': hp})
        inventory, other_gear, proofs = {}, {}, []
        for index, event in enumerate(client['events']):
            when = event['at']
            assert when < end, 'event outside frozen recording window'
            if event['name'] == 'update_health':
                evidence['health'][observer].append({'at': when, 'hp': health_packet(event)})
            if event['name'] == 'damage_event':
                evidence['damage'][observer].append(damage(event, client['registry_names']['minecraft:damage_type']))
            if event['name'] == 'entity_effect':
                raise AssertionError('unexpected active effect during fire-only trial')
            if event['name'] == 'entity_metadata':
                entity, fields = player_metadata(bytes.fromhex(event['hex']))
                if entity == victim and 0 in fields:
                    evidence['burning'][observer].append({'at': when, 'burning': bool(fields[0] & 1)})
            if event['name'] == 'entity_status':
                source = io.BytesIO(bytes.fromhex(event['hex']))
                entity = int.from_bytes(exact(source, 4), 'big', signed=True)
                status = exact(source, 1)[0]
                finished(source)
                assert entity not in (victim, attacker) or status != 3, 'player died'
            # Только действительно полученные ДО отправки удара события текущей
            # записи. Не выдаём снимок памяти/неизвестные пустые слоты за full46.
            if start <= when < at:
                updates = inventory_changes(event)
                equip = equipment(event)
                if updates:
                    inventory.update(updates)
                    proofs.append({'index': index, 'at': when, 'kind': event['name']})
                if equip and equip[0] == (attacker if observer == 0 else victim):
                    other_gear.update(equip[1])
                    proofs.append({'index': index, 'at': when, 'kind': event['name']})
        names = client['registry_names']['minecraft:enchantment']
        armor = inventory if observer == 0 else other_gear
        for (item, owner_slot, wire_slot), level in zip(GEAR, case['levels']):
            stack_matches(armor.get(owner_slot if observer == 0 else wire_slot), items[item],
                          {'minecraft:fire_protection': level} if level else {}, names)
        sword = other_gear.get(0) if observer == 0 else inventory.get(36)
        stack_matches(sword, items['diamond_sword'], {'minecraft:fire_aspect': 1}, names)
        evidence['gear_verified'].append(True)
        inventories.append({'armor': armor, 'sword': sword})
        seen.append(proofs)
    summary = qualify(evidence)
    return {'summary': summary, 'evidence': evidence, 'initial_gear': inventories,
            'actual_pre_attack_receipts': seen}


def analyze(recordings, data, config):
    items = {r['name']: r['id'] for r in json.loads((data / 'items.json').read_text())}
    rows, errors = [], []
    for case in config['cases']:
        for side in ('official', 'ours'):
            try:
                raw = json.loads((recordings / (case['name'] + '-' + side + '.json')).read_text())
                observed = inspect(raw, case, items)
                rows.append({'scenario': case['name'], 'side': side, 'status': 'qualified', 'observed': observed})
            except (AssertionError, ValueError, KeyError, OSError, TypeError, EOFError, StopIteration) as error:
                message = case['name'] + '/' + side + ': ' + repr(error)
                errors.append(message)
                rows.append({'scenario': case['name'], 'side': side, 'status': 'failed', 'error': message})
    return {'schema': 'fire-stacking-discovery-777-v1', 'status': 'qualification-failed' if errors else 'qualified-discovery',
            'cases': rows, 'errors': errors,
            'scope': 'Original outcomes were not predicted. Comparator differences remain open. No blanket parity claim.'}
