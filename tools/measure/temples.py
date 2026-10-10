#!/usr/bin/env python3
# Храмы пустыни и джунглей, хижина ведьмы и иглу по замеру мира оригинала
# 26.1.2. Код игры, jar и выгрузки worldgen не используются: сервер
# оригинала только складывал мир (свой, строения включены) и слушал RCON
# (`locate structure`, `forceload`), а всё снимается с region-файлов —
# блоки, записи о строениях (structures.starts: куски tedp, tejp, tesh,
# iglu с рамкой BB и стороной O) и сущности (entities/*.mca).
#
#   python3 tools/measure/temples.py templates <мир> <вывод.txt>
#       — образцы src/world/temples.txt: каждый вид сведён в рамку куска
#         (a — вбок, b — вперёд, как у nether_fortress.py), клетка —
#         то, что стоит во всех экземплярах (или почти во всех), набор
#         с долями — если в экземплярах разные блоки своего вида, «.» —
#         рельеф (у экземпляров разное);
#   python3 tools/measure/temples.py ground <мир>
#       — высота рамки против рельефа вокруг и что лежит под низом;
#   python3 tools/measure/temples.py census <мир>
#       — перепись блоков в рамках (сверка с тестом temples_census).
import os, sys, re, collections, struct, zlib, glob
sys.path.insert(0, os.path.dirname(__file__))
from nether_fortress import localize, to_local, state_name, base
from monument import World
from nether_census import nbt

KINDS = {
    'minecraft:desert_pyramid': ('desert_pyramid', 'tedp'),
    'minecraft:jungle_pyramid': ('jungle_pyramid', 'tejp'),
    'minecraft:swamp_hut': ('swamp_hut', 'tesh'),
    'minecraft:igloo': ('igloo', 'iglu'),
}
# Сколько клеток вниз от низа рамки снимать (тайник пирамиды — под рамкой).
BELOW = {'desert_pyramid': 14, 'jungle_pyramid': 4, 'swamp_hut': 0, 'igloo': 0}
# Свои материалы вида: клетка, где у экземпляров разные блоки только из
# этого списка, — случайная, иначе рельеф.
OWN = {
    'desert_pyramid': ('sandstone', 'cut_sandstone', 'chiseled_sandstone', 'sandstone_stairs', 'sandstone_slab', 'orange_terracotta', 'blue_terracotta',
                       'sand', 'suspicious_sand', 'tnt', 'stone_pressure_plate', 'chest', 'air'),
    'jungle_pyramid': ('cobblestone', 'mossy_cobblestone', 'cobblestone_stairs', 'chiseled_stone_bricks', 'stone_bricks', 'mossy_stone_bricks', 'cracked_stone_bricks',
                       'tripwire_hook', 'tripwire', 'dispenser', 'redstone_wire', 'repeater', 'sticky_piston', 'lever', 'chest', 'vine', 'air'),
    'swamp_hut': ('spruce_planks', 'oak_log', 'oak_fence', 'spruce_stairs', 'crafting_table', 'cauldron', 'potted_red_mushroom', 'flower_pot', 'air'),
    'igloo': (),
}
SYMBOLS = '#ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!$%&*+-/:;<=>?@^_`{|}~'

def starts(world):
    w = World(world); out = collections.defaultdict(list)
    for name in sorted(os.listdir(w.reg)):
        m = re.match(r'r\.(-?\d+)\.(-?\d+)\.mca$', name)
        if not m: continue
        rx, rz = int(m.group(1)), int(m.group(2))
        for i in range(1024):
            cx, cz = rx * 32 + (i & 31), rz * 32 + (i >> 5)
            root = w.chunk(cx, cz)
            if not root: continue
            for key, st in root.get('structures', {}).get('starts', {}).items():
                if key not in KINDS or st.get('id') == 'INVALID': continue
                ch = st['Children']
                bb = [min(p['BB'][i] for p in ch) for i in range(3)] + [max(p['BB'][i] for p in ch) for i in range(3, 6)]
                if all(w.full(x, z) for x in range((bb[0] >> 4) - 1, (bb[3] >> 4) + 2) for z in range((bb[2] >> 4) - 1, (bb[5] >> 4) + 2)):
                    out[KINDS[key][0]].append((cx, cz, ch, bb))
    return w, out

