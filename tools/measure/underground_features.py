#!/usr/bin/env python3
"""Read-only 26.1.2 shell census; no game code or generated reports.

Six-neighbour components containing amethyst identify geodes. Calcite-only
mountain deposits are excluded, tiny detached fragments are reported separately.
Binary input/output is x,y,z (signed little-endian i32), material (u8).
"""
import argparse
import collections
import json
import os
import statistics
import struct
from structures_audit import records, unpack, window

MATERIALS = ('amethyst_block', 'budding_amethyst', 'calcite', 'smooth_basalt')
CELL = struct.Struct('<iiiB')


def extract(world, output):
    chunks = 0
    counts = collections.Counter()
    with open(output, 'wb') as out:
        for cx, cz, root in records(os.path.join(world, 'dimensions/minecraft/overworld/region')):
            if root.get('Status') not in ('full', 'minecraft:full'):
                continue
            chunks += 1
            for section in root.get('sections', []):
                states = section.get('block_states', {})
                names = [p['Name'].removeprefix('minecraft:') for p in states.get('palette', [])]
                wanted = {i: MATERIALS.index(n) for i, n in enumerate(names) if n in MATERIALS}
                if not wanted:
                    continue
                ids = [0] * 4096 if len(names) == 1 else unpack(states['data'], max(4, (len(names)-1).bit_length()), 4096)
                sy = section['Y'] if section['Y'] < 128 else section['Y'] - 256
                for i, state in enumerate(ids):
                    if state in wanted:
                        material = wanted[state]
                        out.write(CELL.pack(cx*16+i%16, sy*16+i//256, cz*16+i//16%16, material))
                        counts[MATERIALS[material]] += 1
            if chunks % 4096 == 0:
                print('vanilla chunks', chunks, flush=True)
    print('extracted', chunks, dict(counts), flush=True)


def encode(x, y, z):
    return ((x + 32768) << 26) | ((z + 32768) << 10) | (y + 128)


def decode(p):
    return (p >> 26) - 32768, (p & 1023) - 128, ((p >> 10) & 65535) - 32768


def components(cells):
    result = []
    while cells:
        first, material = cells.popitem()
        todo = [first]
        counts = [0]*4
        counts[material] = 1
        low = list(decode(first))
        high = low.copy()
        sums = low.copy()
        while todo:
            p = todo.pop()
            for q in (p-1, p+1, p-1024, p+1024, p-(1 << 26), p+(1 << 26)):
                m = cells.pop(q, None)
                if m is None:
                    continue
                todo.append(q)
                counts[m] += 1
                for axis, value in enumerate(decode(q)):
                    low[axis] = min(low[axis], value)
                    high[axis] = max(high[axis], value)
                    sums[axis] += value
        if counts[0] + counts[1]:
            total = sum(counts)
            result.append(dict(blocks=dict(zip(MATERIALS, counts)), total=total,
                               box=low+high, center=[v/total for v in sums],
                               size=[hi-lo+1 for lo, hi in zip(low, high)]))
    return result


def census(path):
    windows = collections.defaultdict(dict)
    with open(path, 'rb') as f:
        while batch := f.read(CELL.size*65536):
            for x, y, z, material in CELL.iter_unpack(batch):
                windows[window(x//16, z//16)][encode(x,y,z)] = material
    all_components = []
    for w in sorted(windows):
        found = components(windows[w])
        for c in found:
            c['window'] = w
            # A cut shell at a measurement-window edge is kept but labelled.
            gx, gz = map(int, w.split(','))
            half = 1024 if gx == gz == 0 else 512
            c['edge'] = any(c['box'][axis] <= center-half or c['box'][axis+3] >= center+half-1
                            for axis, center in ((0,gx*8192),(2,gz*8192)))
        all_components.extend(found)
        print('window', w, 'amethyst components', len(found), flush=True)
    major = [c for c in all_components if c['total'] >= 64]
    fragments = [c for c in all_components if c['total'] < 64]
    counts = collections.Counter()
    for c in all_components:
        counts.update(c['blocks'])
    hist = collections.Counter(int(c['center'][1] // 16)*16 for c in major)
    summary = dict(components=len(all_components), geodes=len(major), fragments=len(fragments),
                   edge_geodes=sum(c['edge'] for c in major), blocks=dict(counts),
                   mean_blocks=statistics.mean(c['total'] for c in major),
                   median_blocks=statistics.median(c['total'] for c in major),
                   mean_budding=statistics.mean(c['blocks']['budding_amethyst'] for c in major),
                   budding_sum_squares=sum(c['blocks']['budding_amethyst']**2 for c in major),
                   budding_standard_deviation=statistics.stdev(c['blocks']['budding_amethyst'] for c in major) if len(major)>1 else 0.0,
                   budding_fraction=counts['budding_amethyst']/(counts['amethyst_block']+counts['budding_amethyst']),
                   mean_size=[statistics.mean(c['size'][a] for c in major) for a in range(3)],
                   center_y_histogram=dict(sorted(hist.items())))
    return dict(summary=summary, components=sorted(all_components, key=lambda c:c['box']))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('shells')
    p.add_argument('output')
    p.add_argument('--world', help='extract shells from this saved world first')
    args = p.parse_args()
    if args.world:
        extract(args.world, args.shells)
    result = census(args.shells)
    with open(args.output, 'w') as f:
        json.dump(result, f, indent=2, sort_keys=True)
    print(json.dumps(result['summary'], indent=2), flush=True)


if __name__ == '__main__':
    main()
