#!/usr/bin/env python3
"""Только наблюдение vanilla: снятие forceload не доказывает выгрузку."""
import argparse
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import time

import combat_ci as common
from combat_process import ports_free, process_identity, stop_process

PORTS = (26154, 26155)
SELECTOR = '@e[type=minecraft:tnt,limit=1]'


def game_time(answer):
    values = re.findall(r'(?:The game time is (\d+) tick\(s\)|The time is (\d+))(?=\s|$)', answer)
    values = [new or old for new, old in values]
    if len(values) != 1:
        raise ValueError('missing or ambiguous gametime acknowledgment')
    return int(values[0])


def entity_state(answer):
    present = 'has the following entity data:' in answer
    absent = 'No entity was found' in answer
    if present and not absent:
        entity_fields(answer)
        return 'observable'
    if absent and not present:
        return 'not-observable'
    raise ValueError('unknown entity-data response')


def entity_fields(answer):
    # Только явно названные поля наблюдения, не общий parser SNBT.
    def unique(pattern):
        matches = re.findall(pattern, answer)
        if len(matches) != 1:
            raise ValueError('missing or ambiguous TNT data field')
        return matches[0]
    fuse = int(unique(r'(?<![\w])(?:fuse|Fuse):\s*(-?\d+)(?:[sSbB])?(?=\s*[,}])'))
    uuid = [int(v.strip()) for v in unique(r'(?<![\w])UUID:\s*\[I;([^\]]+)\]').split(',')]
    if len(uuid) != 4 or any(not -(2**31) <= v < 2**31 for v in uuid):
        raise ValueError('invalid UUID')
    position = [float(v.strip().rstrip('dDfF')) for v in unique(r'(?<![\w])Pos:\s*\[([^\]]+)\]').split(',')]
    if len(position) != 3 or not all(math.isfinite(v) for v in position):
        raise ValueError('invalid position')
    if not 0 <= fuse <= 80:
        raise ValueError('invalid natural TNT fuse')
    return {'uuid': uuid, 'fuse': fuse, 'position': position}


def validate(receipt):
    rows = receipt['observations']
    if [row['label'] for row in rows] != ['primed-frozen', 'after20', 'after100', 'after200', 'reheld-after220']:
        raise ValueError('incomplete observation sequence')
    if [row['gametime'] - rows[0]['gametime'] for row in rows] != [0, 20, 100, 200, 220]:
        raise ValueError('unexpected observation times')
    if '[Server] MCSA_WITNESS_DIRT' not in rows[0]['witness']:
        raise ValueError('initial block witness missing')
    if rows[0].get('fields', {}).get('fuse') != 80:
        raise ValueError('initial natural TNT fuse must be 80')
    if receipt['case'] == 'held':
        if (rows[1]['entity'] != 'observable' or rows[1]['fields']['uuid'] != rows[0]['fields']['uuid']
                or not 0 < rows[1]['fields']['fuse'] < rows[0]['fields']['fuse']):
            raise ValueError('held control did not advance the same TNT')
    if rows[0]['entity'] != 'observable':
        raise ValueError('primed TNT not observed')
    if receipt['case'] == 'held' and receipt['observations'][-1]['entity'] != 'not-observable':
        raise ValueError('held control TNT still observable after 220 ticks')
    if '[Server] MCSA_COORDINATE_READABLE' not in receipt.get('loaded_witness', ''):
        raise ValueError('coordinate readability not established')
    if receipt['cleanup']['exit_code'] != 0 or receipt['cleanup']['method'] != 'stop-and-eof' or not receipt['cleanup']['group_gone']:
        raise ValueError('unclean shutdown')
    if receipt['ports_free'] != {str(receipt['port']): True}:
        raise ValueError('owned port remains occupied')
    return 'observation-complete-not-parity'


