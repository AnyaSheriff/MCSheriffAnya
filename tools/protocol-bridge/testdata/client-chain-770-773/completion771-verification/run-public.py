"""Все12клиент/основной пары: прежние Status/Play/русский отказ."""
import subprocess
from pathlib import Path
root=Path.cwd();base=Path('/home/lenox/.local/share/mcsa-java-26.3-probe')
runner=root/'tools/protocol-bridge';proof=runner/'testdata/client-chain-770-773'
argv=['python3',str(runner/'probe_public_776.py'),'--binary',str(base/'s26-mv5-target/release/mcsheriffanya'),
      '--protocol',str(base/'registry-26.2-json/924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/protocol.json'),
      '--cache',str(base/'registry-26.2-json'),'--protocol774',str(base/'step1-json/protocol.json'),'--cache774',str(base/'step1-json'),'--admit774',
      '--protocol773',str(base/'client-chain-json/773/protocol.json'),'--cache773',str(base/'client-chain-json/773'),'--admit773',
      '--protocol772',str(base/'client-chain-json/772/protocol.json'),'--cache772',str(base/'client-chain-json/772'),'--admit772',
      '--protocol771',str(base/'client-chain-json/771/protocol.json'),'--cache771',str(base/'client-chain-json/771'),'--admit771',
      '--closed-clients','770','--output',str(proof/'completion771-public-final'),'--port','26184']
with (proof/'completion771-verification/public-final.log').open('w') as output:
    subprocess.run(argv,stdout=output,stderr=subprocess.STDOUT,check=True)
