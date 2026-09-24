# Сравнивает дно: вывод seabed_census.py (оригинал) и теста `seabed_census`
# (наш).
#
# python3 tools/measure/seabed_compare.py vanilla.tsv ours.tsv
import sys, collections

def load(path):
    depth, layers = {}, collections.defaultdict(dict)
    for line in open(path, encoding='utf8'):
        parts = line.rstrip('\n').split('\t')
        if parts[0] == 'depth':
            depth[parts[1]] = tuple(int(v) for v in parts[2:6])
        elif parts[0] == 'layer':
            layers[(parts[1], int(parts[2]))][parts[3]] = float(parts[4])
    return depth, layers

va_depth, va_layers = load(sys.argv[1])
our_depth, our_layers = load(sys.argv[2])

for name in sorted(set(va_depth) & set(our_depth)):
    v, o = va_depth[name], our_depth[name]
    print('%s: глубина 10/50/90%% — ориг. %d/%d/%d, у нас %d/%d/%d' % (name, v[0], v[1], v[2], o[0], o[1], o[2]))
    for k in range(5):
        blocks = set(va_layers[(name, k)]) | set(our_layers[(name, k)])
        row = sorted(blocks, key=lambda b: -va_layers[(name, k)].get(b, 0) - our_layers[(name, k)].get(b, 0))[:5]
        cells = ['%s %.0f/%.0f' % (b, 100 * va_layers[(name, k)].get(b, 0), 100 * our_layers[(name, k)].get(b, 0)) for b in row]
        print('   %s: %s' % ('верх' if k == 0 else '-%d' % k, ', '.join(cells)))
print()
print('только у оригинала:', ', '.join(sorted(set(va_depth) - set(our_depth))))
print('только у нас:', ', '.join(sorted(set(our_depth) - set(va_depth))))
