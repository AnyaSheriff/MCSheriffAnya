#!/usr/bin/env python3
"""Наблюдаем metadata двух своих игроков на официальном340, только обычный вход."""
import argparse
import hashlib
import json
import io
from pathlib import Path
import socket
import subprocess
import tempfile
import time

from probe_legacy_session import Probe
from smoke import handshake, read_varint, send, varint


class Player(Probe):
    stage = 'join'

    def observe_packet(self, name, payload):
        if name == 'entity_metadata' and read_varint(io.BytesIO(payload).read) != self.report.get('actor_entity'):
            return
        if name in ('named_entity_spawn', 'entity_metadata'):
            self.report.setdefault('observations', []).append({'stage': self.stage, 'packet': name, 'hex': payload.hex()})


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--jar', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int, default=26023)
    a = p.parse_args()
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == '886945bfb2b978778c3a0288fd7fab09d315b25f'
    manifest = json.loads((a.data / 'sources.json').read_text())
    for row in manifest['files']:
        assert hashlib.sha256((a.data / row['file']).read_bytes()).hexdigest() == row['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    a.output.mkdir(parents=True, exist_ok=True)
    report = {'protocol': 340, 'packets': [], 'checks': []}
    sockets = []
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-metadata-') as directory:
        root = Path(directory)
        (root / 'eula.txt').write_text('eula=true\n')
        (root / 'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=FLAT\nlevel-seed=1\nview-distance=2\ngamemode=1\nspawn-protection=0\n')
        log = a.output / 'server.log'
        with log.open('w') as output:
            server = subprocess.Popen(['/usr/lib/jvm/java-8-openjdk/jre/bin/java', '-Xmx1G', '-jar', str(a.jar.resolve()), 'nogui'], cwd=root, stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, text=True)

            def command(value):
                server.stdin.write(value + '\n'); server.stdin.flush()

            def join(name, result):
                sock = socket.create_connection(('127.0.0.1', a.port), timeout=10); sockets.append(sock)
                handshake(sock, 340, a.port, 2); send(sock, b'\0' + varint(len(name)) + name.encode())
                player = Player(sock, protocol, result, name)
                player.pump(20, lambda row: row['name'] == 'position'); player.pump(.2)
                return player

            try:
                deadline = time.monotonic() + 120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(.1)
                command('gamerule doMobSpawning false')
                command('weather clear')
                watcher = join('McsaProbe', report)
                actor = join('McsaActor', {'packets': []})
                command('tp McsaProbe 0.5 65 0.5 0 0'); command('tp McsaActor 2.5 65 0.5 0 0')
                watcher.pump(.3); actor.pump(.3); watcher.pump(.5)
                entity = next(row['entity'] for row in actor.report['packets'] if row['name'] == 'login')
                report['actor_entity'] = entity
                for stage, skin, hand in [('left-no-skin', 0, 0), ('right-all-skin', 127, 1)]:
                    watcher.stage = stage
                    actor.send('settings', b'\x05en_us\x02\x00\x01' + bytes([skin, hand]))
                    watcher.pump(.4)
                for stage, action in [('sneaking', 0), ('standing', 1)]:
                    watcher.stage = stage
                    actor.send('entity_action', varint(entity) + varint(action) + b'\0')
                    watcher.pump(.4)
                watcher.stage = 'glowing'
                command('effect McsaActor minecraft:glowing 60 0 true'); watcher.pump(.5)
                watcher.stage = 'invisible'
                command('effect McsaActor minecraft:invisibility 60 0 true'); watcher.pump(.5)
                seen = {row['stage'] for row in report.get('observations', [])}
                assert {'left-no-skin', 'right-all-skin', 'sneaking', 'standing', 'glowing', 'invisible'} <= seen, seen
                report['checks'].append('observed-all-six-player-metadata-changes')
            except BaseException as error:
                report['error'] = str(error); raise
            finally:
                for sock in sockets: sock.close()
                if server.poll() is None:
                    command('stop'); server.stdin.close()
                    try: server.wait(timeout=30)
                    except subprocess.TimeoutExpired: server.kill(); server.wait()
                report['exit_code'] = server.returncode
                (a.output / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    assert report['exit_code'] == 0
    print(json.dumps({'checks': report['checks'], 'observations': len(report['observations'])}))


if __name__ == '__main__': main()
