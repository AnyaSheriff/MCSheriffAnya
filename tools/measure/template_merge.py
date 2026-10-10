#!/usr/bin/env python3
# Сведение образцов строений из замеров (вывод template_structs.py dump):
# все экземпляры одного образца приводятся к его координатам (отражение и
# поворот сняты, направления в состояниях повёрнуты обратно), и по каждой
# клетке рамки берётся то, что стоит в ней у большинства экземпляров.
# Случайные замены оригинала (плачущий обсидиан, потрескавшиеся и замшелые
# кирпичи, магма вместо лавы, пропавшее золото…) у большинства не выпадают,
# поэтому большинство даёт сам образец; рельеф вокруг у каждого экземпляра
# свой и большинства не набирает.
#
#   python3 tools/measure/template_merge.py templates <вывод.txt> <семейство> <dump.json>…
#       — образцы семейства (ruined_portal, shipwreck, ocean_ruin, trail_ruins);
#   python3 tools/measure/template_merge.py census <семейство> <dump.json>…
#       — перепись блоков в рамках строений по экземплярам (сверка с тестами);
#   python3 tools/measure/template_merge.py placement <dump.json>…
#       — высота точки образца против земли вокруг, поля строения.
import sys, os, json, collections, re
sys.path.insert(0, os.path.dirname(__file__))
from template_structs import ROT, inverse

