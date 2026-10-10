#!/usr/bin/env python3
# Испытательные камеры (trial chambers) по замеру мира оригинала 26.1.2. Код
# игры, jar и выгрузки worldgen не используются: сервер оригинала только
# запускается (свой мир, обычный тип, строения включены) и слушает RCON, а всё
# снимается с region-файлов, которые он сложил, — блоки, сущности-блоки
# (block_entities: настройки испытательных спавнеров, хранилищ, таблицы добычи
# сундуков и горшков) и записи о строениях в чанках (structures.starts
# minecraft:trial_chambers: куски Children, у каждого рамка BB).
#
#   python3 tools/measure/trial_chambers.py locate <порт rcon> <пароль> <радиус> <вывод.tsv>
#       — `locate structure minecraft:trial_chambers` из точек сетки 512 блоков;
#   python3 tools/measure/trial_chambers.py generate <мир> <порт rcon> <пароль> <starts.tsv> [сколько]
#       — догружает (forceload) чанки вокруг камер, ждёт полных;
#   python3 tools/measure/trial_chambers.py starts <мир>
#       — какие камеры сложены целиком: старт, рамка, число кусков;
#   python3 tools/measure/trial_chambers.py templates <мир> <вывод.txt> [сколько]
#       — каждая камера целиком своим образцом (src/world/trial_chambers.txt);
#   python3 tools/measure/trial_chambers.py census <мир> [сколько]
#       — перепись блоков в рамках кусков камер (сверка с тестом
#         trial_chambers_census в src/world/trial_chambers.rs).
import os, sys, re, time, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_fortress import Rcon, state_name
from monument import World as MonumentWorld

class World(MonumentWorld):
    def entities_of(self, cx, cz):
        root = self.chunk(cx, cz)
        return root.get('block_entities', []) if root else []
    def chambers(self, count=1000):
        out = []
        for name in sorted(os.listdir(self.reg)):
            m = re.match(r'r\.(-?\d+)\.(-?\d+)\.mca$', name)
            if not m: continue
            rx, rz = int(m.group(1)), int(m.group(2))
            for i in range(1024):
                cx, cz = rx * 32 + (i & 31), rz * 32 + (i >> 5)
                root = self.chunk(cx, cz)
                st = root and root.get('structures', {}).get('starts', {}).get('minecraft:trial_chambers')
                if not st or st.get('id') == 'INVALID': continue
                boxes = [c['BB'] for c in st['Children']]
                bb = union(boxes)
                if all(self.full(x, z) for x in range(bb[0] >> 4, (bb[3] >> 4) + 1) for z in range(bb[2] >> 4, (bb[5] >> 4) + 1)):
                    out.append((cx, cz, bb, boxes, st['Children']))
        out.sort(key=lambda m: m[0] ** 2 + m[1] ** 2)
        return out[:int(count)]

def union(boxes):
    return [min(b[0] for b in boxes), min(b[1] for b in boxes), min(b[2] for b in boxes),
            max(b[3] for b in boxes), max(b[4] for b in boxes), max(b[5] for b in boxes)]

def inside(boxes, x, y, z):
    return any(b[0] <= x <= b[3] and b[1] <= y <= b[4] and b[2] <= z <= b[5] for b in boxes)

def cmd_locate(port, password, radius, out):
    r = Rcon(int(port), password); radius = int(radius); found = set()
    for x in range(-radius, radius + 1, 512):
        for z in range(-radius, radius + 1, 512):
            m = re.search(r'\[(-?\d+), (-?\d+|~), (-?\d+)\]', r.run('execute positioned %d 0 %d run locate structure minecraft:trial_chambers' % (x, z)))
            if m: found.add((int(m.group(1)), int(m.group(3))))
    with open(out, 'w') as f:
        for x, z in sorted(found): f.write('%d\t%d\n' % (x, z))
    print('камер', len(found))

