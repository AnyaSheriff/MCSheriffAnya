#!/usr/bin/env python3
# Деревни (villages) по замеру мира оригинала 26.1.2. Код игры, jar и
# выгрузки worldgen не используются: свой сервер оригинала (семя 1234567890,
# generate-structures=true) только запускается и слушает RCON, а всё
# снимается с region-файлов, которые он сложил, — блоки и записи о строениях
# в чанках (structures.starts minecraft:village_*: куски Children — имя
# образца pool_element.location, поворот rotation, рамка BB, начало
# PosX/Y/Z, проекция projection и стыки junctions).
#
#   python3 tools/measure/village.py locate <порт rcon> <пароль> <вывод.tsv>
#       — `locate structure minecraft:village_<вид>` из сетки точек;
#   python3 tools/measure/village.py generate <мир> <порт rcon> <пароль> <starts.tsv> [вид] [сколько]
#       — догружает (forceload) по 13×13 чанков вокруг деревень;
#   python3 tools/measure/village.py stats <мир> <starts.tsv>
#       — куски на деревню по образцам, поворотам, проекциям; расстояния;
#   python3 tools/measure/village.py census <мир> <starts.tsv>
#       — перепись блоков деревни (кровати, колокол, места работы…) на деревню.
import os, sys, struct, zlib, time, math, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack
from nether_fortress import Rcon, state_name

KINDS = ['plains', 'desert', 'savanna', 'taiga', 'snowy']

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

def read_starts(path, kind=None):
    out = []
    for line in open(path):
        w = line.split()
        if len(w) >= 3 and (kind is None or w[0] == kind): out.append((w[0], int(w[1]), int(w[2])))
    return out

def start_record(world, kind, cx, cz):
    root = world.chunk(cx, cz)
    if root is None: return None
    st = root.get('structures', {}).get('starts', {}).get('minecraft:village_' + kind)
    if not st or st.get('id') == 'INVALID': return None
    return st

def cmd_locate(port, password, out):
    r = Rcon(int(port), password)
    seen = set(); lines = []
    for kind in KINDS:
        for gx in range(-6, 7):
            for gz in range(-6, 7):
                x, z = gx * 1200, gz * 1200
                a = r.run('execute positioned %d 0 %d run locate structure minecraft:village_%s' % (x, z, kind))
                if '[' not in a: continue
                xs = a.split('[')[1].split(']')[0].split(',')
                bx, bz = int(xs[0]), int(xs[2])
                key = (kind, bx >> 4, bz >> 4)
                if key in seen: continue
                seen.add(key); lines.append('%s %d %d' % key)
                print(kind, bx, bz, flush=True)
    open(out, 'w').write('\n'.join(lines) + '\n')

def cmd_generate(world, port, password, starts, kind=None, count=None):
    r = Rcon(int(port), password)
    todo = read_starts(starts, kind)
    if count: todo = todo[:int(count)]
    w = World(world)
    for (k, cx, cz) in todo:
        if w.blocks(cx, cz) is not None and all(w.blocks(cx + dx, cz + dz) is not None for dx in (-6, 6) for dz in (-6, 6)):
            continue
        print('гружу', k, cx, cz, flush=True)
        r.run('forceload add %d %d %d %d' % (cx * 16 - 96, cz * 16 - 96, cx * 16 + 111, cz * 16 + 111))
        for _ in range(120):
            time.sleep(1)
            a = r.run('execute if loaded %d 64 %d if loaded %d 64 %d' % (cx * 16 - 96, cz * 16 - 96, cx * 16 + 111, cz * 16 + 111))
            if 'passed' in a.lower(): break
        time.sleep(2)
        r.run('save-all flush')
        r.run('forceload remove %d %d %d %d' % (cx * 16 - 96, cz * 16 - 96, cx * 16 + 111, cz * 16 + 111))
        w = World(world)

def pieces_of(world, kind, cx, cz):
    st = start_record(world, kind, cx, cz)
    return [] if st is None else st.get('Children', [])

