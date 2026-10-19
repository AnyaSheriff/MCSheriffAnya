#!/usr/bin/env python3
"""Настоящий340-клиент в Xvfb на обычном Play ядра; прозрачная запись своих пакетов."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time

from live_client import stop
from live_legacy_client import launch
from observe import frame
from probe_gameplay import mapping
from probe_legacy_session import string
from smoke import read_varint, send


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--test-binary', type=Path, required=True)
    p.add_argument('--client-dir', type=Path, required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--proxy-port', type=int, default=26028)
    a = p.parse_args()
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / 'protocol.json').read_text())
    names = {direction: {v:k for k,v in mapping(protocol,'play',direction).items()} for direction in ['toServer','toClient']}
    report = {'checks': [], 'packets': [], 'complete_gameplay': False}
    lock = threading.Lock(); stopped = threading.Event(); phase = threading.Event()
    peers = []; workers = []
    a.output.mkdir(parents=True,exist_ok=True)
    server_log = a.output / 'server.log'; client_log = a.output / 'client.log'
    with tempfile.TemporaryDirectory(prefix='mcsa-legacy-core-') as directory, socket.socket() as listener:
        root=Path(directory); core=root/'core'; game=root/'client'; core.mkdir(); game.mkdir()
        (core/'mcsa-gameplay-probe.marker').touch()
        listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
        listener.bind(('127.0.0.1',a.proxy_port)); listener.listen(1); listener.settimeout(30)
        with server_log.open('w') as slog, client_log.open('w') as clog:
            server=subprocess.Popen([str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::serve_java_1_12_2_probe','--nocapture'],cwd=core,stdin=subprocess.PIPE,stdout=slog,stderr=subprocess.STDOUT,text=True)
            client=None

            def command(value):
                server.stdin.write(value+'\n');server.stdin.flush()

            def relay(source,target,direction):
                try:
                    while not stopped.is_set():
                        body=frame(source); data=io.BytesIO(body); ident=read_varint(data.read); payload=data.read()
                        play=phase.is_set()
                        record={'direction':direction,'id':ident,'bytes':len(payload)}
                        if not play and direction=='toClient':
                            assert ident==2, f'Unexpected Login packet {ident}'
                            record['name']='login_success';phase.set()
                        else:
                            name=names[direction].get(ident,f'unknown-{ident}');record['name']=name
                            data=io.BytesIO(payload)
                            if name=='teleport_confirm': record['teleport']=read_varint(data.read)
                            elif name=='keep_alive': record['token']=struct.unpack('>q',payload)[0]
                            elif name=='position' and direction=='toClient':
                                record['position']=list(struct.unpack('>dddff',data.read(32)))
                                record['flags']=data.read(1)[0];record['teleport']=read_varint(data.read)
                            elif name in ('position','position_look') and direction=='toServer':
                                record['position']=list(struct.unpack('>ddd',payload[:24]))
                            elif name=='map_chunk':
                                record['chunk']=list(struct.unpack('>ii',payload[:8]));record['sha256']=hashlib.sha256(payload).hexdigest()
                        with lock: report['packets'].append(record)
                        send(target,body)
                except BaseException as error:
                    if not stopped.is_set():
                        with lock: report.setdefault('relay_errors',[]).append(f'{direction}: {type(error).__name__}: {error}')

            def wait(predicate,seconds=30):
                deadline=time.monotonic()+seconds
                while time.monotonic()<deadline:
                    log=server_log.read_text()
                    assert server.poll() is None,'core exited'
                    assert 'probe connection:' not in log and 'MCSA play probe failure:' not in log, log[-1500:]
                    assert client is None or client.poll() is None,'client exited'
                    with lock:
                        if predicate(report['packets']): return
                    time.sleep(.05)
                raise AssertionError('Timed out waiting for client evidence')

            try:
                wait(lambda _: 'MCSA gameplay probe ready on 26027' in server_log.read_text())
                client=launch(a.client_dir.resolve(),game,a.proxy_port,clog)
                peer,_=listener.accept();peers.append(peer)
                backend=socket.create_connection(('127.0.0.1',26027),timeout=10);backend.settimeout(None);peers.append(backend)
                hello=frame(peer);data=io.BytesIO(hello)
                assert read_varint(data.read)==0 and read_varint(data.read)==340
                string(data);data.read(2);assert read_varint(data.read)==2 and not data.read()
                send(backend,hello);login=frame(peer);data=io.BytesIO(login)
                assert read_varint(data.read)==0 and string(data)=='McsaProbe' and not data.read()
                send(backend,login);report['checks'].append('actual-client-login340')
                for source,target,direction in [(backend,peer,'toClient'),(peer,backend,'toServer')]:
                    worker=threading.Thread(target=relay,args=(source,target,direction),daemon=True);workers.append(worker);worker.start()
                wait(lambda rows:any(r.get('name')=='map_chunk' for r in rows) and any(r.get('name')=='teleport_confirm' for r in rows))
                command('say MCSA-legacy-runtime-ready')
                wait(lambda _: 'MCSA-legacy-runtime-ready' in client_log.read_text())
                report['checks'].append('actual-client-world-and-chat')
                command('tp McsaProbe 1.5 64 1.5')
                wait(lambda rows:len({r.get('teleport') for r in rows if r.get('name')=='teleport_confirm'})>=2)
                report['checks'].append('second-core-teleport-ack')
                wait(lambda rows:any(r.get('name')=='position_look' and r['direction']=='toServer' and all(abs(v-e)<.01 for v,e in zip(r['position'],[1.5,128,1.5])) for r in rows))
                report['checks'].append('client-position-matches-world-window')
                wait(lambda rows:bool({r['token'] for r in rows if r.get('name')=='keep_alive' and r['direction']=='toClient'} & {r['token'] for r in rows if r.get('name')=='keep_alive' and r['direction']=='toServer'}),25)
                report['checks'].append('keepalive-after-world-loaded')
                assert not report.get('relay_errors'), report.get('relay_errors')
                assert not any(word in client_log.read_text() for word in ['DecoderException','ReportedException','Unexpected error'])
            except BaseException as error:
                report['error']=str(error);raise
            finally:
                stopped.set()
                if client is not None: stop(client)
                for peer in peers:
                    try: peer.shutdown(socket.SHUT_RDWR)
                    except OSError: pass
                    peer.close()
                for worker in workers: worker.join(timeout=2)
                if server.poll() is None:
                    command('stop')
                    server.stdin.close()
                    try: server.wait(timeout=10)
                    except subprocess.TimeoutExpired: server.kill();server.wait()
                report['exit_code']=server.returncode
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0
    print(json.dumps({'checks':report['checks'],'packets':len(report['packets'])}))


if __name__=='__main__':main()