def entities(world):
    out = []
    for p in glob.glob(os.path.join(world, 'dimensions/minecraft/overworld/entities/r.*.mca')):
        data = open(p, 'rb').read()
        for i in range(1024):
            off = int.from_bytes(data[i*4:i*4+3], 'big')
            if off == 0 or off * 4096 + 5 > len(data): continue
            q = off * 4096; ln = struct.unpack_from('>i', data, q)[0]
            try: raw = zlib.decompress(data[q+5:q+4+ln])
            except zlib.error: continue
            n = struct.unpack_from('>H', raw, 1)[0]
            out.extend(nbt(raw, 3 + n, 10)[0].get('Entities', []))
    return out

def cut(w, bb, o, below):
    """Экземпляр в рамке: {(a, y, b): состояние в осях рамки}."""
    cells = {}
    for x in range(bb[0], bb[3] + 1):
        for z in range(bb[2], bb[5] + 1):
            a, b = to_local(o, bb, x, z)
            for y in range(bb[1] - below, bb[4] + 1):
                s = w.block(x, y, z) or 'air'
                if base(s) in ('cave_air', 'void_air'): s = 'air'
                cells[(a, y - bb[1], b)] = localize(s, o)
    return cells

def merge(kind, insts):
    """Сводная клетка: состояние, набор с долями или None (рельеф).

    Воздух — только если он во всех экземплярах (иначе это воздух над
    рельефом); у хижины нижний слой — сваи, прочее там рельеф. В пустыне
    песок с песчаником без «подозрительного» песка — рельеф пустыни, а с ним —
    засыпанная комната: песок или подозрительный песок."""
    keys = set().union(*insts)
    out = {}
    for k in keys:
        vals = collections.Counter(i.get(k, 'air') for i in insts)
        top, n = vals.most_common(1)[0]
        names = {base(v) for v in vals}
        if base(top) == 'air' and (n < len(insts) or (kind == 'swamp_hut' and k[1] == 0)):
            out[k] = None
        elif kind == 'desert_pyramid' and 'suspicious_sand' in names:
            sus = sum(c for v, c in vals.items() if base(v) == 'suspicious_sand')
            out[k] = tuple(sorted({'sand': len(insts) - sus, 'suspicious_sand[dusted=0]': sus}.items()))
        elif kind == 'desert_pyramid' and names <= {'sand', 'sandstone', 'air', 'cut_sandstone', 'water', 'stone'} and len(names) > 1:
            out[k] = None
        elif n >= len(insts) * 0.75 and (base(top) in OWN[kind] or n == len(insts)):
            out[k] = top
        elif all(base(v) in OWN[kind] and base(v) != 'air' for v in vals):
            out[k] = tuple(sorted(vals.items()))
        else:
            out[k] = None
    return out

