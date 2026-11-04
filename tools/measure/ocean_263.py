#!/usr/bin/env python3
"""Перепись O3: только сохранённые регионы, плотность по 3D-биому и глубина.

Источники задаются JSON-списком {world, plan}; перекрытия считаются один раз.
План — обычный genaudit с windows.overworld. Никакого кода игры не читаем.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path

import numpy as np

from genaudit_263 import requested, natural
from genaudit_263_io import Reader, level, region_dir


def owned(name):
    return name in {'kelp', 'kelp_plant', 'seagrass', 'tall_seagrass', 'sea_pickle'} or 'coral' in name


def coral_components(corals, known):
    result = {}
    adjacent = [(dx, dy, dz) for dx in (-1, 0, 1) for dy in (-1, 0, 1) for dz in (-1, 0, 1)
                if (dx, dy, dz) != (0, 0, 0)]
    for kind, positions in corals.items():
        positions = set(positions)
        histograms = {k: Counter() for k in ('blocks', 'span_x', 'span_y', 'span_z')}
        joint = Counter()
        clipped = 0
        while positions:
            first = positions.pop()
            stack = [first]
            points = [first]
            while stack:
                x, y, z = stack.pop()
                for dx, dy, dz in adjacent:
                    p = (x+dx, y+dy, z+dz)
                    if p in positions:
                        positions.remove(p)
                        stack.append(p)
                        points.append(p)
            if any(((x+dx)//16, (z+dz)//16) not in known for x, _, z in points
                   for dx in (-1, 0, 1) for dz in (-1, 0, 1)):
                clipped += 1
                continue
            histograms['blocks'][str(len(points))] += 1
            spans = []
            for axis, name in enumerate(('span_x', 'span_y', 'span_z')):
                span = max(p[axis] for p in points)-min(p[axis] for p in points)+1
                histograms[name][str(span)] += 1
                spans.append(span)
            joint[','.join(map(str, [*spans, len(points)]))] += 1
        result[kind] = {'histograms': {k: dict(v) for k, v in histograms.items()},
                        'joint_span_xyz_blocks': dict(joint), 'clipped': clipped}
    return {'method': '26-connected components of one live coral-block kind; boundary-truncated components excluded. Adjacent reefs can merge; tree/claw/mushroom labels cannot be uniquely recovered from saved blocks.',
            'by_kind': result}


def measure(sources, shapes=False):
    reader = Reader()
    seen = set()
    volume = Counter()
    counts = defaultdict(Counter)
    depth = defaultdict(Counter)
    height = defaultdict(Counter)
    kelp = defaultdict(Counter)
    habitat = defaultdict(Counter)
    floor_depth = defaultdict(Counter)
    files = []
    seeds = set()
    corals = defaultdict(set)
    for source in sources:
        world = Path(source['world'])
        plan = json.loads(Path(source['plan']).read_text())
        seed = level(world)['seed']
        assert seed == plan['seed']
        seeds.add(seed)
        wanted = requested(plan, 'overworld') - seen
        found = set()
        folder = region_dir(world, 'overworld')
        for path in sorted([*folder.glob('*.mca'), *folder.glob('*.mcs')]):
            used = False
            for cx, cz, blocks, bios, _ in reader.read(path, wanted):
                found.add((cx, cz))
                used = True
                bio = np.asarray(bios).reshape(96, 4, 4)
                bs, ns = np.unique(bio, return_counts=True)
                volume.update({str(b): int(n)*64 for b, n in zip(bs, ns)})
                ground = np.asarray([natural(n) for n in reader.names])[blocks]
                floor = np.where(ground.any(axis=0), 383-ground[::-1].argmax(axis=0)-64, -64)
                zz, xx = np.indices((16, 16))
                bottom = np.minimum(floor+65, 383)
                floor_bio = bio[bottom//4, zz//4, xx//4]
                wet_codes = [i for i, n in enumerate(reader.names)
                             if n in {'water', 'seagrass', 'tall_seagrass', 'kelp', 'kelp_plant'}]
                wet = (floor < 62) & np.isin(blocks[bottom, zz, xx], wet_codes)
                for biome in np.unique(floor_bio):
                    mask = floor_bio == biome
                    habitat[str(biome)]['columns'] += int(mask.sum())
                    habitat[str(biome)]['wet_floor_columns'] += int((mask & wet).sum())
                    ds, ns = np.unique((62-floor[mask & wet])//8*8, return_counts=True)
                    floor_depth[str(biome)].update({str(int(d)): int(n) for d, n in zip(ds, ns)})
                codes = [i for i, n in enumerate(reader.names) if owned(n)]
                for code in codes:
                    ys, zs, xs = np.where(blocks == code)
                    if not len(ys):
                        continue
                    name = reader.names[code]
                    if shapes and name.endswith('_coral_block') and not name.startswith('dead_'):
                        corals[name].update((cx*16+int(x), int(y)-64, cz*16+int(z)) for y, z, x in zip(ys, zs, xs))
                    biomes = bio[ys//4, zs//4, xs//4]
                    for biome in np.unique(biomes):
                        mask = biomes == biome
                        key = str(biome) + '/' + name
                        counts[str(biome)][name] += int(mask.sum())
                        ds, ns = np.unique((62-(ys[mask]-64))//8*8, return_counts=True)
                        depth[key].update({str(int(d)): int(n) for d, n in zip(ds, ns)})
                        hs, ns = np.unique(ys[mask]-64-floor[zs[mask], xs[mask]], return_counts=True)
                        height[key].update({str(int(h)): int(n) for h, n in zip(hs, ns)})
                    if name == 'kelp':
                        stem = reader.lookup['kelp_plant']
                        for y, z, x, biome in zip(ys, zs, xs, biomes):
                            length = 1
                            while y-length >= 0 and blocks[y-length, z, x] == stem:
                                length += 1
                            kelp[str(biome)][str(length)] += 1
            if used:
                files.append([str(path), hashlib.sha256(path.read_bytes()).hexdigest()])
        assert found == wanted, f'Неполная выборка {world}: {len(found)}/{len(wanted)}'
        seen.update(found)
    assert len(seeds) == 1
    return {'seed': seeds.pop(), 'chunks': len(seen), 'biome_blocks': dict(volume),
            'counts': dict(counts), 'depth_below_sea_8': dict(depth),
            'height_above_floor': dict(height), 'kelp_lengths': dict(kelp),
            'habitat': dict(habitat), 'floor_depth_below_sea_8': dict(floor_depth), 'files': files,
            'coral_components': coral_components(corals, seen) if shapes else None}


def compare(ours, vanilla):
    rows = []
    for biome in sorted(ours['biome_blocks'].keys() | vanilla['biome_blocks'].keys()):
        a, b = ours['biome_blocks'].get(biome, 0), vanilla['biome_blocks'].get(biome, 0)
        if min(a, b) < 65536:
            continue
        ac, bc = ours['counts'].get(biome, {}), vanilla['counts'].get(biome, {})
        names = ac.keys() | bc.keys()
        if 'ocean' in biome:
            names |= {'kelp', 'kelp_plant', 'seagrass', 'tall_seagrass', 'sea_pickle'}
            names |= {kind+'_coral'+suffix for kind in ('tube', 'brain', 'bubble', 'fire', 'horn')
                      for suffix in ('', '_block', '_fan', '_wall_fan')}
        for name in sorted(names):
            av, bv = ac.get(name, 0)*1e6/a, bc.get(name, 0)*1e6/b
            tolerance = max(5, bv*.2)
            rows.append({'metric': f'feature_biome/{biome}/{name}', 'ours': av,
                         'vanilla': bv, 'tolerance': tolerance, 'outside': abs(av-bv)>tolerance,
                         'biome_blocks': [a, b], 'small_sample': min(a, b)<256*384*4})
    return rows


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--sources', type=Path)
    p.add_argument('--measured', type=Path)
    p.add_argument('--reference', type=Path)
    p.add_argument('--reference-label', choices=['owner', 'second'])
    p.add_argument('--official-reports', type=Path, nargs='+')
    p.add_argument('--prepare-export', type=Path)
    p.add_argument('--plans', type=Path)
    p.add_argument('--shapes', action='store_true')
    p.add_argument('--output', required=True, type=Path)
    args = p.parse_args()
    if args.prepare_export:
        root = args.prepare_export.resolve()
        assert not root.is_relative_to(Path(__file__).resolve().parents[2])
        root.mkdir(parents=True, exist_ok=True)
        samples = []
        for label, plan in json.loads(args.plans.read_text())['plans'].items():
            world = root/label
            assert not (world/'level.dat').exists(), 'Нужен свежий мир'
            samples.append({'seed': plan['seed'], 'output': str(world),
                            'chunks': sorted(requested(plan, 'overworld'))})
            file = root/(label+'-plan.json')
            file.write_text(json.dumps(plan)+'\n')
            (root/(label+'-sources.json')).write_text(json.dumps([{'world': str(world), 'plan': str(file)}])+'\n')
        args.output.write_text(json.dumps({'samples': samples})+'\n')
        return
    ours = json.loads(args.measured.read_text())['statistics'] if args.measured else measure(json.loads(args.sources.read_text()), args.shapes)
    result = {'statistics': ours}
    if args.official_reports:
        references = []
        primary = set()
        density = {'seed': ours['seed'], 'biome_blocks': {}, 'counts': {}}
        # Первый отчёт — полная основная перепись. Остальные дают только
        # собственный целевой биом, когда его нет в основном квадрате.
        for index, path in enumerate(args.official_reports):
            report = json.loads(path.read_text())
            assert report['vanilla_level']['seed'] == ours['seed']
            assert isinstance(report['vanilla_level']['version'], dict)
            data = report['statistics']['vanilla']['dimensions']['overworld']
            target = path.stem.removeprefix('biome-') if index else None
            for biome, cells in data['biomes_3d'].items():
                if target and (biome != target or biome in primary):
                    continue
                density['biome_blocks'][biome] = cells*64
                density['counts'][biome] = {k: v for k, v in data['features_by_biome'].get(biome, {}).items() if owned(k)}
            if index == 0:
                primary = set(data['biomes_3d'])
            references.append([str(path), hashlib.sha256(path.read_bytes()).hexdigest()])
        result['density_reference_files'] = references
        result['density_statistics'] = density
        result['density_note'] = 'Full original main census per biome; absent biomes from their own full target only. Depth histograms remain independently sampled regions.'
    if args.reference:
        reference = json.loads(args.reference.read_text())
        if args.reference_label:
            reference = reference['references'][args.reference_label]
        vanilla = reference.get('density_statistics', reference['statistics'])
        assert ours['seed'] == vanilla['seed']
        result['rows'] = compare(ours, vanilla)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    print(ours['chunks'], 'chunks;', sum(r['outside'] for r in result.get('rows', [])), 'outside')


if __name__ == '__main__':
    main()
