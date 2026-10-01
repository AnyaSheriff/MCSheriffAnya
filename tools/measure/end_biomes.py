#!/usr/bin/env python3
# Биомы Энда оригинала: какой биом у клетки 4×4 на уровне y=56 и как он
# связан с расстоянием от начала и с толщиной острова под ним. Только
# чтение region-файлов (каждый четвёртый чанк по обеим осям).
#
#   python3 tools/measure/end_biomes.py <папка region> <end.pkl>
import os, sys, pickle, math, collections
sys.path.insert(0, os.path.dirname(__file__))
from end_main import read_chunk
from nether_census import unpack

def biome_grid(root, y=56):
    sy = y // 16
    for sec in root.get('sections', []):
        if sec.get('Y') != sy: continue
        b = sec.get('biomes')
        if not b: return None
        pal = [p.replace('minecraft:', '') for p in b['palette']]
        if len(pal) == 1: return [pal[0]] * 64
        bits = max(1, (len(pal) - 1).bit_length())
        idx = unpack(b['data'], bits, 64)
        base = ((y % 16) // 4) * 16
        return [pal[idx[base + i]] for i in range(16)]
    return None

if __name__ == '__main__':
    folder = sys.argv[1]
    data = pickle.load(open(sys.argv[2], 'rb'))['columns']
    by_ring = collections.defaultdict(collections.Counter)
    by_thick = collections.defaultdict(collections.Counter)
    for cx in range(-128, 128, 3):
        for cz in range(-128, 128, 3):
            root = read_chunk(folder, cx, cz)
            if not root: continue
            grid = biome_grid(root)
            if not grid: continue
            col = data.get((cx, cz))
            for cell in range(16):
                gx, gz = cell % 4, cell // 4
                r = math.hypot(cx * 16 + gx * 4 + 2, cz * 16 + gz * 4 + 2)
                by_ring[int(r // 250) * 250][grid[cell]] += 1
                if col:
                    c = col[(gz * 4 + 2) * 16 + gx * 4 + 2]
                    if c: by_thick[min(c[2] // 5, 12) * 5][grid[cell]] += 1
                    else: by_thick[-1][grid[cell]] += 1
    print('по расстоянию:')
    for k in sorted(by_ring): print(k, dict(by_ring[k].most_common()))
    print('по толщине эндемняка в столбце (-1 — пусто):')
    for k in sorted(by_thick): print(k, dict(by_thick[k].most_common()))
