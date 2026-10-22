#!/usr/bin/env python3
"""Ограниченная проверка отмены, выброса и смены рук; отдельное окно --execute."""
import argparse
import io
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import socket
import struct
import subprocess
import tempfile

from probe_gameplay import connect
from probe_core_776 import stop_server
from probe_white_concrete_slab import Probe, inputs, TARGET, cleanup_evidence, slot, uint, exact, pack_position, server_properties
from smoke import varint

PORT=26026
TEST='gameplay_probe::serve_java_26_3_probe'


class ActionProbe(Probe):
    def __init__(self,sock,protocol,item_entity,version):
        super().__init__(sock,protocol,item_entity);self.markers=set();self.version=version
    def confirm_teleport(self,teleport,values):
        self.send('teleport_confirm',varint(teleport)+(struct.pack('>dddff',*values) if self.version==777 else b''))
    def observe_packet(self,name,payload):
        if name=='set_player_inventory':
            data=io.BytesIO(payload);index=uint(data);value=slot(data)
            assert not data.read()
            index=menu_slot(index);self.inventory[index]=value
            self.events.append({'name':name,'inventory':dict(self.inventory),'payload_hex':payload.hex()})
        else:super().observe_packet(name,payload)
        if name in ('system_chat','profileless_chat'):
            self.markers.update(m.decode() for m in re.findall(rb'MCSA_ACTION_[A-Z0-9_]+',payload))
    def action(self,status,position=(0,0,0),face=0,sequence=None):
        self.sequence=self.sequence+1 if sequence is None else sequence
        self.send('block_dig',varint(status)+pack_position(position)+bytes([face])+varint(self.sequence))

def menu_slot(index):
    if 0<=index<=8:return index+36
    if 9<=index<=35:return index
    if 36<=index<=39:return 44-index
    if index==40:return 45
    raise ValueError('unknown player inventory slot')

def drops(events):
    return {e['entity']:e['slot'] for e in events if e['name']=='entity_metadata' and e.get('slot')}

def validate(case,events,inventory,names,markers):
    actual=drops(events)
    if case=='cancel':
        assert any(e['name']=='acknowledge_player_digging' and e.get('sequence')==0 for e in events),'cancel ACK missing'
        assert 'MCSA_ACTION_TARGET_RETAINED' in markers,'target was not retained'
        assert not any(e.get('position')==list(TARGET) and e.get('state')==0 for e in events),'target became air'
        assert not actual,'cancel produced drops'
        assert inventory.get(36) is None and inventory.get(45) is None
    elif case in ('drop_one','drop_stack'):
        item=names['stone' if case=='drop_one' else 'cobblestone'];count=1 if case=='drop_one' else 11
        expected={'item':item,'count':6} if case=='drop_one' else None
        assert inventory.get(36)==expected,(case,inventory.get(36))
        assert len(actual)==1 and list(actual.values())==[{'item':item,'count':count}],actual
    elif case=='swap':
        assert inventory.get(36)=={'item':names['gold_ingot'],'count':5},inventory
        assert inventory.get(45)=={'item':names['iron_ingot'],'count':3},inventory
        assert not actual,'swap produced drops'
    else:raise ValueError(case)
    return actual


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute',action='store_true')
    for name in ('test-binary','data','output','jar','actions','native-items'):parser.add_argument('--'+name,type=Path)
    parser.add_argument('--binary-sha256')
    parser.add_argument('--version',type=int,choices=[775,777],default=777)
    parser.add_argument('--case',choices=['cancel','drop_one','drop_stack','swap'],required=True)
    args=parser.parse_args()
    if not args.execute:
        print(json.dumps({'execute':False,'port':PORT,'test':TEST,'scope':'bounded cancel/drop/swap effects, explicit observed action table'}));return
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
    action_table=json.loads(args.actions.read_text());assert action_table['protocol']==args.version
    statuses=action_table['statuses'];assert all(type(v)==int and 0<=v<128 for v in statuses.values())
    with socket.socket() as check:check.bind(('127.0.0.1',PORT))
    cpus=sorted(os.sched_getaffinity(0))[:2];assert len(cpus)==2;os.sched_setaffinity(0,cpus)
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    report={'status':'failed','test_binary_sha256':args.binary_sha256,'protocol':args.version,'native_content':775,'case':args.case,'statuses':statuses,
            'actions_sha256':hashlib.sha256(args.actions.read_bytes()).hexdigest(),'checks':[],'commands':[],'cpu_affinity':cpus,
            'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    server=client=None;previous={}
    with tempfile.TemporaryDirectory(prefix='mcsa-player-actions-') as folder:
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
                setups={'cancel':['setblock 2 65 0 obsidian'],
                        'drop_one':['item replace entity McsaProbe weapon.mainhand with stone 7'],
                        'drop_stack':['item replace entity McsaProbe weapon.mainhand with cobblestone 11'],
                        'swap':['item replace entity McsaProbe weapon.mainhand with iron_ingot 3',
                                'item replace entity McsaProbe weapon.offhand with gold_ingot 5']}
                for text in setups[args.case]:command(text)
                assert client.position[:3]==[.5,65.,.5]
                client.grounded_fixture=True;client.pump(.25)
                report['before']=dict(client.inventory)
                expected={'cancel':None,'drop_one':{'item':names['stone'],'count':7},
                          'drop_stack':{'item':names['cobblestone'],'count':11},
                          'swap':{'item':names['iron_ingot'],'count':3}}
                assert client.inventory.get(36)==expected[args.case],client.inventory
                if args.case=='swap':assert client.inventory.get(45)=={'item':names['gold_ingot'],'count':5}
                report['checks'].append('fixture confirmed')
                start=len(client.events)
                if args.case=='cancel':
                    client.action(statuses['start'],TARGET,1);client.pump(.2)
                    client.action(statuses['cancel'],TARGET,0,sequence=0);client.pump(.5)
                    command('execute if block 2 65 0 obsidian run say MCSA_ACTION_TARGET_RETAINED')
                else:
                    client.action(statuses[args.case],sequence=0);client.pump(.5)
                events=client.events[start:];report['action_events']=events
                report['drops']=validate(args.case,events,client.inventory,names,client.markers)
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
