#!/usr/bin/env python3
# Официальный JAR непрозрачен: команды сервера, сохранение, затем чтение region.
# Версия проверяется и по SHA-256 загрузки, и по сообщению запущенного сервера.
import argparse
import hashlib
import json
import re
import subprocess
import time
from pathlib import Path

SHA256='d052f14d7a173734fba553711e5b570162e2f2a313267ee31a21b975a679be64'


def listeners():
    text=subprocess.check_output(['ss','-H','-ltn'],text=True)
    return sorted({int(m[1]) for m in re.finditer(r':(261\d\d)\s',text)})


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--jar',type=Path,required=True)
    p.add_argument('--plan',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--port',type=int,default=26160)
    a=p.parse_args()
    assert 26100<=a.port<=26199
    jar=a.jar.resolve()
    assert hashlib.sha256(jar.read_bytes()).hexdigest()==SHA256,'expected official 26.3 launcher'
    assert not listeners(),'another official server is listening'
    plan=json.loads(a.plan.read_text())
    root=a.output.resolve();root.mkdir(exist_ok=False)
    (root/'eula.txt').write_text('eula=true\n')
    (root/'server.properties').write_text(
        f'server-ip=127.0.0.1\nserver-port={a.port}\nonline-mode=false\nwhite-list=false\n'
        f'level-type=minecraft:normal\nlevel-seed={plan["seed"]}\nview-distance=2\n'
        'simulation-distance=2\nmax-tick-time=-1\npause-when-empty-seconds=0\n'
        'difficulty=peaceful\nenable-query=false\nenable-rcon=false\n')
    path=root/'server.log'
    observed={'seed':plan['seed'],'launcher_sha256':SHA256,'port':a.port,'sites':[]}
    with path.open('w') as log:
        server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx3G',
            '-XX:ActiveProcessorCount=4','-jar',str(jar),'nogui'],cwd=root,
            stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True)
        def cmd(s):
            server.stdin.write(s+'\n');server.stdin.flush()
        def wait(marker,seconds=120):
            deadline=time.monotonic()+seconds
            while marker not in path.read_text():
                assert server.poll() is None and time.monotonic()<deadline,marker
                time.sleep(.2)
        try:
            wait('Done (')
            version=re.search(r'Starting minecraft server version (\S+)',path.read_text())[1]
            assert version=='26.3',version
            observed['version']=version
            assert listeners()==[a.port]
            cmd('gamerule random_tick_speed 0')
            for i,(x,z) in enumerate(plan['vanilla_search_sites']):
                cx,cz=x//16,z//16
                cmd('forceload remove all')
                cmd(f'forceload add {(cx-4)*16} {(cz-4)*16} {(cx+3)*16} {(cz+3)*16}')
                cmd(f'say BADLANDS_LOAD_{i}_DONE');wait(f'BADLANDS_LOAD_{i}_DONE')
                # Forceload ставит генерацию в очередь; перед сохранением даём ей закончиться.
                time.sleep(10)
                cmd('save-all flush');cmd(f'say BADLANDS_SAVE_{i}_DONE')
                wait(f'BADLANDS_SAVE_{i}_DONE')
                observed['sites'].append([x,z])
                (root/'observed-plan.json').write_text(json.dumps(observed,indent=2)+'\n')
                assert listeners()==[a.port]
        finally:
            if server.poll() is None:
                cmd('stop')
                try:server.wait(timeout=60)
                except subprocess.TimeoutExpired:
                    server.terminate();server.wait(timeout=15)
        assert server.returncode==0,server.returncode
    observed['stopped']=True
    (root/'observed-plan.json').write_text(json.dumps(observed,indent=2)+'\n')
    print(json.dumps(observed))


if __name__=='__main__':
    main()
