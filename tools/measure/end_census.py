#!/usr/bin/env python3
# Перепись Энда по region-файлам мира оригинала: для каждого столбца —
# нижняя и верхняя высота эндемняка и сколько его в столбце; по высотам —
# все прочие блоки. Файлы мира только читаются — это данные, которые сложил
# запущенный сервер; код игры не используется.
#
#   python3 tools/measure/end_census.py <папка region> <файл вывода .pkl>
#
# Вывод: {'columns': {(cx, cz): [(нижний, верхний, число)|None]*256},
#         'blocks': {имя: {y: число}}, 'chunks': число}
import os, sys, pickle, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import chunks, unpack

STONE = 'end_stone'

def main():
    folder, out = sys.argv[1], sys.argv[2]
    columns = {}
    blocks = collections.defaultdict(collections.Counter)
    total = 0
    for root in chunks(folder):
        total += 1
        cx, cz = root['xPos'], root['zPos']
        col = [None] * 256
        any_stone = False
        for sec in root.get('sections', []):
            sy = sec.get('Y', 0)
            if isinstance(sy, int) and sy > 127: sy -= 256
            bs = sec.get('block_states')
            if not bs: continue
            pal = [e['Name'].replace('minecraft:', '') for e in bs['palette']]
            if pal == ['air'] or pal == ['void_air'] or pal == ['cave_air']: continue
            if len(pal) == 1:
                idx = [0] * 4096
            else:
                bits = max(4, (len(pal) - 1).bit_length())
                idx = unpack(bs['data'], bits, 4096)
            for i, k in enumerate(idx):
                name = pal[k]
                if name in ('air', 'void_air', 'cave_air'): continue
                y = sy * 16 + (i >> 8)
                blocks[name][y] += 1
                if name == STONE:
                    c = i & 255
                    e = col[c]
                    col[c] = (y, y, 1) if e is None else (min(e[0], y), max(e[1], y), e[2] + 1)
                    any_stone = True
        if any_stone:
            columns[(cx, cz)] = col
    pickle.dump({'columns': columns, 'blocks': {k: dict(v) for k, v in blocks.items()}, 'chunks': total}, open(out, 'wb'))
    print('чанков %d, с эндемняком %d' % (total, len(columns)))

if __name__ == '__main__':
    main()
