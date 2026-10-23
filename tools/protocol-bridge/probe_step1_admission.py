#!/usr/bin/env python3
"""Сырые Status/Login двух профилей: незавершённые774/776 должны получить отказ.

Это проверка предохранителя, не доказательство игрового входа этих версий.
"""
import argparse
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from smoke import frame, handshake, read_varint, send
from probe_core_776 import stop_server


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--port',type=int,default=26186)
    args = p.parse_args()
    assert 26100 <= args.port <= 26198
    binary = args.binary.resolve()
    report = {'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'checks':[],
              'scope':'Admission guard only; gameplay774/776 is unproven and closed','sessions':[]}
    for offset, (primary, native) in enumerate([('26.1.2',775),('26.3',777)]):
        port = args.port + offset
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1',port))
        with tempfile.TemporaryDirectory(prefix='mcsa-mv1-admission-') as folder:
            root = Path(folder)
            (root/'config').mkdir()
            (root/'config/mcsa.properties').write_text(f'java-version={primary}\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
            (root/'config/skins.toml').write_text('mojang=false\nely=false\n')
            (root/'config/server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
            log = root/'server.log'
            with log.open('w') as output:
                server = subprocess.Popen([str(binary)],cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
                try:
                    deadline = time.monotonic()+120
                    while 'Готово (' not in log.read_text():
                        assert server.poll() is None and time.monotonic()<deadline,log.read_text()[-2000:]
                        time.sleep(.1)
                    for client in [774,776]:
                        row = {'primary':primary,'client':client,'port':port}
                        with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
                            handshake(sock,client,port,1);send(sock,b'\0')
                            response = frame(sock);raw = response.getvalue();assert read_varint(response.read)==0
                            row['status_hex'] = raw.hex()
                            status = json.loads(response.read(read_varint(response.read)))
                            assert status['version']=={'name':primary,'protocol':native},status
                        report['checks'].append(f'{primary}/client{client}: honest status{native}')
                        with socket.create_connection(('127.0.0.1',port),timeout=10) as sock:
                            handshake(sock,client,port,2)
                            response = frame(sock);raw = response.getvalue();assert read_varint(response.read)==0
                            text = response.read(read_varint(response.read)).decode()
                            assert 'version' in text.lower() or 'верси' in text.lower(),text
                            row['disconnect_hex'] = raw.hex();row['disconnect'] = text
                        report['checks'].append(f'{primary}/client{client}: rejected before LoginStart')
                        row['gameplay_checks'] = 'not run: admission closed'
                        report['sessions'].append(row)
                finally:
                    reason = stop_server(server)
                    report.setdefault('shutdown',[]).append({'primary':primary,'pid':server.pid,'exit':server.returncode,'method':reason})
                    assert server.returncode==0,(reason,log.read_text()[-2000:])
    report['status']='pass'
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'status':'pass','checks':len(report['checks'])}))


if __name__=='__main__':
    main()
