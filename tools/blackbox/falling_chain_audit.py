#!/usr/bin/env python3
"""Сверяет по тактам появления падающих блоков и все изменения блоков."""
import argparse
from collections import Counter
import gzip
import json
from pathlib import Path


def trace(path):
    raw = gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()
    events = json.loads(raw)['events']
    base = next(event['tick'] for event in events if event['kind'] == 'block')
    blocks, spawns = [], []
    for event in events:
        tick = event['tick'] - base
        if event['kind'] == 'block':
            blocks.append((tick, tuple(event['at']), event['state']))
        elif event['kind'] == 'spawn' and event['type'] == 'falling_block':
            spawns.append((tick, tuple(event['at']), event['data']))
    return blocks, spawns


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('recordings', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    rows = []
    for path in sorted(args.recordings.glob('falling-chain-*-0-official.json*')):
        name = path.name.removesuffix('.gz').removesuffix('-0-official.json')
        ours = args.recordings / f'{name}-0-ours.json'
        if not ours.exists():
            ours = ours.with_suffix('.json.gz')
        official_blocks, official_spawns = trace(path)
        ours_blocks, ours_spawns = trace(ours)
        expected = {'layer': 24, 'column': 6, 'overhang': 3}[name.rsplit('-', 1)[1]]
        assert len(official_spawns) == expected, (name, 'official', len(official_spawns))
        assert len(ours_spawns) == expected, (name, 'ours', len(ours_spawns))
        def wear_normalized(blocks):
            return Counter((tick, at, 'anvil[facing=north]' if '-anvil-' in name
                            and state in ('anvil[facing=north]', 'chipped_anvil[facing=north]') else state)
                           for tick, at, state in blocks)
        row = {
            'scenario': name,
            'official_spawns_by_tick': sorted(Counter(event[0] for event in official_spawns).items()),
            'ours_spawns_by_tick': sorted(Counter(event[0] for event in ours_spawns).items()),
            'official_block_changes': len(official_blocks),
            'ours_block_changes': len(ours_blocks),
            'spawns_equal': Counter(official_spawns) == Counter(ours_spawns),
            'blocks_equal': Counter(official_blocks) == Counter(ours_blocks),
            'blocks_equal_allowing_anvil_wear': wear_normalized(official_blocks) == wear_normalized(ours_blocks),
        }
        rows.append(row)
    assert len(rows) == 12, ('нужны все 12 сценариев', len(rows))
    args.output.write_text(json.dumps(rows, ensure_ascii=False, indent=2) + '\n')
    print(f"Сценариев: {len(rows)}; появления совпали: {sum(row['spawns_equal'] for row in rows)}; изменения блоков без допуска: {sum(row['blocks_equal'] for row in rows)}; с допуском только износа: {sum(row['blocks_equal_allowing_anvil_wear'] for row in rows)}")
    raise SystemExit(0 if all(row['spawns_equal'] and row['blocks_equal_allowing_anvil_wear'] for row in rows) else 1)


if __name__ == '__main__':
    main()
