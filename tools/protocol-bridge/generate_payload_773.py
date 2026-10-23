#!/usr/bin/env python3
"""Типизированная основа773: только закреплённые JSON, без сети и допуска."""
import argparse
import hashlib
import json
from pathlib import Path
from generate_client_chain import HERE, load, locked
from generate_registry_26_2 import mapper
from generate_static_776 import direct_mapping


def generate(docs, baseline):
    old, new = docs[773], docs[774]
    # Проверяем декларации общих компонент и envelopes; вложенные данные всё
    # равно разбираются typed codec, а не копируются по равенству графов.
    a, b = old['protocol']['types'], new['protocol']['types']
    shared = a['SlotComponent'][1][1]['type'][1]['fields']
    target = b['SlotComponent'][1][1]['type'][1]['fields']
    assert all(target.get(k) == v for k, v in shared.items())
    envelopes = ['Slot', 'UntrustedSlot', 'UntrustedSlotComponent']
    assert all(a[k] == b[k] for k in envelopes)
    serializers = mapper(a['entityMetadataEntry'][1][1]['type'])
    canonical = mapper(baseline['protocol']['types']['entityMetadataEntry'][1][1]['type'])
    text = '// Закреплённые JSON773: client-chain-sources.json; это не живое доказательство.\n'
    for name, source, destination in [('SERIALIZERS_FROM', serializers, canonical), ('SERIALIZERS_TO', canonical, serializers)]:
        text += f'pub(crate) const {name}: &[u16] = &{list(direct_mapping(source, destination))};\n'
    lookup = {r['name']: r for r in old['entities']}
    text += 'pub(crate) const LAYOUTS: &[(u32, &[&str])] = &[\n'
    for row in baseline['entities']:
        if row['name'] in lookup:
            text += f'    ({row["id"]}, &{json.dumps(lookup[row["name"]]["metadataKeys"])}),\n'
    text += '];\n'
    changes = {}
    for row in new['entities']:
        if row['name'] in lookup and row['metadataKeys'] != lookup[row['name']]['metadataKeys']:
            changes[row['name']] = dict(older=lookup[row['name']]['metadataKeys'], newer=row['metadataKeys'])
    report = dict(scope='Typed adjacent773/774 foundation; public admission CLOSED; no live checklist claimed',
                  common_component_definitions=len(shared), equal_envelopes=envelopes,
                  missing_components=sorted(target.keys() - shared.keys()),
                  missing_entities=sorted({r['name'] for r in new['entities']} - lookup.keys()),
                  changed_metadata=changes,
                  semantic_blockers=['remaining_anger_time/anger_end_time: refuse missing fields; semantics unverified',
                                     'Attribute IDs below774 remain closed pending correction of pinned JSON omission',
                                     'Bee NBT entity prefix below774 unobserved; component77 refused'])
    return text, json.dumps(report, indent=2) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--cache774', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    docs = load(args.cache, args.cache774)
    locks = json.loads((HERE/'candidate-sources-26.3.json').read_text())
    baseline = {}
    for name in ['protocol', 'entities']:
        filename = name + '-26.1.json'
        record = next(r for r in locks['files'] + locks['baseline_files'] if r['file'] == filename)
        baseline[name] = locked(args.baseline, filename, record)
    text, report = generate(docs, baseline)
    for path, content in [(HERE/'src/generated_payload_773.rs', text), (HERE/'payload-773-report.json', report)]:
        if args.check:
            assert path.read_text() == content, path
        else:
            path.write_text(content)
    print(json.dumps(dict(common_components=96, sha256=hashlib.sha256(text.encode()).hexdigest())))


if __name__ == '__main__':
    main()
