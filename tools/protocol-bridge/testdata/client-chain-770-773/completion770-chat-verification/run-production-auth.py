import os,subprocess,sys
from pathlib import Path
r=Path.cwd()/'tools/protocol-bridge';b=Path('/home/lenox/.local/share/mcsa-java-26.3-probe')
out=r/'testdata/client-chain-770-773/completion770-chat-production-auth';out.mkdir(exist_ok=True)
for primary in [775,777]:
 folder=out/f'770-password{primary}'
 args=['python3',str(r/'probe_chat_login_770.py'),'--binary',str(b/'s26-mv5-target/release/mcsheriffanya'),
  '--protocol',str(b/'client-chain-json/770/protocol.json'),'--primary',str(primary),'--output',str(folder)]
 with (out/(folder.name+'.log')).open('w') as log:result=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT,env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'})
 print(folder.name,result.returncode,flush=True)
 if result.returncode:sys.exit(result.returncode)
