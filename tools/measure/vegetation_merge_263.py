#!/usr/bin/env python3
"""Проверка наземной регрессии после слияния U: регионы и прежние отчёты.

Никакого подбора окон по растениям: берём старое окно теста, восемь окон
из плана V2 и исходное официальное окно. Сырые метрики 3D не заменяются.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re

import numpy as np

from genaudit_263 import natural, requested
from genaudit_263_io import Reader, level, region_dir
from vegetation_263 import measure as measure_vegetation, reference_side

BIOME = 'old_growth_pine_taiga'
ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT/'testdata/world-profile/genaudit-26.3'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def block_transitions(before, after, plan):
    reader = Reader()
    wanted = requested(plan, 'overworld')
    result = Counter()
    for path in sorted(region_dir(before, 'overworld').glob('*.mcs')):
        previous = {(x, z): blocks for x, z, blocks, _, _ in reader.read(path, wanted)}
        for x, z, blocks, _, _ in reader.read(region_dir(after, 'overworld')/path.name, wanted):
            old = previous.pop((x, z))
            mask = old != blocks
            pairs, counts = np.unique(np.stack((old[mask], blocks[mask]), axis=1), axis=0, return_counts=True)
            for (a, b), count in zip(pairs, counts):
                result[f'{reader.names[a]} -> {reader.names[b]}'] += int(count)
        assert not previous
    # Азалия рисуется после покрова: удалённая трава освобождает листьям
    # воздух, а поиску опоры Part::Ground — подзол вместо непочвы.
    if any(k not in {'short_grass -> air', 'short_grass -> azalea_leaves',
                     'short_grass -> flowering_azalea_leaves', 'podzol -> rooted_dirt',
                     'coarse_dirt -> rooted_dirt'} for k in result):
        raise ValueError(f'Неожиданные изменения блоков: {result}')
    return dict(result)


def inspect(world, bounds, plants, seed, official=False, biome=BIOME):
    metadata = level(world)
    if metadata['seed'] != seed or (official and not isinstance(metadata['version'], dict)):
        raise ValueError('Неверное семя или неофициальный эталон')
    wanted = requested({'windows': {'overworld': [bounds]}}, 'overworld')
    reader = Reader()
    locations = defaultdict(list)
    soils, offsets = Counter(), Counter()
    volume = columns = 0
    seen, files = set(), []
    for path in sorted(region_dir(world, 'overworld').glob('*')):
        if path.suffix not in ('.mcs', '.mca'):
            continue
        used = False
        for cx, cz, blocks, biomes, _ in reader.read(path, wanted):
            if (cx, cz) in seen:
                raise ValueError('Повтор чанка')
            seen.add((cx, cz))
            used = True
            bio = np.asarray(biomes).reshape(96, 4, 4)
            volume += int(np.sum(bio == biome))*64
            columns += int(np.sum(bio[-1] == biome))*16
            substrate = np.isin(blocks, [i for i, n in enumerate(reader.names) if natural(n)])
            top = 383 - np.argmax(substrate[::-1], axis=0)
            for name in plants:
                for y, z, x in np.argwhere(blocks == reader.lookup[name]):
                    if bio[y//4, z//4, x//4] != biome:
                        continue
                    locations[name].append([cx*16+int(x), int(y)-64, cz*16+int(z)])
                    if name == 'brown_mushroom':
                        soils[reader.names[blocks[y-1, z, x]]] += 1
                        offsets[str(y-int(top[z, x]))] += 1
        if used:
            files.append([path.name, sha(path)])
    if seen != wanted:
        raise ValueError(f'Неполное покрытие: {len(seen)} / {len(wanted)}')
    counts = {n: len(locations[n]) for n in plants}
    return {'chunks': len(seen), 'level': metadata, 'files_sha256': files,
            'biome_blocks': volume, 'surface_columns': columns, 'counts': counts,
            'locations_sha256': {n: hashlib.sha256(json.dumps(sorted(locations[n])).encode()).hexdigest()
                                 for n in plants},
            'brown_mushroom_support': dict(soils), 'brown_mushroom_above_surface': dict(offsets),
            'per_million_3d': {n: c*1e6/volume for n, c in counts.items()},
            'per_thousand_surface_columns': {n: c*1000/columns for n, c in counts.items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--audit-root', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root, artifacts = args.audit_root, args.artifacts
    fixture = json.loads((FIXTURES/'vegetation-regression.json').read_text())
    sample = next(s for s in fixture['samples'] if s['biome'] == BIOME)
    plants = sample['bounds_per_million']
    reference_file = root/'reports/targets/biome-old_growth_pine_taiga.json'
    reference = json.loads(reference_file.read_text())
    reference_side(reference, fixture['seed'])
    official_bounds = reference['coverage'][0]['vanilla']['bounds_chunks']
    single = {}
    for name, world, bounds, official in [
        ('pre_merge', root/'genV/v2/verified/world', sample['v2_single_window'], False),
        ('merged_before', artifacts/'pine-before/world', sample['v2_single_window'], False),
        ('merged_after', artifacts/'pine-final/world', sample['v2_single_window'], False),
        ('official', root/'targets/owner-vanilla/world', official_bounds, True),
    ]:
        single[name] = inspect(world, bounds, plants, fixture['seed'], official)
    for side in ('merged_before', 'merged_after'):
        if single[side]['locations_sha256']['brown_mushroom'] != single['pre_merge']['locations_sha256']['brown_mushroom']:
            raise ValueError('Изменились места грибов: причина уже не только в знаменателе')
    ref = single['official']
    expected = sample['surface_reference']
    assert (ref['surface_columns'], ref['biome_blocks'], ref['counts']['brown_mushroom']) == (
        expected['columns'], expected['biome_blocks'], expected['brown_mushrooms'])
    matrices = {}
    for label, old_file in [('main', 'after.json'), ('population', 'population-after.json')]:
        old_path = root/'genV/v2'/old_file
        before_path = artifacts/f'{label}-merged.json'
        after_path = artifacts/f'{label}-after.json'
        old, before, after = [json.loads(p.read_text()) for p in (old_path, before_path, after_path)]
        assert old['plan'] == before['plan'] == after['plan']
        feature_changes = {}
        for biome in before['ours']['features_by_biome'].keys() | after['ours']['features_by_biome'].keys():
            bc = before['ours']['features_by_biome'].get(biome, {})
            ac = after['ours']['features_by_biome'].get(biome, {})
            changes = {n: [bc.get(n, 0), ac.get(n, 0)] for n in bc.keys() | ac.keys()
                       if bc.get(n, 0) != ac.get(n, 0)}
            if changes:
                feature_changes[biome] = changes
        historic = {r['metric']: r for r in old['rows']}
        previous = {r['metric']: r for r in before['rows']}
        rows = [dict(metric=r['metric'], pre_merge=historic.get(r['metric'], {}).get('ours'),
                     merged_before=previous.get(r['metric'], {}).get('ours'),
                     after=r['ours'], vanilla=r['vanilla'], tolerance=r['tolerance'],
                     outside=r['outside']) for r in after['rows']]
        matrices[label] = {'chunks': after['ours']['chunks'],
                           'feature_count_changes': feature_changes,
                           'block_transitions': block_transitions(artifacts/f'{label}-merged/world',
                                                                  artifacts/f'{label}-after/world', after['plan']),
                           'source_sha256': [sha(p) for p in (old_path, before_path, after_path)],
                           'outside_pre_merge': sum(r['outside'] for r in old['rows']),
                           'outside_merged_before': sum(r['outside'] for r in before['rows']),
                           'outside_after': sum(r['outside'] for r in after['rows']), 'rows': rows}
    surface = {}
    for sample in fixture['samples']:
        if 'surface_reference' not in sample:
            continue
        biome = sample['biome']
        plants = sample['bounds_per_million']
        path = root/f'reports/targets/biome-{biome}.json'
        reference = json.loads(path.read_text())
        reference_side(reference, fixture['seed'])
        official_bounds = reference['coverage'][0]['vanilla']['bounds_chunks']
        ref = inspect(root/'targets/owner-vanilla/world', official_bounds, plants,
                      fixture['seed'], True, biome)
        expected = sample['surface_reference']
        assert (ref['surface_columns'], ref['biome_blocks']) == (expected['columns'], expected['biome_blocks'])
        if 'brown_mushrooms' in expected:
            assert ref['counts']['brown_mushroom'] == expected['brown_mushrooms']
        cases = {'official': ref}
        # Только восемь окон каждого биома: случайные вкрапления тайги в
        # остальных окнах общей переписи сюда не подмешиваются.
        plan = {'seed': fixture['seed'], 'windows': {'overworld': sample.get('windows', [sample.get('bounds')])}}
        source = sample.get('source_sample', 'population')
        for side, world in [('before', f'{source}-merged/world'), ('after', f'{source}-after/world')]:
            ours = measure_vegetation(artifacts/world, plan)
            columns = ours['projected_surface_columns_by_biome'][biome]
            counts = ours['features_by_biome'][biome]
            standardized = {n: counts.get(n, 0)/columns*ref['surface_columns']/ref['biome_blocks']*1e6
                            for n in plants}
            if side == 'after':
                assert all(lo <= standardized[n] <= hi for n, (lo, hi) in plants.items())
            cases[side] = {'chunks': ours['chunks'], 'surface_columns': columns,
                          'counts': {n: counts.get(n, 0) for n in plants},
                          'per_thousand_surface_columns': {n: counts.get(n, 0)*1000/columns for n in plants},
                          'per_million_at_reference_depth': standardized}
        cases['unchanged_bounds'] = plants
        cases['reference_sha256'] = sha(path)
        surface[biome] = cases
    test_log = artifacts/'workspace-verified.log'
    test_text = test_log.read_text()
    results = re.findall(r'^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; 0 filtered out;',
                         test_text, re.M)
    clippy_log = artifacts/'clippy-verified.log'
    if not results or 'test result: FAILED' in test_text or 'Finished `release`' not in clippy_log.read_text():
        raise ValueError('Нужны завершённые успешные проверки workspace и Clippy')
    vegetation_log = artifacts/'vegetation-verified.log'
    vegetation_result = re.search(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;', vegetation_log.read_text())
    python_log = artifacts/'python-verified.log'
    python_result = re.search(r'Ran (\d+) tests', python_log.read_text())
    if not vegetation_result or not python_result or '\nOK\n' not in python_log.read_text():
        raise ValueError('Нужны успешные проверки растительности и измерителей')
    report = {'schema': 1, 'base_commit': '1a0505a', 'cause_commit': '2fec4d7', 'seed': fixture['seed'],
              'normalization': 'Raw census remains per million 3D biome blocks. Old-growth surface regressions use all eight preselected V2 population windows each, surface columns and the official reference biome depth; original bounds unchanged.',
              'official_reference_sha256': sha(reference_file), 'single_window': single, 'matrices': matrices, 'surface_regressions': surface,
              'verification': {'workspace_passed': sum(int(p) for p, _ in results),
                               'workspace_ignored': sum(int(i) for _, i in results),
                               'vegetation_passed': int(vegetation_result[1]),
                               'vegetation_ignored': int(vegetation_result[2]),
                               'python_passed': int(python_result[1]),
                               'workspace_log_sha256': sha(test_log), 'clippy_log_sha256': sha(clippy_log),
                               'vegetation_log_sha256': sha(vegetation_log), 'python_log_sha256': sha(python_log)},
              'source_sha256': {str(p.relative_to(ROOT)): sha(p) for p in [
                  ROOT/'src/world/vegetation.rs', ROOT/'tools/measure/vegetation_263.py',
                  Path(__file__).resolve(), FIXTURES/'vegetation-regression.json']}}
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
    print('Pine mushroom locations unchanged; all sixteen surface bounds pass')


if __name__ == '__main__':
    main()
