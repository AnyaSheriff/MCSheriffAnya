#!/usr/bin/env python3
"""Вторая рука и настоящая зарядка арбалета: внешний discovery777."""
import argparse
import hashlib
import io
import json
import math
import struct
import sys
from pathlib import Path

NAME = 'crossbow-hands-loading-discovery-777'
ROTATION = [math.pi, math.pi / 9]
PATTERNS = [['arrow'] * 3, ['firework_rocket'] * 3,
            ['arrow', 'firework_rocket', 'spectral_arrow'],
            ['firework_rocket', 'arrow', 'spectral_arrow']]
CASES = []
for pattern in PATTERNS:
    for initial in [0, 461, 463, 464]:
        CASES.append(dict(hand=1, projectiles=pattern, initial_damage=initial, loading=False, duration=44))
for hand in [0, 1]:
    for initial in [0, 464]:
        CASES.append(dict(hand=hand, projectiles=['arrow'] * 3, initial_damage=initial, loading=True, duration=86))
tick = 0
for number, case in enumerate(CASES):
    case.update(trial=number, start=tick, fire=tick + (60 if case['loading'] else 16))
    tick += case['duration']


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def swap(at):
    return dict(after=at, window_click=dict(inventory=True, slot=36, mode=2, button=40))


def prepare(output, repo):
    output.mkdir(parents=True, exist_ok=False)
    s = json.loads((repo / 'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text())
    actions = []
    for case in CASES:
        t, hand = case['start'], case['hand']
        for kind in ['arrow', 'spectral_arrow', 'firework_rocket']:
            actions.append(dict(after=t, cmd='kill @e[type=' + kind + ',distance=..48]'))
        actions.extend([dict(after=t, cmd='clear $(bot)'),
                        dict(after=t + 1, cmd='tp $(bot) $(3.5,0,3.5) 0 -20')])
        components = ['damage=' + str(case['initial_damage'])]
        if case['loading']:
            components.append('enchantments={multishot:1}')
            actions.append(dict(after=t + 2, cmd='item replace entity $(bot) inventory.0 with minecraft:arrow 3'))
        else:
            contents = ','.join('{id:"minecraft:' + p + '",count:1}' for p in case['projectiles'])
            components.append('charged_projectiles=[' + contents + ']')
        specification = 'minecraft:crossbow[' + ','.join(components) + ']'
        actions.append(dict(after=t + 2, cmd='item replace entity $(bot) weapon.' + ('offhand' if hand else 'mainhand') + ' with ' + specification))
        if hand:
            actions.append(swap(t + 4))
        actions.extend([dict(after=t + 6, click=[5, 0, 3]), dict(after=t + 10, close=True)])
        if hand:
            actions.append(swap(t + 12))
        if case['loading']:
            actions.extend([dict(after=t + 16, look=ROTATION, use=True, hand=hand),
                            dict(after=t + 46, release=True)])
            if hand:
                actions.append(swap(t + 48))
            actions.extend([dict(after=t + 50, click=[5, 0, 3]), dict(after=t + 54, close=True)])
            if hand:
                actions.append(swap(t + 56))
        actions.append(dict(after=case['fire'], look=ROTATION, use=True, hand=hand))
        if hand:
            actions.append(swap(case['fire'] + 10))
        actions.extend([dict(after=case['fire'] + 14, click=[5, 0, 3]),
                        dict(after=case['fire'] + 20, close=True)])
    s.update(name=NAME, about='Discovery777: offhand16 и4 обычных зарядки; сначала observedoriginal, затем freeze исходов.',
             actions=actions, record_ticks=tick, random=[], track_projectiles=True,
             blocks=[dict(at=[5, 0, 3], state='minecraft:chest[facing=west,type=single,waterlogged=false]')])
    s['setup'] = [v for v in s['setup'] if not v.startswith('tp ')] + [
        'tp $(bot) $(3.5,0,3.5) 0 -20', 'tp $(bot2) $(0.5,0,3.5) 90 0']
    s['cleanup'][:0] = ['kill @e[type=firework_rocket,distance=..48]', 'kill @e[type=spectral_arrow,distance=..48]']
    write(output / 'scenario.json', s)
    write(output / 'criteria-before-recording.json', dict(name=NAME, protocol=777, cases=CASES,
        scenario_sha256=hashlib.sha256((output / 'scenario.json').read_bytes()).hexdigest(),
        positive='Каждый выстрел usehand0/1 yaw0,pitch-20; до/после полные chest63snapshot, пустые остальные слоты кроме ровно3→2 обычных стрел при реальной зарядке. Offhand перемещается настоящим inventoryswapbutton40; ОБЯЗАТЕЛЬНЫ полученные доuse echo45/crossbow и36/null послеswapback. У2наблюдателей реальные spawn ID/type иbreakstatus. После: явный finalnull/item в серверномполномокне. Никакого вывода из отсутствующегоSetSlot.',
        loading='4case: main/offhand × damage0/464; MultishotI и0Unbreaking, actual30tickheld/release, exact3заряда в промежуточном полном серверномокне, реальная трата3→2ammo, износ не меняется отзарядки.',
        discovery='Nearbreak число выпущенных зарядов НЕ задаётся доoriginalraw. Mainhandv3raw/expected immutable. Никакихproductionpatch/Cargo.'))


def inspect(raw, names, entities, repo):
    sys.path.insert(0, str(repo / 'tools/protocol-bridge'))
    from probe_crossbow_session import Probe
    from probe_player_actions_runtime import menu_slot
    from smoke import read_varint
    assert raw['scenario'] == NAME and raw['protocol'] == 777 and len(raw['clients']) == 2
    assert all(c['events'] and not c.get('failure') for c in raw['clients'])
    parser = object.__new__(Probe)
    parser.names, parser.version = names, 777
    snapshots, changes, state = [], [], {}
    spawns, statuses = [[], []], [[], []]
    shooter = raw['initial'][0]['entity']
    for observer, client in enumerate(raw['clients']):
        for event in client['events']:
            data = io.BytesIO(bytes.fromhex(event['hex']))
            name = event['name']
            updated = {}
            if observer == 0 and name == 'window_items':
                window, revision, count = [read_varint(data.read) for _ in range(3)]
                assert count <= 100
                slots = [parser.slot(data) for _ in range(count)]
                cursor = parser.slot(data)
                assert not data.read() and cursor is None
                if window > 0:
                    assert count == 63 and slots[:27] == [None] * 27
                    snapshots.append((event['at'], slots))
                else:
                    assert count == 46
                    updated = dict(enumerate(slots))
            elif observer == 0 and name == 'set_slot':
                window, revision = [read_varint(data.read) for _ in range(2)]
                slot = struct.unpack('>h', data.read(2))[0]
                stack = parser.slot(data)
                assert not data.read()
                if window == 0:
                    updated = {slot: stack}
            elif observer == 0 and name == 'set_player_inventory':
                slot = menu_slot(read_varint(data.read))
                updated = {slot: parser.slot(data)}
                assert not data.read()
            elif name == 'spawn_entity':
                entity = read_varint(data.read)
                assert len(data.read(16)) == 16
                kind = entities[read_varint(data.read)]
                if kind in ('arrow', 'spectral_arrow', 'firework_rocket'):
                    spawns[observer].append((event['at'], entity, kind))
            elif name == 'entity_status':
                payload = bytes.fromhex(event['hex'])
                assert len(payload) == 5
                entity, status = struct.unpack('>iB', payload)
                if entity == shooter:
                    statuses[observer].append((event['at'], status))
            if updated:
                state.update(updated)
                changes.append((event['at'], updated, state.copy()))
    uses = [r for r in raw['sent'] if r['action'].get('use')]
    assert len(uses) == len(CASES) + 4
    rows = []
    enchants = raw['clients'][0]['registry_names']['minecraft:enchantment']
    for case in CASES:
        i, hand = case['trial'], case['hand']
        start = raw['start'] + case['start'] * .05
        end = start + case['duration'] * .05
        fire = next(r for r in uses if r['action']['after'] == case['fire'])
        assert fire['action'] == dict(after=case['fire'], look=ROTATION, use=True, hand=hand)
        before = [(at, s) for at, s in snapshots if start <= at < fire['at']]
        after = [(at, s) for at, s in snapshots if fire['at'] <= at < end]
        assert len(before) == (2 if case['loading'] else 1) and len(after) == 1, (i, 'missing inventory')
        initial, loaded, final = before[0][1][54], before[-1][1][54], after[0][1][54]
        assert initial and initial['item'] == 'crossbow' and initial['count'] == 1
        assert initial.get('damage', 0) == case['initial_damage']
        assert {enchants[n]: v for n, v in initial.get('enchantments', [])} == ({'minecraft:multishot': 1} if case['loading'] else {})
        if case['loading']:
            assert not initial.get('projectiles') and before[0][1][27] == dict(item='arrow', count=3)
            assert before[1][1][27] == dict(item='arrow', count=2)
            assert loaded and loaded.get('damage', 0) == case['initial_damage']
            assert loaded.get('enchantments') == initial.get('enchantments')
            assert len(loaded['projectiles']) == 3
            assert all(p['item'] == 'arrow' and p['count'] == 1 for p in loaded['projectiles'])
            load = next(r for r in uses if r['action']['after'] == case['start'] + 16)
            assert load['action']['hand'] == hand
            assert any(r['action'] == dict(after=case['start'] + 46, release=True) for r in raw['sent'])
            assert all(not (load['at'] <= at < fire['at']) for at, _, _ in spawns[0]), (i, 'loading fired a projectile')
        else:
            assert initial['projectiles'] == [dict(item=p, count=1) for p in case['projectiles']]
        if hand:
            swapback = start + (56 if case['loading'] else 12) * .05
            received = [(at, c, s) for at, c, s in changes if swapback <= at < fire['at'] and 45 in c]
            assert received and received[-1][2].get(45) == loaded and received[-1][2].get(36) is None, (i, 'offhand not authoritatively received before use')
        emitted = [[(entity, kind) for at, entity, kind in side if fire['at'] <= at < end] for side in spawns]
        assert sorted(emitted[0]) == sorted(emitted[1]) and 1 <= len(emitted[0]) <= 3, (i, emitted)
        # Status55 — отдельная observer-only анимация нашего измерительного
        # swapbutton40, не поломка арбалета; не требуем её у самого владельца.
        observed_status = [[status for at, status in side if fire['at'] <= at < end and 47 <= status <= 52] for side in statuses]
        assert observed_status[0] == observed_status[1]
        if final:
            assert final['item'] == 'crossbow' and final['count'] == 1 and not final.get('projectiles')
            assert final.get('damage', 0) >= case['initial_damage']
            assert final.get('enchantments', []) == initial.get('enchantments', [])
            assert observed_status == [[], []], (i, 'surviving crossbow received break')
        else:
            assert observed_status == [[48 if hand else 47]] * 2, (i, 'missing actual break')
        for _, snapshot in before + after:
            assert all(stack is None for k, stack in enumerate(snapshot) if k != 54 and not (case['loading'] and k == 27))
        if case['loading']:
            assert after[0][1][27] == dict(item='arrow', count=2)
        rows.append(dict(**case, initial=initial, loaded=loaded, after=final,
                         emitted=[kind for _, kind in sorted(emitted[0])], statuses_by_observer=observed_status,
                         offhand_before_use_echo=bool(hand)))
    return rows


def analyze(output, recordings, data, repo):
    names = {v['id']: v['name'] for v in json.loads((data / 'items.json').read_text())}
    entities = {v['id']: v['name'] for v in json.loads((data / 'entities.json').read_text())}
    raw_sides = {side: json.loads((recordings / (NAME + '-' + side + '.json')).read_text()) for side in ['official', 'ours']}
    sides = {side: inspect(raw, names, entities, repo) for side, raw in raw_sides.items()}
    auxiliary = {}
    for side, raw in raw_sides.items():
        auxiliary[side] = [[dict(at=e['at'], status=struct.unpack('>iB', bytes.fromhex(e['hex']))[1])
                            for e in client['events'] if e['name'] == 'entity_status'
                            and struct.unpack('>iB', bytes.fromhex(e['hex']))[0] == raw['initial'][0]['entity']
                            and not 47 <= struct.unpack('>iB', bytes.fromhex(e['hex']))[1] <= 52]
                           for client in raw['clients']]
    write(output, dict(status='discovery-only-no-full-parity-claim', sides=sides,
                       differences=[i for i, (a, b) in enumerate(zip(sides['official'], sides['ours'])) if a != b],
                       auxiliary_nonbreak_statuses=auxiliary))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['prepare', 'analyze'])
    p.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[2])
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--recordings', type=Path)
    p.add_argument('--data', type=Path)
    a = p.parse_args()
    if a.mode == 'prepare':
        prepare(a.output, a.repo)
    else:
        analyze(a.output, a.recordings, a.data, a.repo)
