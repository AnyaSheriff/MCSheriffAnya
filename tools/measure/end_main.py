#!/usr/bin/env python3
# Главный остров Энда оригинала: блоки центральных чанков в словарь
# (x, y, z) → имя. Только чтение region-файлов.
#
#   python3 tools/measure/end_main.py <папка region> <радиус в чанках> <файл вывода .pkl>
import os, sys, struct, zlib, pickle
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack

def read_chunk(folder, cx, cz):
    path = os.path.join(folder, 'r.%d.%d.mca' % (cx >> 5, cz >> 5))
    if not os.path.exists(path): return None
    data = open(path, 'rb').read()
    i = (cx & 31) + (cz & 31) * 32
    off = int.from_bytes(data[i*4:i*4+3], 'big')
    if off == 0: return None
    p = off * 4096
    ln = struct.unpack_from('>i', data, p)[0]
    raw = zlib.decompress(data[p+5:p+4+ln])
    n = struct.unpack_from('>H', raw, 1)[0]
    root, _ = nbt(raw, 3 + n, 10)
    return root

def blocks_of(root):
    cx, cz = root['xPos'], root['zPos']
    out = {}
    for sec in root.get('sections', []):
        sy = sec.get('Y', 0)
        if isinstance(sy, int) and sy > 127: sy -= 256
        bs = sec.get('block_states')
        if not bs: continue
        pal = [e for e in bs['palette']]
        names = [e['Name'].replace('minecraft:', '') for e in pal]
        if len(pal) == 1:
            idx = [0] * 4096
        else:
            bits = max(4, (len(pal) - 1).bit_length())
            idx = unpack(bs['data'], bits, 4096)
        for i, k in enumerate(idx):
            if names[k] in ('air', 'void_air', 'cave_air'): continue
            props = pal[k].get('Properties')
            label = names[k] if not props else names[k] + '[' + ','.join('%s=%s' % kv for kv in sorted(props.items())) + ']'
            out[(cx * 16 + (i & 15), sy * 16 + (i >> 8), cz * 16 + ((i >> 4) & 15))] = label
    return out

if __name__ == '__main__':
    folder, radius, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    world = {}
    for cx in range(-radius, radius):
        for cz in range(-radius, radius):
            root = read_chunk(folder, cx, cz)
            if root: world.update(blocks_of(root))
    pickle.dump(world, open(out, 'wb'))
    print(len(world))
