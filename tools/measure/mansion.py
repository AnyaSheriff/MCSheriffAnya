#!/usr/bin/env python3
# Лесные особняки (woodland mansion) по замеру мира оригинала 26.1.2. Код
# игры, jar и выгрузки worldgen не используются: свой сервер оригинала
# (обычный мир, generate-structures=true, `tick freeze`, чтобы обитатели
# стояли на местах появления) только складывает мир и слушает RCON, а всё
# снимается с region-файлов: записи о строении (structures.starts
# minecraft:mansion, куски wmp: образец Template, поворот Rot, отражение Mi,
# точка образца TPX/TPY/TPZ, рамка BB), блоки, сущности блоков и сущности
# (entities/*.mca).
#
#   python3 tools/measure/mansion.py locate/generate — как template_structs.py;
#   python3 tools/measure/mansion.py explore <мир> <starts.tsv>
#       — что стоит в рамках: сущности, сущности блоков, высоты, размах;
#   python3 tools/measure/mansion.py build <мир> <starts.tsv> <вывод.txt>
#       — образцы комнат (большинство по экземплярам; клетки, которые позже
#         перекрывает рамка другого куска того же особняка, — только если
#         других наблюдений нет) и раскладки особняков (куски по порядку);
#   python3 tools/measure/mansion.py check <мир> <starts.tsv> <образцы.txt>
#       — сборка снятых особняков из образцов против них самих;
#   python3 tools/measure/mansion.py census <мир> <starts.tsv>
#       — перепись блоков в рамках кусков на особняк (сверка с тестом
#         mansion_census в src/world/mansion.rs).
#
# Образцы снимаются в два прохода: build … <первый.txt> берёт образцы
# первого прохода, чтобы отличить пустоту образца (клетка не трогает того,
# что положили раньше) от своего блока.
import os, sys, struct, zlib, json, collections, re
sys.path.insert(0, os.path.dirname(__file__))
import template_structs as ts
from template_merge import to_template as plain_template, SYMBOLS, DIRS, NAMES
from nether_census import nbt

ROT = ts.ROT

def to_template(s, rot, mirror):
    """Как template_merge.to_template, и ещё формы рельсов (стороны в
    форме поворачиваются обратно)."""
    out = plain_template(s, rot, mirror)
    if not out.startswith(('rail[', 'powered_rail[', 'detector_rail[', 'activator_rail[')): return out
    name, rest = out[:-1].split('[', 1); props = dict(kv.split('=', 1) for kv in rest.split(','))
    turn = lambda d: NAMES[ts.inverse(rot, mirror, *DIRS[d])]
    shape = props['shape']
    if shape.startswith('ascending_'): shape = 'ascending_' + turn(shape[10:])
    else:
        sides = {turn(d) for d in shape.split('_')}
        shape = 'north_south' if {'north', 'south'} <= sides else 'east_west' if {'east', 'west'} <= sides else ('north' if 'north' in sides else 'south') + '_' + ('east' if 'east' in sides else 'west')
    props['shape'] = shape
    return name + '[' + ','.join('%s=%s' % kv for kv in sorted(props.items())) + ']'

def pieces_of(st):
    out = []
    for p in st['Children']:
        q = dict(p); q['Rotation'] = p.get('Rot', 'NONE'); q['Mirror'] = p.get('Mi', 'NONE')
        out.append(q)
    return out

def mansions(w, starts):
    return [(cx, cz, pieces_of(st)) for name, cx, cz, st in ts.records(w, starts)]

def entities(world, cx, cz):
    """Сущности чанка из entities/r.*.mca."""
    p = os.path.join(world, 'dimensions/minecraft/overworld/entities/r.%d.%d.mca' % (cx >> 5, cz >> 5))
    if not os.path.exists(p): return []
    data = open(p, 'rb').read()
    i = (cx & 31) + (cz & 31) * 32
    off = int.from_bytes(data[i*4:i*4+3], 'big')
    if off == 0: return []
    q = off * 4096; ln = struct.unpack_from('>i', data, q)[0]
    raw = zlib.decompress(data[q+5:q+4+ln])
    n = struct.unpack_from('>H', raw, 1)[0]
    return nbt(raw, 3 + n, 10)[0].get('Entities', [])

