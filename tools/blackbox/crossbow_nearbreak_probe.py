#!/usr/bin/env python3
"""Граница поломки арбалета: только внешний discovery, без правки ядра."""
import argparse
import hashlib
import io
import json
import math
import struct
import sys
from pathlib import Path

NAME = 'crossbow-nearbreak-discovery-777'
INTERVAL = 36
PATTERNS = [
    ['arrow'] * 3,
    ['firework_rocket'] * 3,
    ['arrow', 'firework_rocket', 'spectral_arrow'],
    ['firework_rocket', 'arrow', 'spectral_arrow'],
]
CASES = [dict(projectiles=p, initial_damage=d) for p in PATTERNS for d in [0, 461, 463, 464]]


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def prepare(output, repo):
    output.mkdir(parents=True, exist_ok=False)
    scenario = json.loads((repo / 'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text())
    actions = []
    for i, case in enumerate(CASES):
        t = i * INTERVAL
        content = ','.join('{id:"minecraft:' + name + '",count:1}' for name in case['projectiles'])
        stack = 'minecraft:crossbow[damage=' + str(case['initial_damage']) + ',charged_projectiles=[' + content + ']]'
        actions.extend([
            {'after': t, 'cmd': 'kill @e[type=arrow,distance=..48]'},
            {'after': t, 'cmd': 'kill @e[type=spectral_arrow,distance=..48]'},
            {'after': t, 'cmd': 'kill @e[type=firework_rocket,distance=..48]'},
            {'after': t, 'cmd': 'clear $(bot)'},
            {'after': t + 1, 'cmd': 'tp $(bot) $(3.5,0,3.5) 0 -20'},
            {'after': t + 2, 'cmd': 'item replace entity $(bot) weapon.mainhand with ' + stack},
            {'after': t + 6, 'click': [5, 0, 3]},
            {'after': t + 10, 'close': True},
            # Открытие сундука поворачивает клиента; явно задаём безопасный выстрел вверх.
            {'after': t + 12, 'look': [math.pi, math.pi / 9], 'use': True},
            {'after': t + 22, 'click': [5, 0, 3]},
            {'after': t + 28, 'close': True},
        ])
    scenario.update(name=NAME, about='Discovery: остаток прочности арбалета и несколько заряженных снарядов. Нет заранее выбранного результата nearbreak.',
                    blocks=[{'at': [5, 0, 3], 'state': 'minecraft:chest[facing=west,type=single,waterlogged=false]'}],
                    actions=actions, record_ticks=INTERVAL * len(CASES), random=[], track_projectiles=True)
    scenario['setup'] = [v for v in scenario['setup'] if not v.startswith('tp ')] + [
        'tp $(bot) $(3.5,0,3.5) 0 -20', 'tp $(bot2) $(0.5,0,3.5) 90 0']
    scenario['cleanup'].insert(0, 'kill @e[type=firework_rocket,distance=..48]')
    scenario['cleanup'].insert(0, 'kill @e[type=spectral_arrow,distance=..48]')
    write_json(output / 'scenario.json', scenario)
    write_json(output / 'criteria-before-recording.json', {
        'name': NAME, 'protocol': 777, 'interval': INTERVAL, 'cases': CASES,
        'scenario_sha256': hashlib.sha256((output / 'scenario.json').read_bytes()).hexdigest(),
        'scope': 'Обычные use_item с заданными командой тремя зарядами, mainhand/survival/no-enchantments. Не тест естественной зарядки/Unbreaking/offhand.',
        'positive_controls': 'Полный непустой window_items сундука до выстрела: count1, exact damage/3 exact projectiles, mainhand inventory only. Единственный реальный use_item/случай. После выстрела второе полное window_items: held slot null либо exact surviving crossbow. Все spawn и owner entity_status сохранены у обоих наблюдателей, ID/type и их число совпадают. Не выводить отсутствие предмета из отсутствия SetSlot.',
        'discovery': 'Никаких nearbreak ожиданий или Rust patch ДО оригинала. Freshcases должны действительно стрелять. Нормализованные фактические строки original заморозить до реализации. Разницу raw packet order не скрывать.',
        'constraints': 'До root slot не запускать сервер/Cargo. В repo ничего не пишется этим prepare.'})


def inspect(raw, names, entities, repo):
    sys.path.insert(0, str(repo / 'tools/protocol-bridge'))
    from probe_crossbow_session import Probe
    from smoke import read_varint
    assert raw['scenario'] == NAME and raw['protocol'] == 777
    assert len(raw['clients']) == len(raw['initial']) == 2
    assert all(c['events'] and not c.get('failure') for c in raw['clients'])
    parser = object.__new__(Probe)
    parser.names, parser.version = names, 777
    snapshots, spawns, statuses = [], [[], []], [[], []]
    shooter = raw['initial'][0]['entity']
    for observer, client in enumerate(raw['clients']):
        for event in client['events']:
            data = io.BytesIO(bytes.fromhex(event['hex']))
            if event['name'] == 'window_items' and observer == 0:
                window, revision, count = [read_varint(data.read) for _ in range(3)]
                slots = [parser.slot(data) for _ in range(count)]
                cursor = parser.slot(data)
                assert not data.read() and cursor is None
                if window > 0:
                    assert count == 63 and slots[:27] == [None] * 27
                    snapshots.append((event['at'], slots))
            elif event['name'] == 'spawn_entity':
                entity = read_varint(data.read)
                assert len(data.read(16)) == 16
                kind = entities[read_varint(data.read)]
                if kind in ('arrow', 'spectral_arrow', 'firework_rocket'):
                    spawns[observer].append((event['at'], entity, kind, event['hex']))
            elif event['name'] == 'entity_status':
                payload = bytes.fromhex(event['hex'])
                assert len(payload) == 5
                entity, status = struct.unpack('>iB', payload)
                if entity == shooter:
                    statuses[observer].append((event['at'], status))
    uses = [r for r in raw['sent'] if r['action'].get('use')]
    clicks = [r for r in raw['sent'] if 'click' in r['action']]
    assert len(uses) == len(CASES) and len(clicks) == len(CASES) * 2
    rows = []
    for i, (case, use) in enumerate(zip(CASES, uses)):
        start = raw['start'] + i * INTERVAL * .05
        end = start + INTERVAL * .05
        assert use['action'] == {'after': i * INTERVAL + 12, 'look': [math.pi, math.pi / 9], 'use': True}
        before = [s for at, s in snapshots if start <= at < use['at']]
        after = [s for at, s in snapshots if use['at'] <= at < end]
        assert len(before) == len(after) == 1, (i, 'missing full server inventory', len(before), len(after))
        initial, final = before[0][54], after[0][54]
        assert initial and initial['item'] == 'crossbow' and initial['count'] == 1
        assert initial.get('damage', 0) == case['initial_damage']
        assert initial['projectiles'] == [{'item': p, 'count': 1} for p in case['projectiles']], (i, initial)
        assert set(initial) <= {'item', 'count', 'damage', 'projectiles'}, (i, initial)
        assert all(s is None for k, s in enumerate(before[0]) if k != 54)
        assert all(s is None for k, s in enumerate(after[0]) if k != 54)
        emitted = [[(entity, kind) for at, entity, kind, _ in side if use['at'] <= at < end] for side in spawns]
        assert sorted(emitted[0]) == sorted(emitted[1]), (i, emitted)
        assert 1 <= len(emitted[0]) <= 3, (i, 'no actual shot / unexpected projectiles')
        own_status = [[status for at, status in side if use['at'] <= at < end] for side in statuses]
        assert own_status[0] == own_status[1], (i, own_status)
        if final is not None:
            assert final['item'] == 'crossbow' and final['count'] == 1
            assert not final.get('projectiles') and final.get('damage', 0) >= case['initial_damage']
        rows.append(dict(trial=i, **case, emitted=[kind for _, kind in sorted(emitted[0])],
                         after=final, statuses_by_observer=own_status))
    return rows


def analyze(recordings, data, repo, output):
    names = {v['id']: v['name'] for v in json.loads((data / 'items.json').read_text())}
    entities = {v['id']: v['name'] for v in json.loads((data / 'entities.json').read_text())}
    sides = {side: inspect(json.loads((recordings / (NAME + '-' + side + '.json')).read_text()), names, entities, repo)
             for side in ['official', 'ours']}
    write_json(output, {'status': 'discovery-only-no-gameplay-pass', 'sides': sides,
                        'semantic_differences': [i for i, (a, b) in enumerate(zip(sides['official'], sides['ours'])) if a != b]})


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
        analyze(a.recordings, a.data, a.repo, a.output)
