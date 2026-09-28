#!/usr/bin/env python3
# Незер оригинала по биомам: что лежит на полу и висит на потолке пустот,
# чем посыпано, одинаков ли биом по высоте столбца и насколько крупны пятна
# биомов. Файлы мира только читаются — это данные, которые сложил запущенный
# сервер; код игры не используется.
#
#   python3 tools/measure/nether_biomes.py <папка region> [файл карты биомов]
#
# Работает по region-файлам параллельно (по файлу на процесс).
import os, sys, collections, multiprocessing
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack
import struct, zlib

# Сквозь что видно пол: пустота, лава, огонь и растения.
OPEN = {'air', 'cave_air', 'void_air', 'lava', 'fire', 'soul_fire', 'crimson_roots', 'warped_roots',
        'nether_sprouts', 'crimson_fungus', 'warped_fungus', 'brown_mushroom', 'red_mushroom',
        'weeping_vines', 'weeping_vines_plant', 'twisting_vines', 'twisting_vines_plant'}
EMPTY = {'air', 'cave_air', 'void_air'}

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

def one_region(path):
    floor = collections.defaultdict(collections.Counter)      # биом → верх пола
    floor_low = collections.defaultdict(collections.Counter)  # то же у моря лавы (пол y ≤ 34)
    under = collections.defaultdict(collections.Counter)      # биом → (глубина, блок) под полом
    ceiling = collections.defaultdict(collections.Counter)    # биом → низ потолка
    loose = collections.defaultdict(collections.Counter)      # биом → мелочь (не сплошное, не пустота)
    volume = collections.defaultdict(collections.Counter)     # биом → все блоки y 0..127
    columns = collections.Counter()                           # биом → столбцов
    mixed = [0, 0]                                            # клеток с разными биомами по высоте, всего
    cells = []                                                # (x клетки, z клетки, биом на y 64)
    heights = collections.defaultdict(collections.Counter)    # биом → (блок, y) для мелочи и руд
    for root in region_chunks(path):
        cx, cz = root['xPos'], root['zPos']
        grid = [None] * 128   # y → список 256 имён
        bio = {}              # (слой по 4 от y 0, клетка) → биом
        for sec in root.get('sections', []):
            sy = sec.get('Y', 0)
            if isinstance(sy, int) and sy > 127: sy -= 256
            if not 0 <= sy <= 7: continue
            bs = sec.get('block_states')
            if bs:
                pal = [e['Name'].replace('minecraft:', '') for e in bs['palette']]
                if len(pal) == 1: idx = [0] * 4096
                else: idx = unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                for ly in range(16):
                    grid[sy * 16 + ly] = [pal[k] for k in idx[ly * 256:(ly + 1) * 256]]
            bm = sec.get('biomes')
            if bm:
                pal = [b.replace('minecraft:', '') for b in bm['palette']]
                idx = [0] * 64 if len(pal) == 1 else unpack(bm['data'], (len(pal) - 1).bit_length(), 64)
                for i, k in enumerate(idx):
                    bio[(sy * 4 + (i >> 4), i & 15)] = pal[k]
        if any(g is None for g in grid): continue
        for cell in range(16):
            seen = {bio.get((layer, cell)) for layer in range(32)}
            mixed[1] += 1
            if len(seen) > 1: mixed[0] += 1
            cells.append((cx * 4 + (cell & 3), cz * 4 + (cell >> 2), bio.get((16, cell))))
        for i in range(256):
            x, z = i & 15, i >> 4
            b = bio.get((16, (z >> 2) * 4 + (x >> 2)))
            columns[b] += 1
            vol = volume[b]
            col = [grid[y][i] for y in range(128)]
            for y in range(128):
                name = col[y]
                vol[name] += 1
                if name not in EMPTY and name in OPEN and name != 'lava':
                    loose[b][name] += 1
                    heights[b][(name, y // 8 * 8)] += 1
                elif name not in EMPTY and name != 'bedrock':
                    heights[b][(name, y // 8 * 8)] += 1
            for y in range(1, 127):
                name = col[y]
                if name in OPEN: continue
                above, below = col[y + 1], col[y - 1]
                if above in OPEN and above != 'lava' and y >= 31:
                    (floor_low if y <= 34 else floor)[b][name] += 1
                    for d in range(1, 6):
                        if y - d >= 0: under[b][(d, col[y - d])] += 1
                if below in EMPTY and y >= 32:
                    ceiling[b][name] += 1
    return floor, floor_low, under, ceiling, loose, volume, columns, mixed, cells, heights

def merge(into, part):
    for k, c in part.items(): into[k].update(c)

def show(title, table, columns, per_column=False, top=12):
    print('\n== ' + title)
    for b in sorted(table, key=lambda b: -columns[b]):
        c = table[b]; t = sum(c.values()) or 1
        if per_column:
            items = ', '.join('%s %.3f' % (k, v / columns[b]) for k, v in c.most_common(top))
        else:
            items = ', '.join('%s %.1f%%' % (k, 100 * v / t) for k, v in c.most_common(top))
        print('  %-18s %s' % (b, items))

def patches(cells):
    """Пятна биомов по клеткам 4×4 на y 64: доля смены биома у соседей и
    средняя площадь связной области (в клетках, по области, через которую
    она проходит)."""
    where = {(x, z): b for x, z, b in cells}
    diff = [0, 0]
    for (x, z), b in where.items():
        for dx, dz in ((1, 0), (0, 1)):
            o = where.get((x + dx, z + dz))
            if o is None: continue
            diff[1] += 1
            if o != b: diff[0] += 1
    seen = set(); sizes = collections.defaultdict(list)
    for start in where:
        if start in seen: continue
        b = where[start]; stack = [start]; seen.add(start); n = 0
        while stack:
            x, z = stack.pop(); n += 1
            for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                p = (x + dx, z + dz)
                if p in where and p not in seen and where[p] == b:
                    seen.add(p); stack.append(p)
        sizes[b].append(n)
    return diff, sizes

def main():
    folder = sys.argv[1]
    paths = [os.path.join(folder, n) for n in sorted(os.listdir(folder)) if n.endswith('.mca')]
    floor, floor_low, under, ceiling, loose, volume, heights = (collections.defaultdict(collections.Counter) for _ in range(7))
    columns = collections.Counter(); mixed = [0, 0]; cells = []
    with multiprocessing.Pool(min(10, os.cpu_count())) as pool:
        for part in pool.imap_unordered(one_region, paths):
            f, fl, u, c, l, v, cols, m, ce, h = part
            merge(floor, f); merge(floor_low, fl); merge(under, u); merge(ceiling, c); merge(loose, l)
            merge(volume, v); merge(heights, h); columns.update(cols); mixed[0] += m[0]; mixed[1] += m[1]
            cells.extend(ce)
    total = sum(columns.values())
    print('столбцов:', total, 'чанков:', total // 256)
    print('биомы:', ', '.join('%s %.1f%%' % (b, 100 * n / total) for b, n in columns.most_common()))
    print('клеток с разным биомом по высоте (y 0..127): %d из %d' % tuple(mixed))
    diff, sizes = patches(cells)
    print('смена биома у соседних клеток 4×4: %.2f%%' % (100 * diff[0] / max(1, diff[1])))
    for b, s in sorted(sizes.items(), key=lambda kv: -sum(kv[1])):
        s = sorted(s); area = sum(s)
        # средняя «по площади»: какого размера пятно, в котором стоит случайная клетка
        weighted = sum(v * v for v in s) / area
        print('  %-18s пятен %5d, медиана %4d клеток, взвешенная по площади %7.0f клеток' % (b, len(s), s[len(s) // 2], weighted))
    show('верх пола (над ним пусто, y ≥ 35)', floor, columns)
    show('верх пола у моря лавы (y 31..34)', floor_low, columns)
    print('\n== под полом по глубине')
    for b in sorted(under, key=lambda b: -columns[b]):
        for d in range(1, 6):
            c = collections.Counter({k[1]: v for k, v in under[b].items() if k[0] == d}); t = sum(c.values()) or 1
            print('  %-18s %d: %s' % (b, d, ', '.join('%s %.1f%%' % (k, 100 * v / t) for k, v in c.most_common(6))))
    show('низ потолка (под ним пусто)', ceiling, columns)
    show('мелочь на столбец', loose, columns, per_column=True, top=20)
    show('объём на столбец (y 0..127)', volume, columns, per_column=True, top=24)
    print('\n== высоты (блок: доли по 8 блокам высоты, в штуках на столбец)')
    for b in sorted(heights, key=lambda b: -columns[b]):
        names = sorted({k[0] for k in heights[b]})
        for name in names:
            row = ' '.join('%d:%.3f' % (y, heights[b][(name, y)] / columns[b]) for y in range(0, 128, 8) if heights[b][(name, y)])
            print('  %-16s %-20s %s' % (b, name, row))
    if len(sys.argv) > 2:
        with open(sys.argv[2], 'w') as f:
            for x, z, b in cells: f.write('%d\t%d\t%s\n' % (x, z, b))

if __name__ == '__main__':
    main()
