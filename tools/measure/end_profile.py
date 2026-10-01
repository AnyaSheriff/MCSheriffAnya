#!/usr/bin/env python3
# Профиль островов Энда оригинала: толщина, верх и низ в зависимости от
# расстояния до края острова (по 4 соседям), отдельно главный остров и
# внешние. Вход — перепись end_census.py.
#
#   python3 tools/measure/end_profile.py <end.pkl>
import sys, collections, math
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from end_islands import load, components

def edge_distance(cols, comp):
    dist = {}
    queue = collections.deque()
    for p in comp:
        if any((p[0]+d[0], p[1]+d[1]) not in cols for d in ((1,0),(-1,0),(0,1),(0,-1))):
            dist[p] = 1; queue.append(p)
    while queue:
        p = queue.popleft()
        for d in ((1,0),(-1,0),(0,1),(0,-1)):
            q = (p[0]+d[0], p[1]+d[1])
            if q in cols and q not in dist:
                dist[q] = dist[p] + 1; queue.append(q)
    return dist

if __name__ == '__main__':
    data, cols = load(sys.argv[1])
    comps = components(cols)
    groups = {'main': collections.defaultdict(list), 'outer': collections.defaultdict(list)}
    for comp in comps:
        cx = sum(p[0] for p in comp) / len(comp); cz = sum(p[1] for p in comp) / len(comp)
        if len(comp) < 200: continue
        near = math.hypot(cx, cz) < 400
        # для главного острова берём самую большую компоненту у нуля
        if near and len(comp) < 5000: continue
        dist = edge_distance(cols, comp)
        g = groups['main' if near else 'outer']
        for p, d in dist.items():
            lo, hi, n = cols[p]
            g[min(d, 60)].append((hi, lo, n))
    for name, g in groups.items():
        print('==', name)
        print(' d  столбцов  верх  низ  толщина(число блоков)  (верх и низ — средние y)')
        for d in sorted(g):
            L = g[d]
            print('%3d %8d %6.1f %6.1f %6.1f' % (d, len(L), sum(a for a,b,c in L)/len(L), sum(b for a,b,c in L)/len(L), sum(c for a,b,c in L)/len(L)))
