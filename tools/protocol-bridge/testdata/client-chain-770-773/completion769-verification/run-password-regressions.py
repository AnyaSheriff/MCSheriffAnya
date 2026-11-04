import json,os,subprocess,time
from pathlib import Path
r=Path.cwd()/'tools/protocol-bridge';base=r/'testdata/client-chain-770-773'
last=base/'completion769-regression/776-dimensions777/result.json'
deadline=time.monotonic()+900
while not last.exists():
 assert time.monotonic()<deadline,'regression deadline'
 time.sleep(1)
assert json.loads(last.read_text())['status']=='pass'
subprocess.run(['python3',str(base/'completion769-verification/run-checklist.py'),
 '/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv16-769-test-v7',
 'completion769-regression-passwords','771,772,773,774,776','password'],check=True,
 env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
