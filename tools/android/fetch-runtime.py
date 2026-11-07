"""Скачать только закреплённый успешный собственный native run; gh управляет учётными данными."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

REPO = Path(__file__).resolve().parents[2]


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', 'repos/AnyaSheriff/MCSheriffAnya/' + path]))


def main():
    pin = json.loads((REPO / 'android/runtime-pin.json').read_text())
    run = api('actions/runs/' + pin['run_id'])
    assert run['status'] == 'completed' and run['conclusion'] == 'success'
    assert run['head_sha'] == pin['head'] and run['path'] == '.github/workflows/android-apk-native.yml'
    rows = api('actions/runs/' + pin['run_id'] + '/artifacts')['artifacts']
    artifacts = [a for a in rows if a['name'] == pin['artifact_name'] and not a['expired']]
    assert len(artifacts) == 1 and artifacts[0]['digest'] == 'sha256:' + pin['artifact_sha256']
    root = REPO / 'android/runtime'
    assert not root.exists(), 'Use a fresh checkout; never mix artifacts'
    subprocess.run(['gh', 'run', 'download', pin['run_id'], '--repo', 'AnyaSheriff/MCSheriffAnya', '--name', pin['artifact_name'], '--dir', str(root)], check=True)
    receipt = json.loads((root / 'receipt.json').read_text())
    assert receipt == {'source_head': pin['head'], 'api': pin['api'], 'runtimes': pin['runtimes']}
    for abi, expected in pin['runtimes'].items():
        binary = root / abi / 'libmcsa.so'
        assert binary.stat().st_size == expected['bytes'] and hashlib.sha256(binary.read_bytes()).hexdigest() == expected['sha256']
    (root / 'selection.json').write_text(json.dumps({'run_id': run['id'], 'head': run['head_sha'], 'conclusion': run['conclusion'], 'artifact_id': artifacts[0]['id'], 'pin': pin}, indent=2) + '\n')
    print('PASS: exact successful native run, artifact identity, receipt and all three runtime hashes')


if __name__ == '__main__':
    main()
