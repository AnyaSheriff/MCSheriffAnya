#!/usr/bin/env python3
"""Переходы двух Java-клиентов через настоящий сервер и переведённый Bukkit-плагин."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
BRIDGE = ROOT / 'tools/protocol-bridge'
sys.path.insert(0, str(BRIDGE))
from probe_block_entities import nbt
from probe_gameplay import Client, connect

PORT = 27864
NAMES = ('WorldProbeA', 'WorldProbeB')
CASES = (
    ('minecraft:overworld', None),
    ('minecraft:the_nether', 'ChangedWorld world world_nether'),
    ('minecraft:the_nether', None),
    ('minecraft:the_end', 'ChangedWorld world_nether world_the_end'),
    ('minecraft:overworld', 'ChangedWorld world_the_end world'),
)


class Probe(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.world_events = []

    def observe_packet(self, name, payload):
        if name == 'respawn':
            self.world_events.append('respawn')
        if name == 'system_chat' and b'ChangedWorld ' in payload:
            component = nbt(io.BytesIO(payload))
            self.world_events.append(component if isinstance(component, str) else component['text'])


def load_protocol(local):
    files = json.loads((BRIDGE / 'candidate-sources-26.3.json').read_text())['files']
    pin = next(row for row in files if row['file'] == 'protocol.json')
    if local:
        raw = local.read_bytes()
    else:
        with urllib.request.urlopen(pin['url'], timeout=30) as response:
            raw = response.read(pin['bytes'] + 1)
    digest = hashlib.sha256(raw).hexdigest()
    if len(raw) != pin['bytes'] or digest != pin['sha256']:
        raise ValueError('Неверная контрольная сумма таблицы пакетов 26.3')
    return json.loads(raw), digest


def configure(folder, fixture):
    shutil.copytree(fixture, folder / 'plugins/java-bossbar')
    config = folder / 'config'
    config.mkdir()
    (folder / 'eula.txt').write_text('eula=true\n')
    (config / 'mcsa.properties').write_text(
        'java-version=26.3\nsend-error-reports=false\nauth-enabled=false\n'
        'auth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    (config / 'skins.toml').write_text('mojang=false\nely=false\n')
    (config / 'server.properties').write_text(
        f'server-ip=127.0.0.1\nserver-port={PORT}\nonline-mode=false\nenforce-secure-profile=false\n'
        'network-compression-threshold=-1\nlevel-type=minecraft:flat\ngenerate-structures=false\n'
        'view-distance=2\nsimulation-distance=2\nspawn-protection=0\nmax-players=2\n')


def stop(process):
    if process.poll() is not None:
        return
    try:
        process.stdin.write('stop\n')
        process.stdin.flush()
        process.wait(timeout=12)
    except (BrokenPipeError, subprocess.TimeoutExpired):
        if process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=4)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=4)


def verify(args):
    protocol, protocol_sha = load_protocol(args.protocol)
    build = json.loads(args.build_receipt.read_text())
    binary = args.binary.resolve()
    data = binary.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if digest != build['sha256'] or head != build['source'] or len(data) != build['bytes']:
        raise ValueError('Сборка ядра не соответствует текущему коммиту')
    args.output.mkdir(parents=True, exist_ok=True)
    log = args.output / 'world-change-server.log'
    record = {'source': head, 'binary_sha256': digest, 'protocol_sha256': protocol_sha,
              'plugin_sha256': hashlib.sha256((args.fixture / 'plugin.wasm').read_bytes()).hexdigest(), 'cases': []}
    with tempfile.TemporaryDirectory(prefix='mcsa-world-change-') as temp:
        folder = Path(temp)
        configure(folder, args.fixture)
        with socket.socket() as check:
            check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            check.bind(('127.0.0.1', PORT))
        with log.open('w') as output:
            process = subprocess.Popen([str(binary)], cwd=folder, stdin=subprocess.PIPE,
                                       stdout=output, stderr=subprocess.STDOUT, text=True)
            clients = []
            try:
                deadline = time.monotonic() + 35
                while 'Чтобы узнать команды' not in (contents := log.read_text()):
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f'Сервер не запустился: {log}')
                    time.sleep(0.1)
                if 'Плагин загружен: java-bossbar' not in contents:
                    raise RuntimeError(f'Тестовый плагин не загружен: {log}')
                for name in NAMES:
                    client = connect(PORT, protocol, Probe, username=name)
                    clients.append(client)
                    client.pump(8, lambda row: row['name'] == 'position')
                process.stdin.write(f'gamemode creative {NAMES[0]}\n')
                with ThreadPoolExecutor(2) as pool:
                    for dimension, event in CASES:
                        starts = [len(client.world_events) for client in clients]
                        process.stdin.write(f'execute in {dimension} run tp {NAMES[0]} 0.5 100.0 0.5\n')
                        process.stdin.flush()
                        list(pool.map(lambda client: client.pump(3.5 if event else 1.5), clients))
                        observed = [client.world_events[start:] for client, start in zip(clients, starts)]
                        expected = [['respawn', event] if event else [], []]
                        record['cases'].append({'dimension': dimension, 'expected': expected, 'observed': observed})
                        if observed != expected:
                            raise AssertionError(record['cases'][-1])
            finally:
                for client in clients:
                    client.sock.close()
                stop(process)
                record['server_exit'] = process.returncode
                record['saved'] = 'Мир сохранён' in log.read_text()
                (args.output / 'world-change-protocol.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')
    if record['server_exit'] != 0 or not record['saved']:
        raise RuntimeError(f'Тестовый сервер не сохранился: {log}')
    print('Java 26.3: три перехода, два переноса в том же мире, второй игрок без событий')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'build-receipt', 'fixture', 'output'):
        parser.add_argument('--'+name, type=Path, required=True)
    parser.add_argument('--protocol', type=Path)
    verify(parser.parse_args())


if __name__ == '__main__':
    main()
