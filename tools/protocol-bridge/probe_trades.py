#!/usr/bin/env python3
"""Жители и торговля через обычные команды и взаимодействия, без чтения реализации."""
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
        if name in ('spawn_entity','entity_metadata','open_window','trade_list'):
            self.observed.append({'name': name, 'body': (varint(next(k for k,v in self.incoming.items() if v == name)) + payload).hex()})

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--version',type=int,choices=[775,777],required=True)
    p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--port',type=int,default=26029)
    a=p.parse_args();port=a.port
    expected={775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==expected
    sources=json.loads((a.data/'sources.json').read_text())
    for entry in sources['files']+sources['baseline_files']:
        assert hashlib.sha256((a.data/entry['file']).read_bytes()).hexdigest()==entry['sha256']
    protocol=json.loads((a.data/('protocol.json' if a.version==777 else 'protocol-26.1.json')).read_text())
    with socket.socket() as check:check.bind(('127.0.0.1',port))
    a.output.mkdir(parents=True,exist_ok=True)
    report={'version':a.version,'jar_sha1':expected,'variants':[], 'trades':[]}
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
                types=['desert','jungle','plains','savanna','snow','swamp','taiga']
                professions=['none','armorer','butcher','cartographer','cleric','farmer','fisherman','fletcher','leatherworker','librarian','mason','nitwit','shepherd','toolsmith','weaponsmith']
                variants=[(t,'farmer',2) for t in types]+[('plains',p,3) for p in professions]+[('plains','farmer',level) for level in (1,4,5)]
                for variant,profession,level in variants:
                    start=len(client.observed)
                    command(f'summon villager 2 80 2 {{NoAI:1b,VillagerData:{{type:"minecraft:{variant}",profession:"minecraft:{profession}",level:{level}}}}}')
                    event=client.pump(5,lambda r:r['name']=='spawn_entity');client.pump(.3)
                    entity=event['entity'];packets=[]
                    for packet in client.observed[start:]:
                        if packet['name']!='entity_metadata':continue
                        raw=io.BytesIO(bytes.fromhex(packet['body']));read_varint(raw.read)
                        if read_varint(raw.read)==entity:packets.append(packet)
                    assert packets,'no villager metadata'
                    report['variants'].append({'type':variant,'profession':profession,'level':level,'entity':entity,'packets':packets})
                    command('kill @e[type=villager]');client.pump(.2)
                offers=[
                    ('basic','{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:2,priceMultiplier:0.05f}'),
                    ('two_inputs','{buy:{id:"minecraft:emerald",count:5},buyB:{id:"minecraft:book",count:1},sell:{id:"minecraft:diamond_sword",count:1,components:{"minecraft:damage":3}},uses:2,maxUses:8,xp:5,specialPrice:0,priceMultiplier:0.2f,demand:2}'),
                    ('components','{buy:{id:"minecraft:potion",count:1,components:{"minecraft:potion_contents":{potion:"minecraft:swiftness"}}},sell:{id:"minecraft:written_book",count:1,components:{"minecraft:written_book_content":{title:"Story",author:"Alex",pages:["First page"]}}},maxUses:16,xp:3}'),
                    ('disabled','{buy:{id:"minecraft:emerald",count:1},sell:{id:"minecraft:red_wool",count:5},uses:16,maxUses:16,xp:1}')]
                for name,offer in offers:
                    command('summon villager 2 80 2 {NoAI:1b,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:2},Offers:{Recipes:['+offer+']}}')
                    event=client.pump(5,lambda r:r['name']=='spawn_entity');client.pump(.3)
                    start=len(client.observed)
                    client.send('use_entity',varint(event['entity'])+b'\0\0\0')
                    client.pump(5,lambda r:r['name']=='trade_list');client.pump(.2)
                    packets=[p for p in client.observed[start:] if p['name'] in ('open_window','trade_list')]
                    assert {p['name'] for p in packets}=={'open_window','trade_list'},'merchant did not open'
                    raw=io.BytesIO(bytes.fromhex(next(p['body'] for p in packets if p['name']=='open_window')));read_varint(raw.read);window=read_varint(raw.read)
                    report['trades'].append({'name':name,'packets':packets})
                    client.send('close_window',varint(window));client.pump(.1)
                    command('kill @e[type=villager]');client.pump(.2)
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
    print(json.dumps({'variants':len(report['variants']),'trades':len(report['trades']),'exit_code':report['exit_code']}))
if __name__=='__main__':main()
