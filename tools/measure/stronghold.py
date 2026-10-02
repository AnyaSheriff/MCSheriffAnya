#!/usr/bin/env python3
# Крепости (strongholds) по замеру мира оригинала 26.1.2. Код игры, jar и
# выгрузки worldgen не используются: сервер оригинала только запускается
# (свой мир, generate-structures=true) и слушает RCON, а всё снимается с
# region-файлов, которые он сложил, — блоки и записи о строениях в чанках
# (structures.starts minecraft:stronghold: вид куска id, рамка BB, сторона O,
# глубина GD и поля самого куска — Left/Right, Type, Tall, Chest, Steps…).
#
#   python3 tools/measure/stronghold.py locate <порт rcon> <пароль> <вывод.tsv>
#       — `locate structure minecraft:stronghold` из точек колец (2 кольца);
#   python3 tools/measure/stronghold.py generate <мир> <порт rcon> <пароль> <starts.tsv>
#       — догружает (forceload) по 17×17 чанков вокруг крепостей;
#   python3 tools/measure/stronghold.py stats <мир> <starts.tsv>
#       — куски на крепость, поля кусков, выходы, кто к ним пристроен;
#   python3 tools/measure/stronghold.py templates <мир> <starts.tsv> <вывод.txt>
#       — блоки каждого вида кусков (с его полями) по всем экземплярам —
#         src/world/stronghold.txt;
#   python3 tools/measure/stronghold.py blocks <мир> <starts.tsv>
#       — перепись блоков в рамках кусков на крепость (сверка с тестом
#         stronghold_census в src/world/stronghold.rs).
#
# Рамка куска та же, что у крепостей Незера (nether_fortress.py): вперёд —
# сторона O (0 юг, 1 запад, 2 север, 3 восток), вбок — +x для O=0, у O=2 и
# O=3 ось вбок зеркальна.
import os, sys, struct, zlib, re, time, math, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack
from nether_fortress import Rcon, state_name, base, dims, to_world, localize, FWD, SYMBOLS

