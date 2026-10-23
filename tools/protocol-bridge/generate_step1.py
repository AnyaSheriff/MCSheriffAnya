#!/usr/bin/env python3
"""Закреплённые таблицы774↔775; никакого допуска или чужой реализации.

Читаются только JSON; --check воспроизводит Rust без изменения файлов.
"""
import argparse
import hashlib
import json
from pathlib import Path
from compare_schemas import ids, schema
from generate_registry_26_2 import named, block_states, mapper
from generate_static_776 import direct_mapping
from generate_registry_26_2 import LOCAL_LOCKS

ROOT = Path(__file__).resolve().parents[2]
KINDS = [('Login', 'login'), ('Configuration', 'configuration'), ('Play', 'play')]


def generate(cache, baseline):
    manifest = json.loads((ROOT/'tools/protocol-bridge/step1-sources-774.json').read_text())
    docs = {}
    for kind, record in manifest.items():
        raw = (cache/(kind+'.json')).read_bytes()
        assert len(raw) == record['bytes'] and hashlib.sha256(raw).hexdigest() == record['sha256']
        docs[kind] = json.loads(raw)
    locks = json.loads((ROOT/'tools/protocol-bridge/candidate-sources-26.3.json').read_text())
    def old(filename):
        record = next(r for r in locks['files'] + locks['baseline_files'] if r['file'] == filename)
        raw = (baseline/filename).read_bytes()
        assert len(raw) == record['bytes'] and hashlib.sha256(raw).hexdigest() == record['sha256']
        return json.loads(raw)
    for filename, expected in LOCAL_LOCKS.items():
        assert hashlib.sha256((ROOT/filename).read_bytes()).hexdigest() == expected
    protocol = old('protocol-26.1.json')
    data = {
        'Item': (named(docs['items']), named(json.loads((ROOT/'tools/data/items.json').read_text()))),
        'Block': (named(docs['blocks']), named(json.loads((ROOT/'tools/data/blocks.json').read_text()))),
        'BlockState': (block_states(docs['blocks']), block_states(json.loads((ROOT/'tools/data/blocks.json').read_text()))),
        'Entity': (named(docs['entities']), named(old('entities-26.1.json'))),
        'Component': (mapper(docs['protocol']['types']['SlotComponentType']), mapper(protocol['types']['SlotComponentType'])),
        'Particle': (mapper(docs['protocol']['types']['Particle'][1][0]['type']), mapper(protocol['types']['Particle'][1][0]['type'])),
    }
    text = '// Сгенерировано generate_step1.py из закреплённых JSON; step1-sources-774.json.\n'
    text += '// Диапазоны содержат точные соответствия; 65535 означает отсутствие цели.\n'
    text += "use crate::ids::RegistryKind;\n\ntype Ranges = (u32, &'static [(u32,u32,i32)]);\n\n"
    text += 'pub(crate) fn ranges(kind: RegistryKind, old: bool) -> Option<Ranges> {\n    match (kind,old) {\n'
    arrays = ''
    for kind, (before, after) in data.items():
        for is_old, source, target in [(True,before,after),(False,after,before)]:
            values = direct_mapping(source,target)
            symbol = f'{kind.upper()}_{774 if is_old else 775}'
            runs = []
            for i, mapped in enumerate(values):
                offset = -1 if mapped == 65535 else mapped-i
                # Смещение может быть отрицательным; sentinel отделён границей65535.
                encoded = 65535 if mapped == 65535 else offset
                if runs and runs[-1][2] == encoded:
                    runs[-1] = (runs[-1][0],i,encoded)
                else:
                    runs.append((i,i,encoded))
            arrays += f'static {symbol}: &[(u32,u32,i32)] = &[\n'
            arrays += ''.join(f'    ({a},{b},{c}),\n' for a,b,c in runs) + '];\n'
            text += f'        (RegistryKind::{kind},{str(is_old).lower()}) => Some(({len(values)},{symbol})),\n'
    text += '        _ => None,\n    }\n}\n\n' + arrays
    text += '\npub(crate) fn packets(kind: RegistryKind, old: bool) -> Option<&\'static [(&\'static str,u16,bool)]> {\n    match (kind,old) {\n'
    arrays = ''
    for title,phase in KINDS:
        for suffix,direction in [('Clientbound','toClient'),('Serverbound','toServer')]:
            kind = title + suffix
            for is_old, source, target in [(True,docs['protocol'],protocol),(False,protocol,docs['protocol'])]:
                source_ids, target_ids = ids(source,phase,direction),ids(target,phase,direction)
                assert sorted(source_ids.values()) == list(range(len(source_ids)))
                symbol = f'{kind.upper()}_{774 if is_old else 775}'
                arrays += f'static {symbol}: &[(&str,u16,bool)] = &[\n'
                for name, number in sorted(source_ids.items(),key=lambda r:r[1]):
                    target_id = target_ids.get(name,65535)
                    equal = target_id != 65535 and schema(source,phase,direction,name)[0] == schema(target,phase,direction,name)[0]
                    arrays += f'    ({json.dumps(name)},{target_id},{str(equal).lower()}),\n'
                arrays += '];\n'
                text += f'        (RegistryKind::{kind},{str(is_old).lower()}) => Some({symbol}),\n'
    text += '        _ => None,\n    }\n}\n\n' + arrays
    policy = json.loads((ROOT/'src/gameplay_catalog/client-774-views.json').read_text())
    views = policy['entries']
    text += '\npub(crate) fn substitute(kind: RegistryKind, id: u32) -> Option<u32> {\n    match (kind,id) {\n'
    before_blocks = {r['name']:r for r in json.loads((ROOT/'tools/data/blocks.json').read_text())}
    after_blocks = {r['name']:r for r in docs['blocks']}
    for row in views:
        name, target = row['name'], row['substitute']
        for kind in ['Item','Block']:
            after, before = data[kind]
            if name not in before:
                continue
            assert name not in after and target in after
            text += f'        (RegistryKind::{kind},{before[name]}) => Some({after[target]}),\n'
        before, after = before_blocks[name], after_blocks[target]
        assert before['states'] == after['states'] == []
        text += f"        (RegistryKind::BlockState,{before['minStateId']}) => Some({after['minStateId']}),\n"
    after_states, before_states = data['BlockState']
    for row in policy['state_substitutes']:
        found = set()
        for (name, props), number in before_states.items():
            properties = dict(props)
            if name != row['name'] or properties.get(row['property']) not in row['values']:
                continue
            found.add(properties[row['property']])
            assert (name, props) not in after_states
            properties[row['property']] = row['substitute']
            target = after_states[(name,tuple(sorted(properties.items())))]
            text += f'        (RegistryKind::BlockState,{number}) => Some({target}),\n'
        assert found == set(row['values'])
    return text + '        _ => None,\n    }\n}\n' 



def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--cache',type=Path,required=True)
    p.add_argument('--baseline',type=Path,required=True)
    p.add_argument('--check',action='store_true')
    args = p.parse_args()
    body = generate(args.cache,args.baseline)
    target = ROOT/'tools/protocol-bridge/src/generated_step1.rs'
    if args.check:
        assert target.read_text() == body
    else:
        target.write_text(body)
    print(json.dumps({'bytes':len(body.encode()),'sha256':hashlib.sha256(body.encode()).hexdigest()}))


if __name__ == '__main__':
    main()
