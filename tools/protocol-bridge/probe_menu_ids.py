#!/usr/bin/env python3
"""ID меню из обычных взаимодействий официального сервера, без чтения реализации."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import socket
import subprocess
import tempfile
import time
from probe_particles import Probe as ParticleProbe
from probe_gameplay import connect
from smoke import read_varint, varint

class Probe(ParticleProbe):
    def observe_packet(self, name, payload):
        if name in ('open_window',):
            self.observed.append({'name': name, 'body': (varint(next(k for k,v in self.incoming.items() if v == name)) + payload).hex()})

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--version',type=int,choices=[775,777],required=True)
    p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();port=26028
    expected={775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==expected
    sources=json.loads((a.data/'sources.json').read_text())
    for entry in sources['files']+sources['baseline_files']:
        assert hashlib.sha256((a.data/entry['file']).read_bytes()).hexdigest()==entry['sha256']
    protocol=json.loads((a.data/('protocol.json' if a.version==777 else 'protocol-26.1.json')).read_text())
    with socket.socket() as check:check.bind(('127.0.0.1',port))
    a.output.mkdir(parents=True,exist_ok=True)
    report={'version':a.version,'jar_sha1':expected,'menus':[]}
    with tempfile.TemporaryDirectory(prefix='mcsa-menus-') as directory:
        root=Path(directory);(root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n')
        log=root/'server.log'
        with log.open('w') as output:
            server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(a.jar.resolve()),'nogui'],cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
            client=None
            def command(text):server.stdin.write(text+'\n');server.stdin.flush()
            try:
                deadline=time.monotonic()+120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline,'startup failed';time.sleep(.1)
                client=connect(port,protocol,lambda s,p:Probe(s,p,a.version),a.version)
                client.pump(10,lambda r:r['name']=='position');client.pump(.3)
                command('tp McsaProbe 0.5 80 0.5');client.pump(.3)
                command('fill -4 79 -4 4 79 4 stone');client.pump(.3)
                blocks=['chest','barrel','dispenser','dropper','hopper','crafting_table','furnace','blast_furnace','smoker','brewing_stand','enchanting_table','anvil','beacon','cartography_table','grindstone','loom','smithing_table','stonecutter','shulker_box']
                for sequence,block in enumerate(blocks,1):
                    command('setblock 2 80 2 air');command(f'setblock 2 80 2 {block}');client.pump(.2)
                    start=len(client.observed)
                    client.send('block_place',b'\0'+struct.pack('>Q',2<<38|2<<12|80)+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(sequence))
                    client.pump(5,lambda r:r['name']=='open_window')
                    packet=client.observed[start:][-1];raw=io.BytesIO(bytes.fromhex(packet['body']));read_varint(raw.read)
                    window=read_varint(raw.read);kind=read_varint(raw.read)
                    report['menus'].append({'block':block,'type':kind,**packet})
                    client.send('close_window',varint(window));client.pump(.1)
            except BaseException as e:report['error']=str(e);raise
            finally:
                if client:client.sock.close()
                if server.poll() is None:
                    command('stop');server.stdin.close()
                    try:server.wait(timeout=30)
                    except subprocess.TimeoutExpired:server.kill();server.wait()
                report['exit_code']=server.returncode
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n');(a.output/'server.log').write_text(log.read_text())
    assert report['exit_code']==0
    print(json.dumps({'menus':len(report['menus']),'exit_code':report['exit_code']}))
if __name__=='__main__':main()