def experiment(java, jar, output, case, port):
    folder = output / case
    folder.mkdir()
    properties = ('server-ip=127.0.0.1\nserver-port=' + str(port) + '\nonline-mode=false\nenable-rcon=false\n'
                  'level-name=world\nlevel-type=minecraft:flat\n'
                  'generator-settings={"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":3}],"biome":"minecraft:plains"}\n'
                  'generate-structures=false\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\npause-when-empty-seconds=0\n')
    (folder / 'server.properties').write_text(properties)
    (folder / 'server.properties.before').write_text(properties)
    (folder / 'eula.txt').write_text('eula=true\n')
    receipt = {'case': case, 'scope': 'vanilla observation; no unload or explosion inference from entity absence',
               'commands': [], 'observations': [], 'port': port, 'status': 'infrastructure-failed'}
    receipt['config_sha256_before'] = common.sha(folder / 'server.properties')
    proc = identity = None
    logpath = folder / 'console.log'
    deadline = time.monotonic() + 110

    def read():
        if time.monotonic() > deadline:
            raise TimeoutError('case 110s deadline')
        if logpath.stat().st_size > 4 * 1024 * 1024:
            raise ValueError('log bound exceeded')
        return logpath.read_text(errors='replace')

    def wait_for(needle, offset):
        until = min(deadline, time.monotonic() + 35)
        while time.monotonic() < until:
            answer = read()[offset:]
            if needle in answer:
                return answer
            if proc.poll() is not None:
                raise RuntimeError('server exited before acknowledgment')
            time.sleep(.025)
        raise TimeoutError('missing acknowledgment: ' + needle)

    def command(text):
        offset = len(read())
        marker = 'MCSA_BARRIER_' + str(len(receipt['commands']))
        item = {'command': text, 'offset': offset, 'marker': marker}
        receipt['commands'].append(item)
        proc.stdin.write(text + '\nsay ' + marker + '\n')
        proc.stdin.flush()
        answer = wait_for('[Server] ' + marker, offset)
        item['response'] = answer
        return answer

    def forceload(action):
        answer = command('forceload ' + action + ' 2048 2048')
        expected = {'add': 'Marked chunk [128, 128]', 'remove': 'Unmarked chunk [128, 128]',
                    'query': '[128, 128]'}[action]
        if expected not in answer:
            raise ValueError('forceload acknowledgment missing: ' + action)
        if action == 'query':
            marked = 'is marked for force loading' if case == 'held' else 'is not marked for force loading'
            if marked not in answer:
                raise ValueError('wrong forceload query state')
        return answer

    def now():
        return game_time(command('time query gametime'))

    def steps(count):
        start = now()
        answer = command('tick step ' + str(count))
        if 'Stepping' not in answer:
            raise ValueError('tick step not acknowledged')
        until = time.monotonic() + 25
        while time.monotonic() < until:
            current = now()
            if current == start + count:
                return current
            if current < start or current > start + count:
                raise ValueError('unexpected gametime advancement')
            time.sleep(.05)
        raise TimeoutError('tick step did not finish')

    def observe(label):
        answer = command('data get entity ' + SELECTOR)
        item = {'label': label, 'gametime': now(), 'entity': entity_state(answer), 'data_response': answer}
        if item['entity'] == 'observable':
            item['fields'] = entity_fields(answer)
        item['witness'] = command('execute if block 2057 5 2056 minecraft:dirt run say MCSA_WITNESS_DIRT')
        item['air'] = command('execute if block 2057 5 2056 minecraft:air run say MCSA_WITNESS_AIR')
        receipt['observations'].append(item)

    try:
        if not all(ports_free([port]).values()):
            raise ValueError('port occupied before launch')
        with logpath.open('w') as log:
            env = {k: v for k, v in os.environ.items() if k not in ('GH_TOKEN', 'GITHUB_TOKEN')}
            proc = subprocess.Popen([java, '-Xms256M', '-Xmx1G', '-jar', str(jar), 'nogui'],
                                    cwd=folder, stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT,
                                    text=True, start_new_session=True, env=env)
            identity = process_identity(proc.pid)
            receipt['process_identity'] = identity
            wait_for('Done (', 0)
            if 'The game is frozen' not in command('tick freeze'):
                raise ValueError('freeze not acknowledged')
            forceload('add')
            steps(20)
            command('fill 2048 4 2048 2063 4 2063 minecraft:stone')
            command('setblock 2057 5 2056 minecraft:dirt')
            command('setblock 2056 5 2056 minecraft:tnt')
            command('setblock 2056 5 2057 minecraft:redstone_block')
            observe('primed-frozen')
            if case == 'released':
                forceload('remove')
            receipt['forceload_query'] = forceload('query')
            for count, label in ((20, 'after20'), (80, 'after100'), (100, 'after200')):
                steps(count)
                observe(label)
            if case == 'released':
                forceload('add')
            steps(20)
            observe('reheld-after220')
            # Положительная проверка доступности координаты после возврата удержания.
            receipt['loaded_witness'] = command('execute if block 2048 4 2048 minecraft:stone run say MCSA_COORDINATE_READABLE')
            if '[Server] MCSA_COORDINATE_READABLE' not in receipt['loaded_witness']:
                raise ValueError('coordinate readability not established')
    except Exception as error:
        receipt['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        try:
            if proc is not None:
                receipt['cleanup'] = stop_process(proc, identity=identity)
            receipt['config_sha256_after'] = common.sha(folder / 'server.properties')
            receipt['ports_free'] = ports_free([port])
            if 'error' not in receipt:
                receipt['status'] = validate(receipt)
        except Exception as error:
            receipt['cleanup_error'] = str(error)
        common.write_json(folder / 'receipt.json', receipt)
    if receipt['status'] != 'observation-complete-not-parity':
        raise RuntimeError('case failed: ' + case)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--java', default='java')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('GitHub-only; no local Minecraft execution')
    args.output.mkdir(parents=True, exist_ok=False)
    receipt = {'status': 'infrastructure-failed', 'cases': []}
    def expired(sig, frame):
        raise TimeoutError('total 280s deadline')
    for sig in (signal.SIGALRM, signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, expired)
    signal.alarm(280)
    try:
        receipt['java_version'] = subprocess.check_output([args.java, '-version'], stderr=subprocess.STDOUT, text=True, timeout=10)
        if not re.search(r'version "25(?:[.\"]|$)', receipt['java_version']):
            raise ValueError('Java25 required')
        pin = json.loads(common.SOURCES.read_text())['official_jar']
        jar = args.output / 'server.jar'
        common.fetch(pin, jar)
        receipt['actual_jar'] = {'sha256': common.sha(jar), 'bytes': jar.stat().st_size}
        receipt['source_sha256'] = {name: common.sha(common.ROOT / name) for name in (
            'tools/blackbox/tnt_vanilla_lifecycle_ci.py', 'tools/blackbox/test_tnt_vanilla_lifecycle_ci.py',
            'tools/blackbox/combat_ci.py', 'tools/blackbox/combat_process.py',
            'tools/blackbox/ci/sources-777.json', '.github/workflows/tnt-vanilla-lifecycle.yml')}
        receipt['official_jar_pin'] = pin
        receipt['source_head'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=common.ROOT, text=True).strip()
        for case, port in zip(('held', 'released'), PORTS):
            receipt['cases'].append(experiment(args.java, jar.resolve(), args.output, case, port))
        receipt['status'] = 'observation-complete-not-parity'
    except Exception as error:
        receipt['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        common.write_json(args.output / 'receipt.json', receipt)
    if receipt['status'] != 'observation-complete-not-parity':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
