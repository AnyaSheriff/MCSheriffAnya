# Перепись мира официального 26.3: доли биомов (по клеткам 4×4×4 и по поверхности),
# число блоков нового содержимого в каждом биоме, начала лагерей.
# Читаются только region-файлы; jar и код игры не трогаются.
#
# python3 -I tools/measure/census_263.py <папка region> [--camps]
import sys, os, glob, struct, zlib, math, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
def nbt(buf, pos, tag):
    if tag==1: return struct.unpack_from('>b',buf,pos)[0], pos+1
    if tag==2: return struct.unpack_from('>h',buf,pos)[0], pos+2
    if tag==3: return struct.unpack_from('>i',buf,pos)[0], pos+4
    if tag==4: return struct.unpack_from('>q',buf,pos)[0], pos+8
    if tag==5: return struct.unpack_from('>f',buf,pos)[0], pos+4
    if tag==6: return struct.unpack_from('>d',buf,pos)[0], pos+8
    if tag==7:
        n=struct.unpack_from('>i',buf,pos)[0]; return buf[pos+4:pos+4+n], pos+4+n
    if tag==8:
        n=struct.unpack_from('>H',buf,pos)[0]; return buf[pos+2:pos+2+n].decode('utf8','replace'), pos+2+n
    if tag==9:
        t=buf[pos]; n=struct.unpack_from('>i',buf,pos+1)[0]; pos+=5; out=[]
        for _ in range(n):
            v,pos=nbt(buf,pos,t); out.append(v)
        return out,pos
    if tag==10:
        d={}
        while True:
            t=buf[pos]; pos+=1
            if t==0: return d,pos
            n=struct.unpack_from('>H',buf,pos)[0]; name=buf[pos+2:pos+2+n].decode('utf8','replace'); pos+=2+n
            v,pos=nbt(buf,pos,t); d[name]=v
    if tag==11:
        n=struct.unpack_from('>i',buf,pos)[0]; return list(struct.unpack_from('>%di'%n,buf,pos+4)), pos+4+4*n
    if tag==12:
        n=struct.unpack_from('>i',buf,pos)[0]; return list(struct.unpack_from('>%dq'%n,buf,pos+4)), pos+4+8*n
    raise Exception('tag %d'%tag)

