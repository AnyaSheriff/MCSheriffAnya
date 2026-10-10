#!/usr/bin/env python3
# Аванпосты разбойников (pillager outpost) по замеру мира оригинала 26.1.2.
# Код игры, jar и выгрузки worldgen не используются: свой сервер оригинала
# (обычный мир, generate-structures=true) только запускается и слушает RCON,
# а всё снимается с region-файлов, которые он сложил, — блоки, сущности
# блоков (сундук с LootTable), сущности (железные големы и аллаи клеток —
# entities/r.*.mca) и записи о строениях в чанках (structures.starts
# minecraft:pillager_outpost: куски Children — pool_element.location,
# rotation, PosX/Y/Z, BB, projection, junctions).
#
# Найти и сложить:
#   python3 tools/measure/template_structs.py locate <порт> <пароль> pillager_outpost <радиус> <шаг> <вывод.tsv>
#   python3 tools/measure/template_structs.py generate <мир> <порт> <пароль> <вывод.tsv> 3
# Дальше:
#   python3 tools/measure/pillager_outpost.py stats <мир> <starts.tsv>
#       — куски по образцам; высота кусков против земли;
#   python3 tools/measure/pillager_outpost.py export <мир> <starts.tsv> <вывод.txt>
#       — раскладки и образцы кусков (src/world/pillager_outpost.txt);
#   python3 tools/measure/pillager_outpost.py census <мир> <starts.tsv>
#       — перепись блоков в рамках кусков на аванпост, мобы клеток.
#
# Куски: base_plate (старт, rigid), watchtower (rigid, на нём же; запись —
# list_pool_element: башня и поверх неё заросшая с порчей outpost_rot),
# feature_plate (terrain_matching — площадка по рельефу) и на них rigid-куски
# (клетки, палатки, брёвна, мишени), каждый на земле у своего стыка.
import os, sys, collections, json
sys.path.insert(0, os.path.dirname(__file__))
from template_structs import World, read_starts, records
from village import ROT, TURNS, unrotate

SYMBOLS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!$%&*+-<=>?@^~|;:,/'
SOFT = ('air', 'cave_air', 'short_grass', 'tall_grass', 'fern', 'large_fern', 'snow', 'dandelion', 'poppy',
        'azure_bluet', 'oxeye_daisy', 'cornflower', 'sunflower', 'lilac', 'rose_bush', 'peony', 'dead_bush',
        'sweet_berry_bush', 'pumpkin', 'short_dry_grass', 'tall_dry_grass', 'bush', 'firefly_bush', 'leaf_litter',
        'wildflowers', 'pink_petals', 'water')
NATURAL = ('grass_block', 'dirt', 'coarse_dirt', 'sand', 'red_sand', 'sandstone', 'stone', 'gravel', 'snow', 'snow_block',
           'short_grass', 'tall_grass', 'clay', 'andesite', 'diorite', 'granite', 'podzol', 'mud', 'ice', 'powder_snow',
           'terracotta', 'calcite', 'packed_ice', 'tuff', 'deepslate', 'water', 'fern', 'large_fern', 'dandelion', 'poppy',
           'pink_petals', 'leaf_litter', 'bush', 'short_dry_grass', 'tall_dry_grass', 'dead_bush', 'wildflowers',
           'oxeye_daisy', 'cornflower', 'azure_bluet', 'sweet_berry_bush', 'lily_of_the_valley', 'allium', 'red_tulip',
           'orange_tulip', 'white_tulip', 'pink_tulip', 'emerald_ore', 'coal_ore', 'iron_ore', 'copper_ore', 'lapis_ore',
           'gold_ore')

def name_of(p):
    pe = p['pool_element']
    if 'elements' in pe: pe = pe['elements'][0]
    return pe.get('location', '?').replace('minecraft:pillager_outpost/', '')

def proj_of(p):
    return p['pool_element'].get('projection', 'rigid')

def surface(w, x, z, top=320, bottom=-64):
    """Верх рельефа столбца: без травы, цветов, воды, листвы и стволов."""
    for y in range(top, bottom - 1, -1):
        n = w.block(x, y, z)
        if n is None: return None
        base = n.split('[')[0]
        if base not in SOFT and not base.endswith(('_leaves', '_log', 'vine')): return y
    return None

