#!/usr/bin/env python3
"""Сетевая запись Commands/подсказок: JAR исполняется в собственной копии."""
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

from probe_gameplay import Client, connect
from smoke import read_varint, varint
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'measure'))


def string(data):
    return data.read(read_varint(data.read)).decode()


def tree(payload):
    data = io.BytesIO(payload)
    nodes = []
    for _ in range(read_varint(data.read)):
        flags = data.read(1)[0]
        node = dict(flags=flags, children=[read_varint(data.read) for _ in range(read_varint(data.read))])
        if flags & 8:
            node['redirect'] = read_varint(data.read)
        if flags & 3:
            node['name'] = string(data)
        if flags & 3 == 2:
            parser = node['parser'] = read_varint(data.read)
            start = data.tell()
            if parser in (1, 2, 3, 4):
                mask = data.read(1)[0]
                data.read(((mask & 1) + ((mask >> 1) & 1)) * (8 if parser in (2, 4) else 4))
            elif parser == 5:
                read_varint(data.read)
            elif parser in (6, 31):
                data.read(1)
            elif parser == 43:
                data.read(4)
            elif parser in (44, 45, 46, 47, 48):
                string(data)
            # Дополнительные ID проверяются границами всего графа и EOF.
            # [проверить] Имя нового parserID не выводится из его номера.
            assert parser <= 63, parser
            end = data.tell()
            node['properties'] = payload[start:end].hex()
            if flags & 16:
                node['suggestion'] = string(data)
        nodes.append(node)
    root = read_varint(data.read)
    assert 0 <= root < len(nodes)
    for node in nodes:
        assert node['flags'] & ~63 == 0 and node['flags'] & 3 != 3
        assert all(0 <= child < len(nodes) for child in node['children'])
        if 'redirect' in node: assert 0 <= node['redirect'] < len(nodes)
        if 'name' in node: assert node['name'] and not any(ord(ch) < 32 for ch in node['name'])
    assert not data.read()
    return dict(nodes=nodes, root=root)


def nbt(data, kind):
    if kind in (1, 2, 3, 4, 5, 6):
        data.read({1:1, 2:2, 3:4, 4:8, 5:4, 6:8}[kind])
    elif kind in (7, 11, 12):
        count = int.from_bytes(data.read(4), 'big')
        data.read(count * {7:1, 11:4, 12:8}[kind])
    elif kind == 8:
        data.read(int.from_bytes(data.read(2), 'big'))
    elif kind == 9:
        inner = data.read(1)[0]
        for _ in range(int.from_bytes(data.read(4), 'big')):
            nbt(data, inner)
    elif kind == 10:
        while (inner := data.read(1)[0]):
            data.read(int.from_bytes(data.read(2), 'big'))
            nbt(data, inner)
    else:
        raise ValueError(kind)


class Probe(Client):
    def __init__(self, sock, protocol, version=777):
        super().__init__(sock, protocol)
        self.version = version
        self.trees = []
        self.answers = {}
        self.output = None
    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + (struct.pack('>dddff', *values) if self.version == 777 else b''))
    def observe_packet(self, name, payload):
        if name == 'declare_commands':
            if self.output:
                (self.output / f'tree-{len(self.trees)}.bin').write_bytes(payload)
            self.trees.append(payload)
        elif name == 'tab_complete':
            if self.output:
                with (self.output / 'suggestions.raw.jsonl').open('a') as raw:
                    raw.write(json.dumps(dict(payload=payload.hex())) + '\n')
            data = io.BytesIO(payload)
            request, start, length, count = [read_varint(data.read) for _ in range(4)]
            matches = []
            for _ in range(count):
                value = string(data)
                tooltip = data.read(1)[0]
                start_tooltip = data.tell()
                if tooltip:
                    nbt(data, data.read(1)[0])
                matches.append(dict(text=value, tooltip=payload[start_tooltip:data.tell()].hex() if tooltip else None))
            assert not data.read()
            self.answers[request] = dict(start=start, length=length, matches=matches, raw=payload.hex())


