#!/usr/bin/env python3
"""Перепись A6: только свои свежие чанки и чтение официальных регионов."""
import argparse
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'tools/measure'))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'batch-a5'))
from genaudit_263 import analyze, comparison
from genaudit_263_io import manifest
from vertical_profile import profile


def read(path):
    return json.loads(path.read_text())


def write(path, data):
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')


def samples():
    source = ROOT / 'testdata/world-profile/genaudit-26.3'
    plans = read(source / 'batch-a3/measurement-plan.json')['samples']
    peaks = read(source / 'batch-a5/peak-plan.json')
    return plans + [{'label': 'peaks', 'seed': plans[0]['seed'], **peaks}]


def prepare(scratch):
    scratch.mkdir(parents=True, exist_ok=True)
    if scratch.is_relative_to(ROOT):
        raise ValueError('Миры должны оставаться вне репозитория')
    for kind in ('before', 'candidate'):
        plan = []
        for sample in samples():
            chunks = sorted({(x, z) for x0, z0, x1, z1 in sample['windows']['overworld']
                             for x in range(x0, x1+1) for z in range(z0, z1+1)})
            plan.append({'seed': sample['seed'], 'label': sample['label'], 'chunks': chunks,
                         'output': str(scratch / f"{sample['label']}-{kind}/world")})
        write(scratch / f'export-{kind}.json', {'samples': plan})


def measure(scratch, official_root, kind, label_filter=None):
    # Вызывать только после завершения экспортирующего теста: параллельное
    # чтение записываемого региона даёт неполную выборку, которую отвергает comparison.
    for sample in samples():
        label = sample['label']
        if label_filter is not None and label != label_filter:
            continue
        world = scratch / f'{label}-{kind}/world'
        if label != 'peaks':
            official = read(official_root / f'genA3/{label}-vanilla-profile.json')
            current = manifest(official_root / f'worlds/{label}-vanilla/world')
            if current != official['manifest']:
                raise ValueError('Контрольные суммы оригинала отличаются от кэша A3')
            write(scratch / f'{label}-official-manifest.json', current)
            ours = analyze(world, sample, manifest(world))
            if ours['level']['seed'] != sample['seed']:
                raise ValueError('Семя созданного мира не совпадает с планом')
            write(scratch / f'{label}-{kind}-profile.json', ours)
            write(scratch / f'{label}-{kind}.json', comparison(ours, official, False))
        profile(world, sample, scratch / f'{label}-{kind}-vertical.json')


