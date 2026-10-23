import subprocess,json
from pathlib import Path
root=Path.cwd();base=Path('/home/lenox/.local/share/mcsa-java-26.3-probe');runner=root/'tools/protocol-bridge';out=runner/'testdata/step1-774-776';binary=base/'s26-mv5-target/release/deps/mcsheriffanya-b5a369944de6d5e0'
protocols={774:base/'step1-json/protocol.json',776:base/'registry-26.2-json/924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/protocol.json'}
caches={774:base/'step1-json',776:base/'registry-26.2-json'}
for client in (774,776):
 for primary in (775,777):
  name=f'{client}-gameplay{primary}-final';cmd=['python3',str(runner/'probe_step1_gameplay_776.py'),'--client',str(client),'--test-binary',str(binary),'--protocol',str(protocols[client]),'--native-protocol',str(base/'candidate-json/protocol-26.1.json'),'--cache',str(caches[client]),'--primary',str(primary),'--peer','777','--peer-protocol',str(base/'candidate-json/protocol.json'),'--restart','--output',str(out/name)]
  with open('/tmp/s26-mv8-'+name+'.log','w') as log:subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,check=True)
  x=json.loads((out/name/'result.json').read_text());assert x['status']=='pass' and len(x['checks'])==11;print(name,'11/11 PASS',flush=True)
for primary in (775,777):
 for password in (False,True):
  name=f'776-'+('password' if password else 'dimensions')+f'{primary}-regression';cmd=['python3',str(runner/'probe_step1_dimensions_recipes_776.py'),'--test-binary',str(binary),'--protocol',str(protocols[776]),'--cache',str(caches[776]),'--primary',str(primary),'--output',str(out/name)]
  if password:cmd.append('--password')
  with open('/tmp/s26-mv8-'+name+'.log','w') as log:subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,check=True)
  x=json.loads((out/name/'result.json').read_text());assert x['status']=='pass';print(name,'PASS',flush=True)