def cmd_generate(world, port, password, starts, count=1000):
    # Кусок камеры уходит от старта не дальше 116 блоков (вики, Trial
    # Chambers: «max distance from center 116») — ±8 чанков.
    w = World(world); r = Rcon(int(port), password)
    rows = [tuple(map(int, l.split('\t'))) for l in open(starts).read().split('\n') if l]
    rows.sort(key=lambda p: p[0] ** 2 + p[1] ** 2)
    for n, (x, z) in enumerate(rows[:int(count)]):
        cx, cz = x // 16, z // 16
        area = [(i, j) for i in range(cx - 8, cx + 9) for j in range(cz - 8, cz + 9)]
        # forceload берёт не больше 256 чанков за раз — две половины.
        boxes = ['%d %d %d %d' % ((cx - 8) * 16, (cz - 8) * 16, (cx + 8) * 16 + 15, cz * 16 + 15),
                 '%d %d %d %d' % ((cx - 8) * 16, (cz + 1) * 16, (cx + 8) * 16 + 15, (cz + 8) * 16 + 15)]
        if not all(w.full(*c) for c in area):
            for box in boxes: r.run('forceload add ' + box)
            while True:
                time.sleep(5); r.run('save-all'); w.regions.clear(); w.grids.clear()
                if all(w.full(*c) for c in area): break
            for box in boxes: r.run('forceload remove ' + box)
        print(n, x, z, flush=True)
    r.run('save-all')

def cmd_starts(world):
    w = World(world)
    for cx, cz, bb, boxes, kids in w.chambers():
        print('старт', cx, cz, 'рамка', bb, 'кусков', len(boxes), 'низ старта', kids[0]['BB'][1])

# Чем сложена камера: всё, что не порода, руда и не жидкость пещер (перепись
# census: в рамках кусков камеры). Воздух в рамке куска — тоже камера
# (пустоты комнат вырезаны в породе); «.» — порода остаётся как была (там у
# куска пустота образца — structure_void).
TERRAIN = ('stone', 'deepslate', 'tuff', 'granite', 'diorite', 'andesite', 'dirt', 'gravel', 'calcite',
           'water', 'lava', 'bedrock', 'smooth_basalt', 'amethyst_block', 'budding_amethyst', 'clay',
           'glow_lichen', 'sculk', 'sculk_vein', 'moss_block', 'cave_vines', 'cave_vines_plant', 'pointed_dripstone',
           'dripstone_block', 'small_amethyst_bud', 'medium_amethyst_bud', 'large_amethyst_bud', 'amethyst_cluster',
           'infested_stone', 'infested_deepslate', 'raw_iron_block', 'raw_copper_block', 'sculk_sensor', 'sculk_catalyst',
           'sculk_shrieker', 'cobbled_deepslate', 'packed_mud', 'mud', 'sand', 'sandstone', 'red_sand', 'magma_block',
           'obsidian', 'rooted_dirt', 'hanging_roots', 'moss_carpet', 'azalea', 'flowering_azalea', 'spore_blossom',
           'big_dripleaf', 'big_dripleaf_stem', 'small_dripleaf', 'clay', 'mossy_cobblestone', 'cobblestone')
def own(state):
    b = state.split('[')[0]
    return not (b in TERRAIN or b.endswith('_ore'))

# Знаки палитры: одна буква/знак, без цифр (цифры — длина серии), без «.»,
# «#», «~» и «e» (строка сущности-блока).
ALPHABET = [c for c in (chr(i) for i in range(33, 127)) if not c.isdigit() and c not in '.#~e']

