#!/usr/bin/env python3
"""Наши команды → сетевые Slot официальных1.12.2/26.1.2/26.2/26.3. JAR только исполняется."""
import argparse
import hashlib
import io
import json
import re
from pathlib import Path
import select
import socket
import struct
import subprocess
import tempfile
import time
from probe_gameplay import Client, connect
from smoke import varint, read_varint, send, handshake
from observe import frame

CASES = [
 ('stone', 'stone 3 0', 'stone 3'),
 ('granite', 'stone 4 1', 'granite 4'),
 ('red_wool', 'wool 5 14', 'red_wool 5'),
 ('sword', 'diamond_sword 1 7 {Unbreakable:1b,RepairCost:2,ench:[{id:16s,lvl:5s}]}', 'diamond_sword[damage=7,unbreakable={},repair_cost=2,enchantments={sharpness:5}]'),
 ('book', 'enchanted_book 1 0 {StoredEnchantments:[{id:34s,lvl:3s}]}', 'enchanted_book[stored_enchantments={unbreaking:3}]'),
 ('name', 'paper 1 0 {display:{Name:"MCSA probe",Lore:["Line one","Line two"]}}', 'paper[custom_name="MCSA probe",lore=["Line one","Line two"]]'),
 ('leather', 'leather_chestplate 1 9 {display:{color:16711680}}', 'leather_chestplate[damage=9,dyed_color=16711680]'),
 ('map', 'filled_map 1 23', 'filled_map[map_id=23]'),
 ('white_banner', 'banner 2 15', 'white_banner 2'),
 ('empty', 'air 1', 'air'),
]
class Probe(Client):
    def __init__(self,sock,protocol,version):
        super().__init__(sock,protocol); self.version=version; self.slots=[]; self.suggestions=[]
    def confirm_teleport(self,teleport,values):
        self.send('teleport_confirm',varint(teleport)+(struct.pack('>dddff',*values) if self.version==777 else b''))
    def observe_packet(self,name,payload):
        data=io.BytesIO(payload)
        if name=='tab_complete':
            assert read_varint(data.read)==31
            read_varint(data.read);read_varint(data.read)
            for _ in range(read_varint(data.read)):
                self.suggestions.append(data.read(read_varint(data.read)).decode())
                assert data.read(1)==b'\0','unexpected suggestion tooltip'
        if name=='set_slot':
            window=struct.unpack('>b',data.read(1))[0] if self.version==340 else read_varint(data.read)
            if self.version!=340: read_varint(data.read)
            slot=struct.unpack('>h',data.read(2))[0]
            if window==0 and slot==36:self.slots.append(data.read().hex())
        elif name=='set_player_inventory':
            if read_varint(data.read)==0:self.slots.append(data.read().hex())
    def pump(self,duration,predicate=None):
        if self.version!=340:return super().pump(duration,predicate)
        deadline=time.monotonic()+duration
        while time.monotonic()<deadline:
            if not select.select([self.sock],[],[],.05)[0]:continue
            data=io.BytesIO(frame(self.sock));kind=read_varint(data.read);payload=data.read();name=self.incoming.get(kind,'?')
            self.observe_packet(name,payload)
            if name=='keep_alive': self.send('keep_alive',payload)
            elif name=='position':
                data=io.BytesIO(payload);data.read(33);teleport=read_varint(data.read);self.confirm_teleport(teleport,[])
            elif name=='kick_disconnect':raise RuntimeError(payload)


