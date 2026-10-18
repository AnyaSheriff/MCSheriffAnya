#!/usr/bin/env python3
"""Сравнение SHA256 первых25 родных чанков и начального инвентаря двух сборок нашего ядра."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import time
from probe_gameplay import Client, connect
from smoke import varint

class Probe(Client):
    def __init__(self,sock,protocol):
        super().__init__(sock,protocol);self.chunks={};self.inventory=[]
    def confirm_teleport(self,teleport,values):self.send('teleport_confirm',varint(teleport))
    def observe_packet(self,name,payload):
        if name=='map_chunk':
            x,z=struct.unpack('>ii',payload[:8]);self.chunks.setdefault(f'{x},{z}',hashlib.sha256(payload).hexdigest())
        elif name in ('window_items','set_slot','set_cursor_item','set_player_inventory'):
            self.inventory.append((name,hashlib.sha256(payload).hexdigest()))

def observe(binary,protocol,output,port):
    output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='mcsa-native-chunks-') as folder:
        root=Path(folder);(root/'config').mkdir()
        (root/'config/server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
        (root/'config/mcsa.properties').write_text('auth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
        log=output/'server.log';report={}
        with log.open('w') as out:
            server=subprocess.Popen([str(binary.resolve())],cwd=root,stdin=subprocess.PIPE,stdout=out,stderr=subprocess.STDOUT,text=True)
            client=None
            try:
                deadline=time.monotonic()+30
                while 'Готово (' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(.05)
                client=connect(port,protocol,Probe,version=775)
                deadline=time.monotonic()+15
                while len(client.chunks)<25 or not client.inventory:
                    assert time.monotonic()<deadline, f'only {len(client.chunks)} chunks'
                    client.pump(.2)
                report['chunks']=client.chunks
                report['inventory']=client.inventory
            finally:
                if client:client.sock.close()
                if server.poll() is None:
                    server.stdin.write('stop\n');server.stdin.flush()
                    try:server.wait(timeout=30)
                    except subprocess.TimeoutExpired:server.kill();server.wait()
                report['exit_code']=server.returncode
                (output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0
    return {'chunks':report['chunks'],'inventory':report['inventory']}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline',type=Path,required=True);p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--port',type=int,default=26021);a=p.parse_args()
    path=a.data/'protocol-26.1.json'
    manifest=json.loads((a.data/'sources.json').read_text())
    assert hashlib.sha256(path.read_bytes()).hexdigest()==next(f['sha256'] for f in manifest['baseline_files'] if f['file']==path.name)
    protocol=json.loads(path.read_text())
    old=observe(a.baseline,protocol,a.output/'baseline',a.port)
    new=observe(a.binary,protocol,a.output/'current',a.port)
    assert old==new,'native chunk/inventory bytes changed'
    report={'equal_native_chunks':len(old['chunks']),'equal_native_inventory':len(old['inventory']),'baseline':str(a.baseline),'current':str(a.binary)}
    (a.output/'comparison.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))

if __name__=='__main__':main()
