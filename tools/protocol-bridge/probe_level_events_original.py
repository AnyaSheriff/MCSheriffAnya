#!/usr/bin/env python3
"""Только CI: конечные наблюдения1000/1001/1010/1011 на непрозрачном774/777."""
import argparse
import hashlib
import io
import json
import os
import signal
from pathlib import Path
import socket
import struct
import subprocess
import time
import urllib.request

from probe_gameplay import Client, connect, packed
from smoke import varint, read_varint
from generate_tables import particle

ROOT = Path(__file__).resolve().parents[2]
PIN774_CLIENT = 'ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd'
EXPECTED = {'empty': 1001, 'item': 1000, 'jukebox-start': 1010, 'jukebox-stop': 1011}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def event(payload):
    if len(payload) != 17 or payload[-1] not in (0, 1):
        raise ValueError('invalid fixed level-event payload')
    effect, position, data, global_event = struct.unpack('>iQiB', payload)
    return {'effect': effect, 'packed_position': position, 'data': data, 'global': bool(global_event)}


def validate(rows):
    for phase, effect in EXPECTED.items():
        found = [r for r in rows if r['phase'] == phase and r['name'] == 'world_event'
                 and r['event']['effect'] == effect
                 and r['event']['packed_position'] == int.from_bytes(packed(40 if phase in ('empty','item') else 42,100,40),'big')]
        if not found:
            raise ValueError(f'missing actual{phase}/{effect}')
        if phase != 'jukebox-start' and any(r['event']['data'] != 0 for r in found):
            raise ValueError('expected data0')
    if not any(r['phase'] == 'jukebox-start' and r['name'] == 'world_particles' and r.get('note') is True for r in rows):
        raise ValueError('missing jukebox note packet')


def download(url):
    if not url.startswith('https://'):
        raise ValueError('HTTPS required')
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def pinned(record, path, algorithm='sha256'):
    raw = download(record['url'])
    if len(raw) != record.get('bytes', record.get('size')) or hashlib.new(algorithm, raw).hexdigest() != record[algorithm]:
        raise ValueError('download identity mismatch')
    path.write_bytes(raw)
    return raw


def inputs(version, folder):
    if version == 777:
        pins = json.loads((ROOT/'tools/blackbox/ci/sources-777.json').read_text())
        proto = next(x for x in pins['json_files'] if x['file'] == 'protocol.json')
        raw = pinned(proto, folder/'protocol.json')
        jar = pins['official_jar']
        pinned(jar, folder/'server.jar')
    else:
        proto = json.loads((ROOT/'tools/protocol-bridge/step1-sources-774.json').read_text())['protocol']
        raw = pinned(proto, folder/'protocol.json')
        # Тот же manifest/SHA путь, что у своего prepare_registry_client.py;
        # клиент НЕ скачивается, JAR сервера только исполняется.
        manifest = json.loads(download('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'))
        entry = next(x for x in manifest['versions'] if x['id'] == '1.21.11')
        metadata = download(entry['url'])
        if hashlib.sha1(metadata).hexdigest() != entry['sha1']:
            raise ValueError('official metadata SHA1 mismatch')
        doc = json.loads(metadata)
        if doc['id'] != '1.21.11' or doc['downloads']['client']['sha1'] != PIN774_CLIENT:
            raise ValueError('wrong pinned release metadata')
        (folder/'official-version-metadata.json').write_bytes(metadata)
        jar = doc['downloads']['server']
        pinned(jar, folder/'server.jar', 'sha1')
    return json.loads(raw), {'protocol_sha256': digest(raw), 'jar_sha256': digest((folder/'server.jar').read_bytes()), 'jar_pin': jar}


