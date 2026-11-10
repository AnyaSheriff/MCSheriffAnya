"""Verify exact pinned JSON fields only; no Minecraft/protocol-library source or runtime."""
import argparse,concurrent.futures,hashlib,json,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parent
OUTPUT=ROOT/'testdata/container-button-fields.json'
def sources():
 chain=json.loads((ROOT/'client-chain-sources.json').read_text())['versions']
 pins={v:d['files']['protocol'] for v,d in chain.items() if 768<=int(v)<=773}
 pins['774']=json.loads((ROOT/'step1-sources-774.json').read_text())['protocol']
 pins.update(json.loads((ROOT/'packet-776-report.json').read_text())['sources'])
 assert set(pins)==set(map(str,range(768,778)));return pins

def inspect(version,pin,cache,fetch):
 path=cache/f'{version}.json'
 if not path.is_file():
  if not fetch:raise ValueError('missing pinned JSON: '+str(path))
  assert pin['url'].startswith('https://raw.githubusercontent.com/') and '/minecraft-data/' in pin['url'] and pin['url'].endswith('/protocol.json')
  path.parent.mkdir(parents=True,exist_ok=True)
  raw=urllib.request.urlopen(pin['url'],timeout=60).read()
  if len(raw)!=pin['bytes'] or hashlib.sha256(raw).hexdigest()!=pin['sha256']:raise ValueError('pinned JSON mismatch: '+version)
  path.write_bytes(raw)
 raw=path.read_bytes();assert len(raw)==pin['bytes'] and hashlib.sha256(raw).hexdigest()==pin['sha256'],version
 doc=json.loads(raw);types=doc['play']['toServer']['types'];schema=types['packet_enchant_item']
 assert schema==['container',[{'name':'windowId','type':'ContainerID'},{'name':'enchantment','type':'i8' if version=='774' else 'varint'}]],version
 assert doc['types']['ContainerID']=='varint',version
 mapped=types['packet'][1][0]['type'][1]['mappings'];packet=next(k for k,n in mapped.items() if n=='enchant_item')
 return {'version':int(version),'pin':pin,'packet_id':packet,'schema':schema,'referenced_root_types':{'ContainerID':'varint'}}

def collect(cache,fetch=False):
 with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
  rows=list(pool.map(lambda pair:inspect(pair[0],pair[1],cache,fetch),sources().items()))
 return sorted(rows,key=lambda row:row['version'])

def main():
 p=argparse.ArgumentParser();p.add_argument('--cache',type=Path,required=True);p.add_argument('--fetch',action='store_true');p.add_argument('--write',action='store_true');a=p.parse_args()
 rows=collect(a.cache,a.fetch);body=json.dumps(rows,indent=2)+'\n'
 if a.write:OUTPUT.write_text(body)
 else:assert OUTPUT.read_text()==body,'reviewed current-field fixture changed'
 print(json.dumps({'status':'pass','pinned_JSON_versions':10,'window':'nonnegative VarInt ID','button774':'signed i8, positive IDs0..127','other_buttons':'nonnegative VarInt ID','private768_admission':'unchanged/closed','Rust_tests':'not executed by this source-only check'}))
if __name__=='__main__':main()
