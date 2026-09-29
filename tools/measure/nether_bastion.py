#!/usr/bin/env python3
# Бастионы Незера по замеру мира оригинала 26.1.2. Код игры, jar и выгрузки
# worldgen не используются: сервер оригинала только складывает мир (свой,
# generate-structures=true; догрузка — nether_fortress.py generate), а всё
# остальное снимается с region-файлов — блоки и записи о строениях в чанке
# старта (structures.starts: у каждого куска имя образца pool_element,
# точка PosX/PosY/PosZ, поворот rotation и рамка BB).
#
#   python3 tools/measure/nether_bastion.py stats <мир> <starts.tsv> [сколько]
#       — виды бастионов, число кусков, частоты образцов, высоты, размах;
#   python3 tools/measure/nether_bastion.py templates <мир> <starts.tsv> <вывод.txt> [сколько] [мир без строений]
#       — образцы кусков в своём повороте (NONE) по всем экземплярам и
#         раскладки целых бастионов. Вывод — src/world/nether_bastion.txt.
#
# Поворот образца (замер по рамкам BB): клетка (x, y, z) образца встаёт в
#   NONE (P+x, P+z), CLOCKWISE_90 (P−z, P+x), CLOCKWISE_180 (P−x, P−z),
#   COUNTERCLOCKWISE_90 (P+z, P−x).
# Клетку, которую позже перекрыл другой кусок того же бастиона, не считаем:
# там лежит уже его блок. Воздух образца от «рельеф остаётся» отличается по
# миру того же семени без строений (vanilla-big): где там был камень, а стал
# воздух — образец режет; где камень остался — рельеф; где сверить не с чем
# (там и без строения пусто или мир без строений не догружен) — режет.
import os, sys, struct, zlib, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_census import nbt, unpack

ROT = ['NONE', 'CLOCKWISE_90', 'CLOCKWISE_180', 'COUNTERCLOCKWISE_90']
DIRS = ['north', 'east', 'south', 'west']


def to_world(rot, px, pz, x, z):
    if rot == 0: return px + x, pz + z
    if rot == 1: return px - z, pz + x
    if rot == 2: return px - x, pz - z
    return px + z, pz - x


class World:
    def __init__(self, path):
        self.reg = os.path.join(os.path.expanduser(path), 'dimensions/minecraft/the_nether/region')
        self.files = {}
        self.grids = {}

    def chunk(self, cx, cz):
        k = (cx >> 5, cz >> 5)
        if k not in self.files:
            p = os.path.join(self.reg, 'r.%d.%d.mca' % k)
            self.files[k] = open(p, 'rb').read() if os.path.exists(p) else b''
            if len(self.files) > 16: self.files.pop(next(iter(self.files)))
        data = self.files[k]
        if not data: return None
        i = (cx & 31) + (cz & 31) * 32
        off = int.from_bytes(data[i*4:i*4+3], 'big')
        if off == 0 or off * 4096 + 5 > len(data): return None
        q = off * 4096; ln = struct.unpack_from('>i', data, q)[0]
        try: raw = zlib.decompress(data[q+5:q+4+ln])
        except zlib.error: return None
        n = struct.unpack_from('>H', raw, 1)[0]
        return nbt(raw, 3 + n, 10)[0]

    def blocks(self, cx, cz):
        k = (cx, cz)
        if k in self.grids: return self.grids[k]
        root = self.chunk(cx, cz)
        grid = None
        if root is not None and root.get('Status') in ('minecraft:full', 'full'):
            grid = {}
            for sec in root.get('sections', []):
                sy = sec.get('Y', 0)
                if sy > 127: sy -= 256
                if not 0 <= sy <= 7: continue
                bs = sec.get('block_states')
                if not bs: continue
                pal = [name(e) for e in bs['palette']]
                idx = [0] * 4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                for ly in range(16):
                    grid[sy * 16 + ly] = [pal[i] for i in idx[ly * 256:(ly + 1) * 256]]
        self.grids[k] = grid
        if len(self.grids) > 600: self.grids.pop(next(iter(self.grids)))
        return grid

    def block(self, x, y, z):
        g = self.blocks(x >> 4, z >> 4)
        if g is None or y not in g: return None
        return g[y][(z & 15) * 16 + (x & 15)]


def name(e):
    n = e['Name'].replace('minecraft:', '')
    props = e.get('Properties')
    if props: n += '[' + ','.join('%s=%s' % kv for kv in sorted(props.items())) + ']'
    return n


