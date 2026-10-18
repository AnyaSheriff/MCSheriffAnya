#!/usr/bin/env python3
"""Первый запуск настоящего бинарника: pipe и PTY, отдельные папки и порт."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import socket
import subprocess
import tempfile
import time

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    binary=str(a.binary.resolve());a.output.mkdir(parents=True,exist_ok=True)
    checks=[]
    with tempfile.TemporaryDirectory(prefix='mcsa-first-start-') as directory:
        root=Path(directory);fresh=root/'headless';fresh.mkdir()
        for attempt in range(2):
            result=subprocess.run([binary],cwd=fresh,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=20)
            (a.output/f'headless-{attempt}.log').write_text(result.stdout+result.stderr)
            assert result.returncode==0
            assert not (fresh/'world').exists() and not (fresh/'logs').exists()
            text=(fresh/'config/mcsa.properties').read_text();assert 'java-version=choose' in text and 'send-error-reports=choose' in text
            assert 'Запуск остановлен до выбора' in result.stdout
        checks.append('headless-stops-before-world-and-logs')
        panel=root/'panel';(panel/'config').mkdir(parents=True);(panel/'world').mkdir()
        (panel/'config/mcsa.properties').write_text('bedrock-port=0\nauth-enabled=false\n')
        result=subprocess.run([binary],cwd=panel,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=20)
        (a.output/'panel.log').write_text(result.stdout+result.stderr)
        assert result.returncode==0 and 'Запуск остановлен до выбора' in result.stdout
        text=(panel/'config/mcsa.properties').read_text();assert 'java-version=choose' in text and 'send-error-reports=choose' in text and 'bedrock-port=0' in text
        assert not list((panel/'world').iterdir()) and not (panel/'logs').exists()
        checks.append('panel-bootstrap-also-requires-both-choices')
        interactive=root/'interactive';(interactive/'config').mkdir(parents=True)
        (interactive/'config/server.properties').write_text('server-ip=127.0.0.1\nserver-port=26029\nonline-mode=false\nlevel-type=minecraft:flat\nview-distance=2\nsimulation-distance=2\n')
        (interactive/'config/mcsa.properties').write_text('java-version=choose\nsend-error-reports=choose\nbedrock-port=0\nprefetch-chunks=0\n')
        with socket.socket() as check:check.bind(('127.0.0.1',26029))
        master,slave=pty.openpty();output=bytearray()
        server=subprocess.Popen([binary],cwd=interactive,stdin=slave,stdout=slave,stderr=slave,start_new_session=True);os.close(slave)
        def wait_for(text):
            deadline=time.monotonic()+30
            while text not in output.decode(errors='replace'):
                assert time.monotonic()<deadline and server.poll() is None,output.decode(errors='replace')[-2000:]
                if select.select([master],[],[],.2)[0]:output.extend(os.read(master,65536))
        try:
            wait_for('Номер или версия');assert not (interactive/'world').exists()
            os.write(master,b'\n');wait_for('Пустой ввод не выбирает')
            os.write(master,b'1\n');wait_for('Отправлять отчёты?')
            assert not (interactive/'world').exists()
            os.write(master,b'\n');wait_for('Пустой ввод не считается согласием')
            assert 'send-error-reports=choose' in (interactive/'config/mcsa.properties').read_text()
            os.write(master,b'no\n');wait_for('Готово (')
            config=(interactive/'config/mcsa.properties').read_text();assert 'choose' not in config and 'send-error-reports=false' in config
            assert not (interactive/'config/install-id').exists() and not (interactive/'logs/reports').exists()
            os.write(master,b'stop\n');server.wait(timeout=30);assert server.returncode==0
            checks.append('pty-explicit-version-and-no-reports-before-start')
        finally:
            if server.poll() is None:server.kill();server.wait()
            os.close(master);(a.output/'pty.log').write_bytes(output)
        # Существующая установка стартует без повторного диалога.
        result=subprocess.run([binary],input='stop\n',cwd=interactive,capture_output=True,text=True,timeout=30)
        (a.output/'restart.log').write_text(result.stdout+result.stderr)
        assert result.returncode==0 and 'Первоначальная настройка' not in result.stdout
        checks.append('saved-selection-is-not-asked-again')
    report={'checks':checks,'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest()}
    (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