def cmd_stats(world, starts):
    w = World(world)
    by_kind = collections.defaultdict(list)
    for (k, cx, cz) in read_starts(starts):
        ps = pieces_of(w, k, cx, cz)
        if ps and w.blocks(cx, cz) is not None: by_kind[k].append((cx, cz, ps))
    for k, vs in by_kind.items():
        print('==', k, len(vs), 'деревень')
        names = collections.Counter(); cats = collections.Counter(); proj = collections.Counter()
        for cx, cz, ps in vs:
            for p in ps:
                loc = p['pool_element'].get('location', p['pool_element'].get('element_type'))
                names[loc] += 1; proj[p['pool_element'].get('projection')] += 1
                cats[loc.split('/')[2] if loc.count('/') >= 3 else loc] += 1
        for name, n in cats.most_common(): print('  %-30s %6.2f' % (name, n / len(vs)))
        print('  проекции', dict(proj))
        span = []
        for cx, cz, ps in vs:
            x0 = min(p['BB'][0] for p in ps); x1 = max(p['BB'][3] for p in ps)
            z0 = min(p['BB'][2] for p in ps); z1 = max(p['BB'][5] for p in ps)
            span.append((x1 - x0 + 1, z1 - z0 + 1, len(ps), ps[0]['pool_element']['location'].split('/')[-1]))
        for s in span: print('  размах', s)

COUNT = ['white_bed', 'red_bed', 'bell', 'composter', 'smoker', 'blast_furnace', 'cartography_table', 'brewing_stand',
         'barrel', 'fletching_table', 'cauldron', 'lectern', 'stonecutter', 'loom', 'smithing_table', 'grindstone',
         'chest', 'hay_block', 'torch', 'lantern', 'wall_torch', 'oak_planks', 'cobblestone', 'dirt_path', 'glass_pane',
         'oak_log', 'stripped_oak_log', 'farmland', 'wheat', 'water', 'oak_door', 'oak_stairs', 'oak_fence', 'yellow_bed']

def cmd_census(world, starts):
    w = World(world)
    by_kind = collections.defaultdict(list)
    for (k, cx, cz) in read_starts(starts):
        ps = pieces_of(w, k, cx, cz)
        if not ps: continue
        if any(w.blocks(cx + dx, cz + dz) is None for dx in range(-6, 7) for dz in range(-6, 7)): continue
        counts = collections.Counter()
        cells = set()
        for p in ps:
            b = p['BB']
            for x in range(b[0], b[3] + 1):
                for z in range(b[2], b[5] + 1):
                    for y in range(b[1], b[4] + 1):
                        if (x, y, z) in cells: continue
                        cells.add((x, y, z))
                        n = w.block(x, y, z)
                        if n is None: continue
                        n = n.split('[')[0].replace('minecraft:', '')
                        if 'bed' in n and n.endswith('_bed'): n = n  # обе половины
                        counts[n] += 1
        by_kind[k].append(counts)
    for k, cs in by_kind.items():
        print('==', k, len(cs), 'деревень')
        total = collections.Counter()
        for c in cs: total.update(c)
        for n, v in total.most_common(400): print('  %-26s %8.1f' % (n, v / len(cs)))

# ---------- образцы и раскладки ----------
# Поворот куска: местная клетка (a, b) образца → смещение от начала куска.
ROT = {'NONE': lambda a, b: (a, b), 'CLOCKWISE_90': lambda a, b: (-b, a),
       'CLOCKWISE_180': lambda a, b: (-a, -b), 'COUNTERCLOCKWISE_90': lambda a, b: (b, -a)}
