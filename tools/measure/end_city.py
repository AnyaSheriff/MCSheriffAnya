#!/usr/bin/env python3
# Энд-города по замеру мира оригинала 26.1.2. Код игры, jar и выгрузки
# worldgen не используются: сервер оригинала только запускается (свой мир)
# и слушает RCON, а всё снимается с region-файлов Энда, которые он сложил, —
# блоки и записи о строениях в чанках (structures.starts minecraft:end_city:
# у куска имя образца Template, поворот Rot, точка образца TPX/TPY/TPZ, рамка BB).
#
#   python3 tools/measure/end_city.py locate <порт rcon> <пароль> <радиус в блоках> <вывод.tsv>
#       — `locate structure minecraft:end_city` из точек сетки 320 блоков;
#   python3 tools/measure/end_city.py generate <мир> <порт rcon> <пароль> <starts.tsv> [сколько]
#       — догружает (forceload) по 12×12 чанков вокруг городов;
#   python3 tools/measure/end_city.py stats <мир> [мир…]
#       — куски на город, этажность, башни, мосты;
#   python3 tools/measure/end_city.py templates <вывод.txt> <мир> [мир…]
#       — блоки каждого образца по всем его экземплярам (src/world/end_city.txt);
#   python3 tools/measure/end_city.py blocks <мир> [мир…]
#       — перепись блоков городов и хоруса на внешних островах (сверка с нашим
#         тестом end_city_census в src/world/end_city.rs).
#
# Рамка образца: a — вбок (+x у поворота NONE), b — вдоль (+z у NONE), y — от
# TPY. Повороты: NONE (a,b)→(+x,+z); CLOCKWISE_90 (a,b)→(−b,+a);
# CLOCKWISE_180 → (−a,−b); COUNTERCLOCKWISE_90 → (+b,−a); точка образца —
# угол рамки (проверено по всем экземплярам). Мосты идут от двери в сторону −b.
import os, sys, struct, zlib, re, time, glob, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack
from nether_fortress import Rcon, state_name, base, SYMBOLS

ROT = ['NONE', 'CLOCKWISE_90', 'CLOCKWISE_180', 'COUNTERCLOCKWISE_90']

