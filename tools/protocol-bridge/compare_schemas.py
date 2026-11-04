#!/usr/bin/env python3
"""Сравнить схемы JSON вместе с транзитивными типами; не читать реализации."""
import argparse
import hashlib
import json
from pathlib import Path


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)


def schema(document, phase, direction, name):
    local = document[phase][direction]['types']
    types = document['types'] | local
    fields = local['packet'][1][1]['type'][1]['fields']
    root = fields[name]
    # Граф сохраняет полные определения всех достижимых типов, включая циклы.
    # Лишняя найденная ссылка может дать только консервативное несовпадение.
    pending = [root]
    references = {}
    while pending:
        value = pending.pop()
        for ref in strings(value):
            if ref in types and ref not in references:
                references[ref] = types[ref]
                pending.append(types[ref])
    value = {'root': root, 'references': references}
    canonical = json.dumps(value, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(canonical.encode()).hexdigest(), sorted(
        name for name, definition in references.items() if definition == 'native')


def ids(document, phase, direction):
    mapping = document[phase][direction]['types']['packet'][1][0]['type'][1]['mappings']
    return {name: int(key, 16) for key, name in mapping.items()}


def compare(old, new):
    records = []
    for phase in ['login', 'configuration', 'play']:
        for direction in ['toClient', 'toServer']:
            a, b = ids(old, phase, direction), ids(new, phase, direction)
            for name in sorted(a.keys() | b.keys()):
                before = schema(old, phase, direction, name) if name in a else None
                after = schema(new, phase, direction, name) if name in b else None
                records.append({'phase': phase, 'direction': direction, 'name': name,
                                'old_id': a.get(name), 'new_id': b.get(name),
                                'old_schema_sha256': before[0] if before else None,
                                'new_schema_sha256': after[0] if after else None,
                                'schema_equal': before is not None and after is not None and before[0] == after[0],
                                'new_native_dependencies': sorted(set(after[1] if after else []) - set(before[1] if before else []))})
    return records


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--data', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    before = json.loads((args.data / 'protocol-26.1.json').read_text())
    after = json.loads((args.data / 'protocol.json').read_text())
    records = compare(before, after)
    old_native = {k for k, v in before['types'].items() if v == 'native'}
    new_native = {k for k, v in after['types'].items() if v == 'native'}
    report = {'new_native_types': sorted(new_native - old_native), 'scope': 'Schema comparison only. Equal layouts still require registry/content ID translation.',
              'packets': records}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'packets': len(records), 'schema_equal': sum(r['schema_equal'] for r in records),
                      'new_native_types': report['new_native_types']}))


if __name__ == '__main__':
    main()
