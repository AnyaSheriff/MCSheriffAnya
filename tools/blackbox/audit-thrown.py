#!/usr/bin/env python3
"""Offline audit of recorded packets; never starts a client or a server.

Use decoded launch velocities, retaining the uncertainty of their packed wire
format. Independent random launches are NOT compared as identical trajectories.
"""
import argparse
import json
import math
import re
from pathlib import Path

KINDS = {'snowball', 'egg', 'ender_pearl'}


def angles(v):
    return [math.degrees(math.atan2(v[0], v[2])),
            math.degrees(math.atan2(v[1], math.hypot(v[0], v[2])))]


def byte(a):
    return (math.floor(a * 256 / 360) + 128) % 256 - 128


def audit(path):
    data = json.loads(path.read_text())
    events = data['events']
    spawn = next(e for e in events if e.get('type') in KINDS)
    start = spawn['at']
    if 'extra' in spawn:
        start = list(map(float, spawn['extra'].split()[0].split(',')))
    velocity = [spawn['velocity'][k] for k in 'xyz']
    scenario = json.loads((Path('tools/blackbox/scenarios') / (data['scenario'] + '.json')).read_text())
    moving = any(word in data['scenario'] for word in ('walk', 'sprint', 'falling'))
    look = next(s for s in reversed(scenario['setup']) if s.startswith('!look '))
    yaw, pitch = map(float, look.split()[1:])
    # Старые throw-* ошибочно называют радианы градусами. Направление
    # восстанавливаем тригонометрией, а не по имени сценария.
    nominal = angles([math.sin(yaw) * math.cos(pitch), math.sin(pitch), -math.cos(yaw) * math.cos(pitch)])
    initial = [angles(velocity)[0], nominal[1]] if moving else angles(velocity)
    recorded_rotation = None
    rotation_check = 'not recorded'
    if 'extra' in spawn:
        recorded_rotation = list(map(int, re.search(r'yaw=(-?\d+) pitch=(-?\d+)', spawn['extra']).groups()))
        # Движение клиента не записано: для moving проверяется совместимость
        # с направлением взгляда и разбросом, а не выдуманная точная скорость.
        tolerance = 1.5 if moving else 0.01
        feasible = all(any(byte(a + offset) == b for offset in (-tolerance, 0, tolerance))
                       for a, b in zip(initial, recorded_rotation))
        rotation_check = 'compatible' if feasible else 'mismatch'
    at, v, rot = start[:], velocity[:], initial[:]
    observations = []
    elapsed = 0
    maximum = [0.0, 0.0, 0.0]
    cadence = []
    for e in events:
        if e.get('id') != spawn['id'] or e.get('what') not in ('vel', 'sync', 'move'):
            continue
        tick = e['tick'] - spawn['tick']
        if not tick:
            continue
        while elapsed < tick:
            v = [v[0] * .99, (v[1] - .03) * .99, v[2] * .99]
            at = [a + b for a, b in zip(at, v)]
            target = angles(v)
            rot = [rot[0] + ((target[0] - rot[0] + 180) % 360 - 180) * .2,
                   rot[1] + (target[1] - rot[1]) * .2]
            elapsed += 1
        if e['what'] == 'vel':
            cadence.append(tick)
        if e['what'] not in ('vel', 'sync'):
            continue
        observation = {'tick': tick, 'velocity': e['v']}
        maximum[1] = max(maximum[1], max(abs(a - b) for a, b in zip(v, e['v'])))
        if e['what'] == 'sync':
            observation.update(at=e['at'], rotation=[e['yaw'], e['pitch']])
            maximum[0] = max(maximum[0], max(abs(a - b) for a, b in zip(at, e['at'])))
            maximum[2] = max(maximum[2], max(abs(a - b) for a, b in zip(rot, observation['rotation'])))
        observations.append(observation)
    fixture = dict(name=data['scenario'], kind=spawn['type'], start=start,
                   velocity=velocity, rotation=initial, moving=moving,
                   spawn_rotation=recorded_rotation, observations=observations)
    gone = next((e['tick'] - spawn['tick'] for e in events
                 if e.get('id') == spawn['id'] and e.get('what') == 'gone'), None)
    # throw2 — закрытая коробка: внутренние грани -39 и 40, пол y=0.
    impact = None
    if data['scenario'].startswith('throw2-'):
        pos, vel = start[:], velocity[:]
        for t in range(200):
            vel = [vel[0] * .99, (vel[1] - .03) * .99, vel[2] * .99]
            end = [a + b for a, b in zip(pos, vel)]
            if end[1] <= 0 or end[1] >= 64 or any(x <= -39 or x >= 40 for x in (end[0], end[2])):
                impact = t
                break
            pos = end
    launch_action = next(e for e in events if e.get('what') == 'use_item')
    result = dict(file=path.name, spawn_delay=spawn['tick'] - launch_action['tick'],
                  rotation=rotation_check, max_position_error=maximum[0],
                  max_velocity_error=maximum[1], max_rotation_error=maximum[2],
                  velocity_ticks=cadence, gone_tick=gone, box_impact_tick=impact,
                  chickens=sum(e.get('type') == 'chicken' for e in events),
                  endermites=sum(e.get('type') == 'endermite' for e in events))
    return result, fixture


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--fixtures', type=Path)
    args = parser.parse_args()
    results, fixtures = [], []
    for path in sorted(args.directory.glob('*.json')):
        if not path.name.startswith(('vanilla-throw', 'ours-throw')):
            continue
        result, fixture = audit(path)
        results.append(result)
        if path.name.startswith('vanilla-'):
            fixtures.append(fixture)
    if args.fixtures:
        args.fixtures.parent.mkdir(parents=True, exist_ok=True)
        args.fixtures.write_text('[\n' + ',\n'.join(json.dumps(f, ensure_ascii=False) for f in fixtures) + '\n]\n')
    print(json.dumps(results, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
