#!/usr/bin/env python3
"""Own native1.21.3 on OUR opt-in runtime777: bounded stone5/movement/rejoin.

No public admission, packet injection, JAR reading/unpack or fullsupport claim.
"""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time

from capture_native_768 import Observer, PROTOCOL_SHA, CLIENT_SHA1, metadata, opaque_sha, require_runtime, read_handshake, status_exchange
from live_client import launch
from observe import frame
from probe_core_776 import stop_server
from probe_core_776_client import FrameDeadline, free_port, group_pids, stop_client_bounded, gui
from probe_initial_play_776 import capture
from probe_registry_client import exact, read_string, require_end
from smoke import handshake, send, read_varint

MAX_PACKETS=2048
MAX_BYTES=32*1024*1024
sha=lambda raw:hashlib.sha256(raw).hexdigest()


def number(s):
    return read_varint(s.read)


def stone_slot(s,stone):
    count=number(s)
    if count==0:return None
    if not 1<=count<=64 or number(s)!=stone or number(s)!=0 or number(s)!=0:
        raise ValueError('bounded stone-only trusted Slot required')
    return {'item':stone,'count':count}


class CoreObserver(Observer):
    def __init__(self,protocol,stone):
        self.stone=stone;self.slots=None;self.revision=None;self.carried=None;self.moves=[]
        super().__init__(protocol,self.decode_inventory)
    def decode_inventory(self,payload):
        s=io.BytesIO(payload);window=number(s);revision=number(s);count=number(s)
        if window!=0 or count!=46:raise ValueError('real player window0/46 slots required')
        slots=[stone_slot(s,self.stone) for _ in range(46)];carried=stone_slot(s,self.stone);require_end(s,'WindowItems768')
        self.slots=slots;self.revision=revision;self.carried=carried
        return {'window':window,'state':revision,'slots':46,'carried_empty':carried is None}
    def observe(self,direction,body):
        super().observe(direction,body)
        row=self.rows[-1];name=row['name'];s=io.BytesIO(body);number(s)
        if row['phase']=='play' and direction=='toClient' and name=='set_slot':
            window=number(s);revision=number(s);slot=struct.unpack('>h',exact(s,2))[0];stack=stone_slot(s,self.stone);require_end(s,name)
            if window!=0 or self.slots is None or not 0<=slot<46:raise ValueError('SetSlot context')
            self.slots[slot]=stack;self.revision=revision
        if row['phase']=='play' and direction=='toServer' and name in ('position','position_look'):
            pos=struct.unpack('>ddd',exact(s,24))
            if not all(math.isfinite(v) for v in pos):raise ValueError('nonfinite actual native movement')
            self.moves.append(pos)
    def total(self):
        return None if self.slots is None else sum(s['count'] for s in self.slots if s)


