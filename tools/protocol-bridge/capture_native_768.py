#!/usr/bin/env python3
"""Непрозрачный exact1.21.3 → настоящий isolated reference server; не adapter proof.

Без --execute читает только metadata JSON. JAR не распаковываются, registries/chunks
не выгружаются. Реле не вставляет/не меняет пакеты. Запуск — только в выделенном
root слоте/CI; публичный admission ядра и его JavaVersion здесь не участвуют.
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
import threading
import time
import uuid

MAX_ORDINARY_PACKETS = 2048
MAX_ORDINARY_BYTES = 32 * 1024 * 1024

from live_client import launch
from observe import frame
from probe_core_776_client import FrameDeadline, free_port, group_pids, stop_client_bounded
from probe_gameplay import mapping
from probe_initial_play_776 import capture
from probe_registry_client import exact, known_packs, read_string, require_end
from smoke import handshake, send

PROTOCOL_SHA = 'abcd44462eebc703fea2423572719d79b16e92ad398d94ab3235c5a193656865'
METADATA_SHA1 = '5fa4228474e3d59b829637553262b6c06c0a38cf'
CLIENT_SHA1 = '6f67d19b4467240639cb2c368ffd4b94ba889705'
SERVER_SHA1 = '45810d238246d90e811d896f87b14695b7fb6839'
RAW = {'success', 'login_start', 'login_acknowledged', 'finish_configuration',
       'position', 'teleport_confirm', 'player_info', 'window_items', 'set_slot',
       'set_cursor_item', 'set_player_inventory', 'chunk_batch_received'}


def uint(stream):
    # Переиспользуем exact; ограничиваем unsigned IDs, не signed action payloads.
    value = 0
    for index in range(5):
        byte = exact(stream, 1)[0]
        if index == 4 and byte & 0xf8:
            raise ValueError('overflow unsigned protocol ID')
        value |= (byte & 127) << (index * 7)
        if byte < 128:
            return value
    raise ValueError('truncated/oversized VarInt')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def empty_inventory(payload):
    source = io.BytesIO(payload)
    window, state, count = uint(source), uint(source), uint(source)
    if window != 0 or count != 46:
        raise ValueError('expected initial player window0 with46 slots')
    for _ in range(count + 1):
        if uint(source) != 0:
            raise ValueError('initial inventory contains an item; not trusted-empty capture')
    require_end(source, 'empty WindowItems768')
    return {'window':window, 'state':state, 'slots':count, 'carried_empty':True}


class Observer:
    """Общий lock удерживается до forwarding ACK, как в прежнем own GUI relay."""
    def __init__(self, protocol):
        self.names = {p:{d:{v:k for k,v in mapping(protocol,p,d).items()}
                       for d in ('toClient','toServer')}
                      for p in ('login','configuration','play')}
        self.phase = 'login'
        self.rows, self.teleports, self.acks = [], set(), set()
        self.success = self.started = self.finished = False
        self.offered = self.reply = None
        self.inventory = None
        self.raw_bytes = 0
        self.began = time.monotonic()

    def observe(self, direction, body):
        if direction not in ('toClient','toServer') or len(body) > 8*1024*1024 or len(self.rows) >= 8192:
            raise ValueError('capture budget/direction')
        source = io.BytesIO(body); ident = uint(source); payload = source.read()
        name = self.names[self.phase][direction].get(ident)
        if name is None or name in ('disconnect','kick_disconnect','transfer','start_configuration','compress','encryption_begin'):
            raise ValueError(('unexpected phase packet', self.phase, direction, ident, name))
        row = {'phase':self.phase,'direction':direction,'id':ident,'name':name,
               'bytes':len(payload),'sha256':digest(payload),'elapsed':time.monotonic()-self.began}
        self.rows.append(row)
        if name in RAW and not (self.phase == 'play' and name == 'position' and direction == 'toServer'):
            if len(payload)>1024*1024 or self.raw_bytes+len(payload)>8*1024*1024:
                raise ValueError('selected raw payload budget')
            self.raw_bytes += len(payload); row['hex'] = payload.hex()
        data = io.BytesIO(payload)
        if self.phase == 'login':
            if direction == 'toServer' and name == 'login_start':
                if self.started or read_string(data) != 'McsaProbe': raise ValueError('synthetic profile required')
                exact(data,16); require_end(data,name); self.started=True
            elif direction == 'toClient' and name == 'success':
                expected=uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(),version=3).bytes
                if not self.started or self.success or exact(data,16)!=expected or read_string(data)!='McsaProbe' or uint(data)!=0:
                    raise ValueError('unexpected offline LoginSuccess768')
                require_end(data,name); self.success=True
            elif direction == 'toServer' and name == 'login_acknowledged':
                if payload or not self.success: raise ValueError('invalid Login ACK')
                self.phase='configuration'
            else: raise ValueError('unexpected Login packet')
        elif self.phase == 'configuration':
            if name == 'select_known_packs':
                packs=known_packs(data); row['packs']=[list(p) for p in packs]
                if direction=='toClient':
                    if self.offered is not None: raise ValueError('duplicate packs offer')
                    self.offered=packs
                else:
                    if self.offered is None or self.reply is not None or packs not in ([],self.offered):
                        raise ValueError('unexpected known-packs reply')
                    self.reply=packs
            elif name == 'finish_configuration':
                if payload or self.reply is None: raise ValueError('invalid Config Finish')
                if direction=='toClient': self.finished=True
                elif self.finished: self.phase='play'
                else: raise ValueError('Finish ACK before Finish')
        elif direction == 'toClient':
            if name=='position':
                teleport=uint(data); values=struct.unpack('>ddddddffI',exact(data,60)); require_end(data,name)
                if teleport in self.teleports or not all(math.isfinite(v) for v in values[:-1]) or values[-1]&~0x1ff:
                    raise ValueError('invalid/duplicate teleport768')
                self.teleports.add(teleport); row['teleport']=teleport
            elif name=='window_items':
                self.inventory=empty_inventory(payload); row['inventory']=self.inventory
        elif name=='teleport_confirm':
            teleport=uint(data); require_end(data,name)
            if teleport not in self.teleports or teleport in self.acks: raise ValueError('unknown/duplicate ACK')
            self.acks.add(teleport); row['teleport']=teleport

    def complete(self):
        seen={(r['direction'],r['name']) for r in self.rows if r['phase']=='play'}
        return bool(self.phase=='play' and self.acks and self.inventory and
                    {('toClient','login'),('toClient','player_info'),('toClient','map_chunk'),
                     ('toServer','chunk_batch_received')} <= seen and
                    any(d=='toServer' and n in ('position','position_look','look','flying') for d,n in seen))


def validate_ports(port, relay):
    if type(port) is not int or type(relay) is not int or port==relay or not all(26000<=p<27000 for p in (port,relay)):
        raise ValueError('two distinct isolated ports26000..26999 required')


def metadata(path):
    raw=path.read_bytes()
    if hashlib.sha1(raw).hexdigest()!=METADATA_SHA1: raise ValueError('exact official metadata pin required')
    value=json.loads(raw)
    if value['id']!='1.21.3' or value['downloads']['client']['sha1']!=CLIENT_SHA1 or value['downloads']['server']['sha1']!=SERVER_SHA1:
        raise ValueError('exact1.21.3 inputs required')
    return value


def opaque_sha(path, expected):
    if path.is_symlink() or path.suffix!='.jar': raise ValueError('explicit regular opaque JAR required')
    if '/paper/' in str(path.resolve()): raise ValueError('forbidden source location')
    with path.open('rb') as stream:
        if hashlib.file_digest(stream,'sha1').hexdigest()!=expected: raise ValueError('opaque input SHA1 mismatch')


def require_runtime(allow_normal_native_loading=False):
    """Узкое owner-разрешение: только штатный runtime opaque клиента на GitHub."""
    if allow_normal_native_loading is not True:
        raise RuntimeError('BLOCKED_NATIVE_RUNTIME: explicit --allow-normal-native-loading required')
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        raise RuntimeError('native client execution is GitHub ONLY')



def read_handshake(body, relay_port):
    data=io.BytesIO(body)
    if uint(data)!=0 or uint(data)!=768 or read_string(data)!='127.0.0.1' or struct.unpack('>H',exact(data,2))[0]!=relay_port:
        raise ValueError('opaque client handshake mismatch')
    state=uint(data);require_end(data,'Handshake')
    if state not in (1,2):raise ValueError('unsupported initial state')
    return state


def status_exchange(peer, backend, report):
    """QuickPlay status/ping только прозрачно пересылается, не выдаётся за Login."""
    def record(direction,body):
        if len(report.setdefault('prelogin_packets',[]))>=24 or len(body)>65536:
            raise ValueError('prelogin status budget')
        report['prelogin_packets'].append({'direction':direction,'body_hex':body.hex(),'sha256':digest(body)})
    request=frame(FrameDeadline(peer,30))
    if request!=b'\0':raise ValueError('expected ordinary status request')
    record('toServer',request);send(backend,request)
    response=frame(FrameDeadline(backend,30));data=io.BytesIO(response)
    if uint(data)!=0:raise ValueError('expected original status response')
    value=json.loads(read_string(data));require_end(data,'Status')
    if value['version']!={'name':'1.21.3','protocol':768}:raise ValueError('original QuickPlay Status mismatch')
    record('toClient',response);send(peer,response)
    try:ping=frame(FrameDeadline(peer,30))
    except EOFError:return
    if len(ping)!=9 or ping[:1]!=b'\1':raise ValueError('expected ordinary Ping')
    record('toServer',ping);send(backend,ping)
    pong=frame(FrameDeadline(backend,30))
    if pong!=ping:raise ValueError('original Pong mismatch')
    record('toClient',pong);send(peer,pong)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--metadata',type=Path,required=True)
    p.add_argument('--protocol',type=Path,required=True)
    p.add_argument('--client-dir',type=Path,required=True)
    p.add_argument('--server-jar',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--java',type=Path,default=Path('/usr/lib/jvm/java-25-openjdk/bin/java'))
    p.add_argument('--server-port',type=int,default=26268)
    p.add_argument('--relay-port',type=int,default=26269)
    p.add_argument('--execute',action='store_true')
    p.add_argument('--allow-normal-native-loading',action='store_true',help='Owner narrow Nov10 exception: normal opaque runtime loading only, GitHub ONLY')
    a=p.parse_args(); validate_ports(a.server_port,a.relay_port); meta=metadata(a.metadata)
    protocol_raw=a.protocol.read_bytes()
    if digest(protocol_raw)!=PROTOCOL_SHA: raise ValueError('pinned approved protocol768 JSON required')
    if not a.execute:
        print(json.dumps({'status':'plan_only','version':meta['id'],'protocol':768,
            'server_jar_present':a.server_jar.is_file(),'client_cache_present':(a.client_dir/'client.jar').is_file(),
            'executed':False,'scope':'native reference only; no core adapter/admission evidence'})); return
    require_runtime(a.allow_normal_native_loading)
    opaque_sha(a.server_jar,SERVER_SHA1); opaque_sha(a.client_dir/'client.jar',CLIENT_SHA1)
    metadata(a.client_dir/'version.json')
    for port in (a.server_port,a.relay_port):
        with socket.socket() as check: check.bind(('127.0.0.1',port))
    a.output.mkdir(parents=True,exist_ok=False)
    report={'status':'failed','scope':'native1.21.3 reference only; core768 NOT exercised',
            'metadata_sha1':METADATA_SHA1,'protocol_sha256':PROTOCOL_SHA,'client_sha1':CLIENT_SHA1,
            'server_sha1':SERVER_SHA1,'runner_sha256':digest(Path(__file__).read_bytes()),'manual_jar_unpack':False,
            'normal_native_loading_authorized':True,
            'packet_injection':False,'special_registry_request':False,'ordinary_network_raw':True,
            'limits':{'packets':MAX_ORDINARY_PACKETS,'bytes':MAX_ORDINARY_BYTES},'errors':[]}
    observer=Observer(json.loads(protocol_raw)); ending=threading.Event(); lock=threading.Lock()
    peers=[]; workers=[]; client=server=listener=None
    def relay(source,target,direction):
        try:
            while not ending.is_set():
                body=frame(FrameDeadline(source,45))
                with lock:
                    total=report.get('ordinary_bytes',0)+len(body)
                    if len(observer.rows)>=MAX_ORDINARY_PACKETS or total>MAX_ORDINARY_BYTES:
                        raise ValueError('ordinary network capture budget exceeded')
                    observer.observe(direction,body)
                    observer.rows[-1]['body_hex']=body.hex()
                    observer.rows[-1]['body_sha256']=digest(body)
                    report['ordinary_bytes']=total
                    send(target,body)
        except Exception as error:
            if not ending.is_set(): report['errors'].append(direction+': '+repr(error))
    with tempfile.TemporaryDirectory(prefix='mcsa-native768-capture-') as directory:
        root=Path(directory); core=root/'server'; game=root/'client'; core.mkdir();game.mkdir()
        (core/'eula.txt').write_text('eula=true\n')
        (core/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.server_port}\nonline-mode=false\n'
            'enforce-secure-profile=false\nwhite-list=false\nnetwork-compression-threshold=-1\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n'
            'spawn-protection=0\nmax-players=2\n')
        with (a.output/'server.log').open('w') as slog,(a.output/'client.log').open('w') as clog:
            try:
                server=subprocess.Popen([str(a.java),'-Xmx1G','-jar',str(a.server_jar.resolve()),'nogui'],cwd=core,
                    stdin=subprocess.PIPE,stdout=slog,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                deadline=time.monotonic()+120
                while 'Done (' not in (a.output/'server.log').read_text():
                    if server.poll() is not None or time.monotonic()>deadline: raise RuntimeError('reference startup failed/deadline')
                    time.sleep(.1)
                with socket.create_connection(('127.0.0.1',a.server_port),timeout=10) as status:
                    handshake(status,768,a.server_port,1);send(status,b'\0')
                    raw=io.BytesIO(frame(FrameDeadline(status,10)))
                    if uint(raw)!=0: raise ValueError('unexpected Status packet')
                    value=json.loads(read_string(raw));require_end(raw,'Status');report['status_version']=value['version']
                    if value['version']!={'name':'1.21.3','protocol':768}: raise ValueError('reference Status mismatch')
                listener=socket.socket();listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);listener.bind(('127.0.0.1',a.relay_port));listener.listen(4);listener.settimeout(120)
                client=launch(a.client_dir.resolve(),game,a.relay_port,clog,version='1.21.3',gui_scale=2,pause_on_lost_focus=False,java=a.java)
                for attempt in range(4):
                    peer,_=listener.accept();peers.append(peer)
                    backend=socket.create_connection(('127.0.0.1',a.server_port),timeout=10);peers.append(backend)
                    initial=frame(FrameDeadline(peer,30))
                    report.setdefault('initial_handshakes',[]).append({'body_hex':initial.hex(),'sha256':digest(initial)})
                    state=read_handshake(initial,a.relay_port);send(backend,initial)
                    if state==2:break
                    status_exchange(peer,backend,report);peer.close();backend.close()
                else:raise ValueError('Login not reached after bounded QuickPlay status sessions')
                report['handshake_protocol']=768
                for source,target,direction in ((peer,backend,'toServer'),(backend,peer,'toClient')):
                    worker=threading.Thread(target=relay,args=(source,target,direction),daemon=True)
                    workers.append(worker);worker.start()
                deadline=time.monotonic()+120
                while True:
                    if report['errors'] or client.poll() is not None or server.poll() is not None: raise RuntimeError('reference/client/relay failed')
                    with lock: complete=observer.complete()
                    if complete: break
                    if time.monotonic()>deadline: raise TimeoutError('initial opaque E2E criteria incomplete')
                    time.sleep(.05)
                report['screenshot_sha256']=capture(client,a.output/'initial-play.png')
                time.sleep(2)
                if report['errors'] or 'Failed to handle packet' in (a.output/'client.log').read_text():
                    raise RuntimeError('client decode/relay failed after initial Play')
                report['capture_complete']=True
            except Exception as error:
                report['errors'].append(repr(error))
            finally:
                ending.set()
                for peer in peers:
                    try: peer.shutdown(socket.SHUT_RDWR)
                    except OSError: pass
                    peer.close()
                if listener: listener.close()
                for worker in workers: worker.join(timeout=5)
                report['workers_stopped']=all(not worker.is_alive() for worker in workers)
                if client:
                    try: stop_client_bounded(client)
                    except Exception as error: report['errors'].append('client cleanup: '+repr(error))
                if server and server.poll() is None:
                    try:
                        server.stdin.write('stop\n');server.stdin.flush();server.wait(timeout=45)
                    except subprocess.TimeoutExpired:
                        os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=5);report['errors'].append('server required SIGKILL')
                report['process_groups_gone']=all(not group_pids(p.pid) for p in (client,server) if p)
                report['ports_free']=all(free_port(p) for p in (a.server_port,a.relay_port))
                report['server_exit']=server.returncode if server else None
                report['packets']=observer.rows
                report['status']='pass' if report.get('capture_complete') and report['server_exit']==0 and not report['errors'] and all(report[k] for k in ('workers_stopped','process_groups_gone','ports_free')) else 'failed'
                (a.output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    if report['status']!='pass': raise RuntimeError('native768 capture FAILED; inspect report.json')
    print(json.dumps({k:v for k,v in report.items() if k!='packets'}))


if __name__=='__main__': main()
