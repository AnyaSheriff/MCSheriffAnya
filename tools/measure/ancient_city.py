#!/usr/bin/env python3
# Древние города (ancient city) по замеру мира оригинала 26.1.2. Код игры, jar
# и выгрузки worldgen не используются: сервер оригинала только запускается
# (свой мир, обычный тип, строения включены) и слушает RCON, а всё снимается
# с region-файлов, которые он сложил, — блоки, сущности-блоки (сундуки с
# LootTable) и записи о строениях в чанках (structures.starts
# minecraft:ancient_city: куски Children, у каждого рамка BB).
#
# Найти и сложить — как у испытательных камер:
#   python3 tools/measure/template_structs.py locate <порт> <пароль> ancient_city <радиус> <шаг> <вывод.tsv>
#   python3 tools/measure/trial_chambers.py generate <мир> <порт> <пароль> <x z.tsv> [сколько]
# Дальше:
#   python3 tools/measure/ancient_city.py starts <мир> <x z.tsv>
#       — какие города сложены целиком: старт, рамка, кусков, низ стартового;
#   python3 tools/measure/ancient_city.py templates <мир> <x z.tsv> <вывод.txt> [сколько]
#       — каждый город целиком своим образцом (src/world/ancient_city.txt,
#         формат образцов испытательных камер);
#   python3 tools/measure/ancient_city.py census <мир> <x z.tsv> [сколько]
#       — перепись блоков в рамках кусков (сверка с тестом ancient_city_census).
import os, sys, collections
sys.path.insert(0, os.path.dirname(__file__))
from trial_chambers import World as ChamberWorld, union, inside, code, rle

NAME = 'minecraft:ancient_city'

# Порода и жидкости вокруг города — «.» (остаётся как была). Скалк и его
# датчики, крикуны и катализаторы в кусках — города: оригинал кладёт их и
# образцами, и заменами кусков (вики, Ancient City: «sculk … generate much
# more frequently in ancient cities»).
TERRAIN = ('stone', 'deepslate', 'tuff', 'granite', 'diorite', 'andesite', 'dirt', 'gravel', 'calcite',
           'water', 'lava', 'bedrock', 'smooth_basalt', 'amethyst_block', 'budding_amethyst', 'clay',
           'glow_lichen', 'pointed_dripstone', 'dripstone_block', 'small_amethyst_bud', 'medium_amethyst_bud',
           'large_amethyst_bud', 'amethyst_cluster', 'infested_stone', 'infested_deepslate', 'raw_iron_block',
           'raw_copper_block', 'magma_block', 'obsidian', 'moss_block', 'moss_carpet', 'cave_vines',
           'cave_vines_plant', 'big_dripleaf', 'big_dripleaf_stem', 'small_dripleaf', 'spore_blossom',
           'hanging_roots', 'rooted_dirt', 'azalea', 'flowering_azalea', 'sand', 'sandstone')

def own(state):
    b = state.split('[')[0]
    return not (b in TERRAIN or b.endswith('_ore'))

class World(ChamberWorld):
    def cities(self, starts, count=1000):
        """Города у найденных стартов, сложенные целиком: (чанк x, z, рамка, рамки кусков, куски)."""
        out = []; seen = set()
        for x, z in starts:
            for cx in range((x >> 4) - 1, (x >> 4) + 2):
                for cz in range((z >> 4) - 1, (z >> 4) + 2):
                    root = self.chunk(cx, cz)
                    st = root and root.get('structures', {}).get('starts', {}).get(NAME)
                    if not st or st.get('id') == 'INVALID' or (cx, cz) in seen: continue
                    seen.add((cx, cz))
                    boxes = [c['BB'] for c in st['Children']]
                    bb = union(boxes)
                    if all(self.full(i, j) for i in range(bb[0] >> 4, (bb[3] >> 4) + 1) for j in range(bb[2] >> 4, (bb[5] >> 4) + 1)):
                        out.append((cx, cz, bb, boxes, st['Children']))
                    else:
                        print('не сложен', cx, cz, file=sys.stderr)
        return out[:int(count)]

def read(path):
    return [tuple(map(int, l.split()[:2])) for l in open(path) if l.strip()]

