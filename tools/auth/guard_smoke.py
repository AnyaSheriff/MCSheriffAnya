#!/usr/bin/env python3
"""Guard: настоящий запуск, GitHub, offline-кэш и отказ при подмене. Только временный сервер."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--port', type=int, default=25571)
    args=parser.parse_args()
    binary=str(Path(args.binary).resolve())
    with socket.socket() as probe:
        probe.bind(('127.0.0.1',args.port))
    checks=[]
    with tempfile.TemporaryDirectory(prefix='mcsa-guard-smoke-') as tmp:
        root=Path(tmp)
        (root/'config').mkdir()
        (root/'config/server.properties').write_text(
            f'server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\n'
            'level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n')
        properties=root/'config/mcsa.properties'
        settings='auth-enabled=true\nauth-premium=false\nauth-provider=guard\nbedrock-port=0\nprefetch-chunks=0\n'
        properties.write_text(settings)
        def run(label, success, offline=False, register=False, expected=None):
            env=dict(os.environ)
            if offline:
                env.update(HTTPS_PROXY='http://127.0.0.1:1',https_proxy='http://127.0.0.1:1',
                           ALL_PROXY='http://127.0.0.1:1',all_proxy='http://127.0.0.1:1',NO_PROXY='',no_proxy='')
            log=root/(label+'.log')
            with log.open('w') as out:
                proc=subprocess.Popen([binary],cwd=root,env=env,stdin=subprocess.PIPE,stdout=out,stderr=subprocess.STDOUT,text=True)
                try:
                    deadline=time.monotonic()+90
                    while time.monotonic()<deadline:
                        text=log.read_text()
                        if proc.poll() is not None: break
                        if 'Готово (' in text:
                            assert success, 'unexpected successful startup: '+label
                            if register:
                                proc.stdin.write('auth setpassword GuardTest guard-test-password\n')
                                proc.stdin.flush()
                                until=time.monotonic()+15
                                while time.monotonic()<until and not (root/'auth.db').exists(): time.sleep(.05)
                                while time.monotonic()<until:
                                    db=json.loads((root/'auth.db').read_text())
                                    if 'guardtest' in db['accounts']: break
                                    time.sleep(.05)
                                assert db['version']==2 and db['accounts']['guardtest']['protected']
                                assert not any(db['accounts']['guardtest']['password_hash'])
                            proc.stdin.write('stop\n'); proc.stdin.flush()
                            proc.wait(timeout=30)
                            assert proc.returncode==0
                            break
                        time.sleep(.05)
                    else: raise AssertionError('startup timed out: '+label)
                    if success:
                        assert 'подпись подтверждена' in log.read_text()
                    else:
                        assert proc.poll() is not None and proc.returncode!=0
                    if expected: assert expected in log.read_text(), label
                    assert 'guard-test-password' not in log.read_text()
                    checks.append(label)
                finally:
                    if proc.poll() is None:
                        proc.terminate()
                        try: proc.wait(timeout=15)
                        except subprocess.TimeoutExpired: proc.kill();proc.wait()
        run('download-and-register',True,register=True)
        directory=root/'mcsa-guard'
        marker=(directory/'initialized').read_bytes()
        run('offline-cache',True,offline=True,expected='ранее проверенный')
        assert (directory/'initialized').read_bytes()==marker
        bundle=json.loads((directory/'active.bundle').read_text())
        manifest=json.loads(bytes(bundle['manifest']))
        module=directory/manifest['sha256']
        original=module.read_bytes()
        tampered=bytearray(original);tampered[len(tampered)//2]^=1
        module.write_bytes(tampered)
        run('modified-binary',False,offline=True,expected='повреждён или подменён')
        module.write_bytes(original)
        original_bundle=(directory/'active.bundle').read_bytes()
        bundle['signature'][0]^=1
        (directory/'active.bundle').write_text(json.dumps(bundle))
        run('modified-signature',False,offline=True,expected='подпись не подтверждена')
        (directory/'active.bundle').write_bytes(original_bundle)
        (directory/'pepper').rename(root/'test-secret-backup')
        run('missing-secret',False,offline=True,expected='секрета установки')
        assert not (directory/'pepper').exists()
        (root/'test-secret-backup').rename(directory/'pepper')
        properties.write_text(settings.replace('auth-provider=guard','auth-provider=builtin'))
        run('builtin-downgrade',False,offline=True,expected='требуют auth-provider=guard')
        properties.write_text(settings.replace('auth-provider=guard','auth-provider=garud'))
        run('unknown-provider',False,offline=True,expected='auth-provider должен быть')
    print(json.dumps({'passed':len(checks),'checks':checks},ensure_ascii=False))


if __name__=='__main__': main()