def snapshot_files(root):
    # Own test world only. No credentials, foreign assets or runtime files.
    result={}
    for folder in ('world/playerdata','plugins','auth'):
        for p in (root/folder).rglob('*'):
            if p.is_file():result[str(p.relative_to(root))]={'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())}
    return result


def status768(port):
    with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
        handshake(sock,768,port,1);send(sock,b'\0');raw=frame(FrameDeadline(sock,10));data=io.BytesIO(raw)
        if number(data)!=0:raise ValueError('Status response ID')
        value=json.loads(read_string(data));require_end(data,'Status')
        if value['version']!={'name':'26.3','protocol':777}:raise ValueError('primary Status must remain honest777')
        return {'version':value['version'],'body_hex':raw.hex(),'sha256':sha(raw)}

def reject_without_optin(binary,port,output,extra_args=(),source_address=None):
    with tempfile.TemporaryDirectory(prefix='mcsa-private768-closed-') as tmp:
        root=Path(tmp);configure(root,port)
        report={};server=None
        with output.open('w') as log:
            try:
                server=subprocess.Popen([str(binary),*extra_args],cwd=root,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                wait_server(server,output)
                before=snapshot_files(root)
                report['status']=status768(port)
                with socket.create_connection(('127.0.0.1',port),timeout=10,source_address=source_address) as s:
                    handshake(s,768,port,2)
                    raw=frame(FrameDeadline(s,10));reader=io.BytesIO(raw)
                    if number(reader)!=0:raise ValueError('default shipping flagless768 must refuse BEFORE LoginStart')
                    reason=read_string(reader);require_end(reader,'closed Login disconnect')
                    if not reason:raise ValueError('missing closed reason')
                    report['disconnect_hex']=raw.hex()
                time.sleep(.2);after=snapshot_files(root)
                if before!=after:raise ValueError('closed admission changed playerdata/plugins/auth')
                report['before']=before;report['after']=after
            finally:
                if server:report['stop']=stop_server(server);report['exit']=server.returncode;report['group_gone']=not group_pids(server.pid)
                report['port_free']=free_port(port)
        if report['exit']!=0 or not report['group_gone'] or not report['port_free']:raise ValueError('closed runtime cleanup')
        return report


def configure(root,port):
    (root/'config').mkdir()
    (root/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\ndebug=true\n')
    (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
    (root/'config/server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-animals=false\nspawn-monsters=false\n')


def wait_server(server,path):
    deadline=time.monotonic()+120
    while 'Готово (' not in path.read_text():
        if server.poll() is not None or time.monotonic()>deadline:raise RuntimeError('our runtime startup/deadline')
        time.sleep(.1)


def run_session(cache,java,server,port,relay,output,protocol,stone,index):
    output.mkdir();observer=CoreObserver(protocol,stone);report={'status':'failed','index':index,'packets':[],'errors':[]}
    peers=[];workers=[];ending=threading.Event();lock=threading.Lock();client=listener=None
    def forward(source,target,direction):
        try:
            while not ending.is_set():
                body=frame(FrameDeadline(source,45))
                with lock:
                    total=report.get('ordinary_bytes',0)+len(body)
                    if len(observer.rows)>=MAX_PACKETS or total>MAX_BYTES:raise ValueError('capture budget')
                    try:observer.observe(direction,body)
                    except Exception:
                        report['rejected_packet']={'phase':observer.phase,'direction':direction,'body_hex':body.hex(),'sha256':sha(body)}
                        raise
                    observer.rows[-1].update(body_hex=body.hex(),body_sha256=sha(body));report['ordinary_bytes']=total
                    send(target,body)
        except Exception as e:
            if not ending.is_set():report['errors'].append(direction+': '+repr(e))
    def await_condition(condition,label,seconds=60):
        deadline=time.monotonic()+seconds
        while True:
            if report['errors'] or client.poll() is not None or server.poll() is not None:raise RuntimeError(label+' process/relay failed')
            with lock:done=condition()
            if done:return
            if time.monotonic()>deadline:raise TimeoutError(label)
            time.sleep(.05)
    with tempfile.TemporaryDirectory(prefix='mcsa-private768-client-') as game_tmp,(output/'client.log').open('w') as log:
        game=Path(game_tmp)
        try:
            listener=socket.socket();listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);listener.bind(('127.0.0.1',relay));listener.listen(4);listener.settimeout(120)
            client=launch(cache,game,relay,log,version='1.21.3',java=java,gui_scale=2,pause_on_lost_focus=False)
            peer,_=listener.accept();peers.append(peer);backend=socket.create_connection(('127.0.0.1',port),timeout=10);peers.append(backend)
            initial=frame(FrameDeadline(peer,30));report['handshake_hex']=initial.hex()
            if read_handshake(initial,relay)!=2:raise ValueError('expected normal Login connection')
            send(backend,initial)
            for source,target,direction in ((peer,backend,'toServer'),(backend,peer,'toClient')):
                w=threading.Thread(target=forward,args=(source,target,direction),daemon=True);workers.append(w);w.start()
            await_condition(lambda:observer.complete(),'native actual Login/Config/Play/46slots readiness')
            with lock:
                expected=0 if index==1 else 5
                if observer.total()!=expected or observer.carried is not None:raise ValueError('empty first / stone5 persisted rejoin inventory')
                report['initial_inventory']=list(observer.slots);report['initial_inventory_total']=observer.total()
            time.sleep(8)
            report['initial_frame_sha256']=capture(client,output/'initial-world.png')
            if index==1:
                server.stdin.write('give McsaProbe stone 5\n');server.stdin.flush()
                await_condition(lambda:observer.total()==5 and observer.carried is None,'actual authoritative stone5')
                with lock:report['stone_inventory']=list(observer.slots);before=observer.moves[-1] if observer.moves else None;start=len(observer.moves)
                if before is None:raise ValueError('missing actual initial movement')
                gui(client,'keydown','w')
                try:time.sleep(.5)
                finally:gui(client,'keyup','w')
                await_condition(lambda:any(abs(p[0]-before[0])+abs(p[2]-before[2])>.1 for p in observer.moves[start:]),'actual native W movement')
                time.sleep(1);report['stone_frame_sha256']=capture(client,output/'stone5-world.png')
                report['movement_before']=before
            if report['errors'] or any(text in (output/'client.log').read_text() for text in ('Failed to handle packet','Failed to load chunk')):raise RuntimeError('actual native decode failure')
            report['completed']=True
        except Exception as e:
            report['errors'].append(repr(e))
            if client is not None and client.poll() is None:
                try:report['failure_frame_sha256']=capture(client,output/'failure.png')
                except Exception as capture_error:report['failure_frame_error']=repr(capture_error)
            else:report['failure_frame_error']='client already exited; no live own display'
        finally:
            ending.set()
            for peer in peers:
                try:peer.shutdown(socket.SHUT_RDWR)
                except OSError:pass
                peer.close()
            if listener:listener.close()
            for w in workers:w.join(timeout=5)
            if client:
                try:stop_client_bounded(client)
                except Exception as e:report['errors'].append('client stop: '+repr(e))
            report['workers_stopped']=all(not w.is_alive() for w in workers)
            report['client_group_gone']=not group_pids(client.pid) if client else True
            report['relay_port_free']=free_port(relay);report['packets']=observer.rows;report['teleport_acks']=len(observer.acks)
            report['status']='pass' if report.get('completed') and not report['errors'] and all(report[k] for k in ('workers_stopped','client_group_gone','relay_port_free')) else 'failed'
            (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    if report['status']!='pass':raise RuntimeError('native private session failed; inspect report')
    return {k:v for k,v in report.items() if k!='packets'}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','shipping-binary','client-dir','metadata','protocol','items','java','output'):p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--server-port',type=int,default=26270);p.add_argument('--relay-port',type=int,default=26271)
    p.add_argument('--allow-normal-native-loading',action='store_true');a=p.parse_args();require_runtime(a.allow_normal_native_loading)
    if not (26000<=a.server_port<26998 and a.relay_port==a.server_port+1):raise ValueError('isolated two test ports required')
    metadata(a.metadata);opaque_sha(a.client_dir/'client.jar',CLIENT_SHA1)
    protocolraw=a.protocol.read_bytes()
    if sha(protocolraw)!=PROTOCOL_SHA:raise ValueError('pinned protocol768')
    locks=json.loads((Path(__file__).parent/'client-chain-sources.json').read_text())['versions']['768']['files']
    if sha(a.items.read_bytes())!=locks['items']['sha256']:raise ValueError('pinned items768')
    stone=next(row['id'] for row in json.loads(a.items.read_text()) if row['name']=='stone')
    a.output.mkdir(parents=True,exist_ok=False);binary=a.binary.resolve();report={'status':'failed','binary_sha256':sha(binary.read_bytes()),'sessions':[],'errors':[]}
    server=None
    try:
        report['shipping_rejection']=reject_without_optin(a.shipping_binary.resolve(),a.server_port,a.output/'shipping.log',('--private-768',))
        report['shipping_binary_sha256']=sha(a.shipping_binary.read_bytes())
        report['flagless_rejection']=reject_without_optin(binary,a.server_port,a.output/'flagless.log')
        report['nonloopback_rejection']=reject_without_optin(binary,a.server_port,a.output/'nonloopback.log',('--private-768',),('10.254.68.2',0))
        with tempfile.TemporaryDirectory(prefix='mcsa-private768-core-') as tmp,(a.output/'server.log').open('w') as log:
            root=Path(tmp);configure(root,a.server_port)
            try:
                server=subprocess.Popen([str(binary),'--private-768'],cwd=root,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                wait_server(server,a.output/'server.log')
                report['primary_status']=status768(a.server_port)
                for index in (1,2):
                    row=run_session(a.client_dir.resolve(),a.java,server,a.server_port,a.relay_port,a.output/f'session{index}',json.loads(protocolraw),stone,index)
                    report['sessions'].append(row);time.sleep(2)
                report['playerdata_after']=snapshot_files(root)
            finally:
                if server:report['stop']=stop_server(server);report['server_exit']=server.returncode;report['server_group_gone']=not group_pids(server.pid)
                report['ports_free']=all(free_port(p) for p in (a.server_port,a.relay_port))
        if report['server_exit']!=0 or not report['server_group_gone'] or not report['ports_free']:raise ValueError('runtime lifecycle failed')
        report['status']='pass'
    except Exception as e:report['errors'].append(repr(e))
    finally:(a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report))
    if report['status']!='pass':raise RuntimeError('private768 actual native acceptance failed')

if __name__=='__main__':main()
