#!/usr/bin/env python3
"""Настоящие PvP-попадания, износ/поломка брони и боевые чары777."""
import argparse
import io
import json
import math
import os
from pathlib import Path
import struct

import combat_ci as ci
from defense_ci import uint, finished, exact, use_slot, player_metadata, apply_semantic_result
from probe_player_actions_runtime import menu_slot

CONFIG = ci.ROOT / 'tools/blackbox/ci/armor-enchants-777.json'
NAMES = ['pvp-sharpness-777', 'pvp-fire-aspect-777', 'pvp-strength-777',
         'pvp-weakness-777', 'pvp-resistance-777', 'pvp-thorns-777',
         'pvp-armor-worn-777', 'pvp-armor-break-777']


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


def inspect(raw, items):
    name = raw['scenario']
    assert name in NAMES and raw['protocol'] == 777 and raw['isolate_vitals'] is True
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
            if side == 0 and name in NAMES[-3:]:
                slots = inventory_changes(event)
                if 6 in slots: chest.append({'tick': tick, 'stack': slots[6]})
    hits = [d for d in damage if d['side'] == 0 and d['cause'] == d['direct'] == attacker + 1]
    assert hits and min(health[0]) < 20, 'missing actual damaging melee with correct owner'
    if name in NAMES[:5]:
        assert health[1] == [20.0], 'unexpected damage to attacker'
    if name == NAMES[0]:
        assert health[0] == [20.0, 10.0] and len(hits) == 1, 'SharpnessV not observed'
    elif name == NAMES[1]:
        assert health[0][1] == 13 and len(health[0]) >= 3 and health[0][-1] < 13
        assert len(hits) == 1 and any(d['side'] == 0 and d['cause'] == d['direct'] == 0 for d in damage), 'missing fire damage'
        for side in (0, 1):
            flags = [b['burning'] for b in burning if b['side'] == side]
            assert True in flags and flags[-1] is False, 'ignition/extinguish not received by both players'
    elif name in NAMES[2:5]:
        side = 0 if name == NAMES[4] else 1
        amplifier = 0 if name == NAMES[3] else 1
        effect = [e for e in effects if e['side'] == side and e['amplifier'] == amplifier]
        assert len(effect) == 1 and effect[0]['tick'] < hits[0]['tick'], 'effect not observed before real hit'
        if name == NAMES[2]: assert health[0] == [20.0, 7.0] and len(hits) == 1
        if name == NAMES[3]:
            assert health[0] == [20.0, 17.0] and len(hits) == 1
            assert any(e['side'] == side and e['id'] == effect[0]['id'] and e['tick'] > hits[0]['tick'] for e in removed), 'weakness never expired'
        if name == NAMES[4]:
            assert len(hits) == 2 and len(health[0]) == 3 and 11 < health[0][-1] < 12, 'ResistanceII two-hit reduction missing'
    elif name == NAMES[5]:
        reflected = [d for d in damage if d['side'] == 1 and d['cause'] == d['direct'] == victim + 1]
        assert len(reflected) == len(hits) == 1 and len(health[1]) == 2 and 15 <= health[1][-1] < 19
        assert any(c['stack'] and c['stack']['item'] == items['diamond_chestplate'] and c['stack'].get('damage') == 3 for c in chest), 'Thorns wear3 not observed'
    else:
        assert len(attacks) == len(hits) == 2 and len(health[0]) == 3
        expected = 200 if name == NAMES[6] else 239
        stacks = [c for c in chest if c['stack'] and c['stack']['item'] == items['iron_chestplate']]
        assert stacks and stacks[0]['stack'].get('damage') == expected and stacks[0]['tick'] < hits[0]['tick'], 'initial worn chestplate not received before hit'
        if name == NAMES[6]:
            assert [c['stack'].get('damage') for c in stacks] == [200, 201, 202]
            assert not breaks and not any(c['stack'] is None and c['tick'] > stacks[0]['tick'] for c in chest)
            assert abs((20-health[0][1]) - (health[0][1]-health[0][2])) < 0.00001
        else:
            assert len(breaks) == 1 and any(c['stack'] is None and hits[0]['tick'] <= c['tick'] < hits[1]['tick'] for c in chest), 'chestplate did not break before second hit'
            assert all(len(values) == 1 for values in observer_breaks), 'each player must receive exactly one break event'
            assert breaks[0] <= hits[0]['tick'] and observer_break_indices[0][0] < hits[0]['index'], 'owner break event arrived after damage'
            assert health[0] == [20.0, 13.0, 6.0], 'breaking hit must already lose chestplate protection'
    return {'health': health, 'damage': damage, 'effects': effects, 'removed_effects': removed,
            'burning': burning, 'chest': chest, 'break_ticks': breaks, 'observer_break_ticks': observer_breaks}


