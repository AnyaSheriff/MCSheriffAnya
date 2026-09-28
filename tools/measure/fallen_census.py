# Сколько поваленных деревьев на чанк в каждом биоме — по сохранённому миру
# оригинала (region-файлы). Читаются только данные мира; jar и код игры не
# трогаются.
#
# Правило «лежачего бревна»: блок бревна с axis=x или axis=z (горизонтальный),
# у которого блок НЕПОСРЕДСТВЕННО ПОД НИМ — не воздух, не жидкость, не листва
# и не бревно любой породы. То есть под бревном должно быть что-то «земляное»
# (трава, земля, камень, песок, мох...) или невысокое растение/цветок, которое
# само стоит на земле, — это тоже считается опорой. Это правило само отсеивает
# горизонтальные ветки в кроне раскидистых и тёмных дубов: под веткой в кроне
# почти всегда воздух или листва, а не земля.
#
# Одно поваленное дерево = связная группа (26-связность — включая диагонали,
# чтобы не терять переломы ствола на неровном рельефе) лежачих брёвен ОДНОЙ
# породы. Дерево относится к тому биому, в котором лежит большинство его
# брёвен (для стволов, пересекающих границу биома).
#
# python3 tools/measure/fallen_census.py <папка region> [--tsv]
# --tsv: построчно «биом<TAB>чанков<TAB>порода<TAB>деревьев/чанк<TAB>средняя_длина».
import sys, os, struct, zlib, glob, collections, math, multiprocessing

sys.path.insert(0, os.path.dirname(__file__))
from plants_census import nbt, unpack

AIR = {'air', 'cave_air', 'void_air'}
LIQUID = {'water', 'lava'}


def is_ground_support(name):
    if name in AIR or name in LIQUID:
        return False
    if name.endswith('_leaves') or name.endswith('_log'):
        return False
    return True


def decode_section(sec, cache):
    Y = sec['Y']
    if Y in cache:
        return cache[Y]
    bs = sec.get('block_states')
    if not bs:
        cache[Y] = None
        return None
    pal = bs['palette']
    names = [e['Name'].replace('minecraft:', '') for e in pal]
    axes = [e.get('Properties', {}).get('axis', 'y') for e in pal]
    if len(pal) == 1:
        ids = [0] * 4096
    else:
        ids = unpack(bs['data'], max(4, math.ceil(math.log2(len(pal)))), 4096)
    cache[Y] = (names, axes, ids)
    return cache[Y]


def block_below(sections, cache, gy, lx, lz):
    """Имя блока по абсолютному gy и локальным lx,lz в том же чанке."""
    Y = gy >> 4
    ly = gy & 15
    sec = sections.get(Y)
    if not sec:
        return 'air'
    dec = decode_section(sec, cache)
    if dec is None:
        return 'air'
    names, axes, ids = dec
    idx = (ly << 8) | (lz << 4) | lx
    return names[ids[idx]]