SYMBOLS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!$%&*+/<=>?@^`|'
DIRS = {'north': (0, -1), 'south': (0, 1), 'east': (1, 0), 'west': (-1, 0)}
NAMES = {v: k for k, v in DIRS.items()}

def split(s):
    if '[' not in s: return s, {}
    name, rest = s[:-1].split('[', 1)
    return name, dict(kv.split('=', 1) for kv in rest.split(','))

def join(name, props):
    return name + ('[' + ','.join('%s=%s' % kv for kv in sorted(props.items())) + ']' if props else '')

def to_template(s, rot, mirror):
    """Состояние из мира — в повороте образца."""
    name, props = split(s)
    out = {}
    turn = lambda d: NAMES[inverse(rot, mirror, *DIRS[d])]
    for k, v in props.items():
        if k in DIRS: out[turn(k)] = v
        elif k == 'facing' and v in DIRS: out[k] = turn(v)
        elif k == 'axis' and v in 'xz' and rot % 2 == 1: out[k] = 'z' if v == 'x' else 'x'
        elif k == 'rotation':
            # 16 делений по часовой; обратный поворот и отражение.
            r = (int(v) - 4 * rot) % 16
            if mirror == 'LEFT_RIGHT': r = (8 - r) % 16
            elif mirror == 'FRONT_BACK': r = (16 - r) % 16
            out[k] = str(r)
        else: out[k] = v
    if mirror != 'NONE':
        for k in ('shape',):
            if k in out: out[k] = out[k].replace('left', 'L').replace('right', 'left').replace('L', 'right')
        for k in ('hinge', 'type'):
            if k in out and out[k] in ('left', 'right'): out[k] = 'right' if out[k] == 'left' else 'left'
    return join(name, out)

# Чем сложено семейство: остальное в рамке — рельеф и вода. Блоки, которые
# оригинал ставит заменой (обсидиан → плачущий…), сводятся к исходным только
# через большинство, поэтому здесь все варианты — «свои».
OWN = {
    'ruined_portal': re.compile(r'^(obsidian|crying_obsidian|.*stone_brick.*|stone|stone_slab|stone_stairs|chiseled_stone_bricks|netherrack|magma_block|lava|gold_block|chest|iron_bars|chain|iron_chain|.*blackstone.*|.*_slab|.*_stairs|.*_wall)$'),
    'shipwreck': re.compile(r'^(.*_planks|.*_log|.*_wood|.*_fence|.*_slab|.*_stairs|.*_trapdoor|.*_door|chest)$'),
    'ocean_ruin': re.compile(r'^(.*stone_brick.*|chiseled_stone_bricks|gravel|suspicious_gravel|sand|suspicious_sand|.*sandstone.*|prismarine|magma_block|sea_lantern|polished_granite|.*glazed_terracotta|bricks|.*_planks|cobblestone|mossy_cobblestone|obsidian|chest|.*_stairs|.*_slab|.*_wall|cracked_stone_bricks|mossy_stone_bricks|dark_prismarine|prismarine_bricks|stone|granite|diorite|andesite|chiseled_sandstone|cut_sandstone|smooth_sandstone|terracotta|.*_terracotta)$'),
    # Руины тропы закопаны в камень и землю: камень и землю рельеф даёт и
    # сам, свои — всё остальное (вики: «buried in gravel, dirt, and coarse dirt»).
    'trail_ruins': re.compile(r'^(.*terracotta.*|mud_bricks|.*mud_brick.*|packed_mud|mud|gravel|suspicious_gravel|coarse_dirt|.*_planks|.*_stairs|.*_slab|.*_wall|.*_glass_pane|.*_glass|cobblestone|mossy_cobblestone|.*stone_bricks|bricks|.*_candle|candle|decorated_pot|.*_hanging_sign|chest|.*_trapdoor|.*_fence)$'),
}

# Нижний мир кладёт тот же образец чёрным камнем (вики: «replaced with their
# blackstone equivalents, and … chains replace iron bars»): назад к камню.
BLACKSTONE = {
    'polished_blackstone_bricks': 'stone_bricks', 'cracked_polished_blackstone_bricks': 'cracked_stone_bricks',
    'chiseled_polished_blackstone': 'chiseled_stone_bricks', 'polished_blackstone_brick_stairs': 'stone_brick_stairs',
    'polished_blackstone_brick_slab': 'stone_brick_slab', 'polished_blackstone_brick_wall': 'stone_brick_wall',
    'blackstone': 'stone#', 'blackstone_stairs': 'stone_stairs', 'blackstone_slab': 'stone_slab', 'blackstone_wall': 'stone_wall',
    'polished_blackstone': 'smooth_stone', 'polished_blackstone_slab': 'smooth_stone_slab', 'polished_blackstone_stairs': 'stone_stairs',
    'chain': 'iron_bars', 'iron_chain': 'iron_bars',
}

def family_of(piece):
    t = piece['piece'].get('Template', '')
    if 'ruined_portal' in t: return 'ruined_portal'
    if 'shipwreck' in t: return 'shipwreck'
    if 'underwater_ruin' in t: return 'ocean_ruin'
    if 'trail_ruins' in t: return 'trail_ruins'
    return piece['structure']

def load(paths):
    out = []; seen = set()
    for p in paths:
        for pc in json.load(open(p)):
            key = (pc['structure'], tuple(pc['chunk']), pc['piece'].get('Template'), tuple(pc['piece']['BB']))
            if key not in seen: seen.add(key); out.append(pc)
    return out

WOODS = ('dark_oak', 'oak', 'spruce', 'birch', 'jungle', 'acacia', 'mangrove', 'cherry', 'pale_oak', 'bamboo')

def wood_of(name):
    for w in WOODS:
        if name.startswith(w + '_') and not name.endswith(('leaves', 'sapling')): return w
    return None

# Кораблекрушение: дерево двух видов — основное и второе (вики: «Only two
# wood types are used per ship … only 8 combinations are possible»).
SHIP_COMBOS = {('dark_oak', 'jungle'), ('dark_oak', 'spruce'), ('jungle', 'spruce'), ('oak', 'birch'), ('oak', 'spruce'), ('spruce', 'dark_oak'), ('spruce', 'jungle'), ('spruce', 'oak')}

def ship_slots(pieces):
    """Каждому экземпляру — (основное, второе) дерево: по клеткам сводится
    с первым экземпляром, а какое из двух основное — по списку сочетаний вики."""
    views = [oriented(pc, woods=None) for pc in pieces]
    ref = views[0]; ref_woods = [w for w, _ in collections.Counter(wood_of(s.split('[')[0]) for s in ref.values() if wood_of(s.split('[')[0])).most_common(2)]
    pairs = []
    for v in views:
        co = collections.Counter()
        for k, s in v.items():
            a = wood_of(s.split('[')[0]); b = wood_of(ref.get(k, '').split('[')[0])
            if a and b: co[(b, a)] += 1
        mapping = {}
        for (b, a), n in co.most_common():
            if b not in mapping and a not in mapping.values(): mapping[b] = a
        pairs.append(tuple(mapping.get(w) for w in ref_woods))
    straight = sum(1 for p in pairs if p in SHIP_COMBOS); turned = sum(1 for p in pairs if p[::-1] in SHIP_COMBOS)
    return [p if straight >= turned else p[::-1] for p in pairs]

def oriented(piece, woods=None):
    """Клетки экземпляра в координатах и повороте образца: (a, y, b) → состояние.
    `woods` — (основное, второе) дерево: в образце они станут дубом и елью."""
    p = piece['piece']; rot = ROT.index(p.get('Rotation', p.get('Rot', 'NONE'))); mirror = p.get('Mirror', 'NONE')
    black = p.get('Properties', {}).get('replace_with_blackstone')
    cells = {}
    for a, y, b, x, wy, z, s in piece['cells']:
        s = to_template(s, rot, mirror)
        if black:
            name, props = split(s)
            # В Незере рельеф — незеррак, базальт, чёрный камень, руда, а
            # растительность и дельты кладутся уже после портала: голосуют
            # только явно свои блоки (чёрный камень образца — полированный).
            if name == 'blackstone' or not (name in BLACKSTONE or name in ('obsidian', 'crying_obsidian', 'chest', 'gold_block', 'lava', 'air', 'cave_air')): continue
            if name in BLACKSTONE: s = join(BLACKSTONE[name], props)
        if woods:
            name = s.split('[')[0]; w = wood_of(name)
            if w in woods and woods[0] != woods[1]:
                s = ('oak' if w == woods[0] else 'spruce') + s[len(w):]
        cells[(a, y, b)] = s
    return cells

def size_of(piece):
    p = piece['piece']; bb = p['BB']; rot = ROT.index(p.get('Rotation', p.get('Rot', 'NONE')))
    w, h, d = bb[3] - bb[0] + 1, bb[4] - bb[1] + 1, bb[5] - bb[2] + 1
    return (d, h, w) if rot % 2 else (w, h, d)

# Что оригинал ставит только заменой (вики, Ruined Portal: «Many blocks …
# are replaced upon generation»): такие голоса говорят, что клетка своя, но
# сам образец в ней — из незаменённых.
AGED = {
    'ruined_portal': {'cracked_stone_bricks', 'mossy_stone_bricks', 'mossy_stone_brick_stairs', 'mossy_stone_brick_slab', 'mossy_stone_brick_wall', 'crying_obsidian', 'magma_block'},
}

def choose(family, raw, total):
    """Блок клетки по голосам `raw` (состояние → сколько) из `total` экземпляров."""
    own = OWN[family]; aged = AGED.get(family, set())
    names = collections.Counter(); air = 0
    for st, n in raw.items():
        name = st.split('[')[0]
        if own.match(name): names[name] += n
        elif name in ('air', 'cave_air'): air += n
    # «stone#» — камень из чёрного камня Нижнего мира: там рельеф не камень.
    marked = names.pop('stone#', 0)
    if marked: names['stone'] += marked
    raw = collections.Counter({st.replace('stone#', 'stone'): n for st, n in raw.items()})
    mine = sum(names.values())
    if family == 'ruined_portal':
        # Камень — и рельеф: свой камень наполовину стареет (трещины, мох,
        # ступени), а у рельефа старых нет совсем.
        if names and names.most_common(1)[0][0] == 'stone' and not marked and not any(names[n] for n in aged | {'stone_brick_stairs', 'stone_slab', 'stone_brick_slab'}):
            names.pop('stone'); mine = sum(names.values())
            raw = collections.Counter({st: n for st, n in raw.items() if st.split('[')[0] != 'stone'})
        # Золото пропадает в 30 % (вики): треть голосов за золото — золото.
        if names['gold_block'] * 3 >= total:
            return max((st for st in raw if st.startswith('gold_block')), key=raw.get)
        # Незеррак разливается и по рельефу вокруг, но не всюду: свой — почти
        # у всех экземпляров (магма — его замена в 7 %).
        rack = names['netherrack'] + names['magma_block']
        if rack == mine and rack < total * 0.95:
            return '_' if air * 2 >= total else None
    if mine * 2 < total and not (mine >= 2 and mine >= air and mine >= total - mine - air):
        return '_' if air * 2 >= total else None
    plain = collections.Counter({st: n for st, n in raw.items() if own.match(st.split('[')[0]) and st.split('[')[0] not in aged})
    if not plain: plain = collections.Counter({st: n for st, n in raw.items() if own.match(st.split('[')[0])})
    # Сперва имя по большинству, потом свойства — у экземпляров с этим именем.
    best = collections.Counter()
    for st, n in plain.items(): best[st.split('[')[0]] += n
    name = best.most_common(1)[0][0]
    return max((st for st in plain if st.split('[')[0] == name), key=plain.get)

def merge(pieces, family):
    """Образец по большинству: клетка → состояние, '_' (воздух) или None."""
    size = size_of(pieces[0])
    slots = ship_slots(pieces) if family == 'shipwreck' else [None] * len(pieces)
    views = [(pc, oriented(pc, woods)) for pc, woods in zip(pieces, slots)]
    cells = {}
    for a in range(size[0]):
        for y in range(size[1]):
            for b in range(size[2]):
                raw = collections.Counter(); total = 0
                for pc, v in views:
                    s = v.get((a, y, b))
                    if s is None: continue
                    raw[s] += 1; total += 1
                if not total: continue
                got = choose(family, raw, total)
                if got is not None: cells[(a, y, b)] = got
    return size, cells

def cmd_templates(out, family, *paths):
    pieces = [pc for pc in load(paths) if family_of(pc) == family]
    by = collections.defaultdict(list)
    for pc in pieces: by[pc['piece']['Template'].replace('minecraft:', '')].append(pc)
    lines = ['# %s — образцы по замеру мира оригинала 26.1.2 (tools/measure/template_merge.py),' % family,
             '# клетка — то, что стоит в ней у большинства экземпляров. Слои снизу вверх,',
             '# строки — по b (z без поворота), знаки — по a (x). «.» — не трогать, «_» — воздух.', '']
    for name in sorted(by):
        size, cells = merge(by[name], family)
        palette = {}
        for s in sorted(set(v for v in cells.values() if v != '_')): palette[s] = SYMBOLS[len(palette)]
        lines.append('template %s %d %d %d %d' % (name, size[0], size[2], size[1], len(by[name])))
        for s, ch in palette.items(): lines.append('  %s %s' % (ch, s))
        # Таблицы добычи сундуков образца (поле LootTable у экземпляров).
        loot = collections.defaultdict(collections.Counter)
        for pc in by[name]:
            for e in pc['entities']:
                if e.get('loot'): loot[(e['a'], e['ly'], e['b'])][e['loot'].replace('minecraft:chests/', '')] += 1
        for (a, y, b), c in sorted(loot.items()):
            if cells.get((a, y, b), '_').startswith(('chest', 'barrel', 'trapped_chest')):
                lines.append('  loot %d %d %d %s' % (a, y, b, c.most_common(1)[0][0]))
        for y in range(size[1]):
            lines.append('layer %d' % y)
            for b in range(size[2]):
                row = ''.join(palette.get(cells.get((a, y, b)), '_' if cells.get((a, y, b)) == '_' else '.') for a in range(size[0]))
                lines.append('  ' + row)
        lines.append('')
        print(name, 'экземпляров', len(by[name]), 'блоков', sum(1 for v in cells.values() if v != '_'), 'воздуха', sum(1 for v in cells.values() if v == '_'))
    open(out, 'w').write('\n'.join(lines))

def cmd_agree(family, *paths):
    """Насколько экземпляры согласны с большинством (проверка поворотов)."""
    pieces = [pc for pc in load(paths) if family_of(pc) == family]
    by = collections.defaultdict(list)
    for pc in pieces: by[pc['piece']['Template'].replace('minecraft:', '')].append(pc)
    for name in sorted(by):
        size, cells = merge(by[name], family)
        slots = ship_slots(by[name]) if family == 'shipwreck' else [None] * len(by[name])
        for pc, woods in zip(by[name], slots):
            v = oriented(pc, woods); p = pc['piece']
            solid = [(k, s) for k, s in cells.items() if s != '_']
            same = sum(1 for k, s in solid if v.get(k, '').split('[')[0] == s.split('[')[0])
            print(name, p.get('Rotation'), p.get('Mirror'), '%d/%d' % (same, len(solid)))

# Что считает перепись: свои блоки семейства и следы вокруг (разлив, заросли).
CENSUS = {
    'shipwreck': ('planks', 'stairs', 'fence', 'slab', 'log', 'trapdoor', 'door', 'chest'),
    'ruined_portal': ('netherrack', 'magma_block', 'obsidian', 'crying_obsidian', 'stone_bricks', 'cracked_stone_bricks', 'mossy_stone_bricks', 'stone_brick_stairs', 'mossy_stone_brick_stairs', 'stone_brick_slab', 'mossy_stone_brick_slab', 'stone_slab', 'iron_bars', 'gold_block', 'chest', 'lava', 'jungle_leaves', 'vine', 'polished_blackstone_bricks', 'iron_chain'),
}

def cmd_census(family, *paths):
    # «рамка+6»: перепись по всей выгрузке (рамка и запас 6 блоков) — так
    # её считает и наш тест; только свои блоки из списка CENSUS.
    pieces = [pc for pc in load(paths) if family_of(pc) == family]
    if family in CENSUS:
        names = CENSUS[family]; kinds = collections.Counter(); by = collections.defaultdict(collections.Counter)
        for pc in pieces:
            kind = pc['structure'].replace('minecraft:', ''); kinds[kind] += 1
            bb = pc['piece']['BB']
            for a, y, b, x, wy, z, st in pc['cells']:
                name = st.split('[')[0]
                if family == 'shipwreck':
                    # Корабль — только в рамке, дерево — по виду блока.
                    if not (bb[0] <= x <= bb[3] and bb[1] <= wy <= bb[4] and bb[2] <= z <= bb[5]): continue
                    name = name if name == 'chest' else (name[len(wood_of(name)) + 1:] if wood_of(name) else name)
                if name == 'jungle_leaves' and 'persistent=true' not in st: continue
                if name in names: by[kind][name] += 1
        for kind in sorted(kinds):
            print(kind, kinds[kind], ' '.join('%s=%.1f' % (k, by[kind][k] / kinds[kind]) for k in names if by[kind][k]))
        return
    total = collections.Counter(); n = 0
    by_kind = collections.defaultdict(collections.Counter); kinds = collections.Counter()
    for pc in pieces:
        bb = pc['piece']['BB']; one = collections.Counter()
        for a, y, b, x, wy, z, s in pc['cells']:
            if not (bb[0] <= x <= bb[3] and bb[1] <= wy <= bb[4] and bb[2] <= z <= bb[5]): continue
            name = s.split('[')[0]
            if OWN[family].match(name): one[name] += 1
        total.update(one); n += 1
        kind = pc['structure'].replace('minecraft:', ''); kinds[kind] += 1; by_kind[kind].update(one)
    print('экземпляров', n)
    for k, v in total.most_common(): print('  %-36s %.2f' % (k, v / n))
    for kind in sorted(kinds):
        print(kind, kinds[kind], ' '.join('%s=%.1f' % (k, v / kinds[kind]) for k, v in by_kind[kind].most_common(12)))

def cmd_placement(*paths):
    for pc in load(paths):
        p = pc['piece']; bb = p['BB']
        cx, cz = (bb[0] + bb[3]) // 2, (bb[2] + bb[5]) // 2
        # Земля у середины рамки вне её: верх непустого не-воды по столбцам на краю запаса.
        tops = []
        col = collections.defaultdict(list)
        for a, y, b, x, wy, z, s in pc['cells']: col[(x, z)].append((wy, s.split('[')[0]))
        for (x, z), cells in col.items():
            if bb[0] <= x <= bb[3] and bb[2] <= z <= bb[5]: continue
            solid = [wy for wy, s in cells if s not in ('air', 'cave_air', 'water', 'lava') and 'leaves' not in s and s not in ('short_grass', 'tall_grass', 'seagrass', 'tall_seagrass', 'kelp', 'kelp_plant', 'snow')]
            if solid: tops.append(max(solid))
        tops.sort()
        ground = tops[len(tops) // 2] if tops else None
        print(pc['structure'].replace('minecraft:', ''), p.get('Template', '').replace('minecraft:', ''), p.get('VerticalPlacement'), 'TPY', bb[1], 'земля', ground, 'выс', bb[4] - bb[1] + 1, json.dumps(p.get('Properties', {})))

if __name__ == '__main__' and sys.argv[1] in ('templates', 'census', 'placement', 'agree'):
    cmd = sys.argv[1]; args = sys.argv[2:]
    {'templates': cmd_templates, 'census': cmd_census, 'placement': cmd_placement, 'agree': cmd_agree}[cmd](*args)

# ---------------------------------------------------------------------------
# Руины в океане: у каждой клетки — доля экземпляров, где блок стоит.
# Оригинал кладёт куски с неполной целостью (вики: «structure integrity of
# 0.9 if large and 0.8 if small … cracked 0.7 … mossy 0.5»), а холодные руины —
# тремя кусками в одной рамке (brick, cracked, mossy). Поэтому образец руины —
# все её куски вместе, а у клетки — блок большинства и доля, с которой он
# стоит; при постановке клетка кладётся с этой долей. Песок и гравий ниже дна
# вокруг не голосуют (там они и так лежат).
RUIN_TERRAIN = ('gravel', 'sand', 'suspicious_gravel', 'suspicious_sand', 'clay', 'dirt', 'stone', 'sandstone', 'magma_block')
RUIN_OWN = re.compile(r'^(.*stone_brick.*|chiseled_stone_bricks|gravel|suspicious_gravel|sand|suspicious_sand|.*sandstone.*|prismarine|magma_block|sea_lantern|polished_granite|polished_diorite|.*glazed_terracotta|terracotta|bricks|.*_planks|cobblestone|mossy_cobblestone|obsidian|chest|.*_stairs|.*_slab|.*_wall|cracked_stone_bricks|mossy_stone_bricks|stone|granite|diorite|andesite)$')

def ruin_group(pc):
    p = pc['piece']; t = p['Template'].split('/')[-1]
    num = t.rsplit('_', 1)[1]
    big = 'big' if p.get('IsLarge') else 'small'
    return '%s_%s_%s' % (p.get('BiomeType', 'COLD').lower(), big, num)

DROWNED = []

def load_drowned(world):
    """Утопленники мира оригинала (entities/*.mca): места."""
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
                if e.get('id') == 'minecraft:drowned': out.append(tuple(e['Pos']))
    return out

def cmd_ruins(out, world, *paths):
    from template_structs import local_of
    DROWNED.extend(load_drowned(world))
    pieces = load(paths)
    groups = collections.defaultdict(dict)
    for pc in pieces:
        if 'underwater_ruin' not in pc['piece'].get('Template', ''): continue
        key = (pc['structure'], tuple(pc['piece']['BB']))
        groups[ruin_group(pc)].setdefault(key, []).append(pc)
    lines = ['# Руины в океане — образцы по замеру мира оригинала 26.1.2 (tools/measure/',
             '# template_merge.py ruins): руина — все её куски в одной рамке; у знака',
             '# палитры — доля экземпляров, где блок стоит (кладётся с этой долей).', '']
    susp = collections.Counter()
    for name in sorted(groups):
        insts = list(groups[name].values())
        size = size_of(insts[0][0])
        votes = collections.defaultdict(collections.Counter); seen = collections.Counter()
        for parts in insts:
            pc = parts[0]; bb = pc['piece']['BB']
            ring = sorted(g for x, z, g, t in pc['heights'] if g is not None and not (bb[0] <= x <= bb[3] and bb[2] <= z <= bb[5]))
            floor = ring[len(ring) // 2] if ring else -999
            v = {}
            for part in parts:
                for k, s in oriented(part).items(): v[k] = s
            ys = {k: k[1] + bb[1] for k in v}
            for k, s in v.items():
                if not (0 <= k[0] < size[0] and 0 <= k[1] < size[1] and 0 <= k[2] < size[2]): continue
                n = s.split('[')[0]
                if n in RUIN_TERRAIN and ys[k] <= floor: continue
                seen[k] += 1
                if n in ('suspicious_gravel', 'suspicious_sand'): susp[n] += 1
                elif n in ('gravel', 'sand') and ys[k] > floor: susp[n] += 1
                if RUIN_OWN.match(n):
                    s = s.replace('suspicious_gravel', 'gravel').replace('suspicious_sand', 'sand')
                    s = re.sub(r',?dusted=\d', '', s).replace('[]', '')
                    votes[k][s] += 1
        cells = {}
        for k, c in votes.items():
            total = seen[k]; present = sum(c.values())
            share = round(present / total, 1) if total else 0
            if share < 0.3 or total < 2 and present < 1: continue
            cells[k] = (c.most_common(1)[0][0], min(share, 1.0))
        palette = {}
        for st in sorted(set(cells.values())): palette[st] = SYMBOLS[len(palette)]
        lines.append('template %s %d %d %d %d' % (name, size[0], size[2], size[1], len(insts)))
        for (st, share), ch in palette.items(): lines.append('  %s %s %.1f' % (ch, st, share))
        loot = collections.defaultdict(collections.Counter)
        for parts in insts:
            for part in parts:
                for e in part['entities']:
                    if e.get('loot') and 'chests/' in e['loot']: loot[(e['a'], e['ly'], e['b'])][e['loot'].replace('minecraft:chests/', '')] += 1
        for (a, y, b), c in sorted(loot.items()):
            if cells.get((a, y, b), ('',))[0].startswith('chest'):
                lines.append('  loot %d %d %d %s' % (a, y, b, c.most_common(1)[0][0]))
        # Утопленники при сложении (сущности мира оригинала в рамке): места
        # в образце и доля экземпляров, где утопленник есть.
        marks = collections.Counter()
        for parts in insts:
            pc = parts[0]; bb = pc['piece']['BB']; f = local_of(pc['piece'])
            for e in DROWNED:
                x, y, z = e
                if bb[0] <= x <= bb[3] + 1 and bb[2] <= z <= bb[5] + 1 and bb[1] - 1 <= y <= bb[4] + 1:
                    a, b = f(int(x // 1), int(z // 1)); marks[(a, int(y // 1) - bb[1], b)] += 1
        for (a, y, b), n in sorted(marks.items()):
            lines.append('  drowned %d %d %d %.1f' % (a, y, b, min(1.0, n / len(insts))))
        for y in range(size[1]):
            lines.append('layer %d' % y)
            for b in range(size[2]):
                lines.append('  ' + ''.join(palette.get(cells.get((a, y, b)), '.') for a in range(size[0])))
        lines.append('')
        print(name, 'экземпляров', len(insts), 'клеток', len(cells), 'палитра', len(palette))
    print('подозрительных:', dict(susp))
    open(out, 'w').write('\n'.join(lines))

def cmd_cluster(*paths):
    """Малые руины вокруг большой: сколько и где (от точки большой)."""
    pieces = load(paths)
    starts = collections.defaultdict(list)
    for pc in pieces:
        if 'underwater_ruin' not in pc['piece'].get('Template', ''): continue
        starts[(pc['structure'], tuple(pc['chunk']))].append(pc)
    counts = collections.Counter(); offs = []
    for key, pcs in starts.items():
        bigs = [pc for pc in pcs if pc['piece'].get('IsLarge')]
        if not bigs: continue
        big = bigs[0]['piece']; bx, bz = (big['BB'][0] + big['BB'][3]) / 2, (big['BB'][2] + big['BB'][5]) / 2
        smalls = {tuple(pc['piece']['BB']): pc for pc in pcs if not pc['piece'].get('IsLarge')}
        counts[len(smalls)] += 1
        for bb, pc in smalls.items():
            offs.append(((bb[0] + bb[3]) / 2 - bx, (bb[2] + bb[5]) / 2 - bz, bb[1] - big['BB'][1]))
    print('малых у большой:', sorted(counts.items()))
    print('смещения середин (dx, dz, dy):', sorted((round(a), round(b), c) for a, b, c in offs))

if __name__ == '__main__' and sys.argv[1] in ('ruins', 'cluster'):
    {'ruins': cmd_ruins, 'cluster': cmd_cluster}[sys.argv[1]](*sys.argv[2:])
    sys.exit(0)
