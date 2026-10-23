# Снимает куски заброшенных лагерей 26.3 с официального сервера: /place template кладёт каждый
# образец в чистый воздух, потом region-файлы читаются только на чтение (никаких данных из jar).
# Сервер — свой, порт 26100–26199, RCON.
#
# python3 -I tools/measure/camp_templates_263.py <папка мира> <порт rcon> <пароль> <names.json> <выход.txt>
#
# names.json: {"tent/<биом>": [номера], "camp/default/<вид>": [номера], "camp/<биом>": [номера]}.
# Выход — в формате src/world/template.rs; перед первым `template` идут строки
# Ячейка перед укладкой заливается барьером: воздух образца затирает барьер, нетронутое остаётся «.».
# `jigsaw <образец> a y b <orientation> <final_state> <pool> <name> <target>`.
import sys, os, json, time, math, struct, zlib, socket, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import census_263 as c

class Rcon:
    def __init__(s, port, password):
        s.s = socket.create_connection(('127.0.0.1', port)); s.i = 0; s.call(3, password)
    def call(s, k, t):
        s.i += 1; b = t.encode() + b'\0\0'
        s.s.sendall(struct.pack('<iii', len(b) + 8, s.i, k) + b)
        n = struct.unpack('<i', s.rv(4))[0]
        return s.rv(n)[8:-2].decode('utf8', 'replace')
    def rv(s, n):
        o = b''
        while len(o) < n: o += s.s.recv(n - len(o))
        return o
    def run(s, t): return s.call(2, t)

CELL, HEIGHT, X0, Z0, Y0 = 32, 31, 30000, 30000, 150

def names(spec):
    out = []
    for key, ns in spec.items():
        for n in ns:
            if key.startswith('tent/'):
                b = key[5:]; out.append('abandoned_camp/tent/%s/tent_%s_%d' % (b, b, n))
            elif key.startswith('camp/default/'):
                k = key[13:]; out.append('abandoned_camp/camp/default/campsite_default_%s_%d' % (k, n))
            else:
                b = key[5:]; out.append('abandoned_camp/camp/%s/campsite_%s_%d' % (b, b, n))
    return out

def pname(p):
    if isinstance(p, str): return p.replace('minecraft:', ''), {}
    name = (p.get('id') or p.get('') or p.get('Name') or p.get('name')).replace('minecraft:', '')
    return name, p.get('properties', {}) or {k: v for k, v in p.items() if k not in ('id', '', 'Name', 'name', 'properties')}

def text(name, props):
    return name if not props else '%s[%s]' % (name, ','.join('%s=%s' % (k, props[k]) for k in sorted(props)))

