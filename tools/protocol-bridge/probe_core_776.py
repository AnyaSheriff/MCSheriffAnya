#!/usr/bin/env python3
"""Ограниченная настоящая core-сессия776; запуск только после отдельного окна root.

Переиспользует собственные Client/Probe/connect, не повторяет общий TCP клиент.
Сохраняет имена/размеры кадров и выбранные предметы; полный recipes не пишет.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import time

from probe_gameplay import connect
from probe_session import Probe
from probe_776_payloads import table
from smoke import varint

PROTOCOL_SHA = 'a6c9bd7240f74b94eb0d9b1497294e4cfaa4bb02c0023817c952429149e2d50c'
PORT = 26028


class Probe776(Probe):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.markers = []
        self.acks = []

    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport))
        self.acks.append(teleport)

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        if name == 'ping':
            self.send('pong', payload)
        if name == 'system_chat' and b'mcsa776-smoke' in payload:
            self.markers.append('mcsa776-smoke')
def stop_server(server):
    """Закрываем собственный stdin после stop: Tokio blocking read должен увидеть EOF.

    API docs tokio::io::stdin: blocking read cannot be cancelled at shutdown.
    Таймаут/аварийное завершение не маскируются кодом0.
    """
    if server.poll() is not None:
        try:
            server.stdin.close()
        except BrokenPipeError:
            pass
        return 'already-exited'
    try:
        server.stdin.write('stop\n'); server.stdin.flush()
    except BrokenPipeError:
        pass
    finally:
        try:
            server.stdin.close()
        except BrokenPipeError:
            pass
    try:
        server.wait(timeout=30)
        return 'stop-and-stdin-eof'
    except subprocess.TimeoutExpired:
        os.killpg(server.pid, signal.SIGKILL); server.wait(timeout=10)
        return 'killed-after-stop-timeout'



def verify_stale_pickup(inventory, stone):
    # Обычный776: verification-stale-click776-01, SHA5b3db7a83ffad9e4049af37a446d851145c4cef4544d5ac2242a012ccf8267db.
    # State=-1 требует снимок, но не отменяет допустимый pickup из hotbar36.
    assert inventory['window'] == 0
    assert len(inventory['slots']) == 46 and all(stack is None for stack in inventory['slots'])
    assert inventory['cursor'] == {'item': stone, 'count': 5}
    stacks = [*inventory['slots'], inventory['cursor']]
    assert sum(stack['count'] for stack in stacks if stack and stack['item'] == stone) == 5


def plan():
    return {'port': PORT, 'protocol': 776, 'native_protocol': 775,
            'test': 'gameplay_probe::serve_java_26_2_probe',
            'steps': ['ordinary Login/Configuration/enter_world and first ticks',
                      'stone5 via console give; assert authoritative inventory',
                      'stale revision pickup; assert authoritative resync and five stones on cursor',
                      'console op and client chat_command say mcsa776-smoke',
                      'console tp .5 64 .5; short ACK; remain connected',
                      'stop, preserve selected records and log, verify port free'],
            'excluded': ['public admission', 'primary776', 'mobs', 'block entities',
                         'creative input', 'world particles', 'block actions', 'AuthMe/online']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--test-binary', type=Path)
    parser.add_argument('--protocol', type=Path)
    parser.add_argument('--cache', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if not args.execute:
        print(json.dumps(plan(), ensure_ascii=False, indent=2)); return
    assert all([args.test_binary, args.protocol, args.cache, args.output]), 'explicit inputs required'
    assert not args.output.exists(), 'preserve previous runs; choose a new output directory'
    raw = args.protocol.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PROTOCOL_SHA
    protocol = json.loads(raw)
    stone = next(item['id'] for item in table(args.cache, 'items') if item['name'] == 'stone')
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', PORT))
    args.output.mkdir(parents=True)
    report = {'plan': plan(), 'checks': [], 'protocol_sha256': PROTOCOL_SHA,
              'test_binary_sha256': hashlib.sha256(args.test_binary.read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='mcsa-core776-') as directory:
        root = Path(directory)
        (root / 'mcsa-gameplay-probe.marker').touch()
        log = root / 'server.log'
        with log.open('w') as output:
            cpus = sorted(os.sched_getaffinity(0))[:2]
            assert len(cpus) == 2
            server = subprocess.Popen(['taskset', '-c', ','.join(map(str, cpus)), str(args.test_binary.resolve()),
                '--ignored', '--exact', plan()['test'], '--nocapture'], cwd=root,
                stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True, start_new_session=True)
            client = None
            def interrupted(_signum, _frame):
                raise InterruptedError('interrupted; stop own process in finally')
            previous = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGTERM, signal.SIGINT)}
            def command(value):
                server.stdin.write(value + '\n'); server.stdin.flush()
            try:
                deadline = time.monotonic() + 45
                while 'MCSA gameplay probe ready' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.05)
                client = connect(PORT, protocol, Probe776, 776)
                client.pump(15, lambda record: record['name'] == 'position')
                client.pump(4)
                assert 'зашёл на сервер' in log.read_text()
                required = {'login', 'position', 'map_chunk', 'declare_commands', 'declare_recipes',
                            'player_info', 'advancements', 'window_items', 'entity_metadata'}
                assert required <= set(client.names), sorted(required - set(client.names))
                assert client.acks and client.inventory
                report['checks'].append('normal enter_world and mandatory first tick packets')
                command('give McsaProbe stone 5'); client.pump(2)
                assert any(value == {'item': stone, 'count': 5} for value in client.inventory['slots'])
                report['checks'].append('give stone5 authoritative inventory')
                before = client.names.count('window_items')
                client.send('window_click', varint(0) + varint(-1) + struct.pack('>hb', 36, 0) + bytes([0, 0, 0]))
                client.pump(2)
                assert client.names.count('window_items') > before
                verify_stale_pickup(client.inventory, stone)
                report['checks'].append('stale revision pickup and authoritative resync match ordinary776')
                command('op McsaProbe'); client.pump(1)
                message = b'say mcsa776-smoke'
                client.send('chat_command', varint(len(message)) + message)
                client.pump(2); assert client.markers
                report['checks'].append('client command reaches core and chat response')
                before = len(client.acks)
                command('tp McsaProbe 0.5 64 0.5')
                client.pump(5, lambda record: record.get('position', [])[:3] == [.5, 64., .5])
                client.pump(2); assert len(client.acks) > before
                assert 'потерял соединение' not in log.read_text()
                report['checks'].append('core teleport short ACK and session remains connected')
            except BaseException as error:
                report['error'] = f'{type(error).__name__}: {error}'
                raise
            finally:
                try:
                    if client:
                        report['records'] = client.records
                        report['teleport_acks'] = client.acks
                        report['inventory'] = client.inventory
                        client.sock.close()
                    report['shutdown'] = stop_server(server)
                    report['server_exit'] = server.returncode
                    shutil.copyfile(log, args.output / 'server.log')
                    with socket.socket() as check:
                        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                        check.bind(('127.0.0.1', PORT))
                    report['port_free'] = True
                    try:
                        os.killpg(server.pid, 0)
                    except ProcessLookupError:
                        report['process_group_gone'] = True
                    else:
                        report['process_group_gone'] = False
                finally:
                    report['passed'] = (
                        'error' not in report and len(report['checks']) == 5
                        and report.get('server_exit') == 0
                        and report.get('shutdown') == 'stop-and-stdin-eof'
                        and report.get('port_free') is True
                        and report.get('process_group_gone') is True
                    )
                    for sig, handler in previous.items(): signal.signal(sig, handler)
                    (args.output / 'result.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    assert report['passed'], 'probe or clean shutdown failed; see result.json'


if __name__ == '__main__':
    main()
