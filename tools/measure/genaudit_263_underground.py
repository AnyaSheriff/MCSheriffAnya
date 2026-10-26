"""Повтор выборок U полным генератором чанков без сетевого слушателя.

После сборки release-тестов передать исполняемый файл тестов mcsheriffanya,
указанный cargo. Миры оригинала только читаются. Каждый повтор пишет новый
каталог; сохранённые планы выбирают по 4096 чанков на каждом семени.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

REPO = Path(__file__).resolve().parents[2]
FIXTURES = REPO / 'testdata/world-profile/genaudit-26.3/batch-u'


def sulfur_counts(world, plan):
    """Полное покрытие серного биома, без примеси поверхностных биомов."""
    import numpy as np
    from genaudit_263_io import Reader, region_dir
    wanted = {(x, z) for x0, z0, x1, z1 in plan['windows']['overworld']
              for x in range(x0, x1 + 1) for z in range(z0, z1 + 1)}
    reader = Reader()
    counts = dict.fromkeys(('sulfur', 'cinnabar', 'sulfur_spike', 'potent_sulfur'), 0)
    volume = 0
    selected, present = [], set()
    for file in sorted(region_dir(world, 'overworld').glob('*.mcs')):
        for cx, cz, blocks, biomes, _ in reader.read(file, wanted):
            present.add((cx, cz))
            cells = np.asarray(biomes).reshape(96, 4, 4) == 'sulfur_caves'
            if not cells.any():
                continue
            selected.append([cx, cz, cx, cz])
            volume += int(cells.sum()) * 64
            mask = cells.repeat(4, 0).repeat(4, 1).repeat(4, 2)
            for name in counts:
                if name in reader.names:
                    counts[name] += int(((blocks == reader.names.index(name)) & mask).sum())
    if present != wanted:
        raise ValueError('Incomplete sulfur measurement coverage')
    return {'seed': plan['seed'], 'source_world': str(Path(world).resolve()),
            'blocks': counts, 'biome_volume': volume, 'windows': sorted(selected)}


def feature_counts(world, plan):
    """Связные оболочки жеод с внутренним отступом и виды спавнеров."""
    from collections import Counter
    import numpy as np
    from genaudit_263_io import Reader, region_dir
    from underground_features import MATERIALS, components, encode

    bounds = plan['windows']['overworld']
    wanted = {(x, z) for x0, z0, x1, z1 in bounds
              for x in range(x0, x1 + 1) for z in range(z0, z1 + 1)}
    shells = [{} for _ in bounds]
    entities = Counter()
    reader = Reader()
    for file in sorted(region_dir(world, 'overworld').glob('*')):
        if file.suffix not in ('.mca', '.mcs'):
            continue
        for cx, cz, blocks, _, root in reader.read(file, wanted):
            index = next(i for i, (x0, z0, x1, z1) in enumerate(bounds)
                         if x0 <= cx <= x1 and z0 <= cz <= z1)
            for material, name in enumerate(MATERIALS):
                if name not in reader.names:
                    continue
                for iy, iz, ix in zip(*np.nonzero(blocks == reader.names.index(name))):
                    shells[index][encode(cx * 16 + int(ix), int(iy) - 64, cz * 16 + int(iz))] = material
            for entity in root.get('block_entities', []):
                if entity.get('id', '').removeprefix('minecraft:') in ('mob_spawner', 'spawner'):
                    kind = entity.get('SpawnData', {}).get('entity', {}).get('id', '')
                    entities[kind.removeprefix('minecraft:')] += 1
    geodes = interior_chunks = 0
    for cells, (x0, z0, x1, z1) in zip(shells, bounds):
        interior_chunks += max(0, x1 - x0 - 1) * max(0, z1 - z0 - 1)
        for shell in components(cells):
            x, _, z = shell['center']
            if shell['total'] >= 64 and (x0 + 1) * 16 <= x < x1 * 16 and (z0 + 1) * 16 <= z < z1 * 16:
                geodes += 1
    return {'interior_shell_components': geodes, 'interior_chunks': interior_chunks,
            'shell_components_per_chunk': geodes / interior_chunks,
            'spawners_by_entity': dict(entities),
            'dungeons': sum(entities[k] for k in ('zombie', 'skeleton', 'spider'))}


def feature_cache_path(world, plan, cache):
    """Ключ включает алгоритм, реестры, план и хеши всех исходных регионов."""
    import hashlib
    import inspect
    from genaudit_263_io import manifest, registry
    algorithm = (inspect.getsource(feature_counts).encode()
                 + (REPO / 'tools/measure/genaudit_263_io.py').read_bytes()
                 + (REPO / 'tools/measure/underground_features.py').read_bytes())
    key = hashlib.sha256(json.dumps([hashlib.sha256(algorithm).hexdigest(),
                                    registry(), manifest(world), plan], sort_keys=True).encode()).hexdigest()
    return Path(cache) / f'underground-features-{key}.json'


def cached_features(world, plan, cache):
    path = feature_cache_path(world, plan, cache) if cache else None
    if path and path.exists():
        return json.loads(path.read_text())
    result = feature_counts(world, plan)
    if path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(result) + '\n')
    return result


def official_spawners(world, plan):
    """Полная перепись видов спавнеров из NBT без разворачивания блоков."""
    from collections import Counter
    from census_263 import chunks
    from genaudit_263_io import region_dir
    wanted = {(x, z) for x0, z0, x1, z1 in plan['windows']['overworld']
              for x in range(x0, x1 + 1) for z in range(z0, z1 + 1)}
    found, entities = set(), Counter()
    for file in sorted(region_dir(world, 'overworld').glob('*.mca')):
        for root in chunks(file):
            at = (root['xPos'], root['zPos'])
            if at not in wanted:
                continue
            found.add(at)
            for entity in root.get('block_entities', []):
                if entity.get('id', '').removeprefix('minecraft:') in ('mob_spawner', 'spawner'):
                    kind = entity.get('SpawnData', {}).get('entity', {}).get('id', '')
                    entities[kind.removeprefix('minecraft:')] += 1
    if found != wanted:
        raise ValueError('Incomplete official spawner census')
    return {'chunks': len(found), 'spawners_by_entity': dict(entities),
            'dungeons': sum(entities[k] for k in ('zombie', 'skeleton', 'spider'))}


def ore_host_profiles(world, plan):
    """Руды по Y и биому на остаточную породу, без подгонки координат.

    Знаменатель — камень/сланец/гранит/диорит/андезит и уже лежащая руда.
    Это наблюдаемая оценка подложки, а не восстановление состояния до декораций.
    """
    from collections import Counter, defaultdict
    import numpy as np
    from genaudit_263_io import Reader, region_dir
    wanted = {(x, z) for x0, z0, x1, z1 in plan['windows']['overworld']
              for x in range(x0, x1 + 1) for z in range(z0, z1 + 1)}
    reader = Reader()
    by_y, by_biome = defaultdict(Counter), defaultdict(Counter)
    present = set()
    for file in sorted(region_dir(world, 'overworld').glob('*')):
        if file.suffix not in ('.mca', '.mcs'):
            continue
        for x, z, blocks, biomes, _ in reader.read(file, wanted):
            present.add((x, z))
            ores = {i: n for i, n in enumerate(reader.names) if n.endswith('_ore')}
            hosts = [reader.names.index(n) for n in ('stone', 'deepslate', 'granite', 'diorite', 'andesite')]
            host = np.isin(blocks, hosts + list(ores))
            labels, indices = np.unique(np.asarray(biomes).reshape(96, 4, 4), return_inverse=True)
            bio = indices.reshape(96, 4, 4).repeat(4, 0).repeat(4, 1).repeat(4, 2)
            for y in range(-64, 112, 16):
                band = blocks[y + 64:y + 80]
                by_y[str(y)]['hosts'] += int(host[y + 64:y + 80].sum())
                for i, count in zip(*np.unique(band, return_counts=True)):
                    if i in ores:
                        by_y[str(y)][ores[i]] += int(count)
            for i, count in enumerate(np.bincount(bio[host], minlength=len(labels))):
                by_biome[str(labels[i])]['hosts'] += int(count)
            ore_mask = np.isin(blocks, list(ores))
            joint = bio[ore_mask] * len(reader.names) + blocks[ore_mask]
            for code, count in zip(*np.unique(joint, return_counts=True)):
                biome, material = divmod(int(code), len(reader.names))
                by_biome[str(labels[biome])][ores[material]] += int(count)
    if present != wanted:
        raise ValueError('Incomplete ore-host profile coverage')
    return {'chunks': len(present), 'by_y16': dict(by_y), 'by_biome': dict(by_biome)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test-executable', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--reference-root', required=True)
    parser.add_argument('--label', choices=['owner', 'second'], required=True)
    parser.add_argument('--cache', help='Reuse read-only measurement results between replays')
    parser.add_argument('--sulfur-only', action='store_true', help='Regenerate all sulfur-bearing chunks in the full census')
    parser.add_argument('--reference-report', help='Full official census JSON, required with --sulfur-only')
    parser.add_argument('--sulfur-baseline', help='Previously measured sulfur_counts JSON for the unchanged census world')
    parser.add_argument('--sulfur-selection', help='Current-climate sulfur_chunks JSON from genaudit_263_biomes.rs')
    parser.add_argument('--measure-only', action='store_true', help='Remeasure an existing complete --sulfur-only export')
    parser.add_argument('--full-census', action='store_true', help='Regenerate the entire original overworld window; reuse its validated reference statistics')
    parser.add_argument('--ore-host-profiles', action='store_true', help='Also count ores per observed host block by Y and biome')
    args = parser.parse_args()
    output = Path(args.output).resolve()
    if output.is_relative_to(REPO):
        raise ValueError('Generated worlds must be outside the repository')
    if args.sulfur_only and args.full_census:
        raise ValueError('Choose one measurement mode')
    if args.sulfur_selection and not args.sulfur_only:
        raise ValueError('--sulfur-selection requires --sulfur-only')
    if args.ore_host_profiles and (args.sulfur_only or args.full_census):
        raise ValueError('--ore-host-profiles applies to the regular replay')
    if args.measure_only and not (args.sulfur_only or args.full_census):
        raise ValueError('--measure-only requires --sulfur-only or --full-census')
    output.mkdir(parents=True, exist_ok=args.measure_only)
    plan_path = FIXTURES / f'{args.label}-plan.json'
    plan = json.loads(plan_path.read_text())
    reference = Path(args.reference_root).resolve()
    if args.full_census:
        if not args.reference_report:
            raise ValueError('--full-census needs --reference-report')
        plan = json.loads((reference / f'{args.label}-plan.json').read_text())
        tiles = [[x, z, min(x + 15, x1), min(z + 15, z1)]
                 for x0, z0, x1, z1 in plan['windows']['overworld']
                 for x in range(x0, x1 + 1, 16) for z in range(z0, z1 + 1, 16)]
        plan['windows'] = {'overworld': tiles, 'the_nether': [], 'the_end': []}
        plan_path = output / 'plan.json'
        if not args.measure_only:
            plan_path.write_text(json.dumps(plan, indent=2) + '\n')
    if args.sulfur_only:
        if not args.reference_report:
            raise ValueError('--sulfur-only needs --reference-report')
        plan = json.loads((reference / f'{args.label}-plan.json').read_text())
        before = (json.loads(Path(args.sulfur_baseline).read_text()) if args.sulfur_baseline
                  else sulfur_counts(reference / f'{args.label}-ours/world', plan))
        if before['seed'] != plan['seed'] or before['source_world'] != str(reference / f'{args.label}-ours/world'):
            raise ValueError('Sulfur baseline belongs to another world')
        plan['windows'] = {'overworld': before['windows'], 'the_nether': [], 'the_end': []}
        if args.sulfur_selection:
            selected = json.loads(Path(args.sulfur_selection).read_text())
            if selected['seed'] != plan['seed'] or not selected['sulfur_chunks']:
                raise ValueError('Current sulfur selection has another seed or is empty')
            points = [tuple(row['chunk']) for row in selected['sulfur_chunks']]
            original = json.loads((reference / f'{args.label}-plan.json').read_text())['windows']['overworld']
            if len(set(points)) != len(points) or any(not any(x0 <= x <= x1 and z0 <= z <= z1
                    for x0, z0, x1, z1 in original) for x, z in points):
                raise ValueError('Sulfur selection repeats chunks or leaves the census footprint')
            plan['windows']['overworld'] = [[x, z, x, z] for x, z in sorted(points)]
        plan_path = output / 'plan.json'
        if not args.measure_only:
            plan_path.write_text(json.dumps(plan, indent=2) + '\n')
    world = output / 'world'
    world.mkdir(exist_ok=args.measure_only)
    if not args.measure_only:
        (world / 'level.dat').write_text(f'seed={plan["seed"]}\nspawn=0 64 0\n')
    env = dict(os.environ, GENAUDIT_U_PLAN=str(plan_path),
               GENAUDIT_U_OUTPUT=str(world),
               GENAUDIT_U_SOURCE=str(reference / f'{args.label}-ours/world'))
    if not args.measure_only:
        subprocess.run([str(Path(args.test_executable).resolve()),
                        'world::underground::tests::export_genaudit_underground_sample',
                        '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                       env=env, check=True)
    if args.sulfur_only:
        after = sulfur_counts(world, plan)
        if not after['biome_volume']:
            raise ValueError('No sulfur biome blocks in the regenerated selection')
        official = json.loads(Path(args.reference_report).read_text())
        if official['vanilla_level']['seed'] != plan['seed']:
            raise ValueError('Reference seed mismatch')
        dim = official['statistics']['vanilla']['dimensions']['overworld']
        vanilla = {'biome_volume': dim['biomes_3d']['sulfur_caves'] * 64,
                   'blocks': {name: dim['features_by_biome']['sulfur_caves'].get(name, 0) for name in before['blocks']}}
        for row in (before, after):
            row['chunks'] = len(row.pop('windows'))
        result = {'seed': plan['seed'], 'selection': 'All sulfur-bearing chunks in the saved ours census; each side normalized by its own biome volume',
                  'regenerated_chunks': len(plan['windows']['overworld']),
                  'before': before, 'after': after, 'vanilla': vanilla}
        if args.sulfur_selection:
            result['selection'] = 'All sulfur-bearing chunks in the current-climate census footprint; saved baseline and fresh sample each normalized by their own biome volume'
        (output / 'conditional.json').write_text(json.dumps(result, indent=2) + '\n')
        return
    if args.full_census:
        import hashlib
        from genaudit_263 import analyze, comparison, markdown
        from genaudit_263_io import manifest
        saved = json.loads(Path(args.reference_report).read_text())
        source_hash = hashlib.sha256((REPO / 'tools/measure/genaudit_263.py').read_bytes()
                                     + (REPO / 'tools/measure/genaudit_263_io.py').read_bytes()).hexdigest()
        if saved['analyzer_source_sha256'] != source_hash or saved['vanilla_level']['seed'] != plan['seed']:
            raise ValueError('Cached census uses another analyzer or seed')
        vanilla = saved['statistics']['vanilla']
        if manifest(reference / f'{args.label}-vanilla/world') != vanilla['manifest']:
            raise ValueError('Official reference regions changed since the saved census')
        for side in saved['statistics'].values():
            for dim in ('the_nether', 'the_end'):
                side['dimensions'][dim] = {'coverage': {'chunks': 0, 'requested': 0}}
        for stage, ours in [('before', saved['statistics']['ours']),
                            ('after', analyze(world, plan, manifest(world)))]:
            report = comparison(ours, vanilla)
            report.update(analyzer_source_sha256=source_hash,
                          cached_reference_report_sha256=hashlib.sha256(Path(args.reference_report).read_bytes()).hexdigest())
            (output / f'{stage}.json').write_text(json.dumps(report, ensure_ascii=False, separators=(',', ':')) + '\n')
            (output / f'{stage}.md').write_text(markdown(report))
        (output / 'official-spawners.json').write_text(json.dumps(
            official_spawners(reference / f'{args.label}-vanilla/world', plan), indent=2) + '\n')
        return
    analyzer = REPO / 'tools/measure/genaudit_263.py'
    vanilla = reference / f'{args.label}-vanilla/world'
    for stage, ours in [('before', reference / f'{args.label}-ours/world'),
                        ('after', world)]:
        subprocess.run([sys.executable, str(analyzer), str(ours), str(vanilla),
                        '--plan', str(plan_path), '--output', str(output / stage),
                        '--cache', args.cache or str(output / 'cache')], check=True)
    features = {stage: cached_features(path, plan, args.cache) for stage, path in [
        ('before', reference / f'{args.label}-ours/world'), ('after', world), ('vanilla', vanilla)]}
    if not features['vanilla']['dungeons'] or any(not row['interior_shell_components'] for row in features.values()):
        raise ValueError('No official dungeon spawners or geode shells decoded in the sample')
    entities = json.loads((world / 'spawners.json').read_text())
    for stage, counts in zip(('before', 'after'), entities):
        features[stage]['spawners_by_entity'] = counts
        features[stage]['dungeons'] = sum(counts.get('minecraft:' + k, 0)
                                         for k in ('zombie', 'skeleton', 'spider'))
    (output / 'feature-counts.json').write_text(json.dumps(features, indent=2) + '\n')
    if args.ore_host_profiles:
        profiles = {stage: ore_host_profiles(path, plan) for stage, path in [
            ('before', reference / f'{args.label}-ours/world'), ('after', world), ('vanilla', vanilla)]}
        (output / 'ore-host-profiles.json').write_text(json.dumps(profiles, indent=2) + '\n')


if __name__ == '__main__':
    main()
