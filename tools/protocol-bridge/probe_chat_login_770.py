#!/usr/bin/env python3
"""Живая проверка парольного лимбо770; исходящие пароли не записываются."""
import argparse
import io
import hashlib
import json
import secrets
import socket
import struct
import subprocess
import tempfile
import time
from pathlib import Path
from probe_gameplay import connect, mapping
from probe_step1_gameplay_776 import GameplayProbe
from probe_registry_client import read_string
from observe import frame
from smoke import varint, send, read_varint
from probe_core_776 import stop_server


def main():
    p = argparse.ArgumentParser()
    target=p.add_mutually_exclusive_group(required=True)
    target.add_argument('--test-binary', type=Path)
    target.add_argument('--binary', type=Path)
    p.add_argument('--protocol', type=Path, required=True)
    p.add_argument('--primary', type=int, choices=(775,777), required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument("--client",type=int,choices=(769,770),default=770)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    protocol = json.loads(a.protocol.read_text())
    executable=a.binary or a.test_binary
    report = dict(client=a.client, primary=a.primary, checks=[], status='failed', public_admission=bool(a.binary),
                  binary_sha256=hashlib.sha256(executable.read_bytes()).hexdigest())
    password = secrets.token_hex(16)
    wrong = secrets.token_hex(16)
    server = client = None
    with tempfile.TemporaryDirectory(prefix='mcsa-chat770-') as folder:
        root = Path(folder)
        for marker in ['mcsa-gameplay-probe.marker', 'mcsa-password-probe.marker']:
            (root / marker).touch()
        if a.primary == 777: (root / 'mcsa-catalog-preview263.marker').touch()
        if a.binary:
            (root/'config').mkdir()
            primary='26.3' if a.primary==777 else '26.1.2'
            (root/'config/mcsa.properties').write_text(f'java-version={primary}\nauth-enabled=true\nauth-premium=false\nsend-error-reports=false\nbedrock-port=0\nprefetch-chunks=0\n')
            (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
            (root/'config/server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={26126 if a.client==769 else 26127}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
        log_path = a.output / 'server.log'
        with log_path.open('w') as log:
            try:
                argv=[str(a.binary.resolve())] if a.binary else [str(a.test_binary.resolve()), '--ignored', '--exact',
                    ('gameplay_probe::serve_java_1_21_4_probe' if a.client==769 else 'gameplay_probe::serve_java_1_21_5_probe'), '--nocapture']
                server = subprocess.Popen(argv, cwd=root,
                    stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True)
                until = time.monotonic()+120
                ready='Готово (' if a.binary else 'MCSA gameplay probe ready'
                while ready not in log_path.read_text():
                    assert server.poll() is None and time.monotonic()<until
                    time.sleep(.1)
                class Client(GameplayProbe):
                    def __init__(self, sock, pr, registered, reject=False):
                        super().__init__(sock, pr, 0, a.client)
                        self.registered, self.reject = registered, reject
                        self.prompts = self.resets = 0
                        self.joins = log_path.read_text().count('вошёл с номером сущности')
                    def credential(self, text):
                        # Не вызываем записывающие payload отладочные обёртки.
                        raw = text.encode()
                        send(self.sock, varint(self.outgoing['chat_command'])+varint(len(raw))+raw)
                    def observe_packet(self, name, payload):
                        super().observe_packet(name, payload)
                        if name == 'system_chat' and any(s.encode() in payload for s in ['Регистрация:', 'Вход:', 'Неверный пароль']):
                            assert log_path.read_text().count('вошёл с номером сущности') == self.joins
                            if not self.registered: assert not list((root/'world/playerdata').glob('*'))
                            self.prompts += 1
                            if self.prompts == 1:
                                self.send('position', struct.pack('>dddB', 100.5, 100., 100.5, 1))
                                self.send('block_dig', varint(0)+bytes(8)+bytes([1])+varint(1))
                            if self.reject or (self.registered and self.prompts == 1):
                                self.credential('login '+wrong)
                            elif self.registered:
                                self.credential('login '+password)
                            else:
                                self.credential('register '+password+' '+password)
                        elif name == 'start_configuration':
                            self.resets += 1
                            self.send('configuration_acknowledged')
                            cfg = mapping(protocol, 'configuration', 'toClient')
                            for _ in range(160):
                                data = io.BytesIO(frame(self.sock)); pid = read_varint(data.read); body = data.read()
                                if pid == cfg['select_known_packs']: send(self.sock, b'\x07'+body)
                                elif pid == 3:
                                    send(self.sock,b'\x03'); break
                            else: raise AssertionError('configuration budget')
                def login(registered, reject=False):
                    c = connect(26126 if a.client==769 else 26127, protocol, lambda s, pr: Client(s, pr, registered, reject), a.client)
                    return c
                client = login(False)
                client.pump(30, lambda _: client.resets==1 and 'window_items' in client.seen)
                assert client.prompts == 1 and client.resets==1
                report['checks'].append('register-frozen-limbo-no-playerdata-before-password')
                client.sock.close(); client=None
                until=time.monotonic()+10
                while 'McsaProbe вышел' not in log_path.read_text():
                    assert time.monotonic()<until;time.sleep(.1)
                client=login(True, True)
                try: client.pump(15)
                except (RuntimeError, EOFError, ConnectionError, OSError): pass
                assert client.prompts==3 and client.resets==0
                assert log_path.read_text().count('вошёл с номером сущности')==1
                report['checks'].append('three-wrong-passwords-close-without-world-entry')
                client.sock.close();client=None
                client=login(True)
                client.pump(30,lambda _:client.resets==1 and 'window_items' in client.seen)
                assert client.prompts==2 and client.resets==1
                report['checks'].append('relogin-wrong-rejected-correct-enters')
                # Повтор пароля после входа тоже не должен попасть в обычный чат.
                client.credential('login '+password)
                raw=('/login '+password).encode()
                body=varint(len(raw))+raw+bytes(16)+b'\0'+varint(0)+bytes(3)
                send(client.sock,varint(client.outgoing['chat_message'])+body)
                client.pump(.5)
                client.sock.close(); client=None
                report['checks'].append('credential-retries-after-login-remain-private')
                report['status']='pass'
            except BaseException as error:
                report['error']=type(error).__name__+': '+str(error)
            finally:
                if client: client.sock.close()
                if server: report['server_exit']=stop_server(server)
        text=log_path.read_text()
        assert password not in text and wrong not in text
        assert password not in json.dumps(report) and wrong not in json.dumps(report)
        report['checks'].append('passwords-absent-from-server-log-and-results')
    (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(report)
    if report['status']!='pass':raise SystemExit(1)

if __name__=='__main__':main()
