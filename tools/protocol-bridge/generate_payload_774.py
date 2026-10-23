#!/usr/bin/env python3
"""Раскладки774 из закреплённых JSON; без сети, реализации и admission."""
from pathlib import Path
import json,hashlib
import argparse
root=Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cache',type=Path,required=True);p.add_argument('--baseline',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args()
from generate_registry_26_2 import mapper
from generate_static_776 import direct_mapping
locks=json.loads((root/'tools/protocol-bridge/candidate-sources-26.3.json').read_text())
for filename in ['protocol-26.1.json','entities-26.1.json']:
 record=next(r for r in locks['files']+locks['baseline_files'] if r['file']==filename)
 raw=(args.baseline/filename).read_bytes();assert len(raw)==record['bytes'] and hashlib.sha256(raw).hexdigest()==record['sha256']
old=json.loads((args.cache/'protocol.json').read_text());new=json.loads((args.baseline/'protocol-26.1.json').read_text())
# Только закреплённые JSON, без реализации.
for k,v in json.loads((root/'tools/protocol-bridge/step1-sources-774.json').read_text()).items():assert hashlib.sha256((args.cache/f'{k}.json').read_bytes()).hexdigest()==v['sha256']
t='// Закреплённые JSON774: step1-sources-774.json; индексы не являются live proof.\n'
a=mapper(old['types']['entityMetadataEntry'][1][1]['type']);b=mapper(new['types']['entityMetadataEntry'][1][1]['type'])
for name,src,dst in [('SERIALIZERS_TO',b,a),('SERIALIZERS_FROM',a,b)]:t+=f'pub(crate) const {name}: &[u16] = &{list(direct_mapping(src,dst))};\n'
# JSON774 ошибочно потерял четыре атрибута, добавленные ещё в1.21:
# minecraft.wiki/w/Attribute. Сохраняем наблюдённый775 список35, а не31.
# [проверить] Все35 значений774 отдельно не наблюдались; live payload проверяет attack/speed.
observed=json.loads((root/'tools/protocol-bridge/testdata/attribute-id-observations.json').read_text())
assert observed['old']['protocol']==775 and len(observed['old']['cases'])==35
for name in ['ATTRIBUTES_TO','ATTRIBUTES_FROM']:t+=f'pub(crate) const {name}: &[u16] = &{list(range(35))};\n'
entities=json.loads((args.cache/'entities.json').read_text());base=json.loads((args.baseline/'entities-26.1.json').read_text());lookup={v['name']:v for v in entities}
t+='pub(crate) const LAYOUTS: &[(u32, &[&str])] = &[\n'
for r in base:
 if r['name'] in lookup:t+=f'    ({r["id"]}, &{json.dumps(lookup[r["name"]]["metadataKeys"])}),\n'
t+='];\npub(crate) const ITEM_NAMES: &[&str] = &[\n'+''.join('    '+json.dumps(r['name'])+',\n' for r in json.loads((args.cache/'items.json').read_text()))+'];\n'
target=root/'tools/protocol-bridge/src/generated_payload_774.rs'
if args.check: assert target.read_text()==t
else: target.write_text(t)
print(json.dumps({'sha256':hashlib.sha256(t.encode()).hexdigest()}))
