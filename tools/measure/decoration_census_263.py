"""Перепись опоры и расстояний: только чтение region или экспортированных чанков.

python3 decoration_census_263.py INPUT OUTPUT_PREFIX [--registry names.json]
Для .mcs нужен справочник, записанный export_decoration_chunks. Нужны numpy/scipy.
Расстояния евклидовы, в 3D, отдельно до брёвен и листвы в загруженной выборке.
Подзол/грубая земля считаются целиком: происхождение блока из region не доказать.
"""
import argparse
import collections
import ctypes
import ctypes.util
import gzip
import json
from pathlib import Path
import struct

import numpy as np
from scipy.spatial import cKDTree
import census_263 as c

WATCH = {'leaf_litter', 'pink_petals', 'wildflowers', 'moss_carpet',
         'pale_moss_carpet', 'podzol', 'coarse_dirt', 'rooted_dirt', 'pale_moss_block'}


def pname(p):
    if isinstance(p, str):
        return p.removeprefix('minecraft:')
    return (p.get('id') or p.get('Name') or p.get('name') or p['']).removeprefix('minecraft:')


def container(raw, pos, count):
    kind = raw[pos]
    pos += 1
    if kind == 0:
        return [0] * count, pos
    if kind == 1:
        value = int.from_bytes(raw[pos:pos + 2], 'big')
        return [value] * count, pos + 2
    n = count if kind == 3 else int.from_bytes(raw[pos:pos + 2], 'big')
    if kind == 2:
        pos += 2
    palette = list(struct.unpack_from('>' + str(n) + 'H', raw, pos))
    pos += n * 2
    if kind == 3:
        return palette, pos
    assert kind == 2
    bits = (n - 1).bit_length()
    bits = 4 if bits == 3 else 8 if 5 <= bits <= 7 else bits
    size = (count * bits + 7) // 8
    packed = int.from_bytes(raw[pos:pos + size], 'little')
    mask = (1 << bits) - 1
    return [palette[(packed >> (i * bits)) & mask] for i in range(count)], pos + size