class World:
    def __init__(self, world):
        self.reg = os.path.join(world, 'dimensions/minecraft/the_end/region')
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
                    if not 0 <= sy <= 15 or not bs: continue
                    pal = [state_name(e) for e in bs['palette']]
                    idx = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                    for ly in range(16): grid[sy * 16 + ly] = [pal[i] for i in idx[ly * 256:(ly + 1) * 256]]
            self.grids[k] = grid
            if len(self.grids) > 400: self.grids.pop(next(iter(self.grids)))
        return self.grids[k]
    def block(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        if g is None: return None
        if y not in g: return 'air'
        return g[y][(z & 15) * 16 + (x & 15)]
    def cities(self):
        """Все города мира: (чанк старта, куски) — только полностью сложенные."""
        out = []
        for p in sorted(glob.glob(self.reg + '/r.*.mca')):
            data = open(p, 'rb').read()
            for i in range(1024):
                off = int.from_bytes(data[i*4:i*4+3], 'big')
                if not off: continue
                q = off * 4096; ln = struct.unpack_from('>i', data, q)[0]
                try: raw = zlib.decompress(data[q+5:q+4+ln])
                except zlib.error: continue
                n = struct.unpack_from('>H', raw, 1)[0]
                root = nbt(raw, 3 + n, 10)[0]
                st = root.get('structures', {}).get('starts', {}).get('minecraft:end_city')
                if st and st.get('Children'):
                    ch = st['Children']
                    if all(self.full(x >> 4, z >> 4) for p_ in ch for x, z in ((p_['BB'][0], p_['BB'][2]), (p_['BB'][3], p_['BB'][5]))):
                        out.append(((root['xPos'], root['zPos']), ch))
        return out

def all_cities(worlds):
    out = []
    for w in worlds:
        W = World(w); out += [(W, ch) for _, ch in W.cities()]
    return out

# ---------- рамка ----------
def to_world(p, a, b):
    tx, tz = p['TPX'], p['TPZ']
    return {'NONE': (tx + a, tz + b), 'CLOCKWISE_90': (tx - b, tz + a),
            'CLOCKWISE_180': (tx - a, tz - b), 'COUNTERCLOCKWISE_90': (tx + b, tz - a)}[p['Rot']]
def to_local(p, x, z):
    dx, dz = x - p['TPX'], z - p['TPZ']
    return {'NONE': (dx, dz), 'CLOCKWISE_90': (dz, -dx), 'CLOCKWISE_180': (-dx, -dz), 'COUNTERCLOCKWISE_90': (-dz, dx)}[p['Rot']]
def size(p):
    b = p['BB']; w, h, d = b[3] - b[0] + 1, b[4] - b[1] + 1, b[5] - b[2] + 1
    return (d, h, w) if p['Rot'] in ('CLOCKWISE_90', 'COUNTERCLOCKWISE_90') else (w, h, d)
def relrot(a, b): return (ROT.index(b['Rot']) - ROT.index(a['Rot'])) % 4
DIRS = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}
def localize(state, rot):
    """Состояние с направлениями в рамке образца (поворот снят)."""
    if '[' not in state or rot == 'NONE': return state
    name, props = state[:-1].split('[')
    turns = {'CLOCKWISE_90': 3, 'CLOCKWISE_180': 2, 'COUNTERCLOCKWISE_90': 1}[rot]
    ring = ['north', 'east', 'south', 'west']
    def ld(d): return ring[(ring.index(d) + turns) % 4]
    out = {}
    for k, v in (p.split('=') for p in props.split(',')):
        if k in DIRS: out[ld(k)] = v
        elif k == 'facing' and v in DIRS: out[k] = ld(v)
        elif k == 'rotation': out[k] = str((int(v) + 4 * turns) % 16)
        else: out[k] = v
    return name + '[' + ','.join('%s=%s' % kv for kv in sorted(out.items())) + ']'

# ---------- locate / generate ----------
def cmd_locate(port, password, radius, out):
    r = Rcon(int(port), password); radius = int(radius); found = set()
    for x in range(-radius, radius + 1, 320):
        for z in range(-radius, radius + 1, 320):
            if x * x + z * z < 900 * 900: continue
            m = re.search(r'\[(-?\d+), ~, (-?\d+)\]', r.run('execute in minecraft:the_end positioned %d 64 %d run locate structure minecraft:end_city' % (x, z)))
            if m: found.add((int(m.group(1)), int(m.group(2))))
    with open(out, 'w') as f:
        for x, z in sorted(found): f.write('%d\t%d\n' % (x, z))
    print('городов', len(found))

def cmd_generate(world, port, password, starts, count=1000):
    w = World(world); r = Rcon(int(port), password); within = 'execute in minecraft:the_end run '
    rows = [tuple(map(int, l.split('\t'))) for l in open(starts).read().split('\n') if l][:int(count)]
    for n, (x, z) in enumerate(rows):
        cx, cz = x // 16, z // 16
        area = [(i, j) for i in range(cx - 6, cx + 7) for j in range(cz - 6, cz + 7)]
        box = '%d %d %d %d' % ((cx - 6) * 16, (cz - 6) * 16, (cx + 6) * 16 + 15, (cz + 6) * 16 + 15)
        if not all(w.full(*c) for c in area):
            r.run(within + 'forceload add ' + box)
            while True:
                time.sleep(5); r.run('save-all'); w.regions.clear()
                if all(w.full(*c) for c in area): break
            r.run(within + 'forceload remove ' + box)
        print(n, x, z, flush=True)
    r.run('save-all')

