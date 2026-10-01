#!/usr/bin/env python3
# Сверка нашего Энда с оригиналом: доля площади по кольцам, толщина,
# верх и низ, размеры островов. Оригинал — перепись end_census.py (.pkl),
# наш — TSV из ignored-теста `end_census` (x, z, низ, верх через 4 блока).
#
#   python3 tools/measure/end_compare.py <end.pkl> <ours.tsv>
import sys, math, pickle, collections

STEP = 4

def vanilla(path):
    data = pickle.load(open(path, 'rb'))
    cols = {}
    for (cx, cz), col in data['columns'].items():
        for i, c in enumerate(col):
            x, z = cx * 16 + (i & 15), cz * 16 + (i >> 4)
            if c and x % STEP == 0 and z % STEP == 0 and max(abs(x), abs(z)) < 2040:
                cols[(x, z)] = (c[0], c[1])
    return cols

def ours(path):
    cols = {}
    for line in open(path):
        x, z, lo, hi = map(int, line.split('\t'))
        cols[(x, z)] = (lo, hi)
    return cols

def components(cols):
    seen = set(); sizes = []
    for start in cols:
        if start in seen: continue
        stack = [start]; seen.add(start); n = 0
        while stack:
            p = stack.pop(); n += 1
            for d in ((STEP, 0), (-STEP, 0), (0, STEP), (0, -STEP)):
                q = (p[0] + d[0], p[1] + d[1])
                if q in cols and q not in seen:
                    seen.add(q); stack.append(q)
        sizes.append(n * STEP * STEP)
    return sorted(sizes, reverse=True)

def report(name, cols):
    cells = collections.Counter(); area = collections.Counter()
    for (x, z) in cols:
        cells[min(int(math.hypot(x, z) // 200), 14)] += 1
    for x in range(-2040, 2040, STEP):
        for z in range(-2040, 2040, STEP):
            area[min(int(math.hypot(x, z) // 200), 14)] += 1
    print('==', name, 'столбцов', len(cols))
    print('доля площади по кольцам 200:', ' '.join('%d:%.2f' % (k * 200, cells[k] / area[k]) for k in sorted(area) if k < 11))
    outer = [v for (x, z), v in cols.items() if math.hypot(x, z) > 1100]
    th = sorted(hi - lo + 1 for lo, hi in outer)
    q = lambda f: th[int(f * (len(th) - 1))]
    print('толщина (внешние): средняя %.1f, квантили 10/25/50/75/90: %d %d %d %d %d' % (sum(th) / len(th), q(.1), q(.25), q(.5), q(.75), q(.9)))
    tops = sorted(hi for lo, hi in outer); bots = sorted(lo for lo, hi in outer)
    print('верх: средний %.1f, 5/50/95: %d %d %d; низ: средний %.1f, 5/50/95: %d %d %d' % (sum(tops) / len(tops), tops[len(tops) // 20], tops[len(tops) // 2], tops[-len(tops) // 20], sum(bots) / len(bots), bots[len(bots) // 20], bots[len(bots) // 2], bots[-len(bots) // 20]))
    sizes = components({k: v for k, v in cols.items() if math.hypot(*k) > 400})
    total = sum(sizes)
    big = [s for s in sizes if s > 20000]
    print('внешние острова: всего %d, крупнее 20000: %d, доля площади в крупных %.2f, медиана %d, топ-5: %s' % (len(sizes), len(big), sum(big) / total, sizes[len(sizes) // 2], sizes[:5]))
    main = [v for (x, z), v in cols.items() if math.hypot(x, z) < 150]
    print('главный остров (столбцов через 4): %d, верх в центре %s' % (len(main), cols.get((0, 0))))

if __name__ == '__main__':
    report('оригинал', vanilla(sys.argv[1]))
    report('наш', ours(sys.argv[2]))
