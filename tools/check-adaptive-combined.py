#!/usr/bin/env python3
"""Только CI: совместное включение двух режимов, freeze/step и сохранность миров.

Это проверка запуска и управления. Нагрузка >50 мс намеренно не создаётся;
запуск не доказывает фактическую работу помощников или ускорение.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time
import traceback

SCOPE = ('Combined opt-in configuration/startup/reopen/freeze/step/save/cleanup only. '
         'No forced slow tick, helper activation, speedup or full gameplay claim.')
DIMENSIONS = {'nether': 'the_nether', 'end': 'the_end'}


def sha(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def tree(path):
    return {str(p.relative_to(path)): None if p.is_dir() else sha(p)
            for p in sorted(path.rglob('*'))} if path.exists() else None


def port_free(port):
    with socket.socket() as sock:
        try:
            sock.bind(('127.0.0.1', port))
            return True
        except OSError:
            return False


def time_value(text):
    values = re.findall(r'Время: (\d+)', text)
    if len(values) != 1:
        raise ValueError('ожидался один числовой ответ time query: ' + repr(text[-500:]))
    return int(values[0])


def read_settings(path):
    return dict(line.split('=', 1) for raw in path.read_text().splitlines()
                if (line := raw.strip()) and not line.startswith('#') and '=' in line)


def exercise(binary, folder, port, phase):
    result = dict(phase=phase, commands=[], checks={}, forced=False, scope=SCOPE)
    process = None
    logpath = folder / (phase + '.log')
    marker = 0

    def text():
        return logpath.read_text(errors='replace')

    def command(value):
        if process.poll() is not None:
            raise RuntimeError('server exited before command: ' + value)
        result['commands'].append(value)
        process.stdin.write(value + '\n')
        process.stdin.flush()

    def barrier():
        nonlocal marker
        marker += 1
        token = f'MCSA_COMBINED_{phase}_{marker:04d}'
        command('say ' + token)
        end = time.monotonic() + 15
        while token not in text():
            if process.poll() is not None or time.monotonic() >= end:
                raise RuntimeError('console barrier not observed: ' + token)
            time.sleep(.05)

    def query():
        begin = len(text())
        command('time query')
        barrier()
        return time_value(text()[begin:])

    try:
        if not port_free(port):
            raise RuntimeError('own isolated port is occupied')
        # Не наследуем настройки API приложения и прочие переменные MCSA.
        env = {k: v for k, v in os.environ.items() if not k.startswith('MCSA_')}
        with logpath.open('w') as log:
            process = subprocess.Popen([str(binary)], cwd=folder, env=env, stdin=subprocess.PIPE,
                                       stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
        deadline = time.monotonic() + 120
        while 'Готово (' not in text():
            if process.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError('startup failed or timed out')
            time.sleep(.1)
        result['checks']['ready'] = True
        result['checks']['both_modes_logged'] = ('Экспериментальный поиск угроз и целей мобов включён' in text()
                                                 and 'Параллельная блочная часть измерений включена' in text())
        spawn_log_start = len(text())
        command('gamerule spawn_mobs false')
        barrier()
        result['checks']['spawn_mobs_gamerule_acknowledged'] = (
            'Правило spawn_mobs: false' in text()[spawn_log_start:])
        command('tick freeze')
        command('time set 1200')
        barrier()
        first = query()
        time.sleep(.25)
        second = query()
        result['checks']['freeze_keeps_world_time'] = first == second == 1200
        command('tick step 5')
        barrier()
        # say подтверждает обработку команд, но не завершение пяти будущих тиков.
        # Опрос time query в замороженном мире не добавляет логических шагов.
        deadline = time.monotonic() + 15
        while True:
            stepped = query()
            if stepped >= 1205:
                break
            if time.monotonic() >= deadline:
                raise TimeoutError('five requested steps did not finish')
            time.sleep(.1)
        result['checks']['five_steps_exact'] = stepped == 1205
        time.sleep(.25)
        result['checks']['step_returns_to_freeze'] = query() == 1205
        command('tick unfreeze')
        barrier()
        deadline = time.monotonic() + 10
        while True:
            resumed = query()
            if resumed > 1205:
                break
            if time.monotonic() >= deadline:
                raise TimeoutError('world did not resume after unfreeze')
            time.sleep(.1)
        command('tick freeze')
        barrier()
        frozen = query()
        time.sleep(.25)
        result['checks']['freeze_after_resume'] = query() == frozen and frozen >= resumed
        command('save-all')
        barrier()
        result['times'] = dict(initial=first, frozen=second, stepped=stepped, resumed=resumed, frozen_again=frozen)
        # Только наблюдаемые сообщения. Их отсутствие не считается ошибкой:
        # эта постановка не обещает нагрузку, пригодную для активации помощников.
        result['worker_log_lines'] = [line for line in text().splitlines()
                                      if 'исполнителей ' in line or 'помощник' in line]
        result['helper_activity_proved'] = False
    except Exception as error:
        result['error'] = repr(error)
        result['traceback'] = traceback.format_exc()
    finally:
        if process:
            try:
                if process.poll() is None:
                    command('stop')
                    process.wait(timeout=45)
            except Exception as error:
                result['stop_error'] = repr(error)
                result['forced'] = True
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=10)
            result['exit_code'] = process.returncode
            try:
                os.killpg(process.pid, 0)
                result['group_gone'] = False
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                result['group_gone'] = True
        result['port_free'] = port_free(port)
        result['passed'] = (not result.get('error') and bool(result['checks']) and all(result['checks'].values())
                            and result.get('exit_code') == 0 and result.get('group_gone')
                            and result['port_free'] and not result['forced'])
    return result


def run_case(binary, output, index, nether, end, dimension_cap, mob_cap):
    folder = output / f'case-{index}'
    folder.mkdir()
    config = folder / 'config'
    config.mkdir()
    expected = dict(**{'java-version': '26.3', 'send-error-reports': 'false', 'auth-enabled': 'false',
                       'auth-premium': 'false', 'offline-license-mode': 'disabled', 'bedrock-port': '0',
                       'prefetch-chunks': '0', 'parallel-dimension-ticks': 'true',
                       'experimental-parallel-mobs': 'true', 'dimension-tick-threads': str(dimension_cap),
                       'mob-tick-threads': str(mob_cap), 'enable-nether': str(nether).lower(),
                       'enable-end': str(end).lower()})
    (config / 'mcsa.properties').write_text(''.join(f'{k}={v}\n' for k, v in expected.items()))
    port = 27780 + index
    (config / 'server.properties').write_text(
        f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\n'
        'enforce-secure-profile=false\nlevel-type=minecraft:flat\nlevel-seed=1\n'
        'view-distance=2\nsimulation-distance=2\nspawn-protection=0\n'
        'pause-when-empty-seconds=-1\n')
    result = dict(index=index, port=port, expected=expected, phases=[], checks={}, scope=SCOPE)
    base = folder / 'world/dimensions/minecraft'
    enabled = dict(nether=nether, end=end)
    result['phases'].append(exercise(binary, folder, port, 'fresh'))
    result['checks']['overworld_created'] = (folder / 'world/region').is_dir()
    for name, dirname in DIMENSIONS.items():
        target = base / dirname
        result['checks'][name + '_fresh_presence'] = target.is_dir() == enabled[name]
        if not enabled[name]:
            target.mkdir(parents=True, exist_ok=True)
            (target / 'blocks.mcrw').write_bytes(b'own disabled dimension sentinel: must not parse or change\n')
            (target / 'sentinel').write_bytes(b'keep this own test file\n')
    before = {name: tree(base / dirname) for name, dirname in DIMENSIONS.items() if not enabled[name]}
    result['phases'].append(exercise(binary, folder, port, 'reopen'))
    after = {name: tree(base / DIMENSIONS[name]) for name in before}
    result['checks']['disabled_sentinels_unchanged'] = before == after
    actual = read_settings(config / 'mcsa.properties')
    result['checks']['combined_configuration_persisted'] = all(actual.get(k) == v for k, v in expected.items())
    result['disabled_before'] = before
    result['disabled_after'] = after
    result['passed'] = all(p['passed'] for p in result['phases']) and all(result['checks'].values())
    (folder / 'observations.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('GitHub-only: local server execution forbidden')
    binary = args.binary.resolve(strict=True)
    if not binary.is_file():
        parser.error('ordinary native binary required')
    args.output.mkdir(parents=True, exist_ok=False)
    cases = []
    for nether, end in ((False, False), (False, True), (True, False), (True, True)):
        for block_cap, mob_cap in ((1, 1), (3, 8)):
            index = len(cases)
            try:
                cases.append(run_case(binary, args.output.resolve(), index, nether, end, block_cap, mob_cap))
            except Exception as error:
                cases.append(dict(index=index, passed=False, error=repr(error), traceback=traceback.format_exc()))
    summary = dict(source=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                   binary_sha256=sha(binary), scope=SCOPE, cases=cases,
                   helper_activity_proved=False, passed=len(cases) == 8 and all(c['passed'] for c in cases))
    (args.output / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n')
    if not summary['passed']:
        raise SystemExit('combined runtime checks failed; raw evidence retained')


if __name__ == '__main__':
    main()