# ---------- раскладка ----------
HOUSE = ('second_floor_1', 'second_floor_2', 'third_floor_1', 'third_floor_2', 'second_roof', 'third_roof', 'base_roof')
BRIDGE = ('bridge_piece', 'bridge_steep_stairs', 'bridge_gentle_stairs')
def cmd_stats(*worlds):
    cities = all_cities(worlds)
    print('городов', len(cities))
    per = collections.Counter(); kinds = collections.Counter(); rooty = collections.Counter()
    stories = collections.Counter(); tower = collections.Counter(); after_tower = collections.Counter(); fat = collections.Counter()
    chain = collections.Counter(); chain_end = collections.Counter(); parts = collections.Counter(); spread = []
    for _, ch in cities:
        per[len(ch)] += 1; rooty[ch[0]['TPY']] += 1; n = len(ch)
        spread.append(max(max(p['BB'][3] - ch[0]['BB'][0], ch[0]['BB'][3] - p['BB'][0], p['BB'][5] - ch[0]['BB'][2], ch[0]['BB'][5] - p['BB'][2]) for p in ch))
        for p in ch: kinds[p['Template']] += 1
        i = 0
        while i < n:
            p = ch[i]
            if p['Template'] == 'base_floor':
                k = 1; j = i + 1
                while j < n and ch[j]['Template'] in HOUSE:
                    k += 'floor' in ch[j]['Template']; j += 1
                stories[('корень' if i == 0 else 'дальше', k)] += 1
                if j < n and ch[j]['Template'] == 'tower_base':
                    m = j + 1; cnt = 0
                    while m < n and ch[m]['Template'] == 'tower_piece': cnt += 1; m += 1
                    tower[cnt] += 1
                    if m < n and ch[m]['Template'] == 'fat_tower_base':
                        after_tower['fat'] += 1; q = m + 1; fm = 0
                        while q < n and ch[q]['Template'] == 'fat_tower_middle': fm += 1; q += 1
                        fat[fm] += 1
                    else: after_tower['top'] += 1
                i = j; continue
            if p['Template'] == 'bridge_end':
                j = i + 1; k = 0
                while j < n and ch[j]['Template'] in BRIDGE and ch[j]['Rot'] == p['Rot']: parts[ch[j]['Template']] += 1; k += 1; j += 1
                if k: chain[k] += 1; chain_end[ch[j]['Template'] if j < n else '-'] += 1
                i = j; continue
            i += 1
    print('кусков на город:', sorted(per.items()), 'среднее %.1f' % (sum(k * v for k, v in per.items()) / len(cities)))
    print('размах от старта, блоков: среднее %.0f, наибольший %d' % (sum(spread) / len(spread), max(spread)))
    print('виды:', sorted(kinds.items()))
    print('высота старта:', sorted(rooty.items()))
    print('этажей (корень/дальше):', sorted(stories.items()))
    print('кусков башни:', sorted(tower.items()), 'после башни:', dict(after_tower), 'середин толстой:', sorted(fat.items()))
    print('длина моста:', sorted(chain.items()), 'куски:', dict(parts), 'конец моста:', dict(chain_end))

# ---------- образцы ----------
NATURAL = {'end_stone', 'chorus_plant', 'chorus_flower'}
def classify(counter):
    tot = sum(counter.values()); merged = collections.Counter()
    for s, k in counter.items(): merged['air' if base(s) in ('air', 'cave_air') else base(s)] += k
    top, k = merged.most_common(1)[0]
    if top == 'air': return 'air' if k / tot >= 0.8 else '?'
    if top in NATURAL or k / tot < 0.6: return '?'
    return collections.Counter({s: n for s, n in counter.items() if base(s) == top}).most_common(1)[0][0]
