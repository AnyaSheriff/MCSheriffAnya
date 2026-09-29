#!/usr/bin/env python3
# Крепости Незера по замеру мира оригинала 26.1.2. Код игры, jar и выгрузки
# worldgen не используются: сервер оригинала только запускается (свой мир,
# generate-structures=true) и слушает RCON, а всё остальное снимается с
# region-файлов, которые он сложил, — блоки и записи о строениях в чанках
# (structures.starts: вид куска, его рамка BB, сторона O, глубина GD).
#
#   python3 tools/measure/nether_fortress.py locate <порт rcon> <пароль> <регионов от центра> <вывод.tsv>
#       — через `locate structure` находит крепость или бастион в каждом
#         регионе 27×27 чанков и биом в точке старта;
#   python3 tools/measure/nether_fortress.py generate <мир> <порт rcon> <пароль> <starts.tsv> <сколько>
#       — догружает (forceload) по 16×16 чанков вокруг ближайших крепостей;
#   python3 tools/measure/nether_fortress.py stats <мир> <starts.tsv> [сколько]
#       — число кусков каждого вида, выходы кусков и что к ним пристроено,
#         размах крепостей;
#   python3 tools/measure/nether_fortress.py templates <мир> <starts.tsv> <вывод.txt> [сколько]
#       — блоки каждого вида кусков по всем их экземплярам: что ставится
#         всегда, где всегда пусто, где остаётся рельеф, какие столбцы
#         уходят опорой вниз. Вывод — src/world/nether_fortress.txt.
#
# Рамка куска: вперёд — сторона O (0 юг, 1 запад, 2 север, 3 восток), вбок —
# вперёд, повёрнутое на четверть по часовой (для O=0 это +x); у O=2 и O=3
# ось вбок зеркальна (так сходятся несимметричные куски — повороты коридора,
# лестничная комната; проверено по всем экземплярам).
import os, sys, struct, zlib, socket, re, time, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack

# ---------- RCON ----------
class Rcon:
    def __init__(self, port, password):
        self.sock = socket.create_connection(('127.0.0.1', port)); self.id = 0; self.call(3, password)
    def call(self, kind, text):
        self.id += 1; body = text.encode() + b'\0\0'
        self.sock.sendall(struct.pack('<iii', len(body) + 8, self.id, kind) + body)
        size = struct.unpack('<i', self.recv(4))[0]
        return self.recv(size)[8:-2].decode('utf8', 'replace')
    def recv(self, n):
        out = b''
        while len(out) < n:
            part = self.sock.recv(n - len(out))
            if not part: raise ConnectionError('rcon закрылся')
            out += part
        return out
    def run(self, command): return self.call(2, command)

