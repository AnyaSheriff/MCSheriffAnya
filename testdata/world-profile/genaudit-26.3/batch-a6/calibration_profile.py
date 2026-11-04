#!/usr/bin/env python3
"""Малые профили A6: хвосты высот/уклонов, воздух и материалы по биому."""
import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
import numpy as np
ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'tools/measure'))
from genaudit_263 import natural, requested
from genaudit_263_io import Reader, region_dir


def measure(world, plan):
    wanted = requested(plan, 'overworld')
    reader = Reader()
    chunks = {}
    heights = defaultdict(list)
    slopes = defaultdict(list)
    layers = defaultdict(Counter)
    air_bands = defaultdict(lambda: defaultdict(lambda: [0, 0]))
    bands = defaultdict(lambda: defaultdict(Counter))
    for path in sorted(region_dir(world, 'overworld').glob('*.mc*')):
        rx, rz = map(int, path.stem.split('.')[1:])
        if not any(cx//32 == rx and cz//32 == rz for cx, cz in wanted):
            continue
        for cx, cz, blocks, bios, _ in reader.read(path, wanted):
            names = np.asarray(reader.names)
            ground = np.asarray([natural(n) for n in names])[blocks]
            top = np.where(ground.any(axis=0), 383-ground[::-1].argmax(axis=0), 0)
            zz, xx = np.indices(top.shape)
            bio = np.asarray(bios).reshape(96, 4, 4)[top//4, zz//4, xx//4]
            chunks[cx, cz] = (top.astype(np.int32)-64, bio)
            air = np.asarray([n in ('air', 'cave_air', 'void_air') for n in names])[blocks]
            for biome in np.unique(bio):
                mask = bio == biome
                heights[biome].extend((top[mask]-64).tolist())
                for depth in range(5):
                    layers[biome, depth].update(names[blocks[np.maximum(0, top-depth), zz, xx]][mask].tolist())
                for y in (128, 144, 160, 176, 192):
                    eligible = (np.arange(y+64, y+80)[:, None, None] < top[None]-5) & mask[None]
                    count = air_bands[biome][str(y)]
                    count[0] += int((air[y+64:y+80] & eligible).sum())
                    count[1] += int(eligible.sum())
                if 'badlands' in biome:
                    colors = [str(n) for n in names if n == 'terracotta' or n.endswith('_terracotta')]
                    for y in range(63, 161):
                        values = names[blocks[y+64]][mask]
                        bands[biome][str(y)].update(v for v in values if v in colors)
    if set(chunks) != wanted:
        raise ValueError(f'Неполная выборка: {len(chunks)}/{len(wanted)}')
    # Тот же уклон floor(hypot(dx,dz)), что в переписи; без длинных пустых рамок.
    for (cx, cz), (h, bio) in chunks.items():
        dx = np.zeros((16, 16), dtype=np.int32)
        dz = np.zeros((16, 16), dtype=np.int32)
        valid = np.ones((16, 16), dtype=bool)
        dx[:, :-1] = h[:, 1:] - h[:, :-1]
        dz[:-1] = h[1:] - h[:-1]
        if (cx+1, cz) in chunks:
            dx[:, -1] = chunks[cx+1, cz][0][:, 0]-h[:, -1]
        else:
            valid[:, -1] = False
        if (cx, cz+1) in chunks:
            dz[-1] = chunks[cx, cz+1][0][0]-h[-1]
        else:
            valid[-1] = False
        slope = np.floor(np.sqrt(dx*dx+dz*dz)).astype(np.int32)
        for biome in np.unique(bio):
            slopes[biome].extend(slope[valid & (bio == biome)].tolist())
    result = {'chunks': len(chunks), 'definition': 'Natural substrate; per surface biome; '
              'slope is floor(hypot(forward dx, forward dz)); only neighbours present in plan. '
              'Air bands require six blocks below top; zero denominators remain explicit.', 'biomes': {}}
    for biome, h in sorted(heights.items()):
        result['biomes'][biome] = {
            'columns': len(h),
            'height_p50_p95_p99_max': np.percentile(h, [50, 95, 99, 100]).tolist(),
            'slope_p95_p99_max': np.percentile(slopes[biome], [95, 99, 100]).tolist() if slopes[biome] else None,
            'air_bands': dict(air_bands[biome]),
            'layers': {str(d): dict(layers[biome, d]) for d in range(5)},
            'terracotta_by_y': dict(bands[biome])}
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world', type=Path)
    parser.add_argument('--plan', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    result = measure(args.world, json.loads(args.plan.read_text()))
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print(args.output, result['chunks'], flush=True)
