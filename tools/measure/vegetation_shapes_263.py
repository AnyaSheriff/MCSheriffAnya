#!/usr/bin/env python3
"""Только чтение region: цвет естественных тополей и опад изолированных елей.

Основание — три вертикальных брёвен на почве. Цвет берётся у ближайшего
листа к верхушке ствола. Опад учитывается только при отсутствии другого
основания в радиусе 12 блоков; край неполного окна исключён.
"""
import argparse
from collections import Counter
import json
from pathlib import Path

import numpy as np
from scipy.spatial import cKDTree

from genaudit_263 import requested
from genaudit_263_io import Reader, region_dir, level


def measure(world, plan, require_vanilla=False, allow_incomplete=False):
    metadata = level(world)
    if metadata['seed'] != plan['seed']:
        raise ValueError('Семя мира отличается от плана')
    if require_vanilla and not isinstance(metadata['version'],dict):
        raise ValueError('Для эталона нужен официальный мир, а не MCSA')
    reader = Reader()
    wanted = requested(plan, 'overworld')
    bases, leaves, litter = [], {}, set()
    seen = set()
    soil = {reader.lookup[n] for n in ('grass_block', 'dirt', 'coarse_dirt', 'podzol', 'rooted_dirt')}
    names = {n for n in reader.lookup if n.endswith('_log')}
    for file in sorted(region_dir(world, 'overworld').glob('r.*.*')):
        if file.suffix not in ('.mca', '.mcs'):
            continue
        for cx, cz, blocks, _, _ in reader.read(file, wanted):
            seen.add((cx,cz))
            for name in names:
                code = reader.lookup[name]
                for y, z, x in np.argwhere(blocks == code):
                    if y > 0 and y + 2 < 384 and blocks[y-1,z,x] in soil and np.all(blocks[y:y+3,z,x] == code):
                        top = y + 2
                        while top + 1 < 384 and blocks[top+1,z,x] == code:
                            top += 1
                        bases.append((int(cx*16+x),int(y-64),int(cz*16+z),int(top-64),name))
            for code in np.unique(blocks):
                name = reader.names[code]
                if name.endswith('_leaves'):
                    for y,z,x in np.argwhere(blocks == code):
                        leaves[int(cx*16+x),int(y-64),int(cz*16+z)] = name
                elif name == 'leaf_litter':
                    litter.update((int(cx*16+x),int(y-64),int(cz*16+z)) for y,z,x in np.argwhere(blocks == code))
    if seen != wanted and not allow_incomplete:
        raise ValueError(f"Неполное покрытие: {len(seen)}/{len(wanted)}")
    positions = list(leaves)
    canopy = cKDTree(positions) if positions else None
    colours = Counter()
    isolated = []
    for x,y,z,top,name in bases:
        if name == 'poplar_log' and canopy is not None:
            distance,index = canopy.query((x,top+1,z))
            if distance <= 4:
                colours[leaves[positions[index]]] += 1
        if name != 'spruce_log' or any((a-x)**2+(c-z)**2 < 144 for a,b,c,_,_ in bases if (a,b,c) != (x,y,z)):
            continue
        if any(((x+dx)//16,(z+dz)//16) not in seen for dx in (-6,6) for dz in (-6,6)):
            continue
        mask = sorted({(a-x,c-z) for a,b,c in litter if abs(a-x)<=5 and abs(c-z)<=5 and abs(b-y)<=4})
        isolated.append({'origin':[x,y,z], 'mask':mask})
    return {'bases':dict(Counter(b[-1] for b in bases)), 'poplar_tree_colours':dict(colours),
            'isolated_spruce_litter':isolated, 'isolation_radius':12, 'chunks_requested':len(wanted), 'chunks_read':len(seen), 'level':metadata}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('world',type=Path)
    p.add_argument('--plan',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--require-vanilla',action='store_true')
    p.add_argument('--allow-incomplete',action='store_true')
    args = p.parse_args()
    result = measure(args.world,json.loads(args.plan.read_text()),args.require_vanilla,args.allow_incomplete)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k != 'isolated_spruce_litter'}))
    print('isolated spruce:',len(result['isolated_spruce_litter']))


if __name__ == '__main__':
    main()
