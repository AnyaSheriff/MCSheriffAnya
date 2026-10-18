#!/usr/bin/env python3
"""Офлайн-вход до Play/парольного диалога и перехват обращений к Mojang.

Изолированные серверы, синтетический игрок и локальный HTTP proxy с ответом502.
Ни один проверочный HTTP-запрос не пересылается внешнему сервису.
"""
import argparse
import io
import json
import os
from pathlib import Path
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'protocol-bridge'))
from smoke import varint, read_varint, send, handshake
from observe import frame


class Proxy(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.request.settimeout(3)
        line = self.rfile.readline(4096).decode('ascii', errors='replace').split()
        if len(line) >= 2:
            self.server.targets.append(line[1])
        self.wfile.write(b'HTTP/1.1 502 Test proxy\r\nContent-Length: 0\r\nConnection: close\r\n\r\n')


def login(port, password_required):
    with socket.create_connection(('127.0.0.1', port), timeout=10) as sock:
        handshake(sock, 775, port, 2)
        name = b'OfflineProbe'
        send(sock, b'\0' + varint(len(name)) + name + bytes([42]) * 16)
        response = io.BytesIO(frame(sock))
        assert read_varint(response.read) == 2, 'unexpected license challenge/disconnect'
        send(sock, b'\x03')
        for _ in range(160):
            response = io.BytesIO(frame(sock))
            packet_id = read_varint(response.read)
            payload = response.read()
            if packet_id == 14:
                send(sock, b'\x07' + payload)
            elif packet_id == 18:
                assert password_required, 'unexpected AuthMe dialog'
                return 'password-dialog'
            elif packet_id == 3:
                assert not password_required, 'password protection bypassed'
                send(sock, b'\x03')
                assert frame(sock), 'missing Play response'
                return 'play'
        raise AssertionError('configuration did not finish')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--port', type=int, default=26004)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve())
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', args.port))
    checks = []
    with Proxy(('127.0.0.1', 0), Handler) as proxy:
        proxy.targets = []
        threading.Thread(target=proxy.serve_forever, daemon=True).start()
        env = dict(os.environ)
        address = f'http://127.0.0.1:{proxy.server_address[1]}'
        env.update(HTTPS_PROXY=address, https_proxy=address, HTTP_PROXY=address,
                   http_proxy=address, ALL_PROXY=address, all_proxy=address,
                   NO_PROXY='', no_proxy='')
        try:
            for mode, password in [('disabled', False), ('disabled', True), ('cosmetics', False), ('legacy', False)]:
                proxy.targets.clear()
                with tempfile.TemporaryDirectory(prefix='mcsa-offline-smoke-') as directory:
                    root = Path(directory)
                    (root / 'config').mkdir()
                    (root / 'config/server.properties').write_text(
                        f'server-port={args.port}\nonline-mode=false\nnetwork-compression-threshold=-1\n'
                        'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
                    (root / 'config/skins.toml').write_text('mojang=true\nely=false\n')
                    if mode == 'legacy':
                        (root / 'config/rustcraft.toml').write_text(
                            'auth_enabled=false\nauth_premium=false\nbedrock_port=0\nprefetch_chunks=0\n')
                    else:
                        (root / 'config/mcsa.properties').write_text(
                            f'auth-enabled={str(password).lower()}\nauth-premium=true\n'
                            f'offline-license-mode={mode}\nbedrock-port=0\nprefetch-chunks=0\n')
                    log = root / 'server.log'
                    with log.open('w') as output:
                        server = subprocess.Popen([binary], cwd=root, env=env, stdin=subprocess.PIPE,
                                                  stdout=output, stderr=subprocess.STDOUT, text=True)
                        try:
                            deadline = time.monotonic() + 30
                            while 'Готово (' not in log.read_text():
                                assert server.poll() is None, log.read_text()
                                assert time.monotonic() < deadline, 'startup timeout'
                                time.sleep(.05)
                            result = login(args.port, password)
                            if mode == 'cosmetics':
                                assert any('api.mojang.com' in target for target in proxy.targets), proxy.targets
                            else:
                                assert not proxy.targets, 'disabled mode made HTTP requests'
                            if mode == 'legacy':
                                assert 'Первый запуск:' in log.read_text()
                                assert 'Основная версия игры: Minecraft Java 26.1.2' in log.read_text()
                                assert 'java-version=26.1.2' in (root / 'config/mcsa.properties').read_text()
                                assert 'offline-license-mode=disabled' in (root / 'config/mcsa.properties').read_text()
                            checks.append({'mode': mode, 'auth': password, 'result': result,
                                           'http_requests': len(proxy.targets)})
                        finally:
                            if server.poll() is None:
                                server.stdin.write('stop\n')
                                server.stdin.flush()
                                try:
                                    server.wait(timeout=30)
                                except subprocess.TimeoutExpired:
                                    server.kill()
                                    server.wait()
                            assert server.returncode == 0, log.read_text()
            for invalid in ['offline-license-mode=disabeld', 'java-version=26.3', 'java-version=26.2', 'java-version=unknown']:
                with tempfile.TemporaryDirectory(prefix='mcsa-invalid-settings-') as directory:
                    root = Path(directory)
                    (root / 'config').mkdir()
                    (root / 'config/mcsa.properties').write_text(invalid + '\nbedrock-port=0\n')
                    result = subprocess.run([binary], cwd=root, env=env, input='stop\n',
                                            capture_output=True, text=True, timeout=10)
                    assert result.returncode != 0 and 'Готово (' not in result.stdout
                    assert not (root / 'auth.db').exists()
                    checks.append({'rejected': invalid})
        finally:
            proxy.shutdown()
    print(json.dumps({'passed': len(checks), 'checks': checks}, ensure_ascii=False))


if __name__ == '__main__':
    main()
