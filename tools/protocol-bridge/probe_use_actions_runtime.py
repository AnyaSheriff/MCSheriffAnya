#!/usr/bin/env python3
"""Ограниченная проверка release-use и jab; план без соединений по умолчанию."""
import argparse
import io
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import tempfile

from probe_gameplay import connect
from probe_core_776 import stop_server
from probe_white_concrete_slab import inputs, cleanup_evidence, uint, exact, server_properties
from smoke import varint
from probe_player_actions_runtime import ActionProbe as BaseActionProbe, menu_slot

PORT=26026
TEST='gameplay_probe::serve_java_26_3_probe'


def lpvec(data):
    first=exact(data,1)[0]
    if not first:return [0.,0.,0.]
    rest=exact(data,5)
    packed=first | rest[0]<<8 | int.from_bytes(rest[1:],'big')<<16
    scale=(packed&3)+(uint(data)<<2 if packed&4 else 0)
    return [(((packed>>(3+15*i))&32767)/32766*2-1)*scale for i in range(3)]


def use_slot(data):
    count=uint(data)
    if count==0:return None
    assert count<=127
    item,added,removed=uint(data),uint(data),uint(data)
    assert added<=2 and removed==0
    result={'item':item,'count':count};seen=set()
    for _ in range(added):
        kind=uint(data);assert kind not in seen;seen.add(kind)
        if kind==3:result['damage']=uint(data)
        elif kind==13:
            size=uint(data);assert size<=8
            result['enchantments']=[[uint(data),uint(data)] for _ in range(size)]
        else:raise AssertionError('unexpected fixture component '+str(kind))
    return result


class ActionProbe(BaseActionProbe):
    def send(self,name,payload=b''):
        if name=='use_item':self.action_packets.append({'name':name,'payload_hex':payload.hex()})
        super().send(name,payload)
    def observe_packet(self,name,payload):
        assert len(self.events)<2000 and len(self.records)<20000
        data=io.BytesIO(payload)
        if name=='login':self.own_entity=struct.unpack('>i',exact(data,4))[0]
        if name in ('set_slot','set_player_inventory','window_items'):
            if name=='window_items':
                window,revision,size=uint(data),uint(data),uint(data);assert size<=100
                entries={i:use_slot(data) for i in range(size)};use_slot(data)
                if window==0:self.inventory=entries
            else:
                if name=='set_slot':
                    window,revision=uint(data),uint(data);index=struct.unpack('>h',exact(data,2))[0]
                else:window,index=0,menu_slot(uint(data))
                value=use_slot(data)
                if window==0:self.inventory[index]=value
            assert not data.read()
            self.events.append({'name':name,'inventory':dict(self.inventory),'payload_hex':payload.hex()});return
        if name=='entity_velocity':
            entity=uint(data);velocity=lpvec(data);assert not data.read()
            self.events.append({'name':name,'entity':entity,'velocity':velocity,'payload_hex':payload.hex()});return
        if name=='spawn_entity':
            entity=uint(data);exact(data,16);kind=uint(data)
            if kind==self.arrow_entity:
                position=struct.unpack('>ddd',exact(data,24));velocity=lpvec(data)
                self.events.append({'name':'arrow','entity':entity,'position':position,'velocity':velocity})
        if name=='update_health':
            health=struct.unpack('>f',exact(data,4))[0];self.food=uint(data)
        super().observe_packet(name,payload)