def cmd_templates(world, path):
    w, found = starts(world)
    lines = ['# Храмы, хижина, иглу — замер мира оригинала 26.1.2 (tools/measure/temples.py',
             '# templates). Вид — рамка куска: a — вбок, b — вперёд (сторона O рамки, как у',
             '# nether_fortress.py), y — от низа рамки (с запасом вниз). Слои снизу вверх,',
             '# строки — по b, знаки — по a. «.» — рельеф, «#» — первый знак палитры;',
             '# набор «X a=n b=m» — случайный блок с весами (у экземпляров разное).', '']
    for kind in ('desert_pyramid', 'jungle_pyramid', 'swamp_hut'):
        insts = []
        for cx, cz, ch, bb in found[kind]:
            o = ch[0]['O']
            insts.append(cut(w, bb, o, BELOW[kind]))
        merged = merge(kind, insts)
        W = max(k[0] for k in merged) + 1; D = max(k[2] for k in merged) + 1
        ys = sorted({k[1] for k in merged})
        palette = {}
        def sym(v):
            if v is None: return '.'
            if v not in palette: palette[v] = SYMBOLS[len(palette)]
            return palette[v]
        layers = []
        for y in ys:
            layers.append([''.join(sym(merged.get((a, y, b))) for a in range(W)) for b in range(D)])
        lines.append('%s %d %d %d %d %d' % (kind, W, D, len(ys), ys[0], len(insts)))
        for v, ch in palette.items():
            if isinstance(v, tuple):
                lines.append('  %s %s' % (ch, ' '.join('%s=%d' % kv for kv in v)))
            else:
                lines.append('  %s %s' % (ch, v))
        for y, rows in zip(ys, layers):
            lines.append('layer %d' % y); lines.extend('  ' + r for r in rows)
        lines.append('')
        print(kind, 'экземпляров', len(insts), 'рамка', W, D, len(ys), 'палитра', len(palette))
    # Иглу: верх (igloo/top), подвал (igloo/bottom) и шахта (igloo/middle) —
    # каждый образец сводится отдельно, в рамке своего куска.
    parts = collections.defaultdict(list); depth = collections.Counter(); with_basement = 0
    for cx, cz, ch, bb in found['igloo']:
        middles = sum(1 for p in ch if p['Template'] == 'minecraft:igloo/middle')
        depth[middles] += 1; with_basement += middles > 0
        for p in ch:
            parts[p['Template'].split('/')[-1]].append(cut(w, p['BB'], p['O'] if p['O'] >= 0 else 0, 0))
    for part in ('top', 'middle', 'bottom'):
        insts = parts[part]
        if not insts: continue
        # У иглу сторону даёт Rot, а не O; сводим по самому частому размеру.
        size = collections.Counter((max(k[0] for k in i) + 1, max(k[2] for k in i) + 1) for i in insts).most_common(1)[0][0]
        same = [i for i in insts if (max(k[0] for k in i) + 1, max(k[2] for k in i) + 1) == size]
        print('igloo', part, len(insts), 'размер', size, 'того же размера', len(same))
    print('иглу: подвал у', with_basement, 'из', len(found['igloo']), 'шахта (кусков middle):', sorted(depth.items()))
    open(path, 'w').write('\n'.join(lines))

def surface(w, x, z, top=200):
    for y in range(top, -60, -1):
        s = base(w.block(x, y, z) or 'air')
        if s not in ('air', 'cave_air', 'short_grass', 'tall_grass', 'dead_bush', 'fern', 'large_fern', 'vine', 'snow', 'cactus',
                     'oak_leaves', 'jungle_leaves', 'spruce_leaves', 'birch_leaves', 'mangrove_leaves', 'dark_oak_leaves', 'acacia_leaves',
                     'jungle_log', 'oak_log', 'cocoa', 'melon', 'bamboo', 'lily_pad', 'seagrass', 'sugar_cane', 'moss_carpet', 'mangrove_roots', 'mangrove_propagule', 'mangrove_log'):
            return y, s
    return None, None

def cmd_ground(world):
    w, found = starts(world)
    for kind, insts in sorted(found.items()):
        print(kind)
        under = collections.Counter()
        for cx, cz, ch, bb in insts:
            ring = []
            for x in range(bb[0] - 2, bb[3] + 3):
                for z in (bb[2] - 2, bb[5] + 2):
                    ring.append(surface(w, x, z)[0])
            for z in range(bb[2] - 2, bb[5] + 3):
                for x in (bb[0] - 2, bb[3] + 2):
                    ring.append(surface(w, x, z)[0])
            ring = [r for r in ring if r is not None]
            corners = [surface(w, x, z)[0] for x in (bb[0], bb[3]) for z in (bb[2], bb[5])]
            print('   y0 %d  кольцо мин %d сред %.1f макс %d  углы %s  O %s' % (bb[1], min(ring), sum(ring) / len(ring), max(ring), corners, ch[0].get('O')))
            for x in range(bb[0], bb[3] + 1):
                for z in range(bb[2], bb[5] + 1):
                    for d in range(1, 6):
                        under[(d, base(w.block(x, bb[1] - BELOW.get(kind, 0) - d, z) or '?'))] += 1
        print('   под рамкой:', sorted(under.items(), key=lambda kv: (kv[0][0], -kv[1]))[:30])

