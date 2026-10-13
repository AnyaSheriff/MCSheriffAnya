"""Compare matching four-block censuses; normalize patch counts by sample area.

python3 tools/measure/biome_distribution_summary.py vanilla.tsv before.tsv after.tsv
"""
import sys


def load(path):
    data = {'share': {}, 'patch': {}, 'patch_area': {}}
    cells = None
    with open(path, encoding='utf8') as source:
        for line in source:
            r = line.rstrip().split('\t')
            if r[0] in data:
                data[r[0]][r[1]] = list(map(float, r[2:]))
            if r[0] == 'total':
                cells = int(r[1])
            if r[:2] == ['meta', 'cells']:
                cells = int(r[2])
    if not cells:
        raise ValueError('Census must declare its sampled cell count')
    data['cells'] = cells
    return data


def main(paths):
    va, before, after = map(load, paths)
    names = sorted(set(va['share']) | set(before['share']) | set(after['share']),
                   key=lambda n: -va['share'].get(n, [0])[0])
    print('| Biome | Vanilla % | Before % | After % | Relative error | Target |')
    print('|---|---:|---:|---:|---:|---|')
    failures = 0
    for name in names:
        v, b, a = (d['share'].get(name, [0])[0] for d in (va, before, after))
        error = (a/v-1) if v else None
        status = ('pass' if abs(error) <= .3 else 'FAIL') if v >= .005 else 'below 0.5%'
        failures += status == 'FAIL'
        label = f'{error:+.1%}' if error is not None else '—'
        print(f'| {name} | {v*100:.3f} | {b*100:.3f} | {a*100:.3f} | {label} | {status} |')
    print(f'\nCommon-biome failures: {failures}.')
    print('\nPatch sizes are four-neighbour components in 4×4-block cells, clipped by the same nine square boundaries. Counts below are per reference-sized area, not pooled across unequal seed counts.')
    print('\n| Biome | Patches V / before / after | Median cells V / before / after | Area-weighted median blocks² V / before / after |')
    print('|---|---:|---:|---:|')
    for name in names:
        if va['share'].get(name, [0])[0] < .005:
            continue
        counts, medians, weighted = [], [], []
        for d in (va, before, after):
            row = d['patch'].get(name, [0]*5)
            counts.append(f'{row[0]*va["cells"]/d["cells"]:.1f}')
            medians.append(str(int(row[1])))
            weighted.append(str(int(d['patch_area'].get(name, [0])[0])))
        print(f'| {name} | {" / ".join(counts)} | {" / ".join(medians)} | {" / ".join(weighted)} |')


if __name__ == '__main__':
    main(sys.argv[1:])
