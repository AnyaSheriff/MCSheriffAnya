#!/usr/bin/env python3
"""Непрозрачный1.21.5: ввод пароля через GUI, без записи исходящих пакетов."""
import argparse
import hashlib
import json
import secrets
import subprocess
import tempfile
import time
from pathlib import Path
from live_client import launch
from probe_core_776_client import gui, stop_client_bounded
from probe_core_776 import stop_server
from probe_initial_play_776 import capture


def wait(predicate, seconds=90):
    end=time.monotonic()+seconds
    while not predicate():
        if time.monotonic()>end:raise TimeoutError('private GUI barrier')
        time.sleep(.1)


def main():
    p=argparse.ArgumentParser()
    for name in ('test-binary','client-dir','output'):p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--primary',type=int,choices=(775,777),required=True)
    a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    report=dict(client=770,primary=a.primary,status='failed',checks=[],binary_sha256=hashlib.sha256(a.test_binary.read_bytes()).hexdigest())
    password=secrets.token_hex(16);wrong=secrets.token_hex(16)
    client=server=None
    with tempfile.TemporaryDirectory(prefix='mcsa-chat770-gui-') as folder:
        root=Path(folder);core=root/'core';core.mkdir()
        for marker in ['mcsa-gameplay-probe.marker','mcsa-password-probe.marker']:(core/marker).touch()
        if a.primary==777:(core/'mcsa-catalog-preview263.marker').touch()
        server_path=a.output/'server.log'
        with server_path.open('w') as log:
            try:
                server=subprocess.Popen([str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::serve_java_1_21_5_probe','--nocapture'],
                    cwd=core,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
                wait(lambda:'MCSA gameplay probe ready' in server_path.read_text())
                for index in (0,1):
                    game=root/f'game{index}';game.mkdir()
                    client_path=a.output/f'client{index}.log'
                    with client_path.open('w') as clog:
                        client=launch(a.client_dir.resolve(),game,26127,clog,version='1.21.5',gui_scale=2,pause_on_lost_focus=False)
                        wait(lambda:('/register' if index==0 else '/login') in client_path.read_text())
                        time.sleep(2)
                        assert server_path.read_text().count('вошёл с номером сущности')==index
                        if index==0:assert not list((core/'world/playerdata').iterdir())
                        def enter(text):
                            gui(client,'key','t');time.sleep(.2)
                            gui(client,'type','--clearmodifiers','--delay','1',text)
                            gui(client,'key','Return')
                        if index:
                            enter('/login '+wrong)
                            wait(lambda:'Неверный пароль' in client_path.read_text())
                            assert server_path.read_text().count('вошёл с номером сущности')==index
                            report['checks'].append('gui-wrong-password-stays-in-limbo')
                        enter('/login '+password if index else '/register '+password+' '+password)
                        wait(lambda:server_path.read_text().count('вошёл с номером сущности')==index+1)
                        time.sleep(3)
                        token=f'MCSA_CORE_DIG_{110000+index}'
                        server.stdin.write('say '+token+'\n');server.stdin.flush()
                        wait(lambda:token in client_path.read_text())
                        assert client.poll() is None
                        assert 'Failed to handle packet' not in client_path.read_text()
                        screenshot=a.output/f'entered{index}.png'
                        report.setdefault('screenshots',[]).append(dict(file=screenshot.name,sha256=capture(client,screenshot)))
                        report['checks'].append('gui-register-and-play' if index==0 else 'gui-relogin-and-play')
                        stop_client_bounded(client);client=None
                        wait(lambda:server_path.read_text().count('McsaProbe вышел')==index+1)
                report['status']='pass'
            except BaseException as error:
                # Ошибки xdotool могут содержать argv с паролем; сохраняем только класс.
                report['error_type']=type(error).__name__
            finally:
                if client:stop_client_bounded(client)
                if server:report['server_exit']=stop_server(server)
        for path in a.output.glob('*.log'):
            text=path.read_text();assert password not in text and wrong not in text
        report['checks'].append('credentials-absent-from-client-and-server-logs')
    (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(report)
    if report['status']!='pass':raise SystemExit(1)

if __name__=='__main__':main()
