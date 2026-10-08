#!/usr/bin/env python3
# Подводные крепости (ocean monument) по замеру мира оригинала 26.1.2. Код
# игры, jar и выгрузки worldgen не используются: сервер оригинала только
# запускается (свой мир, обычный тип, строения включены) и слушает RCON, а всё
# снимается с region-файлов, которые он сложил, — блоки и записи о строениях
# в чанках (structures.starts minecraft:monument: один кусок omb, рамка BB
# 58×23×58, сторона O).
#
#   python3 tools/measure/monument.py locate <порт rcon> <пароль> <радиус в блоках> <вывод.tsv>
#       — `locate structure minecraft:monument` из точек сетки 512 блоков;
#   python3 tools/measure/monument.py generate <мир> <порт rcon> <пароль> <starts.tsv> [сколько]
#       — догружает (forceload) чанки вокруг крепостей, ждёт полных;
#   python3 tools/measure/monument.py templates <мир> <вывод.txt> [сколько]
#       — каждая крепость целиком своим образцом (src/world/monument.txt);
#   python3 tools/measure/monument.py census <мир> [сколько]
#       — перепись блоков в рамках крепостей (сверка с тестом monument_census
#         в src/world/monument.rs) и что лежит под рамкой.
import os, sys, re, time, collections
sys.path.insert(0, os.path.dirname(__file__))
from nether_fortress import Rcon, state_name
from end_city import World as EndWorld, SYMBOLS

class World(EndWorld):
    def __init__(self, world):
        super().__init__(world)
        self.reg = os.path.join(world, 'dimensions/minecraft/overworld/region')
    def blocks(self, cx, cz):
        # Как у Энда, но по всей высоте обычного мира (секции −4..19).
        k = (cx, cz)
        if k not in self.grids:
            from nether_census import unpack
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
            self.grids[k] = grid
            if len(self.grids) > 400: self.grids.pop(next(iter(self.grids)))
        return self.grids[k]
    def monuments(self, count):
        out = []
        for name in sorted(os.listdir(self.reg)):
            m = re.match(r'r\.(-?\d+)\.(-?\d+)\.mca$', name)
            if not m: continue
            rx, rz = int(m.group(1)), int(m.group(2))
            for i in range(1024):
                cx, cz = rx * 32 + (i & 31), rz * 32 + (i >> 5)
                root = self.chunk(cx, cz)
                st = root and root.get('structures', {}).get('starts', {}).get('minecraft:monument')
                if not st or st.get('id') == 'INVALID': continue
                bb = st['Children'][0]['BB']
                if all(self.full(x, z) for x in range(bb[0] >> 4, (bb[3] >> 4) + 1) for z in range(bb[2] >> 4, (bb[5] >> 4) + 1)):
                    out.append((cx, cz, st['Children'][0]))
        out.sort(key=lambda m: m[0] ** 2 + m[1] ** 2)
        return out[:int(count)]

def elders(world):
    """Древние стражи мира (entities/*.mca): позиции."""
    import glob, struct, zlib
    from nether_census import nbt
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
            for e in nbt(raw, 3 + n, 10)[0].get('Entities', []):
                if e.get('id') == 'minecraft:elder_guardian': out.append(tuple(e['Pos']))
    return out

def cmd_locate(port, password, radius, out):
    r = Rcon(int(port), password); radius = int(radius); found = set()
    for x in range(-radius, radius + 1, 512):
        for z in range(-radius, radius + 1, 512):
            m = re.search(r'\[(-?\d+), ~, (-?\d+)\]', r.run('execute positioned %d 64 %d run locate structure minecraft:monument' % (x, z)))
            if m: found.add((int(m.group(1)), int(m.group(2))))
    with open(out, 'w') as f:
        for x, z in sorted(found): f.write('%d\t%d\n' % (x, z))
    print('крепостей', len(found))

