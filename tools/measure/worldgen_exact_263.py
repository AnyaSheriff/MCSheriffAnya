"""Точное сравнение имён блоков и клеток биомов сохранённых миров26.3.

Состояния блоков НЕ сравниваются. Это диагностика конечного участка мира,
а не доказательство полного совпадения генератора. Только чтение worldfiles.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

import numpy as np

from genaudit_263_io import Reader, level, region_dir


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def parse_plan(plan):
    if not isinstance(plan, dict) or plan.get('version') != '26.3':
        raise ValueError('Explicit plan version26.3 required')
    seed = plan.get('seed')
    if type(seed) is not int or not -(1 << 63) <= seed < (1 << 63):
        raise ValueError('Explicit signed64-bit seed required')
    dimension = plan.get('dimension')
    if dimension != 'overworld':
        raise ValueError('Only explicit overworld dimension is supported in this first diagnostic')
    chunks = plan.get('chunks')
    if not isinstance(chunks, list) or not chunks:
        raise ValueError('Nonempty explicit chunk coordinates required')
    coords = []
    for pair in chunks:
        if not isinstance(pair, list) or len(pair) != 2 or any(type(v) is not int or not -(1 << 27) <= v < (1 << 27) for v in pair):
            raise ValueError('Invalid chunk coordinates')
        coords.append(tuple(pair))
    if len(set(coords)) != len(coords):
        raise ValueError('Duplicate requested chunk coordinates')
    return seed, dimension, set(coords)


def fingerprints(paths):
    return {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def chunk_data(reader, blocks, biomes):
    blocks = np.asarray(blocks)
    if blocks.shape != (384, 16, 16) or blocks.dtype.kind not in 'iu':
        raise ValueError('Invalid block array shape/type')
    if np.any(blocks < 0) or np.any(blocks >= len(reader.names)):
        raise ValueError('Unknown block code')
    names = np.asarray(reader.names, dtype=object)[blocks]
    bios = np.asarray(biomes, dtype=object)
    if bios.shape != (24, 64):
        raise ValueError('Incomplete biome sections')
    for values in (np.unique(names), np.unique(bios)):
        if any(not isinstance(v, str) or not v or v in ('?', 'unknown') for v in values):
            raise ValueError('Unknown block or biome name')
    bios = bios.reshape(24, 4, 4, 4)
    occupied = ~np.isin(names, ['air', 'cave_air', 'void_air'])
    # Высота именно верхнего НЕвоздушного блока: вода/листва не исключаются.
    heights = np.where(occupied.any(axis=0), 319 - np.argmax(occupied[::-1], axis=0), -65)
    return names, bios, heights


def array_hash(array, header):
    # Строки, а не paletteIDs: порядок палитры не влияет на результат.
    digest = hashlib.sha256(canonical(header) + b'\n')
    for name in array.flat:
        value = name.encode('utf-8')
        digest.update(len(value).to_bytes(4, 'big'))
        digest.update(value)
    return digest.hexdigest()


def compare_worlds(ours, vanilla, plan, example_limit=20):
    report = dict(schema=1, recording_valid=False, parity=False, props_not_compared=True,
                  full_state_parity='unknown', registry_identity_verified=False,
                  scope='Exact block NAMES and quart biome cells in requested saved chunks only; not full-state or whole-generator parity.',
                  block_y_range=[-64, 319], biome_resolution=[4, 4, 4],
                  coverage={}, chunks=[], examples={'block_names': [], 'biome_cells': [], 'heights': []}, errors=[],
                  totals={key: {'compared': 0, 'mismatches': 0} for key in ('block_names', 'biome_cells', 'top_non_air_heights')})
    report['comparison_on_union_nonair'] = {'excluded_name': 'air', 'compared': 0, 'mismatches': 0, 'diagnostic_only': True}
    report['height_definition'] = 'Top block other than air/cave_air/void_air; includes fluids and leaves; empty column=-65.'
    pairs = Counter()
    try:
        seed, dimension, wanted = parse_plan(plan)
        if type(example_limit) is not int or not 0 <= example_limit <= 1000:
            raise ValueError('Invalid example limit')
        report.update(seed=seed, dimension=dimension, plan_sha256=hashlib.sha256(canonical(plan)).hexdigest())
        reader = Reader()
        worlds, paths = {}, []
        for label, directory in [('ours', Path(ours)), ('vanilla', Path(vanilla))]:
            metadata = level(directory)
            if type(metadata.get('seed')) is not int or metadata['seed'] != seed:
                raise ValueError(f'{label}: level seed differs from explicit plan')
            paths.append(directory / 'level.dat')
            if label == 'vanilla':
                version = metadata.get('version')
                if not isinstance(version, dict) or version.get('Name') != '26.3' or metadata.get('data_version') != 5023:
                    raise ValueError('vanilla: expected pinned26.3/DataVersion5023')
            else:
                profile_path = directory / 'mcsa-world.json'
                profile = json.loads(profile_path.read_text())
                if type(profile.get('schema')) is not int or profile['schema'] != 1 or profile.get('gameplay_version') != '26.3' or not isinstance(profile.get('registry_id'), str) or not re.fullmatch(r'mcsa-catalog-v1:sha256:[0-9a-f]{64}', profile['registry_id']):
                    raise ValueError('ours: explicit saved26.3 profile required')
                report['declared_ours_registry_id'] = profile['registry_id']
                paths.append(profile_path)
            region = region_dir(directory, dimension)
            region_coords = {(x // 32, z // 32) for x, z in wanted}
            files = []
            for path in sorted(region.iterdir()):
                if path.suffix not in ('.mcs', '.mca'):
                    continue
                parts = path.stem.split('.')
                if len(parts) != 3 or parts[0] != 'r':
                    raise ValueError(f'{label}: malformed region filename')
                if (int(parts[1]), int(parts[2])) in region_coords:
                    if path.suffix != ('.mcs' if label == 'ours' else '.mca'):
                        raise ValueError(f'{label}: wrong region format')
                    files.append(path)
            before = fingerprints(files)
            records = {}
            for path in files:
                for cx, cz, blocks, biomes, _ in reader.read(path, wanted):
                    coord = (cx, cz)
                    if coord not in wanted or coord in records:
                        raise ValueError(f'{label}: unexpected/duplicate chunk {coord}')
                    records[coord] = chunk_data(reader, blocks, biomes)
            missing = wanted - records.keys()
            report['coverage'][label] = {'requested': len(wanted), 'read': len(records), 'missing': [list(c) for c in sorted(missing)]}
            if missing:
                raise ValueError(f'{label}: incomplete coverage ({len(missing)} missing chunks)')
            if before != fingerprints(files):
                raise ValueError(f'{label}: regions changed during read')
            worlds[label] = records
            paths += files
        # Содержимое входов сохраняется для независимого контроля происхождения.
        report['input_sha256'] = fingerprints(paths)
        for coord in sorted(wanted):
            a, b = worlds['ours'][coord], worlds['vanilla'][coord]
            row = {'chunk': list(coord)}
            union = (a[0] != 'air') | (b[0] != 'air')
            report['comparison_on_union_nonair']['compared'] += int(np.count_nonzero(union))
            report['comparison_on_union_nonair']['mismatches'] += int(np.count_nonzero((a[0] != b[0]) & union))
            mismatch = a[0] != b[0]
            pairs.update(zip(a[0][mismatch].tolist(), b[0][mismatch].tolist()))
            for label, arrays in [('ours', a), ('vanilla', b)]:
                header = {'version': '26.3', 'seed': seed, 'dimension': dimension, 'chunk': list(coord)}
                row[label] = {'block_names_sha256': array_hash(arrays[0], {**header, 'kind': 'block_names', 'axis': 'y,z,x'}),
                              'biomes_sha256': array_hash(arrays[1], {**header, 'kind': 'quart_biomes', 'axis': 'section,qy,qz,qx'})}
            for i, (category, field) in enumerate([('block_names', 'block_name_mismatches'), ('biome_cells', 'biome_cell_mismatches'), ('top_non_air_heights', 'height_column_mismatches')]):
                different = a[i] != b[i]
                count = int(np.count_nonzero(different))
                row[field] = count
                report['totals'][category]['compared'] += int(different.size)
                report['totals'][category]['mismatches'] += count
                example_key = 'heights' if i == 2 else category
                examples = report['examples'][example_key]
                remaining = example_limit - len(examples)
                if remaining <= 0:
                    continue
                for point in np.argwhere(different)[:remaining]:
                    index = tuple(int(v) for v in point)
                    if i == 0:
                        y, z, x = index
                        position = {'block': [coord[0] * 16 + x, y - 64, coord[1] * 16 + z]}
                    elif i == 1:
                        sy, qy, qz, qx = index
                        quart = [coord[0] * 4 + qx, sy * 4 + qy - 16, coord[1] * 4 + qz]
                        position = {'quart': quart, 'block_origin': [v * 4 for v in quart]}
                    else:
                        z, x = index
                        position = {'column': [coord[0] * 16 + x, coord[1] * 16 + z]}
                    examples.append({**position, 'ours': int(a[i][index]) if i == 2 else a[i][index],
                                     'vanilla': int(b[i][index]) if i == 2 else b[i][index]})
            report['chunks'].append(row)
        if report['input_sha256'] != fingerprints(paths):
            raise ValueError('Inputs changed during comparison')
        report['recording_valid'] = True
        report['parity'] = all(v['mismatches'] == 0 for v in report['totals'].values())
        for values in list(report['totals'].values()) + [report['comparison_on_union_nonair']]:
            values['exact_cells'] = values['compared'] - values['mismatches']
        report['block_name_pairs'] = [{'ours': a, 'vanilla': b, 'count': n} for (a, b), n in sorted(pairs.items())]
    except Exception as error:
        report['errors'].append(f'{type(error).__name__}: {error}')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('ours', 'vanilla', 'plan', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--example-limit', type=int, default=20)
    parser.add_argument('--require-match', action='store_true')
    args = parser.parse_args()
    try:
        plan = json.loads(args.plan.read_text())
    except (OSError, ValueError) as error:
        parser.error(str(error))
    result = compare_worlds(args.ours, args.vanilla, plan, args.example_limit)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + '\n')
    return 0 if result['recording_valid'] and (not args.require_match or result['parity']) else 1


if __name__ == '__main__':
    raise SystemExit(main())
