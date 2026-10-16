#!/usr/bin/env python3
"""Offline 26.1.2 packet analysis. No clients, servers or protocol libraries.

Keep conditional hatch checks separate from random occurrence counts. Export
small vanilla fixtures for Rust replay; never compare independent random paths.
"""
import argparse
import collections
import json
import math
from pathlib import Path
import re
import statistics


def position(e):
    return list(map(float, e['extra'].split()[0].split(','))) if 'extra' in e else e['at']


def angles(v):
    return [math.degrees(math.atan2(v[0], v[2])),
            math.degrees(math.atan2(v[1], math.hypot(v[0], v[2])))]


def moments(values):
    mean = statistics.mean(values)
    variance = statistics.pvariance(values)
    return dict(n=len(values), mean=mean, sd=math.sqrt(variance),
                minimum=min(values), maximum=max(values),
                kurtosis=statistics.mean((v-mean)**4 for v in values)/variance**2)


def repeated(data):
    events = data['events']
    eggs = [e for e in events if e.get('type') == 'egg']
    pearls = [e for e in events if e.get('type') == 'ender_pearl']
    chicks = [e for e in events if e.get('type') == 'chicken']
    mites = [e for e in events if e.get('type') == 'endermite']
    counts = collections.Counter(e['tick'] for e in chicks)
    heads = collections.Counter(int(re.search(r'head=(-?\d+)', e['extra'])[1]) for e in chicks)
    conditional = []
    for chick in chicks:
        index = events.index(chick)
        egg = next(e for e in reversed(events[:index]) if e.get('type') == 'egg')
        status = next(i for i, e in enumerate(events) if e.get('id') == egg['id'] and e.get('what') == 'status')
        gone = next(i for i, e in enumerate(events) if e.get('id') == egg['id'] and e.get('what') == 'gone')
        meta = next(i for i, e in enumerate(events[index+1:], index+1) if e.get('id') == chick['id'] and e['kind'] == 'meta')
        rotation = list(map(int, re.search(r'yaw=(-?\d+) pitch=(-?\d+) head=(-?\d+)', chick['extra']).groups()))
        conditional.append(dict(at=position(chick), rotation=rotation,
                                order_ok=index < meta < status < gone,
                                same_tick=chick['tick'] == events[status]['tick'] == events[gone]['tick']))
    n = len(eggs or pearls)
    rate = len(counts) / n
    z = 1.96
    centre = (rate + z*z/(2*n)) / (1+z*z/n)
    radius = z*math.sqrt(rate*(1-rate)/n+z*z/(4*n*n)) / (1+z*z/n)
    result = dict(trials=n, hatches=len(counts), hatch_rate=rate,
                  hatch_rate_wilson_95=[centre-radius, centre+radius], chicks=len(chicks),
                  chicks_per_hatch=dict(collections.Counter(counts.values())),
                  head_bytes=dict(heads), endermites=len(mites), conditional=conditional)
    if data['scenario'] == 'throw3-egg-hatch-repeat':
        nominal = [-math.sqrt(3/8), -.5, -math.sqrt(3/8)]
        noise = [e['velocity'][axis]/1.5 - aim for e in eggs for axis, aim in zip('xyz', nominal)]
        result['spread'] = moments(noise)
    return result


