#!/usr/bin/env python3
# Заброшенные шахты по замеру мира оригинала 26.1.2. Код игры, jar и выгрузки
# worldgen не используются: сервер оригинала только складывал мир (свой,
# строения включены) и слушал RCON (`forceload` вокруг стартов), а всё
# снимается с region-файлов — записи о строениях (structures.starts
# minecraft:mineshaft / mineshaft_mesa: куски msroom, mscorridor, mscrossing,
# msstairs с рамкой BB, стороной O, коленом GD и признаками Num, hr, sc, tf, D),
# блоки в рамках и вагонетки в entities/*.mca.
#
#   python3 tools/measure/mineshaft.py generate <мир> <порт rcon> <пароль> [сколько]
#       — догружает (forceload) по 15×15 чанков вокруг стартов шахт;
#   python3 tools/measure/mineshaft.py stats <мир>
#       — куски, колена, длины коридоров, кто к кому пристроен (вперёд,
#         вбок, через колено), выходы комнаты, пределы по высоте и дальности;
#   python3 tools/measure/mineshaft.py cells <мир>
#       — чистые коридоры (проход цел): опоры, верхний ряд, факелы, рельсы,
#         паутина, спавнеры, вагонетки;
#   python3 tools/measure/mineshaft.py census <мир>
#       — перепись блоков шахт, сложенных целиком (сверка с тестом
#         mineshaft_world в src/world/mineshaft.rs).
#
# Рамка куска: f — вперёд от входа (по O: 0 юг, 1 запад, 2 север, 3 восток),
# s — вбок от левого края, y — от низа рамки.
import os, sys, re, time, collections, struct, zlib, glob
sys.path.insert(0, os.path.dirname(__file__))
from nether_fortress import Rcon
from monument import World
from nether_census import nbt

C = collections.Counter
F = {0: (0, 1), 1: (-1, 0), 2: (0, -1), 3: (1, 0)}
KEYS = ('minecraft:mineshaft', 'minecraft:mineshaft_mesa')

def shafts(w):
    out = []
    for name in sorted(os.listdir(w.reg)):
        m = re.match(r'r\.(-?\d+)\.(-?\d+)\.mca$', name)
        if not m: continue
        rx, rz = int(m.group(1)), int(m.group(2))
        for i in range(1024):
            cx, cz = rx * 32 + (i & 31), rz * 32 + (i >> 5)
            root = w.chunk(cx, cz)
            if not root: continue
            for key in KEYS:
                st = root.get('structures', {}).get('starts', {}).get(key)
                if st and st.get('id') != 'INVALID': out.append((cx, cz, st['Children']))
    return out

def name(p): return p['id'][12:]

def touch(a, b):
    ov = lambda i: a[i] <= b[i + 3] and b[i] <= a[i + 3]
    for ax in (0, 2):
        o = [i for i in (0, 1, 2) if i != ax]
        if (a[ax + 3] + 1 == b[ax] or b[ax + 3] + 1 == a[ax]) and all(ov(i) for i in o): return True
    return False

def frame(b, o):
    fx, fz = F[o]
    ox, oz = {0: (b[0], b[2]), 2: (b[3], b[5]), 1: (b[3], b[2]), 3: (b[0], b[5])}[o]
    return lambda x, z: ((x - ox) * fx + (z - oz) * fz, (x - ox) * (-fz) + (z - oz) * fx)

def full_bb(w, b): return all(w.full(x, z) for x in range(b[0] >> 4, (b[3] >> 4) + 1) for z in range(b[2] >> 4, (b[5] >> 4) + 1))

def cmd_generate(world, port, password, count=40):
    w = World(world); r = Rcon(int(port), password)
    pts = sorted(((cx, cz) for cx, cz, _ in shafts(w)), key=lambda p: p[0] ** 2 + p[1] ** 2)[:int(count)]
    for n, (cx, cz) in enumerate(pts):
        area = [(i, j) for i in range(cx - 7, cx + 8) for j in range(cz - 7, cz + 8)]
        if not all(w.full(*c) for c in area):
            box = '%d %d %d %d' % ((cx - 7) * 16, (cz - 7) * 16, (cx + 7) * 16 + 15, (cz + 7) * 16 + 15)
            r.run('forceload add ' + box)
            for _ in range(120):
                time.sleep(3); r.run('save-all'); w.regions.clear(); w.grids.clear()
                if all(w.full(*c) for c in area): break
            r.run('forceload remove ' + box)
        print(n, cx, cz, flush=True)

