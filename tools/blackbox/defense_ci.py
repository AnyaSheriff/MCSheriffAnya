#!/usr/bin/env python3
"""Свежие P0 проверки щитов и брони777; запись и сравнение общим own-code стендом."""
import argparse
import io
import json
import math
import os
from pathlib import Path
import struct
import sys

import combat_ci as ci
sys.path.insert(0, str(ci.ROOT / 'tools/protocol-bridge'))
from probe_block_entities import exact
from probe_persistence_777 import player_metadata
from probe_use_actions_runtime import use_slot
from smoke import read_varint

CONFIG = ci.ROOT / 'tools/blackbox/ci/defense-777.json'
NAMES = ['pvp-shield-front-777', 'pvp-shield-rear-777', 'pvp-shield-recovery-777',
         'pvp-shield', 'pvp-armor-777', 'pvp-protection-777']


def uint(data):
    value = read_varint(data.read)
    assert 0 <= value < 2**31
    return value


def finished(data):
    assert not data.read(), 'trailing payload'


def shield_damages(event, shield):
    data = io.BytesIO(bytes.fromhex(event['hex']))
    if event['name'] == 'window_items':
        window, revision, count = uint(data), uint(data), uint(data)
        assert window == 0 and count <= 100
        stacks = [use_slot(data) for _ in range(count)]; use_slot(data)
    elif event['name'] == 'set_slot':
        window, revision = uint(data), uint(data); exact(data, 2)
        assert window == 0
        stacks = [use_slot(data)]
    elif event['name'] == 'set_player_inventory':
        uint(data); stacks = [use_slot(data)]
    else:
        return []
    finished(data)
    return [stack.get('damage', 0) for stack in stacks if stack and stack['item'] == shield]


def inspect(raw, shield):
    name = raw['scenario']
    assert name in NAMES and raw['protocol'] == 777 and raw['isolate_vitals'] is True
    assert len(raw['initial']) == len(raw['clients']) == 2
    assert all(x.get('health') == 20 for x in raw['initial']), 'missing observed initial20HP'
    for initial in raw['initial']:
        receipt = initial.get('health_receipt')
        assert isinstance(receipt,dict) and receipt['name']=='update_health' and receipt['at']<=raw['start'], 'missing initial health wire receipt'
        data=io.BytesIO(bytes.fromhex(receipt['hex']))
        hp=struct.unpack('>f',exact(data,4))[0];food=uint(data);sat=struct.unpack('>f',exact(data,4))[0];finished(data)
        assert hp==initial['health']==20 and 0<=food<=20 and math.isfinite(sat), 'initial HP not derived from captured health packet'
    assert all(c['events'] and not c.get('failure') for c in raw['clients'])
    if name.startswith('pvp-shield'):
        actions = [row['action'] for row in raw['sent']]
        assert any(a.get('use') and a.get('hand') == 1 for a in actions), 'no authored offhand use'
        assert any(a.get('attack') and a.get('bot') == 2 for a in actions), 'no authored player attack'
    victim, attacker = [x['entity'] for x in raw['initial']]
    hp = [20.0]; damage = []; cooldowns = []; cooldown_ticks = []; wear = []; active = []
    for i, client in enumerate(raw['clients']):
        for e in client['events']:
            data = io.BytesIO(bytes.fromhex(e['hex']))
            if i == 0 and e['name'] == 'update_health':
                value = struct.unpack('>f', exact(data, 4))[0]; food = uint(data)
                saturation = struct.unpack('>f', exact(data, 4))[0]; finished(data)
                assert math.isfinite(value) and 0 < value <= 20 and 0 <= food <= 20 and math.isfinite(saturation)
                hp.append(value)
            elif i == 0 and e['name'] == 'damage_event':
                entity, source, cause, direct = [uint(data) for _ in range(4)]
                present = exact(data, 1); assert present in (b'\0', b'\1')
                if present == b'\1': exact(data, 24)
                finished(data)
                assert entity == victim and cause == direct == attacker + 1, 'not an actual attack from the second player'
                damage.append((e['at'] - raw['start']) * 20)
            elif i == 0 and e['name'] == 'set_cooldown':
                group = exact(data, uint(data)).decode(); ticks = uint(data); finished(data)
                cooldowns.append([group, ticks]); cooldown_ticks.append((e['at'] - raw['start']) * 20)
            elif e['name'] == 'entity_metadata' and uint(data) == victim:
                _, fields = player_metadata(bytes.fromhex(e['hex']))
                if 8 in fields:
                    assert type(fields[8]) is int
                    if fields[8] & 1: active.append((e['at'] - raw['start']) * 20)
            if i == 0 and name.startswith('pvp-shield'):
                wear.extend(shield_damages(e, shield))
    result = {'health': hp, 'damage_ticks': damage, 'cooldowns': cooldowns,
              'shield_damage': max(wear, default=0), 'active_ticks': active, 'cooldown_ticks': cooldown_ticks}
    if name.startswith('pvp-shield'):
        assert active, 'no received active-hand metadata'
    if name == NAMES[0]:
        assert min(hp) == 20 and not damage and result['shield_damage'] == 8, 'front sword was not blocked/worn'
        assert not cooldowns, 'sword disabled shield'
    elif name == NAMES[1]:
        assert min(hp) < 20 and len(damage) == 1 and result['shield_damage'] == 0, 'rear sword must hurt without shield wear'
        assert not cooldowns
    elif name == NAMES[2]:
        assert min(hp) == hp[-1] == 13 and len(damage) == 1 and 35 <= damage[0] < 100, 'only the during-cooldown sword may hurt'
        assert cooldowns == [['minecraft:shield', 100], ['minecraft:shield', 0]] and result['shield_damage'] == 18, 'axe cooldown/wear differs'
        # Пакет принимается между границами; сравниваем целые такты20t/s.
        assert 98 <= round(cooldown_ticks[1] - cooldown_ticks[0]) <= 100, 'cooldown ending outside observed99±1ticks'
        assert any(t >= 125 for t in active), 'shield never reactivated after cooldown'
    else:
        assert damage and min(hp) < 20, 'no genuine damaging positive control'
        if name == 'pvp-shield': assert cooldowns == [['minecraft:shield', 100]]
    return result


