#!/usr/bin/env python3
"""Точные имена, ID и графы схем пакетов776. Только локальные pinned JSON.

Равенство графов схем не доказывает совместимость контента/opaque NBT.
Публичные payload и admission этой таблицей не включаются.
"""
import argparse
import hashlib
import json
from pathlib import Path

from compare_schemas import ids, schema
from generate_registry_26_2 import CANDIDATE, Sources
from generate_static_776 import dense_count

VERSIONS = {775: 'V26_1_2', 776: 'V26_2', 777: 'V26_3'}
PAIRS = [(775, 776), (776, 775), (776, 777), (777, 776)]
KINDS = [(phase.title() + suffix, phase, direction)
         for phase in ['login', 'configuration', 'play']
         for direction, suffix in [('toClient', 'Clientbound'), ('toServer', 'Serverbound')]]


def make_route(source, target, source_schemas, target_schemas):
    dense_count(source)
    dense_count(target)
    rows = []
    for name, number in sorted(source.items(), key=lambda item: item[1]):
        if name not in source_schemas:
            raise ValueError('Нет схемы исходного пакета: ' + name)
        mapped = target.get(name)
        if mapped is not None and name not in target_schemas:
            raise ValueError('Нет схемы целевого пакета: ' + name)
        equal = mapped is not None and source_schemas[name] == target_schemas[name]
        rows.append((number, name, mapped, equal))
    return rows


def load(root, data777, cache776):
    manifest = json.loads((root / 'tools/protocol-bridge/candidate-sources-26.3.json').read_text())
    docs, sources = {}, {}
    for version, filename in [(775, 'protocol-26.1.json'), (777, 'protocol.json')]:
        entry = next(row for row in manifest['files'] + manifest['baseline_files'] if row['file'] == filename)
        raw = (data777 / filename).read_bytes()
        if len(raw) != entry['bytes'] or hashlib.sha256(raw).hexdigest() != entry['sha256']:
            raise ValueError('Изменился закреплённый JSON: ' + filename)
        docs[version] = json.loads(raw)
        sources[str(version)] = {'url': entry['url'], 'bytes': len(raw), 'sha256': entry['sha256']}
    locked = json.loads((root / 'tools/protocol-bridge/registry-candidate-26.2.json').read_text())['sources']
    middle = Sources(cache776, False, locked)
    docs[776] = middle.read(CANDIDATE, 'data/pc/26.2/protocol.json')
    entry = next(iter(middle.manifest.values()))
    sources['776'] = {key: entry[key] for key in ['url', 'bytes', 'sha256']}
    return docs, sources


def build(docs, sources):
    names, layouts = {}, {}
    for kind, phase, direction in KINDS:
        for version in VERSIONS:
            mapping = ids(docs[version], phase, direction)
            dense_count(mapping)
            names[kind, version] = mapping
            layouts[kind, version] = {name: schema(docs[version], phase, direction, name)[0] for name in mapping}
    text = '// Сгенерировано generate_packet_776.py из pinned JSON; packet-776-report.json.\n'
    text += '// Только имена/ID/равенство графа схем; не разрешение payload/admission.\n'
    text += 'use crate::{JavaVersion, ids::RegistryKind};\n\n'
    text += 'pub const MISSING: u16 = u16::MAX;\n\n'
    text += "pub fn names(kind: RegistryKind) -> Option<&'static [&'static str]> {\n    match kind {\n"
    for kind, _, _ in KINDS:
        text += f'        RegistryKind::{kind} => Some(&{kind.upper()}_NAMES),\n'
    text += '        _ => None,\n    }\n}\n\n'
    text += "pub fn route(kind: RegistryKind, from: JavaVersion, to: JavaVersion) -> Option<&'static [(u16, bool)]> {\n    match (kind, from, to) {\n"
    for kind, _, _ in KINDS:
        for source, target in PAIRS:
            text += f'        (RegistryKind::{kind}, JavaVersion::{VERSIONS[source]}, JavaVersion::{VERSIONS[target]}) => Some(&{kind.upper()}_{source}_{target}),\n'
    text += '        _ => None,\n    }\n}\n\n'
    records = []
    for kind, _, _ in KINDS:
        middle = names[kind, 776]
        text += f'static {kind.upper()}_NAMES: [&str; {len(middle)}] = [\n'
        for name in sorted(middle, key=middle.get):
            text += '    ' + json.dumps(name) + ',\n'
        text += '];\n\n'
        for source, target in PAIRS:
            rows = make_route(names[kind, source], names[kind, target], layouts[kind, source], layouts[kind, target])
            text += f'static {kind.upper()}_{source}_{target}: [(u16, bool); {len(rows)}] = [\n'
            for number, name, mapped, equal in rows:
                text += f'    ({mapped if mapped is not None else 65535}, {str(equal).lower()}), // {number}: {name}\n'
            text += '];\n\n'
            records.append({'registry': kind, 'from': source, 'to': target,
                            'packets': [{'source_id': number, 'name': name, 'target_id': mapped, 'schema_equal': equal,
                                         'source_schema_sha256': layouts[kind, source][name],
                                         'target_schema_sha256': layouts[kind, target].get(name)}
                                        for number, name, mapped, equal in rows]})
    text = text.rstrip('\n') + '\n'
    report = {'format': 1, 'sources': sources, 'scope': 'Private packet candidates only; existing775↔777 reused; payload/admission776 remain closed.',
              'equality': 'Exact full transitive schema graph SHA256 for SAME packet name in the requested pair; not observed semantic equivalence.',
              'source_presence': 'All source registries proven dense; reject holes, duplicate/negative/overflow IDs.',
              'sentinel': 65535, 'rust_source_sha256': hashlib.sha256(text.encode()).hexdigest(),
              'counts': {kind: {str(v): len(names[kind, v]) for v in VERSIONS} for kind, _, _ in KINDS}, 'routes': records}
    return text, json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + '\n'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    p.add_argument('--data777', type=Path, required=True)
    p.add_argument('--cache776', type=Path, required=True)
    p.add_argument('--output', type=Path)
    p.add_argument('--report', type=Path)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    bodies = build(*load(a.root, a.data777, a.cache776))
    targets = [a.output or a.root / 'tools/protocol-bridge/src/generated_packet_776.rs',
               a.report or a.root / 'tools/protocol-bridge/packet-776-report.json']
    for path, body in zip(targets, bodies):
        if a.check:
            if not path.exists() or path.read_text() != body:
                raise SystemExit('Не воспроизводится ' + str(path))
        else:
            path.write_text(body)
    report = json.loads(bodies[1])
    print(json.dumps({'routes': len(report['routes']), 'source_sha256': report['rust_source_sha256']}))


if __name__ == '__main__':
    main()