def cmd_stats(world):
    w = World(world); all_ = shafts(w)
    dims = collections.defaultdict(C); attrs = collections.defaultdict(C); gd = C(); kids = collections.defaultdict(C)
    tops, bottoms, far = [], [], 0
    for cx, cz, ch in all_:
        r = ch[0]['BB']
        tops.append(max(p['BB'][4] for p in ch)); bottoms.append(min(p['BB'][1] for p in ch))
        for p in ch:
            b = p['BB']; t = name(p); gd[p['GD']] += 1
            dims[t][(b[3] - b[0] + 1, b[4] - b[1] + 1, b[5] - b[2] + 1)] += 1
            for k in ('Num', 'hr', 'sc', 'tf'):
                if k in p: attrs[(t, k)][p[k]] += 1
            near = lambda lo, hi, at: 0 if lo <= at <= hi else min(abs(lo - at), abs(hi - at))
            if p is not ch[0]: far = max(far, near(b[0], b[3], r[0]), near(b[2], b[5], r[2]))
            # Потомки: касающиеся куски на 1 и 2 колена глубже.
            if t in ('corridor', 'crossing', 'stairs'):
                o = p['O'] if p['O'] >= 0 else p['D']; loc = frame(b, o)
                L = (b[3] - b[0] + 1, b[5] - b[2] + 1)[o in (0, 2)]
                for q in ch:
                    d = q['GD'] - p['GD']
                    if d not in (1, 2) or not touch(b, q['BB']): continue
                    qb = q['BB']; cs = [loc(x, z) for x in (qb[0], qb[3]) for z in (qb[2], qb[5])]
                    fmin = min(c[0] for c in cs)
                    pos = 'вперёд' if fmin >= L else 'назад' if max(c[0] for c in cs) < 0 else 'вбок'
                    kids[(t, d, pos)][name(q)] += 1
    print('шахт', len(all_), 'кусков на шахту %.1f' % (sum(gd.values()) / len(all_)))
    print('колена', sorted(gd.items()))
    for t, d in dims.items(): print(t, sorted(d.items())[:20])
    for k, d in sorted(attrs.items()): print(k, sorted(d.items()))
    for k, d in sorted(kids.items()): print('потомки', k, dict(d))
    print('верх', min(tops), '…', max(tops), ' низ', min(bottoms), '…', max(bottoms), ' дальность от угла комнаты', far)

def cmd_cells(world):
    w = World(world); AIRY = ('cave_air', 'air', 'cobweb', 'rail', 'wall_torch', 'spawner')
    base = lambda s: (s or '?').split('[')[0]
    sup = C(); ceil = C(); torch = C(); rails = C(); webs = C(); n = C()
    for cx, cz, ch in shafts(w):
        for p in ch:
            if name(p) != 'corridor' or not full_bb(w, p['BB']): continue
            b = p['BB']; L = p['Num'] * 5; loc = frame(b, p['O'])
            inv = {loc(x, z): (x, z) for x in range(b[0], b[3] + 1) for z in range(b[2], b[5] + 1)}
            at = lambda f, s, y: base(w.block(inv[(f, s)][0], b[1] + y, inv[(f, s)][1]))
            if not all(at(f, s, y) in AIRY or at(f, s, y).endswith('fence') for f in range(L) for s in (-2, -1, 0) for y in (0, 1)): continue
            key = (p['hr'], p['sc']); n[key] += 1
            for f in range(L):
                if f % 5 == 2:
                    sup[(key, at(f, -2, 0), at(f, -1, 2))] += 1
                else:
                    for s in (-2, -1, 0): ceil[(key, s, at(f, s, 2))] += 1
                    for s in (-2, 0): webs[(key, at(f, s, 0) == 'cobweb')] += 1
                if 'torch' in at(f, -1, 2): torch[(key, f % 5)] += 1
                if key[0]: rails[at(f, -1, 0)] += 1
    print('чистых коридоров', dict(n))
    print('опоры (столб, середина балки)', sup.most_common(12))
    print('верхний ряд', ceil.most_common(20))
    print('факелы', sorted(torch.items())); print('рельсы посередине', rails.most_common(4)); print('паутина внизу', sorted(webs.items()))

def cmd_census(world):
    w = World(world)
    KEEP = ('planks', 'fence', 'rail', 'cobweb', 'wall_torch', 'spawner', 'log', 'iron_chain')
    total = C(); n = 0; pieces = 0
    for cx, cz, ch in shafts(w):
        if not all(full_bb(w, p['BB']) for p in ch): continue
        n += 1; pieces += len(ch); seen = set()
        for p in ch:
            b = p['BB']
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1):
                    for y in range(b[1] - 1, b[4] + 1):
                        if (x, y, z) in seen: continue
                        seen.add((x, y, z))
                        s = (w.block(x, y, z) or '?').split('[')[0]
                        k = next((k for k in KEEP if s.endswith(k) and ('oak' in s or k not in ('planks', 'fence', 'log'))), None)
                        if k: total[k] += 1
    print('шахт целиком', n, 'кусков на шахту %.1f' % (pieces / max(n, 1)))
    print('на шахту:', ' '.join('%s=%.1f' % (k, total[k] / max(n, 1)) for k in KEEP))

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'generate': cmd_generate, 'stats': cmd_stats, 'cells': cmd_cells, 'census': cmd_census}[cmd](*args)
