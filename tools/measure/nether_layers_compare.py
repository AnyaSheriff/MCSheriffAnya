#!/usr/bin/env python3
# Сверка Незера по слоям: оригинал против нашего (вывод nether_layers.py
# и ignored-теста nether_layers).
#
#   python3 tools/measure/nether_layers_compare.py <оригинал.tsv> <наш.tsv> [полоса]
import sys, collections

GROUPS = {
    'пусто': ('air', 'cave_air'),
    'незеррак': ('netherrack',),
    'лава': ('lava',),
    'базальт': ('basalt',),
    'чернокамень': ('blackstone',),
    'пески душ': ('soul_sand', 'soul_soil'),
    'нилий': ('crimson_nylium', 'warped_nylium'),
    'гравий': ('gravel',),
    'магма': ('magma_block',),
    'бедрок': ('bedrock',),
}

def load(path):
    d = collections.defaultdict(dict)
    for line in open(path):
        a, b, c = line.rstrip('\n').split('\t')
        d[a][b] = float(c)
    return d

def dist(d, key):
    return {int(k): v for k, v in d[key].items()}

def mean(dist_):
    return sum(k * v for k, v in dist_.items()) / (sum(dist_.values()) or 1)

def main():
    v, o = load(sys.argv[1]), load(sys.argv[2])
    band = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    print('чанков: оригинал %d, наш %d' % (v['chunks']['-'], o['chunks']['-']))
    print('\n== доли по полосам высоты (оригинал / наш)')
    print('%-7s' % 'y' + ''.join('%-17s' % g for g in GROUPS))
    worst = []
    for y0 in range(0, 128, band):
        cells = []
        for g, names in GROUPS.items():
            a = sum(v['y%d' % y].get(n, 0) for y in range(y0, y0 + band) for n in names) / band
            b = sum(o['y%d' % y].get(n, 0) for y in range(y0, y0 + band) for n in names) / band
            cells.append('%5.3f/%5.3f    ' % (a, b))
            worst.append((abs(a - b), y0, g, a, b))
        print('%-7s' % ('%d-%d' % (y0, y0 + band - 1)) + ''.join(cells))
    print('\nсамые большие расхождения:')
    for diff, y0, g, a, b in sorted(worst, reverse=True)[:12]:
        print('  y %3d..%3d %-12s %.3f / %.3f (%+.3f)' % (y0, y0 + band - 1, g, a, b, b - a))
    print('\n== столбцы (над морем лавы, y 32..125)')
    for key in ('floors', 'ceilings'):
        a, b = dist(v, key), dist(o, key)
        print('  %-9s среднее %.3f / %.3f; ' % (key, mean(a), mean(b)) +
              ', '.join('%d: %.3f/%.3f' % (k, a.get(k, 0), b.get(k, 0)) for k in range(0, 8)))
    for key in ('gap', 'slab'):
        a, b = dist(v, key), dist(o, key)
        sa, sb = sum(a.values()), sum(b.values())
        print('  %-9s на столбец %.3f / %.3f, толщина средняя %.1f / %.1f' % (key, sa, sb, mean(a), mean(b)))
        edges = [1, 2, 3, 5, 9, 17, 33, 65, 100]
        parts = []
        for lo, hi in zip(edges, edges[1:]):
            pa = sum(p for k, p in a.items() if lo <= k < hi) / (sa or 1)
            pb = sum(p for k, p in b.items() if lo <= k < hi) / (sb or 1)
            parts.append('%d-%d: %.2f/%.2f' % (lo, hi - 1, pa, pb))
        print('            ' + ', '.join(parts))
    for key in ('floor_y', 'ceiling_y'):
        a, b = dist(v, key), dist(o, key)
        print('  %-9s по 16: ' % key + ', '.join('%d: %.3f/%.3f' % (y0, sum(a.get(y, 0) for y in range(y0, y0 + 16)),
              sum(b.get(y, 0) for y in range(y0, y0 + 16))) for y0 in range(32, 128, 16)))
    if any(k.startswith('reach') for k in o):
        print('  пустота вдаль (доля пустых в d блоках по x от пустого), оригинал / наш:')
        for key in sorted(k for k in o if k.startswith('reach')):
            print('    y %s: ' % key[5:] + ', '.join('d%s %.3f/%.3f' % (d, v.get(key, {}).get(d, 0), p)
                  for d, p in sorted(o[key].items(), key=lambda kv: int(kv[0]))))
    print('\n== объём на столбец (блоков, y 0..127)')
    total = collections.Counter(); ours = collections.Counter()
    for y in range(128):
        for n, p in v['y%d' % y].items(): total[n] += p
        for n, p in o['y%d' % y].items(): ours[n] += p
    for n, c in total.most_common(24):
        print('  %-22s %7.3f / %7.3f' % (n, c, ours[n]))

if __name__ == '__main__':
    main()
