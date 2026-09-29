#!/usr/bin/env python3
# Форма шляп огромных грибов Незера: сколько наростов и светогрибов на каждом
# слое относительно верха ствола, как далеко от ствола, сколько из них
# открыто сверху (считается полом). Файлы мира только читаются — это
# данные, которые сложил запущенный сервер; код игры не используется.
#
#   python3 tools/measure/nether_fungus.py region <папка region> [файлы…]
#   python3 tools/measure/nether_fungus.py dump <файл ignored-теста nether_fungus_dump>
#
# Оба пути сводят кусок мира y 32..127 в коды блоков (CODES) и считают
# одинаково, так что оригинал и наш сравниваются одним счётом.
import os, sys, struct, zlib, collections, multiprocessing
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack

Y0, H = 32, 96
AIR, CSTEM, CWART, SHROOM, NYLIUM, PLANT, OTHER, WSTEM, WWART, LAVA = range(10)
CODES = {
    'air': AIR, 'cave_air': AIR, 'void_air': AIR,
    'crimson_stem': CSTEM, 'nether_wart_block': CWART, 'shroomlight': SHROOM,
    'crimson_nylium': NYLIUM, 'warped_nylium': NYLIUM,
    'warped_stem': WSTEM, 'warped_wart_block': WWART, 'lava': LAVA,
}
for name in ('crimson_roots', 'warped_roots', 'nether_sprouts', 'crimson_fungus', 'warped_fungus',
             'weeping_vines', 'weeping_vines_plant', 'twisting_vines', 'twisting_vines_plant', 'fire'):
    CODES[name] = PLANT
OPEN = (AIR, PLANT, LAVA)
FREE_ONLY = os.environ.get('FREE') == '1'

def region_grid(path):
    """Region-файл → (x0, z0, ширина, коды), недогруженные чанки — OTHER."""
    rx, rz = (int(v) for v in os.path.basename(path).split('.')[1:3])
    W = 512
    grid = bytearray([OTHER]) * (W * W * H)
    data = open(path, 'rb').read()
    full = 0
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
        full += 1
        lx0, lz0 = (root['xPos'] - rx * 32) * 16, (root['zPos'] - rz * 32) * 16
        for sec in root.get('sections', []):
            sy = sec.get('Y', 0)
            if isinstance(sy, int) and sy > 127: sy -= 256
            if not 2 <= sy <= 7: continue
            bs = sec.get('block_states')
            if not bs: continue
            pal = [CODES.get(e['Name'].replace('minecraft:', ''), OTHER) for e in bs['palette']]
            if len(pal) == 1:
                if pal[0] == OTHER: continue
                idx = [0] * 4096
            else:
                idx = unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
            for i, k in enumerate(idx):
                y = sy * 16 + (i >> 8) - Y0
                grid[(y * W + lz0 + ((i >> 4) & 15)) * W + lx0 + (i & 15)] = pal[k]
    return W, grid, full

def dump_grids(path):
    """Вывод ignored-теста: подряд квадраты «ширина (u32), коды»."""
    data = open(path, 'rb').read()
    pos = 0
    while pos < len(data):
        W = struct.unpack_from('<I', data, pos)[0]; pos += 4
        size = W * W * H
        yield W, bytearray(data[pos:pos + size])
        pos += size