class World:
    def __init__(self, world):
        self.reg = os.path.join(world, 'dimensions/minecraft/overworld/region')
        self.regions = {}; self.grids = {}
    def region(self, rx, rz):
        k = (rx, rz)
        if k not in self.regions:
            p = os.path.join(self.reg, 'r.%d.%d.mca' % k)
            self.regions[k] = open(p, 'rb').read() if os.path.exists(p) else b''
            if len(self.regions) > 12: self.regions.pop(next(iter(self.regions)))
        return self.regions[k]
    def chunk(self, cx, cz):
        data = self.region(cx >> 5, cz >> 5)
        if not data: return None
        i = (cx & 31) + (cz & 31) * 32
        off = int.from_bytes(data[i*4:i*4+3], 'big')
        if off == 0: return None
        q = off * 4096; ln = struct.unpack_from('>i', data, q)[0]
        try: raw = zlib.decompress(data[q+5:q+4+ln])
        except zlib.error: return None
        n = struct.unpack_from('>H', raw, 1)[0]
        return nbt(raw, 3 + n, 10)[0]
    def full(self, cx, cz):
        root = self.chunk(cx, cz)
        return root is not None and root.get('Status') in ('minecraft:full', 'full')
    def blocks(self, cx, cz):
        k = (cx, cz)
        if k not in self.grids:
            root = self.chunk(cx, cz); grid = None
            if root is not None and root.get('Status') in ('minecraft:full', 'full'):
                grid = {}
                for sec in root.get('sections', []):
                    sy = sec.get('Y', 0)
                    if sy > 127: sy -= 256
                    bs = sec.get('block_states')
                    if not -4 <= sy <= 19 or not bs: continue
                    pal = [state_name(e) for e in bs['palette']]
                    idx = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                    for ly in range(16): grid[sy * 16 + ly] = [pal[i] for i in idx[ly * 256:(ly + 1) * 256]]
                grid['be'] = root.get('block_entities', [])
            self.grids[k] = grid
            if len(self.grids) > 600: self.grids.pop(next(iter(self.grids)))
        return self.grids[k]
    def block(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        if g is None or y not in g: return None
        return g[y][(z & 15) * 16 + (x & 15)]
    def entity(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        for e in (g or {}).get('be', []):
            if (e.get('x'), e.get('y'), e.get('z')) == (x, y, z): return e
    def strongholds(self, starts):
        rows = [l.split('\t') for l in open(starts).read().split('\n') if l]
        out = []
        for r in rows:
            x, z = int(r[0]), int(r[1])
            root = self.chunk(x // 16, z // 16)
            st = root and root.get('structures', {}).get('starts', {}).get('minecraft:stronghold')
            if st and st.get('Children'):
                out.append(((x // 16, z // 16), [dict(p, id=p['id'].replace('minecraft:', '')) for p in st['Children']]))
        return out

STD = {'id', 'BB', 'O', 'GD'}
# Поля, от которых образец не зависит: дверь входа, боковые проёмы, сундук,
# спавнер и длина — их кладёт stronghold.rs сам.
LOOSE = {'EntryDoor', 'Left', 'Right', 'leftLow', 'leftHigh', 'rightLow', 'rightHigh', 'Chest', 'Mob', 'Source', 'Steps'}
def variant(p):
    """Поля куска, от которых зависит образец; у поворотов — ещё и
    зеркальна ли рамка (m=1 при O 2 и 3)."""
    out = tuple(sorted((k, v) for k, v in p.items() if k not in STD and k not in LOOSE))
    if p['id'] in ('shlt', 'shrt'): out += (('m', int(p['O'] >= 2)),)
    if p['id'] == 'shfc': out += (('Steps', p['Steps']),)
    return out

# ---------- locate / generate ----------
def cmd_locate(port, password, out):
    r = Rcon(int(port), password); found = {}
    for ring, dist in ((1, 2000), (2, 5100), (3, 8200)):
        for i in range(36):
            a = i * math.pi * 2 / 36
            x, z = int(math.cos(a) * dist), int(math.sin(a) * dist)
            m = re.search(r'\[(-?\d+), ~, (-?\d+)\]', r.run('execute positioned %d 64 %d run locate structure minecraft:stronghold' % (x, z)))
            if m: found[(int(m.group(1)), int(m.group(2)))] = 1
    with open(out, 'w') as f:
        for x, z in sorted(found, key=lambda p: p[0]**2 + p[1]**2):
            f.write('%d\t%d\t%.0f\t%.1f\n' % (x, z, math.hypot(x, z), math.degrees(math.atan2(z, x))))
    print(open(out).read())

def cmd_generate(world, port, password, starts, limit=99):
    """Догружает чанки под рамками кусков (рамки берутся из записи о
    строении в чанке старта, её кладёт уже `locate`); forceload — не больше
    256 чанков за раз, поэтому полосами."""
    w = World(world); r = Rcon(int(port), password)
    rows = [l.split('\t') for l in open(starts).read().split('\n') if l][:int(limit)]
    for n, row in enumerate(rows):
        x, z = int(row[0]), int(row[1]); cx, cz = x // 16, z // 16
        if w.chunk(cx, cz) is None:
            r.run('forceload add %d %d' % (x, z)); time.sleep(3); r.run('save-all'); w.regions.clear(); r.run('forceload remove %d %d' % (x, z))
        st = (w.chunk(cx, cz) or {}).get('structures', {}).get('starts', {}).get('minecraft:stronghold', {})
        bbs = [p['BB'] for p in st.get('Children', [])] or [[x, 0, z, x, 0, z]]
        x0, z0 = min(b[0] for b in bbs) // 16, min(b[2] for b in bbs) // 16
        x1, z1 = max(b[3] for b in bbs) // 16, max(b[5] for b in bbs) // 16
        step = max(1, 256 // (z1 - z0 + 1))
        for sx in range(x0, x1 + 1, step):
            ex = min(x1, sx + step - 1)
            area = [(i, j) for i in range(sx, ex + 1) for j in range(z0, z1 + 1)]
            if all(w.full(*c) for c in area): continue
            box = '%d %d %d %d' % (sx * 16, z0 * 16, ex * 16 + 15, z1 * 16 + 15)
            print('  ', r.run('forceload add ' + box), flush=True)
            while True:
                time.sleep(4); r.run('save-all'); w.regions.clear()
                if all(w.full(*c) for c in area): break
            r.run('forceload remove ' + box)
        print(n, x, z, len(bbs), flush=True)
    r.run('save-all')

# ---------- раскладка ----------
def cmd_stats(world, starts):
    w = World(world); hs = w.strongholds(starts)
    print('крепостей', len(hs))
    per = collections.defaultdict(list); fields = collections.defaultdict(collections.Counter)
    sizes = {}; depth = collections.Counter(); reach = []; start_y = []; portal_gd = []; order = collections.Counter()
    for (cx, cz), ch in hs:
        c = collections.Counter(p['id'] for p in ch)
        for k in set(p['id'] for _, q in hs for p in q): per[k].append(c[k])
        x0, z0 = (ch[0]['BB'][0] + ch[0]['BB'][3]) / 2, (ch[0]['BB'][2] + ch[0]['BB'][5]) / 2
        reach.append(max(max(abs(p['BB'][0] - x0), abs(p['BB'][3] - x0), abs(p['BB'][2] - z0), abs(p['BB'][5] - z0)) for p in ch))
        start_y.append((ch[0]['BB'][1], ch[0]['BB'][0] - cx * 16, ch[0]['BB'][2] - cz * 16, ch[0]['O']))
        for i, p in enumerate(ch):
            depth[p['GD']] += 1
            if p['id'] == 'shpr': portal_gd.append((p['GD'], i, len(ch)))
            fields[p['id']][variant(p)] += 1
            W, D = dims(p['O'], p['BB']); H = p['BB'][4] - p['BB'][1] + 1
            sizes.setdefault(p['id'], collections.Counter())[(W, D, H)] += 1
        order[tuple(p['id'] for p in ch[:3])] += 1
    print('кусков на крепость (среднее, наименьшее, наибольшее):')
    for k, v in sorted(per.items()): print('  %-8s %6.2f %3d %3d' % (k, sum(v) / len(v), min(v), max(v)))
    print('всего кусков: среднее %.1f' % (sum(sum(v) for v in zip(*per.values())) / len(hs)))
    print('размеры (a, b, y):'); [print('  ', k, dict(v)) for k, v in sorted(sizes.items())]
    print('поля:'); [print('  ', k, v.most_common(12)) for k, v in sorted(fields.items())]
    print('глубины:', sorted(depth.items()))
    print('портальная комната (GD, номер, всего):', portal_gd)
    print('старт (низ, сдвиг x, z в чанке, O):', start_y)
    print('размах от середины старта:', reach)
    print('первые куски:', order.most_common())

def cmd_dump(world, starts, n=0):
    w = World(world); hs = w.strongholds(starts)
    for p in hs[int(n)][1]: print(p)

# ---------- образцы ----------
NATURAL = {'stone', 'deepslate', 'andesite', 'diorite', 'granite', 'tuff', 'dirt', 'gravel', 'coal_ore', 'iron_ore', 'copper_ore',
           'deepslate_iron_ore', 'deepslate_coal_ore', 'deepslate_copper_ore', 'gold_ore', 'deepslate_gold_ore', 'redstone_ore',
           'deepslate_redstone_ore', 'lapis_ore', 'deepslate_lapis_ore', 'diamond_ore', 'deepslate_diamond_ore', 'emerald_ore',
           'calcite', 'dripstone_block', 'pointed_dripstone', 'glow_lichen', 'sand', 'sandstone', 'clay',
           'grass_block', 'moss_block', 'smooth_basalt', 'amethyst_block', 'budding_amethyst', 'raw_iron_block', 'raw_copper_block',
           'infested_stone', 'infested_deepslate', 'bedrock', 'magma_block', 'obsidian', 'cave_vines', 'cave_vines_plant',
           'moss_carpet', 'sculk', 'sculk_vein', 'snow_block', 'packed_ice', 'ice', 'powder_snow', 'mud', 'clay', 'terracotta'}
# Камень крепости: обычный, замшелый, треснутый, с чешуйницей — одна клетка,
# вид выбирается случайно при постройке.
BRICK = {'stone_bricks', 'mossy_stone_bricks', 'cracked_stone_bricks', 'infested_stone_bricks'}
def classify(counter):
    """'?' — рельеф как есть, 'air' — всегда пусто, '#' — камень крепости, иначе блок."""
    tot = sum(counter.values()); merged = collections.Counter()
    for s, k in counter.items():
        b = base(s)
        merged['air' if b in ('air', 'cave_air') else '#' if b in BRICK else b] += k
    top, k = merged.most_common(1)[0]
    if top == 'air': return 'air' if k / tot >= 0.75 else '?'
    if top == '#': return '#' if k / tot >= 0.5 else '?'
    if top in NATURAL or k / tot < 0.5: return '?'
    return collections.Counter({s: n for s, n in counter.items() if base(s) == top}).most_common(1)[0][0]

def cmd_templates(world, starts, out):
    w = World(world); hs = w.strongholds(starts)
    by = collections.defaultdict(list)
    for _, ch in hs:
        for p in ch: by[(p['id'],) + variant(p)].append(p)
    lines = ['# Куски крепостей (strongholds) — замер мира оригинала 26.1.2 (%d крепостей),' % len(hs),
             '# tools/measure/stronghold.py templates. Рамка куска: a — вбок, b — вперёд',
             '# от входа, y — от низа. Направления в состояниях — как у куска, смотрящего',
             '# на юг: вперёд — south, вбок (+a) — east. «.» — рельеф остаётся, «_» — пусто,',
             '# «#» — камень крепости (обычный, замшелый, треснутый или с чешуйницей —',
             '# выбирается при постройке). После piece — вид, поля куска (как в записи',
             '# о строении), размеры a b y и число экземпляров.', '']
    mix = collections.Counter()
    for key in sorted(by, key=str):
        ps = by[key]; o0, bb0 = ps[0]['O'], ps[0]['BB']
        W, D = dims(o0, bb0); H = bb0[4] - bb0[1] + 1
        cells = collections.defaultdict(collections.Counter); n = 0
        for p in ps:
            o, bb = p['O'], p['BB']
            if dims(o, bb) != (W, D) or bb[4] - bb[1] + 1 != H: continue
            grid = {}; ok = True
            for a in range(W):
                for b in range(D):
                    x, z = to_world(o, bb, a, b)
                    for y in range(H):
                        s = w.block(x, bb[1] + y, z)
                        if s is None: ok = False; break
                        grid[(a, y, b)] = localize(s, o)
                        if base(s) in BRICK: mix[base(s)] += 1
                    if not ok: break
                if not ok: break
            if not ok: continue
            n += 1
            for k, v in grid.items(): cells[k][v] += 1
        if n == 0: continue
        palette = {}; layers = []
        for y in range(H):
            rows = []
            for b in range(D):
                row = ''
                for a in range(W):
                    c = classify(cells[(a, y, b)])
                    row += '.' if c == '?' else '_' if c == 'air' else '#' if c == '#' else palette.setdefault(c, SYMBOLS[len(palette)])
                rows.append(row)
            layers.append(rows)
        fields = ' '.join('%s=%s' % kv for kv in key[1:])
        lines.append('piece %s %d %d %d %d%s' % (key[0], W, D, H, n, (' ' + fields) if fields else ''))
        for s, ch in palette.items(): lines.append('  %s %s' % (ch, s))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y)
            lines.extend('  ' + r for r in rows)
        lines.append('')
        print(key, 'экземпляров', n, 'размер', W, D, H)
    tot = sum(mix.values())
    print('камень:', [(k, round(v / tot, 3)) for k, v in mix.most_common()])
    open(out, 'w').write('\n'.join(lines))

def cmd_blocks(world, starts):
    """Блоки в рамках кусков (без повторов клеток) — на крепость; рамки
    портальных рамок — глаза и стороны; спавнер и сундуки."""
    w = World(world); hs = w.strongholds(starts); total = collections.Counter(); eyes = collections.Counter()
    for _, ch in hs:
        seen = set()
        for p in ch:
            b = p['BB']
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1):
                    for y in range(b[1], b[4] + 1):
                        if (x, y, z) in seen: continue
                        seen.add((x, y, z)); s = w.block(x, y, z)
                        if s is None: continue
                        total[base(s)] += 1
                        if base(s) == 'end_portal_frame': eyes['eye=true' in s] += 1
                        if base(s) == 'spawner':
                            e = w.entity(x, y, z); total['spawner:' + str((e or {}).get('SpawnData', {}).get('entity', {}).get('id'))] += 1
                        if base(s) == 'chest':
                            e = w.entity(x, y, z); total['chest:' + str((e or {}).get('LootTable'))] += 1
    print('крепостей', len(hs))
    keep = ('stone_bricks', 'mossy_stone_bricks', 'cracked_stone_bricks', 'infested_stone_bricks', 'end_portal_frame', 'spawner',
            'chest', 'bookshelf', 'cobweb', 'iron_bars', 'oak_door', 'iron_door', 'stone_button', 'torch', 'wall_torch', 'ladder',
            'oak_planks', 'oak_fence', 'stone_brick_stairs', 'cobblestone_stairs', 'stone_slab', 'smooth_stone_slab', 'stone_brick_slab',
            'water', 'lava', 'cobblestone', 'oak_slab', 'air', 'cave_air', 'end_portal')
    for k, v in total.most_common():
        if base(k.split(':')[0]) in keep: print('  %-50s %8.1f' % (k, v / len(hs)))
    print('рамки с оком / без:', eyes[True], eyes[False])

def to_frame(o, bb, x, z):
    from nether_fortress import to_local
    return to_local(o, bb, x, z)

def links(hs):
    """Кто к кому пристроен: (родитель, сторона ребёнка относительно
    родителя, ребёнок) → углы ребёнка (a=0, b=0, низ) в рамке родителя.
    Родитель — кусок на глубину выше, чью рамку задевает клетка перед
    входом ребёнка."""
    out = collections.defaultdict(collections.Counter)
    TURN = {0: 'F', 1: '+a', 3: '-a', 2: 'B'}
    for _, ch in hs:
        for c in ch[1:]:
            W, D = dims(c['O'], c['BB'])
            for p in ch:
                if p['GD'] != c['GD'] - 1: continue
                bb = p['BB']; hit = None
                for a in range(W):
                    x, z = to_world(c['O'], c['BB'], a, -1)
                    if bb[0] <= x <= bb[3] and bb[2] <= z <= bb[5] and bb[1] <= c['BB'][1] + 1 <= bb[4]:
                        hit = a; break
                if hit is None: continue
                x, z = to_world(c['O'], c['BB'], 0, 0)
                a, b = to_frame(p['O'], p['BB'], x, z)
                from nether_fortress import lateral
                F, L = FWD[p['O']], lateral(p['O']); cf = FWD[c['O']]
                rel = {(F[0], F[1]): 'F', (-F[0], -F[1]): 'B', (L[0], L[1]): '+a', (-L[0], -L[1]): '-a'}[cf]
                cl = lateral(c['O'])
                flip = (cl[0] * L[0] + cl[1] * L[1] + cl[0] * F[0] + cl[1] * F[1])
                out[(p['id'], rel, p['O'] if p['id'] in ('shlt', 'shrt') else '')][(c['id'], a, c['BB'][1] - p['BB'][1], b, flip)] += 1
                break
    return out

def cmd_links(world, starts):
    w = World(world); hs = w.strongholds(starts)
    for k, v in sorted(links(hs).items()): print(k, sorted(v.items()))

# Середина входа куска по a: у кусков шириной 5 и у тюрьмы — 2, у широких — 5.
DOOR_MID = {'shrc': 5, 'shli': 5, 'shpr': 5, 'sh5c': 5}
def door_mid(pid): return DOOR_MID.get(pid, 2)
def door_floor(w, hs):
    """Пол входа по y: под первой пустой клеткой середины входа (по
    экземплярам с проёмом без двери)."""
    got = collections.defaultdict(collections.Counter)
    for _, ch in hs:
        for p in ch:
            if p.get('EntryDoor') != 'OPENING': continue
            x, z = to_world(p['O'], p['BB'], door_mid(p['id']), 0)
            for y in range(p['BB'][1], p['BB'][4] + 1):
                if base(w.block(x, y, z) or 'stone') in ('air', 'cave_air'):
                    got[p['id']][y - 1 - p['BB'][1]] += 1; break
    return {k: v.most_common(1)[0][0] for k, v in got.items()}

def cmd_exits(world, starts):
    from nether_fortress import lateral, to_local
    w = World(world); hs = w.strongholds(starts); floor = door_floor(w, hs)
    print('пол входа:', floor)
    out = collections.defaultdict(collections.Counter)
    for _, ch in hs:
        for c in ch[1:]:
            W, D = dims(c['O'], c['BB'])
            for p in ch:
                if p['GD'] != c['GD'] - 1: continue
                bb = p['BB']
                x, z = to_world(c['O'], c['BB'], door_mid(c['id']), -1)
                if not (bb[0] <= x <= bb[3] and bb[2] <= z <= bb[5]): continue
                x, z = to_world(c['O'], c['BB'], door_mid(c['id']), 0)
                a, b = to_local(p['O'], p['BB'], x, z)
                F, L = FWD[p['O']], lateral(p['O']); cf = FWD[c['O']]
                rel = {(F[0], F[1]): 'F', (-F[0], -F[1]): 'B', (L[0], L[1]): '+a', (-L[0], -L[1]): '-a'}[cf]
                y = c['BB'][1] + floor.get(c['id'], 0) - p['BB'][1]
                key = p['id'] + ('/m' if p['id'] in ('shlt', 'shrt') and p['O'] >= 2 else '')
                out[key][(a, y, b, rel)] += 1
                break
    for k, v in sorted(out.items()): print(k, sorted(v.items()))

if __name__ == '__main__':
    what, args = sys.argv[1], sys.argv[2:]
    {'locate': cmd_locate, 'generate': cmd_generate, 'stats': cmd_stats, 'templates': cmd_templates, 'blocks': cmd_blocks,
     'dump': cmd_dump, 'links': cmd_links, 'exits': cmd_exits}[what](*args)