def code(i):
    # Номер палитры: один знак до 80, дальше «~» + два знака.
    if i < len(ALPHABET): return ALPHABET[i]
    i -= len(ALPHABET)
    return '~' + ALPHABET[i // len(ALPHABET)] + ALPHABET[i % len(ALPHABET)]

def rle(cells):
    out = []; i = 0
    while i < len(cells):
        j = i
        while j < len(cells) and cells[j] == cells[i]: j += 1
        n = j - i
        out.append(cells[i] + (str(n) if n > 1 else ''))
        i = j
    return ''.join(out)

def entities(w, bb, boxes):
    """Сущности-блоки камеры: (x, y, z, вид) — спавнер с настройками,
    хранилище с таблицей, сундук/горшок/раздатчик с таблицей добычи."""
    out = []
    for cx in range(bb[0] >> 4, (bb[3] >> 4) + 1):
        for cz in range(bb[2] >> 4, (bb[5] >> 4) + 1):
            for e in w.entities_of(cx, cz):
                x, y, z = e['x'], e['y'], e['z']
                if not inside(boxes, x, y, z): continue
                kind = e.get('id', '').replace('minecraft:', '')
                if kind == 'trial_spawner':
                    n = e.get('normal_config', '?'); o = e.get('ominous_config', '?')
                    if isinstance(n, dict) or isinstance(o, dict): n, o = 'inline', 'inline'
                    out.append((x, y, z, 'spawner', n.replace('minecraft:', '') + '|' + o.replace('minecraft:', '')))
                elif kind == 'vault':
                    cfg = e.get('config', {})
                    # Нет loot_table в config — таблица по умолчанию (вики, Vault: «loot_table
                    # … Defaults to chests/trial_chambers/reward»).
                    out.append((x, y, z, 'vault', cfg.get('loot_table', 'chests/trial_chambers/reward').replace('minecraft:', '')))
                elif 'LootTable' in e:
                    out.append((x, y, z, kind, e['LootTable'].replace('minecraft:', '')))
    return out

def cmd_templates(world, out, count=4):
    w = World(world); cs = w.chambers(count)
    lines = ['# Испытательные камеры — замер мира оригинала 26.1.2 (%d камер),' % len(cs),
             '# tools/measure/trial_chambers.py templates. Каждая камера — свой образец',
             '# в рамке всех своих кусков: слои снизу вверх от низа рамки, строки — по z,',
             '# знаки — по x, серии сжаты (знак и число повторов). «.» — порода как была',
             '# (вне кусков или пустота образца), «#» — воздух, «~XY» — знак палитры',
             '# за 80-м. Строка chamber: ширина x, глубина z, высота, низ рамки (y),',
             '# сдвиг рамки от чанка старта по x и z (блоки), низ стартового куска. Затем палитра, сущности-',
             '# блоки (e x y z вид данные — от угла рамки) и слои.', '']
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
        for x, y, z, kind, data in entities(w, bb, boxes):
            lines.append('  e %d %d %d %s %s' % (x - bb[0], y - bb[1], z - bb[2], kind, data))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y); lines.extend('  ' + r for r in rows)
        lines.append('')
        print('камера', cx, cz, 'рамка', W, D, H, 'низ', bb[1], 'палитра', len(palette), flush=True)
    open(out, 'w').write('\n'.join(lines))

def cmd_census(world, count=8):
    w = World(world); cs = w.chambers(count)
    total = collections.Counter(); terrain = collections.Counter(); kinds = collections.Counter(); bottoms = []
    for cx, cz, bb, boxes, kids in cs:
        one = collections.Counter()
        for x in range(bb[0], bb[3] + 1):
            for z in range(bb[2], bb[5] + 1):
                for y in range(bb[1], bb[4] + 1):
                    if not inside(boxes, x, y, z): continue
                    s = w.block(x, y, z).split('[')[0]
                    if own(s): one[s] += 1
                    else: terrain[s] += 1
        for e in entities(w, bb, boxes): kinds[(e[3], e[4])] += 1
        total.update(one); bottoms.append(kids[0]['BB'][1])
        print('камера', cx, cz, 'рамка', bb, 'кусков', len(boxes), ' '.join('%s=%d' % kv for kv in one.most_common(14)), flush=True)
    n = max(1, len(cs))
    print('низ стартового куска:', bottoms)
    print('всего (%d), среднее на камеру:' % len(cs), ' '.join('%s=%.1f' % (k, v / n) for k, v in total.most_common()))
    print('порода в кусках:', terrain.most_common(20))
    print('сущности-блоки:', sorted(kinds.items()))

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'locate': cmd_locate, 'generate': cmd_generate, 'starts': cmd_starts, 'templates': cmd_templates, 'census': cmd_census}[cmd](*args)
