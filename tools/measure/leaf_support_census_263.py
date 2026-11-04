"""Шесть соседей по листве до любого бревна: фактическая опора против stored distance.
Только чтение region/.mcs/экспорта; один чанк каймы исключены из итогов.
"""
import argparse
import collections
import json
from pathlib import Path
from decoration_census_263 import roots

NEIGHBOURS = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))


def census(source, output, registry=None, plan=None):
    leaves, logs, chunks = {}, set(), set()
    wanted = set(map(tuple, plan['chunks'])) if plan else None
    for root in roots(Path(source), registry):
        cx, cz = root['xPos'], root['zPos']
        if (cx,cz) in chunks: continue
        chunks.add((cx, cz))
        for sec in root['sections']:
            pal = sec['palette']
            props = sec.get('properties', [{}] * len(pal))
            interest = {i for i, n in enumerate(pal) if n.endswith(('_leaves', '_log', '_wood'))}
            for k, pid in enumerate(sec['ids']):
                if pid not in interest:
                    continue
                x, y, z = k & 15, sec['Y'] * 16 + (k >> 8), (k >> 4) & 15
                at = (cx * 16 + x, y, cz * 16 + z)
                if pal[pid].endswith('_leaves'):
                    bio = sec['biomes'][(((y & 15) >> 2) * 4 + (z >> 2)) * 4 + (x >> 2)]
                    leaves[at] = (pal[pid], props[pid] or {}, bio)
                else:
                    logs.add(at)
    distance = {}
    queue = collections.deque((p, 0) for p in logs)
    while queue:
        (x, y, z), d = queue.popleft()
        if d >= 6:
            continue
        for dx, dy, dz in NEIGHBOURS:
            p = x + dx, y + dy, z + dz
            if p in leaves and p not in distance:
                distance[p] = d + 1
                queue.append((p, d + 1))
    groups = collections.defaultdict(collections.Counter)
    bad = []
    for (x, y, z), (name, props, bio) in leaves.items():
        cx, cz = x // 16, z // 16
        if wanted is not None and (cx, cz) not in wanted:
            continue
        if any((cx + dx, cz + dz) not in (wanted if wanted is not None else chunks)
               for dx in range(-1, 2) for dz in range(-1, 2)):
            continue
        stored = int(props.get('distance') or 7)
        persistent = props.get('persistent') == 'true'
        actual = distance.get((x, y, z), 7)
        for key in ('all', name, bio + '/' + name):
            group = groups[key]
            group['leaves'] += 1
            group['persistent'] += persistent
            group['stored7_nonpersistent'] += stored == 7 and not persistent
            group['unsupported_nonpersistent'] += actual == 7 and not persistent
            group['state_mismatch'] += stored != actual
            group['stored' + str(stored)] += 1
            group['actual' + str(actual)] += 1
        if actual == 7 or stored == 7:
            bad.append([x, y, z, name, bio, stored, actual, persistent])
    result = {'groups': dict(sorted(groups.items())), 'unsupported_or_stored7': bad,
              'method': 'six-connected leaf BFS, any log/wood, cap7; exclude one chunk boundary (16>6)',
              'chunks': len(chunks)}
    Path(output).write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    print(source, 'chunks', len(chunks))
    for key, group in sorted(groups.items()):
        if '/' not in key:
            print(key, dict(group))
    return result


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('input'); p.add_argument('output'); p.add_argument('--registry'); p.add_argument('--plan')
    args = p.parse_args()
    census(args.input, args.output,
           json.loads(Path(args.registry).read_text()) if args.registry else None,
           json.loads(Path(args.plan).read_text()) if args.plan else None)
