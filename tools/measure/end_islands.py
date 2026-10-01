#!/usr/bin/env python3
# Разбор переписи Энда (end_census.py): плотность островов по расстоянию,
# островы как связные куски, профиль главного острова, прочие блоки.
#
#   python3 tools/measure/end_islands.py <end.pkl> [radial|islands|blocks|main]
import sys, pickle, math, collections

def load(path):
    data = pickle.load(open(path, 'rb'))
    cols = {}
    for (cx, cz), col in data['columns'].items():
        for i, c in enumerate(col):
            if c: cols[(cx * 16 + (i & 15), cz * 16 + (i >> 4))] = c
    return data, cols

def radial(cols, width=100):
    by = collections.defaultdict(lambda: [0, 0, 0])
    for (x, z), (lo, hi, n) in cols.items():
        r = int(math.hypot(x, z) // width)
        e = by[r]; e[0] += 1; e[1] += n; e[2] += hi - lo + 1
    print('расстояние  столбцов-с-камнем  камня/столбец  доля-площади(кольцо, полная)')
    for r in sorted(by):
        area = math.pi * (((r + 1) * width) ** 2 - (r * width) ** 2)
        print('%5d-%5d %9d %7.1f %.4f' % (r * width, (r + 1) * width, by[r][0], by[r][1] / by[r][0], by[r][0] / area))

def components(cols):
    seen = set(); out = []
    for start in cols:
        if start in seen: continue
        stack = [start]; seen.add(start); comp = []
        while stack:
            p = stack.pop(); comp.append(p)
            for d in ((1,0),(-1,0),(0,1),(0,-1)):
                q = (p[0]+d[0], p[1]+d[1])
                if q in cols and q not in seen:
                    seen.add(q); stack.append(q)
        out.append(comp)
    return out

def islands(cols):
    comps = components(cols)
    print('островов', len(comps))
    rows = []
    for comp in comps:
        cx = sum(p[0] for p in comp) / len(comp); cz = sum(p[1] for p in comp) / len(comp)
        tops = [cols[p][1] for p in comp]; bots = [cols[p][0] for p in comp]
        vol = sum(cols[p][2] for p in comp)
        xs = [p[0] for p in comp]; zs = [p[1] for p in comp]
        rows.append((len(comp), cx, cz, max(tops), min(tops), min(bots), vol, max(xs)-min(xs)+1, max(zs)-min(zs)+1))
    rows.sort(reverse=True)
    for r in rows[:25]:
        print('площадь %7d центр (%7.0f,%7.0f) верх %d..%d низ %d объём %8d ширина %dx%d' % (r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]))
    return rows

def blocks(data):
    for name, ys in sorted(data['blocks'].items(), key=lambda kv: -sum(kv[1].values())):
        tot = sum(ys.values())
        print('%-24s %10d  y %d..%d' % (name, tot, min(ys), max(ys)))

if __name__ == '__main__':
    data, cols = load(sys.argv[1])
    mode = sys.argv[2] if len(sys.argv) > 2 else 'radial'
    {'radial': lambda: radial(cols), 'islands': lambda: islands(cols), 'blocks': lambda: blocks(data)}[mode]()
