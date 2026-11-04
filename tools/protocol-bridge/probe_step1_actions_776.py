#!/usr/bin/env python3
"""Одна ограниченная сетевая проба776: авторский камень и сундук, без дампов."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time

from probe_block_entities import exact, nbt
from probe_core_776 import PROTOCOL_SHA
from probe_gameplay import connect
from probe_step1_gameplay_776 import GameplayProbe
from probe_white_concrete_slab import pack_position
from smoke import read_varint, varint
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'measure'))
from pregen import Rcon


class ActionProbe(GameplayProbe):
    def __init__(self, *args):
        super().__init__(*args)
        self.chests = []

    def observe_packet(self, name, payload):
        if name == 'map_chunk' and payload[:8] == struct.pack('>ii', -1, -1):
            data = io.BytesIO(payload[8:])
            for _ in range(read_varint(data.read)):
                read_varint(data.read)
                exact(data, read_varint(data.read) * 8)
            exact(data, read_varint(data.read))
            for _ in range(read_varint(data.read)):
                xz, y = exact(data, 1)[0], int.from_bytes(exact(data, 2), 'big', signed=True)
                kind = read_varint(data.read)
                start = data.tell()
                value = nbt(data)
                if (xz, y) == (0, 65):
                    self.chests.append(dict(kind=kind, nbt=value, hex=data.getvalue()[start:data.tell()].hex()))
            self.raw.append(dict(name=name, bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest()))
            self.seen.append(name)
            return
        super().observe_packet(name, payload)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'protocol', 'entities', 'output'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--port', type=int, default=26188)
    a = p.parse_args()
    assert 26100 <= a.port <= 26198
    for port in (a.port, a.port + 1):
        with socket.socket() as s:
            s.bind(('127.0.0.1', port))
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == '823e2250d24b3ddac457a60c92a6a941943fcd6a'
    assert hashlib.sha256(a.protocol.read_bytes()).hexdigest() == PROTOCOL_SHA
    protocol = json.loads(a.protocol.read_text())
    entity = next(v['id'] for v in json.loads(a.entities.read_text()) if v['name'] == 'item')
    a.output.mkdir(parents=True, exist_ok=False)
    report = dict(protocol=776, commands=[], candidates=[], status='failed',
                  jar_sha256=hashlib.sha256(a.jar.read_bytes()).hexdigest(),
                  runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    clients = []
    client = None
    with tempfile.TemporaryDirectory(prefix='mcsa-actions776-') as folder:
        root = Path(folder)
        shutil.copyfile(a.jar, root / 'server.jar')
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={a.port}\nrcon.port={a.port+1}\n'
            'enable-rcon=true\nrcon.password=local-actions-probe\nonline-mode=false\n'
            'white-list=false\nenforce-whitelist=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\nmax-tick-time=0\n')
        log = a.output / 'server.log'
        with log.open('w') as out:
            server = subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', 'server.jar', 'nogui'],
                                      cwd=root, stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT)
            report['pid'] = server.pid
            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.2)
                rcon = Rcon(a.port + 1, 'local-actions-probe')
                def cmd(text):
                    result = rcon.run(text)
                    report['commands'].append(dict(command=text, result=result))
                    if client:
                        client.pump(.2)
                    return result
                def login():
                    c = connect(a.port, protocol, lambda s, pr: ActionProbe(s, pr, entity, 776), 776)
                    clients.append(c)
                    c.pump(20, lambda _: c.position is not None and 'map_chunk' in c.seen)
                    c.pump(1.)
                    return c
                client = login()
                for text in ('fill -18 64 -18 5 64 5 stone', 'fill -18 65 -18 5 68 5 air',
                             'tp McsaProbe 0.5 65 0.5', 'gamemode survival McsaProbe',
                             'item replace entity McsaProbe weapon.mainhand with iron_pickaxe',
                             'forceload add -16 -16', 'setblock -16 65 -16 chest'):
                    cmd(text)
                client.grounded_fixture = True
                for candidate in (1, 2, 3):
                    cmd('setblock 2 65 0 stone')
                    before = len(client.raw)
                    client.send('block_dig', varint(0) + pack_position((2, 65, 0)) + b'\x01' + varint(candidate * 2))
                    client.pump(1.5)
                    client.send('block_dig', varint(candidate) + pack_position((2, 65, 0)) + b'\x01' + varint(candidate * 2 + 1))
                    client.pump(.5)
                    offset = len(log.read_text())
                    result = cmd(f'execute if block 2 65 0 air run say ACTION_{candidate}_AIR')
                    result += log.read_text()[offset:]
                    report['candidates'].append(dict(start=0, end=candidate, result=result, records=client.raw[before:]))
                client.sock.close()
                client = None
                time.sleep(.5)
                client = login()
                report['chests'] = client.chests
                assert client.chests == [dict(kind=1, nbt=None, hex='00')], client.chests
                assert 'ACTION_2_AIR' in report['candidates'][1]['result'], report['candidates']
                assert all('AIR' not in report['candidates'][i]['result'] for i in (0, 2))
                report['status'] = 'pass'
            except BaseException as error:
                report['error'] = repr(error)
                raise
            finally:
                report['sessions'] = [dict(sent=c.sent, records=c.raw, chests=c.chests) for c in clients]
                for c in clients:
                    c.sock.close()
                server.terminate()
                try:
                    server.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=10)
                report['server_exit'] = server.returncode
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(status=report['status'], candidates=[v['result'] for v in report['candidates']], chests=report['chests'])))


if __name__ == '__main__':
    main()