def box_of(pieces):
    bbs = [p['BB'] for p in pieces]
    return [min(b[0] for b in bbs), min(b[1] for b in bbs), min(b[2] for b in bbs), max(b[3] for b in bbs), max(b[4] for b in bbs), max(b[5] for b in bbs)]

def inside(b, x, y, z):
    return b[0] <= x <= b[3] and b[1] <= y <= b[4] and b[2] <= z <= b[5]

def cmd_explore(world, starts):
    w = ts.World(world); kinds = collections.Counter(); be = collections.Counter(); per = []
    for cx, cz, pieces in mansions(w, ts.read_starts(starts)):
        b = box_of(pieces); x0, z0 = cx * 16, cz * 16
        ents = []
        for i in range((b[0]) >> 4, (b[3] >> 4) + 1):
            for j in range((b[2]) >> 4, (b[5] >> 4) + 1):
                for e in entities(world, i, j):
                    pos = e['Pos']
                    if inside(b, int(pos[0] // 1), int(pos[1] // 1), int(pos[2] // 1)):
                        ents.append(e); kinds[e['id']] += 1
        y0 = pieces[0]['TPY']
        print('особняк', cx, cz, 'рамка от старта', b[0] - x0, b[2] - z0, b[3] - x0, b[5] - z0, 'y', b[1], b[4], 'вход', pieces[0]['Template'], pieces[0]['BB'][:3], 'кусков', len(pieces))
        for e in ents[:4]: print('   ', e['id'], [round(v, 2) for v in e['Pos']], e.get('PersistenceRequired'), [k for k in e if k not in ('Pos','Motion','Rotation','UUID','attributes','Attributes','Brain','ArmorItems','HandItems','equipment','drop_chances')][:30])
        for i in range(b[0] >> 4, (b[3] >> 4) + 1):
            for j in range(b[2] >> 4, (b[5] >> 4) + 1):
                g = w.blocks(i, j)
                for (x, y, z), e in (g or {}).get('be', {}).items():
                    if inside(b, x, y, z):
                        key = (e.get('id'), e.get('LootTable'), json.dumps(e.get('Items', [])[:3], default=str)[:120] if not e.get('LootTable') else '')
                        be[key] += 1
        # Земля: на углах общей рамки (на 2 наружу) — для правила высоты.
        def ground(x, z):
            for y in range(320, -64, -1):
                s = w.block(x, y, z)
                if s is None: return None
                if not re.match(r'^(air|cave_air|water|.*leaves|short_grass|tall_grass|fern|large_fern|snow|vine|.*_flower|dandelion|poppy|.*_log|.*_wood|brown_mushroom|red_mushroom|pale_moss_carpet|pale_hanging_moss|moss_carpet|leaf_litter|bush|firefly_bush)$', s.split('[')[0]): return y
        per.append((y0, [ground(x0 + dx, z0 + dz) for dx, dz in ((0, 0), (15, 0), (0, 15), (15, 15), (7, 7))]))
    print(kinds.most_common())
    for k, v in be.most_common(): print(v, k)
    for y0, g in per: print('y0', y0, 'земля у углов старта', g)

SYMS = SYMBOLS + 'АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЭЮЯабвгдежзийклмнопрстуфхцчшщэюяΓΔΘΛΞΠΣΦΨΩαβγδεζηθλμξπσφψω'

def orient_of(p):
    """Поворот и отражение куска в записи Orient (template.rs): отражение
    a → −a, потом поворот по часовой. LEFT_RIGHT (b → −b) = FRONT_BACK и
    поворот на 180°."""
    rot = ROT.index(p['Rotation']); mi = p['Mirror']
    return ((rot + (2 if mi == 'LEFT_RIGHT' else 0)) % 4, 0 if mi == 'NONE' else 1)

def observe(w, world, starts, first=None):
    """Наблюдения по образцам: клетка образца → Counter((состояние, перекрыта)),
    сущности блоков, мобы; раскладки особняков."""
    obs = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    sizes = {}; count = collections.Counter(); bes = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    mobs = collections.defaultdict(collections.Counter); layouts = []
    for cx, cz, pieces in mansions(w, ts.read_starts(starts)):
        b = box_of(pieces)
        if not all(w.full(i, j) for i in range(b[0] >> 4, (b[3] >> 4) + 1) for j in range(b[2] >> 4, (b[5] >> 4) + 1)):
            print('не сложено', cx, cz, file=sys.stderr); continue
        last = {}
        for i, p in enumerate(pieces):
            bb = p['BB']
            for x in range(bb[0], bb[3] + 1):
                for z in range(bb[2], bb[5] + 1):
                    for y in range(bb[1], bb[4] + 1): last[(x, y, z)] = i
        y0 = pieces[0]['TPY']; layout = []; painted = {}
        for i, p in enumerate(pieces):
            name = p['Template']; bb = p['BB']; f = ts.local_of(p)
            rot = ROT.index(p['Rotation']); mi = p['Mirror']
            corners = [f(x, z) for x in (bb[0], bb[3]) for z in (bb[2], bb[5])]
            size = (max(c[0] for c in corners) + 1, max(c[1] for c in corners) + 1, bb[4] - bb[1] + 1)
            sizes.setdefault(name, size); count[name] += 1
            if sizes[name] != size: print('размер не тот', name, size, sizes[name], file=sys.stderr)
            turns, mirror = orient_of(p)
            if first is not None and i > 0:
                q = pieces[i - 1]; t = first[q['Template']]; qa, qb, _ = t['size']; qt, qm = orient_of(q)
                for (ta, ty, tb), st in t['cells'].items():
                    dx, dz = offset(qt, qm, ta, tb, qa, qb)
                    painted[(q['BB'][0] + dx, q['BB'][1] + ty, q['BB'][2] + dz)] = (i - 1, st)
            layout.append((name, turns, mirror, bb[0] - cx * 16, bb[1] - y0, bb[2] - cz * 16))
            for x in range(bb[0], bb[3] + 1):
                for z in range(bb[2], bb[5] + 1):
                    a, bb_ = f(x, z)
                    for y in range(bb[1], bb[4] + 1):
                        s = w.block(x, y, z)
                        if s is None: continue
                        covered = last[(x, y, z)] != i
                        void = first is not None
                        if first is not None and (x, y, z) in painted:
                            j, st = painted[(x, y, z)]; q = pieces[j]
                            got = to_template(s, ROT.index(q['Rotation']), q['Mirror'])
                            void = (got if got.split('[')[0] != 'cave_air' else 'air') == st
                        obs[name][(a, y - bb[1], bb_)][(to_template(s, rot, mi), covered, void)] += 1
                        if not covered:
                            e = w.entity(x, y, z)
                            if e:
                                if e.get('LootTable'): key = 'loot ' + e['LootTable'].split('/')[-1]
                                elif e.get('id') == 'minecraft:mob_spawner': key = 'spawner ' + e.get('SpawnData', {}).get('entity', {}).get('id', '?').replace('minecraft:', '')
                                elif e.get('Items'):
                                    key = 'items ' + ';'.join('%d:%s:%d%s' % (it['Slot'], it['id'].replace('minecraft:', ''), it['count'], ':' + ','.join('%s=%s' % (k.replace('minecraft:', ''), v) for k, v in it.get('components', {}).get('minecraft:enchantments', {}).items()) if it.get('components') else '') for it in e['Items'])
                                else: key = None
                                if key: bes[name][(a, y - bb[1], bb_)][key] += 1
        for i in range(b[0] >> 4, (b[3] >> 4) + 1):
            for j in range(b[2] >> 4, (b[5] >> 4) + 1):
                for e in entities(world, i, j):
                    x, y, z = (int(v // 1) for v in e['Pos'])
                    if (x, y, z) not in last: continue
                    p = pieces[last[(x, y, z)]]; a, bb_ = ts.local_of(p)(x, z)
                    mobs[p['Template']][(a, y - p['BB'][1], bb_, e['id'].replace('minecraft:', ''))] += 1
        layouts.append((cx, cz, y0, layout))
    return obs, sizes, count, bes, mobs, layouts

def choose(cell):
    """Клетка образца: что стоит у неперекрытых экземпляров. Пустота образца
    (клетка не трогает того, что было) — когда всё увиденное объясняется
    прежним: блоком раньше положенного куска (по образцам первого прохода)
    или рельефом, а своё значение — нет."""
    own = collections.Counter(); every = collections.Counter(); own_void = every_void = 0
    for (s, covered, void), n in cell.items():
        every[s] += n; every_void += n * void
        if not covered: own[s] += n; own_void += n * void
    src, voids = (own, own_void) if own else (every, every_void)
    total = sum(src.values())
    top, k = src.most_common(1)[0]
    if k == total: return top
    # Один и тот же блок с разными формами (стенки, панели на стыках) —
    # свой блок образца, не рельеф.
    names = collections.Counter()
    for st, n in src.items(): names[st.split('[')[0]] += n
    name, kn = names.most_common(1)[0]
    if kn == total and name not in ('air', 'cave_air'):
        return max((st for st in src if st.split('[')[0] == name), key=lambda st: src[st])
    if voids == total: return None
    if k >= voids and k * 2 >= total: return top
    names = collections.Counter()
    for s, n in src.items(): names[s.split('[')[0]] += n
    name, k = names.most_common(1)[0]
    if k * 2 >= total: return max((s for s in src if s.split('[')[0] == name), key=lambda s: src[s])
    return None

def cmd_build(world, starts, out, first=None):
    w = ts.World(world)
    obs, sizes, count, bes, mobs, layouts = observe(w, world, starts, read_templates(first) if first else None)
    f = open(out, 'w')
    f.write('# Лесной особняк: образцы кусков и раскладки, замер оригинала 26.1.2\n# (tools/measure/mansion.py build, %d особняков). Не править руками.\n' % len(layouts))
    for name in sorted(sizes):
        sa, sb, sy = sizes[name]; cells = {}
        for at, cell in obs[name].items(): cells[at] = choose(cell)
        palette = {}
        for s in cells.values():
            if s is not None and s.split('[')[0] not in ('air', 'cave_air') and s not in palette: palette[s] = SYMS[len(palette)]
        f.write('template %s %d %d %d %d\n' % (name, sa, sb, sy, count[name]))
        for s, c in palette.items(): f.write('  %s %s\n' % (c, s))
        for at in sorted(bes[name]):
            key, n = bes[name][at].most_common(1)[0]
            kind, data = key.split(' ', 1)
            if kind == 'loot': f.write('  loot %d %d %d %s\n' % (at[0], at[1], at[2], data))
            else: f.write('  %s %d %d %d %s %.2f\n' % (kind, at[0], at[1], at[2], data, n / count[name]))
        for at in sorted(mobs[name]):
            f.write('  mob %d %d %d %s %.2f\n' % (at[0], at[1], at[2], at[3], mobs[name][at] / count[name]))
        for y in range(sy):
            f.write('layer %d\n' % y)
            for b in range(sb):
                row = ''
                for a in range(sa):
                    s = cells.get((a, y, b))
                    row += '.' if s is None else '_' if s.split('[')[0] in ('air', 'cave_air') else palette[s]
                f.write('  ' + row + '\n')
    f.close(); f = open(out.replace('.txt', '_layouts.txt'), 'w')
    f.write('# Раскладки лесных особняков: куски по порядку сложения, замер оригинала 26.1.2\n# (tools/measure/mansion.py build). Строка куска: образец, четверти поворота и\n# отражение (как Orient в template.rs), угол рамки от угла чанка старта и от\n# пола входа. Не править руками.\n')
    for cx, cz, y0, layout in layouts:
        f.write('mansion %d %d %d %d\n' % (cx, cz, y0, len(layout)))
        for name, turns, mirror, dx, dy, dz in layout:
            f.write('  p %s %d %d %d %d %d\n' % (name, turns, mirror, dx, dy, dz))
    print('образцов', len(sizes), 'особняков', len(layouts))

def offset(turns, mirror, a, b, wd, ln):
    """Как Orient::offset в template.rs."""
    if mirror: a = wd - 1 - a
    return [(a, b), (ln - 1 - b, a), (wd - 1 - a, ln - 1 - b), (b, wd - 1 - a)][turns]

def read_templates(path):
    out = {}; cur = None; layer = None; row = 0
    for line in open(path):
        if line.startswith('#'): continue
        wds = line.split()
        if not wds: continue
        if wds[0] == 'template':
            cur = {'size': tuple(int(v) for v in wds[2:5]), 'pal': {}, 'cells': {}}; out[wds[1]] = cur; layer = None
        elif wds[0] == 'mansion': cur = None
        elif cur is None: continue
        elif wds[0] == 'layer': layer = int(wds[1]); row = 0
        elif layer is not None:
            for a, ch in enumerate(wds[0]):
                if ch != '.': cur['cells'][(a, layer, row)] = 'air' if ch == '_' else cur['pal'][ch]
            row += 1
        elif len(wds) == 2: cur['pal'][wds[0]] = wds[1]
    return out

def cmd_check(world, starts, path):
    """Сборка снятых особняков из образцов против самих снятых: доля
    совпавших клеток в рамках кусков."""
    w = ts.World(world); tpl = read_templates(path); same = miss = keep = 0; bad = collections.Counter()
    for cx, cz, pieces in mansions(w, ts.read_starts(starts)):
        painted = {}
        for i, p in enumerate(pieces):
            t = tpl[p['Template']]; sa, sb, sy = t['size']; bb = p['BB']; turns, mirror = orient_of(p)
            for (a, y, b), st in t['cells'].items():
                dx, dz = offset(turns, mirror, a, b, sa, sb)
                painted[(bb[0] + dx, bb[1] + y, bb[2] + dz)] = (i, st)
        cover = set()
        for p in pieces:
            bb = p['BB']
            for x in range(bb[0], bb[3] + 1):
                for z in range(bb[2], bb[5] + 1):
                    for y in range(bb[1], bb[4] + 1): cover.add((x, y, z))
        for at in cover:
            s = w.block(*at)
            if s is None: continue
            if at not in painted: keep += 1; continue
            i, st = painted[at]; p = pieces[i]
            got = to_template(s, ROT.index(p['Rotation']), p['Mirror'])
            if got.split('[')[0] in ('cave_air',): got = 'air'
            if got == st: same += 1
            else: miss += 1; bad[(p['Template'], st.split('[')[0], got.split('[')[0])] += 1
    print('совпало', same, 'нет', miss, 'не тронуто', keep, '%.4f' % (same / (same + miss)))
    for k, v in bad.most_common(40): print(v, k)

def cmd_census(world, starts):
    w = ts.World(world); total = collections.Counter(); n = 0
    for cx, cz, pieces in mansions(w, ts.read_starts(starts)):
        b = box_of(pieces)
        if not all(w.full(i, j) for i in range(b[0] >> 4, (b[3] >> 4) + 1) for j in range(b[2] >> 4, (b[5] >> 4) + 1)): continue
        cells = set()
        for p in pieces:
            bb = p['BB']
            for x in range(bb[0], bb[3] + 1):
                for z in range(bb[2], bb[5] + 1):
                    for y in range(bb[1], bb[4] + 1): cells.add((x, y, z))
        for at in cells: total[w.block(*at).split('[')[0]] += 1
        n += 1
    print('особняков', n)
    for k, v in total.most_common(40): print('%-28s %9.1f' % (k, v / n))

if __name__ == '__main__':
    cmd = sys.argv[1]; args = sys.argv[2:]
    if cmd in ('locate', 'generate'):
        getattr(ts, 'cmd_' + cmd)(*args)
    else:
        globals()['cmd_' + cmd](*args)
