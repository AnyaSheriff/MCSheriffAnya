# Общий шлюз сборок: не мешаем параллельному чёрному ящику.
from pathlib import Path
import os
import subprocess
import time

_raw_run = subprocess.run

def wait_for_idle():
    while True:
        cargo = _raw_run(['pgrep', '-x', 'cargo'], capture_output=True).returncode == 0
        processes = _raw_run(['ps', '-eo', 'args'], capture_output=True, text=True).stdout
        if not cargo and not any('compare.js' in line for line in processes.splitlines()):
            break
        print('Ожидание cargo / compare.js перед сборкой...', flush=True)
        time.sleep(30)

def run(args, **kwargs):
    wait_for_idle()
    env = dict(kwargs.pop('env', os.environ))
    env['CARGO_BUILD_JOBS'] = '4'
    command = list(map(str, args))
    # Приоритет сборки не наследуется живым ядром/ботом из Python-обвязки.
    build_wrapper = len(command) > 1 and Path(command[1]).name in {'translate.py', 'build-fixtures.py', 'test-inspector.py'}
    if build_wrapper or Path(command[0]).name in {'cargo', 'mvn', 'java', 'javac', 'jar', 'clang', 'wit-bindgen', 'wasm-tools'}:
        command = ['nice', '-n', '19', *command]
    return _raw_run(command, env=env, **kwargs)
