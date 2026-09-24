# Соседство биомов по сохранённому миру оригинала (region-файлы): доли,
# размеры пятен и с кем граничит каждый биом. Читаются только данные мира.
#
# python3 tools/measure/biome_census.py <папка region> > vanilla.tsv
#
# Биом берётся у поверхности, по клетке 4×4 (как хранит игра). Тот же вывод
# даёт наш тест `biome_neighbours` в src/world/terrain.rs — их сравнивает
# tools/measure/biome_compare.py.
import sys, os, glob, struct, zlib, math, collections
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
        print('share\t%s\t%.4f' % (name, count / total))

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
    cells = {}
    with Pool() as pool:
        for part in pool.imap_unordered(chunk_cells, glob.glob(sys.argv[1] + '/*.mca')):
            cells.update(part)
    report(cells)

if __name__ == '__main__':
    main()