def mcs_chunks(path, registry):
    lib = ctypes.CDLL(ctypes.util.find_library('zstd'))
    lib.ZSTD_decompress.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
    lib.ZSTD_decompress.restype = ctypes.c_size_t
    for file in sorted(path.glob('*.mcs')):
        _, rx, rz, _ = file.name.split('.')
        data = file.read_bytes()
        assert data[:5] == b'MCS\x01\x01'
        for slot in range(1024):
            off, size = struct.unpack_from('>II', data, 8 + slot * 8)
            if not size:
                continue
            record = data[off * 256:off * 256 + size]
            crc, n = struct.unpack_from('>II', record)
            import zlib
            assert n == len(record) - 8 and zlib.crc32(record[8:]) == crc
            dest = ctypes.create_string_buffer(64 << 20)
            length = lib.ZSTD_decompress(dest, len(dest), record[8:], n)
            assert length <= len(dest)
            raw = dest.raw[:length]
            assert raw[0] in (4, 5), raw[0]
            pos = 1
            blocks, biomes = [], []
            for _ in range(24):
                ids, pos = container(raw, pos, 4096)
                blocks.append(ids)
            for _ in range(24):
                ids, pos = container(raw, pos, 64)
                biomes.append([registry['biomes'][i] for i in ids])
            sections = []
            for sy in range(1, 20):
                ids = blocks[sy + 4]
                pal = sorted(set(ids))
                index = {v: i for i, v in enumerate(pal)}
                sections.append({'Y': sy, 'palette': [registry['blocks'][i] for i in pal],
                                 'ids': [index[i] for i in ids], 'biomes': biomes[sy + 4],
                                 'properties': [registry['properties'][i] for i in pal]})
            yield {'xPos': int(rx) * 32 + slot % 32, 'zPos': int(rz) * 32 + slot // 32, 'sections': sections}


def roots(path, registry):
    if path.is_file():
        with gzip.open(path, 'rt') as stream:
            yield from map(json.loads, stream)
    elif list(path.glob('*.mcs')):
        yield from mcs_chunks(path, registry)
    else:
        for file in sorted(path.glob('*.mca')):
            for root in c.chunks(file):
                sections = []
                for sec in root.get('sections', []):
                    bs = sec.get('block_states')
                    if not bs or not 1 <= sec['Y'] < 20:
                        continue
                    pal = [pname(p) for p in bs['palette']]
                    ids = [0] * 4096 if len(pal) == 1 else c.unpack(bs['data'], max(4, (len(pal) - 1).bit_length()), 4096)
                    bio = c.biome_cells(sec)
                    sections.append({'Y': sec['Y'], 'palette': pal, 'ids': ids,
                                     'biomes': [b.removeprefix('minecraft:') for b in bio],
                                     'properties': [(p.get('properties') or p.get('Properties') or p) if isinstance(p, dict) else {} for p in bs['palette']]})
                yield {'xPos': root['xPos'], 'zPos': root['zPos'], 'sections': sections}


def census(source, output, registry=None, plan=None):
    logs, leaves, decorations = [], [], []
    chunks = {}
    wanted = set(map(tuple, plan["chunks"])) if plan else None
    chunk_biomes = collections.Counter()
    tree_counts = collections.defaultdict(collections.Counter)
    surfaces = collections.defaultdict(collections.Counter)
    for root in roots(Path(source), registry):
        cx, cz = root['xPos'], root['zPos']
        if (cx,cz) in chunks: continue
        chunks[cx, cz] = True
        secs = {s['Y']: s for s in root['sections']}
        if wanted is None or (cx, cz) in wanted:
            top = next(((sy, sec, y) for sy, sec in sorted(secs.items(), reverse=True)
                        for y in range(15, -1, -1)
                        if sec['palette'][sec['ids'][(y << 8) | (8 << 4) | 8]] not in ('air', 'cave_air', 'void_air')), None)
            if top:
                sy, sec, y = top
                chunk_biomes[sec['biomes'][((y >> 2) * 4 + 2) * 4 + 2]] += 1
        if wanted is None or (cx,cz) in wanted:
            remaining = set(range(256))
            ground_names = {'grass_block','dirt','coarse_dirt','podzol','rooted_dirt','moss_block','mud','mycelium',
                            'pale_moss_block','snow_block','sand','red_sand','gravel','clay','stone','granite',
                            'diorite','andesite','calcite','tuff','mossy_cobblestone','terracotta'}
            soil_names = {'grass_block','dirt','coarse_dirt','podzol','rooted_dirt','moss_block','mud','mycelium','pale_moss_block'}
            for sy,sec in sorted(secs.items(),reverse=True):
                ground_ids = {i for i,n in enumerate(sec['palette']) if n in ground_names or n.endswith(('_ore','_terracotta'))}
                if not ground_ids: continue
                for column in list(remaining):
                    for ly in range(15,-1,-1):
                        pid = sec['ids'][(ly<<8)|column]
                        if pid not in ground_ids: continue
                        bio = sec['biomes'][((ly>>2)*4+((column>>4)>>2))*4+((column&15)>>2)]
                        name = sec['palette'][pid]
                        surfaces[bio][name] += 1
                        if sy*16+ly>=63 and name in soil_names: surfaces[bio]['_land_soil_columns'] += 1
                        remaining.remove(column)
                        break
        for sy, sec in secs.items():
            pal, ids = sec['palette'], sec['ids']
            interesting = {i for i, name in enumerate(pal)
                           if name in WATCH or name.endswith(('_log', '_leaves', '_wood'))}
            if not interesting:
                continue
            for i, pid in enumerate(ids):
                if pid not in interesting:
                    continue
                name = pal[pid]
                x, y, z = i & 15, sy * 16 + (i >> 8), (i >> 4) & 15
                at = (cx * 16 + x, y, cz * 16 + z)
                if wanted is None or (cx,cz) in wanted:
                    bio = sec['biomes'][(((y & 15) >> 2) * 4 + (z >> 2)) * 4 + (x >> 2)]
                    if name.endswith(('_log','_wood')): tree_counts[bio]['logs'] += 1
                    elif name.endswith('_leaves'): tree_counts[bio]['leaves'] += 1
                if name.endswith(('_log', '_wood')):
                    logs.append(at)
                elif name.endswith('_leaves'):
                    leaves.append(at)
                else:
                    if y < 32: continue
                    below = secs.get((y - 1) >> 4)
                    under = below['palette'][below['ids'][(((y - 1) & 15) << 8) | (z << 4) | x]] if below else 'unknown'
                    biome = sec['biomes'][(((y & 15) >> 2) * 4 + (z >> 2)) * 4 + (x >> 2)]
                    if wanted is None or (cx, cz) in wanted:
                        decorations.append((*at, name, under, biome))
    points = np.array([r[:3] for r in decorations], dtype=float).reshape((-1, 3))
    distances = []
    for trees in (logs, leaves):
        distances.append(cKDTree(trees).query(points, )[0] if trees else np.full(len(points), np.inf))
    groups = collections.defaultdict(lambda: {'count': 0, 'under': collections.Counter(), 'logs': collections.Counter(), 'leaves': collections.Counter()})
    chunk_counts = collections.defaultdict(collections.Counter)
    def bucket(d):
        return next((str(n) for n in (2, 4, 8, 16, 32) if d <= n), '>32')
    rows_path = str(output) + '.rows.tsv.gz'
    with gzip.open(rows_path, 'wt') as stream:
        stream.write('x\ty\tz\tdecoration\tunder\tbiome\tlog_distance\tleaf_distance\tedge32\n')
        for row, dl, df in zip(decorations, *distances):
            x, y, z, name, under, biome = row
            cx, cz = x // 16, z // 16
            edge = any((cx + dx, cz + dz) not in (wanted if wanted is not None else chunks) for dx in range(-2, 3) for dz in range(-2, 3))
            stream.write('\t'.join(map(str, (*row, round(float(dl), 4), round(float(df), 4), int(edge)))) + '\n')
            for key in ('all/' + name, biome + '/' + name):
                group = groups[key]
                group['count'] += 1
                group['under'][under] += 1
                if not edge:
                    group['logs'][bucket(dl)] += 1
                    group['leaves'][bucket(df)] += 1
            chunk_counts[f'{cx},{cz}'][biome + '/' + name] += 1
    for cx,cz in chunks:
        if wanted is None or (cx,cz) in wanted: chunk_counts[f'{cx},{cz}']
    result = {'chunks': len(set(chunks).intersection(wanted)) if wanted is not None else len(chunks), 'loaded_logs': len(logs), 'loaded_leaves': len(leaves), 'tree_blocks_by_biome': dict(tree_counts),
              'groups': dict(sorted(groups.items())), 'per_chunk': dict(sorted(chunk_counts.items())), 'chunks_by_surface_biome': dict(chunk_biomes), 'surface_columns_by_biome': dict(surfaces),
              'distance': '3D Euclidean nearest in loaded sample; bins upper bounds; exclude two-chunk boundary for histogram',
              'surface_band': '32 <= y < 320; section y=16..31 loaded for support only',
              'ground_origin': 'podzol/coarse_dirt include natural surface and decorators; attribution unavailable'}
    Path(str(output) + '.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    print(source, 'chunks', len(chunks), 'decorations', len(decorations), 'logs', len(logs), 'leaves', len(leaves), flush=True)
    for key, group in sorted(groups.items()):
        if key.startswith('all/'):
            print(key, group, flush=True)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input')
    parser.add_argument('output')
    parser.add_argument('--registry')
    parser.add_argument('--plan')
    args = parser.parse_args()
    census(args.input, args.output, json.loads(Path(args.registry).read_text()) if args.registry else None,
           json.loads(Path(args.plan).read_text()) if args.plan else None)
