#!/usr/bin/env python3
"""Повторная биомная перепись T/F/O из сохранённых регионов, без кода игры.

Места биомов различаются: сравниваются независимые условные плотности,
а не совпадение координат. Знаменатель и допуск — как у genaudit_263.
Пример: vegetation_263.py WORLD --plan vegetation-plan.json
  --reference reports/owner.json --output result.json
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path

import numpy as np

from genaudit_263 import feature, requested, natural
from genaudit_263_io import Reader, level, region_dir

PLANTS = set('bamboo bamboo_sapling open_eyeblossom closed_eyeblossom short_grass tall_grass fern large_fern leaf_litter wildflowers pink_petals dandelion poppy cornflower azure_bluet oxeye_daisy allium lily_of_the_valley blue_orchid red_tulip orange_tulip white_tulip pink_tulip sunflower lilac rose_bush peony dead_bush bush firefly_bush red_shrub shelf_mushroom brown_mushroom red_mushroom sugar_cane pumpkin melon cactus cactus_flower sweet_berry_bush lily_pad short_dry_grass tall_dry_grass kelp kelp_plant seagrass tall_seagrass sea_pickle mangrove_roots muddy_mangrove_roots vine bee_nest moss_carpet'.split())


def owned(name):
    return (name in PLANTS or name in {'mushroom_stem', 'brown_mushroom_block', 'red_mushroom_block', 'cocoa', 'pale_hanging_moss', 'pale_moss_carpet'}
            or name.endswith(('_log', '_wood', '_leaves', '_coral', '_coral_block', '_coral_fan', '_coral_wall_fan')))


def reference_side(report, seed):
    metadata = report['vanilla_level']
    if not isinstance(metadata['version'],dict):
        raise ValueError('Для эталона нужен официальный мир, а не MCSA')
    if metadata['seed'] != seed:
        raise ValueError('Семя официальной выборки отличается от плана')
    return report['statistics']['vanilla']['dimensions']['overworld']


def measure(world, plan):
    if level(world)['seed'] != plan['seed']:
        raise ValueError('Семя мира отличается от плана')
    reader = Reader()
    wanted = requested(plan, 'overworld')
    seen = set()
    volume = Counter()
    counts = defaultdict(Counter)
    kelp = Counter()
    heights = defaultdict(Counter)
    surfaces = defaultdict(Counter)
    files = []
    folder = region_dir(world, 'overworld')
    for path in sorted([*folder.glob('*.mcs'), *folder.glob('*.mca')]):
        files.append([path.name, hashlib.sha256(path.read_bytes()).hexdigest()])
        for x, z, blocks, biomes, _ in reader.read(path, wanted):
            if (x, z) in seen:
                raise ValueError('Повтор чанка')
            seen.add((x, z))
            bio = np.asarray(biomes).reshape(96, 4, 4)
            keys, ns = np.unique(bio, return_counts=True)
            volume.update({str(k): int(n) for k, n in zip(keys, ns)})
            # Та же естественная поверхность, что у полной переписи.
            # Разделяем недостаток подходящей почвы и шанс растения: камень
            # высокогорного окна нельзя исправлять густотой травы.
            substrate = np.isin(blocks,[i for i,n in enumerate(reader.names) if natural(n)])
            top = 383 - np.argmax(substrate[::-1],axis=0)
            top = np.where(substrate.any(axis=0),top,0)
            zz,xx = np.indices((16,16))
            surface_bio = bio[top//4,zz//4,xx//4]
            surface_blocks = blocks[top,zz,xx]
            for biome in np.unique(surface_bio):
                mask = surface_bio == biome
                ys,ns = np.unique(top[mask]-64,return_counts=True)
                heights[str(biome)].update({str(int(y)):int(n) for y,n in zip(ys,ns)})
                codes,ns = np.unique(surface_blocks[mask],return_counts=True)
                surfaces[str(biome)].update({reader.names[int(c)]:int(n) for c,n in zip(codes,ns)})
            for si in range(24):
                section = blocks[si*16:si*16+16]
                b3 = bio[si*4:si*4+4].repeat(4, 0).repeat(4, 1).repeat(4, 2)
                for code in np.unique(section):
                    name = reader.names[code]
                    if not feature(name):
                        continue
                    bs, ns = np.unique(b3[section == code], return_counts=True)
                    for b, n in zip(bs, ns):
                        counts[str(b)][name] += int(n)
            stem = reader.lookup['kelp_plant']
            top = reader.lookup['kelp']
            for bz, bx in np.argwhere((blocks == top).any(axis=0)):
                for y in np.flatnonzero(blocks[:, bz, bx] == top):
                    length = 1
                    while y-length >= 0 and blocks[y-length, bz, bx] == stem:
                        length += 1
                    kelp[length] += 1
    if seen != wanted:
        raise ValueError(f'Неполное покрытие: {len(seen)} / {len(wanted)}')
    return {'chunks': len(seen), 'chunk_sha256': hashlib.sha256(
                json.dumps(sorted(seen)).encode()).hexdigest(), 'files': files,
            'biomes_3d': dict(volume), 'features_by_biome': dict(counts),
            'kelp_height_histogram': dict(sorted(kelp.items())),
            'height_by_biome':dict(heights), 'surface_blocks_by_biome':dict(surfaces)}


def compare(ours, vanilla):
    rows = []
    for biome in sorted(ours['biomes_3d'].keys() & vanilla['biomes_3d'].keys()):
        a, b = ours['biomes_3d'][biome]*64, vanilla['biomes_3d'][biome]*64
        # Очень малые вкрапления у границы не оценивают густоту целого биома.
        if min(a, b) < 256*384*4:
            continue
        # Полное отсутствие растений — тоже результат, а не повод убрать
        # биом из сравнения. Объём достаточен независимо от числа находок.
        ac, bc = ours['features_by_biome'].get(biome, {}), vanilla['features_by_biome'].get(biome, {})
        for name in sorted(ac.keys() | bc.keys()):
            if not owned(name):
                continue
            av, bv = ac.get(name, 0)*1e6/a, bc.get(name, 0)*1e6/b
            tolerance = max(5, bv*.2)
            rows.append({'metric': f'feature_biome/{biome}/{name}',
                         'ours': round(av, 6), 'vanilla': round(bv, 6),
                         'tolerance': tolerance, 'outside': abs(av-bv) > tolerance,
                         'biome_blocks': [a, b]})
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world', type=Path, nargs='?')
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--reference', type=Path, nargs='+')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--prepare-export', type=Path)
    parser.add_argument('--recompare', type=Path, help='Повторить сравнение сохранённых сырых счётчиков без перечитывания мира')
    args = parser.parse_args()
    plan = json.loads(args.plan.read_text())
    if args.prepare_export:
        root = args.prepare_export.resolve()
        if root.is_relative_to(Path(__file__).resolve().parents[2]) or (root/'world').exists():
            raise ValueError('Нужен свежий каталог мира вне репозитория')
        root.mkdir(parents=True, exist_ok=True)
        (root/'export-plan.json').write_text(json.dumps({'samples': [{
            'seed': plan['seed'], 'output': str(root/'world'),
            'chunks': sorted(requested(plan, 'overworld'))}]}, indent=2)+'\n')
        return
    if (args.world is None and args.recompare is None) or args.reference is None or args.output is None:
        parser.error('Нужны world, --reference и --output либо --prepare-export')
    if args.world is not None and args.recompare is not None:
        parser.error('Выберите world или --recompare')
    vanilla = {'biomes_3d': {}, 'features_by_biome': {}}
    references = []
    for reference in args.reference:
        report = json.loads(reference.read_text())
        side = reference_side(report,plan['seed'])
        references.append([reference.name, hashlib.sha256(reference.read_bytes()).hexdigest()])
        for biome, volume in side.get('biomes_3d', {}).items():
            # Основная перепись приоритетнее целевых окон; последние нужны
            # для биомов, которых в основной выборке нет или слишком мало.
            target = reference.stem.removeprefix('biome-')
            if reference != args.reference[0] and biome != target:
                continue
            if vanilla['biomes_3d'].get(biome, 0)*64 < 256*384*4 and volume > vanilla['biomes_3d'].get(biome, 0):
                vanilla['biomes_3d'][biome] = volume
                vanilla['features_by_biome'][biome] = side['features_by_biome'].get(biome, {})
    if args.recompare:
        previous = json.loads(args.recompare.read_text())
        if previous['plan'] != plan:
            raise ValueError('План сохранённых счётчиков отличается')
        ours = previous['ours']
    else:
        ours = measure(args.world, plan)
    args.output.write_text(json.dumps({'schema': 1, 'method': 'Independent biome-conditioned densities; per million 3D biome blocks; not coordinate matching',
        'plan': plan, 'references': references,
        'ours': ours, 'rows': compare(ours, vanilla)}, indent=2)+'\n')
    print(ours['chunks'], 'chunks;', sum(r['outside'] for r in compare(ours, vanilla)), 'outside')


if __name__ == '__main__':
    main()
