#!/usr/bin/env python3
"""Изолированная проверка готового бинарника; запускать только после Cargo, в CI.

Сеть должна быть отдельным Linux namespace, содержащим только loopback:
это исключает отправку даже случайного отчёта при проверке новых defaults.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import tempfile
import time

WARNING = 'Включена анонимная отправка отчётов об ошибках разработчикам'
ENABLED = 'Отчёты об ошибках включены:'


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(root):
    return {str(p.relative_to(root)): sha(p) for p in sorted(root.rglob('*')) if p.is_file()}


def varint(number):
    result = bytearray()
    while number > 127:
        result.append((number & 127) | 128)
        number >>= 7
    return bytes(result + bytes([number]))


def read_varint(stream):
    value = 0
    for shift in range(0, 35, 7):
        byte = stream.read(1)
        if not byte:
            raise EOFError('incomplete VarInt')
        value |= (byte[0] & 127) << shift
        if byte[0] < 128:
            return value
    raise ValueError('oversized VarInt')


def status(port):
    with socket.create_connection(('127.0.0.1', port), timeout=1) as sock:
        sock.settimeout(2)
        host = b'localhost'
        handshake = b'\x00' + varint(777) + varint(len(host)) + host + struct.pack('>H', port) + b'\x01'
        sock.sendall(varint(len(handshake)) + handshake + b'\x01\x00')
        stream = sock.makefile('rb')
        length = read_varint(stream)
        require(0 < length < 65536, 'invalid status packet length')
        payload = stream.read(length)
        require(len(payload) == length, 'truncated status packet')
        body = io.BytesIO(payload)
        require(read_varint(body) == 0, 'invalid status packet id')
        text = body.read(read_varint(body))
        require(body.read() == b'', 'trailing status bytes')
        answer = json.loads(text)
        require(answer['version'] == {'name': '26.3', 'protocol': 777}, 'incorrect native status version')
        sock.sendall(b'\x09\x01' + struct.pack('>q', 263777))
        require(read_varint(stream) == 9, 'invalid pong length')
        require(stream.read(9) == b'\x01' + struct.pack('>q', 263777), 'invalid pong')
        return answer


def free_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def config(root, extra, port):
    (root / 'config').mkdir()
    (root / 'config/mcsa.properties').write_text(
        'auth-enabled=false\noffline-license-mode=disabled\nbedrock-port=0\nconsole-colors=never\n' + extra)
    (root / 'config/server.properties').write_text(
        f'online-mode=false\nserver-ip=127.0.0.1\nserver-port={port}\n'
        'level-type=minecraft\\:flat\nview-distance=2\nsimulation-distance=2\n')


def run_case(binary, output, name, root, port, enabled=None, warning=None, reject=False):
    before = snapshot(root)
    result = {'name': name, 'before': before, 'port': port}
    log_path = output / (name + '.log')
    process = None
    try:
        with log_path.open('wb') as log:
            process = subprocess.Popen([str(binary)], cwd=root, stdin=subprocess.PIPE,
                                       stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            if reject:
                process.wait(timeout=15)
                require(process.returncode != 0, 'invalid configuration accepted')
            else:
                deadline = time.monotonic() + 90
                last_error = None
                while time.monotonic() < deadline:
                    require(process.poll() is None, 'server exited before status')
                    try:
                        result['status'] = status(port)
                        break
                    except (OSError, EOFError) as error:
                        last_error = str(error)
                        time.sleep(0.2)
                require('status' in result, f'startup deadline exceeded: {last_error}')
                # Status succeeds only once the world and shared state are ready.
                process.stdin.write(b'list\nstop\n')
                process.stdin.flush()
                process.stdin.close()
                process.wait(timeout=45)
                require(process.returncode == 0, 'unclean shutdown')
        result['exit_code'] = process.returncode
        result['after'] = snapshot(root)
        text = log_path.read_text(errors='replace')
        require('Мастер первого запуска' not in text, 'interactive wizard appeared')
        require('Выберите версию' not in text, 'version prompt appeared')
        if reject:
            require(result['before'] == result['after'], 'rejection modified configuration/world/files')
            require('поддерживается только' in text and '26.3' in text, 'missing version rejection')
        else:
            require((WARNING in text) == warning, 'incorrect first-start notice')
            require((ENABLED in text) == enabled, 'incorrect reporting state')
            saved = (root / 'config/mcsa.properties').read_text()
            require('java-version=26.3\n' in saved, 'primary version not persisted')
            require(f'send-error-reports={str(enabled).lower()}\n' in saved, 'reporting state not persisted')
            require(not list((root / 'logs/reports').rglob('*')), 'unexpected generated report')
            require('/ERROR]:' not in text and 'ПАНИКА' not in text, 'runtime error in smoke log')
        result['passed'] = True
        return result
    except Exception as error:
        result['failure'] = str(error)
        raise
    finally:
        if process is not None and process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=10)
        result['log_sha256'] = sha(log_path)
        (output / (name + '.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    binary, output = args.binary.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    # Fail closed: never run reporting-enabled cases with an outbound interface.
    require({name for _, name in socket.if_nameindex()} == {'lo'}, 'run inside unshare --net with loopback only')
    receipt = {'binary_sha256': sha(binary), 'cases': [], 'passed': False,
               'network_interfaces': sorted(name for _, name in socket.if_nameindex()),
               'network_namespace': os.readlink('/proc/self/ns/net'),
               'network_addresses': json.loads(subprocess.check_output(['ip', '-json', 'address'], text=True)),
               'network_routes': json.loads(subprocess.check_output(['ip', '-json', 'route', 'show', 'table', 'all'], text=True))}
    try:
        with tempfile.TemporaryDirectory(prefix='mcsa-startup-263-') as temporary:
            base = Path(temporary)
            for name, extra, enabled in [
                ('fresh-panel-partial', '', True),
                ('explicit-reports-false', 'send-error-reports=false\n', False),
                ('obsolete-choose', 'java-version=choose\nsend-error-reports=choose\n', True),
            ]:
                root = base / name
                root.mkdir()
                port = free_port()
                config(root, extra, port)
                (root / 'world').mkdir()
                receipt['cases'].append(run_case(binary, output, name, root, port, enabled, enabled))
                if name == 'fresh-panel-partial':
                    receipt['cases'].append(run_case(binary, output, 'restart', root, port, True, False))
            for version in ['26.1.2', '26.4']:
                name = 'reject-' + version
                root = base / name
                root.mkdir()
                port = free_port()
                config(root, f'java-version={version}\n', port)
                (root / 'world').mkdir()
                (root / 'world/old-world-marker').write_bytes(b'unsupported-world-must-survive\x00')
                receipt['cases'].append(run_case(binary, output, name, root, port, reject=True))
        receipt['passed'] = True
    finally:
        (output / 'startup263.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'startup263': 'PASS', 'cases': len(receipt['cases']), 'binary_sha256': receipt['binary_sha256']}))


if __name__ == '__main__':
    main()
