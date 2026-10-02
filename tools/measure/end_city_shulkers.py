#!/usr/bin/env python3
# Шалкеры энд-городов по замеру мира оригинала 26.1.2: сущности читаются из
# entities/*.mca Энда (id minecraft:shulker, Pos, AttachFace), куски городов —
# из записей о строениях (end_city.py). Кода игры здесь нет.
#
#   python3 tools/measure/end_city_shulkers.py <мир> [мир…]
#
# Для каждого образца печатает места шалкеров в рамке образца (a, y, b — как
# в end_city.py) и долю экземпляров, где шалкер на этом месте есть.
import os, sys, struct, zlib, glob, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt
from end_city import World, to_local

FACES = ['down', 'up', 'north', 'south', 'west', 'east']


def shulkers(world):
    out = []
    for p in sorted(glob.glob(os.path.join(world, 'dimensions/minecraft/the_end/entities/r.*.mca'))):
        data = open(p, 'rb').read()
        for i in range(1024):
            off = int.from_bytes(data[i*4:i*4+3], 'big')
            if not off: continue
            q = off * 4096; ln = struct.unpack_from('>i', data, q)[0]
            try: raw = zlib.decompress(data[q+5:q+4+ln])
            except zlib.error: continue
            n = struct.unpack_from('>H', raw, 1)[0]
            root = nbt(raw, 3 + n, 10)[0]
            for e in root.get('Entities', []):
                if e.get('id') == 'minecraft:shulker':
                    x, y, z = e['Pos']
                    out.append((int(x // 1), int(y // 1), int(z // 1), e.get('AttachFace', 0), e.get('Color', 16), e.get('Peek', 0)))
    return out


def main(worlds):
    seen = collections.Counter(); spots = collections.Counter(); colours = collections.Counter(); peeks = collections.Counter()
    total = 0
    for w in worlds:
        W = World(w)
        cities = W.cities()
        found = shulkers(w)
        total += len(found)
        for _, ch in cities:
            for piece in ch:
                seen[piece['Template']] += 1
                b = piece['BB']
                for x, y, z, face, colour, peek in found:
                    if b[0] <= x <= b[3] and b[1] <= y <= b[4] and b[2] <= z <= b[5]:
                        a, bb = to_local(piece, x, z)
                        spots[(piece['Template'], a, y - piece['TPY'], bb, FACES[face] if face < 6 else face)] += 1
                        colours[colour] += 1; peeks[peek] += 1
    print('шалкеров', total, 'цвета', dict(colours), 'приоткрытость', dict(peeks))
    for key, count in sorted(spots.items()):
        print('%-22s a=%3d y=%3d b=%3d %-5s %4d/%-4d' % (key[0], key[1], key[2], key[3], key[4], count, seen[key[0]]))


if __name__ == '__main__':
    main(sys.argv[1:])
