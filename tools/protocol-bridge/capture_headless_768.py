#!/usr/bin/env python3
"""Обычный TCP вход собственного768 клиента в opaque exact1.21.3 server.

Только нормальные пакеты входа; bounded network recordings, не reports/dumps.
Не запускает GUI/native client и не снимает его отдельный runtime gate.
"""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import tempfile
import time
import uuid

from capture_native_768 import (Observer, PROTOCOL_SHA, SERVER_SHA1, METADATA_SHA1,
                                uint, metadata, opaque_sha, digest)
from observe import frame
from probe_core_776_client import FrameDeadline, free_port, group_pids
from probe_registry_client import exact, require_end, string
from smoke import handshake, send, varint

MAX_PACKETS = 2048
MAX_BYTES = 32 * 1024 * 1024


class Recording:
    def __init__(self, protocol):
        self.observer = Observer(protocol)
        self.rows = []
        self.bytes = 0
        self.registries = []

    def add(self, direction, body):
        if len(self.rows) >= MAX_PACKETS or self.bytes + len(body) > MAX_BYTES:
            raise ValueError('ordinary network recording budget exceeded')
        phase = self.observer.phase
        self.observer.observe(direction, body)
        observed = self.observer.rows[-1]
        row = {k: v for k, v in observed.items() if k != 'hex'}
        row['phase'] = phase
        row['body_hex'] = body.hex()
        row['body_sha256'] = digest(body)
        self.rows.append(row)
        self.bytes += len(body)
        if phase == 'configuration' and direction == 'toClient' and observed['name'] == 'registry_data':
            self.registries.append(row['body_sha256'])
        return observed['name'], body[len(varint(observed['id'])):]

    def ready(self):
        return self.observer.complete() and bool(self.registries)


def valid_port(port):
    if type(port) is not int or not 26000 <= port < 27000:
        raise ValueError('isolated port26000..26999 required')


def pose(payload):
    data = io.BytesIO(payload)
    teleport = uint(data)
    values = struct.unpack('>ddddddffI', exact(data, 60))
    require_end(data, 'position768')
    if values[-1] != 0 or not all(math.isfinite(v) for v in values[:-1]):
        raise ValueError('initial absolute finite position required')
    return teleport, (*values[:3], *values[6:8])


