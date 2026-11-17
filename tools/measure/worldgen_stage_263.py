"""Read-only полные столбцы и собственные стадии; не исправление генерации."""
import gzip
import hashlib
import json
import struct

import numpy as np

AIR = frozenset(('air', 'cave_air', 'void_air'))
# Копия неизменного natural() из genaudit_263.py, без изменения frozen analyzer.
GROUND = frozenset(('grass_block', 'dirt', 'coarse_dirt', 'rooted_dirt', 'podzol', 'mycelium',
                   'mud', 'clay', 'sand', 'red_sand', 'sandstone', 'red_sandstone', 'gravel',
                   'stone', 'granite', 'diorite', 'andesite', 'deepslate', 'tuff', 'calcite',
                   'terracotta', 'snow_block', 'powder_snow', 'ice', 'packed_ice', 'blue_ice',
                   'moss_block', 'pale_moss_block', 'netherrack', 'soul_sand', 'soul_soil',
                   'basalt', 'blackstone', 'end_stone', 'bedrock', 'obsidian', 'sulfur', 'cinnabar'))
STAGES = ('base_underground', 'dungeons', 'mineshaft', 'stronghold', 'ruined_portal',
          'shipwreck', 'ocean_ruin', 'trail_ruins', 'trees', 'vegetation', 'structure_terrain',
          'abandoned_camp', 'village', 'mansion', 'temples', 'pillager_outpost', 'ocean',
          'monument', 'cave_biomes', 'ancient_city', 'trial_chambers', 'final')
SEEDS = (1792932773345722119, 100554032945340, 42, -20261116)
TRACE_CHUNKS = ((512, 512), (512, 513))


def natural(name):
    return name in GROUND or name.endswith(('_ore', '_terracotta')) or name.startswith('raw_')


def top(values, predicate):
    return next((y - 64 for y in range(383, -1, -1) if predicate(values[y])), -65)


def rle_names(values):
    result = []
    start = -64
    previous = str(values[0])
    for i in range(1, 384):
        name = str(values[i])
        if name != previous:
            result.append([start, i - 65, previous])
            start, previous = i - 64, name
    result.append([start, 319, previous])
    return result


def column_record(names):
    height = top(names, lambda n: n not in AIR)
    return {'top_non_air': height, 'top_name': str(names[height + 64]) if height >= -64 else None,
            'top_water': top(names, lambda n: n == 'water'),
            'top_substrate': top(names, natural), 'profile_y_inclusive_name': rle_names(names)}


def height_argmax_diagnostics(ours, vanilla):
    """ALL columns, fixed lexicographic (worldX,worldZ) tie; no example truncation."""
    if not ours or ours.keys() != vanilla.keys():
        raise ValueError('complete paired coverage required')
    maximum = -1
    ties = []
    compared = 0
    error_sum = 0
    for (cx, cz), a in sorted(ours.items()):
        b = vanilla[cx, cz]
        for x in range(16):
            for z in range(16):
                error = abs(int(a[2][z, x]) - int(b[2][z, x]))
                compared += 1
                error_sum += error
                if error > maximum:
                    maximum, ties = error, []
                if error == maximum:
                    ties.append((cx * 16 + x, cz * 16 + z))
    selected = min(ties)
    neighbors = []
    for x in range(selected[0] - 1, selected[0] + 2):
        for z in range(selected[1] - 1, selected[1] + 2):
            key = x // 16, z // 16
            entry = {'column': [x, z], 'in_plan': key in ours}
            if key in ours:
                for label, records in [('ours', ours), ('vanilla', vanilla)]:
                    entry[label] = column_record(records[key][0][:, z % 16, x % 16])
            neighbors.append(entry)
    return {'diagnostic_only': True, 'height_definition': 'Top non air/cave_air/void_air including fluids/plants/leaves; empty=-65.',
            'substrate_definition': {'ground': sorted(GROUND), 'also': ['*_ore', '*_terracotta', 'raw_*'],
                                     'range': [-64, 319], 'empty': -65,
                                     'warning': 'Material whitelist, includes structure materials; not pure terrain or official density.'},
            'compared_columns': compared, 'abs_error_sum': error_sum, 'abs_error_max': maximum,
            'tie_count': len(ties), 'tie_rule': 'minimum lexicographic (worldX,worldZ), over ALL planned columns',
            'selected_column': list(selected), 'neighbors': neighbors}


