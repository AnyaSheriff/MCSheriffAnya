#!/usr/bin/env python3
"""Только явные имена/ID биомов340 из закреплённой JSON-таблицы."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--data', type=Path, required=True)
    a = p.parse_args()
    root = Path(__file__).resolve().parent
    manifest = json.loads((root / 'legacy-world-sources-1.12.2.json').read_text())
    entry = next(row for row in manifest['files'] if row['file'] == 'biomes.json')
    raw = (a.data / 'biomes.json').read_bytes()
    assert len(raw) == entry['bytes'] and hashlib.sha256(raw).hexdigest() == entry['sha256']
    rows = json.loads(raw)
    assert len({row['id'] for row in rows}) == len(rows) == len({row['name'] for row in rows})
    text = '// Сгенерировано generate_legacy_biomes.py; pinned JSON, без подмен новых биомов.\n'
    text += 'pub fn id(name: &str) -> Option<u8> {\n    match name {\n'
    for row in sorted(rows, key=lambda row: row['id']):
        assert 0 <= row['id'] <= 255 and all(c.isascii() and (c.isalnum() or c == '_') for c in row['name'])
        text += f'        "{row["name"]}" => Some({row["id"]}),\n'
    text += '        _ => None,\n    }\n}\n'
    (root / 'src/legacy_biomes.rs').write_text(text)
    print('Generated', len(rows), 'legacy biome names')


if __name__ == '__main__': main()
