#!/usr/bin/env python3
"""ID эффектов из обычных команд официального сервера, без чтения реализации."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time
from probe_particles import Probe as ParticleProbe
from probe_gameplay import connect
from smoke import read_varint, varint

class Probe(ParticleProbe):
    def observe_packet(self, name, payload):
        if name in ('entity_effect', 'remove_entity_effect', 'sound_effect', 'entity_update_attributes'):
            self.observed.append({'name': name, 'body': (varint(next(k for k,v in self.incoming.items() if v == name)) + payload).hex()})

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--version',type=int,choices=[775,777],required=True)
    p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True)
    p.add_argument('--effects',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--attributes',action='store_true',help='проверить /attribute base set вместо эффектов')
    a=p.parse_args();port=26027
    expected={775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==expected
    sources=json.loads((a.data/'sources.json').read_text())
    for entry in sources['files']+sources['baseline_files']:
        assert hashlib.sha256((a.data/entry['file']).read_bytes()).hexdigest()==entry['sha256']
    protocol=json.loads((a.data/('protocol.json' if a.version==777 else 'protocol-26.1.json')).read_text())
    effects=json.loads(a.effects.read_text())
    with socket.socket() as check:check.bind(('127.0.0.1',port))
    a.output.mkdir(parents=True,exist_ok=True)
    report={'version':a.version,'jar_sha1':expected,'effects':[]}
    with tempfile.TemporaryDirectory(prefix='mcsa-effects-') as directory:
        root=Path(directory);(root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\n')
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
                if a.attributes:
                    report['attribute_updates']=[]
                    for name,value in [('movement_speed',.25),('max_health',30),('scale',1.25)]:
                        start=len(client.observed);command(f'attribute McsaProbe minecraft:{name} base set {value}');client.pump(.3)
                        packets=[r for r in client.observed[start:] if r['name']=='entity_update_attributes'];assert packets,name
                        report['attribute_updates'].append({'name':name,'value':value,'packets':packets})
                for effect in ([] if a.attributes else effects):
                    name=re.sub(r'(?<!^)(?=[A-Z])','_',effect['name']).lower()
                    name={'bad_luck':'unluck'}.get(name,name)
                    start=len(client.observed);command(f'effect give McsaProbe {name} 100 0 true')
                    client.pump(.25)
                    packets=[r for r in client.observed[start:] if r['name']=='entity_effect']
                    ids=[]
                    for r in packets:
                        raw=io.BytesIO(bytes.fromhex(r['body']));read_varint(raw.read);read_varint(raw.read);ids.append(read_varint(raw.read))
                    report['effects'].append({'name':name,'source_id':effect['id'],'observed_ids':ids,'packets':packets})
                    command(f'effect clear McsaProbe {name}');client.pump(.15)
                command('playsound minecraft:entity.item.pickup master McsaProbe ~ ~ ~ 1 1')
                client.pump(.3);report['sounds']=[r for r in client.observed if r['name']=='sound_effect']
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
    print(json.dumps({'observed':sum(bool(e['observed_ids']) for e in report['effects']),'attribute_updates':len(report.get('attribute_updates',[]))}))
if __name__=='__main__':main()