def cmd_templates(out, *worlds):
    cities = all_cities(worlds)
    by = collections.defaultdict(list)
    for w, ch in cities:
        for p in ch: by[p['Template']].append((w, p))
    lines = ['# Куски Энд-городов — замер мира оригинала 26.1.2 (%d городов),' % len(cities),
             '# tools/measure/end_city.py templates. Рамка: a — вбок, b — вдоль, y — от низа;',
             '# направления в состояниях — как у образца без поворота. «.» — рельеф',
             '# остаётся, «_» — пусто, «#» — purpur_block.', '']
    for name in sorted(by):
        ps = by[name]; W, H, D = size(ps[0][1])
        cells = collections.defaultdict(collections.Counter); n = 0
        for w, p in ps:
            for a in range(W):
                for b in range(D):
                    x, z = to_world(p, a, b)
                    for y in range(H):
                        s = w.block(x, p['TPY'] + y, z)
                        cells[(a, y, b)][localize(s, p['Rot'])] += 1
            n += 1
        palette = {'purpur_block': '#'}; layers = []
        for y in range(H):
            rows = []
            for b in range(D):
                row = ''
                for a in range(W):
                    c = classify(cells[(a, y, b)])
                    row += '.' if c == '?' else '_' if c == 'air' else palette.setdefault(c, SYMBOLS[len(palette) - 1])
                rows.append(row)
            layers.append(rows)
        lines.append('piece %s %d %d %d %d' % (name, W, D, H, n))
        for s, ch in palette.items():
            if ch != '#': lines.append('  %s %s' % (ch, s))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y); lines.extend('  ' + r for r in rows)
        lines.append('')
        print(name, 'экземпляров', n, 'размер', W, D, H, 'палитра', len(palette))
    open(out, 'w').write('\n'.join(lines))

# ---------- перепись ----------
CITY = ('purpur_block', 'purpur_pillar', 'purpur_stairs', 'purpur_slab', 'end_stone_bricks', 'magenta_stained_glass',
        'end_rod', 'chest', 'obsidian', 'ladder', 'dragon_head', 'brewing_stand', 'magenta_wall_banner', 'magenta_banner')
def cmd_blocks(*worlds):
    """Блоки городов на столбец внешних островов (дальше 1000 от центра) и
    число городов на регион 20×20 чанков — тот же счёт, что end_city_census."""
    for world in worlds:
        w = World(world); total = collections.Counter(); columns = 0; chunks = 0; regions = set(); cities = set()
        for p in sorted(glob.glob(w.reg + '/r.*.mca')):
            rx, rz = map(int, os.path.basename(p)[2:-4].split('.'))
            for cx in range(rx * 32, rx * 32 + 32):
                for cz in range(rz * 32, rz * 32 + 32):
                    if (cx * 16 + 8) ** 2 + (cz * 16 + 8) ** 2 < 1000 ** 2: continue
                    g = w.blocks(cx, cz)
                    if g is None: continue
                    chunks += 1; regions.add((cx // 20, cz // 20))
                    root = w.chunk(cx, cz)
                    if root.get('structures', {}).get('starts', {}).get('minecraft:end_city', {}).get('Children'): cities.add((cx, cz))
                    stone = set()
                    for y, row in g.items():
                        for i, s in enumerate(row):
                            b = base(s)
                            if b == 'end_stone': stone.add(i)
                            elif b in CITY or b.startswith('chorus'): total[b] += 1
                    columns += len(stone)
            w.grids.clear()
        print(world, 'чанков', chunks, 'столбцов с эндемняком', columns, 'регионов', len(regions), 'городов', len(cities))
        for k, v in total.most_common(): print('  %-22s %8d  на 10⁴ столбцов %.1f' % (k, v, v * 1e4 / max(columns, 1)))

if __name__ == '__main__':
    what, args = sys.argv[1], sys.argv[2:]
    {'locate': cmd_locate, 'generate': cmd_generate, 'stats': cmd_stats, 'templates': cmd_templates, 'blocks': cmd_blocks}[what](*args)