def main():
    p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path)
    p.add_argument('--version',type=int,choices=[340,775,776,777],required=True);p.add_argument('--port',type=int,default=26023);p.add_argument('--output',type=Path,required=True)
    import official_26_2_inputs
    official_26_2_inputs.arguments(p)
    p.add_argument('--replay',type=Path)
    p.add_argument('--cases',type=Path,help='свои команды для дополнительных современных компонентов')
    p.add_argument('--allow-missing-attributes',action='store_true',help='записать явный отказ неизвестного имени атрибута')
    p.add_argument('--discover-attributes',action='store_true',help='обычное дополнение /attribute и отдельный предмет с каждым предложенным свойством')
    a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    cases=CASES
    if a.cases:
        assert a.version in (775,776,777) and not a.replay
        cases=[(entry['name'],'',entry['command']) for entry in json.loads(a.cases.read_text())]
    expected={340:'886945bfb2b978778c3a0288fd7fab09d315b25f',775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}
    if a.version == 776:
        protocol, _, provenance = official_26_2_inputs.load(a)
        expected[776] = provenance['jar_sha1']
    else:
        assert a.data is not None, '--data required for340/775/777'
        with a.jar.open('rb') as f:assert hashlib.file_digest(f,'sha1').hexdigest()==expected[a.version]
        path=a.data/('protocol-26.1.json' if a.version==775 else 'protocol.json');manifest=json.loads((a.data/'sources.json').read_text())
        assert hashlib.sha256(path.read_bytes()).hexdigest()==next(r['sha256'] for r in manifest['files']+manifest.get('baseline_files',[]) if r['file']==path.name)
        protocol=json.loads(path.read_text());provenance=None
    report={'protocol':a.version,'jar_sha1':expected[a.version],'cases':[]}
    if provenance is not None: report['provenance']=provenance
    with socket.socket() as check:check.bind(('127.0.0.1',a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-item-slots-') as directory:
        root=Path(directory);(root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type='+('FLAT' if a.version==340 else 'minecraft:flat')+'\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=1\n')
        log=a.output/'server.log'
        with log.open('w') as output:
            java='/usr/lib/jvm/java-8-openjdk/jre/bin/java' if a.version==340 else '/usr/lib/jvm/java-25-openjdk/bin/java'
            server=subprocess.Popen([java,'-Xmx1G','-jar',str(a.jar.resolve()),'nogui'],cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
            client=None
            def command(value):server.stdin.write(value+'\n');server.stdin.flush()
            try:
                deadline=time.monotonic()+120
                while 'Done (' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(.1)
                if a.version==340:
                    sock=socket.create_connection(('127.0.0.1',a.port),timeout=10);handshake(sock,340,a.port,2)
                    send(sock,b'\0\x09McsaProbe');data=io.BytesIO(frame(sock));assert read_varint(data.read)==2
                    client=Probe(sock,protocol,340)
                else:client=connect(a.port,protocol,lambda s,p:Probe(s,p,a.version),version=a.version)
                client.pump(2)
                if a.discover_attributes:
                    assert a.version in (775,776,777) and not a.replay and not a.cases
                    command('op McsaProbe');client.pump(.5)
                    text=b'/attribute McsaProbe '
                    client.send('tab_complete',varint(31)+varint(len(text))+text)
                    deadline=time.monotonic()+5
                    while not client.suggestions:
                        assert time.monotonic()<deadline,'attribute suggestions missing';client.pump(.1)
                    assert all(re.fullmatch(r'minecraft:[a-z0-9_]+',name) for name in client.suggestions)
                    report['attribute_suggestions']=client.suggestions
                    cases=[(name,'','stick[attribute_modifiers=[{type:"'+name+'",id:"mcsa:probe",amount:1,operation:"add_value",slot:"mainhand"}]]') for name in client.suggestions]
                for name,old,new in ([] if a.replay else cases):
                    client.slots.clear()
                    log_start=len(log.read_text())
                    command(('replaceitem entity McsaProbe slot.hotbar.0 '+old) if a.version==340 else ('item replace entity McsaProbe hotbar.0 with '+new))
                    deadline=time.monotonic()+8
                    while not client.slots:
                        error=log.read_text()[log_start:]
                        if a.allow_missing_attributes and 'Unknown registry key' in error and 'minecraft:attribute' in error:
                            report['cases'].append({'name':name,'command':new,'missing_attribute':True});break
                        assert time.monotonic()<deadline, name+' command produced no slot'
                        client.pump(.1)
                    if client.slots:report['cases'].append({'name':name,'command':old if a.version==340 else new,'slot_hex':client.slots[-1]})
                if a.replay:
                    assert a.version==340
                    replay=json.loads(a.replay.read_text())
                    assert len(replay)==len(CASES)
                    command('gamemode creative McsaProbe');client.pump(.5)
                    for sent,(name,old,_) in zip(replay,CASES):
                        assert sent['name']==name
                        if name=='map':
                            # Без настоящей карты сервер1.12.2 сам заменяет23 на
                            # новыйID0 при тике предмета. Создаём карты командами.
                            for damage in range(24):
                                command('replaceitem entity McsaProbe slot.hotbar.0 filled_map 1 '+str(damage))
                                client.pump(.15)
                            start=len(log.read_text())
                            command('testfor McsaProbe {Inventory:[{Slot:0b,id:"minecraft:filled_map",Damage:23s}]}')
                            deadline=time.monotonic()+5
                            while 'Found McsaProbe' not in log.read_text()[start:]:
                                assert time.monotonic()<deadline,'map23 preparation failed'
                                client.pump(.05)
                            report['map23_prepared']=True
                        client.send('set_creative_slot',struct.pack('>h',36)+bytes.fromhex(sent['slot_hex']))
                        client.pump(.3)
                        parts=old.split(' ',3)
                        if name=='empty':test='{Inventory:[]}'
                        else:
                            item,count,damage=parts[:3]
                            tag=',tag:'+parts[3] if len(parts)==4 else ''
                            test='{Inventory:[{Slot:0b,id:"minecraft:'+item+'",Count:'+count+'b,Damage:'+damage+'s'+tag+'}]}'
                        start=len(log.read_text());command('testfor McsaProbe '+test)
                        deadline=time.monotonic()+5
                        while 'Found McsaProbe' not in log.read_text()[start:]:
                            assert time.monotonic()<deadline, name+': '+log.read_text()[start:]
                            client.pump(.05)
                        report['cases'].append({'name':name,'accepted_creative':True,'testfor':test})
            finally:
                if client:client.sock.close()
                if server.poll() is None:
                    command('stop')
                    try:server.wait(timeout=30)
                    except subprocess.TimeoutExpired:server.kill();server.wait()
                report['exit_code']=server.returncode;(a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0;print(json.dumps({'version':a.version,'cases':len(report['cases'])}))
if __name__=='__main__':main()
