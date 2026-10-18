#!/usr/bin/env python3
"""Наш перевод ID из разрешённых JSON; исходные реализации не используются.

Требует локальные candidate-json из pinned PR1301 (см. candidate-sources-26.3.json).
Имена и все свойства состояния должны совпасть точно. Нет неявных замен на air.
"""
import argparse
import hashlib
import itertools
import json
from compare_schemas import schema
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def named(entries):
    result = {}
    for entry in entries:
        key = entry['name']
        if key in result:
            raise ValueError('duplicate name: ' + key)
        result[key] = entry['id']
    return result


def states(blocks):
    result = {}
    for block in blocks:
        properties = []
        for prop in block['states']:
            values = prop.get('values')
            if values is None:
                if prop['type'] == 'bool':
                    values = ['true', 'false']
                elif prop['type'] == 'int':
                    values = [str(i) for i in range(prop['num_values'])]
                else:
                    raise ValueError('missing state values: ' + block['name'])
            assert len(values) == prop['num_values']
            properties.append([(prop['name'], str(value)) for value in values])
        start = block['minStateId']
        combinations = list(itertools.product(*properties))
        assert len(combinations) == block['maxStateId'] - start + 1
        for offset, combo in enumerate(combinations):
            key = (block['name'], tuple(sorted(combo)))
            assert key not in result
            result[key] = start + offset
    return result


def mapper(value):
    assert value[0] == 'mapper'
    return {name: int(key, 0) if key.startswith('0x') else int(key)
            for key, name in value[1]['mappings'].items()}


def packets(document, phase, direction):
    return mapper(document[phase][direction]['types']['packet'][1][0]['type'])


def particle(document):
    return mapper(document['types']['Particle'][1][0]['type'])


def attributes(document):
    fields = document['play']['toClient']['types']['packet_entity_update_attributes'][1]
    return mapper(fields[1]['type'][1]['type'][1][0]['type'])


def observed_attributes(path):
    # Кандидат JSON ошибочно оставил старые номера/неполный список777.
    # Обычный Tab команды /attribute, затем предмет с каждым предложенным
    # свойством: никакого чтения реализации, reports или дампов датапаков.
    observations=json.loads(path.read_text())
    result=[]
    for side,protocol,count in [('old',775,35),('new',777,40)]:
        sample=observations[side];assert sample['protocol']==protocol and sample['exit_code']==0
        values={}
        for entry in sample['cases']:
            data=iter(bytes.fromhex(entry['slot_hex']))
            def uint():
                value=0
                for shift in range(0,35,7):
                    byte=next(data);assert shift!=28 or byte<8
                    value|=(byte&127)<<shift
                    if byte<128:return value
                raise ValueError('oversized attribute VarInt')
            assert uint()==1;uint() # количество и ID stick
            assert [uint(),uint(),uint(),uint()]==[1,0,16,1]
            number=uint();name=entry['name'];assert name not in values;values[name]=number
        assert len(values)==count and set(values.values())==set(range(count))
        assert set(values)==set(sample['attribute_suggestions'])
        result.append(values)
    return result


