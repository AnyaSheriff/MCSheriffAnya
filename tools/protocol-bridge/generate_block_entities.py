#!/usr/bin/env python3
"""Таблица только из наших записей поставленных блоков и разрешённых blocks.json."""
import hashlib
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent

def main():
    old=json.loads((HERE/'testdata/block-entities-26.1.2.json').read_text())
    new=json.loads((HERE/'testdata/block-entities-26.3.json').read_text())
    assert old['jar_sha1']=='97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51'
    assert new['jar_sha1']=='33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'
    assert old['exit_code']==new['exit_code']==0
    assert new['bed_block_present'], 'отсутствие BE проверяется только при сохранённом блоке кровати'
    a={e['block']:e for e in old['entities']};b={e['block']:e for e in new['entities']}
    assert len(a)==len(old['entities']) and len(b)==len(new['entities'])
    assert set(a)-set(b)=={'white_bed'} and not set(b)-set(a)
    for data in [a,b]: assert len({v['kind'] for v in data.values()})==len(data)
    rows=[{'block':name,'old':v['kind'],'new':b[name]['kind'] if name in b else None} for name,v in sorted(a.items(),key=lambda nv:nv[1]['kind'])]
    (HERE/'block-entity-observations.json').write_text(json.dumps(rows,indent=2)+'\n')
    path=ROOT/'tools/data/blocks.json';blocks=json.loads(path.read_text())
    ranges=sorted((v['minStateId'],v['maxStateId']) for v in blocks if v['name'].endswith('_bed'))
    assert len(ranges)==16
    merged=[]
    for lo,hi in ranges:
        if merged and merged[-1][1]+1==lo: merged[-1]=(merged[-1][0],hi)
        else: merged.append((lo,hi))
    text='// Собственные наблюдения probe_block_entities.py; не таблица из дампа реестров.\n'
    text+='// Сгенерировано generate_block_entities.py. Не наблюдавшиеся ID запрещены.\n'
    text+='pub const OBSERVED: &[(u32, Option<u32>, &str)] = &[\n'
    for r in rows:
        text+=f'    ({r["old"]}, '+('None' if r['new'] is None else f'Some({r["new"]})')+f', "{r["block"]}"),\n'
    text+='];\n\n// tools/data/blocks.json SHA256 '+hashlib.sha256(path.read_bytes()).hexdigest()+'\n'
    text+='pub fn legacy_bed(state: u32) -> bool {\n    matches!(state, '+' | '.join(f'{lo}..={hi}' for lo,hi in merged)+')\n}\n'
    (HERE/'src/block_entity_ids.rs').write_text(text)

if __name__=='__main__':main()
