#!/usr/bin/env python3
"""Исходная высота и состояния бамбука: только чтение сохранённых region.

План должен описывать уже сохранённый официальный мир с выключенными
случайными тиками. Высота роста по возрасту не подменяет высоту генерации.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

import numpy as np

from census_263 import chunks, unpack
from genaudit_263 import requested
from genaudit_263_io import Reader, level, region_dir


def measure(world, plan):
    metadata = level(world)
    if metadata['seed'] != plan['seed'] or not isinstance(metadata['version'], dict):
        raise ValueError('Нужен официальный мир с семенем плана')
    wanted = requested(plan, 'overworld')
    reader = Reader()
    seen, files = set(), []
    heights, grounds, states = Counter(), Counter(), Counter()
    for file in sorted(region_dir(world, 'overworld').glob('*.mca')):
        rx, rz = map(int, file.stem.split('.')[1:])
        if not any(x//32 == rx and z//32 == rz for x, z in wanted):
            continue
        files.append([file.name, hashlib.sha256(file.read_bytes()).hexdigest()])
        for cx, cz, blocks, _, _ in reader.read(file, wanted):
            seen.add((cx, cz))
            bamboo = blocks == reader.lookup['bamboo']
            for z, x in np.argwhere(bamboo.any(axis=0)):
                for y in np.flatnonzero(bamboo[:, z, x] & ~np.r_[False, bamboo[:-1, z, x]]):
                    height = 1
                    while y+height < 384 and bamboo[y+height, z, x]:
                        height += 1
                    heights[int(height)] += 1
                    grounds[reader.names[int(blocks[y-1, z, x])]] += 1
        # Reader объединяет состояния по имени блока; свойства считаем
        # отдельно из той же сохранённой палитры чанка.
        for root in chunks(file):
            if (root['xPos'], root['zPos']) not in wanted:
                continue
            for section in root.get('sections', []):
                container = section.get('block_states')
                if not container:
                    continue
                palette = container['palette']
                ids = unpack(container['data'], max(4, (len(palette)-1).bit_length()), 4096) if len(palette) > 1 else [0]*4096
                for index, count in Counter(ids).items():
                    block = palette[index]
                    if isinstance(block, str):
                        name, properties = block.split('[')[0], {}
                    else:
                        name = block.get('id', block.get('Name', block.get('', '')))
                        properties = block.get('properties', block.get('Properties', {}))
                    if name.removeprefix('minecraft:') == 'bamboo':
                        states[','.join(f'{k}={v}' for k, v in sorted(properties.items()))] += count
    if seen != wanted:
        raise ValueError(f'Неполное покрытие: {len(seen)}/{len(wanted)}')
    return {'plan': plan, 'level': metadata, 'files': files,
            'chunks': len(seen), 'height_histogram': dict(sorted(heights.items())),
            'ground_at_base': dict(grounds), 'states': dict(states),
            'bamboo_blocks': sum(states.values()), 'columns': len(seen)*256}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world', type=Path)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = measure(args.world, json.loads(args.plan.read_text()))
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print(result['chunks'], 'chunks;', result['bamboo_blocks'], 'bamboo blocks')


if __name__ == '__main__':
    main()