def census(f):
    chunk_biomes = collections.Counter()
    blocks = []  # (x, y, z, порода, биом чанка)
    data = open(f, 'rb').read()
    if len(data) < 8192:
        return chunk_biomes, blocks
    for i in range(1024):
        off = int.from_bytes(data[i * 4:i * 4 + 3], 'big')
        if off == 0:
            continue
        p = off * 4096
        ln = struct.unpack_from('>i', data, p)[0]
        if data[p + 4] != 2:
            continue
        try:
            raw = zlib.decompress(data[p + 5:p + 4 + ln])
        except zlib.error:
            continue
        n = struct.unpack_from('>H', raw, 1)[0]
        root, _ = nbt(raw, 3 + n, 10)
        if root.get('Status') not in ('minecraft:full', 'full'):
            continue
        hm = root.get('Heightmaps', {}).get('WORLD_SURFACE')
        if not hm:
            continue
        top = unpack(hm, 9, 256)[8 * 16 + 8] - 64 - 1
        sections = {s['Y']: s for s in root.get('sections', [])}
        bsec = sections.get(top // 16)
        if not bsec or 'biomes' not in bsec:
            continue
        b = bsec['biomes']
        pal = b['palette']
        ids = [0] * 64 if len(pal) == 1 else unpack(b['data'], max(1, math.ceil(math.log2(len(pal)))), 64)
        cell = ((top % 16) // 4) * 16 + 2 * 4 + 2
        biome = pal[ids[cell]].replace('minecraft:', '')
        chunk_biomes[biome] += 1

        cx = root.get('xPos')
        cz = root.get('zPos')
        if cx is None or cz is None:
            continue
        cache = {}
        for Y, sec in sections.items():
            bs = sec.get('block_states')
            if not bs:
                continue
            pal2 = bs['palette']
            if not any(
                e['Name'].replace('minecraft:', '').endswith('_log') and
                e.get('Properties', {}).get('axis', 'y') in ('x', 'z')
                for e in pal2
            ):
                continue
            dec = decode_section(sec, cache)
            names, axes, ids2 = dec
            for idx in range(4096):
                v = ids2[idx]
                name = names[v]
                if not name.endswith('_log') or axes[v] not in ('x', 'z'):
                    continue
                ly = idx >> 8
                lz = (idx >> 4) & 15
                lx = idx & 15
                gy = Y * 16 + ly
                below = block_below(sections, cache, gy - 1, lx, lz)
                if not is_ground_support(below):
                    continue
                species = name[:-4]
                gx = cx * 16 + lx
                gz = cz * 16 + lz
                blocks.append((gx, gy, gz, species, biome))
    return chunk_biomes, blocks


def main():
    reg = sys.argv[1]
    tsv = '--tsv' in sys.argv[2:]

    total_chunks = collections.Counter()
    all_blocks = []
    with multiprocessing.Pool() as pool:
        for cb, blocks in pool.imap_unordered(census, glob.glob(reg + '/*.mca')):
            total_chunks.update(cb)
            all_blocks.extend(blocks)

    pos_index = {(x, y, z): i for i, (x, y, z, sp, bi) in enumerate(all_blocks)}
    n = len(all_blocks)
    parent = list(range(n))

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    def union(a, b):
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[ra] = rb

    offsets = [(dx, dy, dz) for dx in (-1, 0, 1) for dy in (-1, 0, 1) for dz in (-1, 0, 1) if (dx, dy, dz) != (0, 0, 0)]
    for i, (x, y, z, species, biome) in enumerate(all_blocks):
        for dx, dy, dz in offsets:
            j = pos_index.get((x + dx, y + dy, z + dz))
            if j is not None and all_blocks[j][3] == species:
                union(i, j)

    groups = collections.defaultdict(list)
    for i in range(n):
        groups[find(i)].append(i)

    trees_per_biome_species = collections.Counter()
    lengths_per_biome = collections.defaultdict(list)
    lengths_per_biome_species = collections.defaultdict(list)
    for members in groups.values():
        species = all_blocks[members[0]][3]
        biome_counts = collections.Counter(all_blocks[m][4] for m in members)
        biome = biome_counts.most_common(1)[0][0]
        trees_per_biome_species[(biome, species)] += 1
        lengths_per_biome[biome].append(len(members))
        lengths_per_biome_species[(biome, species)].append(len(members))

    for biome, n_chunks in total_chunks.most_common():
        if n_chunks < 30:
            continue
        species_here = {sp: c for (bi, sp), c in trees_per_biome_species.items() if bi == biome}
        lens = lengths_per_biome.get(biome, [])
        if not species_here:
            continue
        avg_len = sum(lens) / len(lens) if lens else 0.0
        if tsv:
            for sp, c in sorted(species_here.items()):
                slens = lengths_per_biome_species[(biome, sp)]
                print('%s\t%d\t%s\t%.4f\t%.2f' % (biome, n_chunks, sp, c / n_chunks, sum(slens) / len(slens)))
            continue
        total_trees = sum(species_here.values())
        parts = ', '.join('%s %.3f' % (sp, c / n_chunks) for sp, c in sorted(species_here.items(), key=lambda kv: -kv[1]))
        print('%s (%d чанков, %.3f деревьев/чанк, ствол %.1f блоков): %s' %
              (biome, n_chunks, total_trees / n_chunks, avg_len, parts))


if __name__ == '__main__':
    main()
