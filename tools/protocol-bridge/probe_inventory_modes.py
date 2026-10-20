#!/usr/bin/env python3
"""Те же действия инвентаря на775/777; обычные пакеты, без распаковки JAR."""
import argparse
import hashlib
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
from probe_container_session import Probe
from probe_gameplay import connect
from smoke import varint


def main():
    p = argparse.ArgumentParser(description=__doc__)
    target=p.add_mutually_exclusive_group(required=True)
    target.add_argument('--jar',type=Path)
    target.add_argument('--test-binary',type=Path)
    p.add_argument('--version', type=int, choices=[775,777], required=True)
    p.add_argument('--data', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--port', type=int)
    a = p.parse_args()
    expected = {775:'97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51',777:'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}[a.version]
    a.port=a.port or (26026 if a.test_binary else 26024)
    if a.test_binary: assert a.port==26026
    else: assert hashlib.sha1(a.jar.read_bytes()).hexdigest() == expected
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files'] + manifest['baseline_files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    protocol = json.loads((a.data / ('protocol.json' if a.version == 777 else 'protocol-26.1.json')).read_text())
    items = a.data / 'items.json' if a.version == 777 else Path(__file__).resolve().parents[2] / 'tools/data/items.json'
    names = {v['id']:v['name'] for v in json.loads(items.read_text())}
    fixture = json.loads((Path(__file__).parent / 'testdata/legacy-inventory.json').read_text())
    report = {'protocol':a.version,'target':'core' if a.test_binary else 'official','jar_sha1':expected if a.jar else None,'snapshots':[]}
    a.output.mkdir(parents=True,exist_ok=True)
    with socket.socket() as check: check.bind(('127.0.0.1',a.port))
    with tempfile.TemporaryDirectory(prefix='mcsa-inventory-modes-') as directory:
        root=Path(directory)
        (root/'eula.txt').write_text('eula=true\n')
        (root/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=flat\nlevel-seed=1\nview-distance=2\n')
        log=a.output/'server.log'
        with log.open('w') as output:
            if a.test_binary:
                (root/'mcsa-gameplay-probe.marker').touch()
                argv=[str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::serve_java_26_3_probe','--nocapture']
            else: argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(a.jar.resolve()),'nogui']
            server=subprocess.Popen(argv,cwd=root,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True)
            client=None
            def command(value): server.stdin.write(value+'\n');server.stdin.flush()
            try:
                deadline=time.monotonic()+120
                ready='MCSA gameplay probe ready' if a.test_binary else 'Done ('
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic()<deadline
                    time.sleep(.1)
                command('gamerule minecraft:spawn_mobs false');command('gamerule minecraft:random_tick_speed 0')
                client=connect(a.port,protocol,lambda s,p:Probe(s,p,a.version,names),a.version)
                client.pump(10,lambda r:r['name']=='position');client.pump(1)
                for snapshot in fixture['snapshots']:
                    stage=snapshot['stage'].removesuffix('-observe')
                    client.send('close_window',b'\0');client.pump(.2)
                    command('clear McsaProbe');command('kill @e[type=item]')
                    command('gamemode creative McsaProbe' if 'creative' in stage else 'gamemode survival McsaProbe')
                    command('item replace entity McsaProbe hotbar.0 with bedrock 8');client.pump(.5)
                    for action in fixture['actions']:
                        if action['stage'] not in (stage,stage+'-pickup'):continue
                        # Современный сервер проверяет свои вещи независимо от
                        # предсказания пустого курсора и не получает old Slot.
                        inv=client.inventory;inv['cursor']=None
                        client.send('window_click',b'\0'+varint(inv['revision'])+struct.pack('>hb',action['slot'],action['button'])+varint(action['mode'])+b'\0\0')
                        client.pump(.15)
                    # Неверный state ID просит полный авторитетный снимок.
                    # Слот шлема не принимает bedrock, действие не меняет вещи.
                    before=len(client.observed)
                    client.send('window_click',b'\0'+varint(-1)+struct.pack('>hb',5,0)+b'\0\0\0')
                    client.pump(.3)
                    if not a.test_binary: assert any(r['name']=='window_items' for r in client.observed[before:]),stage
                    current={'stage':stage,'slots':list(client.inventory['slots']),'cursor':client.inventory['cursor']}
                    if a.test_binary:
                        def plain(v): return None if v is None else {'item':'bedrock','count':v['count']}
                        assert current['slots']==[plain(v) for v in snapshot['slots']],(stage,'slots',current)
                        assert current['cursor']==plain(snapshot['cursor']),(stage,'cursor',current)
                    report['snapshots'].append(current)
            except BaseException as error: report['error']=str(error);raise
            finally:
                if client: report['packets']=client.observed;client.sock.close()
                if server.poll() is None:
                    command('stop');server.stdin.close()
                    try:server.wait(timeout=30)
                    except subprocess.TimeoutExpired:server.kill();server.wait()
                report['exit_code']=server.returncode
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    assert report['exit_code']==0
    print(json.dumps({'protocol':a.version,'cases':len(report['snapshots']),'creative_drag':next(r['cursor'] for r in report['snapshots'] if r['stage']=='creative-drag')}))


if __name__=='__main__':main()
