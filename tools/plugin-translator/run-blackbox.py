#!/usr/bin/env python3
# Собственный процесс ядра, собственные настройки и свободный тестовый порт.
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import uuid

root = Path(__file__).resolve().parent
repo = root.parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=26210)
args = parser.parse_args()
if not 26200 <= args.port <= 26299:
    raise SystemExit('Тестовый порт: 26200..26299')
port = args.port
binary = repo / 'target/release/mcsheriffanya'
if not binary.is_file():
    raise SystemExit('Сначала соберите ядро: CARGO_BUILD_JOBS=6 cargo build --release')
with socket.socket() as probe:
    if probe.connect_ex(('127.0.0.1', port)) == 0:
        raise SystemExit(f'Порт {port} занят; выберите другой тестовый порт')
subprocess.run([sys.executable, str(root / 'build-fixtures.py')], check=True)
(repo / 'tmp').mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='java-blackbox-', dir=repo / 'tmp') as temp:
    work = Path(temp)
    (work / 'config').mkdir()
    (work / 'config/server.properties').write_text(f'server-port={port}\nonline-mode=false\nlevel-type=minecraft\\:flat\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\n')
    (work / 'config/mcsa.properties').write_text('auth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\nconsole-colors=never\nsend-error-reports=false\n')
    identity = bytearray(hashlib.md5(b'OfflinePlayer:JavaTester').digest())
    identity[6] = (identity[6] & 15) | 48
    identity[8] = (identity[8] & 63) | 128
    (work / 'ops.json').write_text(json.dumps([{'uuid': str(uuid.UUID(bytes=bytes(identity))), 'name': 'JavaTester', 'level': 4}]))
    for name in ('command', 'events', 'scheduler'):
        subprocess.run([sys.executable, str(root / 'translate.py'), str(root / 'target' / (name + '.jar')), '--output', str(work / 'plugins')], check=True)
    # Проверяем восстановление config.yml из встроенного ресурса saveDefaultConfig.
    config = work / 'plugins/java-scheduler/data/config.yml'
    config.unlink()
    log_path = root / 'target/blackbox-core.log'
    with log_path.open('w') as log:
        server = subprocess.Popen([str(binary)], cwd=work, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(300):
                if server.poll() is not None:
                    raise RuntimeError('Ядро завершилось: ' + str(log_path))
                if 'Готово (' in log_path.read_text():
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError('Таймаут запуска ядра')
            subprocess.run(['node', str(root / 'blackbox.cjs'), str(port)], check=True, timeout=60)
            assert config.read_text() == (root / 'fixtures/scheduler/config.yml').read_text(), 'config.yml не восстановлен из JAR'
        finally:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=20)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
print('PASS: изолированное ядро завершено; журнал tools/plugin-translator/target/blackbox-core.log')
