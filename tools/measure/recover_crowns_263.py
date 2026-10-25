"""Восстанавливает утраченный срез из полных масок; сверяет прежнюю статистику.

Это обратная упаковка сохранённых наблюдений, а не независимый замер оригинала.
"""
import argparse
import gzip
import json
from pathlib import Path

from crown_statistics import population
from pack_crowns_263 import pack


def recover(models):
    raw = {}
    for model in models:
        name = model['species']
        leaves = sorted([bit-8, y, z, distance]
                        for y, z, mask, distance in model['rows']
                        for bit in range(17) if mask & (1 << bit))
        raw.setdefault(name, []).append(dict(
            type=name, trunk=model['trunk'],
            logs=[[x, y, z, 'xyz'[axis]] for x, y, z, axis in model['logs']],
            leaves=leaves))
    return raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--models', type=Path, required=True)
    parser.add_argument('--statistics', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    models = json.loads(args.models.read_text())
    raw = recover(models)
    assert len(models) == 576
    assert {name: len(trees) for name, trees in raw.items()} == {
        name: 128 if name == 'poplar' else 64 for name in raw}
    assert pack(raw) == models
    actual = {name: population(trees) for name, trees in raw.items()}
    assert actual == json.loads(args.statistics.read_text())
    args.output.write_bytes(gzip.compress(
        json.dumps(raw, separators=(',', ':')).encode(), mtime=0))
    print(f'576 forms and all statistics identical; {args.output.stat().st_size} bytes')


if __name__ == '__main__':
    main()
