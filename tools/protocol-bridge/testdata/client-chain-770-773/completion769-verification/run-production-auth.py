import subprocess,sys
from pathlib import Path
r=Path.cwd()/'tools/protocol-bridge';b=Path('/home/lenox/.local/share/mcsa-java-26.3-probe')
for client in [769,770]:
 for primary in [775,777]:
  args=['python3',str(r/'probe_chat_login_770.py'),'--binary',str(b/'s26-mv5-target/release/mcsheriffanya'),
    '--protocol',str(b/f'client-chain-json/{client}/protocol.json'),'--client',str(client),'--primary',str(primary),
    '--output',str(r/f'testdata/client-chain-770-773/completion769-production-auth/{client}-password{primary}')]
  result=subprocess.run(args)
  if result.returncode:sys.exit(result.returncode)
