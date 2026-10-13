# Соседство биомов по сохранённому миру оригинала (region-файлы): доли,
# размеры пятен и с кем граничит каждый биом. Читаются только данные мира.
#
# python3 tools/measure/biome_census.py <папка region> > vanilla.tsv
#
# Биом берётся у поверхности, по клетке 4×4 (как хранит игра). Тот же вывод
# даёт наш тест `biome_neighbours` в src/world/terrain.rs — их сравнивает
# tools/measure/biome_compare.py.
import sys, os, glob, struct, zlib, math, collections, argparse, pickle
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(__file__))
from plants_census import nbt, unpack

def chunk_cells(f):
    """(клетка x, клетка z) -> биом у поверхности, для полных чанков файла."""
    out = {}
    data = open(f, 'rb').read()
    if len(data) < 8192:
        return out
    for i in range(1024):
        off = int.from_bytes(data[i*4:i*4+3], 'big')
        if off == 0:
            continue
        p = off * 4096
        ln = struct.unpack_from('>i', data, p)[0]
        if data[p+4] != 2:
            continue
        raw = zlib.decompress(data[p+5:p+4+ln])
        n = struct.unpack_from('>H', raw, 1)[0]
        root, _ = nbt(raw, 3+n, 10)
        if root.get('Status') not in ('minecraft:full', 'full'):
            continue
        maps = root.get('Heightmaps', {})
        if 'OCEAN_FLOOR' not in maps:
            continue
        floor = [h - 64 - 1 for h in unpack(maps['OCEAN_FLOOR'], 9, 256)]
        sections = {s['Y']: s['biomes'] for s in root.get('sections', []) if 'biomes' in s}
        cx, cz = root['xPos'], root['zPos']
        for cell in range(16):
            x, z = cell & 3, cell >> 2
            y = min(max(floor[(z * 4 + 2) * 16 + x * 4 + 2], -64), 319)
            b = sections.get(y >> 4)
            if not b:
                continue
            pal = b['palette']
            if len(pal) == 1:
                name = pal[0]
            else:
                ids = unpack(b['data'], max(1, math.ceil(math.log2(len(pal)))), 64)
                name = pal[ids[((y & 15) >> 2) * 16 + z * 4 + x]]
            out[(cx * 4 + x, cz * 4 + z)] = (name.replace('minecraft:', ''), y)
    return out

def report(cells):
    """Печатает доли, пятна и соседей — общий формат для обоих миров."""
    heights = collections.defaultdict(list)
    for name, y in cells.values():
        heights[name].append(y)
    for name, values in sorted(heights.items()):
        values.sort()
        at = lambda part: values[int((len(values) - 1) * part)]
        print('height\t%s\t%d\t%d\t%d\t%d\t%d' % (name, at(0.05), at(0.25), at(0.5), at(0.75), at(0.95)))
    cells = {at: name for at, (name, _) in cells.items()}
    share = collections.Counter(cells.values())
    total = sum(share.values())
    for name, count in share.most_common():
        print('share\t%s\t%.6f' % (name, count / total))

    seen, patches = set(), collections.defaultdict(list)
    for start, name in cells.items():
        if start in seen:
            continue
        seen.add(start)
        stack, size = [start], 0
        while stack:
            x, z = stack.pop()
            size += 1
            for near in ((x+1, z), (x-1, z), (x, z+1), (x, z-1)):
                if near not in seen and cells.get(near) == name:
                    seen.add(near)
                    stack.append(near)
        patches[name].append(size)
    for name, sizes in sorted(patches.items()):
        sizes.sort()
        small = sum(1 for s in sizes if s <= 3)
        print('patch\t%s\t%d\t%d\t%d\t%d\t%.3f' % (name, len(sizes), sizes[len(sizes) // 2],
              sizes[int((len(sizes) - 1) * 0.9)], sizes[-1], small / len(sizes)))
        weighted = []
        for part in (0.5, 0.9):
            acc = 0
            for size in sizes:
                acc += size
                if acc >= sum(sizes) * part:
                    weighted.append(size * 16)
                    break
        print('patch_area\t%s\t%d\t%d' % (name, *weighted))
    sizes = sorted(size for sizes in patches.values() for size in sizes)
    print('patch_summary\tall_surface\t%d\t%.3f\t%d\t%d' % (
        len(sizes), total/len(sizes), sizes[len(sizes)//2], sizes[(len(sizes)-1)*9//10]))

    pairs = collections.defaultdict(collections.Counter)
    for (x, z), name in cells.items():
        for near in ((x+1, z), (x, z+1)):
            other = cells.get(near)
            if other is not None and other != name:
                pairs[name][other] += 1
                pairs[other][name] += 1
    for name, others in sorted(pairs.items()):
        edges = sum(others.values())
        for other, count in others.most_common():
            print('pair\t%s\t%s\t%.4f' % (name, other, count / edges))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('region')
    parser.add_argument('--workers', type=int, default=2)
    parser.add_argument('--cache', help='Local decoded-cell cache; source regions stay read-only')
    parser.add_argument('--reference', action='store_true', help='Nine solid vanilla-big squares only')
    args = parser.parse_args()
    cells = {}
    if args.cache and os.path.exists(args.cache):
        with open(args.cache, 'rb') as source:
            cells = pickle.load(source)
    else:
        with Pool(args.workers) as pool:
            for part in pool.imap_unordered(chunk_cells, sorted(glob.glob(args.region + '/*.mca'))):
                cells.update(part)
        if args.cache:
            with open(args.cache, 'wb') as target:
                pickle.dump(cells, target)
    print('meta\tfull_cells\t%d' % len(cells))
    if args.reference:
        selected = {}
        for cx in (-8192, 0, 8192):
            for cz in (-8192, 0, 8192):
                half = 1024 if cx == cz == 0 else 512
                count = 0
                for x in range((cx-half)//4, (cx+half)//4):
                    for z in range((cz-half)//4, (cz+half)//4):
                        if (x, z) in cells:
                            selected[x, z] = cells[x, z]
                            count += 1
                expected = (half//2)**2
                print('window\t%d\t%d\t%d\t%d\t%d' % (cx, cz, half, count, expected))
                if count != expected:
                    raise ValueError('Incomplete reference square: %s' % ((cx, cz, count, expected),))
        cells = selected
    if not cells:
        raise ValueError('No full surface cells found')
    print('meta\tcells\t%d\tblock_area\t%d' % (len(cells), len(cells)*16))
    report(cells)

if __name__ == '__main__':
    main()
