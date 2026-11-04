#!/usr/bin/env python3
"""Свежие offline777 auth-сценарии; только CI в loopback-only namespace."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import subprocess
import tempfile
import time
from unittest.mock import patch

from probe_gameplay import Client, connect, mapping
import probe_gameplay
from observe import frame
from smoke import read_varint, send, varint


def require(value, message):
    if not value:
        raise AssertionError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wait(predicate, seconds=20):
    deadline = time.monotonic() + seconds
    while not predicate():
        require(time.monotonic() < deadline, 'observation deadline')
        time.sleep(.05)


def validate_boundary(join_count, listed, before, after):
    require(join_count == 0, 'world join before password')
    require(listed == 0, 'unauthenticated player listed')
    require(before == after, 'playerdata changed before password')


def parse_list_count(line):
    # Console renderer main использует английский перевод; старый ответ
    # поддержан только в точном формате. Чужая строка не означает ноль.
    line = re.sub(r'^\[\d{2}:\d{2}:\d{2}\] \[Server thread/INFO\]: ', '', line, count=1)
    number = r'(0|[1-9][0-9]*)'
    match = re.fullmatch(r'There are '+number+r' of a max of '+number+r' players online: (.*)', line)
    if match:
        count, names = int(match[1]), match[3]
    else:
        if re.fullmatch(r'Игроков на сервере: 0 из '+number, line):
            return 0
        match = re.fullmatch(r'Игроков на сервере: ([1-9][0-9]*) из '+number+r': (.*)', line)
        if not match:
            return None
        count, names = int(match[1]), match[3]
    players = names.split(', ') if names else []
    if len(players) != count or any(not re.fullmatch(r'[A-Za-z0-9_]{1,16}', name) for name in players):
        return None
    return count


def validate_default_dialog(packet_id, payload):
    # Pinned protocol777: ShowDialog=0x13; 0x12 уже ClearDialog.
    require(packet_id == 0x13, 'expected ShowDialog777, not ClearDialog or canonical775 ID')
    require(b'mcsheriffanya:register' in payload and b'password' in payload, 'wrong custom dialog')


def playerfiles(root):
    return {str(p.relative_to(root)): digest(p) for p in sorted((root/'world/playerdata').glob('*')) if p.is_file()}


class BoundaryObserved(Exception):
    pass


def exercise(binary, protocol, output, mode, password, wrong):
    result = {'mode': mode, 'checks': [], 'wire': [], 'passed': False}
    process = client = None
    log_path = output / ('auth777-' + mode + '.log')
    with tempfile.TemporaryDirectory(prefix='mcsa-auth777-') as directory:
        root = Path(directory)
        (root/'config').mkdir()
        with socket.socket() as temporary_socket:
            temporary_socket.bind(('127.0.0.1', 0))
            port = temporary_socket.getsockname()[1]
        settings = ('java-version=26.3\nauth-enabled=' + ('false' if mode == 'disabled' else 'true') +
                    '\noffline-license-mode=disabled\nsend-error-reports=false\nbedrock-port=0\n'
                    'prefetch-chunks=0\nconsole-colors=never\n')
        if mode == 'chat': settings += 'auth-mode=chat\n'
        if mode == 'proxy': settings += 'proxy-protocol=true\n'
        (root/'config/mcsa.properties').write_text(settings)
        (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
        (root/'config/server.properties').write_text(
            f'server-port={port}\nonline-mode=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
        with log_path.open('w') as log:
            try:
                process = subprocess.Popen([str(binary)], cwd=root, stdin=subprocess.PIPE,
                                           stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
                def text(): return log_path.read_text()
                wait(lambda: process.poll() is not None or 'Готово (' in text(), 120)
                require(process.poll() is None, 'startup exited')
                def command(value):
                    process.stdin.write(value+'\n'); process.stdin.flush()
                def boundary(baseline):
                    def observations():
                        return [count for line in text().splitlines() if (count := parse_list_count(line)) is not None]
                    count = len(observations())
                    command('list')
                    wait(lambda: len(observations()) > count)
                    listed = observations()[-1]
                    validate_boundary(text().count('вошёл с номером сущности'), listed, baseline, playerfiles(root))
                    result['checks'].append('no-world-join-list-or-playerdata-before-password')
                handshake = probe_gameplay.handshake
                def proxy_handshake(sock, *args):
                    sock.sendall(b'PROXY TCP4 127.0.0.2 127.0.0.1 40000 '+str(port).encode()+b'\r\n')
                    handshake(sock, *args)
                def open_client(factory=Client, handler=None):
                    with patch.object(probe_gameplay, 'handshake', proxy_handshake if mode == 'proxy' else handshake):
                        return connect(port, protocol, factory, 777, username='Auth777Probe', configuration_handler=handler)
                if mode == 'dialog':
                    baseline = playerfiles(root)
                    def configuration(sock, packet_id, payload):
                        result['wire'].append({'phase':'configuration', 'id':packet_id, 'hex':payload.hex()})
                        require(packet_id != 3, 'FinishConfiguration bypassed dialog')
                        if packet_id in (0x12, 0x13):
                            validate_default_dialog(packet_id, payload)
                            boundary(baseline)
                            result['checks'].append('default-custom-dialog-before-finish-configuration')
                            raise BoundaryObserved()
                        return False
                    try: open_client(handler=configuration)
                    except BoundaryObserved: pass
                    else: raise AssertionError('dialog boundary not observed')
                else:
                    class Probe(Client):
                        def __init__(self, *args):
                            super().__init__(*args)
                            self.prompts = self.resets = self.logins = 0
                        def credential(self, value):
                            raw = value.encode()
                            # Пароли никогда не проходят записывающую обёртку.
                            send(self.sock, varint(self.outgoing['chat_command'])+varint(len(raw))+raw)
                        def observe_packet(self, name, payload):
                            result['wire'].append({'phase':'play', 'name':name, 'hex':payload.hex()})
                            require(len(result['wire']) <= 10000, 'packet budget')
                            if name == 'login': self.logins += 1
                            if name == 'start_configuration':
                                self.resets += 1
                                self.send('configuration_acknowledged')
                                incoming = mapping(protocol, 'configuration', 'toClient')
                                for _ in range(160):
                                    packet = io.BytesIO(frame(self.sock)); kind = read_varint(packet.read); data = packet.read()
                                    result['wire'].append({'phase':'reconfiguration', 'id':kind, 'hex':data.hex()})
                                    if kind == incoming['select_known_packs']: send(self.sock, b'\x07'+data)
                                    elif kind == 4: send(self.sock,b'\x04'+data)
                                    elif kind == 3:
                                        send(self.sock,b'\x03'); break
                                else: raise AssertionError('reconfiguration budget')
                    if mode == 'disabled':
                        client = open_client(Probe)
                        client.pump(30, lambda _: client.logins == 1 and any(r['name']=='window_items' for r in client.records))
                        require(client.resets == 0, 'disabled auth unexpectedly reconfigured')
                        require('вошёл с номером сущности' in text(), 'no real join')
                        result['checks'].append('disabled-direct-real-play')
                    else:
                        baseline = playerfiles(root)
                        client = open_client(Probe)
                        client.pump(30, lambda r: r['name']=='system_chat' and 'Регистрация:'.encode() in bytes.fromhex(result['wire'][-1]['hex']))
                        boundary(baseline)
                        require(client.logins == 1 and client.resets == 0, 'not isolated limbo')
                        client.credential('register '+password+' '+password)
                        client.pump(30, lambda _: client.resets == 1 and client.logins == 2 and any(r['name']=='window_items' for r in client.records))
                        require(text().count('вошёл с номером сущности') == 1, 'real join missing or duplicated')
                        result['checks'].append('registration-reconfigures-then-real-play')
                        if mode == 'proxy':
                            require('Java PROXY: 127.0.0.2:40000 via 127.0.0.1:' in text(), 'proxy identity not applied')
                            result['checks'].append('trusted-proxy-forwarded-identity-chat-limbo')
                        if mode == 'chat':
                            client.sock.close(); client = None
                            wait(lambda: bool(playerfiles(root)))
                            result['checks'].append('authenticated-playerdata-persisted')
                            # Сохранение завершилось до следующего входа.
                            wait(lambda: 'Auth777Probe вышел' in text())
                            client = open_client(Probe)
                            client.pump(30, lambda r: r['name']=='system_chat')
                            baseline = playerfiles(root)
                            client.credential('login '+wrong)
                            client.pump(10, lambda r: r['name']=='system_chat' and 'Неверный пароль'.encode() in bytes.fromhex(result['wire'][-1]['hex']))
                            require(client.resets == 0 and client.logins == 1, 'wrong password entered real world')
                            require(text().count('вошёл с номером сущности') == 1, 'wrong password caused join')
                            require(playerfiles(root) == baseline, 'wrong password wrote playerdata')
                            result['checks'].append('wrong-password-blocked')
                            client.credential('login '+password)
                            client.pump(30, lambda _: client.resets == 1 and client.logins == 2 and any(r['name']=='window_items' for r in client.records))
                            require(text().count('вошёл с номером сущности') == 2, 'correct login did not join')
                            result['checks'].append('correct-password-relogin-enters')
                    if client: client.sock.close(); client = None
                result['passed'] = True
            except Exception as error:
                result['failure'] = type(error).__name__ + ': ' + str(error)
            finally:
                if client: client.sock.close()
                if process and process.poll() is None:
                    process.stdin.write('stop\n'); process.stdin.flush(); process.stdin.close()
                    try: process.wait(timeout=45)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=10)
                result['server_exit'] = process.returncode if process else None
        result['playerdata_after'] = playerfiles(root)
    raw_log = log_path.read_text()
    require(password not in raw_log and wrong not in raw_log, 'credential exposed in server log')
    serialized = json.dumps(result, ensure_ascii=False)
    require(password not in serialized and wrong not in serialized and password.encode().hex() not in serialized and wrong.encode().hex() not in serialized, 'credential exposed in evidence')
    result['log_sha256'] = digest(log_path)
    result['passed'] = result['passed'] and result['server_exit'] == 0
    if any('/ERROR]:' in line for line in raw_log.splitlines()):
        result['passed'] = False
        result['failure'] = 'runtime ERROR in auth smoke'
    (output/('auth777-'+mode+'.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    require(result['passed'], 'auth777 '+mode+' failed: '+result.get('failure', 'server exit'))
    return {'mode':mode, 'checks':result['checks'], 'passed':True, 'server_exit':result['server_exit'],
            'evidence_sha256':{path.name:digest(path) for path in [log_path, output/('auth777-'+mode+'.json')]}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--protocol', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    require({name for _,name in socket.if_nameindex()} == {'lo'}, 'loopback-only network namespace required')
    manifest = json.loads(Path(__file__).with_name('candidate-sources-26.3.json').read_text())
    pin = next(row for row in manifest['files'] if row['file']=='protocol.json')
    require(digest(args.protocol) == pin['sha256'], 'unpinned protocol JSON')
    args.output.mkdir(parents=True, exist_ok=True)
    receipt = {'passed':False, 'binary_sha256':digest(args.binary), 'protocol_sha256':digest(args.protocol), 'cases':[], 'namespace':os.readlink('/proc/self/ns/net')}
    try:
        protocol = json.loads(args.protocol.read_text())
        password, wrong = secrets.token_hex(16), secrets.token_hex(16)
        for mode in ['chat', 'proxy', 'disabled', 'dialog']:
            receipt['cases'].append(exercise(args.binary.resolve(), protocol, args.output, mode, password, wrong))
        receipt['passed'] = True
    except Exception as error:
        receipt['failure'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        (args.output/'auth777-summary.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({'auth777':'PASS', 'cases':len(receipt['cases']), 'checks':sum(len(case['checks']) for case in receipt['cases'])}))


if __name__ == '__main__': main()