def unpack(longs, bits, count):
    per=64//bits; mask=(1<<bits)-1; out=[]
    for i in range(count):
        l=longs[i//per] & 0xFFFFFFFFFFFFFFFF
        out.append((l>>((i%per)*bits))&mask)
    return out


INTEREST = ('sulfur', 'cinnabar', 'poplar', 'shelf_mushroom', 'red_shrub', 'leaf_litter',
            'brown_mushroom', 'spruce_log', 'campfire', 'spike')

def chunks(path):
    data = open(path, 'rb').read()
    if len(data) < 8192:
        return
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
        root, _ = nbt(raw, 3 + n, 10)
        if root.get('Status') in ('minecraft:full', 'full'):
            yield root

def biome_cells(section):
    b = section.get('biomes')
    if not b:
        return None
    pal = b['palette']
    if len(pal) == 1:
        return [pal[0]] * 64
    ids = unpack(b['data'], max(1, math.ceil(math.log2(len(pal)))), 64)
    return [pal[i] for i in ids]

def one(path):
    cells = collections.Counter()          # все клетки 4×4×4
    surface = collections.Counter()        # клетка 4×4 у поверхности
    blocks = collections.defaultdict(collections.Counter)  # биом -> блок -> число
    nchunks = collections.Counter()        # поверхностный биом середины чанка -> чанков
    camps = []
    sulfur = collections.Counter()
    for root in chunks(path):
        for key in root.get('structures', {}).get('starts', {}):
            if 'camp' in key:
                camps.append((key, root['xPos'], root['zPos']))
        hm = root.get('Heightmaps', {}).get('WORLD_SURFACE')
        secs = {s['Y']: s for s in root.get('sections', [])}
        cb = {}
        for y, s in secs.items():
            c = biome_cells(s)
            if c:
                cb[y] = c
                cells.update(c)
        if hm:
            top = unpack(hm, 9, 256)[8*16+8] - 64 - 1
            if top >> 4 in cb:
                nchunks[cb[top >> 4][(((top & 15) >> 2) * 4 + 2) * 4 + 2].replace('minecraft:', '')] += 1
            floor = [h - 64 - 1 for h in unpack(root['Heightmaps']['OCEAN_FLOOR'], 9, 256)] if 'OCEAN_FLOOR' in root['Heightmaps'] else None
            if floor:
                for cz in range(4):
                    for cx in range(4):
                        y = min(max(floor[(cz*4+2)*16 + cx*4+2], -64), 319)
                        if y >> 4 in cb:
                            surface[cb[y >> 4][(((y & 15) >> 2) * 4 + cz) * 4 + cx]] += 1
        if hm:
            wsurf = unpack(hm, 9, 256)
            for y, c in cb.items():
                for i, name in enumerate(c):
                    if name == 'minecraft:sulfur_caves':
                        cx, cz, cy = i & 3, (i >> 2) & 3, i >> 4
                        top = wsurf[(cz * 4 + 2) * 16 + cx * 4 + 2] - 64 - 1
                        sulfur['depth%d' % max(0, min(12, (top - (y * 16 + cy * 4 + 2)) // 16))] += 1
                        sulfur['y%d' % ((y * 16 + cy * 4) // 16)] += 1
                        stop = cb.get(top >> 4)
                        if stop:
                            sulfur['above_' + stop[(((top & 15) >> 2) * 4 + cz) * 4 + cx].replace('minecraft:', '')] += 1
        for y, s in secs.items():
            bs = s.get('block_states')
            if not bs or y not in cb:
                continue
            pal = [((p.get('id') or p.get('') or p.get('Name') or p.get('name')) if isinstance(p, dict) else p).replace('minecraft:', '') for p in bs['palette']]
            if not any(any(k in n for k in INTEREST) for n in pal) or len(pal) < 2:
                continue
            ids = unpack(bs['data'], max(4, math.ceil(math.log2(len(pal)))), 4096)
            for i, pid in enumerate(ids):
                n = pal[pid]
                if any(k in n for k in INTEREST):
                    x, z, yy = i & 15, (i >> 4) & 15, i >> 8
                    blocks[cb[y][((yy >> 2) * 4 + (z >> 2)) * 4 + (x >> 2)].replace('minecraft:', '')][n] += 1
    return cells, surface, blocks, nchunks, camps, sulfur

def main():
    from multiprocessing import Pool
    files = sorted(glob.glob(os.path.join(sys.argv[1], 'r.*.mca')))
    cells, surface, blocks, nch, camps = collections.Counter(), collections.Counter(), collections.defaultdict(collections.Counter), collections.Counter(), []
    sulfurs = collections.Counter()
    with Pool(8) as pool:
        for c, s, b, n, cp, sf in pool.map(one, files):
            sulfurs.update(sf)
            cells.update(c); surface.update(s); nch.update(n); camps += cp
            for k, v in b.items():
                blocks[k].update(v)
    for title, cnt in (('cells3d', cells), ('surface', surface), ('chunks', nch)):
        total = sum(cnt.values())
        for name, v in cnt.most_common():
            print('%s\t%s\t%d\t%.5f' % (title, name.replace('minecraft:', ''), v, v / total))
    for biome, cnt in sorted(blocks.items()):
        for n, v in cnt.most_common():
            print('blocks\t%s\t%s\t%d\t%.2f/chunk' % (biome, n, v, v / max(1, nch.get(biome, 1))))
    for k, v in sorted(sulfurs.items()):
        print('sulfur\t%s\t%d' % (k, v))
    for c in camps:
        print('camp\t%s\t%d\t%d' % c)

if __name__ == '__main__':
    main()
