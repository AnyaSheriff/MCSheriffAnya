#!/usr/bin/env python3
"""Private776 payload: закреплённые JSON, без сети и изменения admission."""
import argparse
import hashlib
import json
from pathlib import Path
from compare_schemas import strings
from generate_registry_26_2 import CANDIDATE, Sources

ROOT = Path(__file__).resolve().parents[2]
COMPONENTS = (0, 1, 2, 3, 4, 6, 9, 11, 12, 13, 16, 18, 19, 20, 21, 22,
              23, 40, 42, 44, 45, 46, 48, 51, 52, 54, 55, 63, 73, 84)
# Эти ссылки имеют отдельный typed remapper; равенство числовых ID не требуется.
BOUNDARIES = {'SlotComponentType', 'ItemStackTemplate'}


def graph(types, root):
    pending, references = [root], {}
    while pending:
        for ref in strings(pending.pop()):
            if ref in types and ref not in references:
                references[ref] = 'typed remapper' if ref in BOUNDARIES else types[ref]
                if ref not in BOUNDARIES:
                    pending.append(types[ref])
    return {'root': root, 'references': references}


def generate(baseline, cache):
    locks = json.loads((ROOT / 'tools/protocol-bridge/candidate-sources-26.3.json').read_text())
    sources = {}
    def old(filename):
        record = next(r for r in locks['files'] + locks['baseline_files'] if r['file'] == filename)
        raw = (baseline / filename).read_bytes()
        assert len(raw) == record['bytes'] and hashlib.sha256(raw).hexdigest() == record['sha256']
        sources[record['url']] = {'sha256': record['sha256'], 'bytes': len(raw)}
        return json.loads(raw)
    pinned = Sources(cache, False, json.loads((ROOT / 'tools/protocol-bridge/registry-candidate-26.2.json').read_text())['sources'])
    before = old('protocol-26.1.json')['types']
    after = pinned.read(CANDIDATE, 'data/pc/26.2/protocol.json')['types']
    old_entities = old('entities-26.1.json')
    entities = pinned.read(CANDIDATE, 'data/pc/26.2/entities.json')
    for record in pinned.manifest.values():
        sources[record['url']] = {k: record[k] for k in ('sha256', 'bytes')}
    # Дискриминатор metadata и envelope Creative Slot совпадают буквально.
    for key in ('entityMetadataEntry', 'UntrustedSlot', 'UntrustedSlotComponent'):
        assert before[key] == after[key], key
    names = before['SlotComponentType'][1]['mappings']
    fields = [v['SlotComponent'][1][1]['type'][1]['fields'] for v in (before, after)]
    components = []
    for kind in COMPONENTS:
        name = names[str(kind)]
        a, b = [graph(types, f[name]) for types, f in zip((before, after), fields)]
        assert a == b, name
        components.append({'kind': kind, 'name': name,
                           'schema_sha256': hashlib.sha256(json.dumps(a, sort_keys=True).encode()).hexdigest()})
    target = {v['name']: v for v in entities}
    layouts = []
    for row in old_entities:
        assert row['id'] == len(layouts)
        keys = target[row['name']]['metadataKeys']
        assert len(keys) < 255 and len(keys) == len(set(keys))
        layouts.append((row['id'], row['name'], keys))
    # sulfur_cube существует в776; его canonical index добавлен после cushion.
    keys = target['sulfur_cube']['metadataKeys']
    layouts.append((len(old_entities) + 1, 'sulfur_cube', keys))
    # Только общие частицы: новые26.2 не становятся известными775 автоматически.
    particles = [v['Particle'][1][1]['type'][1]['fields'] for v in (before, after)]
    for name in before['Particle'][1][0]['type'][1]['mappings'].values():
        a, b = [graph(types, f.get(name, 'void')) for types, f in zip((before, after), particles)]
        assert a == b, name
    text = '// Сгенерировано generate_payload_776.py; payload-776-report.json.\n'
    text += '// Индексы по JSON; schema-based расширение не является live776 доказательством.\n'
    text += 'pub(crate) const COMPONENTS: &[u32] = &[' + ', '.join(map(str, COMPONENTS)) + '];\n'
    text += "pub(crate) const LAYOUTS: &[(u32, &[&str])] = &[\n"
    for kind, _, keys in layouts:
        text += f'    ({kind}, &{json.dumps(keys)}),\n'
    text += '];\n'
    report = {'sources': sources, 'components': components,
              'layouts': [{'kind': k, 'name': n, 'fields': f} for k, n, f in layouts],
              'scope': 'Pinned schemas + existing typed775 codecs; synthetic coverage, not new live776 evidence. Unsupported component data/serializers remain rejected; admission unchanged.',
              'rust_sha256': hashlib.sha256(text.encode()).hexdigest()}
    return text, json.dumps(report, ensure_ascii=False, indent=2) + '\n'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline', type=Path, required=True)
    p.add_argument('--cache', type=Path, required=True)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    source, report = generate(a.baseline, a.cache)
    for path, data in [('src/generated_payload_776.rs', source), ('payload-776-report.json', report)]:
        path = ROOT / 'tools/protocol-bridge' / path
        if a.check:
            assert path.read_text() == data, path
        else:
            path.write_text(data)
    print(json.dumps({'components': len(COMPONENTS), 'layouts': len(json.loads(report)['layouts']), 'check': a.check}))


if __name__ == '__main__':
    main()
