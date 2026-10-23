#!/usr/bin/env python3
"""Один запуск непрозрачного официального сервера: правила 26.2/26.3.
Команды через RCON, действия игрока через существующий сетевой клиент.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import sys
import time

from probe_particles import Probe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1, HASHES, pack_position
from smoke import varint

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'measure'))
from pregen import Rcon


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--jar', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int, default=26172)
    a = p.parse_args()
    assert 26100 <= a.port < 26199
    for port in (a.port, a.port + 1):
        with socket.socket() as s:
            s.bind(('127.0.0.1', port))
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == JAR_SHA1
    for name, digest in HASHES.items():
        assert hashlib.sha256((a.data / name).read_bytes()).hexdigest() == digest, name
    a.output.mkdir(parents=True, exist_ok=False)
    root = a.output / 'server'
    root.mkdir()
    shutil.copyfile(a.jar, root / 'server.jar')
    (root / 'eula.txt').write_text('eula=true\n')
    (root / 'server.properties').write_text(
        f'server-ip=127.0.0.1\nserver-port={a.port}\nrcon.port={a.port+1}\n'
        'enable-rcon=true\nrcon.password=local-rules-probe\nonline-mode=false\n'
        'white-list=false\nenforce-whitelist=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\n'
        'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
        'spawn-protection=0\nmax-tick-time=0\n')
    report = dict(jar_sha1=JAR_SHA1, json_sha256=HASHES, commands=[], cases=[])
    client = None
    log = a.output / 'server.log'
    with log.open('w') as output:
        server = subprocess.Popen(
            ['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', 'server.jar', 'nogui'],
            cwd=root, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT)
        report['pid'] = server.pid
        try:
            deadline = time.monotonic() + 120
            while 'Done (' not in log.read_text():
                assert server.poll() is None and time.monotonic() < deadline
                time.sleep(.2)
            rcon = Rcon(a.port + 1, 'local-rules-probe')
            def cmd(command, delay=.1):
                result = rcon.run(command)
                report['commands'].append(dict(command=command, result=result))
                if client:
                    client.pump(delay)
                return result
            def check(label, command, expected):
                offset = len(log.read_text())
                result = cmd(command)
                evidence = log.read_text()[offset:]
                report['cases'].append(dict(label=label, result=result, expected=expected, log=evidence,
                                           passed=expected in result or (command.endswith('run say ' + expected) and expected in evidence)))
            protocol = json.loads((a.data / 'protocol.json').read_text())
            client = connect(a.port, protocol, lambda s, pr: Probe(s, pr, 777), 777)
            client.pump(10, lambda r: r['name'] == 'position')
            cmd('gamerule minecraft:spawn_mobs false')
            cmd('fill -8 63 -8 8 63 8 stone')
            cmd('tp McsaProbe 0.5 64 0.5 0 0')
            cmd('gamemode survival McsaProbe')
            sequence = 0
            for tool in ('iron_hoe', 'iron_shovel'):
                for shield, sneak in ((True, False), (True, True), (False, False), (False, True)):
                    sequence += 1
                    client.send('block_dig', varint(5) + pack_position((0, 0, 0)) + b'\0' + varint(sequence))
                    client.send('player_input', bytes([32 if sneak else 0]))
                    cmd('setblock 0 63 1 grass_block')
                    cmd(f'item replace entity McsaProbe weapon.mainhand with {tool}')
                    cmd('item replace entity McsaProbe weapon.offhand with ' + ('shield' if shield else 'air'))
                    sequence += 1
                    client.send('block_place', b'\0' + pack_position((0, 63, 1)) + varint(1) + struct.pack('>fff', .5, 1., .5) + b'\0\0' + varint(sequence))
                    client.pump(.3)
                    expected = 'grass_block' if shield and not sneak else ('farmland' if tool.endswith('hoe') else 'dirt_path')
                    check(f'{tool}-shield{shield}-sneak{sneak}', f'execute if block 0 63 1 {expected} run say RULE_OK', 'RULE_OK')
            client.send('player_input', b'\0')
            cmd('clear McsaProbe')
            cmd('team add rules')
            for color, accepted in [('dark_purple', True), ('darkpurple', False), ('DarkPurple', False)]:
                result = cmd('team modify rules color ' + color)
                report['cases'].append(dict(label='color-'+color, result=result, passed=('Unknown color' not in result and 'Incorrect' not in result) == accepted))
            cmd('attribute McsaProbe minecraft:knockback_resistance base set -2')
            check('knockback-min', 'attribute McsaProbe minecraft:knockback_resistance get', '-2')
            # Замороженное время даёт снимки до/после одного физического такта.
            cmd('tick freeze')
            cmd('setblock 4 64 4 red_bed')
            cmd('summon armor_stand 4.5 64.57 4.5 {Tags:["bounce"],Motion:[0d,-1d,0d]}')
            cmd('tick step 1', .3)
            check('bed-bounce', 'data get entity @e[tag=bounce,limit=1] Motion', '0.6569')
            # Проверка запрета телепортации: сплошной пол bedrock и смертельная вода.
            cmd('fill 12 63 12 28 63 28 bedrock')
            cmd('summon enderman 20.5 64 20.5 {Tags:["rider"],NoAI:1b}')
            cmd('summon oak_boat 20.5 64 20.5 {Tags:["vehicle"]}')
            cmd('ride @e[tag=rider,limit=1] mount @e[tag=vehicle,limit=1]')
            cmd('damage @e[tag=rider,limit=1] 2 minecraft:arrow')
            check('riding-enderman-projectile', 'data get entity @e[tag=rider,limit=1] Health', '38.0')
            # Снимки сторон, владения, фактических изменений и размеров.
            cmd('setblock 6 64 6 decorated_pot')
            cmd('data get block 6 64 6')
            cmd('setblock 6 64 6 decorated_pot{sherds:[{id:"minecraft:brick",components:{"minecraft:custom_name":"side"}},{},{},{}]}')
            cmd('data get block 6 64 6')
            cmd('setblock 6 64 6 air destroy')
            cmd('data get entity @e[type=item,x=6,y=64,z=6,distance=..3,limit=1]')
            cmd('summon evoker 5 64 5 {Tags:["owner"],NoAI:1b,UUID:[I;1,2,3,4]}')
            cmd('summon vex 5 65 5 {Tags:["owned"],NoAI:1b,Owner:[I;1,2,3,4]}')
            # [проверить] Имя поля владельца и доступность id в снимке сущности.
            check('vex-owner', 'execute as @e[tag=owned] on owner run data get entity @s id', 'evoker')
            for action in ('revoke', 'grant', 'grant', 'revoke', 'revoke'):
                cmd(f'advancement {action} McsaProbe only minecraft:story/root')
            report['dimensions'] = {}
            for kind in ('hoglin', 'zoglin', 'strider', 'zombie', 'piglin', 'chicken'):
                cmd('kill @e[tag=dim]')
                cmd('tag @e[tag=dim] remove dim')
                cmd(f'summon {kind} 40.5 64 40.5 {{Tags:["dim"],Age:-24000,IsBaby:1b,NoAI:1b}}')
                bounds = []
                for axis, base in ((0,40.5),(1,64.0)):
                    low, high = 0., 2.
                    for _ in range(15):
                        mid = (low + high) / 2
                        at = [40.5,64.,40.5]
                        at[axis] = base + mid
                        response = cmd('execute positioned ' + ' '.join(map(str,at)) + ' if entity @e[tag=dim,dx=0,dy=0,dz=0]', 0)
                        if 'Test passed' in response: low = mid
                        else: high = mid
                    bounds.append([low,high])
                report['dimensions'][kind] = bounds
            cmd('time set noon')
            cmd('summon zombie 7 64 7 {Tags:["sunwear"],NoAI:1b,equipment:{head:{id:"minecraft:iron_helmet",components:{"minecraft:unbreakable":{}}}}}')
            cmd('tick step 400', 21)
            check('sun-unbreakable', 'data get entity @e[tag=sunwear,limit=1] equipment.head', 'unbreakable')
            cmd('tick unfreeze')
            cmd('gamemode spectator McsaProbe')
            for kind in ('nether_portal', 'end_portal', 'end_gateway'):
                cmd('execute in minecraft:overworld run tp McsaProbe 0.5 64 0.5')
                cmd('execute in minecraft:overworld run setblock 0 64 1 ' + (kind + '{Age:201L,ExactTeleport:1b,exit_portal:[I;8,65,8]}' if kind == 'end_gateway' else kind))
                # Движение в портал после подтверждения командного телепорта.
                client.send('position', struct.pack('>dddB', .5, 64., 1.5, 0))
                client.pump(5)
                result = cmd('data get entity McsaProbe Dimension')
                report['cases'].append(dict(label='spectator-'+kind, result=result, pos=cmd('data get entity McsaProbe Pos')))
                cmd('execute in minecraft:overworld run setblock 0 64 1 air')
        except BaseException as error:
            report['error'] = repr(error)
            raise
        finally:
            if client:
                client.sock.close()
            server.send_signal(signal.SIGTERM)
            try:
                server.wait(timeout=30)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
            report['exit_code'] = server.returncode
            (a.output / 'observations.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report['cases'], ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
