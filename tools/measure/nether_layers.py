#!/usr/bin/env python3
# Незер по слоям: доли блоков на каждой высоте и полы на столбец — сколько
# раз в столбце сплошной блок стоит под пустотой (над морем лавы), сколько
# потолков, какой толщины пустоты и перемычки между ними. Файлы мира только
# читаются — это данные, которые сложил запущенный сервер; код игры не
# используется.
#
#   python3 tools/measure/nether_layers.py <папка region> <вывод .tsv> [шаг]
#
# Шаг — брать каждый n-й region-файл (для быстрой выборки). Тот же вывод
# пишет наш ignored-тест nether_layers (src/world/nether.rs), сверка —
# tools/measure/nether_layers_compare.py.
import os, sys, collections, multiprocessing
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack
import struct, zlib

EMPTY = {'air', 'cave_air', 'void_air'}
# Сквозь что видно пол: пустота, огонь и растения (лава — нет: пол у моря
# лавы считается отдельно).
PLANTS = {'fire', 'soul_fire', 'crimson_roots', 'warped_roots', 'nether_sprouts', 'crimson_fungus',
          'warped_fungus', 'brown_mushroom', 'red_mushroom', 'weeping_vines', 'weeping_vines_plant',
          'twisting_vines', 'twisting_vines_plant'}

def region_chunks(path):
    data = open(path, 'rb').read()
    for i in range(1024):
        off = int.from_bytes(data[i*4:i*4+3], 'big')
        if off == 0: continue
        p = off * 4096
        if p + 5 > len(data): continue
        ln = struct.unpack_from('>i', data, p)[0]
        try: raw = zlib.decompress(data[p+5:p+4+ln])
        except zlib.error: continue
        n = struct.unpack_from('>H', raw, 1)[0]
        root, _ = nbt(raw, 3 + n, 10)
        if root.get('Status') not in ('minecraft:full', 'full'): continue
        yield root

def column_stats(col, stats):
    """Полы, потолки, пустоты и перемычки одного столбца (список 128 имён)."""
    open_ = [c in EMPTY or c in PLANTS for c in col]
    floors = ceilings = 0
    for y in range(32, 126):
        if not open_[y] and col[y] != 'lava' and open_[y + 1]:
            floors += 1
            stats['floor_y'][y] += 1
        if not open_[y] and col[y] != 'lava' and open_[y - 1] and col[y - 1] != 'lava':
            ceilings += 1
            stats['ceiling_y'][y] += 1
    stats['floors'][floors] += 1
    stats['ceilings'][ceilings] += 1
    # Пустоты (над морем) и перемычки между ними — по толщине.
    y = 32
    while y < 127:
        if open_[y]:
            top = y
            while top < 127 and open_[top]: top += 1
            stats['gap'][min(top - y, 99)] += 1
            y = top
        else:
            top = y
            while top < 127 and not open_[top]: top += 1
            if y > 32 and top < 127:
                stats['slab'][min(top - y, 99)] += 1
            y = top

REACH_Y = (40, 56, 72, 88)
REACH_D = (2, 4, 8, 16, 32)

def one_region(path):
    per_y = collections.defaultdict(collections.Counter)
    stats = {k: collections.Counter() for k in ('floors', 'ceilings', 'floor_y', 'ceiling_y', 'gap', 'slab', 'reach')}
    chunks = 0
    opens = {}
    for root in region_chunks(path):
        grid = [None] * 128
        for sec in root.get('sections', []):
            sy = sec.get('Y', 0)
            if isinstance(sy, int) and sy > 127: sy -= 256
            if not 0 <= sy <= 7: continue
            bs = sec.get('block_states')
            if not bs: continue
            pal = [e['Name'].replace('minecraft:', '') for e in bs['palette']]
            idx = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
            for ly in range(16):
                grid[sy * 16 + ly] = [pal[k] for k in idx[ly * 256:(ly + 1) * 256]]
        if any(g is None for g in grid): continue
        chunks += 1
        cx, cz = root['xPos'], root['zPos']
        for y in REACH_Y:
            for i in range(256):
                opens[(y, cx * 16 + (i & 15), cz * 16 + (i >> 4))] = grid[y][i] in EMPTY
        for y in range(128):
            per_y[y].update(grid[y])
        for i in range(256):
            column_stats([grid[y][i] for y in range(128)], stats)
    # Как далеко по горизонтали тянется пустота: из пустого места на d
    # блоков по x — пусто ли там.
    for (y, x, z), o in opens.items():
        if not o: continue
        for d in REACH_D:
            v = opens.get((y, x + d, z))
            if v is not None:
                stats['reach'][(y, d, 0)] += 1
                stats['reach'][(y, d, 1)] += v
    return chunks, per_y, stats

def write(out, chunks, per_y, stats):
    """Общий формат вывода: строки «раздел ключ значение»."""
    columns = chunks * 256
    with open(out, 'w') as f:
        f.write('chunks\t-\t%d\n' % chunks)
        for y in sorted(per_y):
            t = sum(per_y[y].values())
            for b, c in per_y[y].items():
                f.write('y%d\t%s\t%.6f\n' % (y, b, c / t))
        for (y, d, kind), n in sorted(stats['reach'].items()):
            if kind == 0: f.write('reach%d\t%d\t%.6f\n' % (y, d, stats['reach'][(y, d, 1)] / n))
        for k, c in stats.items():
            if k == 'reach': continue
            for v, n in sorted(c.items()):
                f.write('%s\t%d\t%.6f\n' % (k, v, n / columns))

def main():
    folder, out = sys.argv[1], sys.argv[2]
    step = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    paths = [os.path.join(folder, n) for n in sorted(os.listdir(folder)) if n.endswith('.mca')][::step]
    per_y = collections.defaultdict(collections.Counter)
    stats = {k: collections.Counter() for k in ('floors', 'ceilings', 'floor_y', 'ceiling_y', 'gap', 'slab', 'reach')}
    chunks = 0
    with multiprocessing.Pool(min(6, os.cpu_count())) as pool:
        for n, py, st in pool.imap_unordered(one_region, paths):
            chunks += n
            for y, c in py.items(): per_y[y].update(c)
            for k, c in st.items(): stats[k].update(c)
    write(out, chunks, per_y, stats)
    print('чанков:', chunks, '→', out)

if __name__ == '__main__':
    main()
