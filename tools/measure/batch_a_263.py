#!/usr/bin/env python3
"""Перепись поверхностной сетки Batch A из biome_profile_263 CSV.

Те же шаг 4, четыре соседа и цензурирование краёв, что у genaudit_263.
Пустоты между окнами неизвестны: они не считаются сушей или частью пятна.
CSV измеряет свой генератор, JSON оригинала берётся из genaudit_263.
"""
import argparse
import csv
import json
from pathlib import Path

import numpy as np
from scipy import ndimage


def quantiles(values):
    return None if not len(values) else {
        str(q): round(float(np.percentile(values, q)), 5)
        for q in (0, 5, 25, 50, 75, 95, 99, 100)
    }


def measure(path):
    with path.open() as source:
        rows = list(csv.DictReader(source))
    if not rows:
        raise ValueError('Пустая выборка')
    xs = np.array([int(row['x']) for row in rows])
    zs = np.array([int(row['z']) for row in rows])
    if np.any(xs % 4 != xs[0] % 4) or np.any(zs % 4 != zs[0] % 4):
        raise ValueError('Нужна сетка клеток 4×4')
    names = sorted({row['biome'] for row in rows})
    indices = {name: i + 1 for i, name in enumerate(names)}
    grid = np.zeros(((zs.max()-zs.min())//4+1, (xs.max()-xs.min())//4+1), dtype=np.uint16)
    ix, iz = (xs-xs.min())//4, (zs-zs.min())//4
    if len(set(zip(ix.tolist(), iz.tolist()))) != len(rows):
        raise ValueError('Повтор клеток смешивает окна')
    grid[iz, ix] = [indices[row['biome']] for row in rows]
    valid = grid != 0
    edge = ndimage.distance_transform_edt(np.pad(valid, 1))[1:-1, 1:-1]*4
    ocean = np.isin(grid, [indices[name] for name in names if 'ocean' in name])
    land = valid & ~ocean
    distance = ndimage.distance_transform_edt(~land)*4
    result = {'cells': len(rows), 'step': 4, 'surface_biomes': {}, 'patches': {}, 'ocean_coast_distance': {}}
    for name, code in indices.items():
        mask = grid == code
        result['surface_biomes'][name] = round(float(mask.sum()/valid.sum()*100), 6)
        labels, count = ndimage.label(mask)
        sizes = np.bincount(labels.ravel())[1:]*16
        touched = set(np.unique(labels[edge <= 4]))
        complete = [size for i, size in enumerate(sizes, 1) if i not in touched]
        result['patches'][name] = {'count': int(count), 'uncensored_count': len(complete),
                                  'uncensored_area_quantiles': quantiles(complete)}
        if 'ocean' in name:
            selected = mask & (distance < edge)
            result['ocean_coast_distance'][name] = {
                'n': int(selected.sum()), 'censored': int(mask.sum()-selected.sum()),
                'distance_blocks': quantiles(distance[selected]),
            }
    heights = [int(row['height']) for row in rows if int(row['height']) != -999]
    result['height_quantiles'] = quantiles(heights)
    return result


def prepare(root):
    repo = Path(__file__).resolve().parents[2]
    root = root.resolve()
    if root.is_relative_to(repo):
        raise ValueError('Большие результаты должны быть вне репозитория')
    root.mkdir(parents=True, exist_ok=True)
    fixture = json.loads((repo/'testdata/world-profile/genaudit-26.3/batch-a-plan.json').read_text())
    exported = []
    for label, sample in fixture['samples'].items():
        plan = {'seed': sample['seed'], 'windows': sample['windows']}
        (root/f'{label}-sample-plan.json').write_text(json.dumps(plan, indent=2)+'\n')
        areas = sample['windows']['overworld']
        chunks = [[x,z] for x0,z0,x1,z1 in areas for z in range(z0,z1+1) for x in range(x0,x1+1)]
        exported.append({'seed': sample['seed'], 'output': str(root/sample['output_relative']), 'chunks': chunks})
        rects = [[x0*16+2,z0*16+2,(x1-x0+1)*16,(z1-z0+1)*16,4] for x0,z0,x1,z1 in areas]
        (root/f'{label}-sample-profile.json').write_text(json.dumps({'seed':sample['seed'],'heights':True,'rects':rects},indent=2)+'\n')
        if 'full_rects' in sample:
            (root/f'{label}-full-height-profile.json').write_text(json.dumps(
                {'seed':sample['seed'],'heights':True,'rects':sample['full_rects']},indent=2)+'\n')
    (root/'export-plan.json').write_text(json.dumps({'samples':exported},indent=2)+'\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('csv', type=Path, nargs='?')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--prepare', type=Path, help='Подготовить воспроизводимые планы вне репозитория')
    args = parser.parse_args()
    if args.prepare:
        prepare(args.prepare)
        return
    if args.csv is None or args.output is None:
        parser.error('Нужны CSV и --output либо --prepare')
    args.output.write_text(json.dumps(measure(args.csv), ensure_ascii=False, indent=2)+'\n')


if __name__ == '__main__':
    main()
