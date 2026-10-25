#!/usr/bin/env python3
"""Обычный blackbox с симметричным внешним ViaProxy; JAR остаётся непрозрачным."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
JAVA = '/usr/lib/jvm/java-25-openjdk/bin/java'
COUNTS = re.compile(r'Совпало: (\d+).*?Сдвинуто по тактам: (\d+).*?Нет у нас: (\d+).*?Лишнее у нас: (\d+)')


class WireTap:
    """Прозрачная запись сетевого трафика для проверки внешнего перевода."""
    def __init__(self, port, target, path):
        self.listener = socket.socket()
        self.listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.listener.bind(('127.0.0.1', port))
        self.listener.listen()
        self.target, self.path = target, path
        self.counter = 0
        threading.Thread(target=self.serve, daemon=True).start()

    def serve(self):
        while True:
            try:
                client, _ = self.listener.accept()
            except OSError:
                return
            backend = socket.create_connection(('127.0.0.1', self.target))
            self.counter += 1
            for source, dest, save in ((client, backend, True), (backend, client, False)):
                threading.Thread(target=self.pump, args=(source, dest, save, self.counter), daemon=True).start()

    def pump(self, source, dest, save, number):
        suffix = '' if save else '-down'
        log = self.path.with_name(self.path.name + suffix + f'-{number}.bin').open('wb')
        try:
            while data := source.recv(65536):
                if log:
                    log.write(data)
                    log.flush()
                dest.sendall(data)
        except OSError:
            pass
        finally:
            if log:
                log.close()
            source.close()
            dest.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--proxy', type=Path)
    parser.add_argument('--primary', choices=('26.3', '26.1.2'), default='26.3')
    parser.add_argument('--port', type=int, default=26170)
    parser.add_argument('--ours-port', type=int, default=26270)
    parser.add_argument('--wire', action='store_true')
    parser.add_argument('--streaming-observation', action='store_true')
    parser.add_argument('--debug', action='store_true')
    parser.add_argument('--repeat', type=int, default=1)
    parser.add_argument('--names', type=Path)
    parser.add_argument('--official-recordings', type=Path)
    parser.add_argument('--reference-wait', type=float, default=0)
    parser.add_argument('scenarios', nargs='*')
    args = parser.parse_args()
    assert 26100 <= args.port <= 26194
    assert 26200 <= args.ours_port <= 26296
    assert args.repeat > 0
    assert 0 <= args.reference_wait <= 600
    assert args.primary != '26.3' or args.proxy, '26.3 requires symmetric translation'
    names = args.scenarios or json.loads(args.names.read_text())
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    manifest = {'primary': args.primary, 'scenarios': names, 'repeat': args.repeat,
                'port': args.port, 'ours_port': args.ours_port, 'translation': bool(args.proxy),
                'official_recordings': str(args.official_recordings.resolve()) if args.official_recordings else None}
    for key in ('binary', 'jar', 'proxy'):
        path = getattr(args, key)
        if path:
            manifest[key] = {'path': str(path.resolve()), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    manifest['scripts'] = {n: hashlib.sha256((ROOT / 'tools/blackbox' / n).read_bytes()).hexdigest()
                           for n in ('bot.js', 'compare.js', 'via_ticks.js', '../protocol-bridge/probe_gameplay.py')}
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    children, logs, rows, taps = [], [], [], []

    def start(argv, folder, port):
        log = (folder / 'server.log').open('w')
        logs.append(log)
        child = subprocess.Popen(argv, cwd=folder, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        children.append(child)
        (folder / 'pid').write_text(str(child.pid))
        deadline = time.monotonic() + 150
        while time.monotonic() < deadline:
            if child.poll() is not None:
                raise RuntimeError(f'{folder.name} exited: ' + (folder / 'server.log').read_text()[-1800:])
            try:
                with socket.create_connection(('127.0.0.1', port), timeout=.3):
                    return
            except OSError:
                time.sleep(.3)
        raise TimeoutError(str(folder))

    try:
        for p in (args.port, args.port + 1, args.port + 4, args.ours_port, args.ours_port + 1, args.ours_port + 4):
            with socket.socket() as check:
                check.bind(('127.0.0.1', p))
        sides = [('ours', 2)] if args.official_recordings else [('official', 0), ('ours', 2)]
        for label, offset in sides:
            port = args.port if label == 'official' else args.ours_port
            folder = out / label
            folder.mkdir()
            cfg = folder if label == 'official' else folder / 'config'
            cfg.mkdir(exist_ok=True)
            (cfg / 'server.properties').write_text(
                f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\n'
                'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n'
                'generator-settings={"layers":[{"block":"minecraft:bedrock","height":1},'
                '{"block":"minecraft:stone","height":3}],"biome":"minecraft:plains"}\n'
                f'generate-structures=false\nwhite-list=false\nlevel-seed=1\nview-distance={10 if args.streaming_observation else 4}\n'
                'simulation-distance=4\nspawn-protection=0\nallow-flight=true\ngamemode=creative\nmax-tick-time=0\n')
            ops = []
            for name in ('blackbox', 'blackboxb', 'StreamProbe', 'TabProbe'):
                raw = bytearray(hashlib.md5(('OfflinePlayer:' + name).encode()).digest())
                raw[6] = (raw[6] & 15) | 48
                raw[8] = (raw[8] & 63) | 128
                ops.append({'uuid': str(uuid.UUID(bytes=bytes(raw))), 'name': name, 'level': 4})
            (folder / 'ops.json').write_text(json.dumps(ops))
            if label == 'official':
                shutil.copyfile(args.jar, folder / 'server.jar')
                (folder / 'eula.txt').write_text('eula=true\n')
                argv = [JAVA, '-Xms128M', '-Xmx768M', '-jar', 'server.jar', 'nogui']
            else:
                (cfg / 'mcsa.properties').write_text(
                    f'java-version={args.primary}\nsend-error-reports=false\nauth-enabled=false\n'
                    'auth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n' + ('debug=true\n' if args.debug else ''))
                (cfg / 'skins.toml').write_text('mojang=false\nely=false\n')
                argv = [str(args.binary.resolve())]
            start(argv, folder, port)
            if args.proxy:
                target = port
                if args.wire:
                    target = port + 4
                    taps.append(WireTap(target, port, out / (label + '-wire')))
                folder = out / (label + '-proxy')
                folder.mkdir()
                # Отключаем неиспользуемый HTTP-служебный порт Bedrock до запуска.
                (folder / 'viabedrock.yml').write_text('translate-resource-packs: false\n')
                start([JAVA, '-Xmx512M', '-jar', str(args.proxy.resolve()), 'cli',
                       '--bind-address', f'127.0.0.1:{port + 1}', '--target-address', f'127.0.0.1:{target}',
                       '--target-version', args.primary, '--compression-threshold', '-1'], folder, port + 1)
        if args.streaming_observation:
            assert not args.official_recordings
            observation = out / 'streaming'
            subprocess.run(['python3', str(ROOT / 'tools/protocol-bridge/probe_gameplay.py'),
                '--review-streaming', '--binary', str(args.binary.resolve()), '--jar', str(args.jar.resolve()),
                '--data', '/home/lenox/.local/share/mcsa-java-26.3-probe/candidate-json',
                '--output', str(observation), '--official-port', str(args.port),
                '--ours-port', str(args.ours_port), '--attach'], cwd=ROOT, check=False)
        for repeat in range(args.repeat):
            for name in names:
                captures = {}
                if args.official_recordings:
                    reference = args.official_recordings / f'{name}-0-official.json'
                    if not reference.is_file():
                        previous = args.official_recordings / f'vanilla-{name}.json'
                        if previous.is_file(): reference = previous
                    # Повтор может идти следом за ещё работающим оригинальным захватом.
                    deadline = time.monotonic() + args.reference_wait
                    while not reference.is_file() and time.monotonic() < deadline:
                        time.sleep(.25)
                    assert reference.is_file(), str(reference)
                    captures['official'] = reference.resolve()
                def record(label, offset):
                    dest = out / f'{name}-{repeat}-{label}.json'
                    logpath = out / f'{name}-{repeat}-{label}.log'
                    argv = ['node']
                    if args.proxy:
                        argv += ['--require', str(ROOT / 'tools/blackbox/via_ticks.js')]
                    argv += [str(ROOT / 'tools/blackbox/bot.js'), '127.0.0.1',
                            str((args.port if label == 'official' else args.ours_port) + bool(args.proxy)),
                            str(ROOT / f'tools/blackbox/scenarios/{name}.json'), str(dest), '1']
                    with logpath.open('w') as log:
                        try:
                            environment = dict(os.environ)
                            if args.streaming_observation: environment['MCSA_BB_VIEW_DISTANCE'] = 'short'
                            result = subprocess.run(argv, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=240, env=environment)
                            code = result.returncode
                        except subprocess.TimeoutExpired:
                            code = 124
                    return label, dest, code
                with ThreadPoolExecutor(max_workers=2) as pool:
                    results = list(pool.map(lambda side: record(*side), sides))
                row = {'scenario': name, 'repeat': repeat, 'exits': {label: code for label, _, code in results},
                       'status': 'capture-open'}
                if args.official_recordings:
                    row['official_sha256'] = hashlib.sha256(captures['official'].read_bytes()).hexdigest()
                for label, dest, _ in results:
                    captures[label] = dest
                if all(code == 0 for _, _, code in results):
                    result = subprocess.run(['node', str(ROOT / 'tools/blackbox/compare.js'), name, '--files',
                                             str(captures['official']), str(captures['ours'])],
                                            cwd=ROOT, text=True, capture_output=True, timeout=30)
                    (out / f'{name}-{repeat}-compare.log').write_text(result.stdout + result.stderr)
                    match = COUNTS.search(result.stdout)
                    row['counts'] = list(map(int, match.groups())) if match else None
                    row['warnings'] = [line for line in result.stdout.splitlines() if line.startswith('Внимание:')]
                    row['status'] = 'match' if row['counts'] is not None and not any(row['counts'][1:]) and not row['warnings'] else 'mismatch-open'
                rows.append(row)
                (out / 'summary.json').write_text(json.dumps(rows, indent=2) + '\n')
                print(row, flush=True)
    finally:
        for tap in taps:
            tap.listener.close()
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
        for child in reversed(children):
            try:
                child.wait(30)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
        for log in logs:
            log.close()


if __name__ == '__main__':
    main()
