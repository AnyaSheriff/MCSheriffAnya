#!/usr/bin/env python3
"""Четыре bounded metadata layouts776 из pinned JSON и собственных ordinary packets."""
import argparse
import hashlib
import json
from pathlib import Path

LOCKS = {
    'old_entities': ('58d24389309b26db59454cf8b7813ad688beeb55/data/pc/26.1/entities.json',
                     'e65d88a15fac5fc090cb2555c8cddaa80d9daf8c554d252680cec60a1d6eefc3'),
    'entities': ('924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/entities.json',
                 '6ea2d04f7f78ea56fa3db8453dd62c4b6021b5eef01994e24e3e3e5f6c28295d'),
    'protocol': ('924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/protocol.json',
                 'a6c9bd7240f74b94eb0d9b1497294e4cfaa4bb02c0023817c952429149e2d50c'),
}
OBSERVATION_SHA = 'ba43741cda1d2df28bebc4271a27f3cd908b355116e5b8f41cb5afe1e9e6c38b'
NAMES = ('cow', 'slime', 'magma_cube', 'player')
# Типы общих полей переиспользованы из нашего775 encoder и ordinary776.
# Player776 не наблюдался отдельно: layout/schema + synthetic tests, не GUI proof.
SERIALIZERS = {
    'shared_flags': 0, 'air_supply': 1, 'custom_name': 6, 'custom_name_visible': 8,
    'silent': 8, 'no_gravity': 8, 'pose': 20, 'ticks_frozen': 1,
    'living_entity_flags': 0, 'health': 3, 'effect_particles': 17,
    'effect_ambience': 8, 'arrow_count': 1, 'stinger_count': 1, 'sleeping_pos': 11,
    'mob_flags': 0, 'baby': 8, 'age_locked': 8, 'size': 1,
    'variant': 23, 'sound_variant': 24, 'player_main_hand': 42,
    'player_mode_customisation': 0, 'player_absorption': 3, 'score': 1,
    'shoulder_parrot_left': 19, 'shoulder_parrot_right': 19,
}
SERIALIZER_NAMES = {
    0: 'byte', 1: 'int', 3: 'float', 6: 'optional_component', 8: 'boolean',
    11: 'optional_block_pos', 17: 'particles', 19: 'optional_unsigned_int',
    20: 'pose', 23: 'cow_variant', 24: 'cow_sound_variant', 42: 'humanoid_arm',
}


def generate(cache, observed):
    tables, sources = {}, {}
    for name, (relative, expected) in LOCKS.items():
        data = (cache / relative).read_bytes()
        actual = hashlib.sha256(data).hexdigest()
        assert actual == expected, (relative, actual)
        tables[name] = json.loads(data)
        sources[name] = {'file': relative, 'sha256': actual, 'bytes': len(data)}
    raw = observed.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == OBSERVATION_SHA
    observation = json.loads(raw)
    assert observation['version'] == 776 and observation['exit_code'] == 0
    sources['observation'] = {'file': 'verification-protocol776-ordinary-entities-02/entities/result.json',
                              'sha256': OBSERVATION_SHA, 'bytes': len(raw)}
    mapper = tables['protocol']['types']['entityMetadataEntry'][1][1]['type'][1]['mappings']
    assert all(mapper[str(number)] == name for number, name in SERIALIZER_NAMES.items())
    by_name = [{row['name']: row for row in tables[key]} for key in ('old_entities', 'entities')]
    layouts = []
    for name in NAMES:
        old, new = (table[name] for table in by_name)
        fields = new['metadataKeys']
        assert len(fields) < 255 and len(set(fields)) == len(fields)
        assert all(field in SERIALIZERS for field in fields)
        layouts.append({'name': name, 'native_id': old['id'], 'protocol776_id': new['id'],
                        'fields': fields, 'serializers': [SERIALIZERS[field] for field in fields],
                        'old_fields': old['metadataKeys'],
                        'evidence': 'pinned layout + own metadata packets' if name != 'player'
                                    else 'pinned layout + own775 encoder; synthetic test only'})
    layouts.sort(key=lambda row: row['native_id'])
    # Наблюдавшиеся поля сверяются с данными, а не с заявлением о metadataKeys.
    observed_entities = []
    for entity in observation['entities']:
        layout = next(row for row in layouts if row['name'] == entity['type'])
        for index, field in entity['metadata_fields'].items():
            assert field['serializer'] == layout['serializers'][int(index)]
        observed_entities.append({key: entity[key] for key in
                                  ('type', 'id', 'metadata_fields')} |
                                 {'packets': [packet for packet in entity['packets']
                                              if packet['name'] == 'entity_metadata']})
    assert {entity['type'] for entity in observed_entities} == {'cow', 'slime', 'magma_cube'}
    source = '// Сгенерировано generate_metadata_776.py: закреплённые JSON + собственные ordinary776.\n'
    source += "pub(crate) type Layout = (u32, &'static str, &'static [&'static str]);\n"
    source += 'pub(crate) const LAYOUTS: &[Layout] = &[\n'
    for row in layouts:
        source += f'    ({row["native_id"]}, {json.dumps(row["name"])}, &{json.dumps(row["fields"])}),\n'
    source += '];\n'
    source += "pub(crate) const SERIALIZERS: &[(&str, u32)] = &[\n"
    for field, number in sorted(SERIALIZERS.items()):
        source += f'    ({json.dumps(field)}, {number}),\n'
    source += '];\n'
    report = {'protocol': 776, 'sources': sources, 'layouts': layouts,
              'observed_entities': observed_entities,
              'limits': ['player is schema-based only, not new official metadata observation',
                         'nonempty particles/complex unknown serializers refused for776',
                         'public wire/Tracker/admission remains closed',
                         'fields absent from775 fail MissingField, never silently dropped'],
              'generated_sha256': hashlib.sha256(source.encode()).hexdigest()}
    return source, json.dumps(report, ensure_ascii=False, indent=2) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--observed', type=Path, required=True)
    parser.add_argument('--output-root', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    source, report = generate(args.cache, args.observed)
    for relative, data in [('src/entity_layouts_776.rs', source), ('metadata-776-report.json', report)]:
        target = args.output_root / relative
        if args.check:
            assert target.read_text() == data, relative
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(data)
    print(json.dumps({'layouts': 4, 'observed_entities': 3, 'source_bytes': len(source.encode()),
                      'check': args.check}))


if __name__ == '__main__':
    main()
