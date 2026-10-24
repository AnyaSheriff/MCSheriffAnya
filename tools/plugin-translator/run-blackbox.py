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
from build_guard import run as guarded_run, wait_for_idle
import sys
import tempfile
import time
import uuid
import zipfile

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
    raise SystemExit('Сначала соберите ядро: CARGO_BUILD_JOBS=4 nice -n 19 cargo build --release')
with socket.socket() as probe:
    if probe.connect_ex(('127.0.0.1', port)) == 0:
        raise SystemExit(f'Порт {port} занят; выберите другой тестовый порт')
guarded_run([sys.executable, str(root / 'build-fixtures.py')], check=True)
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
    cache = root / 'target/blackbox-plugins'
    sdk_digest = hashlib.sha256()
    sources = [*sorted((root/'mcsa-bukkit/src/main/java').rglob('*.java')), *sorted((root/'src/main/java').rglob('*.java')), root/'adapter.c', root/'bridge.h', root/'translate.py', root/'decimal_runtime.py', root/'decimal-runtime.h', repo/'wit/mcsa-api.wit']
    for source in sources:
        sdk_digest.update(str(source.relative_to(repo)).encode());sdk_digest.update(source.read_bytes())
    for name in ('command', 'events', 'scheduler'):
        digest = sdk_digest.copy()
        jar = root / 'target' / (name + '.jar')
        with zipfile.ZipFile(jar) as archive:
            for entry in sorted(archive.namelist()):
                digest.update(entry.encode());digest.update(archive.read(entry))
        plugin_name = 'java-' + name
        stamp = cache / plugin_name / '.fixture-hash'
        if not stamp.is_file() or stamp.read_text() != digest.hexdigest() or not (cache/plugin_name/'plugin.wasm').is_file():
            with tempfile.TemporaryDirectory(prefix='fixture-cache-', dir=root/'target') as staging:
                guarded_run([sys.executable, str(root / 'translate.py'), str(jar), '--output', staging], check=True)
                cache.mkdir(exist_ok=True)
                shutil.rmtree(cache/plugin_name, ignore_errors=True)
                shutil.move(str(Path(staging)/plugin_name), str(cache/plugin_name))
            stamp.write_text(digest.hexdigest())
        shutil.copytree(cache/plugin_name, work/'plugins'/plugin_name)

    # Проверяем восстановление config.yml из встроенного ресурса saveDefaultConfig.
    config = work / 'plugins/java-scheduler/data/config.yml'
    config.unlink()
    log_path = root / 'target/blackbox-core.log'
    wait_for_idle()
    with log_path.open('w') as log:
        server = subprocess.Popen(['nice', '-n', '19', str(binary)], cwd=work, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(300):
                if server.poll() is not None:
                    raise RuntimeError('Ядро завершилось: ' + str(log_path))
                if 'Готово (' in log_path.read_text():
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError('Таймаут запуска ядра')
            guarded_run(['nice', '-n', '19', 'node', str(root / 'blackbox.cjs'), str(port)], check=True, timeout=120)
            player_pdc=list((work/'plugins/java-command/data').glob('entity-*.pdc.json'));assert len(player_pdc)==1 and 'java-command:probe' in player_pdc[0].read_text(), 'PDC сущности не сохранён в data/'
            assert (config.parent / 'queue.yml').read_text() == 'order: [first, second]', 'очередь не сохранила файл через хозяина'
            assert config.read_text() == (root / 'fixtures/scheduler/config.yml').read_text(), 'config.yml не восстановлен из JAR'
        finally:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=20)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
    # Новый процесс ядра читает те же данные, сохранённые щелчком в GUI.
    wait_for_idle()
    with log_path.open('a') as log:
        server = subprocess.Popen(['nice', '-n', '19', str(binary)], cwd=work, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(300):
                if server.poll() is not None:
                    raise RuntimeError('Ядро завершилось при перезапуске: ' + str(log_path))
                if log_path.read_text().count('Готово (') >= 2:
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError('Таймаут перезапуска ядра')
            guarded_run(['nice', '-n', '19', 'node', str(root / 'blackbox.cjs'), str(port), '--restart'], check=True, timeout=30)
        finally:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=20)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
print('PASS: изолированное ядро завершено; журнал tools/plugin-translator/target/blackbox-core.log')
