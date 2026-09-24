# Ширина рек, глубина и профиль берегов — по сохранённому миру оригинала
# (region-файлы). Читаются только данные мира; jar и код игры не трогаются.
#
# python3 tools/measure/river_census.py <папка region>
#
# Берутся карты высот WORLD_SURFACE и OCEAN_FLOOR и биом у поверхности.
# Вода в столбце — там, где верх выше дна. Строки вдоль X и вдоль Z через
# каждые 64 блока; отрезки, упёршиеся в несгенерированный чанк, не считаются.
# Тем же способом меряет наш тест `river_widths` в src/world/terrain.rs.
import sys, os, glob, struct, zlib, math, collections
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(__file__))
from plants_census import nbt, unpack

SEA = 63
RIVERS = {'minecraft:river', 'minecraft:frozen_river'}

def chunk_columns(f):
    """Для каждого полного чанка: (cx, cz) -> список 256 (река?, верх, дно)."""
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
        if 'WORLD_SURFACE' not in maps or 'OCEAN_FLOOR' not in maps:
            continue
        top = [h - 64 - 1 for h in unpack(maps['WORLD_SURFACE'], 9, 256)]
        floor = [h - 64 - 1 for h in unpack(maps['OCEAN_FLOOR'], 9, 256)]
        sections = {s['Y']: s for s in root.get('sections', []) if 'biomes' in s}
        cols = []
        for index in range(256):
            x, z = index & 15, index >> 4
            y = min(max(floor[index], -64), 319)
            s = sections.get(y >> 4)
            river = False
            if s:
                pal = s['biomes']['palette']
                if len(pal) == 1:
                    name = pal[0]
                else:
                    ids = unpack(s['biomes']['data'], max(1, math.ceil(math.log2(len(pal)))), 64)
                    name = pal[ids[((y & 15) >> 2) * 16 + (z >> 2) * 4 + (x >> 2)]]
                river = name in RIVERS
            cols.append((river, top[index], floor[index]))
        out[(root['xPos'], root['zPos'])] = cols
    return out

def main():
    files = glob.glob(sys.argv[1] + '/*.mca')
    world = {}
    with Pool() as pool:
        for part in pool.imap_unordered(chunk_columns, files):
            world.update(part)

    def column(x, z):
        cols = world.get((x >> 4, z >> 4))
        return None if cols is None else cols[(z & 15) * 16 + (x & 15)]

    xs = [c[0] for c in world]
    zs = [c[1] for c in world]
    lo_x, hi_x, lo_z, hi_z = min(xs) * 16, max(xs) * 16 + 15, min(zs) * 16, max(zs) * 16 + 15
    biome_runs, water_runs, depths = [], [], []
    bank = [[0, 0] for _ in range(65)]

    for along_x in (True, False):
        fixed_range = range(lo_z, hi_z + 1, 64) if along_x else range(lo_x, hi_x + 1, 64)
        for fixed in fixed_range:
            steps = range(lo_x, hi_x + 1) if along_x else range(lo_z, hi_z + 1)
            line = [column(s, fixed) if along_x else column(fixed, s) for s in steps]
            wet = [c is not None and c[0] and c[1] > c[2] and c[1] >= SEA - 1 for c in line]
            biome_run = water_run = 0
            broken_biome = broken_water = False
            for at, c in enumerate(line):
                if c is None:
                    biome_run = water_run = 0
                    broken_biome = broken_water = True
                    continue
                if c[0]:
                    biome_run += 1
                else:
                    if biome_run and not broken_biome:
                        biome_runs.append(biome_run)
                    biome_run, broken_biome = 0, False
                if wet[at]:
                    water_run += 1
                    depths.append(SEA - c[2])
                else:
                    if water_run and not broken_water:
                        water_runs.append(water_run)
                    water_run, broken_water = 0, False
                    if c[1] >= SEA and c[1] == c[2]:
                        for far in range(1, 65):
                            if (at >= far and wet[at - far]) or (at + far < len(wet) and wet[at + far]):
                                bank[far][0] += c[1] - SEA
                                bank[far][1] += 1
                                break

    def show(name, runs):
        runs.sort()
        if not runs:
            print(name, 'нет данных')
            return
        at = lambda part: runs[int((len(runs) - 1) * part)]
        print('%s: %d шт., 10%% %d, медиана %d, 90%% %d, наиб. %d' % (name, len(runs), at(0.1), at(0.5), at(0.9), runs[-1]))

    land = sorted(c[1] for cols in world.values() for c in cols[::17] if c[1] == c[2] and c[1] >= SEA)
    print('чанков', len(world))
    print('суша: 10%% %d, 25%% %d, медиана %d, 75%% %d, 90%% %d' % tuple(land[int((len(land) - 1) * q)] for q in (0.1, 0.25, 0.5, 0.75, 0.9)))
    show('биом реки', biome_runs)
    show('вода в реке', water_runs)
    show('глубина', depths)
    print('берег над морем, 1,2,4,8,16,24,32,48,64 блока от воды:', ' '.join('%.1f' % (s / max(n, 1)) for s, n in (bank[f] for f in (1, 2, 4, 8, 16, 24, 32, 48, 64))))

if __name__ == '__main__':
    main()
