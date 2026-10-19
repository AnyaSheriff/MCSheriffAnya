#!/usr/bin/env python3
"""Ограниченная взаимно однозначная таблица block states из разрешённых JSON."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
from generate_tables import states

ROOT = Path(__file__).resolve().parents[2]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--data', type=Path, required=True)
    a = p.parse_args()
    manifest = json.loads((a.data / 'sources.json').read_text())
    for entry in manifest['files']:
        assert hashlib.sha256((a.data / entry['file']).read_bytes()).hexdigest() == entry['sha256']
    source = ROOT / 'tools/data/blocks.json'
    modern = states(json.loads(source.read_text()))
    assert max(modern.values()) < 65535
    legacy = json.loads((a.data / 'legacy.json').read_text())['blocks']
    rows, skipped = [], []
    # Эти свойства могут вычисляться из соседей/BlockEntity, отсутствующих
    # в legacy metadata. Их нельзя объявлять точным переводом одной ячейки.
    derived = {'shape', 'snowy', 'north', 'east', 'south', 'west', 'up', 'down',
               'distance', 'persistent', 'occupied', 'in_wall', 'instrument', 'note'}
    for pair, label in legacy.items():
        block, meta = map(int, pair.split(':'))
        assert 0 <= block < 512 and 0 <= meta < 16
        name, _, properties = label.removeprefix('minecraft:').partition('[')
        props = dict(v.split('=') for v in properties.removesuffix(']').split(',')) if properties else {}
        reason = None
        if block in {64, 71, 140, 144, 175, 176, 177, 193, 194, 195, 196, 197}:
            reason = 'requires-neighbours-or-block-entity'
        elif block in {104, 105} and meta >= 8:
            reason = 'attached-stem-requires-neighbours'
        elif derived.intersection(props):
            reason = 'requires-neighbours-or-block-entity'
        elif block == 155 and meta in (6, 10):
            reason = 'official-command-normalizes-to-155:0'
        key = (name, tuple(sorted(props.items())))
        if key not in modern:
            reason = reason or 'no-exact-modern-state'
        if reason:
            skipped.append({'legacy': pair, 'name': label, 'reason': reason})
        else:
            rows.append((block * 16 + meta, modern[key], label))
    counts = Counter(row[1] for row in rows)
    accepted = []
    for old, new, name in rows:
        if counts[new] != 1:
            skipped.append({'legacy': f'{old//16}:{old%16}', 'name': name, 'reason': 'ambiguous-reverse-state'})
        else:
            accepted.append((old, new, name))
    text = '// Сгенерировано generate_legacy_blocks.py из закреплённых JSON.\n'
    text += '// Только точные свойства без соседских состояний и визуальных замен.\n'
    text += f'pub const NATIVE_STATES: usize = {max(modern.values()) + 1};\n'
    text += 'pub const BLOCKS: &[(u16, u32)] = &[\n'
    for old, new, name in sorted(accepted):
        text += f'    ({old}, {new}), // {name}\n'
    text += '];\n'
    (ROOT / 'tools/protocol-bridge/src/legacy_block_ids.rs').write_text(text)
    report = {'mapped': len(accepted), 'core_blocks_sha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'skipped': skipped}
    (ROOT / 'tools/protocol-bridge/legacy-blocks-1.12.2.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'mapped': len(accepted), 'skipped': len(skipped)}))


if __name__ == '__main__':
    main()
