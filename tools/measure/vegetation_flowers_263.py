#!/usr/bin/env python3
"""Совпадение видов соседних цветов; только чтение сохранённых регионов.

Ландыш исключён: он имеет отдельные грядки поверх градиента. Частоты
видов и соседство измеряются раздельно: случайная смесь может совпасть
по общему числу цветов, но не воспроизводить градиент.
"""
import argparse
from collections import Counter
import json
from pathlib import Path

import numpy as np
from scipy.spatial import cKDTree

from genaudit_263 import requested
from genaudit_263_io import Reader, level, region_dir

FLOWERS = 'dandelion poppy allium azure_bluet red_tulip orange_tulip white_tulip pink_tulip oxeye_daisy cornflower'.split()


def measure(world, plan, biome):
    metadata = level(world)
    if metadata['seed'] != plan['seed']:
        raise ValueError('Семя мира отличается от плана')
    reader = Reader()
    codes = [reader.lookup[n] for n in FLOWERS]
    wanted = requested(plan, 'overworld')
    seen, positions, species = set(), [], []
    for file in sorted(region_dir(world, 'overworld').glob('r.*.*')):
        if file.suffix not in ('.mca', '.mcs'):
            continue
        for cx,cz,blocks,bios,_ in reader.read(file,wanted):
            seen.add((cx,cz))
            bio = np.asarray(bios).reshape(96,4,4)
            for y,z,x in np.argwhere(np.isin(blocks,codes)):
                if bio[y//4,z//4,x//4] == biome:
                    positions.append((int(cx*16+x),int(y-64),int(cz*16+z)))
                    species.append(reader.names[int(blocks[y,z,x])])
    if seen != wanted:
        raise ValueError(f'Неполное покрытие: {len(seen)}/{len(wanted)}')
    counts = Counter(species)
    bands = [dict(pairs=0,same_species=0) for _ in range(5)]
    if positions:
        points = np.asarray(positions)
        for i,j in cKDTree(points).query_pairs(32):
            distance = np.linalg.norm(points[i]-points[j])
            band = int(np.searchsorted([2,4,8,16,32],distance,side='left'))
            bands[band]['pairs'] += 1
            bands[band]['same_species'] += int(species[i]==species[j])
    for band in bands:
        band['agreement'] = band['same_species']/band['pairs'] if band['pairs'] else None
    return {'level':metadata,'plan':plan,'biome':biome,'flowers':dict(counts),
            'independent_mixture_agreement':sum((n/len(species))**2 for n in counts.values()),
            'distance_bands':['<=2','2..4','4..8','8..16','16..32'],'bands':bands,
            'chunks':len(seen),'method':'3D Euclidean distances, each unordered pair counted once; lily_of_the_valley excluded'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world',type=Path)
    parser.add_argument('--plan',type=Path,required=True)
    parser.add_argument('--biome',choices=['flower_forest','meadow'],required=True)
    parser.add_argument('--output',type=Path,required=True)
    args = parser.parse_args()
    result = measure(args.world,json.loads(args.plan.read_text()),args.biome)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(result['flowers'],result['bands'])