def analyse(W, grid):
    at = lambda x, y, z: grid[(y * W + z) * W + x]
    tops = []
    for y in range(1, H - 1):
        base = y * W * W
        for i in range(W * W):
            c = grid[base + i]
            if c != CSTEM and c != WSTEM: continue
            if grid[base + W * W + i] == c: continue
            tops.append((i % W, y, i // W, c))
    # Рядом ли другой гриб: верх ствола в 8 блоках по горизонтали и 20 по высоте.
    cells = collections.defaultdict(list)
    for t in tops: cells[(t[0] // 8, t[2] // 8)].append(t)
    out = []
    margin = 8
    for x, y, z, c in tops:
        if not (margin <= x < W - margin and margin <= z < W - margin): continue
        if y < 14: continue
        lonely = True
        for gx in range(x // 8 - 1, x // 8 + 2):
            for gz in range(z // 8 - 1, z // 8 + 2):
                for ox, oy, oz, _ in cells.get((gx, gz), ()):
                    if (ox, oy, oz) == (x, y, z): continue
                    if max(abs(ox - x), abs(oz - z)) <= 8 and abs(oy - y) <= 20 and max(abs(ox - x), abs(oz - z)) > 1:
                        lonely = False
        # Ствол: вниз до не-ствола.
        h = 0
        while y - h >= 0 and at(x, y - h, z) == c: h += 1
        foot = at(x, y - h, z) if y - h >= 0 else OTHER
        if foot != NYLIUM: continue
        wart = CWART if c == CSTEM else WWART
        # Свободный: шляпе ничто не мешало — ни породы, ни лавы рядом.
        free = all(at(x + dx, y + dy, z + dz) not in (OTHER, LAVA, NYLIUM)
                   for dy in range(-6, 3) for dx in range(-3, 4) for dz in range(-3, 4) if 0 <= y + dy < H)
        layers = {}
        for dy in range(-14, 4):
            yy = y + dy
            if not 0 <= yy < H - 1: continue
            row = collections.Counter()
            for dx in range(-5, 6):
                for dz in range(-5, 6):
                    b = at(x + dx, yy, z + dz)
                    if b != wart and b != SHROOM: continue
                    r = max(abs(dx), abs(dz))
                    row[('w' if b == wart else 's', r)] += 1
                    if at(x + dx, yy + 1, z + dz) in OPEN:
                        row[('open', r)] += 1
            layers[dy] = row
        out.append((c, lonely and (free or not FREE_ONLY), h, layers))
    return out

def show(results, title):
    print('\n==', title)
    for species, stem in (('crimson', CSTEM), ('warped', WSTEM)):
        group = [r for r in results if r[0] == stem and r[1]]
        if not group: continue
        n = len(group)
        heights = collections.Counter(r[2] for r in group)
        total = collections.Counter()
        for r in group:
            for dy, row in r[3].items():
                for k, v in row.items(): total[(dy,) + k] += v
        wart = sum(v for k, v in total.items() if k[1] in 'ws')
        opened = sum(v for k, v in total.items() if k[1] == 'open')
        shroom = sum(v for k, v in total.items() if k[1] == 's')
        print('  %s: грибов %d (одиночных), ствол в среднем %.1f; на гриб наростов и светогрибов %.1f, '
              'открытых сверху %.1f (%.0f %%), светогрибов %.1f' % (species, n, sum(r[2] for r in group) / n,
              wart / n, opened / n, 100 * opened / max(1, wart), shroom / n))
        print('    ствол:', ', '.join('%d:%.2f' % (h, heights[h] / n) for h in sorted(heights)))
        print('    слой от верха ствола: блоков на гриб по кольцам r0..r5 | открытых | заполнение колец r1..r4')
        for dy in range(3, -15, -1):
            ring = [sum(total[(dy, k, r)] for k in 'ws') / n for r in range(6)]
            if sum(ring) < 0.05: continue
            opened = sum(total[(dy, 'open', r)] for r in range(6)) / n
            fill = [ring[r] / (8 * r) for r in range(1, 5)]
            print('    %+3d  %s | %.2f | %s' % (dy, ' '.join('%5.2f' % v for v in ring), opened,
                  ' '.join('%.2f' % v for v in fill)))

def main():
    mode = sys.argv[1]
    results = []
    if mode == 'region':
        folder = sys.argv[2]
        names = sys.argv[3:] or sorted(n for n in os.listdir(folder) if n.endswith('.mca'))
        paths = [os.path.join(folder, n) for n in names]
        with multiprocessing.Pool(min(6, len(paths))) as pool:
            for part in pool.imap_unordered(one_region, paths):
                results.extend(part)
    else:
        for W, grid in dump_grids(sys.argv[2]):
            results.extend(analyse(W, grid))
    show(results, mode)

def one_region(path):
    W, grid, full = region_grid(path)
    return analyse(W, grid) if full else []

if __name__ == '__main__':
    main()