class Observer(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.version = None
        self.phase = 'join'
        self.selected = []
        self.note_id = particle(protocol)['note']

    def confirm_teleport(self, teleport, values):
        self.send('teleport_confirm', varint(teleport) + (struct.pack('>dddff', *values) if self.version == 777 else b''))

    def observe_packet(self, name, payload):
        if name in ('world_event', 'world_particles', 'kick_disconnect'):
            if len(self.selected) >= 256:
                raise ValueError('selected packet budget exceeded')
            row = {'phase': self.phase, 'name': name, 'payload_hex': payload.hex()}
            if name == 'world_event':
                row['event'] = event(payload)
            elif name == 'world_particles':
                start = 0 if self.version == 777 else 46
                row['particle_id'] = read_varint(io.BytesIO(payload[start:]).read)
                row['note'] = row['particle_id'] == self.note_id
            self.selected.append(row)


def run(version, port, output, java):
    folder = output/str(version)
    folder.mkdir()
    report = {'version': version, 'port': port, 'passed': False, 'commands': [], 'scope': 'official opaque original only; no candidate/whole gameplay claim'}
    process = client = None
    try:
        for checkport in (port, port+1):
            with socket.socket() as check:
                check.bind(('127.0.0.1', checkport))
        protocol, identity = inputs(version, folder)
        report.update(identity)
        (folder/'eula.txt').write_text('eula=true\n')
        (folder/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\ngamemode=creative\nmax-tick-time=0\n')
        with (folder/'server.log').open('w') as log:
            process = subprocess.Popen([java, '-Xmx1G', '-XX:ActiveProcessorCount=2', '-jar', 'server.jar', 'nogui'], cwd=folder, stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
            deadline = time.monotonic()+120
            while 'Done (' not in (folder/'server.log').read_text():
                if process.poll() is not None or time.monotonic()>deadline:
                    raise RuntimeError('original startup failed')
                time.sleep(.1)
            client = connect(port, protocol, Observer, version=version, username='LevelProbe')
            client.version = version
            client.pump(2, lambda r: client.position is not None)
            def command(text):
                report['commands'].append({'phase': client.phase, 'text': text})
                process.stdin.write(text+'\n'); process.stdin.flush(); client.pump(.7)
            command('tp LevelProbe 40.5 101 42.5 180 20')
            command('fill 38 99 38 44 99 44 stone')
            client.phase = 'empty'
            command('setblock 40 100 40 dispenser[facing=east]')
            command('setblock 40 101 40 redstone_block')
            command('setblock 40 101 40 air')
            client.phase = 'item'
            command('item replace block 40 100 40 container.0 with stone 1')
            command('setblock 40 101 40 redstone_block')
            command('setblock 40 101 40 air')
            command('setblock 42 100 40 jukebox')
            command('item replace entity LevelProbe weapon.mainhand with music_disc_cat 1')
            def click(sequence):
                client.send('block_place', b'\0'+packed(42,100,40)+b'\1'+struct.pack('>fff',.5,1.,.5)+b'\0\0'+varint(sequence))
                client.pump(2)
            client.phase = 'jukebox-start'; click(1)
            client.phase = 'jukebox-stop'; click(2)
            validate(client.selected)
            client.pump(1)
            report['passed'] = True
    except Exception as error:
        report['error'] = repr(error)
    finally:
        if client:
            report['selected_packets'] = client.selected
            client.sock.close()
        if process:
            if process.poll() is None:
                process.stdin.write('stop\n'); process.stdin.flush()
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    report['forced_kill'] = True; os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=10)
            report['server_exit'] = process.returncode
            if process.returncode != 0 or report.get('forced_kill'):
                report['passed'] = False
            try:
                os.killpg(process.pid, 0)
                report['process_group_gone'] = False
            except ProcessLookupError:
                report['process_group_gone'] = True
            if not report['process_group_gone']:
                report['passed'] = False
        with socket.socket() as check:
            report['port_free'] = check.connect_ex(('127.0.0.1', port)) != 0
        if not report['port_free']:
            report['passed'] = False
        (folder/'observations.json').write_text(json.dumps(report, indent=2)+'\n')
    return report['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--java', required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('CI only: no local server launch')
    args.output.mkdir(parents=True, exist_ok=False)
    outcomes = [run(v, p, args.output, args.java) for v,p in [(774,26350),(777,26352)]]
    if not all(outcomes):
        raise SystemExit('Original observation failed; retain raw evidence')


if __name__ == '__main__':
    main()