def main():
    from pregen import Rcon
    output = Path(sys.argv[1]); output.mkdir(parents=True, exist_ok=True)
    binary = Path(sys.argv[4]).resolve() if len(sys.argv) > 4 else None
    port = 26231 if binary else 26131
    source = Path('/home/lenox/.local/share/mcsa-java-26.3-probe')
    version = int(sys.argv[2]) if len(sys.argv) > 2 else 777
    jar = source / ('server.jar' if version == 777 else 'server-26.1.2.jar')
    assert binary or hashlib.sha1(jar.read_bytes()).hexdigest() == {777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c',775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51'}[version]
    protocol = json.loads((source / ('candidate-json/protocol.json' if version == 777 else 'candidate-json/protocol-26.1.json')).read_text())
    for bound_port in (port, port + 1):
        with socket.socket() as check:
            check.bind(('127.0.0.1', bound_port))
    samples = ['/','/g','/gamemode ', '/gamemode c', '/gamemode creative ', '/gamemode invalid ',
      '/give ', '/give M', '/give McsaProbe ', '/give McsaProbe sto', '/give McsaProbe stone ',
      '/tp ', '/tp M', '/tp McsaProbe ', '/tp McsaProbe ~ ', '/tp McsaProbe ~ ~ ',
      '/effect ', '/effect give ', '/effect give McsaProbe ', '/effect give McsaProbe sp',
      '/effect give McsaProbe speed ', '/effect clear McsaProbe ', '/enchant ', '/enchant McsaProbe ',
      '/enchant McsaProbe sh', '/enchant McsaProbe sharpness ', '/summon ', '/summon zo',
      '/summon minecraft:zombie ', '/execute ', '/execute as ', '/execute as McsaProbe ',
      '/execute as McsaProbe run ', '/execute as McsaProbe run gamemode ', '/execute if ',
      '/execute if entity ', '/execute in ', '/kill ', '/kill @', '/kill @e[', '/kill @e[type=',
      '/kill @e[type=z', '/time ', '/time set ', '/time set n', '/time query ', '/weather ',
      '/weather rain ', '/gamerule ', '/gamerule minecraft:keep_inventory ', '/gamerule minecraft:spawn_mobs ',
      '/op ', '/op M', '/deop ', '/deop M', '/kick ', '/kick M', '/ban ', '/ban M', '/pardon ',
      '/say ', '/say M', '/msg ', '/msg M', '/msg McsaProbe ', '/tell ', '/tell McsaProbe hello ',
      '/playsound ', '/stopsound McsaProbe ', '/stopsound McsaProbe master ', '/attribute McsaProbe ',
      '/locate biome ', '/locate structure ', '/particle ', '/difficulty ', '/help ', '/list ',
      '/experience ', '/experience add ', '/recipe give McsaProbe ', '/setblock ~ ~ ~ ',
      '/fill ~ ~ ~ ~ ~ ~ ', '/data ', '/scoreboard ', '/team ', '/tag McsaProbe ', '/bossbar ',
      '/trigger ', '/function ', '/schedule ', '/return ', '/worldborder ', '/tick ', '/whitelist ',
      '/save-all ', '/defaultgamemode ', '/spawnpoint ', '/random ', '/ride ', '/rotate ', '/spectate ']
    if len(sys.argv) > 3 and sys.argv[3] == 'heldout':
        samples = ['/kill @e[type=#', '/kill @e[type=zombie', '/kill @e[type=minecraft:zombie', '/kill @e[type=!zombie', '/kill @e[type=#minecraft:boat', '/kill @e[type=#minecraft:z', '/kill @e[type=zombie,', '/kill @e[sort=nearest', '/kill @e[limit=1', '/give McsaProbe m', '/give McsaProbe stone[', '/playsound block.stone', '/msg @', '/msg @a ', '/gamemode invalid', '/gamemode 1 ', '/execute as @e run give @s sto']
    report = dict(protocol=version, jar_sha1=None if binary else hashlib.sha1(jar.read_bytes()).hexdigest(), roles={})
    if binary:
        with binary.open('rb') as stream: report['binary_sha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    with tempfile.TemporaryDirectory(prefix='mcsa-commands-') as directory:
        root = Path(directory)
        if not binary: shutil.copyfile(jar, root / 'server.jar')
        (root / 'eula.txt').write_text('eula=true\n')
        properties_root = root
        if binary:
            properties_root = root / 'config'; properties_root.mkdir()
            (properties_root / 'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
            (properties_root / 'skins.toml').write_text('mojang=false\nely=false\n')
        (properties_root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nenable-rcon=true\nrcon.port={port+1}\nrcon.password=commands-probe\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nmax-tick-time=0\n')
        with (output / 'server.log').open('w') as log:
            argv = [str(binary)] if binary else ['/usr/lib/jvm/java-25-openjdk/bin/java', '-Xmx1G', '-jar', 'server.jar', 'nogui']
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE if binary else subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, text=True)
            client = None
            try:
                deadline = time.monotonic() + 120
                while True:
                    assert server.poll() is None and time.monotonic() < deadline
                    if binary:
                        try:
                            with socket.create_connection(('127.0.0.1', port), timeout=.2): pass
                            break
                        except OSError: pass
                    elif 'Done (' in (output / 'server.log').read_text(): break
                    time.sleep(.2)
                rcon = None if binary else Rcon(port + 1, 'commands-probe')
                client = connect(port, protocol, lambda sock, p: Probe(sock, p, version), version=version)
                client.output = output
                client.pump(2)
                for role in ('nonop', 'op4'):
                    if role == 'op4':
                        previous = len(client.trees)
                        if binary:
                            server.stdin.write('op McsaProbe\n'); server.stdin.flush()
                        else: rcon.run('op McsaProbe')
                        client.pump(4, lambda packet: len(client.trees) > previous)
                    assert client.trees
                    raw = client.trees[-1]
                    try:
                        parsed = tree(raw)
                    except (AssertionError, ValueError, IndexError, UnicodeDecodeError) as error:
                        parsed = dict(decode_error=str(error))
                    (output / (role + '.bin')).write_bytes(raw)
                    entry = dict(tree=parsed, suggestions=[])
                    report['roles'][role] = entry
                    queries = list(samples)
                    if role == 'op4' and not (len(sys.argv) > 3 and sys.argv[3] == 'heldout'):
                        for prefix in ('/give McsaProbe ', '/setblock ~ ~ ~ ', '/playsound ', '/recipe give McsaProbe '):
                            queries.extend(prefix + 'minecraft:' + char for char in 'abcdefghijklmnopqrstuvwxyz')
                        queries.extend(['/kill @e[type=!', '/kill @e[type=#', '/kill @e[sort=', '/kill @e[gamemode=', '/kill @e[limit=', '/tp @s ', '/tp @s ~ ', '/tp @s ~ ~ ', '/tp ~ ', '/tp ~ ~ '])
                    for text in queries:
                        request = len(client.answers) + 1
                        encoded = text.encode()
                        client.send('tab_complete', varint(request) + varint(len(encoded)) + encoded)
                        client.pump(3, lambda record: request in client.answers)
                        entry['suggestions'].append(dict(input=text, **client.answers[request]))
                    (output / 'official.json').write_text(json.dumps(report, indent=2) + '\n')
                if rcon: rcon.sock.close()
            finally:
                if client: client.sock.close()
                server.terminate()
                try: server.wait(30)
                except subprocess.TimeoutExpired: server.kill(); server.wait()
    print(json.dumps({role: dict(nodes=len(entry['tree'].get('nodes', [])), suggestions=len(entry['suggestions'])) for role,entry in report['roles'].items()}))


if __name__ == '__main__':
    main()
