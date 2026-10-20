#!/usr/bin/env python3
"""Отбирает собственные ordinary776 payloads; не запускает сервер и не скачивает данные."""
import argparse
import hashlib
import json
from pathlib import Path


SOURCES = {
    'light': ('verification-protocol776-ordinary-light-02/light/result.json',
              'd42b0e8fa613fdb6759057748b0292c0c7772e835a520ce7c54232b0f47c4746'),
    'entities': ('verification-protocol776-ordinary-entities-02/entities/result.json',
                 'ba43741cda1d2df28bebc4271a27f3cd908b355116e5b8f41cb5afe1e9e6c38b'),
}


def extract(source_root):
    reports, sources = {}, {}
    for name, (relative, expected) in SOURCES.items():
        data = (source_root / relative).read_bytes()
        actual = hashlib.sha256(data).hexdigest()
        assert actual == expected, (relative, actual, expected)
        record = json.loads(data)
        assert record['exit_code'] == 0 and record.get('protocol', record.get('version')) == 776
        reports[name] = record
        sources[name] = {'file': relative, 'sha256': actual, 'bytes': len(data)}
    light = reports['light']
    entities = reports['entities']
    cases = []
    for entry in light['cases']:
        payload = bytes.fromhex(entry['light_hex'])
        assert entry['case'] in ('placed', 'removed')
        assert entry['torch_cell_level'] == (14 if entry['case'] == 'placed' else 0)
        cases.append({key: entry[key] for key in
                      ('case', 'chunk', 'masks', 'torch_cell_level', 'light_hex')} |
                     {'payload_sha256': hashlib.sha256(payload).hexdigest()})
    assert len(cases) == 2
    sections = []
    for entry in light['authored_sections']:
        assert entry['width'] == 15 and entry['blocks_present'] == 4096
        assert entry['fluids_present'] == 0 and entry['unique_states'] == 300
        # Отбираем только заголовок и палитру авторских блоков. Сырой suffix
        # биомов остаётся в исходной записи обычного пакета, но не в fixture.
        section = bytes.fromhex(entry['section_hex'])
        blocks = section[:4 + 1 + 1024 * 8]
        assert blocks[:5] == b'\x10\x00\x00\x00\x0f' and len(blocks) == 8197
        assert len(section) > len(blocks) and len(entry['expected_prefix']) == 299
        sections.append({key: entry[key] for key in
                         ('case', 'section_y', 'width', 'blocks_present', 'fluids_present',
                          'unique_states', 'expected_prefix', 'remaining_state')} |
                        {'blocks_hex': blocks.hex(),
                         'blocks_sha256': hashlib.sha256(blocks).hexdigest(),
                         'biomes': 'omitted; tests append an explicitly synthetic single biome0'})
    assert len(sections) == 2 and sections[0]['blocks_hex'] == sections[1]['blocks_hex']
    cow = next(entry for entry in entities['entities'] if entry['type'] == 'cow')
    motions = entities['motions']
    assert len(motions) == 3
    for entry in motions:
        assert entry['position'] == entry['expected_position']
        assert any(packet['name'] == 'entity_move_look' for packet in entry['packets'])
        assert all(packet['name'] in ('entity_move_look', 'rel_entity_move')
                   for packet in entry['packets'])
    return {
        'protocol': 776,
        'jar_sha1': light['jar_sha1'],
        'source_reports': sources,
        'provenance': light['provenance'],
        'limits': [
            'Ordinary network payloads only; no registry, worldgen, datapack or JAR-content dump.',
            'Only authored block palette is extracted; biome0 in Rust tests is synthetic.',
            'Movement is three authored cow moves; no full entity/gameplay claim.',
            'Teleport ACK shape is pinned JSON plus own probe acceptance, not captured GUI-client bytes.',
            'No public wire/admission/primary776 route is enabled.',
        ],
        'light': cases,
        'sections': sections,
        'cow': {'id': cow['id'], 'spawn_position': cow['spawn_position']},
        'motions': motions,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    data = (json.dumps(extract(args.source_root), ensure_ascii=False, indent=2) + '\n').encode()
    if args.check:
        assert args.output.read_bytes() == data, 'fixture differs'
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(data)
    print(json.dumps({'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
                      'check': args.check}))


if __name__ == '__main__':
    main()
