#!/usr/bin/env python3
"""Только разрешённые JSON: индексы metadata и список нового контента."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--data', type=Path, required=True)
    args = parser.parse_args()
    sources = json.loads((args.data/'sources.json').read_text())
    for entry in sources['files'] + sources['baseline_files']:
        assert hashlib.sha256((args.data/entry['file']).read_bytes()).hexdigest() == entry['sha256']
    def load(name): return json.loads((args.data/name).read_text())
    old = load('entities-26.1.json'); new = load('entities.json')
    by_name = {v['name']:v for v in new}
    text = '// Сгенерировано generate_entity_layouts.py; только pinned minecraft-data JSON.\n'
    text += "pub(crate) type Layout = (&'static str, &'static [&'static str], &'static [&'static str]);\n"
    text += 'pub(crate) const LAYOUTS: &[Layout] = &[\n'
    for i,v in enumerate(old):
        assert v['id']==i
        keys = [v['metadataKeys'],by_name[v['name']]['metadataKeys']]
        assert all(len(k)==len(set(k)) and len(k)<255 for k in keys)
        text += '    ('+json.dumps(v['name'])+', &'+json.dumps(keys[0])+', &'+json.dumps(keys[1])+'),\n'
    extra = by_name['cushion']['metadataKeys']  # 26.3: сущность подушки (нет в 775)
    text += '    // 26.3: сущность подушки (нет в 775); добавляется генератором после обычных строк.\n'
    text += '    ("cushion", &'+json.dumps(extra[:8])+', &'+json.dumps(extra)+'),\n'
    text += '];\n'
    for name in ['player','falling_block']:
        text += 'pub(crate) const '+name.upper()+': u32 = '+str(next(v['id'] for v in old if v['name']==name))+';\n'
    (ROOT/'tools/protocol-bridge/src/entity_layouts.rs').write_text(text)
    report = {'source_commit':sources['commit'], 'native':'26.1.2', 'target':'26.3', 'implemented_gameplay':False, 'new':{}}
    for key,file in [('blocks','blocks.json'),('items','items.json'),('entities','entities.json')]:
        baseline = old if key=='entities' else json.loads((ROOT/'tools/data'/file).read_text())
        names = {v['name'] for v in baseline}
        report['new'][key] = [{'name':v['name'],'target_id':v['id']} for v in load(file) if v['name'] not in names]
    report['changed_metadata'] = [v['name'] for v in old if v['metadataKeys'] != by_name[v['name']]['metadataKeys']]
    (ROOT/'tools/protocol-bridge/content-gap-26.3.json').write_text(json.dumps(report,indent=2)+'\n')
    print({k:len(v) for k,v in report['new'].items()})

if __name__ == '__main__': main()