def chests(w, bb, boxes):
    out = []
    for cx in range(bb[0] >> 4, (bb[3] >> 4) + 1):
        for cz in range(bb[2] >> 4, (bb[5] >> 4) + 1):
            for e in w.entities_of(cx, cz):
                x, y, z = e['x'], e['y'], e['z']
                if inside(boxes, x, y, z) and 'LootTable' in e:
                    out.append((x, y, z, e.get('id', '').replace('minecraft:', ''), e['LootTable'].replace('minecraft:', '')))
    return out

def cmd_starts(world, starts):
    w = World(world)
    for cx, cz, bb, boxes, kids in w.cities(read(starts)):
        print('старт', cx, cz, 'рамка', bb, 'кусков', len(boxes), 'низ старта', kids[0]['BB'][1],
              'старт', kids[0].get('pool_element', {}).get('location'), kids[0]['BB'], kids[0].get('rotation'))

def cmd_templates(world, starts, out, count=4):
    w = World(world); cs = w.cities(read(starts), count)
    lines = ['# Древние города — замер мира оригинала 26.1.2 (%d городов),' % len(cs),
             '# tools/measure/ancient_city.py templates. Каждый город — свой образец',
             '# в рамке всех своих кусков: слои снизу вверх от низа рамки, строки — по z,',
             '# знаки — по x, серии сжаты (знак и число повторов). «.» — порода как была',
             '# (вне кусков или порода в куске), «#» — воздух, «~XY» — знак палитры',
             '# за 80-м. Строка chamber: ширина x, глубина z, высота, низ рамки (y),',
             '# сдвиг рамки от чанка старта по x и z (блоки), низ стартового куска. Затем палитра,',
             '# сундуки (e x y z вид таблица — от угла рамки) и слои.', '']
    for cx, cz, bb, boxes, kids in cs:
        W, H, D = bb[3] - bb[0] + 1, bb[4] - bb[1] + 1, bb[5] - bb[2] + 1
        palette = {}; layers = []
        for y in range(H):
            rows = []
            for b in range(D):
                cells = []
                for a in range(W):
                    x, yy, z = bb[0] + a, bb[1] + y, bb[2] + b
                    if not inside(boxes, x, yy, z): cells.append('.'); continue
                    s = w.block(x, yy, z)
                    if s in ('air', 'cave_air'): cells.append('#'); continue
                    if not own(s): cells.append('.'); continue
                    if s not in palette: palette[s] = code(len(palette))
                    cells.append(palette[s])
                rows.append(rle(cells))
            layers.append(rows)
        lines.append('chamber %d %d %d %d %d %d %d' % (W, D, H, bb[1], bb[0] - cx * 16, bb[2] - cz * 16, kids[0]['BB'][1]))
        for s, ch in palette.items(): lines.append('  %s %s' % (ch, s))
        for x, y, z, kind, data in chests(w, bb, boxes):
            lines.append('  e %d %d %d %s %s' % (x - bb[0], y - bb[1], z - bb[2], kind, data))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y); lines.extend('  ' + r for r in rows)
        lines.append('')
        print('город', cx, cz, 'рамка', W, D, H, 'низ', bb[1], 'палитра', len(palette), flush=True)
    open(out, 'w').write('\n'.join(lines))

def cmd_census(world, starts, count=8):
    w = World(world); cs = w.cities(read(starts), count)
    total = collections.Counter(); kinds = collections.Counter(); bottoms = []
    for cx, cz, bb, boxes, kids in cs:
        one = collections.Counter(); states = collections.Counter()
        for x in range(bb[0], bb[3] + 1):
            for z in range(bb[2], bb[5] + 1):
                for y in range(bb[1], bb[4] + 1):
                    if not inside(boxes, x, y, z): continue
                    s = w.block(x, y, z)
                    if own(s): one[s.split('[')[0]] += 1
                    if s.startswith('sculk_shrieker'): states[s] += 1
        for e in chests(w, bb, boxes): kinds[e[4]] += 1
        total.update(one); bottoms.append(kids[0]['BB'][1])
        print('город', cx, cz, 'рамка', bb, 'кусков', len(boxes), ' '.join('%s=%d' % kv for kv in one.most_common(16)), flush=True)
        print('  крикуны:', dict(states))
    n = max(1, len(cs))
    print('низ стартового куска:', bottoms)
    print('всего (%d), среднее на город:' % len(cs), ' '.join('%s=%.1f' % (k, v / n) for k, v in total.most_common()))
    print('сундуки:', sorted(kinds.items()))

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'starts': cmd_starts, 'templates': cmd_templates, 'census': cmd_census}[cmd](*args)
