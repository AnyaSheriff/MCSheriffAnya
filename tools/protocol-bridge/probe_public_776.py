#!/usr/bin/env python3
"""Production: честный Status, допущенный Play и русский отказ закрытым клиентам."""
import argparse
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time

from probe_core_776 import PROTOCOL_SHA, stop_server
from probe_776_payloads import table
from probe_gameplay import connect
from probe_step1_gameplay_776 import GameplayProbe
from smoke import frame, handshake, read_varint, send


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','protocol','cache','output'):p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--protocol769',type=Path)
    p.add_argument('--cache769',type=Path)
    p.add_argument('--admit769',action='store_true')
    p.add_argument('--protocol770',type=Path)
    p.add_argument('--cache770',type=Path)
    p.add_argument('--admit770',action='store_true')
    p.add_argument('--protocol771',type=Path)
    p.add_argument('--cache771',type=Path)
    p.add_argument('--admit771',action='store_true')
    p.add_argument('--protocol772',type=Path)
    p.add_argument('--cache772',type=Path)
    p.add_argument('--admit772',action='store_true')
    p.add_argument('--protocol773',type=Path)
    p.add_argument('--cache773',type=Path)
    p.add_argument('--admit773',action='store_true')
    p.add_argument('--protocol774',type=Path)
    p.add_argument('--cache774',type=Path)
    p.add_argument('--admit774',action='store_true')
    p.add_argument('--closed-clients',type=int,nargs='+',choices=(769,770,771,772,773),default=[])
    p.add_argument('--port',type=int,default=26184)
    a=p.parse_args();assert 26100<=a.port<=26198
    assert hashlib.sha256(a.protocol.read_bytes()).hexdigest()==PROTOCOL_SHA
    protocol=json.loads(a.protocol.read_text())
    item=next(r['id'] for r in table(a.cache,'entities') if r['name']=='item')
    protocols={776:protocol};items={776:item}
    if a.admit769:
        assert a.protocol769 and a.cache769 and 769 not in a.closed_clients
        locks=json.loads((Path(__file__).parent/'client-chain-sources.json').read_text())['versions']['769']['files']
        assert hashlib.sha256(a.protocol769.read_bytes()).hexdigest()==locks['protocol']['sha256']
        protocols[769]=json.loads(a.protocol769.read_text())
        items[769]=next(r['id'] for r in table(a.cache769,'entities') if r['name']=='item')
    if a.admit770:
        assert a.protocol770 and a.cache770 and 770 not in a.closed_clients
        assert hashlib.sha256(a.protocol770.read_bytes()).hexdigest()=='8695c1cd011af37d214cc2139fc9bf6e7b445ce45305fbfa28c93f47563cb988'
        protocols[770]=json.loads(a.protocol770.read_text())
        items[770]=next(r['id'] for r in table(a.cache770,'entities') if r['name']=='item')
    if a.admit774:
        assert a.protocol774 and a.cache774
        assert hashlib.sha256(a.protocol774.read_bytes()).hexdigest()=='4108833ded28ee21786e4905d0a07b3a06164a3842a8d6bab98096fd3c2e28ab'
        protocols[774]=json.loads(a.protocol774.read_text())
        items[774]=next(r['id'] for r in table(a.cache774,'entities') if r['name']=='item')
    if a.admit771:
        assert a.protocol771 and a.cache771 and 771 not in a.closed_clients
        assert hashlib.sha256(a.protocol771.read_bytes()).hexdigest()=='11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be'
        protocols[771]=json.loads(a.protocol771.read_text())
        items[771]=next(r['id'] for r in table(a.cache771,'entities') if r['name']=='item')
    if a.admit772:
        assert a.protocol772 and a.cache772 and 772 not in a.closed_clients
        assert hashlib.sha256(a.protocol772.read_bytes()).hexdigest()=='11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be'
        protocols[772]=json.loads(a.protocol772.read_text())
        items[772]=next(r['id'] for r in table(a.cache772,'entities') if r['name']=='item')
    if a.admit773:
        assert a.protocol773 and a.cache773 and 773 not in a.closed_clients
        assert hashlib.sha256(a.protocol773.read_bytes()).hexdigest()=='0f34fce39ca160cf9856013befd52bdc67cf30687306f47dacd007693d5a2d70'
        protocols[773]=json.loads(a.protocol773.read_text())
        items[773]=next(r['id'] for r in table(a.cache773,'entities') if r['name']=='item')
    a.output.mkdir(parents=True,exist_ok=False)
    report=dict(status='failed',binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),checks=[],sessions=[],shutdown=[])
    try:
        for index,(primary,native) in enumerate([('26.1.2',775),('26.3',777)]):
            port=a.port+index
            with socket.socket() as check:
                check.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);check.bind(('127.0.0.1',port))
            with tempfile.TemporaryDirectory(prefix='mcsa-public776-') as folder:
                root=Path(folder);(root/'config').mkdir()
                (root/'config/mcsa.properties').write_text(f'java-version={primary}\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
                (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
                (root/'config/server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
                path=a.output/f'primary{native}.log'
                with path.open('w') as log:
                    server=subprocess.Popen([str(a.binary.resolve())],cwd=root,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
                    client=None
                    try:
                        deadline=time.monotonic()+120
                        while 'Готово (' not in path.read_text():
                            assert server.poll() is None and time.monotonic()<deadline,path.read_text()[-2000:];time.sleep(.1)
                        for version in (*([769] if a.admit769 else []),*([770] if a.admit770 else []),*([771] if a.admit771 else []),*([772] if a.admit772 else []),*([773] if a.admit773 else []),774,776,*a.closed_clients):
                            row=dict(primary=primary,client=version)
                            with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
                                handshake(sock,version,port,1);send(sock,b'\0')
                                response=frame(sock);raw=response.getvalue();assert read_varint(response.read)==0
                                status=json.loads(response.read(read_varint(response.read)))
                                assert status['version']==dict(name=primary,protocol=native),status
                                row['status_hex']=raw.hex()
                            report['checks'].append(f'{primary}/{version}: honest Status{native}')
                            if version in a.closed_clients or (version==774 and not a.admit774):
                                with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
                                    handshake(sock,version,port,2);response=frame(sock);raw=response.getvalue()
                                    assert read_varint(response.read)==0
                                    text=response.read(read_varint(response.read)).decode()
                                    assert 'Ваша версия не подходит под ядро сервера' in text,text
                                    row.update(admitted=False,disconnect_hex=raw.hex(),disconnect=text)
                                report['checks'].append(f'{primary}/{version}: existing Russian rejection')
                            else:
                                client=connect(port,protocols[version],lambda sock,pr:GameplayProbe(sock,pr,items[version],version),version=version,username=f'Mcsa{version}')
                                client.pump(30,lambda _:client.position is not None and 'window_items' in client.seen and 'map_chunk' in client.seen)
                                assert client.position is not None and client.acks and {'login','window_items','map_chunk'}<=set(client.seen)
                                client.pump(.5)
                                token='MCSA_CORE_DIG_900001'
                                server.stdin.write('say '+token+'\n');server.stdin.flush()
                                client.pump(10,lambda _:token in client.markers);assert token in client.markers
                                row.update(admitted=True,position=client.position,teleport_acks=client.acks,records=client.raw,sent=client.sent)
                                report['checks'].append(f'{primary}/{version}: public Login/Configuration/Play/chunks/inventory/ACK/command barrier')
                                client.sock.close();client=None
                            report['sessions'].append(row)
                    finally:
                        if client:client.sock.close()
                        reason=stop_server(server)
                        report['shutdown'].append(dict(primary=primary,pid=server.pid,exit=server.returncode,method=reason))
                        assert server.returncode==0,path.read_text()[-2000:]
        report['status']='pass'
    except BaseException as error:
        report['error']=repr(error);raise
    finally:
        (a.output/'result.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],checks=len(report['checks']))))


if __name__=='__main__':main()