def cmd_census(world):
    w, found = starts(world)
    ents = entities(world)
    for kind, insts in sorted(found.items()):
        total = collections.Counter(); mobs = collections.Counter()
        for cx, cz, ch, bb in insts:
            for p in ch:
                b = p['BB']
                for x in range(b[0], b[3] + 1):
                    for z in range(b[2], b[5] + 1):
                        for y in range(b[1] - BELOW.get(kind, 0), b[4] + 1):
                            s = base(w.block(x, y, z) or 'air')
                            if s not in ('air', 'cave_air'): total[s] += 1
            for e in ents:
                x, y, z = e['Pos']
                if bb[0] - 1 <= x <= bb[3] + 2 and bb[2] - 1 <= z <= bb[5] + 2 and bb[1] - 40 <= y <= bb[4] + 2:
                    mobs[e['id']] += 1
        n = len(insts)
        print(kind, n, ' '.join('%s=%.1f' % (k, v / n) for k, v in total.most_common(30)))
        print('   сущности', {k: round(v / n, 2) for k, v in mobs.items()})


# ---------- иглу: куски-образцы с поворотом Rot ----------
# Образцы иглу вращаются не вокруг точки TP (у сторон сдвиги звеньев разные),
# поэтому рамка — от угла BB: NONE (a, b) → (x0 + a, z0 + b), CLOCKWISE_90 →
# (x1 − b, z0 + a), CLOCKWISE_180 → (x1 − a, z1 − b), COUNTERCLOCKWISE_90 →
# (x0 + b, z1 − a).
def bb_world(p, a, b):
    x0, _, z0, x1, _, z1 = p['BB']
    return {'NONE': (x0 + a, z0 + b), 'CLOCKWISE_90': (x1 - b, z0 + a), 'CLOCKWISE_180': (x1 - a, z1 - b), 'COUNTERCLOCKWISE_90': (x0 + b, z1 - a)}[p['Rot']]
def bb_local(p, x, z):
    x0, _, z0, x1, _, z1 = p['BB']
    return {'NONE': (x - x0, z - z0), 'CLOCKWISE_90': (z - z0, x1 - x), 'CLOCKWISE_180': (x1 - x, z1 - z), 'COUNTERCLOCKWISE_90': (z1 - z, x - x0)}[p['Rot']]
