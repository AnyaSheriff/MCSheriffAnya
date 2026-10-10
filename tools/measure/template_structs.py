#!/usr/bin/env python3
# Строения из одного образца (разрушенные порталы, кораблекрушения, руины в
# океане, руины тропы) по замеру мира оригинала 26.1.2. Код игры, jar и
# выгрузки worldgen не используются: свой сервер оригинала (обычный мир,
# generate-structures=true) только запускается и слушает RCON, а всё
# снимается с region-файлов, которые он сложил, — блоки, сущности блоков и
# записи о строениях в чанках (structures.starts minecraft:<строение>:
# куски Children — образец Template, поворот Rot, отражение Mirror, точка
# образца TPX/TPY/TPZ, рамка BB и поля строения).
#
#   python3 tools/measure/template_structs.py locate <порт rcon> <пароль> <строение> <радиус> <шаг> <вывод.tsv> [измерение]
#       — `locate structure` из точек сетки; дописывает найденное в вывод;
#   python3 tools/measure/template_structs.py generate <мир> <порт rcon> <пароль> <starts.tsv> <запас в чанках> [измерение]
#       — догружает (forceload) чанки вокруг найденных стартов, ждёт полных;
#   python3 tools/measure/template_structs.py records <мир> <starts.tsv> [измерение]
#       — записи о строениях (куски и поля) у стартов;
#   python3 tools/measure/template_structs.py dump <мир> <starts.tsv> <вывод.json> [измерение]
#       — по каждому куску: блоки рамки (+запас) в координатах образца и
#         сущности блоков (сундуки с LootTable, подозрительный песок), — для
#         сведения образцов (tools/measure/template_merge.py).
#
# Координаты образца: a, b — в образце (отражение, потом поворот), y — от TPY.
# Повороты (как в end_city.py): NONE (a,b)→(+x,+z); CLOCKWISE_90 → (−b,+a);
# CLOCKWISE_180 → (−a,−b); COUNTERCLOCKWISE_90 → (+b,−a) от точки образца.
# Отражение LEFT_RIGHT — b → −b, FRONT_BACK — a → −a (до поворота).
import os, sys, struct, zlib, re, time, json, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack
from nether_fortress import Rcon, state_name

ROT = ['NONE', 'CLOCKWISE_90', 'CLOCKWISE_180', 'COUNTERCLOCKWISE_90']

