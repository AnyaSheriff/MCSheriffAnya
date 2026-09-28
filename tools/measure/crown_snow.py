# Снег на кронах у оригинала: в столбцах, где верхний блок — листва,
# лежит ли на ней снег; и что под кроной на земле (снег, заснеженная трава).
# Только данные мира (region-файлы).
import sys, struct, zlib, glob, collections, math
sys.path.insert(0, '/home/lenox/.cache/mcsheriffanya/work/fix-gen/tools/measure')
sys.argv = [sys.argv[0], sys.argv[1]] + sys.argv[2:]
from plants_census import nbt, unpack

SNOWY = {'snowy_taiga', 'snowy_plains', 'grove', 'snowy_slopes', 'frozen_river', 'ice_spikes'}

def column_biome(sections, y, x, z):
    s = sections.get(y // 16)
    if not s or 'biomes' not in s: return None
    b = s['biomes']; pal = b['palette']
    ids = [0]*64 if len(pal) == 1 else unpack(b['data'], max(1, math.ceil(math.log2(len(pal)))), 64)
    return pal[ids[((y % 16)//4)*16 + (z//4)*4 + x//4]].replace('minecraft:', '')

def census(f):
    out = collections.Counter()
    data = open(f, 'rb').read()
    if len(data) < 8192: return out
    for i in range(1024):
        off = int.from_bytes(data[i*4:i*4+3], 'big')
        if off == 0: continue
        p = off*4096; ln = struct.unpack_from('>i', data, p)[0]
        if data[p+4] != 2: continue
        raw = zlib.decompress(data[p+5:p+4+ln]); n = struct.unpack_from('>H', raw, 1)[0]
        root, _ = nbt(raw, 3+n, 10)
        if root.get('Status') not in ('minecraft:full', 'full'): continue
        sections = {}
        for s in root.get('sections', []):
            if 'block_states' not in s: continue
            bs = s['block_states']; pal = bs['palette']
            names = []
            for e in pal:
                nm = e['Name'].replace('minecraft:', '')
                if nm == 'grass_block' and e.get('Properties', {}).get('snowy') == 'true': nm = 'grass_block[snowy]'
                names.append(nm)
            ids = [0]*4096 if len(pal) == 1 else unpack(bs['data'], max(4, math.ceil(math.log2(len(pal)))), 4096)
            sections[s['Y']] = (names, ids, s)
        bsec = {Y: v[2] for Y, v in sections.items()}
        def at(x, y, z):
            s = sections.get(y // 16)
            if not s: return 'air'
            names, ids, _ = s
            return names[ids[((y % 16)*16 + z)*16 + x]]
        top_y = max(sections) * 16 + 15 if sections else 0
        for x in range(16):
            for z in range(16):
                y = top_y
                while y > -64 and at(x, y, z) in ('air', 'cave_air'): y -= 1
                top = at(x, y, z)
                under_snow = top == 'snow'
                if under_snow:
                    y -= 1; top = at(x, y, z)
                if not top.endswith('_leaves'): continue
                biome = column_biome(bsec, y, x, z)
                if biome not in SNOWY: continue
                out[(biome, 'крона', 'снег сверху' if under_snow else 'без снега')] += 1
                # вниз сквозь крону до земли
                g = y - 1
                while g > -64 and (at(x, g, z).endswith('_leaves') or at(x, g, z).endswith('_log') or at(x, g, z) in ('air', 'vine')): g -= 1
                ground = at(x, g, z)
                out[(biome, 'под кроной', ground)] += 1
    return out

if __name__ == '__main__':
    import multiprocessing
    total = collections.Counter()
    files = glob.glob(sys.argv[1] + '/*.mca')
    with multiprocessing.Pool(6) as pool:
        for c in pool.imap_unordered(census, files):
            total.update(c)
    for k, v in sorted(total.items(), key=lambda kv: (kv[0][0], kv[0][1], -kv[1])):
        if v >= 20: print(k, v)
