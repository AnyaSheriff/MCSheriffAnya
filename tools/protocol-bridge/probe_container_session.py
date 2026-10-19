#!/usr/bin/env python3
"""Обычное размещение шалкера и перенос предметов на оригинале или нашем ядре."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
from probe_particles import Probe as BaseProbe
from probe_gameplay import connect
from smoke import read_varint, varint


class Probe(BaseProbe):
    def __init__(self, sock, protocol, version, names):
        super().__init__(sock, protocol, version)
        self.names = names
        self.inventory = None
        self.windows = []

    def slot(self, data, template=False, depth=0):
        assert depth < 16
        if template:
            item = read_varint(data.read)
            count = read_varint(data.read)
        else:
            count = read_varint(data.read)
            if not count:
                return None
            item = read_varint(data.read)
        added, removed = read_varint(data.read), read_varint(data.read)
        assert 0 <= added <= 8 and removed == 0
        result = {'item': self.names[item], 'count': count}
        for _ in range(added):
            kind = read_varint(data.read)
            if kind == (53 if self.version == 777 else 51):
                assert data.read(1) == b'\1'
                result['potion'] = read_varint(data.read)
                assert data.read(1) == b'\0' and read_varint(data.read) == 0 and data.read(1) == b'\0'
            elif kind == (77 if self.version == 777 else 75):
                size = read_varint(data.read)
                assert 0 <= size <= 27
                contents = []
                for _ in range(size):
                    present = data.read(1)
                    assert present in (b'\0', b'\1')
                    contents.append(self.slot(data, True, depth + 1) if present == b'\1' else None)
                result['contents'] = contents
            else:
                raise AssertionError(f'unexpected component {kind}')
        return result

    def observe_packet(self, name, payload):
        data = io.BytesIO(payload)
        if name in ('window_items', 'set_slot', 'set_cursor_item', 'open_window'):
            self.observed.append({'name': name, 'payload': payload.hex()})
        if name == 'window_items':
            window, revision = read_varint(data.read), read_varint(data.read)
            size = read_varint(data.read)
            assert 0 <= size <= 100
            slots = [self.slot(data) for _ in range(size)]
            cursor = self.slot(data)
            assert not data.read()
            self.inventory = {'window': window, 'revision': revision, 'slots': slots, 'cursor': cursor}
        elif name == 'set_slot':
            window, revision = read_varint(data.read), read_varint(data.read)
            place = struct.unpack('>h', data.read(2))[0]
            value = self.slot(data)
            assert not data.read()
            if self.inventory and self.inventory['window'] == window:
                self.inventory['slots'][place] = value
                self.inventory['revision'] = revision
        elif name == 'set_cursor_item':
            value = self.slot(data)
            assert not data.read()
            if self.inventory:
                self.inventory['cursor'] = value
        elif name == 'open_window':
            self.windows.append({'window': read_varint(data.read), 'type': read_varint(data.read)})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--version', type=int, choices=[775, 777], required=True)
    source = p.add_mutually_exclusive_group(required=True)
    source.add_argument('--jar', type=Path)
    source.add_argument('--test-binary', type=Path)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int, required=True)
    a = p.parse_args()
    if a.test_binary:
        assert a.version == 777 and a.port == 26026
    sources = json.loads((a.data / 'sources.json').read_text())
    for entry in sources['files'] + sources['baseline_files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / ('protocol.json' if a.version == 777 else 'protocol-26.1.json')).read_text())
    item_table = a.data / 'items.json' if a.version == 777 else Path(__file__).resolve().parents[2] / 'tools/data/items.json'
    names = {v['id']: v['name'] for v in json.loads(item_table.read_text())}
    commands = Path(__file__).parent / 'testdata/nested-item-commands.json'
    item = next(c['command'] for c in json.loads(commands.read_text()) if c['name'] == 'container_sparse')
    report = {'version': a.version, 'checks': [], 'snapshots': {}, 'commands_sha256': hashlib.sha256(commands.read_bytes()).hexdigest()}
    report['items_sha256'] = hashlib.sha256(item_table.read_bytes()).hexdigest()
    if a.jar:
        expected = {775: '97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51', 777: '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
        assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == expected
        report['jar_sha1'] = expected
    else:
        report['test_binary_sha256'] = hashlib.sha256(a.test_binary.read_bytes()).hexdigest()
    with socket.socket() as check:
        check.bind(('127.0.0.1', a.port))
    a.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='mcsa-container-session-') as directory:
        root = Path(directory)
        if a.jar:
            (root / 'eula.txt').write_text('eula=true\n')
            (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n')
            argv = ['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui']
            ready = 'Done ('
        else:
            (root / 'mcsa-gameplay-probe.marker').touch()
            (root / 'config').mkdir()
            (root / 'world/playerdata').mkdir(parents=True)
            (root / 'skins').mkdir()
            argv = [str(a.test_binary.resolve()), '--ignored', '--exact', 'gameplay_probe::serve_java_26_3_probe', '--nocapture']
            ready = 'MCSA gameplay probe ready'
        log = root / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)
            client = None
            def command(text):
                server.stdin.write(text + '\n')
                server.stdin.flush()
            try:
                deadline = time.monotonic() + 120
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, 'startup failed'
                    time.sleep(.1)
                client = connect(a.port, protocol, lambda s, p: Probe(s, p, a.version, names), a.version)
                client.pump(10, lambda r: r['name'] == 'position')
                client.pump(1)
                for cmd in ['fill -4 63 -4 4 63 4 stone', 'tp McsaProbe 0.5 64 0.5', 'clear McsaProbe', 'gamemode survival McsaProbe', 'item replace entity McsaProbe weapon.mainhand with ' + item]:
                    command(cmd)
                    client.pump(.4)
                def use(y, sequence):
                    client.send('block_place', b'\0' + struct.pack('>Q', 2 << 38 | 2 << 12 | y) + b'\1' + struct.pack('>fff', .5, .5, .5) + b'\0\0' + varint(sequence))
                    client.pump(.8)
                use(63, 1)
                use(64, 2)
                assert client.windows and client.windows[-1]['type'] == 20, 'shulker did not open'
                expected = [None] * 63
                expected[0] = {'item': 'stone', 'count': 3}
                expected[26] = {'item': 'potion', 'count': 1, 'potion': 13}
                assert client.inventory['slots'] == expected, client.inventory
                report['checks'].append('place-and-open-exact-contents')
                report['snapshots']['opened'] = json.loads(json.dumps(expected))
                def click(place):
                    inv = client.inventory
                    # Проба заявляет пустой курсор: если он действительно пуст,
                    # оригинал не обязан повторять это отдельным пакетом.
                    inv['cursor'] = None
                    client.send('window_click', varint(inv['window']) + varint(inv['revision']) + struct.pack('>hb', place, 0) + b'\0\0\0')
                    client.pump(.4)
                for source, destination in [(0, 27), (26, 28), (27, 0)]:
                    moving = expected[source]
                    click(source)
                    assert client.inventory['cursor'] == moving
                    click(destination)
                    expected[source], expected[destination] = None, moving
                    assert client.inventory['slots'] == expected and client.inventory['cursor'] is None, client.inventory
                    report['checks'].append(f'move-{source}-to-{destination}')
                client.send('close_window', varint(client.inventory['window']))
                client.pump(.3)
                use(64, 3)
                assert client.inventory['slots'] == expected and client.inventory['cursor'] is None
                report['checks'].append('reopen-keeps-contents-without-duplication')
                report['snapshots']['reopened'] = expected
                assert 'Перевод Java-пакета:' not in log.read_text(), 'translation error'
            except BaseException as error:
                report['error'] = str(error)
                raise
            finally:
                if client:
                    report['packets'] = client.observed
                    client.sock.close()
                if server.poll() is None:
                    command('stop')
                    server.stdin.close()
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report['exit_code'] = server.returncode
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
                (a.output / 'server.log').write_text(log.read_text())
    assert report['exit_code'] == 0
    print(json.dumps({'checks': report['checks'], 'exit_code': report['exit_code']}))


if __name__ == '__main__':
    main()
