#!/usr/bin/env python3
"""CI-only обычный777 ELF, клиенты774/777,10импульсов и повторный вход."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import threading
import time
import traceback

from probe_level_events_original import Observer, pinned, validate, ROOT
from probe_gameplay import connect, packed
from smoke import varint


class LiveObserver(Observer):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.keepalives = 0
        self.keepalive_samples = []
        self.born = time.monotonic()

    def observe_packet(self, name, payload):
        super().observe_packet(name, payload)
        if name == 'keep_alive':
            self.keepalives += 1
            self.keepalive_samples.append({'phase':self.phase,'elapsed':time.monotonic()-self.born})


def run(version, port, output, binary):
    folder = output/str(version); folder.mkdir()
    report = {'version':version,'port':port,'passed':False,'commands':[],'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}
    process = client = worker = None
    peers=[]; worker_errors=[]; lock=threading.Lock()
    try:
        with socket.socket() as check: check.bind(('127.0.0.1',port))
        if version == 774:
            pin=json.loads((ROOT/'tools/protocol-bridge/step1-sources-774.json').read_text())['protocol']
        else:
            pin=next(x for x in json.loads((ROOT/'tools/blackbox/ci/sources-777.json').read_text())['json_files'] if x['file']=='protocol.json')
        raw=pinned(pin,folder/'protocol.json'); protocol=json.loads(raw); report['protocol_sha256']=hashlib.sha256(raw).hexdigest()
        config=folder/'config';config.mkdir()
        (config/'mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
        (config/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\ngamemode=creative\nmax-players=3\n')
        log=(folder/'server.log').open('w')
        process=subprocess.Popen([str(binary)],cwd=folder,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True)
        deadline=time.monotonic()+60
        while True:
            if process.poll() is not None or time.monotonic()>deadline: raise RuntimeError('candidate startup failed')
            with socket.socket() as check:
                if check.connect_ex(('127.0.0.1',port))==0:break
            time.sleep(.1)
        def join():
            c=connect(port,protocol,LiveObserver,version=version,username='LevelProbe');c.version=version;peers.append(c);c.pump(3,lambda r:c.position is not None);return c
        client=join()
        def send_command(text, phase):
            with lock:
                report['commands'].append({'phase':phase,'text':text})
                process.stdin.write(text+'\n');process.stdin.flush()
        def command(text): send_command(text,client.phase);client.pump(.7)
        command('tp LevelProbe 40.5 101 42.5 180 20');command('fill 38 99 38 44 99 44 stone')
        client.phase='empty';command('setblock 40 100 40 dispenser[facing=east]');command('setblock 40 101 40 redstone_block');command('setblock 40 101 40 air')
        client.phase='item';command('item replace block 40 100 40 container.0 with stone 1');command('setblock 40 101 40 redstone_block');command('setblock 40 101 40 air')
        command('setblock 42 100 40 jukebox');command('item replace entity LevelProbe weapon.mainhand with music_disc_cat 1')
        def click(sequence):
            client.send('block_place',b'\0'+packed(42,100,40)+b'\1'+struct.pack('>fff',.5,1.,.5)+b'\0\0'+varint(sequence));client.pump(2)
        client.phase='jukebox-start';click(1);client.phase='jukebox-stop';click(2);validate(client.selected)
        client.phase='clock-before-reconnect'
        def pulse():
            try:
                for i in range(10):
                    send_command('setblock 40 101 40 redstone_block',f'pulse-{i+1}-on');time.sleep(.6)
                    send_command('setblock 40 101 40 air',f'pulse-{i+1}-off');time.sleep(.6)
            except Exception as e:worker_errors.append(repr(e))
        worker=threading.Thread(target=pulse);worker.start();client.pump(3.6);client.sock.close();time.sleep(.7)
        client=join();client.phase='clock-after-reconnect';client.pump(20);worker.join(timeout=2)
        if worker.is_alive() or worker_errors:raise RuntimeError('pulse worker incomplete:'+repr(worker_errors))
        for c in peers:
            phase='clock-before-reconnect' if c is peers[0] else 'clock-after-reconnect'
            if not any(r['phase']==phase and r['name']=='world_event' and r['event']['effect']==1001 for r in c.selected):raise ValueError('clock1001 not observed in '+phase)
        if not any(x['elapsed']>=10 and x['phase']=='clock-after-reconnect' for x in client.keepalive_samples):raise ValueError('no late post-reconnect keepalive')
        report['post_reconnect_keepalives']=client.keepalives;report['pulses_completed']=10;report['passed']=True
    except Exception as error:
        report['error']=repr(error)
        report['traceback']=traceback.format_exc()
    finally:
        if worker:
            worker.join(timeout=15)
            if worker.is_alive():report['passed']=False;report['worker_alive']=True
        report['connections']=[{'selected_packets':c.selected,'keepalives':c.keepalives,'keepalive_samples':c.keepalive_samples} for c in peers]
        for c in peers:c.sock.close()
        if process:
            try:
                if process.poll() is None:
                    process.stdin.write('stop\n');process.stdin.flush();process.wait(timeout=30)
            except Exception as error:
                report['stop_error']=repr(error);report['forced_kill']=True;os.killpg(process.pid,signal.SIGKILL);process.wait(timeout=10)
            report['server_exit']=process.returncode
            try:os.killpg(process.pid,0);report['process_group_gone']=False
            except ProcessLookupError:report['process_group_gone']=True
            if process.returncode!=0 or not report['process_group_gone'] or report.get('forced_kill'):report['passed']=False
        with socket.socket() as check:report['port_free']=check.connect_ex(('127.0.0.1',port))!=0
        if not report['port_free']:report['passed']=False
        (folder/'observations.json').write_text(json.dumps(report,indent=2)+'\n')
    return report['passed']


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--binary',type=Path,required=True);args=p.parse_args()
    if os.environ.get('GITHUB_ACTIONS')!='true':p.error('CI-only runtime probe')
    args.output.mkdir(parents=True,exist_ok=False);binary=args.binary.resolve()
    outcomes=[run(v,port,args.output,binary) for v,port in [(774,26354),(777,26356)]]
    if not all(outcomes):raise SystemExit('Candidate observation failed; raw evidence retained')


if __name__=='__main__':main()
