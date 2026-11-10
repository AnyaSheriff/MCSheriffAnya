#!/usr/bin/env python3
"""Следующий discovery: фиксированные128 nearbreakUBIII, без автоматического запуска."""
import argparse
import hashlib
import io
import json
import math
import struct
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent
CRITERIA = BASE / 'ci/crossbow-ub128-criteria.json'
CRITERIA_SHA256 = '2fa312fb5a41fb04e0b2e761566603f38b8cc71d5d076e8bb9b6db12070f4a02'
NAME = 'crossbow-unbreaking-nearbreak-observe-777'
INTERVAL = 40
ROTATION = [math.pi, math.pi / 9]
CASES = [dict(control=True, initial_damage=0, unbreaking=0)] * 2 + [
    dict(control=False, initial_damage=464, unbreaking=3)] * 128 + [
    dict(control=True, initial_damage=0, unbreaking=0)] * 2


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def criteria():
    raw = CRITERIA.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == CRITERIA_SHA256
    return json.loads(raw)


def prepare(output, repo):
    fixed = criteria()
    output.mkdir(parents=True, exist_ok=False)
    s = json.loads((repo / 'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text())
    actions = []
    for i, case in enumerate(CASES):
        tick = i * INTERVAL
        specification = 'minecraft:crossbow[damage=' + str(case['initial_damage'])
        if case['unbreaking']:
            specification += ',enchantments={unbreaking:3}'
        specification += ',charged_projectiles=[{id:"minecraft:arrow",count:1},{id:"minecraft:arrow",count:1},{id:"minecraft:arrow",count:1}]]'
        actions.extend([
            dict(after=tick, cmd='kill @e[type=arrow]'),
            dict(after=tick, cmd='clear $(bot)'),
            dict(after=tick + 1, cmd='tp $(bot) $(3.5,0,3.5) 0 -20'),
            dict(after=tick + 2, cmd='item replace entity $(bot) weapon.mainhand with ' + specification),
            dict(after=tick + 10, click=[5, 0, 3]),
            dict(after=tick + 16, close=True),
            dict(after=tick + 20, look=ROTATION, use=True),
            dict(after=tick + 28, click=[5, 0, 3]),
            dict(after=tick + 34, close=True),
        ])
    s.update(name=NAME, about='UBIIInearbreak fixed128,4plainfreshcontrols; predeclared4categorybinomialbounds; discoveryНЕготовыйfix.',
             actions=actions, record_ticks=len(CASES) * INTERVAL, random=[], track_projectiles=True,
             blocks=[dict(at=[5, 0, 3], state='minecraft:chest[facing=west,type=single,waterlogged=false]')])
    s['setup'] = [v for v in s['setup'] if not v.startswith('tp ')] + [
        'tp $(bot) $(3.5,0,3.5) 0 -20', 'tp $(bot2) $(0.5,0,3.5) 90 0']
    write(output / 'scenario.json', s)
    (output / 'fixed-probability-criteria.json').write_bytes(CRITERIA.read_bytes())
    write(output / 'criteria-before-recording.json', dict(
        fixed_criteria_sha256=CRITERIA_SHA256,
        scenario_sha256=hashlib.sha256((output / 'scenario.json').read_bytes()).hexdigest(),
        cases=CASES, interval=INTERVAL, record_ticks=s['record_ticks'],
        scope=fixed['scope'], note='Границы/128неизменныДОсъёмки. НикакихRustизмененийдоoriginal. ПроигравшийoriginalНЕпересниматьрадиgreen.'))


def inspect(raw, names, entities, repo):
    sys.path.insert(0, str(repo / 'tools/protocol-bridge'))
    from probe_crossbow_session import Probe
    from smoke import read_varint
    assert raw['scenario'] == NAME and raw['protocol'] == 777
    assert len(raw['clients']) == len(raw['initial']) == 2
    assert all(c['events'] and not c.get('failure') for c in raw['clients'])
    parser = object.__new__(Probe)
    parser.names, parser.version = names, 777
    snapshots, spawns, breaks = [], [[], []], [[], []]
    owner = raw['initial'][0]['entity']
    for observer, client in enumerate(raw['clients']):
        for event in client['events']:
            data = io.BytesIO(bytes.fromhex(event['hex']))
            if observer == 0 and event['name'] == 'window_items':
                window, revision, count = [read_varint(data.read) for _ in range(3)]
                assert count <= 100
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
                    spawns[observer].append((event['at'], entity, kind))
            elif event['name'] == 'entity_status':
                payload = bytes.fromhex(event['hex'])
                assert len(payload) == 5
                entity, status = struct.unpack('>iB', payload)
                if entity == owner and 47 <= status <= 52:
                    breaks[observer].append((event['at'], status))
    uses = [r for r in raw['sent'] if r['action'].get('use')]
    assert len(uses) == len(CASES)
    enchantments = raw['clients'][0]['registry_names']['minecraft:enchantment']
    rows = []
    for i, (case, use) in enumerate(zip(CASES, uses)):
        start = raw['start'] + i * INTERVAL * .05
        end = start + INTERVAL * .05
        assert use['action'] == dict(after=i * INTERVAL + 20, look=ROTATION, use=True)
        before = [s for at, s in snapshots if start <= at < use['at']]
        after = [s for at, s in snapshots if use['at'] <= at < end]
        assert len(before) == len(after) == 1
        initial, final = before[0][54], after[0][54]
        assert initial and initial['item'] == 'crossbow' and initial['count'] == 1
        assert initial.get('damage', 0) == case['initial_damage']
        assert {enchantments[n]: v for n, v in initial.get('enchantments', [])} == ({'minecraft:unbreaking': 3} if case['unbreaking'] else {})
        assert initial['projectiles'] == [dict(item='arrow', count=1)] * 3
        for snapshot in before + after:
            assert all(stack is None for slot, stack in enumerate(snapshot) if slot != 54)
        emitted = [[(entity, kind) for at, entity, kind in side if use['at'] <= at < end] for side in spawns]
        assert sorted(emitted[0]) == sorted(emitted[1]) and 1 <= len(emitted[0]) <= 3
        assert all(kind == 'arrow' for _, kind in emitted[0])
        statuses = [[status for at, status in side if use['at'] <= at < end] for side in breaks]
        assert statuses[0] == statuses[1]
        if final:
            assert final['item'] == 'crossbow' and final['count'] == 1 and not final.get('projectiles')
            assert final.get('enchantments', []) == initial.get('enchantments', [])
            assert statuses == [[], []]
            assert final.get('damage', 0) == (3 if case['control'] else 464)
        else:
            assert not case['control'] and statuses == [[47], [47]]
        count = len(emitted[0])
        if case['control']:
            assert count == 3 and final is not None
            category = 'plain_fresh_control'
        else:
            assert count == 3 or final is None, 'salvo stopped without breaking'
            category = ('one_arrow_then_break' if count == 1 else 'two_arrows_then_break' if count == 2 else
                        'three_arrows_then_break' if final is None else 'three_arrows_survived')
        rows.append(dict(trial=i, control=case['control'], emitted=count, after=final,
                         statuses_by_observer=statuses, category=category))
    return rows


def statistics(rows):
    fixed = criteria()
    assert len(rows) == 132
    controls = [r for r in rows if r['control']]
    sample = [r for r in rows if not r['control']]
    assert len(controls) == 4 and len(sample) == fixed['sample_trials_per_side'] == 128
    assert [r['trial'] for r in controls] == [0, 1, 130, 131]
    assert all(r['category'] == 'plain_fresh_control' and r['emitted'] == 3 and r['after'] is not None for r in controls)
    counts = {label: sum(r['category'] == label for r in sample) for label in fixed['categories']}
    assert sum(counts.values()) == 128
    failures = [label for label, count in counts.items()
                if not fixed['categories'][label]['inclusive_count_bounds'][0] <= count <= fixed['categories'][label]['inclusive_count_bounds'][1]]
    return dict(fixed_n=128, counts=counts, failures=failures, passed=not failures,
                fixed_criteria_sha256=CRITERIA_SHA256)


def analyze(output, recordings, data, repo):
    sys.path.insert(0, str(repo / 'tools/blackbox'))
    from crossbow_nearbreak_ci import check_raw_controls
    names = {v['id']: v['name'] for v in json.loads((data / 'items.json').read_text())}
    entities = {v['id']: v['name'] for v in json.loads((data / 'entities.json').read_text())}
    protocol = json.loads((data / 'protocol.json').read_text())
    rows, samples = {}, {}
    for side in ['official', 'ours']:
        raw = json.loads((recordings / (NAME + '-' + side + '.json')).read_text())
        check_raw_controls(raw, protocol, repo)
        rows[side] = inspect(raw, names, entities, repo)
        samples[side] = statistics(rows[side])
    write(output, dict(status='discovery-not-a-candidate-fix', sides=rows, samples=samples,
                       original_hypothesis_passed=samples['official']['passed'],
                       note='Неравенствослучайныхвыборокнеошибка. Еслиoriginalнепрошёлфиксграницы,сохранитьfailureиНЕменятьих.'))


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