class World:
    def __init__(self, world, dim='overworld'):
        self.root = world
        self.reg = os.path.join(world, 'dimensions/minecraft/%s/region' % dim)
        self.low = -4 if dim == 'overworld' else 0
        self.high = 19 if dim == 'overworld' else 15
        self.regions = {}; self.grids = {}
    def region(self, rx, rz):
        k = (rx, rz)
        if k not in self.regions:
            p = os.path.join(self.reg, 'r.%d.%d.mca' % k)
            self.regions[k] = open(p, 'rb').read() if os.path.exists(p) else b''
            if len(self.regions) > 16: self.regions.pop(next(iter(self.regions)))
        return self.regions[k]
    def chunk(self, cx, cz):
        data = self.region(cx >> 5, cz >> 5)
        if not data: return None
        i = (cx & 31) + (cz & 31) * 32
        off = int.from_bytes(data[i*4:i*4+3], 'big')
        if off == 0 or off * 4096 + 5 > len(data): return None
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
                    if not self.low <= sy <= self.high or not bs: continue
                    pal = [state_name(e) for e in bs['palette']]
                    idx = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                    for ly in range(16): grid[sy * 16 + ly] = [pal[i] for i in idx[ly * 256:(ly + 1) * 256]]
                grid['be'] = {(e['x'], e['y'], e['z']): e for e in root.get('block_entities', [])}
            self.grids[k] = grid
            if len(self.grids) > 800: self.grids.pop(next(iter(self.grids)))
        return self.grids[k]
    def block(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        if g is None: return None
        if y not in g: return 'air'
        return g[y][(z & 15) * 16 + (x & 15)]
    def entity(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        return None if g is None else g['be'].get((x, y, z))

def read_starts(path):
    out = []
    for line in open(path):
        w = line.split()
        if len(w) >= 3: out.append((w[0], int(w[1]), int(w[2])))
    return out

def cmd_locate(port, password, structure, radius, step, out, dim='overworld'):
    r = Rcon(int(port), password); radius = int(radius); step = int(step)
    known = set(read_starts(out)) if os.path.exists(out) else set()
    found = set()
    for x in range(-radius, radius + 1, step):
        for z in range(-radius, radius + 1, step):
            text = r.run('execute in minecraft:%s positioned %d 64 %d run locate structure minecraft:%s' % (dim, x, z, structure))
            m = re.search(r'\[(-?\d+), ~, (-?\d+)\]', text)
            if m: found.add((structure, int(m.group(1)), int(m.group(2))))
    with open(out, 'a') as f:
        for s, x, z in sorted(found - known): f.write('%s\t%d\t%d\n' % (s, x, z))
    print(structure, 'найдено', len(found), 'новых', len(found - known))

def cmd_generate(world, port, password, starts, margin, dim='overworld', batch=8, limit=100000):
    w = World(world, dim); r = Rcon(int(port), password); margin = int(margin); batch = int(batch)
    rows = sorted(set((x >> 4, z >> 4) for _, x, z in read_starts(starts)), key=lambda p: p[0] ** 2 + p[1] ** 2)[:int(limit)]
    within = 'execute in minecraft:%s run ' % dim
    area = lambda cx, cz: [(i, j) for i in range(cx - margin, cx + margin + 1) for j in range(cz - margin, cz + margin + 1)]
    box = lambda cx, cz: '%d %d %d %d' % ((cx - margin) * 16, (cz - margin) * 16, (cx + margin) * 16 + 15, (cz + margin) * 16 + 15)
    todo = [c for c in rows if not all(w.full(*a) for a in area(*c))]
    started = time.time()
    # Кусками по `batch` стартов: сервер оригинала складывает чанки в
    # несколько потоков, пока ждём весь кусок.
    for at in range(0, len(todo), batch):
        part = todo[at:at + batch]
        for c in part: r.run(within + 'forceload add ' + box(*c))
        while True:
            time.sleep(4); r.run('save-all'); w.regions.clear(); w.grids.clear()
            if all(w.full(*a) for c in part for a in area(*c)): break
        for c in part: r.run(within + 'forceload remove ' + box(*c))
        print('%d/%d %.0f с' % (at + len(part), len(todo), time.time() - started), flush=True)
    r.run('save-all flush')

def records(w, starts):
    """Записи о строениях у стартов: (имя строения, чанк, запись)."""
    seen = set(); out = []
    for s, x, z in starts:
        cx, cz = x >> 4, z >> 4
        for i in range(cx - 1, cx + 2):
            for j in range(cz - 1, cz + 2):
                root = w.chunk(i, j)
                if not root: continue
                for name, st in root.get('structures', {}).get('starts', {}).items():
                    if st.get('id') == 'INVALID' or (name, i, j) in seen: continue
                    if name.replace('minecraft:', '') != s: continue
                    seen.add((name, i, j)); out.append((name, i, j, st))
    return out

def cmd_records(world, starts, dim='overworld'):
    w = World(world, dim)
    for name, cx, cz, st in records(w, read_starts(starts)):
        for p in st['Children']:
            extra = {k: v for k, v in p.items() if k not in ('BB', 'id')}
            print(name, cx, cz, p['id'], p['BB'], json.dumps(extra, default=str, ensure_ascii=False))

# Повороты и отражения: прямое преобразование образца в мир — сперва
# отражение (LEFT_RIGHT: b → −b, FRONT_BACK: a → −a), потом поворот по
# часовой (CLOCKWISE_90: (a, b) → (−b, a)). Начало образца в мире не
# снимается (у оригинала поворот — вокруг своей точки), а находится: образец
# начинается в углу рамки, так что после обратного преобразования вычитается
# наименьшее по углам рамки.
def forward(rot, mirror, a, b):
    if mirror == 'LEFT_RIGHT': b = -b
    elif mirror == 'FRONT_BACK': a = -a
    return [(a, b), (-b, a), (-a, -b), (b, -a)][rot]

def inverse(rot, mirror, x, z):
    a, b = [(x, z), (z, -x), (-x, -z), (-z, x)][rot]
    if mirror == 'LEFT_RIGHT': b = -b
    elif mirror == 'FRONT_BACK': a = -a
    return a, b

def local_of(p):
    """Перевод мировых x, z в координаты образца куска (a, b)."""
    rot = ROT.index(p.get('Rotation', p.get('Rot', 'NONE'))); mirror = p.get('Mirror', 'NONE')
    bb = p['BB']
    corners = [inverse(rot, mirror, x, z) for x in (bb[0], bb[3]) for z in (bb[2], bb[5])]
    a0 = min(c[0] for c in corners); b0 = min(c[1] for c in corners)
    def f(x, z):
        a, b = inverse(rot, mirror, x, z)
        return a - a0, b - b0
    return f

def cmd_dump(world, starts, out, dim='overworld', margin=6):
    w = World(world, dim); margin = int(margin); pieces = []
    for name, cx, cz, st in records(w, read_starts(starts)):
        for p in st['Children']:
            bb = p['BB']
            # Кусок мозаики (jigsaw): образец — pool_element.location, поворот — rotation.
            if 'pool_element' in p:
                p = dict(p); p['Template'] = p['pool_element'].get('location', ''); p['Rotation'] = p.get('rotation', 'NONE'); p['TPX'] = p.get('PosX')
            if not all(w.full(i, j) for i in range((bb[0] - margin) >> 4, ((bb[3] + margin) >> 4) + 1) for j in range((bb[2] - margin) >> 4, ((bb[5] + margin) >> 4) + 1)):
                print('не сложено', name, cx, cz, file=sys.stderr); continue
            f = local_of(p) if 'TPX' in p else None
            y0 = bb[1]
            cells = []; ents = []
            for x in range(bb[0] - margin, bb[3] + margin + 1):
                for z in range(bb[2] - margin, bb[5] + margin + 1):
                    for y in range(bb[1] - margin, bb[4] + margin + 1):
                        s = w.block(x, y, z)
                        if s is None: continue
                        a, b = f(x, z) if f else (x - bb[0], z - bb[2])
                        cells.append((a, y - y0, b, x, y, z, s))
                        e = w.entity(x, y, z)
                        if e: ents.append({'x': x, 'y': y, 'z': z, 'a': a, 'b': b, 'ly': y - y0, 'id': e.get('id'), 'loot': e.get('LootTable'), 'item': e.get('item')})
            # Земля по столбцам рамки и запаса: верх твёрдого (не воздух, не
            # жидкость, не листва и не трава) и верх непустого — для высоты.
            heights = []
            soft = re.compile(r'^(air|cave_air|void_air|water|lava|.*leaves|short_grass|tall_grass|fern|large_fern|seagrass|tall_seagrass|kelp|kelp_plant|snow|vine|.*_flower|dandelion|poppy|bubble_column)$')
            for x in range(bb[0] - margin, bb[3] + margin + 1):
                for z in range(bb[2] - margin, bb[5] + margin + 1):
                    ground = top = None
                    for y in range(w.high * 16 + 15, w.low * 16 - 1, -1):
                        s = w.block(x, y, z)
                        if s is None: break
                        base = s.split('[')[0]
                        if top is None and base not in ('air', 'cave_air', 'void_air'): top = y
                        if not soft.match(base): ground = y; break
                    heights.append((x, z, ground, top))
            pieces.append({'structure': name, 'chunk': [cx, cz], 'piece': {k: v for k, v in p.items()}, 'cells': cells, 'entities': ents, 'heights': heights})
            print(name, cx, cz, p.get('Template'), len(cells), flush=True)
    json.dump(pieces, open(out, 'w'), default=str)

if __name__ == '__main__' and sys.argv[1] != 'layouts':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'locate': cmd_locate, 'generate': cmd_generate, 'records': cmd_records, 'dump': cmd_dump}[cmd](*args)

def cmd_layouts(world, starts, out, dim='overworld'):
    """Строения из многих кусков (руины тропы): каждое — своим образцом.
    Клетки — все непустые блоки в рамках кусков (руины «completely solid»,
    вики), подозрительный гравий — с таблицей; земля вокруг — по столбцам."""
    import json as _json
    w = World(world, dim); layouts = []
    by = collections.defaultdict(list)
    for name, cx, cz, st in records(w, read_starts(starts)):
        by[(name, cx, cz)].extend(st['Children'])
    for (name, cx, cz), pieces in sorted(by.items()):
        bbs = [p['BB'] for p in pieces]
        x0 = min(b[0] for b in bbs); y0 = min(b[1] for b in bbs); z0 = min(b[2] for b in bbs)
        x1 = max(b[3] for b in bbs); y1 = max(b[4] for b in bbs); z1 = max(b[5] for b in bbs)
        if not all(w.full(i, j) for i in range((x0 - 4) >> 4, ((x1 + 4) >> 4) + 1) for j in range((z0 - 4) >> 4, ((z1 + 4) >> 4) + 1)):
            print('не сложено', name, cx, cz, file=sys.stderr); continue
        cells = {}; loot = {}
        for b in bbs:
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1):
                    for y in range(b[1], b[4] + 1):
                        s = w.block(x, y, z)
                        if s is None or s.split('[')[0] in ('air', 'cave_air', 'water'): continue
                        cells[(x - x0, y - y0, z - z0)] = s
                        e = w.entity(x, y, z)
                        if e and e.get('LootTable'): loot[(x - x0, y - y0, z - z0)] = e['LootTable']
        soft = re.compile(r'^(air|cave_air|water|.*leaves|short_grass|tall_grass|fern|large_fern|snow|vine|.*_flower|dandelion|poppy|sweet_berry_bush|.*_log|.*_wood)$')
        ground = {}
        for x in range(x0 - 4, x1 + 5):
            for z in range(z0 - 4, z1 + 5):
                for y in range(w.high * 16 + 15, w.low * 16 - 1, -1):
                    s = w.block(x, y, z)
                    if s is None: break
                    if not soft.match(s.split('[')[0]): ground[(x - x0, z - z0)] = y; break
        start = [p for p in pieces if 'tower/tower' in p.get('pool_element', {}).get('location', '')]
        layouts.append({'structure': name, 'chunk': [cx, cz], 'origin': [x0, y0, z0], 'size': [x1 - x0 + 1, y1 - y0 + 1, z1 - z0 + 1],
                        'cells': [[k[0], k[1], k[2], v] for k, v in cells.items()], 'loot': [[k[0], k[1], k[2], v] for k, v in loot.items()],
                        'ground': [[k[0], k[1], v] for k, v in ground.items()], 'pieces': [[p['BB'], p.get('pool_element', {}).get('location')] for p in pieces]})
        print(name, cx, cz, len(pieces), len(cells), flush=True)
    _json.dump(layouts, open(out, 'w'))

if __name__ == '__main__' and sys.argv[1] == 'layouts':
    cmd_layouts(*sys.argv[2:])