def flight(data):
    events = data['events']
    spawn = next(e for e in events if e.get('type') in ('egg', 'snowball', 'ender_pearl'))
    launch_tick = next((e['tick'] for e in reversed(events[:events.index(spawn)])
                        if e.get('sound') == f"minecraft:entity.{spawn['type']}.throw"), spawn['tick'])
    velocity = [spawn['velocity'][axis] for axis in 'xyz']
    observations = []
    for e in events:
        if e.get('id') != spawn['id'] or e.get('what') not in ('vel', 'sync'):
            continue
        observation = dict(tick=e['tick']-launch_tick, velocity=e['v'])
        if e['what'] == 'sync':
            observation.update(at=e['at'], rotation=[e['yaw'], e['pitch']])
        observations.append(observation)
    gone = next((e['tick']-launch_tick for e in events if (e.get('id') == spawn['id'] and e.get('what') == 'gone') or spawn['id'] in e.get('ids', [])), None)
    at, v, age = position(spawn)[:], velocity[:], 0
    errors = dict(position=0.0, velocity=0.0)
    for observation in observations:
        while age < observation['tick']:
            v = [v[0]*.99, (v[1]-.03)*.99, v[2]*.99]
            at = [a+b for a,b in zip(at,v)]
            age += 1
        errors['velocity'] = max(errors['velocity'], max(abs(a-b) for a,b in zip(v,observation['velocity'])))
        if 'at' in observation:
            errors['position'] = max(errors['position'], max(abs(a-b) for a,b in zip(at,observation['at'])))
    predicted_impact = None
    if data['scenario'].startswith(('throw2-', 'throw3-')):
        at, v = position(spawn)[:], velocity[:]
        for tick in range(160):
            v = [v[0]*.99, (v[1]-.03)*.99, v[2]*.99]
            end = [a+b for a,b in zip(at,v)]
            if end[1] <= 0 or end[1] >= 64 or any(x <= -39 or x >= 40 for x in (end[0],end[2])):
                predicted_impact = dict(tick=tick, before_step=at)
                break
            at = end
    teleport = next((e for e in events if e.get('kind') == 'self' and e.get('what') == 'pos' and '"_value":504' in e.get('flags', '')), None)
    phases = []
    if teleport:
        for e in events[events.index(teleport):]:
            if e['tick'] > teleport['tick']+1:
                break
            if e.get('kind') in ('self', 'sv') or e.get('sound') == 'minecraft:entity.player.teleport' or e.get('what') == 'gone':
                phases.append(dict(offset=e['tick']-teleport['tick'], kind=e['kind'], what=e.get('text',e.get('what',e.get('sound')))))
    return dict(name=data['scenario'], kind=spawn['type'], start=position(spawn),
                teleport=teleport, teleport_phases=phases,
                replay_max_error=errors, predicted_box_impact=predicted_impact,
                velocity=velocity, observations=observations, gone=gone,
                spawn_extra=spawn.get('extra'), spawn_delay=spawn['tick']-launch_tick, rotation=angles(velocity))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--fixtures', type=Path)
    args = parser.parse_args()
    result, fixtures = {}, []
    for path in sorted(args.directory.glob('*-throw3-*.json')):
        if not path.name.startswith(('vanilla-', 'ours-')):
            continue
        data = json.loads(path.read_text())
        name = data['scenario']
        if name.endswith(('hatch-repeat', 'hatch-wall', 'endermite-repeat')):
            result[path.stem] = repeated(data)
        elif name.endswith(('falling-owner', 'owner-return', 'health-order', 'moving')):
            result[path.stem] = flight(data)
            if path.name.startswith('vanilla-'):
                fixtures.append({k:v for k,v in result[path.stem].items() if k not in ('replay_max_error', 'predicted_box_impact')})
    for name in ('throw-snowball-down20', 'throw-snowball-walk', 'throw-ender_pearl-flat',
                 'throw2-ender_pearl-up45-yaw90-standing'):
        for side in ('vanilla', 'ours'):
            path = args.directory / f'{side}-{name}.json'
            result[path.stem] = flight(json.loads(path.read_text()))
            if side == 'vanilla':
                fixtures.append({k:v for k,v in result[path.stem].items() if k not in ('replay_max_error', 'predicted_box_impact')})
    result['limitations'] = [
        'Endermite-repeat inherits spawn_mobs=false from bot.js: zero is not a 5% measurement.',
        'Owner-return misses the owner laterally: it cannot locate the exclusion boundary.',
        'The falling-owner runs launch from different heights/ticks; do not hide this with random jitter.',
        'Chick head bounds are measured; the continuous distribution and wall/corner generalization need another probe.',
    ]
    if args.fixtures:
        args.fixtures.write_text(json.dumps(fixtures, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
