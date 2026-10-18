#!/usr/bin/env python3
"""Полная игровая сессия через настоящие обработчики ядра в отдельном test binary."""
import argparse
import hashlib
import io
import json
import shutil
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
from probe_gameplay import Client,connect
from smoke import read_varint,varint

def slot(data):
    count=read_varint(data.read)
    if not count:return None
    item=read_varint(data.read);added=read_varint(data.read);removed=read_varint(data.read)
    for _ in range(added):
        kind=read_varint(data.read)
        assert kind==3, f'unexpected component in simple session fixture: {kind}'
        read_varint(data.read)
    for _ in range(removed):read_varint(data.read)
    return {'item':item,'count':count}

class Probe(Client):
    def __init__(self,sock,protocol):
        super().__init__(sock,protocol);self.names=[];self.inventory=None;self.windows=[]
    def observe_packet(self,name,payload):
        self.names.append(name)
        if name=='window_items':
            data=io.BytesIO(payload);window=read_varint(data.read);revision=read_varint(data.read)
            slots=[slot(data) for _ in range(read_varint(data.read))];cursor=slot(data)
            assert not data.read(),'trailing inventory data'
            self.inventory={'window':window,'revision':revision,'slots':slots,'cursor':cursor}
        elif name=='open_window':
            data=io.BytesIO(payload);self.windows.append({'window':read_varint(data.read),'type':read_varint(data.read)})

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--test-binary',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--join-only',action='store_true');p.add_argument('--client-dir',type=Path);a=p.parse_args()
    protocol=json.loads((a.data/'protocol.json').read_text())
    sources=json.loads((a.data/'sources.json').read_text());entry=next(v for v in sources['files'] if v['file']=='protocol.json')
    assert hashlib.sha256((a.data/'protocol.json').read_bytes()).hexdigest()==entry['sha256']
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);check.bind(('127.0.0.1',26026))
    a.output.mkdir(parents=True,exist_ok=True);report={'checks':[],'test_binary_sha256':hashlib.sha256(a.test_binary.read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='mcsa-full-session-') as directory:
        root=Path(directory);(root/'mcsa-gameplay-probe.marker').touch();(root/'config').mkdir();(root/'world/playerdata').mkdir(parents=True);(root/'skins').mkdir()
        log=root/'server.log'
        with log.open('w') as output:
            server=subprocess.Popen([str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::serve_java_26_3_probe','--nocapture'],cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
            client=None;graphical=None;client_log=None
            def command(text):server.stdin.write(text+'\n');server.stdin.flush()
            def pump(seconds):
                if client:client.pump(seconds)
                else:
                    time.sleep(seconds)
                    assert graphical.poll() is None,'official client exited'
                    assert 'потерял соединение' not in log.read_text(),'official client disconnected'
                    assert not list((root/'client/debug').glob('disconnect-*.txt')),'client decode failure'
            try:
                deadline=time.monotonic()+30
                while 'MCSA gameplay probe ready' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline,'test server startup failed';time.sleep(.05)
                if a.client_dir:
                    from live_client import launch
                    (root/'client').mkdir();client_log=(a.output/'client.log').open('w')
                    graphical=launch(a.client_dir.resolve(),root/'client',26026,client_log)
                    deadline=time.monotonic()+120
                    while 'зашёл на сервер' not in log.read_text():
                        assert time.monotonic()<deadline,'official client did not join';pump(.2)
                    pump(3)
                else:
                    client=connect(26026,protocol,Probe,777)
                    client.pump(10,lambda r:r['name']=='position');client.pump(3)
                assert 'зашёл на сервер' in log.read_text(),'full join did not finish'
                report['checks'].append('full-join')
                if not a.join_only:
                    for cmd in ['gamemode creative McsaProbe','tp McsaProbe 0.5 66 0.5','give McsaProbe red_wool 5','give McsaProbe diamond_sword','effect give McsaProbe speed 100 1','summon cow 2 64 2','time set noon','weather clear']:
                        command(cmd);pump(1);report['checks'].append(cmd)
                    if client:
                        wool=next(x['id'] for x in json.loads((a.data/'items.json').read_text()) if x['name']=='red_wool')
                        # Ложное предсказание пустого курсора не заменяет расчёт сервера.
                        for place in [36,38]:
                            before=client.names.count('window_items')
                            client.send('window_click',varint(0)+varint(client.inventory['revision'])+struct.pack('>hb',place,0)+b'\0\0\0')
                            pump(.5);assert client.names.count('window_items')>before,'no authoritative inventory resync'
                        assert client.inventory['slots'][36] is None
                        assert client.inventory['slots'][38]=={'item':wool,'count':5}
                        assert client.inventory['cursor'] is None
                        report['checks'].append('click-authoritative-inventory')
                        command('tp McsaProbe 0.5 64 0.5');pump(.3)
                        command('setblock 2 64 2 chest');pump(.3)
                        client.send('block_place',b'\0'+struct.pack('>Q',2<<38|2<<12|64)+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0\1')
                        pump(.5);assert client.windows and client.windows[-1]['type']==2,'chest did not open'
                        client.send('close_window',varint(client.windows[-1]['window']));pump(.3)
                        report['checks'].append('open-close-chest')
                assert 'Перевод Java-пакета:' not in log.read_text(),'translation failure'
            except BaseException as e:report['error']=str(e);raise
            finally:
                if client:report['packets']=client.names;report['inventory']=client.inventory;client.sock.close()
                if graphical:
                    from live_client import stop
                    stop(graphical)
                    for folder in ['debug','crash-reports','logs']:
                        if (root/'client'/folder).exists():shutil.copytree(root/'client'/folder,a.output/folder,dirs_exist_ok=True)
                if client_log:client_log.close()
                if server.poll() is None:
                    command('stop');server.stdin.close()
                    try:server.wait(timeout=15)
                    except subprocess.TimeoutExpired:server.kill();server.wait()
                report['exit_code']=server.returncode
                (a.output/'server.log').write_text(log.read_text());(a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0, 'test server did not stop cleanly'
    print(json.dumps({'checks':report['checks'],'exit_code':report['exit_code']}))
if __name__=='__main__':main()
