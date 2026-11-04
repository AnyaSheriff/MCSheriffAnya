import subprocess,os,json,socket,time
from pathlib import Path
r=Path.cwd()/'tools/protocol-bridge';base=Path('/home/lenox/.local/share/mcsa-java-26.3-probe');out=r/'testdata/client-chain-770-773/completion770-password'
out.mkdir(exist_ok=True);rows=[]
for primary in [775,777]:
 name=f'770-password{primary}'
 if (out/name/'result.json').exists():continue
 deadline=time.monotonic()+100
 while True:
  try:
   with socket.socket() as check:check.bind(('127.0.0.1',26127))
   break
  except OSError:
   assert time.monotonic()<deadline
   time.sleep(1)
 cmd=['python3',str(r/'probe_step1_dimensions_recipes_776.py'),'--test-binary',str(base/'s26-mv5-target/release/deps/mcsheriffanya-12cfc85e9e96ee73'),'--protocol',str(base/'client-chain-json/770/protocol.json'),'--cache',str(base/'client-chain-json/770'),'--client','770','--primary',str(primary),'--password','--output',str(out/name)]
 with (out/(name+'.log')).open('w') as log:result=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
 report=json.loads((out/name/'result.json').read_text());assert result.returncode!=0 and report['status']=='failed'
 rows.append({'primary':primary,'exit':result.returncode,'error':report['error']});print(rows[-1],flush=True)
(out/'jobs.json').write_text(json.dumps(rows,indent=2)+'\n')