def make_tables(source, target):
    for values in [source, target]:
        ids = list(values.values())
        assert len(ids) == len(set(ids)), 'duplicate ID'
        assert set(ids) == set(range(max(ids) + 1)), 'sparse registry needs explicit presence table'
    forward = [-1] * len(source)
    backward = [-1] * len(target)
    for name, old_id in source.items():
        if name in target:
            new_id = target[name]
            forward[old_id] = new_id
            backward[new_id] = old_id
    return forward, backward


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--data', type=Path, required=True)
    args = parser.parse_args()
    provenance = json.loads((args.data / 'sources.json').read_text())
    for entry in provenance['files'] + provenance['baseline_files']:
        content = (args.data / entry['file']).read_bytes()
        assert hashlib.sha256(content).hexdigest() == entry['sha256'], entry['file']
    def load(name):
        return json.loads((args.data / name).read_text())
    old = load('protocol-26.1.json')
    new = load('protocol.json')
    blocks_old = json.loads((ROOT / 'tools/data/blocks.json').read_text())
    blocks_new = load('blocks.json')
    attribute_source=ROOT/'tools/protocol-bridge/testdata/attribute-id-observations.json'
    attributes_old,attributes_new=observed_attributes(attribute_source)
    definitions = [
        ('Item', named(json.loads((ROOT / 'tools/data/items.json').read_text())), named(load('items.json'))),
        ('Block', named(blocks_old), named(blocks_new)),
        ('BlockState', states(blocks_old), states(blocks_new)),
        ('Entity', named(load('entities-26.1.json')), named(load('entities.json'))),
        ('Particle', particle(old), particle(new)),
        ('Attribute', attributes_old, attributes_new),
        ('Component', mapper(old['types']['SlotComponentType']), mapper(new['types']['SlotComponentType'])),
    ]
    for phase in ['login', 'configuration', 'play']:
        for direction, suffix in [('toClient', 'Clientbound'), ('toServer', 'Serverbound')]:
            definitions.append((phase.title() + suffix, packets(old, phase, direction), packets(new, phase, direction)))
    text = '// Сгенерировано generate_tables.py из разрешённых JSON, см. candidate-sources-26.3.json.\n'
    text += '// Только соответствие ID; одинаковый ID/имя не означает совместимость payload.\n\n'
    text += '// Attribute: собственные наблюдения обычных команд, testdata/attribute-id-observations.json.\n\n'
    text += '#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum RegistryKind {\n'
    text += ''.join(f'    {name},\n' for name, _, _ in definitions) + '}\n\n'
    text += '#[cfg(test)]\npub const ALL: &[RegistryKind] = &[\n'
    text += ''.join(f'    RegistryKind::{name},\n' for name, _, _ in definitions) + '];\n\n'
    text += 'pub fn tables(kind: RegistryKind) -> (&\'static [i32], &\'static [i32]) {\n    match kind {\n'
    text += ''.join(f'        RegistryKind::{name} => (&{name.upper()}_FORWARD, &{name.upper()}_BACKWARD),\n' for name, _, _ in definitions)
    text += '    }\n}\n\n'
    report = []
    for name, a, b in definitions:
        forward, backward = make_tables(a, b)
        for suffix, values in [('FORWARD', forward), ('BACKWARD', backward)]:
            text += f'static {name.upper()}_{suffix}: [i32; {len(values)}] = [\n'
            for start in range(0, len(values), 24):
                text += '    ' + ', '.join(map(str, values[start:start + 24])) + ',\n'
            text += '];\n\n'
        report.append({'registry': name, 'old_count': len(a), 'new_count': len(b),
                       'mapped': sum(x >= 0 for x in forward),
                       'renumbered': sum(x >= 0 and x != i for i, x in enumerate(forward)),
                       'old_unmapped': [str(k) for k, v in a.items() if forward[v] < 0],
                       'new_unmapped_count': sum(x < 0 for x in backward)})
    text += "pub fn packet_info(kind: RegistryKind, modern: bool, id: usize) -> Option<(&'static str, bool)> {\n"
    text += '    let entries: &[(&str, bool)] = match (kind, modern) {\n'
    for phase in ['login', 'configuration', 'play']:
        for direction, suffix in [('toClient', 'Clientbound'), ('toServer', 'Serverbound')]:
            name = phase.title() + suffix
            a, b = packets(old, phase, direction), packets(new, phase, direction)
            equal = {key for key in a.keys() & b.keys()
                     if schema(old, phase, direction, key)[0] == schema(new, phase, direction, key)[0]}
            for modern, mapping in [(False, a), (True, b)]:
                text += f'        (RegistryKind::{name}, {str(modern).lower()}) => &[\n'
                for key, _ in sorted(mapping.items(), key=lambda entry: entry[1]):
                    text += f'            ("{key}", {str(key in equal).lower()}),\n'
                text += '        ],\n'
    text += '        _ => return None,\n    };\n    entries.get(id).copied()\n}\n'
    (ROOT / 'tools/protocol-bridge/src/generated_ids.rs').write_text(text)
    inputs = ['protocol-26.1.json', 'entities-26.1.json']
    baseline = {name: hashlib.sha256((args.data / name).read_bytes()).hexdigest() for name in inputs}
    for name in ['blocks.json', 'items.json']:
        baseline['core/' + name] = hashlib.sha256((ROOT / 'tools/data' / name).read_bytes()).hexdigest()
    (ROOT / 'tools/protocol-bridge/registry-diff-26.3.json').write_text(json.dumps(
        {'candidate_commit': provenance['commit'], 'baseline_sha256': baseline,
         'attribute_observations_sha256':hashlib.sha256(attribute_source.read_bytes()).hexdigest(),
         'registries': report}, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