TURNS = {'NONE': 0, 'CLOCKWISE_90': 1, 'CLOCKWISE_180': 2, 'COUNTERCLOCKWISE_90': 3}
# Стороны по часовой: north → east → south → west.
SIDES = ['north', 'east', 'south', 'west']
def unrotate(state, turns):
    """Состояние в рамке образца (поворот снят)."""
    if '[' not in state or turns == 0: return state
    name, props = state[:-1].split('[')
    back = lambda d: SIDES[(SIDES.index(d) - turns) % 4]
    out = {}
    for k, v in (q.split('=') for q in props.split(',')):
        if k in SIDES: out[back(k)] = v
        elif k in ('facing', 'horizontal_facing') and v in SIDES: out[k] = back(v)
        elif k == 'axis' and v in ('x', 'z') and turns % 2 == 1: out[k] = 'z' if v == 'x' else 'x'
        elif k == 'rotation' and v.isdigit(): out[k] = str((int(v) - 4 * turns) % 16)
        else: out[k] = v
    if turns % 2 == 1 and name.endswith('_wall') and False: pass
    return name + '[' + ','.join('%s=%s' % kv for kv in sorted(out.items())) + ']'

SKIP = ('/villagers/', '/animals/', 'iron_golem', '/zombie/')
def usable(p):
    loc = p['pool_element'].get('location')
    return loc and not any(s in loc for s in SKIP)

def size_of(p):
    b = p['BB']; t = TURNS[p['rotation']]
    w, h, l = b[3] - b[0] + 1, b[4] - b[1] + 1, b[5] - b[2] + 1
    return (w, l, h) if t % 2 == 0 else (l, w, h)

def surface(w, x, z, top, bottom):
    """Высота верхнего твёрдого блока столбца (без травы, цветов, снега слоем)."""
    soft = ('air', 'cave_air', 'short_grass', 'tall_grass', 'fern', 'large_fern', 'snow', 'dandelion', 'poppy',
            'azure_bluet', 'oxeye_daisy', 'cornflower', 'sunflower', 'lilac', 'rose_bush', 'peony', 'dead_bush',
            'sweet_berry_bush', 'pumpkin', 'short_dry_grass', 'tall_dry_grass', 'bush', 'firefly_bush', 'leaf_litter', 'wildflowers')
    for y in range(top, bottom - 1, -1):
        n = w.block(x, y, z)
        if n is None: return None
        if n.split('[')[0].replace('minecraft:', '') not in soft: return y
    return None