def ring(w, b):
    """Медиана земли по кольцу вокруг рамки (как village.py export и
    постановка в src/world/pillager_outpost.rs)."""
    vals = [surface(w, x, z) for x in range(b[0] - 1, b[3] + 2) for z in (b[2] - 1, b[5] + 1)]
    vals += [surface(w, x, z) for z in range(b[2], b[5] + 1) for x in (b[0] - 1, b[3] + 1)]
    vals = sorted(v for v in vals if v is not None)
    return vals[len(vals) // 2] if vals else None

def size_of(p):
    b = p['BB']; t = TURNS[p['rotation']]
    w, h, l = b[3] - b[0] + 1, b[4] - b[1] + 1, b[5] - b[2] + 1
    return (w, l, h) if t % 2 == 0 else (l, w, h)

def outposts(w, starts):
    by = collections.OrderedDict()
    for name, cx, cz, st in records(w, starts):
        by[(cx, cz)] = st['Children']
    out = []
    for (cx, cz), ps in by.items():
        bbs = [p['BB'] for p in ps]
        x0 = min(b[0] for b in bbs); z0 = min(b[2] for b in bbs); x1 = max(b[3] for b in bbs); z1 = max(b[5] for b in bbs)
        if all(w.full(i, j) for i in range((x0 - 2) >> 4, ((x1 + 2) >> 4) + 1) for j in range((z0 - 2) >> 4, ((z1 + 2) >> 4) + 1)):
            out.append((cx, cz, ps))
        else:
            print('не сложен', cx, cz, file=sys.stderr)
    return out

def anchor_of(p):
    """Стык, которым rigid-кусок висит на площадке: мировые x, z источника."""
    for j in p.get('junctions', []):
        if j.get('dest_proj') == 'terrain_matching': return j['source_x'], j['source_z']
    return None

def mobs_in(w, world, boxes):
    """Сущности (железные големы, аллаи) в рамках: entities/r.*.mca."""
    ew = World(world); ew.reg = os.path.join(world, 'dimensions/minecraft/overworld/entities')
    out = []
    chunks = set()
    for b in boxes:
        for i in range(b[0] >> 4, (b[3] >> 4) + 1):
            for j in range(b[2] >> 4, (b[5] >> 4) + 1): chunks.add((i, j))
    for i, j in chunks:
        root = ew.chunk(i, j)
        if not root: continue
        for e in root.get('Entities', []):
            pos = e.get('Pos', [0, 0, 0])
            x, y, z = (int(v // 1) for v in pos)
            for b in boxes:
                if b[0] <= x <= b[3] and b[1] <= y <= b[4] and b[2] <= z <= b[5]:
                    out.append((e.get('id', '?').replace('minecraft:', ''), pos, b))
    return out

def cmd_stats(world, starts):
    w = World(world)
    kinds = collections.Counter(); per = []
    for cx, cz, ps in outposts(w, read_starts(starts)):
        per.append(len(ps))
        start = ps[0]
        g = surface(w, start['PosX'], start['PosZ'])
        print('аванпост', cx, cz, 'кусков', len(ps), 'старт', name_of(start), start['rotation'], 'PosY', start['PosY'], 'земля у угла', g)
        for p in ps[1:]:
            kinds[name_of(p)] += 1
            a = anchor_of(p)
            if a and proj_of(p) == 'rigid':
                print('   ', name_of(p), p['rotation'], 'PosY', p['PosY'], 'земля у стыка', surface(w, *a))
        boxes = [p['BB'] for p in ps if 'cage' in name_of(p)]
        for kind, pos, b in mobs_in(w, world, boxes): print('    моб', kind, pos, b)
    print('куски:', kinds.most_common(), 'на аванпост', per)

def cmd_export(world, starts, out):
    w = World(world)
    grids = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    meta = {}; seen = collections.Counter(); layouts = []; loot = {}; mobs = collections.defaultdict(list)
    for cx, cz, ps in outposts(w, read_starts(starts)):
        plate = ps[0]['BB']; g0 = ring(w, plate)
        lay = []
        for p in ps:
            loc = name_of(p); proj = proj_of(p)
            # Плита старта и площадки — одни стыки, своих блоков у них нет
            # (их рамки — это башня и рельеф).
            if loc in ('base_plate', 'feature_plate'): continue
            t = TURNS[p['rotation']]; W, L, H = size_of(p)
            x0, y0, z0 = p['PosX'], p['PosY'], p['PosZ']
            meta[loc] = (W, L, H, proj); seen[loc] += 1
            for a in range(W):
                for bb in range(L):
                    dx, dz = ROT[p['rotation']](a, bb)
                    for y in range(H):
                        n = w.block(x0 + dx, y0 + y, z0 + dz)
                        if n is None: continue
                        grids[loc][(a, y, bb)][unrotate(n, t)] += 1
                        e = w.entity(x0 + dx, y0 + y, z0 + dz)
                        if e and e.get('LootTable'): loot[(loc, a, y, bb)] = e['LootTable'].replace('minecraft:', '')
            # Башня — над землёй вокруг плиты старта, прочие — вокруг себя.
            how = 'start' if loc == 'watchtower' else 'ring'
            ground = g0 if how == 'start' else ring(w, p['BB'])
            lay.append((loc, t, x0 - cx * 16, y0 - ground, z0 - cz * 16, how))
            if 'cage' in loc:
                found = [m for m in mobs_in(w, world, [p['BB']]) if m[0] in ('iron_golem', 'allay')]
                mobs[loc].append(len(found))
        layouts.append(lay)
    lines = ['# Аванпосты разбойников — замер мира оригинала 26.1.2 (%d аванпостов),' % len(layouts),
             '# tools/measure/pillager_outpost.py export. Сперва раскладки: layout — один',
             '# аванпост; кусок: образец, поворот (четверти по часовой), начало от угла',
             '# чанка старта по x и z, высота над медианой земли по кольцу вокруг рамки:',
             '# start — плиты старта 16×16 (у башни), ring — своей. Потом образцы (формат',
             '# src/world/template.rs): рамка',
             '# в собственном повороте куска, «.» — рельеф остаётся, «_» — пусто; loot —',
             '# таблица сундука; mob — моб клетки, место от начала куска и доля.', '']
    for lay in layouts:
        lines.append('layout %d' % len(lay))
        for loc, t, dx, dy, dz, how in lay:
            lines.append('  %s %d %d %d %d %s' % (loc, t, dx, dy, dz, how))
        lines.append('')
    for loc in sorted(meta):
        W, L, H, proj = meta[loc]
        cells = grids[loc]; pal = {}
        def sym(state):
            if state not in pal: pal[state] = SYMBOLS[len(pal)]
            return pal[state]
        layers = []
        for y in range(H):
            rows = []
            for bb in range(L):
                row = ''
                for a in range(W):
                    c = cells.get((a, y, bb))
                    if not c: row += '.'; continue
                    state, n = c.most_common(1)[0]
                    base = state.split('[')[0]
                    if n * 2 <= sum(c.values()) and seen[loc] > 1: row += '.'
                    elif base in NATURAL: row += '.'
                    elif base in ('air', 'cave_air'): row += '_'
                    else: row += sym(state)
                rows.append(row)
            layers.append(rows)
        lines.append('template %s %d %d %d %d' % (loc, W, L, H, seen[loc]))
        for state, ch in pal.items(): lines.append('  %s %s' % (ch, state))
        for (l, a, y, bb), table in sorted(loot.items()):
            if l == loc: lines.append('  loot %d %d %d %s' % (a, y, bb, table))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y)
            lines += ['  ' + r for r in rows]
        lines.append('')
    open(out, 'w').write('\n'.join(lines) + '\n')
    print(len(layouts), 'аванпостов,', len(meta), 'образцов', dict(seen))
    print('мобы клеток:', json.dumps({k: v for k, v in mobs.items()}, ensure_ascii=False))

def cmd_census(world, starts):
    w = World(world)
    total = collections.Counter(); n = 0
    for cx, cz, ps in outposts(w, read_starts(starts)):
        one = collections.Counter(); seen = set()
        for p in ps:
            if proj_of(p) != 'rigid': continue
            b = p['BB']
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1):
                    for y in range(b[1], b[4] + 1):
                        if (x, y, z) in seen: continue
                        seen.add((x, y, z))
                        s = w.block(x, y, z)
                        if s is None: continue
                        base = s.split('[')[0]
                        if base not in NATURAL and base not in ('air', 'cave_air'): one[base] += 1
        total.update(one); n += 1
        print('аванпост', cx, cz, ' '.join('%s=%d' % kv for kv in one.most_common(14)), flush=True)
    print('всего (%d), среднее на аванпост:' % n, ' '.join('%s=%.1f' % (k, v / max(1, n)) for k, v in total.most_common()))

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'stats': cmd_stats, 'export': cmd_export, 'census': cmd_census}[cmd](*args)