def summarize(scratch, official_root):
    result = {'status': 'partial; experimental router is opt-in; default terrain unchanged',
              'method': 'Fresh generated native chunks; unchanged A3/A5 plans and tolerances. '
                        'Official manifests checked against cached A3 statistics. '
                        'Biome/depth statistics, no equal-coordinate height comparison.',
              'full_census': 'Not run: GitHub workflow builds a committed revision; this work is uncommitted.',
              'baseline_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'candidate_switch': 'MCSA_A6_DENSITY=1; absent or zero keeps the original generator',
              'generated_support_control_fixture': read(Path(__file__).parent / 'support.json'),
              'seeds': {}}
    for label in ('owner', 'second'):
        before = read(scratch / f'{label}-before.json')
        after = read(scratch / f'{label}-candidate.json')
        seed = next(s['seed'] for s in samples() if s['label'] == label)
        if before['ours_level']['seed'] != seed or after['ours_level']['seed'] != seed:
            raise ValueError('Семя отчёта не совпадает с планом')
        if not all(c['complete'] and c['same_chunks'] for r in (before, after) for c in r['coverage']):
            raise ValueError('Неполное или различное покрытие')
        previous = {r['metric']: r for r in before['rows']}
        current = {r['metric']: r for r in after['rows']}
        new = [r for m, r in current.items() if r['verdict'] == 'outside'
               and m in previous and previous[m]['verdict'] == 'within']
        closed = [r for m, r in current.items() if r['verdict'] == 'within'
                  and m in previous and previous[m]['verdict'] == 'outside']
        worsened = [r for m, r in current.items() if r['verdict'] == 'outside'
                    and m in previous and abs(r['delta']) > abs(previous[m]['delta']) + 1e-6]
        worsened_within = [r for m, r in current.items() if r['verdict'] == 'within'
                           and m in previous and abs(r['delta']) > abs(previous[m]['delta']) + 1e-6]
        rows = []
        for metric, r in current.items():
            if (metric.startswith(('height_biome/', 'slope/', 'depth/160/', 'depth/192/', 'biomes_', 'surface_biomes/'))
                or metric in ('feature_biome/windswept_hills/snow', 'top/meadow/grass_block',
                              'layer/cherry_grove/-3/dirt', 'layer/beach/-4/stone', 'blocks/red_terracotta')):
                p = previous.get(metric)
                rows.append({'metric': metric, 'before': p['ours'] if p else None,
                             'candidate': r['ours'], 'vanilla': r['vanilla'], 'tolerance': r['tolerance'],
                             'denominator_before': p['denominator'] if p else None,
                             'denominator_candidate': r['denominator'], 'verdict': r['verdict']})
        result['seeds'][label] = {
            'coverage': after['coverage'],
            'outside_before_by_batch': dict(Counter(r['batch'] for r in before['rows'] if r['verdict'] == 'outside')),
            'outside_candidate_by_batch': dict(Counter(r['batch'] for r in after['rows'] if r['verdict'] == 'outside')),
            'new_outside_common_by_batch': dict(Counter(r['batch'] for r in new)),
            'resolved_common_by_batch': dict(Counter(r['batch'] for r in closed)),
            'worsened_outside_common_by_batch': dict(Counter(r['batch'] for r in worsened)),
            'new_outside_common': [{'before': previous[r['metric']]['ours'], **r} for r in new],
            'worsened_outside_common': [{'before': previous[r['metric']]['ours'], **r} for r in worsened],
            'worsened_within_common': [{'before': previous[r['metric']]['ours'], **r} for r in worsened_within],
            'new_rows': [r for m, r in current.items() if m not in previous],
            'removed_rows': [r for m, r in previous.items() if m not in current],
            'selected_metrics': rows,
        }
        result['seeds'][label]['upper_air_census_counts'] = {}
        for kind in ('before', 'candidate', 'vanilla'):
            path = (official_root / f'genA3/{label}-vanilla-profile.json' if kind == 'vanilla'
                    else scratch / f'{label}-{kind}-profile.json')
            bands = read(path)['dimensions']['overworld']['depth_bands']
            result['seeds'][label]['upper_air_census_counts'][kind] = {
                y: {'air': bands[y]['underground_air'], 'denominator': bands[y]['below_surface_volume']}
                for y in ('160', '192')}
    result['vertical_profiles'] = {
        label: {kind: read(scratch / f'{label}-{kind}-vertical.json')
                for kind in ('before', 'candidate')} for label in ('owner', 'second', 'peaks')}
    result['official_vertical_profiles'] = {
        label: read((scratch if label == 'peaks' else official_root / 'genA5')
                    / f'{label}-vanilla-vertical.json')
        for label in ('owner', 'second', 'peaks')}
    result['binaries_sha256'] = {
        kind: hashlib.sha256((scratch / f'test-{kind}').read_bytes()).hexdigest()
        for kind in ('before', 'candidate')}
    result['candidate_binary_note'] = ('Binary hashes identify the measured executables; source hashes identify the reporting tree. Consult the corresponding build logs for compile provenance.')
    result['official_manifest_check'] = {}
    for label in ('owner', 'second'):
        actual = read(scratch / f'{label}-official-manifest.json')
        expected = read(official_root / f'genA3/{label}-vanilla-profile.json')['manifest']
        if actual != expected:
            raise ValueError('Манифест оригинала не совпадает с кэшем')
        result['official_manifest_check'][label] = {
            'cached_manifest_matches_current_official_files': True,
            'manifest_sha256': hashlib.sha256(json.dumps(actual, sort_keys=True).encode()).hexdigest()}
    result['tracked_sources_sha256'] = {
        str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in (ROOT / 'src/world/terrain.rs', ROOT / 'src/world/terrain_density.rs',
                     ROOT / 'tools/measure/genaudit_263.py',
                     ROOT / 'testdata/world-profile/genaudit-26.3/batch-a3/measurement-plan.json',
                     ROOT / 'testdata/world-profile/genaudit-26.3/batch-a5/peak-plan.json')}
    log = (scratch / 'workspace-final.log').read_text()
    counts = [tuple(map(int, match)) for match in re.findall(
        r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; 0 filtered out;', log)]
    result['verification'] = {
        'release_workspace': dict(zip(('passed', 'failed', 'ignored'), map(sum, zip(*counts)))),
        'nested_filtered_subprocess_counted_twice': False,
        'release_clippy': (scratch / 'clippy-final.log').read_text().splitlines()[-1].strip(),
        'workspace_log_sha256': hashlib.sha256(log.encode()).hexdigest(),
        'clippy_log_sha256': hashlib.sha256((scratch / 'clippy-final.log').read_bytes()).hexdigest(),
        'cargo_build_jobs': 4, 'cargo_shared_lock': str(official_root / '.cargo.lock'),
        'cargo_fmt_run': False}
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('prepare', 'measure', 'summarize'))
    parser.add_argument('--scratch', type=Path, required=True)
    parser.add_argument('--official-root', type=Path)
    parser.add_argument('--kind', choices=('before', 'candidate'))
    parser.add_argument('--label', choices=('owner', 'second', 'peaks'))
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    scratch = args.scratch.resolve()
    if args.action == 'prepare':
        prepare(scratch)
    elif args.action == 'measure':
        if args.official_root is None or args.kind is None:
            parser.error('measure требует --official-root и --kind')
        measure(scratch, args.official_root, args.kind, args.label)
    else:
        if args.output is None:
            parser.error('summarize требует --output')
        write(args.output, summarize(scratch, args.official_root or scratch.parent))
