#!/usr/bin/env python3
"""Проверка реальной книги рецептов775/776/777 в отдельном мире26.3."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

from probe_gameplay import Client, connect
from recipe_book_decoder import Decoder
from smoke import varint


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--data775', type=Path, required=True)
    parser.add_argument('--data776', type=Path, required=True)
    parser.add_argument('--data777', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--port', type=int, default=26261)
    args = parser.parse_args()
    assert 26200 <= args.port <= 26299
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    root = out / 'server'
    config = root / 'config'
    config.mkdir(parents=True)
    (config / 'server.properties').write_text(
        f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
        'network-compression-threshold=-1\nlevel-type=minecraft:flat\nview-distance=2\n')
    (config / 'mcsa.properties').write_text(
        'java-version=26.3\nauth-enabled=false\nauth-premium=false\n'
        'bedrock-port=0\nsend-error-reports=false\ndebug=true\nprefetch-chunks=0\n')
    (config / 'skins.toml').write_text('mojang=false\nely=false\n')
    report = {'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'clients': []}
    with (out / 'server.log').open('w') as log:
        process = subprocess.Popen([str(args.binary.resolve())], cwd=root, stdin=subprocess.PIPE,
                                   stdout=log, stderr=subprocess.STDOUT, text=True)
        def command(text):
            process.stdin.write(text + '\n')
            process.stdin.flush()
        client = None
        try:
            deadline = time.monotonic() + 100
            while 'Готово (' not in (out / 'server.log').read_text():
                assert process.poll() is None and time.monotonic() < deadline, 'startup failed'
                time.sleep(.1)
            for version in (775, 776, 777):
                data = getattr(args, f'data{version}')
                protocol = json.loads((data / 'protocol.json').read_text())
                names = {v['id']: v['name'] for v in json.loads((data / 'items.json').read_text())}
                row = {'protocol': version, 'packets': []}
                class Probe(Client):
                    def confirm_teleport(self, teleport, values):
                        if version == 777:
                            super().confirm_teleport(teleport, values)
                        else:
                            self.send('teleport_confirm', varint(teleport))

                    def observe_packet(self, name, payload):
                        if name == 'recipe_book_add':
                            entries, replace = Decoder(payload, names, string_tags=version == 775).book()
                            path = out / f'{version}-recipe-{len(row["packets"])}.bin'
                            path.write_bytes(payload)
                            row['packets'].append({'entries': len(entries), 'replace': replace,
                                                   'sha256': hashlib.sha256(payload).hexdigest()})
                username = f'Registry{version}'
                client = connect(args.port, protocol, Probe, version=version, username=username, login_uuid=bytes([version % 256]) * 16)
                client.pump(3)
                for text in (f'give {username} white_wool', f'give {username} poplar_log', f'recipe give {username} *'):
                    command(text)
                    client.pump(2)
                assert row['packets'] and sum(p['entries'] for p in row['packets']) >= 1600, row
                row['no_disconnect'] = True
                report['clients'].append(row)
                client.sock.close()
                client = None
            command('stop')
            process.wait(timeout=40)
            assert process.returncode == 0
        finally:
            if client:
                client.sock.close()
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=30)
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'protocols': [r['protocol'] for r in report['clients']], 'no_disconnect': True}))


if __name__ == '__main__':
    main()