def cmd_export(world, starts, out, kind='plains', count='12', seed='1234567890'):
    w = World(world)
    villages = []
    for (k, cx, cz) in read_starts(starts, kind):
        ps = pieces_of(w, k, cx, cz)
        if not ps: continue
        if any(w.blocks(cx + dx, cz + dz) is None for dx in range(-6, 7) for dz in range(-6, 7)): continue
        villages.append((cx, cz, ps))
        if len(villages) >= int(count): break
    grids = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    meta = {}; seen = collections.Counter(); layouts = []
    for cx, cz, ps in villages:
        lay = []
        for p in ps:
            # Стога сена (pile_hay) — особый кусок без образца: место его
            # середины; сам стог ставит наша генерация (village.rs).
            if p['pool_element'].get('feature') == 'minecraft:pile_hay':
                lay.append(('feature:pile_hay', 0, p['PosX'] - cx * 16, 0, p['PosZ'] - cz * 16))
                continue
            if not usable(p): continue
            loc = p['pool_element']['location'].replace('minecraft:village/', '')
            proj = p['pool_element'].get('projection', 'rigid')
            t = TURNS[p['rotation']]; W, L, H = size_of(p)
            meta[loc] = (W, L, H if proj == 'rigid' else 2, proj); seen[loc] += 1
            x0, y0, z0 = p['PosX'], p['PosY'], p['PosZ']
            b = p['BB']
            if proj == 'rigid':
                ring = [surface(w, x, z, b[4] + 8, b[1] - 24) for x in range(b[0] - 1, b[3] + 2) for z in (b[2] - 1, b[5] + 1)]
                ring += [surface(w, x, z, b[4] + 8, b[1] - 24) for z in range(b[2], b[5] + 1) for x in (b[0] - 1, b[3] + 1)]
                ring = sorted(v for v in ring if v is not None)
                ground = ring[len(ring) // 2] if ring else y0
                for a in range(W):
                    for bb in range(L):
                        dx, dz = ROT[p['rotation']](a, bb)
                        for y in range(H):
                            n = w.block(x0 + dx, y0 + y, z0 + dz)
                            if n is not None: grids[loc][(a, y, bb)][unrotate(n.replace('minecraft:', ''), t)] += 1
                lay.append((loc, t, x0 - cx * 16, y0 - ground, z0 - cz * 16))
            else:
                for a in range(W):
                    for bb in range(L):
                        dx, dz = ROT[p['rotation']](a, bb)
                        top = surface(w, x0 + dx, z0 + dz, b[4] + 4, b[1] - 8)
                        if top is None: continue
                        for y in range(2):
                            n = w.block(x0 + dx, top + y, z0 + dz)
                            if n is not None: grids[loc][(a, y, bb)][unrotate(n.replace('minecraft:', ''), t)] += 1
                lay.append((loc, t, x0 - cx * 16, 0, z0 - cz * 16))
        layouts.append(lay)
    lines = ['# Деревни (%s) — замер мира оригинала 26.1.2 (%d деревень, семя %s),' % (kind, len(villages), seed),
             '# tools/measure/village.py export. Образец: рамка в его собственном повороте',
             '# (a — x, b — z, y — от низа); «.» — рельеф остаётся, «_» — пусто. У образцов',
             '# terrain_matching (улицы) два слоя: верх рельефа столбца и над ним.',
             '# layout — раскладка одной деревни: образец, поворот (четверти по часовой),',
             '# начало куска от угла чанка старта по x и z и высота над землёй вокруг.', '']
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
                    total = sum(c.values())
                    if n * 3 < total * 2 and seen[loc] > 1: row += '.'
                    elif proj != 'rigid' and state.split('[')[0] in NATURAL + ('air', 'cave_air'): row += '.'
                    elif state.split('[')[0] in ('air', 'cave_air'): row += '_'
                    # У улиц рельеф вокруг дорожки — не часть образца.
                    elif proj != 'rigid' and (y > 0 and state.split('[')[0] in ('air', 'cave_air') or state.split('[')[0] in NATURAL): row += '.'
                    else: row += sym(state)
                rows.append(row)
            layers.append(rows)
        if len(pal) > len(SYMBOLS): print('много состояний', loc, file=sys.stderr); continue
        lines.append('template %s %d %d %d %s %d' % (loc, W, L, H, proj, seen[loc]))
        for state, ch in pal.items(): lines.append('  %s %s' % (ch, state))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y)
            lines += ['  ' + r for r in rows]
        lines.append('')
    for lay in layouts:
        lines.append('layout %d' % len(lay))
        for loc, t, dx, dy, dz in lay: lines.append('  %s %d %d %d %d' % (loc, t, dx, dy, dz))
        lines.append('')
    open(out, 'w').write('\n'.join(lines) + '\n')
    print(len(villages), 'деревень,', len(meta), 'образцов')

SYMBOLS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!$%&*+-<=>?@^~|;:,/'
NATURAL = ('grass_block', 'dirt', 'coarse_dirt', 'sand', 'red_sand', 'sandstone', 'stone', 'gravel', 'water', 'snow', 'snow_block',
           'short_grass', 'tall_grass', 'clay', 'andesite', 'diorite', 'granite', 'podzol', 'mud', 'ice', 'powder_snow', 'terracotta')

if __name__ == '__main__':
    a = sys.argv[1:]
    {'locate': lambda: cmd_locate(*a[1:4]), 'generate': lambda: cmd_generate(*a[1:]), 'stats': lambda: cmd_stats(*a[1:3]),
     'census': lambda: cmd_census(*a[1:3]), 'export': lambda: cmd_export(*a[1:])}[a[0]]()
