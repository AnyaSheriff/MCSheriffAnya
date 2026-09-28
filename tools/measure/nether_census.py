#!/usr/bin/env python3
# Перепись Незера по region-файлам мира оригинала: доли блоков на каждой
# высоте, пустоты, биомы. Файлы мира только читаются — это данные, которые
# сложил запущенный сервер; код игры не используется.
#
#   python3 tools/measure/nether_census.py <папка region> [файл вывода]
import os, sys, struct, zlib, collections

def nbt(data, pos, kind):
    if kind == 1: return data[pos], pos + 1
    if kind == 2: return struct.unpack_from('>h', data, pos)[0], pos + 2
    if kind == 3: return struct.unpack_from('>i', data, pos)[0], pos + 4
    if kind == 4: return struct.unpack_from('>q', data, pos)[0], pos + 8
    if kind == 5: return struct.unpack_from('>f', data, pos)[0], pos + 4
    if kind == 6: return struct.unpack_from('>d', data, pos)[0], pos + 8
    if kind == 7:
        n = struct.unpack_from('>i', data, pos)[0]; pos += 4
        return data[pos:pos + n], pos + n
    if kind == 8:
        n = struct.unpack_from('>H', data, pos)[0]; pos += 2
        return data[pos:pos + n].decode('utf8', 'replace'), pos + n
    if kind == 9:
        sub = data[pos]; n = struct.unpack_from('>i', data, pos + 1)[0]; pos += 5
        out = []
        for _ in range(n):
            v, pos = nbt(data, pos, sub); out.append(v)
        return out, pos
    if kind == 10:
        out = {}
        while True:
            t = data[pos]; pos += 1
            if t == 0: return out, pos
            n = struct.unpack_from('>H', data, pos)[0]; pos += 2
            name = data[pos:pos + n].decode('utf8', 'replace'); pos += n
            out[name], pos = nbt(data, pos, t)
    if kind == 11:
        n = struct.unpack_from('>i', data, pos)[0]; pos += 4
        return list(struct.unpack_from('>%di' % n, data, pos)), pos + 4 * n
    if kind == 12:
        n = struct.unpack_from('>i', data, pos)[0]; pos += 4
        return list(struct.unpack_from('>%dq' % n, data, pos)), pos + 8 * n
    raise ValueError('тип %d' % kind)

def unpack(longs, bits, count):
    per = 64 // bits; mask = (1 << bits) - 1; out = []
    for v in longs:
        v &= (1 << 64) - 1
        for i in range(per):
            out.append((v >> (i * bits)) & mask)
            if len(out) == count: return out
    return out

def chunks(folder):
    for name in sorted(os.listdir(folder)):
        if not name.endswith('.mca'): continue
        data = open(os.path.join(folder, name), 'rb').read()
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

def main():
    folder = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else None
    per_y = collections.defaultdict(collections.Counter)
    biomes = collections.Counter()
    total_chunks = 0
    for root in chunks(folder):
        total_chunks += 1
        for sec in root.get('sections', []):
            sy = sec.get('Y', 0)
            if isinstance(sy, int) and sy > 127: sy -= 256
            bs = sec.get('block_states')
            if bs:
                pal = [e['Name'].replace('minecraft:', '') for e in bs['palette']]
                if len(pal) == 1:
                    idx = [0] * 4096
                else:
                    bits = max(4, (len(pal) - 1).bit_length())
                    idx = unpack(bs['data'], bits, 4096)
                for i, k in enumerate(idx):
                    y = sy * 16 + (i >> 8)
                    per_y[y][pal[k]] += 1
            bm = sec.get('biomes')
            if bm:
                pal = [b.replace('minecraft:', '') for b in bm['palette']]
                if len(pal) == 1:
                    biomes[pal[0]] += 64
                else:
                    bits = (len(pal) - 1).bit_length()
                    for k in unpack(bm['data'], bits, 64): biomes[pal[k]] += 1
    print('чанков:', total_chunks)
    tb = sum(biomes.values())
    print('биомы:', ', '.join('%s %.1f%%' % (b, 100 * c / tb) for b, c in biomes.most_common()))
    totals = collections.Counter()
    for y in per_y: totals.update(per_y[y])
    allb = sum(totals.values())
    print('всего блоков по видам (доля от объёма y 0..127):')
    for b, c in totals.most_common(40):
        print('  %-24s %8.4f%%' % (b, 100 * c / allb))
    if out:
        names = [b for b, _ in totals.most_common(60)]
        with open(out, 'w') as f:
            f.write('y\t' + '\t'.join(names) + '\n')
            for y in sorted(per_y):
                t = sum(per_y[y].values()) or 1
                f.write('%d\t' % y + '\t'.join('%.5f' % (per_y[y][b] / t) for b in names) + '\n')
        print('по высотам →', out)

if __name__ == '__main__':
    main()