def decode_stage(stage):
    states = np.empty((384, 16, 16), dtype=np.int32)
    seen = set()
    names = stage['state_names']
    for col in stage['columns']:
        x, z = col['local']
        if type(x) is not int or type(z) is not int or not 0 <= x < 16 or not 0 <= z < 16 or (x, z) in seen:
            raise ValueError('invalid/duplicate local column')
        seen.add((x, z))
        cursor = -64
        for start, end, state in col['runs']:
            if any(type(v) is not int for v in (start, end, state)) or start != cursor or not start <= end <= 319 or not 0 <= state < 65536:
                raise ValueError('invalid/noncontiguous full profile')
            name = names.get(str(state))
            if not isinstance(name, str) or not name or name == '?':
                raise ValueError('unknown state name')
            states[start + 64:end + 65, z, x] = state
            cursor = end + 1
        if cursor != 320:
            raise ValueError('truncated profile')
    if len(seen) != 256:
        raise ValueError('missing full columns')
    block_hash = hashlib.sha256(b'mcsa-stage-blockstates-v1:y,z,x:i32be\n' + states.astype('>i4').tobytes()).hexdigest()
    ids, biomes = stage['biome_ids'], stage['biomes']
    if len(ids) != 1536 or len(biomes) != 1536 or any(type(v) is not int or not 0 <= v < 65536 for v in ids):
        raise ValueError('invalid biome coverage')
    id_names = {}
    for number, name in zip(ids, biomes):
        if not isinstance(name, str) or not name or name == '?' or id_names.setdefault(number, name) != name:
            raise ValueError('invalid biome identity')
    biome_hash = hashlib.sha256(b'mcsa-stage-biomes-v1:qy,qz,qx:u16be\n' + struct.pack('>1536H', *ids)).hexdigest()
    if block_hash != stage['blockstates_sha256'] or biome_hash != stage['biome_ids_sha256']:
        raise ValueError('native hash differs from full exported data')
    flat_names = np.asarray([names[str(v)] for v in states.flat], dtype=object).reshape(states.shape)
    return flat_names, np.asarray(biomes, dtype=object).reshape(24, 4, 4, 4)


def read_traces(path):
    records = {}
    with gzip.open(path, 'rt') as stream:
        for line in stream:
            record = json.loads(line)
            key = record['seed'], tuple(record['chunk'])
            if (record.get('schema') != 1 or record.get('version') != '26.3' or key in records
                    or key[0] not in SEEDS or key[1] not in TRACE_CHUNKS or record.get('density_router') is not False
                    or record.get('holdout') != (key[0] in SEEDS[2:])):
                raise ValueError('unexpected trace identity')
            if [s['stage'] for s in record['stages']] != list(STAGES):
                raise ValueError('missing/reordered stage')
            for stage in record['stages']:
                decode_stage(stage)
            identity = record['identity']
            final = record['stages'][-1]
            if (identity.get('equal_blockstates') is not True or identity.get('equal_biomes') is not True
                    or identity.get('compared_blockstates') != 98304 or identity.get('compared_biomes') != 1536
                    or identity.get('ordinary_blockstates_sha256') != final['blockstates_sha256']
                    or identity.get('ordinary_biome_ids_sha256') != final['biome_ids_sha256']):
                raise ValueError('ordinary/observed identity gate failed')
            records[key] = record
    if set(records) != {(seed, chunk) for seed in SEEDS for chunk in TRACE_CHUNKS}:
        raise ValueError('missing trace chunks/seeds')
    return records


def link_trace(record, saved, selected_columns):
    """Full final chunk names/biomes vs ordinary server save; stage tops only our data."""
    final = decode_stage(record['stages'][-1])
    if not np.array_equal(final[0], saved[0]) or not np.array_equal(final[1], saved[1]):
        raise ValueError('traced final differs from ordinary saved chunk')
    cx, cz = record['chunk']
    result = {'chunk': [cx, cz], 'final_saved_names_equal': True, 'final_saved_biomes_equal': True, 'columns': []}
    diagnostics = {tuple(c['column']): c for c in record['diagnostic_columns']}
    for x, z in selected_columns:
        if (x // 16, z // 16) == (cx, cz):
            result['columns'].append({'column': [x, z], 'own_diagnostics': diagnostics[x, z], 'own_stages': []})
    for stage in record['stages']:
        arrays = decode_stage(stage)
        for row in result['columns']:
            x, z = row['column']
            row['own_stages'].append({'stage': stage['stage'], **column_record(arrays[0][:, z % 16, x % 16])})
    return result
