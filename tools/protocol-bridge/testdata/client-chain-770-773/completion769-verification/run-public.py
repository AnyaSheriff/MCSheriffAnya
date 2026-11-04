import subprocess
from pathlib import Path
r=Path.cwd()/'tools/protocol-bridge';b=Path('/home/lenox/.local/share/mcsa-java-26.3-probe')
a=['python3',str(r/'probe_public_776.py'),'--binary',str(b/'s26-mv5-target/release/mcsheriffanya'),
 '--protocol',str(b/'registry-26.2-json/924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/protocol.json'),'--cache',str(b/'registry-26.2-json')]
for c,cache in [(769,b/'client-chain-json/769'),(770,b/'client-chain-json/770'),(771,b/'client-chain-json/771'),(772,b/'client-chain-json/772'),(773,b/'client-chain-json/773'),(774,b/'step1-json')]:
 a += [f'--protocol{c}',str(cache/'protocol.json'),f'--cache{c}',str(cache),f'--admit{c}']
a += ['--port','26184','--output',str(r/'testdata/client-chain-770-773/completion769-public-final')]
subprocess.run(a,check=True)
