#!/usr/bin/env python3
"""Production: честный Status, открытый776 Play и прежний отказ774 на двух основных."""
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
    p.add_argument('--port',type=int,default=26184)
    a=p.parse_args();assert 26100<=a.port<=26198
    assert hashlib.sha256(a.protocol.read_bytes()).hexdigest()==PROTOCOL_SHA
    protocol=json.loads(a.protocol.read_text())
    item=next(r['id'] for r in table(a.cache,'entities') if r['name']=='item')
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
                        for version in (774,776):
                            row=dict(primary=primary,client=version)
                            with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
                                handshake(sock,version,port,1);send(sock,b'\0')
                                response=frame(sock);raw=response.getvalue();assert read_varint(response.read)==0
                                status=json.loads(response.read(read_varint(response.read)))
                                assert status['version']==dict(name=primary,protocol=native),status
                                row['status_hex']=raw.hex()
                            report['checks'].append(f'{primary}/{version}: honest Status{native}')
                            if version==774:
                                with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
                                    handshake(sock,version,port,2);response=frame(sock);raw=response.getvalue()
                                    assert read_varint(response.read)==0
                                    text=response.read(read_varint(response.read)).decode()
                                    assert 'Ваша версия не подходит под ядро сервера' in text,text
                                    row.update(admitted=False,disconnect_hex=raw.hex(),disconnect=text)
                                report['checks'].append(f'{primary}/774: existing Russian rejection')
                            else:
                                client=connect(port,protocol,lambda sock,pr:GameplayProbe(sock,pr,item,776),version=776)
                                client.pump(30,lambda _:client.position is not None and 'window_items' in client.seen and 'map_chunk' in client.seen)
                                assert client.position is not None and client.acks and {'login','window_items','map_chunk'}<=set(client.seen)
                                client.pump(.5)
                                token='MCSA_CORE_DIG_900001'
                                server.stdin.write('say '+token+'\n');server.stdin.flush()
                                client.pump(10,lambda _:token in client.markers);assert token in client.markers
                                row.update(admitted=True,position=client.position,teleport_acks=client.acks,records=client.raw,sent=client.sent)
                                report['checks'].append(f'{primary}/776: public Login/Configuration/Play/chunks/inventory/ACK/command barrier')
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