# ---------- чтение мира ----------
class World:
    def __init__(self, world):
        self.reg = os.path.join(world, 'dimensions/minecraft/the_nether/region')
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
        raw = zlib.decompress(data[q+5:q+4+ln]); n = struct.unpack_from('>H', raw, 1)[0]
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
                    if not 0 <= sy <= 7 or not bs: continue
                    pal = [state_name(e) for e in bs['palette']]
                    idx = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                    for ly in range(16): grid[sy * 16 + ly] = [pal[i] for i in idx[ly * 256:(ly + 1) * 256]]
            self.grids[k] = grid
            if len(self.grids) > 400: self.grids.pop(next(iter(self.grids)))
        return self.grids[k]
    def block(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        if g is None or y not in g: return None
        return g[y][(z & 15) * 16 + (x & 15)]
    def fortresses(self, starts, count):
        rows = [l.split('\t') for l in open(starts).read().split('\n') if l]
        rows = sorted([(int(r[1]), int(r[2])) for r in rows if r[0] == 'fortress'], key=lambda p: p[0]**2 + p[1]**2)[:count]
        out = []
        for x, z in rows:
            root = self.chunk(x // 16, z // 16)
            st = root and root.get('structures', {}).get('starts', {}).get('minecraft:fortress')
            if st: out.append(((x // 16, z // 16), [dict(p, id=p['id'].replace('minecraft:', '')) for p in st['Children']]))
        return out

def state_name(e):
    name = e['Name'].replace('minecraft:', '')
    props = e.get('Properties')
    return name + ('[' + ','.join('%s=%s' % kv for kv in sorted(props.items())) + ']' if props else '')
def base(s): return s.split('[')[0]

# ---------- рамка куска ----------
FWD = {0: (0, 1), 1: (-1, 0), 2: (0, -1), 3: (1, 0)}
def mirrored(o): return o >= 2
def lateral(o):
    fx, fz = FWD[o]
    lx, lz = fz, -fx
    return (-lx, -lz) if mirrored(o) else (lx, lz)
def dims(o, bb):
    wx, wz = bb[3] - bb[0] + 1, bb[5] - bb[2] + 1
    return (wx, wz) if o in (0, 2) else (wz, wx)
def frame(o, bb):
    fx, fz = FWD[o]; lx, lz = lateral(o)
    xs, zs = (bb[0], bb[3]), (bb[2], bb[5])
    return (fx, fz, lx, lz, min(lx * X + lz * Z for X in xs for Z in zs), min(fx * X + fz * Z for X in xs for Z in zs))
def to_world(o, bb, a, b):
    fx, fz, lx, lz, amin, bmin = frame(o, bb)
    A, B = a + amin, b + bmin
    return lx * A + fx * B, lz * A + fz * B
def to_local(o, bb, x, z):
    fx, fz, lx, lz, amin, bmin = frame(o, bb)
    return lx * x + lz * z - amin, fx * x + fz * z - bmin
DIRS = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}
def localize(state, o):
    """Состояние с направлениями в рамке куска: вперёд — south, вбок — east."""
    if '[' not in state: return state
    name, props = state[:-1].split('[')
    L, F = lateral(o), FWD[o]
    def ld(d):
        v = DIRS[d]
        return {(1, 0): 'east', (-1, 0): 'west', (0, 1): 'south', (0, -1): 'north'}[(v[0] * L[0] + v[1] * L[1], v[0] * F[0] + v[1] * F[1])]
    out = {}
    for k, v in (p.split('=') for p in props.split(',')):
        if k in DIRS: out[ld(k)] = v
        elif k == 'facing' and v in DIRS: out[k] = ld(v)
        elif k == 'shape' and mirrored(o) and ('left' in v or 'right' in v):
            out[k] = v.replace('left', 'X').replace('right', 'left').replace('X', 'right')
        else: out[k] = v
    return name + '[' + ','.join('%s=%s' % kv for kv in sorted(out.items())) + ']'

# ---------- locate / generate ----------
BIOMES = ('nether_wastes', 'crimson_forest', 'warped_forest', 'soul_sand_valley', 'basalt_deltas')
def cmd_locate(port, password, radius, out):
    r = Rcon(int(port), password); radius = int(radius); found = {}
    for rx in range(-radius, radius):
        for rz in range(-radius, radius):
            x, z = rx * 432 + 216, rz * 432 + 216
            for s in ('fortress', 'bastion_remnant'):
                m = re.search(r'\[(-?\d+), ~, (-?\d+)\]', r.run('execute in minecraft:the_nether positioned %d 64 %d run locate structure minecraft:%s' % (x, z, s)))
                if m: found[(s, int(m.group(1)), int(m.group(2)))] = 1
    def biome(x, z):
        for name in BIOMES:
            if '(0 blocks away)' in r.run('execute in minecraft:the_nether positioned %d 64 %d run locate biome minecraft:%s' % (x, z, name)):
                return name
    per_region = collections.defaultdict(list); share = collections.Counter(); offsets = collections.Counter()
    with open(out, 'w') as f:
        for s, x, z in sorted(found):
            b = biome(x, z)
            f.write('%s\t%d\t%d\t%s\n' % (s, x, z, b))
            cx, cz = x // 16, z // 16
            if -radius <= cx // 27 < radius and -radius <= cz // 27 < radius:
                per_region[(cx // 27, cz // 27)].append(s); share[(s, b == 'basalt_deltas')] += 1
                offsets[cx % 27] += 1; offsets[cz % 27] += 1
    print('регионов', (2 * radius) ** 2, 'с постройкой', len(per_region))
    print('по видам и дельтам:', dict(share))
    print('сдвиг старта в регионе (чанков):', sorted(offsets.items()))

def cmd_generate(world, port, password, starts, count):
    w = World(world); r = Rcon(int(port), password); within = 'execute in minecraft:the_nether run '
    rows = [l.split('\t') for l in open(starts).read().split('\n') if l]
    rows = sorted([(int(x[1]), int(x[2])) for x in rows if x[0] == 'fortress'], key=lambda p: p[0]**2 + p[1]**2)[:int(count)]
    for n, (x, z) in enumerate(rows):
        cx, cz = x // 16, z // 16
        area = [(i, j) for i in range(cx - 8, cx + 8) for j in range(cz - 8, cz + 8)]
        box = '%d %d %d %d' % ((cx - 8) * 16, (cz - 8) * 16, (cx + 7) * 16 + 15, (cz + 7) * 16 + 15)
        if not all(w.full(*c) for c in area):
            r.run(within + 'forceload add ' + box)
            while True:
                time.sleep(5); r.run('save-all'); w.regions.clear()
                if all(w.full(*c) for c in area): break
            r.run(within + 'forceload remove ' + box)
        print(n, x, z, flush=True)
    r.run('save-all')

# ---------- раскладка ----------
# Выходы кусков: (a, y уровня, b, куда, семья: B — мост, C — замок).
EXITS = {
    'nebcr': [(-1, 0, 9, '-a', 'B'), (9, 0, 19, 'F', 'B'), (19, 0, 9, '+a', 'B')],
    'nebs': [(2, 0, 19, 'F', 'B')],
    'nerc': [(-1, -3, 3, '-a', 'B'), (3, -3, 7, 'F', 'B'), (7, -3, 3, '+a', 'B')],
    'nesr': [(7, 3, 3, '+a', 'B')],
    'nece': [(6, 3, 13, 'F', 'C')],
    'nesc': [(2, 0, 5, 'F', 'C')],
    'nescsc': [(-1, 0, 2, '-a', 'C'), (2, 0, 5, 'F', 'C'), (5, 0, 2, '+a', 'C')],
    'nesclt': [(-1, 0, 2, '-a', 'C')],
    'nescrt': [(5, 0, 2, '+a', 'C')],
    'nectb': [(-1, 0, 2, '-a', 'C'), (9, 0, 2, '+a', 'C')],
    'neccs': [(2, 0, 10, 'F', 'C')],
    'necsr': [(6, 3, 13, 'F', 'C'), (6, 11, 13, 'F', 'C')],
}
def entrance(p):
    """Середина входа куска (середина вбок, низ, b = 0) и клетка перед ним."""
    o, bb = p['O'], p['BB']; W, D = dims(o, bb)
    x, z = to_world(o, bb, W // 2, 0); F = FWD[o]
    return (x, bb[1], z), (x - F[0], bb[1], z - F[1])
def cmd_stats(world, starts, count=40):
    w = World(world); fs = w.fortresses(starts, int(count))
    print('крепостей', len(fs))
    per = collections.defaultdict(list); spread = []; start_y = collections.Counter()
    exits = collections.defaultdict(collections.Counter); limited = collections.defaultdict(collections.Counter)
    CAP = {'nebcr': 5, 'nerc': 4, 'nesr': 3, 'nemt': 2, 'nece': 1}
    for (cx, cz), ch in fs:
        c = collections.Counter(p['id'] for p in ch)
        for k in set(p['id'] for _, q in fs for p in q): per[k].append(c[k])
        spread.append(max(p['BB'][3] for p in ch) - min(p['BB'][0] for p in ch))
        start_y[ch[0]['BB'][1]] += 1
        kids = collections.defaultdict(list)
        for p in ch[1:]:
            cell, _ = entrance(p); kids[(cell[0], cell[2])].append(p)
        for p in ch:
            for a, y, b, d, fam in EXITS.get(p['id'], []):
                x, z = to_world(p['O'], p['BB'], a, b)
                got = [k for k in kids.get((x, z), []) if k['GD'] == p['GD'] + 1]
                exits[(p['id'], a, y, b)][got[0]['id'] if got else '-'] += 1
        # порядок в списке — порядок постройки: что ставится, когда
        # ограниченные виды мостов кончились
        done = collections.Counter()
        for p in ch:
            if p['id'] in list(CAP) + ['nebs'] or (p['id'] == 'nebef' and (p['BB'][1] - ch[0]['BB'][1]) % 1 == 0):
                limited[sum(1 for k in CAP if done[k] < CAP[k])][p['id']] += 1
            done[p['id']] += 1
    print('кусков на крепость (среднее, наибольшее):')
    for k, v in sorted(per.items()): print('  %-7s %6.2f %3d' % (k, sum(v) / len(v), max(v)))
    print('размах по x, блоков: среднее %.0f' % (sum(spread) / len(spread)))
    print('высота старта (низ креста):', sorted(start_y.items()))
    print('выходы: вид, a, y, b → что пристроено')
    for k in sorted(exits): print('  ', k, ', '.join('%s %d' % kv for kv in exits[k].most_common()))
    print('мостовые куски по числу оставшихся ограниченных видов:')
    for k in sorted(limited): print('  ', k, ', '.join('%s %d' % kv for kv in limited[k].most_common()))

# ---------- образцы ----------
NATURAL = {'netherrack', 'basalt', 'blackstone', 'magma_block', 'gravel', 'soul_soil', 'nether_quartz_ore', 'nether_gold_ore',
           'crimson_nylium', 'warped_nylium', 'glowstone', 'bedrock', 'nether_wart_block', 'warped_wart_block', 'shroomlight',
           'crimson_stem', 'warped_stem', 'ancient_debris', 'polished_basalt', 'fire', 'soul_fire', 'crimson_roots',
           'warped_roots', 'nether_sprouts', 'weeping_vines', 'weeping_vines_plant', 'twisting_vines', 'twisting_vines_plant',
           'crimson_fungus', 'warped_fungus', 'brown_mushroom', 'red_mushroom'}
def classify(counter):
    """'?' — рельеф как есть, 'air' — всегда пусто, иначе блок."""
    tot = sum(counter.values()); merged = collections.Counter()
    for s, k in counter.items(): merged['air' if base(s) in ('air', 'cave_air') else base(s)] += k
    top, k = merged.most_common(1)[0]
    if top == 'air': return 'air' if k / tot >= 0.9 else '?'
    if top in NATURAL or k / tot < 0.6: return '?'
    return collections.Counter({s: n for s, n in counter.items() if base(s) == top}).most_common(1)[0][0]
SYMBOLS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
def cmd_templates(world, starts, out, count=40):
    w = World(world); fs = w.fortresses(starts, int(count))
    by = collections.defaultdict(list)
    for _, ch in fs:
        for p in ch: by[p['id']].append(p)
    lines = ['# Куски крепостей Незера — замер мира оригинала 26.1.2 (%d крепостей),' % len(fs),
             '# tools/measure/nether_fortress.py templates. Рамка куска: a — вбок, b — вперёд',
             '# от входа, y — от низа. Направления в состояниях — как у куска, смотрящего',
             '# на юг: вперёд — south, вбок (+a) — east. «.» — рельеф остаётся, «_» — пусто,',
             '# «#» — nether_bricks. supports — столбцы (a,b), что уходят кирпичом вниз до',
             '# первого сплошного блока (там, где под куском пусто, кирпич ушёл вниз',
             '# чаще, чем нет).', '']
    for pid in sorted(by):
        ps = by[pid]; o0, bb0 = ps[0]['O'], ps[0]['BB']
        W, D = dims(o0, bb0); H = bb0[4] - bb0[1] + 1
        cells = collections.defaultdict(collections.Counter); support = collections.defaultdict(collections.Counter); n = 0
        for p in ps:
            o, bb = p['O'], p['BB']; grid = {}
            for a in range(W):
                for b in range(D):
                    x, z = to_world(o, bb, a, b)
                    for y in range(H):
                        s = w.block(x, bb[1] + y, z)
                        if s is None: break
                        grid[(a, y, b)] = localize(s, o)
                    if s is None: break
                    if base(grid[(a, 0, b)]) == 'nether_bricks':
                        y = bb[1] - 1; k = 0
                        while y > 0 and base(w.block(x, y, z) or '') == 'nether_bricks': y -= 1; k += 1
                        under = base(w.block(x, y, z) or '?')
                        # кирпич ушёл вниз — опора; не ушёл, хотя снизу
                        # пусто, — не опора; стоит на сплошном — не видно
                        if k > 0: support[(a, b)]['fill'] += 1
                        elif under in ('air', 'cave_air', 'lava'): support[(a, b)]['open'] += 1
                if s is None: break
            if s is None: continue
            n += 1
            for k, v in grid.items(): cells[k][v] += 1
        palette = {'nether_bricks': '#'}; layers = []
        for y in range(H):
            rows = []
            for b in range(D):
                row = ''
                for a in range(W):
                    c = classify(cells[(a, y, b)])
                    row += '.' if c == '?' else '_' if c == 'air' else palette.setdefault(c, SYMBOLS[len(palette) - 1])
                rows.append(row)
            layers.append(rows)
        sup = sorted(k for k, v in support.items() if v['fill'] > v['open'])
        lines.append('piece %s %d %d %d %d' % (pid, W, D, H, n))
        for s, ch in palette.items():
            if ch != '#': lines.append('  %s %s' % (ch, s))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y)
            lines.extend('  ' + r for r in rows)
        lines.append('supports ' + ' '.join('%d,%d' % k for k in sup))
        lines.append('')
        print(pid, 'экземпляров', n, 'размер', W, D, H, 'опор', len(sup))
    open(out, 'w').write('\n'.join(lines))

# ---------- блоки крепостей ----------
def cmd_blocks(world, starts, count=40):
    """Сколько каких блоков в столбцах под рамками кусков крепости (все
    высоты) — на крепость. Тем же счётом меряет наш ignored-тест
    fortress_blocks (src/world/nether.rs)."""
    w = World(world); fs = w.fortresses(starts, int(count)); total = collections.Counter()
    for _, ch in fs:
        columns = collections.defaultdict(list)
        for p in ch:
            b = p['BB']
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1): columns[(x, z)].append((b[1], b[4]))
        for (x, z), spans in columns.items():
            for y in range(1, 127):
                s = w.block(x, y, z)
                if s and base(s) == 'nether_bricks' and not any(lo <= y <= hi for lo, hi in spans):
                    total['nether_bricks вне рамок (опоры)'] += 1
                if s and base(s) in ('nether_bricks', 'nether_brick_fence', 'nether_brick_stairs', 'spawner', 'chest',
                                     'nether_wart', 'soul_sand', 'lava'):
                    total[base(s)] += 1
                    if base(s) == 'nether_bricks' and y < 45: total['nether_bricks ниже y 45'] += 1
    # Опоры мостов (nebcr, nebs): где кончается кирпич под низом куска и на
    # чём стоит — тот же счёт, что «низ опор мостов» в fortress_blocks.
    ends = collections.Counter(); below = collections.Counter()
    for _, ch in fs:
        for p in ch:
            if p['id'] not in ('nebcr', 'nebs'): continue
            b = p['BB']
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1):
                    if base(w.block(x, b[1], z) or '') != 'nether_bricks': continue
                    y = b[1] - 1
                    while y > 0 and base(w.block(x, y, z) or '') == 'nether_bricks': y -= 1
                    if y < b[1] - 1:
                        ends[y // 4 * 4] += 1; below[base(w.block(x, y, z) or '?')] += 1
    print('крепостей', len(fs))
    for k, v in total.most_common(): print('  %-22s %8.1f' % (k, v / len(fs)))
    print('  низ опор мостов по высоте:', sorted(ends.items()))
    print('  под опорами:', below.most_common(8))

if __name__ == '__main__':
    what, args = sys.argv[1], sys.argv[2:]
    {'locate': cmd_locate, 'generate': cmd_generate, 'stats': cmd_stats, 'templates': cmd_templates, 'blocks': cmd_blocks}[what](*args)