def semantics(results, data):
    items = json.loads((data / 'items.json').read_text()); shield = next(x['id'] for x in items if x['name'] == 'shield')
    rows = []
    for name in NAMES:
        for side in ('official', 'ours'):
            raw = json.loads(ci.checked_file(results / f'{name}-{side}.json').read_text())
            rows.append({'scenario': name, 'side': side, **inspect(raw, shield)})
    return {'schema': 'defense-777-v2', 'cases': rows, 'status': 'verified-semantic-controls'}


def apply_semantic_result(result, check):
    if check['status'] == 'failed':
        result['semantic_error'] = check['error']
        if result['status'] == 'match':
            result['status'] = 'mismatch-open'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('pin', 'run'))
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--artifact', type=Path); parser.add_argument('--work', type=Path)
    parser.add_argument('--java', default='java'); a = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true': parser.error('CI-only')
    a.output.mkdir(parents=True, exist_ok=True)
    config = json.loads(CONFIG.read_text()); assert config['scenarios'] == NAMES
    try:
        if a.mode == 'pin':
            pin = ci.resolve_pin(json.loads(ci.PIN.read_text()), os.environ.get('INPUT_BUILD_RUN_ID', ''),
                                 os.environ.get('INPUT_BUILD_HEAD', ''), os.environ.get('INPUT_RUNTIME_SHA256', ''))
            selected = ci.preflight(pin, a.output)
            with open(os.environ['GITHUB_OUTPUT'], 'a') as f:
                f.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={selected["artifact_name"]}\n')
            return
        assert a.work and a.artifact
        extras = [CONFIG, Path(__file__), ci.ROOT/'tools/blackbox/test_defense_ci.py',
                  ci.ROOT/'.github/workflows/defense-checks.yml', ci.ROOT/'docs/defense-26.3.md',
                  ci.ROOT/'tools/protocol-bridge/testdata/defense-777-observations.json']
        result = ci.execute(a.artifact, json.loads((a.output/'build-selection.json').read_text()),
                            a.work.resolve(), a.output.resolve(), a.java, sources_path=CONFIG, extra_sources=extras)
        try:
            check = semantics(a.work/'recordings', a.work/'data')
        except Exception as e:
            check = {'status': 'failed', 'error': type(e).__name__+': '+str(e)}
        apply_semantic_result(result, check)
        ci.write_json(a.output/'defense-semantics.json', check)
        ci.write_json(a.output/'ci-receipt.json', result)
        ci.collect(a.output, a.work/'recordings', a.work/'data')
        if result['status'] != 'match': raise SystemExit(1)
    except Exception as e:
        ci.write_json(a.output/'entrypoint-error.json', {'status':'infrastructure-failed','error':repr(e)})
        raise SystemExit(1)


if __name__ == '__main__': main()