def turn(state, quarter):
    """Состояние, повёрнутое на quarter четвертей по часовой."""
    if '[' not in state or quarter % 4 == 0: return state
    base, rest = state.split('[', 1)
    out = []
    for pair in rest.rstrip(']').split(','):
        k, v = pair.split('=')
        if k in DIRS: k = DIRS[(DIRS.index(k) + quarter) % 4]
        elif k == 'facing' and v in DIRS: v = DIRS[(DIRS.index(v) + quarter) % 4]
        elif k == 'axis' and quarter % 2 and v in ('x', 'z'): v = 'z' if v == 'x' else 'x'
        elif k == 'rotation': v = str((int(v) + 4 * quarter) % 16)
        out.append((k, v))
    return base + '[' + ','.join('%s=%s' % kv for kv in sorted(out)) + ']'


def bastions(world, starts, count):
    rows = [l.split('\t') for l in open(starts).read().split('\n') if l]
    rows = sorted([(int(x), int(z)) for s, x, z in rows if s == 'bastion_remnant'], key=lambda p: p[0]**2 + p[1]**2)[:count]
    out = []
    for x, z in rows:
        root = world.chunk(x // 16, z // 16)
        if root is None: continue
        st = root.get('structures', {}).get('starts', {}).get('minecraft:bastion_remnant')
        if not st: continue
        pieces = []
        for c in st['Children']:
            loc = c['pool_element'].get('location', '').replace('minecraft:bastion/', '')
            if not loc: continue
            pieces.append((loc, c['PosX'], c['PosY'], c['PosZ'], ROT.index(c['rotation']), c['BB']))
        out.append(((x, z), pieces))
    return out


def kind(pieces):
    return pieces[0][0].split('/')[0]


def complete(world, pieces):
    x0 = min(p[5][0] for p in pieces) >> 4; x1 = max(p[5][3] for p in pieces) >> 4
    z0 = min(p[5][2] for p in pieces) >> 4; z1 = max(p[5][5] for p in pieces) >> 4
    return all(world.blocks(cx, cz) is not None for cx in range(x0, x1 + 1) for cz in range(z0, z1 + 1))


def stats(world, starts, count):
    bs = bastions(world, starts, count)
    kinds = collections.Counter(); pieces = collections.Counter(); per = collections.defaultdict(list)
    ys = collections.Counter(); reach = []; full = 0; rots = collections.Counter(); origin = collections.Counter()
    for (x, z), ps in bs:
        k = kind(ps); kinds[k] += 1; per[k].append(len(ps))
        full += complete(world, ps)
        ys[ps[0][2]] += 1; rots[ps[0][4]] += 1
        origin[(ps[0][1] - x, ps[0][3] - z)] += 1
        for p in ps: pieces[p[0]] += 1
        reach.append(max(max(abs(p[5][0] - x), abs(p[5][3] - x), abs(p[5][2] - z), abs(p[5][5] - z)) for p in ps))
    print('бастионов', len(bs), 'из них догружены целиком', full)
    for k, n in kinds.most_common():
        print('  %-14s %3d  кусков в среднем %.1f (%d..%d)' % (k, n, sum(per[k]) / n, min(per[k]), max(per[k])))
    print('высота первого куска', sorted(ys.items()), 'поворот', sorted(rots.items()))
    print('точка первого куска от угла чанка старта', origin.most_common(5))
    print('размах от угла чанка старта: до', max(reach), 'среднее', sum(reach) / len(reach))
    print('образцов', len(pieces))
    for loc, n in sorted(pieces.items()): print('  %5.2f %s' % (n / len(bs), loc))


def templates(world, starts, out, count, bare):
    plain = World(bare) if bare else None
    bs = [b for b in bastions(world, starts, count) if complete(world, b[1])]
    sizes = {}; votes = {}; carved = {}; kept = {}
    for (x, z), ps in bs:
        for i, (loc, px, py, pz, rot, bb) in enumerate(ps):
            dx, dy, dz = bb[3] - bb[0] + 1, bb[4] - bb[1] + 1, bb[5] - bb[2] + 1
            sx, sz = (dx, dz) if rot % 2 == 0 else (dz, dx)
            if sizes.setdefault(loc, (sx, dy, sz)) != (sx, dy, sz): print('размер не сходится', loc, file=sys.stderr)
            later = [q[5] for q in ps[i + 1:] if q[5][0] <= bb[3] and bb[0] <= q[5][3] and q[5][1] <= bb[4] and bb[1] <= q[5][4] and q[5][2] <= bb[5] and bb[2] <= q[5][5]]
            v = votes.setdefault(loc, {}); c = carved.setdefault(loc, collections.Counter()); kp = kept.setdefault(loc, collections.Counter())
            for ly in range(dy):
                wy = py + ly
                for lz in range(sz):
                    for lx in range(sx):
                        wx, wz = to_world(rot, px, pz, lx, lz)
                        if any(q[0] <= wx <= q[3] and q[1] <= wy <= q[4] and q[2] <= wz <= q[5] for q in later): continue
                        s = world.block(wx, wy, wz)
                        if s is None: continue
                        s = turn(s, -rot)
                        key = (lx, ly, lz)
                        v.setdefault(key, collections.Counter())[s] += 1
                        if plain is not None:
                            t = plain.block(wx, wy, wz)
                            if t is None: continue
                            t = turn(t, -rot)
                            solid = t not in ('air', 'cave_air')
                            if s in ('air', 'cave_air') and solid: c[key] += 1
                            elif s == t and solid: kp[key] += 1
    with open(out, 'w') as f:
        f.write('# Куски бастионов Незера — замер мира оригинала 26.1.2 (%d бастионов),\n' % len(bs))
        f.write('# tools/measure/nether_bastion.py templates. Образец в своём повороте\n')
        f.write('# (NONE): клетки по y, в слое по z, в ряду по x; «.» — рельеф остаётся,\n')
        f.write('# «_» — пусто, число — номер в палитре; «k*n» — n одинаковых подряд.\n')
        f.write('# layout — раскладка целого бастиона: образец, точка от угла чанка\n')
        f.write('# старта (dx, y, dz) и поворот (четверти по часовой).\n')
        names = sorted(sizes)
        for loc in names:
            sx, sy, sz = sizes[loc]
            v = votes[loc]; c = carved[loc]; kp = kept[loc]
            palette = []; cells = []
            for ly in range(sy):
                for lz in range(sz):
                    for lx in range(sx):
                        key = (lx, ly, lz)
                        if key not in v: cells.append('.'); continue
                        s, _ = v[key].most_common(1)[0]
                        if s in ('air', 'cave_air'):
                            cells.append('_' if c[key] > kp[key] or kp[key] == 0 else '.')
                            continue
                        if kp[key] >= v[key][s] and kp[key] > 0: cells.append('.'); continue
                        if s not in palette: palette.append(s)
                        cells.append(str(palette.index(s)))
            f.write('template %s %d %d %d\n' % (loc, sx, sy, sz))
            for s in palette: f.write('  %s\n' % s)
            runs = []; prev = None; n = 0
            for cell in cells + [None]:
                if cell == prev: n += 1; continue
                if prev is not None: runs.append(prev if n == 1 else '%s*%d' % (prev, n))
                prev, n = cell, 1
            for i in range(0, len(runs), 24): f.write('  = ' + ' '.join(runs[i:i + 24]) + '\n')
        # Порча (как у оригинала часть кирпича трескается или выпадает):
        # в клетках, где экземпляров не меньше трёх, — как часто вместо
        # самого частого блока стоит другой. Только простые блоки без
        # свойств.
        swaps = collections.Counter(); seen = collections.Counter()
        for loc in names:
            for key, cnt in votes[loc].items():
                total = sum(cnt.values())
                if total < 3: continue
                s, n = cnt.most_common(1)[0]
                if '[' in s or s in ('air', 'cave_air') or kept[loc][key]: continue
                seen[s] += total
                for t, m in cnt.items():
                    if t != s and '[' not in t: swaps[(s, 'air' if t == 'cave_air' else t)] += m
        for (s, t), m in sorted(swaps.items()):
            if m / seen[s] >= 0.01 and seen[s] >= 200 and 'blackstone' in s + t or s == 'gold_block':
                if m / seen[s] >= 0.01: f.write('degrade %s %s %.3f\n' % (s, t, m / seen[s]))
        for (x, z), ps in bs:
            f.write('layout %s %d\n' % (kind(ps), len(ps)))
            for loc, px, py, pz, rot, bb in ps:
                f.write('  %d %d %d %d %d\n' % (names.index(loc), px - x, py, pz - z, rot))
    print('бастионов', len(bs), 'образцов', len(sizes), file=sys.stderr)


if __name__ == '__main__':
    mode, world = sys.argv[1], World(sys.argv[2])
    if mode == 'stats': stats(world, sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 40)
    elif mode == 'templates':
        templates(world, sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 40, sys.argv[6] if len(sys.argv) > 6 else None)


def blocks_count(world, starts, count):
    """Сколько каких блоков в рамках кусков бастиона — на бастион. Тем же
    счётом меряет ignored-тест bastion_blocks (src/world/nether.rs)."""
    bs = [b for b in bastions(world, starts, count) if complete(world, b[1])]
    total = collections.Counter()
    for _, ps in bs:
        cells = set()
        for p in ps:
            b = p[5]
            for x in range(b[0], b[3] + 1):
                for y in range(b[1], b[4] + 1):
                    for z in range(b[2], b[5] + 1): cells.add((x, y, z))
        for x, y, z in cells:
            s = (world.block(x, y, z) or '?').split('[')[0]
            total[s] += 1
    print('бастионов', len(bs))
    for k, v in total.most_common(24): print('  %-36s %8.1f' % (k, v / len(bs)))


if __name__ == '__main__' and sys.argv[1] == 'blocks':
    blocks_count(World(sys.argv[2]), sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 40)
