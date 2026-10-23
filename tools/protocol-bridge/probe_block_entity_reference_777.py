#!/usr/bin/env python3
"""Один оригинал777: только авторские блок-сущности, RCON и stdin=/dev/null."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

from probe_block_entities import BLOCKS, Probe
from probe_gameplay import connect
from probe_white_concrete_slab import inputs, JAR_SHA1
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'measure'))
from pregen import Rcon


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('jar','data','output'):p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--port',type=int,default=26190)
    a=p.parse_args();assert 26100<=a.port<=26198
    assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==JAR_SHA1
    tables=inputs(a.data)
    for port in (a.port,a.port+1):
        with socket.socket() as check:check.bind(('127.0.0.1',port))
    a.output.mkdir(parents=True,exist_ok=False)
    report=dict(status='failed',protocol=777,commands=[],runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                jar_sha256=hashlib.sha256(a.jar.read_bytes()).hexdigest(),ports=[a.port,a.port+1])
    with tempfile.TemporaryDirectory(prefix='mcsa-blockentity777-reference-') as folder:
        root=Path(folder);shutil.copyfile(a.jar,root/'server.jar')
        (root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nrcon.port={a.port+1}\n'
            'enable-rcon=true\nrcon.password=local-blockentity-probe\nonline-mode=false\nwhite-list=false\n'
            'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\n'
            'view-distance=2\nsimulation-distance=2\nspawn-protection=0\nmax-tick-time=0\n')
        client=None
        with (a.output/'server.log').open('w') as log:
            server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar','server.jar','nogui'],
                cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT)
            report['pid']=server.pid
            try:
                deadline=time.monotonic()+120
                while 'Done (' not in (a.output/'server.log').read_text():
                    assert server.poll() is None and time.monotonic()<deadline;time.sleep(.2)
                rcon=Rcon(a.port+1,'local-blockentity-probe')
                def command(text):report['commands'].append(dict(command=text,result=rcon.run(text)))
                command('forceload add -16 -16')
                command('fill -16 64 -16 -1 64 -1 stone')
                command('fill -16 66 -16 -1 66 -1 stone')
                for i,block in enumerate(BLOCKS):
                    suffix='[attached=true]' if block=='oak_hanging_sign' else ''
                    if block=='oak_sign':suffix='{front_text:{messages:["MCSA probe","","",""]}}'
                    command(f'setblock {-16+i%16} 65 {-16+i//16} minecraft:{block}'+suffix)
                time.sleep(1)
                command('execute if block -8 65 -15 white_bed run say MCSA_BED_PRESENT')
                client=connect(a.port,tables['protocol.json'],lambda sock,p:Probe(sock,p,777),version=777)
                deadline=time.monotonic()+30
                while client.entities is None:
                    assert time.monotonic()<deadline;client.pump(.2)
                report['entities']=client.entities
                report['missing']=[block for block in BLOCKS if block not in {row['block'] for row in client.entities}]
                assert report['missing']==['piston','white_bed'],report['missing']
                assert len(client.entities)==43
                report['status']='pass'
            except BaseException as error:
                report['error']=repr(error);raise
            finally:
                if client:client.sock.close()
                server.terminate()
                try:server.wait(timeout=30)
                except subprocess.TimeoutExpired:server.kill();server.wait(timeout=10)
                report['server_exit']=server.returncode
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],entities=len(report['entities']),missing=report['missing'])))


if __name__=='__main__':main()
