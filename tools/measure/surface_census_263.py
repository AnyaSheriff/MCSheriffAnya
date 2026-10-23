# Верхний блок и опора под растением по биомам — мир официального 26.3, только чтение region.
# python3 -I tools/measure/surface_census_263.py <папка region> биом [биом ...]
import sys, os, glob, math, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import census_263 as c

def pname(p):
    return ((p.get('id') or p.get('') or p.get('Name') or p.get('name')) if isinstance(p, dict) else p).replace('minecraft:', '')

def one(args):
    path, want = args
    out = collections.defaultdict(collections.Counter)
    chunks_n = collections.Counter()
    for root in c.chunks(path):
        hm = root.get('Heightmaps', {}).get('WORLD_SURFACE')
        if not hm:
            continue
        top = c.unpack(hm, 9, 256)
        secs = {s['Y']: s for s in root.get('sections', [])}
        cache = {}
        def block(x, y, z):
            sy = y >> 4
            if sy not in cache:
                s = secs.get(sy)
                bs = s.get('block_states') if s else None
                if not bs:
                    cache[sy] = None
                else:
                    pal = [pname(p) for p in bs['palette']]
                    ids = [0] * 4096 if len(pal) == 1 else c.unpack(bs['data'], max(4, math.ceil(math.log2(len(pal)))), 4096)
                    cache[sy] = (pal, ids)
            e = cache[sy]
            if not e:
                return 'air'
            return e[0][e[1][((y & 15) << 8) | (z << 4) | x]]
        bio = None
        t = top[8 * 16 + 8] - 65
        s = secs.get(t >> 4)
        if s and 'biomes' in s:
            bio = c.biome_cells(s)[(((t & 15) >> 2) * 4 + 2) * 4 + 2].replace('minecraft:', '')
        if bio not in want:
            continue
        chunks_n[bio] += 1
        for z in range(16):
            for x in range(16):
                y = top[z * 16 + x] - 65
                a = block(x, y, z)
                if a in ('air', 'water'):
                    out[bio]['_' + a] += 1
                    continue
                below = block(x, y - 1, z)
                if a in ('short_grass', 'tall_grass', 'fern', 'large_fern', 'leaf_litter', 'red_shrub', 'brown_mushroom',
                         'red_mushroom', 'dead_bush', 'bush', 'sweet_berry_bush', 'poplar_sapling') or 'flower' in a or a in ('dandelion', 'poppy', 'wildflowers', 'short_dry_grass'):
                    out[bio]['plant:' + a] += 1
                    out[bio]['under:' + below] += 1
                else:
                    out[bio]['top:' + a] += 1
    return out, chunks_n

if __name__ == '__main__':
    from multiprocessing import Pool
    want = set(sys.argv[2:])
    files = sorted(glob.glob(os.path.join(sys.argv[1], 'r.*.mca')))
    tot = collections.defaultdict(collections.Counter); nc = collections.Counter()
    with Pool(8) as pool:
        for o, n in pool.map(one, [(f, want) for f in files]):
            nc.update(n)
            for b, cn in o.items():
                tot[b].update(cn)
    for b in sorted(tot):
        print('biome', b, 'chunks', nc[b])
        for k, v in tot[b].most_common(40):
            print('  %-34s %.4f/column  %.2f/chunk' % (k, v / (nc[b] * 256), v / nc[b]))
