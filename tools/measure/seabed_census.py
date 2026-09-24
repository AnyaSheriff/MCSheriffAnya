# Дно под водой по биомам — по сохранённому миру оригинала (region-файлы):
# глубина и какие блоки лежат сверху дна и на 1..4 ниже. Читаются только
# данные мира; jar и код игры не трогаются.
#
# python3 tools/measure/seabed_census.py <папка region> > vanilla.tsv
#
# Тот же вывод даёт наш тест `seabed_census` в src/world/mod.rs; сравнивает
# их tools/measure/seabed_compare.py.
#
# Столбец водный, если сверху (с высоты 62 вниз) первым идёт вода, лёд или
# водное растение; дно — первый блок ниже них, не жидкость и не растение.
import sys, os, glob, struct, zlib, math, collections
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(__file__))
from plants_census import nbt, unpack

SEA = 63
WET = {'water', 'seagrass', 'tall_seagrass', 'kelp', 'kelp_plant', 'bubble_column', 'sea_pickle'}
TOP = WET | {'ice', 'packed_ice', 'blue_ice', 'lily_pad'}

def is_wet(name):
    return name in WET or name.endswith('coral') or name.endswith('coral_fan')

def census(f):
    depth = collections.defaultdict(list)
    layers = collections.defaultdict(collections.Counter)
    chunks = collections.Counter()
    data = open(f, 'rb').read()
    if len(data) < 8192:
        return depth, layers, chunks
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
        secs = {s['Y']: s for s in root.get('sections', [])}
        cache = {}

        def section(Y):
            if Y not in cache:
                s = secs.get(Y)
                if not s or 'block_states' not in s:
                    cache[Y] = None
                else:
                    bs = s['block_states']
                    pal = [e['Name'].replace('minecraft:', '') for e in bs['palette']]
                    ids = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, math.ceil(math.log2(len(pal)))), 4096)
                    cache[Y] = (pal, ids)
            return cache[Y]

        def block(x, y, z):
            s = section(y >> 4)
            if s is None:
                return 'air'
            pal, ids = s
            return pal[ids[((y & 15) * 16 + z) * 16 + x]]

        def biome(x, y, z):
            s = secs.get(y >> 4)
            if not s or 'biomes' not in s:
                return None
            pal = s['biomes']['palette']
            if len(pal) == 1:
                return pal[0].replace('minecraft:', '')
            ids = unpack(s['biomes']['data'], max(1, math.ceil(math.log2(len(pal)))), 64)
            return pal[ids[((y & 15) >> 2) * 16 + (z >> 2) * 4 + (x >> 2)]].replace('minecraft:', '')

        seen = set()
        for x in range(16):
            for z in range(16):
                y = SEA - 1
                if block(x, y, z) not in TOP and not is_wet(block(x, y, z)):
                    continue
                while y > -60 and (block(x, y, z) in TOP or is_wet(block(x, y, z))):
                    y -= 1
                name = biome(x, y, z)
                if name is None:
                    continue
                if name not in seen:
                    seen.add(name)
                    chunks[name] += 1
                depth[name].append(SEA - 1 - y)
                for k in range(5):
                    layers[(name, k)][block(x, y - k, z)] += 1
    return depth, layers, chunks

def main():
    depth = collections.defaultdict(list)
    layers = collections.defaultdict(collections.Counter)
    chunks = collections.Counter()
    with Pool() as pool:
        for d, l, c in pool.imap_unordered(census, glob.glob(sys.argv[1] + '/*.mca')):
            for k, v in d.items():
                depth[k].extend(v)
            for k, v in l.items():
                layers[k].update(v)
            chunks.update(c)
    for name, values in sorted(depth.items()):
        if len(values) < 2000:
            continue
        values.sort()
        at = lambda part: values[int((len(values) - 1) * part)]
        print('depth\t%s\t%d\t%d\t%d\t%d' % (name, at(0.1), at(0.5), at(0.9), len(values)))
        for k in range(5):
            counter = layers[(name, k)]
            total = sum(counter.values())
            for block, count in counter.most_common(8):
                print('layer\t%s\t%d\t%s\t%.4f' % (name, k, block, count / total))

if __name__ == '__main__':
    main()