def cmd_igloo(world, path):
    """Образцы иглу (верх, звено шахты, подвал) в рамке образца с точкой TP —
    src/world/igloo.txt; сдвиги звеньев и подвала от точки верха; высота верха
    против рельефа; жители подвала."""
    import end_city as ec
    w, found = starts(world)
    ents = entities(world)
    parts = collections.defaultdict(list); offsets = collections.Counter(); ground = []; mobs = collections.Counter()
    for cx, cz, ch, bb in found['igloo']:
        top = next(p for p in ch if p['Template'].endswith('/top'))
        # Высота в записи — до посадки на рельеф (у всех верхов TPY 90):
        # настоящую ищем по снегу в рамке верха, сдвиг тот же у всех кусков.
        tb = top['BB']
        def snow(y0):
            return sum(1 for x in range(tb[0], tb[3] + 1) for z in range(tb[2], tb[5] + 1) for y in range(y0, y0 + 5) if base(w.block(x, y, z) or '') == 'snow_block')
        lift = max(range(-80, 120), key=lambda d: snow(top['TPY'] + d))
        for p in ch:
            p = dict(p); p['TPY'] += lift; p['BB'] = [p['BB'][0], p['BB'][1] + lift, p['BB'][2], p['BB'][3], p['BB'][4] + lift, p['BB'][5]]
            W, H, D = ec.size(p)
            cells = {}
            for a in range(W):
                for b in range(D):
                    x, z = bb_world(p, a, b)
                    for y in range(H):
                        s = w.block(x, p['TPY'] + y, z) or 'air'
                        if base(s) in ('cave_air', 'void_air'): s = 'air'
                        cells[(a, y, b)] = ec.localize(s, p['Rot'])
            part = p['Template'].split('/')[-1]
            parts[part].append(cells)
            if part != 'top':
                # Угол куска (a = 0, b = 0) в рамке верха.
                x, z = bb_world(p, 0, 0)
                a, b = bb_local(top, x, z)
                offsets[(part, a, p['TPY'] - top['TPY'], b, ec.relrot(top, p))] += 1
            if part == 'bottom':
                for e in ents:
                    if e['id'] in ('minecraft:villager', 'minecraft:zombie_villager'):
                        x, y, z = e['Pos']
                        if p['BB'][0] <= x <= p['BB'][3] + 1 and p['BB'][2] <= z <= p['BB'][5] + 1 and p['BB'][1] <= y <= p['BB'][4] + 1:
                            a, b = bb_local(p, int(x // 1), int(z // 1))
                            mobs[(e['id'], a, round(y - p['TPY'], 2), b)] += 1
        ring = [surface(w, x, z)[0] for x in range(top['BB'][0] - 2, top['BB'][3] + 3) for z in (top['BB'][2] - 2, top['BB'][5] + 2)]
        ring = [r for r in ring if r is not None]
        ground.append((top['TPY'] + lift, min(ring), round(sum(ring) / len(ring), 1), max(ring)))
    lines = ['# Иглу — замер мира оригинала 26.1.2 (tools/measure/temples.py igloo).',
             '# Образцы igloo/top, igloo/middle, igloo/bottom в рамке образца: a, b от точки',
             '# TP (поворот снят), слои снизу вверх от TPY. «.» — рельеф, «#» — первый знак.', '']
    for part in ('top', 'middle', 'bottom'):
        insts = parts[part]
        keys = set().union(*insts)
        W = max(k[0] for k in keys) + 1; H = max(k[1] for k in keys) + 1; D = max(k[2] for k in keys) + 1
        palette = {}
        def sym(k):
            a, y, b = k
            vals = collections.Counter(i.get(k, 'air') for i in insts)
            top, n = vals.most_common(1)[0]
            if n < len(insts) * 0.75: return '.'
            # Воздух — только общий для всех (иначе это воздух над рельефом),
            # снежный покров — погода, не образец.
            if (base(top) == 'air' and n < len(insts)) or base(top) == 'snow': return '.'
            if 'trapdoor' in top: print('люк', top, (a, y, b))
            if top not in palette: palette[top] = SYMBOLS[len(palette)]
            return palette[top]
        layers = [[''.join(sym((a, y, b)) for a in range(W)) for b in range(D)] for y in range(H)]
        lines.append('%s %d %d %d 0 %d' % (part, W, D, H, len(insts)))
        for v, ch in palette.items(): lines.append('  %s %s' % (ch, v))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y); lines.extend('  ' + r for r in rows)
        lines.append('')
        print(part, len(insts), (W, H, D), 'палитра', len(palette))
    open(path, 'w').write('\n'.join(lines))
    print('сдвиги от точки верха (кусок, a, b, поворот):', sorted(offsets.items()))
    print('верх: TPY, рельеф мин/сред/макс:', ground)
    print('жители подвала (a, y от TPY, b):', sorted(mobs.items()))

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'templates': cmd_templates, 'ground': cmd_ground, 'census': cmd_census, 'igloo': cmd_igloo}[cmd](*args)
