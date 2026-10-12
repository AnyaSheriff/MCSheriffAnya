#!/usr/bin/env python3
"""Матрица throw2: только JSON, без запуска игры или записывателя."""
import json
import math
from pathlib import Path

DEST = Path(__file__).resolve().parent / 'scenarios'
ITEMS = ['snowball', 'egg', 'ender_pearl', 'splash_potion', 'lingering_potion',
         'experience_bottle', 'wind_charge', 'trident', 'bow', 'crossbow']
MOTIONS = ['standing', 'walking', 'sprinting', 'sprintjump-apex',
           'sprintjump-falling', 'falling', 'minecart', 'boat']
PITCHES = [('up45', -45), ('flat', 0), ('down30', 30)]
YAWS = [('yaw0', 0), ('yaw90', 90), ('yaw-135', -135)]


def fill(a, b, block):
    def pos(p):
        return '$(' + ','.join(map(str, p)) + ')'
    return f'fill {pos(a)} {pos(b)} minecraft:{block}'


# Каждый /fill укладывается в обычный предел 32768 блоков.
AIR = [fill((-40, y, -40), (40, min(y + 3, 64), 40), 'air') for y in range(0, 65, 4)]
FLOOR = fill((-40, -1, -40), (40, -1, 40), 'stone')
WALLS = [fill((-40, 0, -40), (-40, 63, 40), 'stone'),
         fill((40, 0, -40), (40, 63, 40), 'stone'),
         fill((-39, 0, -40), (39, 63, -40), 'stone'),
         fill((-39, 0, 40), (39, 63, 40), 'stone'),
         fill((-40, 64, -40), (40, 64, 40), 'stone')]
# Уборка привязана к площадке, даже если жемчуг перенёс робота.
KILL = ['execute positioned $(0,0,0) run kill @e[type=' + kind + ',distance=..128]'
        for kind in ['snowball', 'egg', 'ender_pearl', 'splash_potion', 'lingering_potion', 'experience_bottle',
                     'wind_charge', 'trident', 'arrow', 'spectral_arrow', 'firework_rocket',
                     'area_effect_cloud', 'chicken', 'endermite', 'experience_orb',
                     'minecart', 'oak_boat', 'item']]


def scenario(item, pitch_name, pitch, yaw_name, yaw, motion):
    name = f'throw2-{item}-{pitch_name}-{yaw_name}-{motion}'
    # Mineflayer: yaw = pi − yaw Minecraft; положительный pitch смотрит вверх.
    look = [math.pi - math.radians(yaw), -math.radians(pitch)]
    setup = ['difficulty peaceful', 'gamemode creative $(bot)', 'clear $(bot)', 'ride $(bot) dismount'] + KILL + AIR + [FLOOR] + WALLS
    if motion == 'minecart':
        setup += [fill((3, -1, -30), (3, -1, 30), 'redstone_block'),
                  fill((3, 0, -30), (3, 0, 30), 'powered_rail[shape=north_south,powered=true]')]
    if motion == 'boat':
        setup += [fill((1, 0, 0), (5, 0, 12), 'water')]
    setup += ['tp $(bot) $(3.5,1,2.5) 0 0', '!stopfly', 'gamemode survival $(bot)',
              'effect give $(bot) minecraft:instant_health 1 5 true',
              '!hotbar 0', f'give $(bot) minecraft:{item} 1']
    if item in ['bow', 'crossbow']:
        setup += ['give $(bot) minecraft:arrow 16']
    setup += [f'!look {look[0]:.15f} {look[1]:.15f}', '!wait 10']
    actions = []

    def act(t, **fields):
        actions.append(dict(after=t, **fields))

    base = 0
    if item in ['bow', 'trident', 'crossbow']:
        act(0, use=True)
        base = 32
        if item == 'crossbow':
            act(28, release=True)
    if motion == 'standing':
        throw = base + 2
    elif motion in ['walking', 'sprinting', 'sprintjump-apex', 'sprintjump-falling']:
        act(base, control=['forward', True])
        if motion != 'walking':
            act(base, control=['sprint', True])
        throw = base + 10
        if motion.startswith('sprintjump'):
            act(base + 6, control=['jump', True])
            act(base + 7, control=['jump', False])
            throw = base + (11 if motion.endswith('apex') else 14)
        act(throw + 2, control=['forward', False])
        act(throw + 2, control=['sprint', False])
    elif motion == 'falling':
        act(base, cmd=f'tp $(bot) $(3.5,8,2.5) {yaw} {pitch}')
        act(base + 2, look=look)
        throw = base + 6
    elif motion == 'minecart':
        act(base, cmd='summon minecraft:minecart $(3.5,0.1,2.5) {Motion:[0.0d,0.0d,0.3d]}')
        act(base + 2, interact=True)
        act(base + 5, look=look)
        throw = base + 10
    else:
        act(base, cmd='summon minecraft:oak_boat $(3.5,0.4,2.5)')
        act(base + 2, interact=True)
        act(base + 5, look=look)
        for tick in range(6, 15):
            act(base + tick, raw=[
                dict(name='steer_boat', params=dict(leftPaddle=True, rightPaddle=True)),
                dict(name='vehicle_move', params=dict(x='@x:3.5', y='@y:0.4',
                     z=f'@z:{2.5 + (tick - 5) * 0.15:.2f}', yaw=0.0, pitch=0.0, onGround=False))])
        throw = base + 12
    act(throw, **({'release': True} if item in ['bow', 'trident'] else {'use': True}))
    if motion == 'boat':
        act(throw + 4, raw=dict(name='steer_boat', params=dict(leftPaddle=False, rightPaddle=False)))
    # Отпустить все кнопки, которые мог нажать сценарий.
    for control in ['jump', 'forward', 'sprint']:
        act(throw + 6, control=[control, False])
    actions.sort(key=lambda action: action['after'])
    cleanup = ['ride $(bot) dismount', 'gamemode creative $(bot)', 'tp $(bot) $(3.5,1,2.5)', '!startfly']
    cleanup += KILL + AIR + [FLOOR, 'clear $(bot)', 'effect clear $(bot)', '!nosprint']
    return dict(name=name,
        about=f'{item}: Minecraft yaw {yaw}°, pitch {pitch}°; {motion}. '
              'Angles passed to mineflayer are radians. [проверить] Release phase and mounted movement against vanilla 26.1.2.',
        blocks=[], setup=setup, actions=actions, cleanup=cleanup, record_ticks=180,
        track_projectiles=True, track_self=True, track_mobs=True, pad_wide=64, pad_up=80,
        timeout_ms=240000)


def write_all():
    count = 0
    for item in ITEMS:
        for pn, pitch in PITCHES:
            for yn, yaw in YAWS:
                for motion in MOTIONS:
                    spec = scenario(item, pn, pitch, yn, yaw, motion)
                    (DEST / (spec['name'] + '.json')).write_text(json.dumps(spec, ensure_ascii=False, indent=1) + '\n')
                    count += 1
    print(f'Wrote {count} throw2 scenarios')


if __name__ == '__main__':
    write_all()