def validate(case,events,inventory,names,own_entity):
    assert not any(e['name']=='spawn_entity' for e in events),'dropped item'
    tool=inventory.get(36)
    if case=='malformed-bow-release':
        assert tool and tool['item']==names['bow'] and tool['count']==1 and tool.get('damage',0)==0,tool
        assert inventory.get(45)=={'item':names['arrow'],'count':3},inventory
        assert not any(e['name']=='arrow' for e in events),'malformed release fired arrow'
        return {'arrows':[]}
    assert tool and tool['item']==names['bow' if case=='bow-release' else 'iron_spear'] and tool['count']==1 and tool.get('damage')==1,tool
    arrows={e['entity'] for e in events if e['name']=='arrow'}
    if case=='bow-release':
        assert len([e for e in events if e['name']=='arrow'])==1,events
        assert len(arrows)==1,arrows
        assert inventory.get(45)=={'item':names['arrow'],'count':2},inventory
    else:
        assert not arrows
        assert inventory.get(45) is None
        impulses=[e for e in events if e['name']=='entity_velocity' and e['entity']==own_entity and e['velocity'][0]>.1]
        assert len(impulses)==1,impulses
        assert all(abs(v)<4 for v in impulses[0]['velocity']),impulses
    return {'arrows':sorted(arrows)}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute',action='store_true')
    for name in ('test-binary','data','output','jar','actions','native-items'):parser.add_argument('--'+name,type=Path)
    parser.add_argument('--binary-sha256')
    parser.add_argument('--version',type=int,choices=[775,777],default=777)
    parser.add_argument('--case',choices=['bow-release','spear-jab','malformed-bow-release'],required=True)
    args=parser.parse_args()
    if not args.execute:
        print(json.dumps({'execute':False,'port':PORT,'test':TEST,'scope':'bounded bow release/spear jab; supplied observed actions only'}));return
    assert args.case!='malformed-bow-release' or args.version==775,'malformed baseline native775 only'
    if not all((args.data,args.output,args.actions)):parser.error('pinned data, fresh output and observed actions required')
    assert bool(args.test_binary)!=bool(args.jar),'choose one server'
    if args.test_binary:
        assert args.binary_sha256 and hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256
    else:
        assert args.version==775,'official mode restricted to native775 confirmation'
        assert hashlib.sha1(args.jar.read_bytes()).hexdigest()=='97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51'
    tables=inputs(args.data)
    if args.version==777:
        protocol=tables['protocol.json'];item_table=tables['items.json'];entities=tables['entities.json']
    else:
        manifest=json.loads((args.data/'sources.json').read_text())
        for entry in manifest['baseline_files']:
            assert hashlib.sha256((args.data/entry['file']).read_bytes()).hexdigest()==entry['sha256']
        protocol=json.loads((args.data/'protocol-26.1.json').read_text())
        assert args.native_items,'explicit native item table required'
        item_path=args.native_items
        assert hashlib.sha256(item_path.read_bytes()).hexdigest()=='0d085dbf8a6fa129b94f696b77286d65ce7b04476eab398492558ab03cc1c45e'
        item_table=json.loads(item_path.read_text());entities=json.loads((args.data/'entities-26.1.json').read_text())
    names={v['name']:v['id'] for v in item_table}
    item=next(v['id'] for v in entities if v['name']=='item')
    arrow=next(v['id'] for v in entities if v['name']=='arrow')
    action_table=json.loads(args.actions.read_text());assert action_table['protocol']==args.version
    statuses=action_table['statuses'];assert all(k in statuses for k in ('release_use','jab'));assert all(type(v)==int and 0<=v<128 for v in statuses.values())
    with socket.socket() as check:check.bind(('127.0.0.1',PORT))
    cpus=sorted(os.sched_getaffinity(0))[:2];assert len(cpus)==2;os.sched_setaffinity(0,cpus)
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    (output/'used-runner.py').write_bytes(Path(__file__).read_bytes())
    report={'status':'failed','test_binary_sha256':args.binary_sha256,'protocol':args.version,'native_content':775,'case':args.case,'statuses':statuses,
            'actions_sha256':hashlib.sha256(args.actions.read_bytes()).hexdigest(),'checks':[],'commands':[],'cpu_affinity':cpus,
            'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    server=client=None;previous={}
    with tempfile.TemporaryDirectory(prefix='mcsa-use-actions-') as folder:
        root=Path(folder);(root/'mcsa-gameplay-probe.marker').touch();log=output/'server.log'
        if args.jar:
            (root/'eula.txt').write_text('eula=true\n');(root/'server.properties').write_text(server_properties(PORT))
        with log.open('w') as out:
            def interrupted(signum,_frame):raise InterruptedError('bounded coreprobe interrupted '+str(signum))
            for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGALRM):previous[sig]=signal.signal(sig,interrupted)
            signal.alarm(180)
            try:
                argv=([str(args.test_binary.resolve()),'--ignored','--exact',TEST,'--nocapture'] if args.test_binary else ['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(args.jar.resolve()),'nogui'])
                server=subprocess.Popen(argv,cwd=root,
                    stdin=subprocess.PIPE,stdout=out,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                import time
                end=time.monotonic()+120
                ready='MCSA gameplay probe ready on 26026' if args.test_binary else 'Done ('
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic()<end,'startup timeout'
                    time.sleep(.05)
                client=connect(PORT,protocol,lambda s,p:ActionProbe(s,p,item,args.version),version=args.version)
                client.arrow_entity=arrow;client.food=None
                client.pump(10,lambda row:row['name']=='position')
                marker=0
                def command(text):
                    nonlocal marker
                    marker+=1;token=f'MCSA_ACTION_{marker}'
                    report['commands'].append(text)
                    server.stdin.write(text+'\nsay '+token+'\n');server.stdin.flush()
                    client.pump(5,lambda _:token in client.markers)
                    client.pump(.15)
                for text in ['forceload add -24 -24 23 23',
                             'fill -4 64 -4 5 64 4 stone','fill -4 65 -4 5 68 4 air',
                             'tp McsaProbe 0.5 65 0.5','gamemode survival McsaProbe','clear McsaProbe']:
                    command(text)
                setups={'bow-release':['item replace entity McsaProbe weapon.mainhand with bow 1',
                                       'item replace entity McsaProbe weapon.offhand with arrow 3'],
                        'spear-jab':['item replace entity McsaProbe weapon.mainhand with iron_spear[enchantments={lunge:1}] 1']}
                bow_case=args.case in ('bow-release','malformed-bow-release')
                for text in setups['bow-release' if bow_case else args.case]:command(text)
                pitch=-5. if bow_case else 0.
                command(f'tp McsaProbe 0.5 65 0.5 -90 {pitch}')
                command('effect give McsaProbe saturation 1 10 true')
                client.grounded_fixture=True;client.pump(2.)
                tool=client.inventory.get(36)
                assert tool and tool['item']==names['bow' if bow_case else 'iron_spear'] and tool['count']==1 and tool.get('damage',0)==0,tool
                assert client.inventory.get(45)==({'item':names['arrow'],'count':3} if bow_case else None)
                if args.case=='spear-jab':assert client.food==20 and tool.get('enchantments'),(client.food,tool)
                report['before']=dict(client.inventory);report['checks'].append('fixture confirmed')
                start=len(client.events)
                if bow_case:
                    client.sequence+=1
                    client.send('use_item',varint(0)+varint(client.sequence)+struct.pack('>ff',-90.,pitch))
                    client.pump(1.3)
                    if args.case=='malformed-bow-release':client.send('block_dig',varint(statuses['release_use']))
                    else:client.action(statuses['release_use'],sequence=0)
                else:client.action(statuses['jab'],sequence=0)
                client.pump(1.)
                events=client.events[start:];report['action_events']=events
                report['effects']=validate(args.case,events,client.inventory,names,client.own_entity)
                report['checks'].append(args.case+' authoritative result confirmed')
                report['status']='pass'
            except BaseException as error:report['error']=repr(error)
            finally:
                for sig in previous:signal.signal(sig,signal.SIG_IGN)
                signal.alarm(0);errors=[]
                if client:
                    report.update(events=client.events,inventory=client.inventory,action_packets=client.action_packets)
                    try:client.sock.close()
                    except BaseException as error:errors.append({'step':'close-client','error':repr(error)})
                if server:
                    try:report['shutdown']=stop_server(server)
                    except BaseException as error:
                        errors.append({'step':'stop-server','error':repr(error)})
                        try:os.killpg(server.pid,signal.SIGKILL)
                        except ProcessLookupError:pass
                        try:server.wait(timeout=5)
                        except BaseException as error:errors.append({'step':'reap-server','error':repr(error)})
                    report['server_exit']=server.returncode
                    report['cleanup']=cleanup_evidence(server.pid,PORT)
                    if (server.returncode!=0 or report['cleanup']['remaining_own_group'] or not report['cleanup']['port_released'] or report['cleanup']['errors']):report['status']='failed'
                report['cleanup_errors']=errors
                if errors:report['status']='failed'
                try:(output/'result.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
                finally:
                    for sig,handler in previous.items():signal.signal(sig,handler)
    if report['status']!='pass':raise SystemExit(1)
    print(json.dumps({'status':'pass','checks':report['checks']}))


if __name__=='__main__':main()
