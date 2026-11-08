#!/usr/bin/env python3
"""Обычные ШипыIII: фиксированная выборка настоящих ударов и сырых HP/Slot777.

Статистический аудит отдельно от exact-tick compare.js. Старые маски не меняются.
"""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import sys

from combat_wire import health_packet, inventory_changes
from defense_ci import exact, finished, uint

ROOT = Path(__file__).resolve().parents[2]
NAME = 'pvp-thorns-natural-sample-777'
INTERVAL = 32


def binomial_bounds(n, probability=.45, alpha=.001):
    masses = [math.comb(n, k) * probability**k * (1-probability)**(n-k) for k in range(n+1)]
    lower = 0
    while sum(masses[:lower+1]) < alpha/2:
        lower += 1
    upper = n
    while sum(masses[upper:]) < alpha/2:
        upper -= 1
    return [lower, upper]


def prepare(output, smoke=False):
    output.mkdir(parents=True, exist_ok=False)
    counts = [2, 4, 2] if smoke else [8, 256, 8]
    levels = [0]*counts[0] + [3]*counts[1] + [7]*counts[2]
    actions = []
    for i, level in enumerate(levels):
        tick = i*INTERVAL
        for name in ('$(bot)', '$(bot2)'):
            actions.append({'after': tick, 'cmd': 'effect give '+name+' minecraft:instant_health 1 10'})
        actions += [{'after': tick+2, 'cmd': 'tp $(bot) $(3.5,0,3.5) 180 0'},
                    {'after': tick+2, 'cmd': 'tp $(bot2) $(3.5,0,1.5) 0 0'}]
        item = 'minecraft:diamond_chestplate[damage=0'+(',enchantments={thorns:'+str(level)+'}' if level else '')+']'
        actions += [{'after': tick+4, 'cmd': 'item replace entity $(bot) armor.chest with '+item},
                    {'after': tick+12, 'attack': True, 'bot': 2}]
    base = json.loads((ROOT/'tools/blackbox/scenarios/pvp-thorns-control-777.json').read_text())
    base.update(name=NAME, about='Фиксированные обычные удары: plain/ThornsIII/forcedVII, HP20 перед каждым, freshchestplate. Статистика отдельно от exact comparator.',
                actions=actions, record_ticks=len(levels)*INTERVAL, random=[])
    base['setup'].insert(2, 'effect clear $(bot)')
    base['setup'].insert(3, 'effect clear $(bot2)')
    target = ROOT/'tools/blackbox/scenarios'/f'{NAME}.json'
    assert not target.exists(), 'Never overwrite an existing scenario'
    raw = (json.dumps(base, ensure_ascii=False, indent=2)+'\n').encode()
    target.write_bytes(raw)
    (output/'scenario.json').write_bytes(raw)
    criteria = {'schema': 'natural-thorns-777-v1', 'scenario': NAME, 'smoke': smoke,
                'levels': levels, 'interval_ticks': INTERVAL, 'attack_offset_ticks': 12,
                'scenario_sha256': hashlib.sha256(raw).hexdigest(),
                'probability': .45, 'alpha_per_side': .001,
                'natural_count_bounds': None if smoke else binomial_bounds(counts[1]),
                'positive': 'Every trial: observed HP20 before real fist attack; fresh correct diamond chestplate/enchantment; exactly one correctly attributed player_attack received by both observers; victim HP falls; no death/missing packet/truncated semantic payload',
                'reflection': 'Plain0 none; forcedVII all; normalIII binomial p=.45, alpha=.001 per side fixed before sample. Both observers must see same attributed thorns event iff attacker HP falls, real damage1..5, wear3 iff reflection else wear1. No masking of HP/count/gear.',
                'limits': 'Not exact RNG equality, not a complete RNG distribution or full B06/B07 proof. Other armor, Unbreaking, ranged and breaking are outside this experiment.'}
    (output/'criteria-before-recording.json').write_text(json.dumps(criteria, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({'scenario': str(target), 'trials': len(levels), 'bounds': criteria['natural_count_bounds']}))


def damage(event, names):
    source = io.BytesIO(bytes.fromhex(event['hex']))
    entity, kind, cause, direct = [uint(source) for _ in range(4)]
    present = exact(source, 1)
    assert present in (b'\0', b'\1')
    if present == b'\1':
        exact(source, 24)
    finished(source)
    assert 0 <= kind < len(names)
    return {'entity': entity, 'kind': names[kind], 'cause': cause, 'direct': direct, 'at': event['at']}


def inspect(raw, criteria, items):
    assert raw['scenario'] == NAME and raw['protocol'] == 777 and raw['isolate_vitals'] is True
    assert len(raw['initial']) == len(raw['clients']) == 2
    victim, attacker = [x['entity'] for x in raw['initial']]
    for initial in raw['initial']:
        assert initial['health'] == health_packet(initial['health_receipt']) == 20
        assert initial['health_receipt']['at'] <= raw['start']
    levels = criteria['levels']
    attacks = [x for x in raw['sent'] if x['action'].get('attack')]
    assert len(attacks) == len(levels)
    for i, action in enumerate(attacks):
        assert action['action'] == {'after': i*INTERVAL+12, 'attack': True, 'bot': 2}
    health, gear, harms = [[], []], [], [[], []]
    for side, client in enumerate(raw['clients']):
        assert client['events'] and not client.get('failure')
        health[side].append((raw['start']-.001, 20.0))
        for event in client['events']:
            if event['name'] == 'update_health':
                health[side].append((event['at'], health_packet(event)))
            elif event['name'] == 'damage_event':
                harms[side].append(damage(event, client['registry_names']['minecraft:damage_type']))
            if side == 0:
                slots = inventory_changes(event)
                if 6 in slots:
                    gear.append((event['at'], slots[6]))
    enchantments = raw['clients'][0]['registry_names']['minecraft:enchantment']
    results = []
    for i, (level, attack) in enumerate(zip(levels, attacks)):
        start = raw['start'] + i*INTERVAL*.05
        end = raw['start'] + (i+1)*INTERVAL*.05
        at = attack['at']
        before = [next(hp for timestamp, hp in reversed(values) if timestamp < at) for values in health]
        after = [next(hp for timestamp, hp in reversed(values) if timestamp < end) for values in health]
        assert before == [20., 20.], ('HP not reset', i, before)
        assert 0 < after[0] < 20, ('no damaging melee', i, after)
        initial = next(stack for timestamp, stack in reversed(gear) if timestamp < at)
        final = next(stack for timestamp, stack in reversed(gear) if timestamp < end)
        assert initial and final and initial['item'] == final['item'] == items['diamond_chestplate']
        assert initial.get('damage', 0) == 0, ('not a fresh chestplate', i, initial)
        actual = {enchantments[number]: value for number, value in initial.get('enchantments', [])}
        assert actual == ({'minecraft:thorns': level} if level else {}), ('wrong enchantment', i, actual)
        reflected = []
        for side, events in enumerate(harms):
            events = [x for x in events if start <= x['at'] < end]
            incoming = [e for e in events if e['entity'] == victim and e['kind'] == 'minecraft:player_attack' and e['cause'] == e['direct'] == attacker+1]
            returns = [e for e in events if e['entity'] == attacker and e['kind'] == 'minecraft:thorns' and e['cause'] == e['direct'] == victim+1]
            assert len(incoming) == 1 and len(returns) <= 1 and len(events) == len(incoming)+len(returns), ('wrong causal hits', i, side, events)
            reflected.append(len(returns))
        assert reflected[0] == reflected[1], ('observers disagree', i)
        hit = bool(reflected[0])
        dealt = 20-after[1]
        assert (1 <= dealt <= 5) if hit else dealt == 0, ('reflection HP not observed', i, after)
        assert final.get('damage') == (3 if hit else 1), ('wrong actual wear', i, final, hit)
        if level in (0, 7):
            assert hit == bool(level), ('failed deterministic control', i, level, hit)
        results.append({'trial': i, 'level': level, 'reflection': hit, 'victim_damage': 20-after[0], 'reflected_damage': dealt, 'wear': final['damage']})
    natural = [r for r in results if r['level'] == 3]
    count = sum(r['reflection'] for r in natural)
    if not criteria['smoke']:
        low, high = criteria['natural_count_bounds']
        assert low <= count <= high, ('natural chance outside declared binomial bounds', count, low, high)
    values = [r['reflected_damage'] for r in natural if r['reflection']]
    return {'trials': results, 'natural_trials': len(natural), 'natural_reflections': count,
            'natural_rate': count/len(natural), 'natural_damage_min': min(values, default=None),
            'natural_damage_max': max(values, default=None), 'natural_damage_mean': sum(values)/len(values) if values else None}


def analyze(directory, recordings, data):
    criteria = json.loads((directory/'criteria-before-recording.json').read_text())
    assert hashlib.sha256((directory/'scenario.json').read_bytes()).hexdigest() == criteria['scenario_sha256']
    items = {x['name']: x['id'] for x in json.loads((data/'items.json').read_text())}
    summary = {'criteria': criteria, 'sides': {}, 'status': 'failed', 'scope': 'statistical semantic audit; not exact packet comparison'}
    for side in ('official', 'ours'):
        path = recordings/f'{NAME}-{side}.json'
        summary['sides'][side] = inspect(json.loads(path.read_text()), criteria, items)
    # Случайные исходы не приравниваются. Детерминированный урон жертвы обязан совпадать.
    a, b = [summary['sides'][s] for s in ('official', 'ours')]
    assert [r['victim_damage'] for r in a['trials']] == [r['victim_damage'] for r in b['trials']], 'victim damage differs'
    summary['status'] = 'verified-smoke' if criteria['smoke'] else 'verified-declared-statistical-controls'
    (directory/'semantic-result.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({side: {k: v for k, v in row.items() if k != 'trials'} for side, row in summary['sides'].items()}, indent=2))


def ci_run(output):
    """Только CI: тяжёлый повтор в отдельной VM, JAR остаётся непрозрачным."""
    assert os.environ.get('GITHUB_ACTIONS') == 'true'
    import combat_ci
    work = Path(os.environ['RUNNER_TEMP'])/'mcsa-natural-thorns'
    work.mkdir(exist_ok=False)
    prepare(output)
    from thorns_unbreaking_regression import prepare as prepare_unbreaking, analyze as analyze_unbreaking, NAME as UNBREAKING_NAME
    unbreaking=output/'unbreaking';prepare_unbreaking(unbreaking)
    sources = json.loads((ROOT/'tools/blackbox/ci/protection-edges-777.json').read_text())
    data = work/'data';data.mkdir()
    combat_ci.fetch(sources['official_jar'], work/'server.jar')
    for entry in sources['json_files']:
        if entry['file'] in ('protocol.json', 'items.json', 'blocks.json', 'entities.json'):
            combat_ci.fetch(entry, data/entry['file'])
    runtime = output/'runtime';runtime.mkdir()
    binary = runtime/'mcsheriffanya';shutil.copy2(ROOT/'target/release/mcsheriffanya', binary);binary.chmod(0o700)
    shutil.copy2(ROOT/'Cargo.lock', runtime/'Cargo.lock')
    manifest = {'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'runtime_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                'sources_sha256': {name: hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in
                                   ['tools/blackbox/thorns_natural_sample.py', 'tools/blackbox/record777.py', 'tools/blackbox/combat_wire.py', 'tools/blackbox/test_thorns_natural_sample.py', 'tools/blackbox/thorns_unbreaking_regression.py', 'tools/blackbox/test_thorns_unbreaking_regression.py']},
                'pinned_inputs': sources, 'status': 'running'}
    (output/'run.json').write_text(json.dumps(manifest, indent=2)+'\n')
    command = [sys.executable, str(ROOT/'tools/blackbox/record777.py'), '--data', str(data),
               '--output', str(work/'recordings'), '--binary', str(binary.resolve()), '--jar', str(work/'server.jar'),
               '--java', 'java', '--official-port', '26160', '--core-port', '26162',
               '--parallel', '--max-seconds', '900', '--isolate-vitals', NAME]
    try:
        with (output/'recorder.log').open('w') as log:
            subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=960)
        analyze(output, work/'recordings', data)
        lifecycle = json.loads((work/'recordings/lifecycle.json').read_text())
        assert not lifecycle['cleanup_errors'] and all(lifecycle['ports_free'].values()), lifecycle
        second=command.copy();second[second.index('--output')+1]=str(work/'unbreaking-recordings');second[-1]=UNBREAKING_NAME
        with (unbreaking/'recorder.log').open('w') as log:
            subprocess.run(second,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=960)
        analyze_unbreaking(unbreaking,work/'unbreaking-recordings',data)
        second_lifecycle=json.loads((work/'unbreaking-recordings/lifecycle.json').read_text())
        assert not second_lifecycle['cleanup_errors'] and all(second_lifecycle['ports_free'].values()),second_lifecycle
        manifest['status'] = 'verified-declared-statistical-controls'
    finally:
        # Только свои сырые наблюдения и логи, не JAR/ресурсы/официальные чанки.
        for path in (work/'recordings').glob('*.json'):
            shutil.copy2(path, output/path.name)
        for side in ('official', 'ours'):
            source = work/'recordings'/side/'server.log'
            if source.exists():
                shutil.copy2(source, output/(side+'-server.log'))
        for path in data.glob('*.json'):
            shutil.copy2(path, output/path.name)
        for path in (work/'unbreaking-recordings').glob('*.json'):
            shutil.copy2(path,unbreaking/path.name)
        for side in ('official','ours'):
            source=work/'unbreaking-recordings'/side/'server.log'
            if source.exists():shutil.copy2(source,unbreaking/(side+'-server.log'))
        if manifest['status']=='running':manifest['status']='failed'
        (output/'run.json').write_text(json.dumps(manifest, indent=2)+'\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', 'analyze', 'ci'])
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('--recordings', type=Path)
    parser.add_argument('--data', type=Path)
    args = parser.parse_args()
    if args.mode == 'prepare':
        prepare(args.output, args.smoke)
    elif args.mode == 'analyze':
        analyze(args.output, args.recordings, args.data)
    else:
        ci_run(args.output)


if __name__ == '__main__':
    main()
