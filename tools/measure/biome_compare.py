# Сравнивает вывод biome_census.py (оригинал) и теста `biome_neighbours` (наш).
#
# python3 tools/measure/biome_compare.py vanilla.tsv ours.tsv
#
# Печатает: доли биомов, пятна (медиана, доля крошечных ≤3 клеток) и
# соседства, которые есть у одного мира и почти не бывают у другого.
import sys, collections

def load(path):
    share, patch, pair, height = {}, {}, collections.defaultdict(dict), {}
    for line in open(path, encoding='utf8'):
        parts = line.rstrip('\n').split('\t')
        if parts[0] == 'share':
            share[parts[1]] = float(parts[2])
        elif parts[0] == 'patch':
            patch[parts[1]] = (int(parts[2]), int(parts[3]), int(parts[4]), int(parts[5]), float(parts[6]))
        elif parts[0] == 'pair':
            pair[parts[1]][parts[2]] = float(parts[3])
        elif parts[0] == 'height':
            height[parts[1]] = tuple(int(v) for v in parts[2:7])
    return share, patch, pair, height

va_share, va_patch, va_pair, va_height = load(sys.argv[1])
our_share, our_patch, our_pair, our_height = load(sys.argv[2])
names = sorted(set(va_share) | set(our_share), key=lambda n: -va_share.get(n, 0))

print('биом | доля ориг. / наша | пятен | медиана пятна (клеток) | крошечных ≤3')
for name in names:
    vp = va_patch.get(name, (0, 0, 0, 0, 0))
    op = our_patch.get(name, (0, 0, 0, 0, 0))
    print('%-26s %5.1f%% / %5.1f%% | %5d / %5d | %4d / %4d | %3.0f%% / %3.0f%%' % (
        name, 100 * va_share.get(name, 0), 100 * our_share.get(name, 0), vp[0], op[0], vp[1], op[1],
        100 * vp[4], 100 * op[4]))

print()
# Судить о соседстве можно, только если у оригинала оба биома встречаются
# часто: иначе «0%» значит лишь, что их не оказалось рядом в выборке.
common = {n for n in va_share if va_share[n] >= 0.01 and va_patch.get(n, (0,))[0] >= 20}
print('соседи, которых у оригинала почти нет (<1% границы), а у нас ≥3% (оба биома у оригинала частые):')
for name in names:
    for other, part in sorted(our_pair.get(name, {}).items(), key=lambda kv: -kv[1]):
        if name not in common or other not in common:
            continue
        if part >= 0.03 and va_pair.get(name, {}).get(other, 0) < 0.01 and name in va_pair:
            print('  %s — %s: у нас %.0f%%, у оригинала %.1f%%' % (name, other, 100 * part,
                  100 * va_pair[name].get(other, 0)))
print()
print('соседи, частые у оригинала (≥5%), а у нас редкие (<1%) (оба биома у оригинала частые):')
for name in names:
    for other, part in sorted(va_pair.get(name, {}).items(), key=lambda kv: -kv[1]):
        if name not in common or other not in common:
            continue
        if part >= 0.05 and our_pair.get(name, {}).get(other, 0) < 0.01 and name in our_pair:
            print('  %s — %s: у оригинала %.0f%%, у нас %.1f%%' % (name, other, 100 * part,
                  100 * our_pair[name].get(other, 0)))

print()
print('высота поверхности (дно под водой), p5/p25/p50/p75/p95: оригинал | у нас')
for name in names:
    if name in va_height and name in our_height:
        print('  %-26s %s | %s' % (name, '/'.join(map(str, va_height[name])), '/'.join(map(str, our_height[name]))))
