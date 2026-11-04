#!/usr/bin/env python3
"""Private preview263: настоящие пакеты777 установки/добычи плиты. План без --run."""
import argparse
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
from probe_white_concrete_slab import Probe, inputs, TARGET, FINISH_STATUS_777, cleanup_evidence

PORT=26026
TEST='catalog_preview_probe::serve_java_26_3_catalog_probe'


class CatalogProbe(Probe):
    def __init__(self,*args):super().__init__(*args);self.markers=set()
    def send(self,name,payload=b''):
        if name=='block_place':
            self.action_packets.append({'name':name,'packet_id':self.outgoing[name],
                'payload_hex':payload.hex(),'client_pose':self.position,'play_loaded':self.loaded})
        super().send(name,payload)
    def observe_packet(self,name,payload):
        super().observe_packet(name,payload)
        if name in ('system_chat','profileless_chat'):
            self.markers.update(m.decode() for m in re.findall(rb'MCSA_CORE_DIG_[0-9]+',payload))



def validate_mining(events, inventory, tool, slab, expected_count):
    assert any(e.get('position')==list(TARGET) and e.get('state')==0 for e in events), 'target not air'
    drops={e['entity']:e['slot'] for e in events if e['name']=='entity_metadata' and e.get('slot')}
    assert all(v['item']==slab for v in drops.values()), ('wrong drop, possibly thrown tool',drops)
    assert sum(v['count'] for v in drops.values())==expected_count,drops
    expected_held={'item':tool,'count':1,'damage':1} if expected_count else None
    assert inventory.get(36)==expected_held,inventory
    return drops


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run',action='store_true')
    for name in ('test-binary','data','output'):parser.add_argument('--'+name,type=Path)
    parser.add_argument('--binary-sha256')
    args=parser.parse_args()
    if not args.run:
        print(json.dumps({'run':False,'port':PORT,'test':TEST,'scope':'private Preview26_3 in-memory world, raw777 slab placement/merge/mining; persistent player fixture only; public admission unchanged'}));return
    if not all((args.test_binary,args.data,args.output,args.binary_sha256)):parser.error('explicit binary/SHA, pinned data and fresh output required')
    assert hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256,'binary SHA mismatch'
    tables=inputs(args.data);protocol=tables['protocol.json']
    tools={v['name']:v['id'] for v in tables['items.json']}
    tool,slab=tools['diamond_pickaxe'],tools['white_concrete_slab'];assert slab==721
    item=next(v['id'] for v in tables['entities.json'] if v['name']=='item')
    with socket.socket() as check:check.bind(('127.0.0.1',PORT))
    cpus=sorted(os.sched_getaffinity(0))[:2];assert len(cpus)==2;os.sched_setaffinity(0,cpus)
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    report={'status':'failed','test_binary_sha256':args.binary_sha256,'protocol':777,'server_catalog':'Preview26_3','persistent_world':False,
            'finish_status':FINISH_STATUS_777,'checks':[],'commands':[],'cpu_affinity':cpus,
            'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    server=client=None;previous={}
    with tempfile.TemporaryDirectory(prefix='mcsa-core777-catalog-') as folder:
        root=Path(folder);(root/'mcsa-gameplay-probe.marker').touch();(root/'mcsa-catalog-preview263.marker').touch();log=output/'server.log'
        with log.open('w') as out:
            def interrupted(signum,_frame):raise InterruptedError('bounded coreprobe interrupted '+str(signum))
            for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGALRM):previous[sig]=signal.signal(sig,interrupted)
            signal.alarm(180)
            try:
                server=subprocess.Popen([str(args.test_binary.resolve()),'--ignored','--exact',TEST,'--nocapture'],cwd=root,
                    stdin=subprocess.PIPE,stdout=out,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                import time
                end=time.monotonic()+30
                while 'MCSA gameplay probe ready on 26026' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<end,'startup timeout'
                    time.sleep(.05)
                client=connect(PORT,protocol,lambda s,p:CatalogProbe(s,p,item))
                client.pump(10,lambda row:row['name']=='position')
                marker=0
                def command(text):
                    nonlocal marker
                    marker+=1;token=f'MCSA_CORE_DIG_{marker}'
                    report['commands'].append(text)
                    server.stdin.write(text+'\nsay '+token+'\n');server.stdin.flush()
                    client.pump(5,lambda _:token in client.markers)
                    client.pump(.15)
                assert 'MCSA catalog preview263 active; world=in_memory; persistent_world=false' in log.read_text()
                for text in ['gamerule doMobSpawning false','forceload add -24 -24 23 23',
                             'fill -4 64 -4 5 64 4 stone','fill -4 65 -4 5 68 4 air',
                             'tp McsaProbe 0.5 65 0.5','gamemode survival McsaProbe','clear McsaProbe']:
                    command(text)
                assert client.position[:3]==[.5,65.,.5]
                client.grounded_fixture=True;client.pump(.25)
                client.send('held_item_slot',struct.pack('>h',0))
                def expect_state(expected, action):
                    start=len(client.target_updates)
                    action();client.pump(.6)
                    updates=client.target_updates[start:]
                    assert updates and updates[-1]['state']==expected, updates
                    return updates
                for state, expected in tables['states'].items():
                    half,wet=state.split('/')
                    expect_state(expected,lambda half=half,wet=wet:command(f'setblock 2 65 0 white_concrete_slab[type={half},waterlogged={wet}]'))
                report['checks'].append('all six appended internal states reach exact777 wire states')
                command('setblock 2 65 0 air')
                command('give McsaProbe white_concrete_slab 65')
                stacks=[v['count'] for v in client.inventory.values() if v and v['item']==slab]
                assert sorted(stacks)==[1,64],stacks
                report['checks'].append('real give slab65 splits64+1 and translates item721')
                command('clear McsaProbe');command('give McsaProbe white_concrete_slab 2')
                assert client.inventory.get(36)=={'item':slab,'count':2},client.inventory
                expect_state(tables['states']['bottom/false'],lambda:client.place((2,64,0),1,(.5,1.,.5)))
                assert client.inventory.get(36)=={'item':slab,'count':1},client.inventory
                expect_state(tables['states']['double/false'],lambda:client.place(TARGET,1,(.5,.5,.5)))
                assert client.inventory.get(36) is None,client.inventory
                report['checks'].append('raw777 places bottom then merges double, consumes exactly2')
                command('setblock 2 65 0 air');command('setblock 2 66 0 stone')
                command('give McsaProbe white_concrete_slab 1')
                expect_state(tables['states']['top/false'],lambda:client.place((2,66,0),0,(.5,0.,.5)))
                assert client.inventory.get(36) is None,client.inventory
                command('setblock 2 66 0 air')
                report['checks'].append('raw777 places top from lower face, consumes1')
                command('setblock 2 65 0 water[level=0]');command('give McsaProbe white_concrete_slab 1')
                expect_state(tables['states']['bottom/true'],lambda:client.place((2,64,0),1,(.5,1.,.5)))
                assert client.inventory.get(36) is None,client.inventory
                report['checks'].append('raw777 places waterlogged bottom, consumes1')
                command('fill -10 65 -10 12 68 10 air')
                for half,count,tool_name,delay in [('bottom',1,'diamond_pickaxe',2.),('double',2,'diamond_pickaxe',2.),('bottom',0,'air',12.)]:
                    command('kill @e[type=item]');command('clear McsaProbe')
                    command(f'setblock 2 65 0 white_concrete_slab[type={half},waterlogged=false]')
                    command('item replace entity McsaProbe weapon.mainhand with '+tool_name)
                    start=len(client.events);client.dig(0);client.pump(delay);client.dig(FINISH_STATUS_777);client.pump(1.)
                    events=client.events[start:]
                    drops=validate_mining(events,client.inventory,tool,slab,count)
                    report.setdefault('dig_cases',[]).append({'half':half,'tool':tool_name,'events':events,'drops':drops,'expected_count':count})
                    report['checks'].append(f'raw777 START0/FINISH3 mines {half}, {tool_name}, exact slab drop{count}, held tool conserved')
                command('kill @e[type=item]');command('clear McsaProbe')
                command('give McsaProbe white_concrete_slab 3')
                assert client.inventory.get(36)=={'item':slab,'count':3},client.inventory
                command('give McsaProbe diamond_pickaxe[damage=7] 1')
                assert client.inventory.get(37)=={'item':tool,'count':1,'damage':7},client.inventory
                first_client=client
                first_client.sock.shutdown(socket.SHUT_RDWR);first_client.sock.close();client=None
                # Завершение старой connection task сохраняет игрока перед новым login.
                end=time.monotonic()+5
                while not list((root/'world/playerdata').glob('*.mcrw')):
                    assert server.poll() is None and time.monotonic()<end,'player save timeout'
                    time.sleep(.05)
                report['first_session']={'events':first_client.events,'actions':first_client.action_packets}
                client=connect(PORT,protocol,lambda s,p:CatalogProbe(s,p,item))
                client.pump(10,lambda row:row['name']=='position')
                client.pump(.5)
                assert client.inventory.get(36)=={'item':slab,'count':3},client.inventory
                assert client.inventory.get(37)=={'item':tool,'count':1,'damage':7},client.inventory
                report['player_fixture']=[{'file':p.name,'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
                    for p in sorted((root/'world/playerdata').glob('*.mcrw'))]
                assert report['player_fixture'],'no own player fixture'
                assert not (root/'world/region').exists() and not (root/'region').exists(),'private memory world unexpectedly wrote regions'
                report['world_region_files_created']=False
                report['checks'].append('raw777 logout/relogin preserves slab3 and pickaxe damage7 in real player save; world remains in memory')
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