def session(port, protocol, recording):
    with socket.create_connection(('127.0.0.1', port), timeout=10) as sock:
        handshake(sock, 768, port, 2)
        def outgoing(name, payload=b''):
            phase = recording.observer.phase
            ids = {v:k for k,v in recording.observer.names[phase]['toServer'].items()}
            body = varint(ids[name]) + payload
            recording.add('toServer', body)
            send(sock, body)
        identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(), version=3)
        outgoing('login_start', string('McsaProbe') + identity.bytes)
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            name, payload = recording.add('toClient', frame(FrameDeadline(sock, min(20, deadline-time.monotonic()))))
            phase = recording.rows[-1]['phase']
            if phase == 'login':
                if name != 'success': raise ValueError('expected uncompressed offline LoginSuccess')
                outgoing('login_acknowledged')
                outgoing('settings', string('en_us') + b'\x02\x00\x01\x7f\x01\x00\x01\x00')
            elif phase == 'configuration':
                if name == 'select_known_packs':
                    if recording.observer.offered != [('minecraft','core','1.21.3')]:
                        raise ValueError('unexpected original known packs')
                    # Обычный клиент вправе не иметь встроенного pack: server sends registry values.
                    outgoing(name, b'\0')
                elif name == 'keep_alive':
                    if len(payload) != 8: raise ValueError('bad config keepalive')
                    outgoing(name, payload)
                elif name == 'ping':
                    if len(payload) != 4: raise ValueError('bad config ping')
                    outgoing('pong', payload)
                elif name == 'finish_configuration': outgoing(name)
            else:
                if name == 'position':
                    teleport, values = pose(payload)
                    outgoing('teleport_confirm', varint(teleport))
                    outgoing('position_look', struct.pack('>dddffB', *values, 0))
                elif name == 'chunk_batch_finished':
                    data=io.BytesIO(payload)
                    if not 1 <= uint(data) <= 1024: raise ValueError('invalid chunk batch size')
                    require_end(data, name)
                    outgoing('chunk_batch_received', struct.pack('>f', 8.0))
                elif name == 'keep_alive':
                    if len(payload) != 8: raise ValueError('bad play keepalive')
                    outgoing(name, payload)
                elif name == 'ping':
                    if len(payload) != 4: raise ValueError('bad play ping')
                    outgoing('pong', payload)
                if recording.ready(): return
        raise TimeoutError('headless768 ordinary entry criteria incomplete')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('metadata','protocol','server-jar','output','java'):
        p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--port',type=int,default=26268)
    a=p.parse_args();valid_port(a.port);metadata(a.metadata);opaque_sha(a.server_jar,SERVER_SHA1)
    raw=a.protocol.read_bytes()
    if digest(raw)!=PROTOCOL_SHA: raise ValueError('pinned approved protocol768 required')
    if not free_port(a.port): raise ValueError('isolated port occupied')
    a.output.mkdir(parents=True,exist_ok=False)
    recording=Recording(json.loads(raw))
    report={'status':'failed','scope':'own headless768 TCP client / opaque original1.21.3 only; NOT native GUI/core adapter proof',
            'server_sha1':SERVER_SHA1,'metadata_sha1':METADATA_SHA1,'protocol_sha256':PROTOCOL_SHA,
            'runner_sha256':digest(Path(__file__).read_bytes()),'limits':{'packets':MAX_PACKETS,'bytes':MAX_BYTES,'entry_seconds':60},
            'gui_client_executed':False,'native_gui_gate':'BLOCKED_NATIVE_RUNTIME','errors':[]}
    server=None
    with tempfile.TemporaryDirectory(prefix='mcsa-headless768-') as tmp:
        root=Path(tmp)
        (root/'eula.txt').write_text('eula=true\n')
        properties=(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\n'
            'enforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\n'
            'use-native-transport=false\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\n'
            'simulation-distance=2\nspawn-protection=0\nmax-players=2\n')
        (root/'server.properties').write_text(properties)
        report['properties_sha256']=digest(properties.encode())
        with (a.output/'server.log').open('w') as log:
            try:
                # JNA/Netty fallback extraction disabled; no foreign JAR inspected by harness.
                server=subprocess.Popen([str(a.java),'-Xmx1G','-Djna.nounpack=true','-Dio.netty.noNative=true',
                    '-jar',str(a.server_jar.resolve()),'nogui'],cwd=root,stdin=subprocess.PIPE,
                    stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                deadline=time.monotonic()+150
                while 'Done (' not in (a.output/'server.log').read_text():
                    if server.poll() is not None or time.monotonic()>deadline: raise RuntimeError('opaque server startup/deadline')
                    time.sleep(.1)
                with socket.create_connection(('127.0.0.1',a.port),timeout=10) as sock:
                    handshake(sock,768,a.port,1);send(sock,b'\0')
                    data=io.BytesIO(frame(FrameDeadline(sock,10)))
                    if uint(data)!=0: raise ValueError('bad status')
                    from probe_registry_client import read_string
                    status=json.loads(read_string(data));require_end(data,'status')
                    report['status_version']=status['version']
                    if status['version']!={'name':'1.21.3','protocol':768}:raise ValueError('original status mismatch')
                session(a.port,json.loads(raw),recording)
                report['entry_complete']=True
            except Exception as error:
                report['errors'].append(repr(error))
            finally:
                if server and server.poll() is None:
                    try:
                        server.stdin.write('stop\n');server.stdin.flush();server.wait(timeout=45)
                    except (BrokenPipeError,subprocess.TimeoutExpired):
                        os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=5);report['errors'].append('forced server stop')
                report.update(server_exit=server.returncode if server else None,
                    process_group_gone=not group_pids(server.pid) if server else True,
                    port_free=free_port(a.port),packets=recording.rows,registry_packets=len(recording.registries),
                    ordinary_bytes=recording.bytes)
                report['status']='pass' if (report.get('entry_complete') and not report['errors'] and
                    report['server_exit']==0 and report['process_group_gone'] and report['port_free']) else 'failed'
                (a.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k!='packets'}))
    if report['status']!='pass':raise RuntimeError('headless768 capture failed')

if __name__=='__main__':main()
