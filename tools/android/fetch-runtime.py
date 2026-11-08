"""Скачать только закреплённый успешный собственный native run; gh управляет учётными данными."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import shutil
import tempfile

REPO = Path(__file__).resolve().parents[2]


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', 'repos/AnyaSheriff/MCSheriffAnya/' + path]))


def main():
    pin = json.loads((REPO / 'android/runtime-pin.json').read_text())
    run = api('actions/runs/' + pin['run_id'])
    assert run['status'] == 'completed' and run['conclusion'] == 'success'
    expected_workflow = '.github/workflows/android-core-native.yml' if pin.get('format') == 2 else '.github/workflows/android-apk-native.yml'
    assert run['head_sha'] == pin['head'] and run['path'] == expected_workflow
    rows = api('actions/runs/' + pin['run_id'] + '/artifacts')['artifacts']
    artifacts = [a for a in rows if a['name'] == pin['artifact_name'] and not a['expired']]
    assert len(artifacts) == 1 and artifacts[0]['digest'] == 'sha256:' + pin['artifact_sha256']
    root = REPO / 'android/runtime'
    assert not root.exists(), 'Use a fresh checkout; never mix artifacts'
    with tempfile.TemporaryDirectory(prefix='mcsa-native-fetch-') as directory:
        downloaded=Path(directory)
        subprocess.run(['gh', 'run', 'download', pin['run_id'], '--repo', 'AnyaSheriff/MCSheriffAnya', '--name', pin['artifact_name'], '--dir', str(downloaded)], check=True)
        receipt = json.loads((downloaded / 'receipt.json').read_text())
        expected_receipt={'source_head': pin['head'], 'api': pin['api'], 'runtimes': pin['runtimes']}
        if pin.get('format')==2:
            expected_receipt.update(format=2,host_api=pin['host_api'],core_revision=pin['core_revision'],updates=pin['updates'])
        assert receipt==expected_receipt
        for abi, expected in pin['runtimes'].items():
            files={'libmcsa.so':expected['host'],'libmcsa_core.so':expected['core']} if pin.get('format')==2 else {'libmcsa.so':expected}
            for name,info in files.items():
                binary=downloaded/abi/name
                assert binary.stat().st_size==info['bytes'] and hashlib.sha256(binary.read_bytes()).hexdigest()==info['sha256']
        if pin.get('format')==2:
            for revision,update in pin['updates'].items():
                for abi,info in update['runtimes'].items():
                    binary=downloaded/'updates'/revision/abi/'libmcsa_core.so'
                    assert binary.stat().st_size==info['bytes'] and hashlib.sha256(binary.read_bytes()).hexdigest()==info['sha256']
        root.mkdir()
        for abi in pin['runtimes']:shutil.copytree(downloaded/abi,root/abi)
        shutil.copy2(downloaded/'receipt.json',root/'receipt.json')
    (root / 'selection.json').write_text(json.dumps({'run_id': run['id'], 'head': run['head_sha'], 'conclusion': run['conclusion'], 'artifact_id': artifacts[0]['id'], 'pin': pin}, indent=2) + '\n')
    print('PASS: exact successful native run, artifact identity, receipt and every host/core hash')


if __name__ == '__main__':
    main()