def semantics(results, data):
    items = {x['name']: x['id'] for x in json.loads((data/'items.json').read_text())}
    rows = []
    for name in NAMES:
        for side in ('official', 'ours'):
            try:
                raw = json.loads(ci.checked_file(results/f'{name}-{side}.json').read_text())
                rows.append({'scenario': name, 'side': side, 'status': 'pass', 'observed': inspect(raw, items)})
            except (AssertionError, EOFError, ValueError, KeyError, OSError) as error:
                rows.append({'scenario': name, 'side': side, 'status': 'failed', 'error': repr(error)})
    for name in NAMES:
        pair = [r for r in rows if r['scenario'] == name]
        if all(r['status'] == 'pass' for r in pair):
            first, second = [r['observed']['health'] for r in pair]
            # Шипы случайны только у ударившего. HP основной жертвы также exact.
            exact_health = first[0] == second[0] and (name == NAMES[5] or first[1] == second[1])
            if not exact_health:
                pair[1].update(status='failed', error='actual f32 health differs from original')
    return {'schema': 'armor-enchants-777-v2', 'cases': rows,
            'status': 'failed' if any(r['status'] != 'pass' for r in rows) else 'verified-semantic-controls',
            'error': '; '.join(r['scenario']+'/'+r['side']+':'+r['error'] for r in rows if r['status'] != 'pass')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('pin', 'run'))
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--artifact', type=Path); parser.add_argument('--work', type=Path)
    parser.add_argument('--java', default='java'); args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true': parser.error('CI-only')
    args.output.mkdir(parents=True, exist_ok=True)
    assert json.loads(CONFIG.read_text())['scenarios'] == NAMES
    try:
        if args.mode == 'pin':
            pin = ci.resolve_pin(json.loads(ci.PIN.read_text()), os.environ.get('INPUT_BUILD_RUN_ID', ''),
                                 os.environ.get('INPUT_BUILD_HEAD', ''), os.environ.get('INPUT_RUNTIME_SHA256', ''))
            selected = ci.preflight(pin, args.output)
            with open(os.environ['GITHUB_OUTPUT'], 'a') as out:
                out.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={selected["artifact_name"]}\n')
            return
        assert args.work and args.artifact
        extras = [CONFIG, Path(__file__), ci.ROOT/'tools/blackbox/test_armor_enchants_ci.py',
                  ci.ROOT/'.github/workflows/armor-enchants-checks.yml', ci.ROOT/'docs/armor-enchants-26.3.md']
        result = ci.execute(args.artifact, json.loads((args.output/'build-selection.json').read_text()),
                            args.work.resolve(), args.output.resolve(), args.java, sources_path=CONFIG, extra_sources=extras)
        check = semantics(args.work/'recordings', args.work/'data')
        apply_semantic_result(result, check)
        ci.write_json(args.output/'armor-enchants-semantics.json', check)
        ci.write_json(args.output/'ci-receipt.json', result)
        ci.collect(args.output, args.work/'recordings', args.work/'data')
        if result['status'] != 'match': raise SystemExit(1)
    except Exception as error:
        ci.write_json(args.output/'entrypoint-error.json', {'status':'infrastructure-failed','error':repr(error)})
        raise SystemExit(1)


if __name__ == '__main__': main()
