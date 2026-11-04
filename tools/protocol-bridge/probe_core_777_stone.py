#!/usr/bin/env python3
"""Маркерный core777: native775stone, наблюдаемыйSTART0/FINISH3; --execute после GO."""
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
TEST='gameplay_probe::serve_java_26_3_probe'


class StoneProbe(Probe):
    def __init__(self,*args):super().__init__(*args);self.markers=set()
    def observe_packet(self,name,payload):
        super().observe_packet(name,payload)
        if name in ('system_chat','profileless_chat'):
            self.markers.update(m.decode() for m in re.findall(rb'MCSA_CORE_DIG_[0-9]+',payload))


def validate_stone(events, inventory, tool, cobble):
    assert any(e.get('position')==list(TARGET) and e.get('state')==0 for e in events), 'target stone not air'
    held=inventory.get(36)
    assert held and held['item']==tool and held['count']==1 and held.get('damage')==1, ('tool not retained at damage1',held)
    drops={e['entity']:e['slot'] for e in events if e['name']=='entity_metadata' and e.get('slot')}
    assert drops and all(v['item']==cobble for v in drops.values()), ('unexpected drop, possibly thrown tool',drops)
    assert sum(v['count'] for v in drops.values())==1, drops
    return drops


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute',action='store_true')
    for name in ('test-binary','data','output'):parser.add_argument('--'+name,type=Path)
    parser.add_argument('--binary-sha256')
    args=parser.parse_args()
    if not args.execute:
        print(json.dumps({'execute':False,'port':PORT,'test':TEST,'scope':'primary775stone only; 777actions0/3; tool retained damage1 and cobble62'}));return
    if not all((args.test_binary,args.data,args.output,args.binary_sha256)):parser.error('explicit binary/SHA, pinned data and fresh output required')
    assert hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256,'binary SHA mismatch'
    tables=inputs(args.data);protocol=tables['protocol.json']
    tools={v['name']:v['id'] for v in tables['items.json']}
    tool,cobble=tools['diamond_pickaxe'],tools['cobblestone'];assert cobble==62
    item=next(v['id'] for v in tables['entities.json'] if v['name']=='item')
    with socket.socket() as check:check.bind(('127.0.0.1',PORT))
    cpus=sorted(os.sched_getaffinity(0))[:2];assert len(cpus)==2;os.sched_setaffinity(0,cpus)
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    report={'status':'failed','test_binary_sha256':args.binary_sha256,'protocol':777,'native_content':775,
            'finish_status':FINISH_STATUS_777,'checks':[],'commands':[],'cpu_affinity':cpus,
            'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    server=client=None;previous={}
    with tempfile.TemporaryDirectory(prefix='mcsa-core777-stone-') as folder:
        root=Path(folder);(root/'mcsa-gameplay-probe.marker').touch();log=output/'server.log'
        with log.open('w') as out:
            def interrupted(signum,_frame):raise InterruptedError('bounded coreprobe interrupted '+str(signum))
            for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGALRM):previous[sig]=signal.signal(sig,interrupted)
            signal.alarm(120)
            try:
                server=subprocess.Popen([str(args.test_binary.resolve()),'--ignored','--exact',TEST,'--nocapture'],cwd=root,
                    stdin=subprocess.PIPE,stdout=out,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                import time
                end=time.monotonic()+30
                while 'MCSA gameplay probe ready on 26026' not in log.read_text():
                    assert server.poll() is None and time.monotonic()<end,'startup timeout'
                    time.sleep(.05)
                client=connect(PORT,protocol,lambda s,p:StoneProbe(s,p,item))
                client.pump(10,lambda row:row['name']=='position')
                marker=0
                def command(text):
                    nonlocal marker
                    marker+=1;token=f'MCSA_CORE_DIG_{marker}'
                    report['commands'].append(text)
                    server.stdin.write(text+'\nsay '+token+'\n');server.stdin.flush()
                    client.pump(5,lambda _:token in client.markers)
                    client.pump(.15)
                for text in ['gamerule doMobSpawning false','forceload add -24 -24 23 23',
                             'fill -4 64 -4 5 64 4 stone','fill -4 65 -4 5 68 4 air',
                             'tp McsaProbe 0.5 65 0.5','gamemode survival McsaProbe','clear McsaProbe',
                             'item replace entity McsaProbe weapon.mainhand with diamond_pickaxe','setblock 2 65 0 stone']:
                    command(text)
                assert client.position[:3]==[.5,65.,.5]
                client.grounded_fixture=True;client.pump(.25)
                assert client.inventory.get(36)=={'item':tool,'count':1},client.inventory
                report['checks'].append('authored native775 stone, survival, diamond pickaxe held, grounded fixture')
                start=len(client.events);client.dig(0);client.pump(2.);client.dig(FINISH_STATUS_777);client.pump(1.)
                events=client.events[start:];report['dig_events']=events
                report['drops']=validate_stone(events,client.inventory,tool,cobble)
                report['checks'].append('777START0/FINISH3 targetair, tool1052retaineddamage1, cobble62count1, no tool drop')
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