def main():
    world, port, password, spec_path, out_path = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4], sys.argv[5]
    all_names = names(json.load(open(spec_path)))
    rcon = Rcon(port, password)
    per_row = 18
    for cx in range(0, per_row * CELL // 16 + 2, 16):
        for cz in range(0, (len(all_names) // per_row + 2) * CELL // 16 + 2, 16):
            rcon.run('forceload add %d %d %d %d' % (X0 + cx * 16, Z0 + cz * 16, X0 + cx * 16 + 255, Z0 + cz * 16 + 255))
    time.sleep(8)
    cells = {}
    for i, name in enumerate(all_names):
        ox, oz = X0 + (i % per_row) * CELL, Z0 + (i // per_row) * CELL
        rcon.run('fill %d %d %d %d %d %d barrier' % (ox, Y0, oz, ox + CELL - 1, Y0 + HEIGHT - 1, oz + CELL - 1))
        r = rcon.run('place template minecraft:%s %d %d %d' % (name, ox, Y0, oz))
        if 'Loaded' not in r: print('не лёг', name, r)
        cells[name] = (ox, oz)
    rcon.run('save-all flush'); time.sleep(5); rcon.run('save-all flush'); time.sleep(3)
    region = os.path.join(world, 'dimensions/minecraft/overworld/region')
    chunk_blocks = {}
    lo_x, hi_x = X0 >> 9, (X0 + per_row * CELL) >> 9
    lo_z, hi_z = Z0 >> 9, (Z0 + (len(all_names) // per_row + 1) * CELL) >> 9
    ents = {}
    for rx in range(lo_x, hi_x + 1):
        for rz in range(lo_z, hi_z + 1):
            f = os.path.join(region, 'r.%d.%d.mca' % (rx, rz))
            if not os.path.exists(f): continue
            for root in c.chunks(f):
                for e in root.get('block_entities', []): ents[(e['x'], e['y'], e['z'])] = e
                for s in root.get('sections', []):
                    bs = s.get('block_states')
                    if not bs or s['Y'] * 16 + 15 < Y0 or s['Y'] * 16 > Y0 + HEIGHT: continue
                    pal = [pname(p) for p in bs['palette']]
                    ids = [0] * 4096 if len(pal) == 1 else c.unpack(bs['data'], max(4, math.ceil(math.log2(len(pal)))), 4096)
                    for k, pid in enumerate(ids):
                        nm, pr = pal[pid]
                        if nm == 'barrier': continue
                        wx, wz, wy = root['xPos'] * 16 + (k & 15), root['zPos'] * 16 + ((k >> 4) & 15), s['Y'] * 16 + (k >> 8)
                        if not (Y0 <= wy < Y0 + HEIGHT): continue
                        if not any(ox <= wx < ox + CELL and oz <= wz < oz + CELL for ox, oz in cells.values()): continue
                        chunk_blocks[(wx, wy, wz)] = (nm if nm != 'cave_air' else 'air', pr)
    head, body = [], []
    for name in all_names:
        ox, oz = cells[name]
        mine = {p: v for p, v in chunk_blocks.items() if ox <= p[0] < ox + CELL and oz <= p[2] < oz + CELL and Y0 <= p[1] < Y0 + HEIGHT}
        if not mine:
            print('пусто', name); continue
        mx = [min(p[i] for p in mine) for i in range(3)]; Mx = [max(p[i] for p in mine) for i in range(3)]
        # Образец всегда начинается в углу ячейки (ox, Y0, oz): рамка берётся от неё.
        mx = [ox, Y0, oz]
        w, h, l = Mx[0] - mx[0] + 1, Mx[1] - mx[1] + 1, Mx[2] - mx[2] + 1
        palette, letters, loot = {}, {}, []
        alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@%&*+=?!'
        for (x, y, z), (nm, pr) in sorted(mine.items()):
            t = text(nm, pr)
            if nm == 'air':
                palette[(x, y, z)] = '_'
                continue
            if nm == 'jigsaw':
                e = ents.get((x, y, z), {})
                fs = e.get('final_state', 'air').replace('minecraft:', '')
                head.append('jigsaw %s %d %d %d %s %s %s %s %s' % (name, x - mx[0], y - mx[1], z - mx[2], pr.get('orientation', 'up'), fs.replace(' ', ''), e.get('pool', '').replace('minecraft:', ''), e.get('name', '').replace('minecraft:', ''), e.get('target', '').replace('minecraft:', '')))
                if fs == 'air':
                    palette[(x, y, z)] = '_'
                    continue
                t = fs
            if t not in letters:
                letters[t] = alphabet[len(letters)]
            palette[(x, y, z)] = letters[t]
            e = ents.get((x, y, z))
            if e and 'LootTable' in e: loot.append((x - mx[0], y - mx[1], z - mx[2], e['LootTable'].replace('minecraft:', '')))
        body.append('template %s %d %d %d' % (name, w, l, h))
        for t, ch in letters.items(): body.append('  %s %s' % (ch, t))
        for a, y, b, tbl in loot: body.append('  loot %d %d %d %s' % (a, y, b, tbl))
        for yy in range(h):
            body.append('layer %d' % yy)
            for zz in range(l):
                body.append(''.join(palette.get((mx[0] + xx, mx[1] + yy, mx[2] + zz), '.') for xx in range(w)))
    open(out_path, 'w').write('# Заброшенные лагеря 26.3: куски сняты с официального сервера (/place template + чтение region,\n# tools/measure/camp_templates_263.py). «.» — воздух образца (рельеф остаётся).\n' + '\n'.join(head) + '\n' + '\n'.join(body) + '\n')
    print('записано образцов:', len(all_names), 'jigsaw:', len(head))

if __name__ == '__main__':
    main()