def cmd_generate(world, port, password, starts, count=1000):
    w = World(world); r = Rcon(int(port), password)
    rows = [tuple(map(int, l.split('\t'))) for l in open(starts).read().split('\n') if l]
    rows.sort(key=lambda p: p[0] ** 2 + p[1] ** 2)
    for n, (x, z) in enumerate(rows[:int(count)]):
        cx, cz = x // 16, z // 16
        area = [(i, j) for i in range(cx - 3, cx + 4) for j in range(cz - 3, cz + 4)]
        box = '%d %d %d %d' % ((cx - 3) * 16, (cz - 3) * 16, (cx + 3) * 16 + 15, (cz + 3) * 16 + 15)
        if not all(w.full(*c) for c in area):
            r.run('forceload add ' + box)
            while True:
                time.sleep(4); r.run('save-all'); w.regions.clear(); w.grids.clear()
                if all(w.full(*c) for c in area): break
            r.run('forceload remove ' + box)
        print(n, x, z, flush=True)
    r.run('save-all')

# Чем сложена крепость (вики, Ocean Monument, «Structure»).
OWN = ('prismarine', 'prismarine_bricks', 'dark_prismarine', 'sea_lantern', 'wet_sponge', 'gold_block')

def cmd_templates(world, out, count=4):
    w = World(world); ms = w.monuments(count); old = elders(world)
    lines = ['# Подводные крепости — замер мира оригинала 26.1.2 (%d крепостей),' % len(ms),
             '# tools/measure/monument.py templates. Каждая крепость — свой образец 58×58,',
             '# слои снизу вверх от низа рамки (y 39), строки — по z, знаки — по x;',
             '# сторона O — как в записи о строении. «~» — вода, «#» — prismarine_bricks.',
             '# Под каждой prismarine_bricks нижнего слоя — опора из неё же до грунта.',
             '# elder — где при сложении стоят древние стражи (рамка: x, y, z от угла).', '']
    for cx, cz, p in ms:
        bb = p['BB']; W, H, D = bb[3] - bb[0] + 1, bb[4] - bb[1] + 1, bb[5] - bb[2] + 1
        palette = {'prismarine_bricks': '#', 'water[level=0]': '~'}; layers = []
        for y in range(H):
            rows = []
            for b in range(D):
                row = ''
                for a in range(W):
                    s = w.block(bb[0] + a, bb[1] + y, bb[2] + b)
                    # Внутри рамки рельефа нет (перепись: только вода, ламинария,
                    # водоросли и лёд айсбергов — их кладут позже), так что всё
                    # не своё — вода.
                    row += palette.setdefault(s, SYMBOLS[len(palette) - 2]) if s.split('[')[0] in OWN else '~'
                rows.append(row)
            layers.append(rows)
        lines.append('monument %d %d %d %d %d' % (W, D, H, p['O'], bb[1]))
        for s, ch in palette.items():
            if ch not in '#~': lines.append('  %s %s' % (ch, s))
        for e in old:
            if bb[0] <= e[0] <= bb[3] + 1 and bb[2] <= e[2] <= bb[5] + 1:
                lines.append('  elder %g %g %g' % (e[0] - bb[0], e[1] - bb[1], e[2] - bb[2]))
        for y, rows in enumerate(layers):
            lines.append('layer %d' % y); lines.extend('  ' + r for r in rows)
        lines.append('')
        print('крепость', cx, cz, 'O', p['O'], 'палитра', len(palette))
    open(out, 'w').write('\n'.join(lines))

def cmd_census(world, count=8):
    w = World(world); ms = w.monuments(count)
    total = collections.Counter(); below = collections.Counter(); keep = collections.Counter()
    for cx, cz, p in ms:
        bb = p['BB']; one = collections.Counter()
        for x in range(bb[0], bb[3] + 1):
            for z in range(bb[2], bb[5] + 1):
                for y in range(bb[1], bb[4] + 1):
                    s = w.block(x, y, z).split('[')[0]
                    one[s] += 1
                    if s not in OWN and s != 'water': keep[s] += 1
                for y in range(bb[1] - 8, bb[1]):
                    s = w.block(x, y, z).split('[')[0]
                    if s in OWN: below[(bb[1] - y, s)] += 1
        total.update(one)
        print('крепость', cx, cz, 'O', p['O'], ' '.join('%s=%d' % kv for kv in sorted(one.items()) if kv[0] in OWN))
    print('всего (%d):' % len(ms), ' '.join('%s=%.1f' % (k, v / len(ms)) for k, v in sorted(total.items()) if k in OWN or k == 'water'))
    print('не своё в рамке:', keep.most_common(12))
    print('своё под рамкой (глубина, блок):', sorted(below.items())[:40])

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'locate': cmd_locate, 'generate': cmd_generate, 'templates': cmd_templates, 'census': cmd_census}[cmd](*args)
