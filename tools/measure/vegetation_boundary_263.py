#!/usr/bin/env python3
"""Разделяет плотности на расстоянии от биома-источника; только чтение region.

Расстояние между центрами 4x4x4 ячеек, в блоках. Чанк каймы исключён:
неизвестный сосед не выдаётся за отсутствие биома. Общий допуск переписи
не меняется; это диагностика различий между чистым окном и границей.
"""
import argparse
from collections import Counter
import json
from pathlib import Path

import numpy as np
from scipy.ndimage import distance_transform_edt

from genaudit_263_io import Reader, region_dir, level

PAIRS = {
    'old_birch_dark_oak': ('old_growth_birch_forest',('dark_forest',),'dark_oak_log'),
    'plains_litter': ('plains',('forest','dark_forest','dappled_forest','wooded_badlands'),'leaf_litter'),
    'snowy_taiga_poplar': ('snowy_taiga',('dappled_forest',),'yellow_poplar_leaves'),
}


def measure(world, plan):
    metadata = level(world)
    if metadata['seed'] != plan['seed']:
        raise ValueError('Семя мира отличается от плана')
    reader = Reader()
    results = []
    for x0,z0,x1,z1 in plan['windows']['overworld']:
        wanted = {(x,z) for x in range(x0,x1+1) for z in range(z0,z1+1)}
        biome = np.full((96,(z1-z0+1)*4,(x1-x0+1)*4),'unknown',dtype='U32')
        seen = set()
        features = {name:[] for _,_,name in PAIRS.values()}
        for file in sorted(region_dir(world,'overworld').glob('r.*.*')):
            if file.suffix not in ('.mca','.mcs'):
                continue
            for cx,cz,blocks,bios,_ in reader.read(file,wanted):
                if (cx,cz) in seen:
                    raise ValueError('Повтор чанка')
                seen.add((cx,cz))
                qx,qz=(cx-x0)*4,(cz-z0)*4
                biome[:,qz:qz+4,qx:qx+4]=np.asarray(bios).reshape(96,4,4)
                if cx in (x0,x1) or cz in (z0,z1):
                    continue
                for name,points in features.items():
                    points.extend((int(y//4),qz+int(z//4),qx+int(x//4))
                                  for y,z,x in np.argwhere(blocks == reader.lookup[name]))
        if seen != wanted:
            raise ValueError(f'Неполное покрытие: {len(seen)}/{len(wanted)}')
        interior = np.zeros_like(biome,dtype=bool)
        interior[:,4:-4,4:-4]=True
        pairs = {}
        for key,(target,sources,name) in PAIRS.items():
            source = np.isin(biome,sources)
            distances = distance_transform_edt(~source,sampling=4) if source.any() else np.full(biome.shape,np.inf)
            # 0: до 4, 1: до 8, 2: до 12, 3: дальше 12 блоков.
            bands = np.searchsorted([4,8,12],distances,side='left')
            mask = (biome == target)&interior
            volume = Counter({str(i):int(np.count_nonzero(mask&(bands==i)))*64 for i in range(4)})
            counts = Counter()
            for y,z,x in features[name]:
                if biome[y,z,x] == target:
                    counts[str(bands[y,z,x])] += 1
            pairs[key]={'biome_blocks':dict(volume),'blocks':dict(counts),
                        'per_million':{i:counts[i]*1e6/v if v else None for i,v in volume.items()}}
        results.append({'bounds': [x0,z0,x1,z1], 'chunks_read':len(seen), 'pairs':pairs})
    return {'seed':plan['seed'],'level':metadata,'windows':results,
            'distance_bands_blocks':['<=4','4..8','8..12','>12'],
            'method':'3D quart-cell centre distance, one-chunk halo excluded'}


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('world',type=Path)
    p.add_argument('--plan',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    result=measure(a.world,json.loads(a.plan.read_text()))
    a.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))
