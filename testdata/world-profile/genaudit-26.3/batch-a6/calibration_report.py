#!/usr/bin/env python3
"""Сводка калибровки A6 с историческим первым опытом и малыми контролями."""
import argparse
import json
from pathlib import Path
from measure import summarize, read, write
from calibration_profile import measure


def report(scratch, history, official_root):
    result = summarize(scratch, official_root)
    previous = read(Path(__file__).parent / 'measurements.json')
    result['round'] = 'A6 calibration; final reciprocal-factor candidate'
    result['accepted_as_default'] = False
    result['samples'] = {'inherited_default_and_first_prototype_chunks': 9729,
                         'small_candidate_v1_chunks': 831, 'small_candidate_v2_chunks': 831,
                         'fresh_final_chunks': 9729,
                         'note': 'Small windows overlap A3 and each other; counts are not independent coverage.'}
    result['first_prototype_outside_by_batch'] = {
        label: s['outside_candidate_by_batch'] for label, s in previous['seeds'].items()}
    result['first_prototype_selected_metrics'] = {
        label: s['selected_metrics'] for label, s in previous['seeds'].items()}
    result['first_prototype_vertical_profiles'] = {
        label: data['candidate'] for label, data in previous['vertical_profiles'].items()}
    result['final_peak_height_and_slope_tails'] = read(scratch / 'peaks-calibration-tails.json')
    result['small_profiles'] = {}
    for label in ('owner', 'second', 'peaks'):
        result['small_profiles'][label] = {
            kind: read(history / f'{label}-small-{kind}.json') for kind in ('vanilla', 'prototype', 'calibrated')}
        result['small_profiles'][label]['calibrated_v2'] = read(history / f'v2/{label}-small-calibrated.json')
    result['small_plans'] = {
        label: read(history / f'{label}-primary-small-plan.json') for label in ('owner', 'second')}
    result['small_plans']['peaks'] = read(Path(__file__).parent.parent / 'batch-a5/peak-plan.json')
    result['raw_terrain_attribution_first_prototype'] = {
        'biome': 'jagged_peaks', 'window': [139,105,147,113],
        'bands': {'128': {'eligible': 45312, 'raw_terrain_air': 0, 'carve_predicate': 9565},
                  '144': {'eligible': 29496, 'raw_terrain_air': 0, 'carve_predicate': 5371},
                  '160': {'eligible': 3592, 'raw_terrain_air': 0, 'carve_predicate': 635}},
        'note': 'Raw pre-carver denominators match native profile. Carve predicate also includes later aquifer barriers; it is not the final air count.'}
    result['paired_cherry_depth3'] = read(history / 'material-attribution.json')
    result['cherry_fuzzy_witnesses'] = read(history / 'cherry-fuzzy-witnesses.json')
    fixture = read(Path(__file__).parent / 'support.json')
    plan = {'windows': {'overworld': [[x,z,x,z] for x,z in fixture['chunks']]}}
    result['matched_generated_support_profile'] = measure(scratch / 'peaks-candidate/world', plan)
    result['first_241_outcomes'] = []
    for label in ('owner', 'second'):
        current = {r['metric']: r for r in read(scratch / f'{label}-candidate.json')['rows']}
        for row in previous['seeds'][label]['new_outside_common']:
            now = current.get(row['metric'])
            result['first_241_outcomes'].append({
                'seed': label, 'batch': row['batch'], 'metric': row['metric'],
                'default': row['before'], 'prototype': row['ours'], 'vanilla': row['vanilla'],
                'calibrated': now['ours'] if now else None,
                'verdict': now['verdict'] if now else 'removed; not a fix',
                'denominator': now['denominator'] if now else None})
    result['official_aggregate_tails'] = {}
    for label in ('owner', 'second'):
        d = read(official_root / f'genA3/{label}-vanilla-profile.json')['dimensions']['overworld']
        result['official_aggregate_tails'][label] = {
            'height_quantiles': d['height_quantiles'], 'slope_quantiles': d['slope_quantiles'],
            'height_by_biome_histograms': d['height_by_biome']}
    result['unverified'] = [
        'Coastal upper-air fractions and rare height/slope tails remain outside tolerance.',
        'The high-C/low-E spline extension is not validated on a holdout seed.',
        'The palette uses correct absolute-Y indexing; its seed-general band layout is unverified.',
        'Feature effects are still failures when outside tolerance, even if terrain-mediated.',
        'Per-row causal fractions for mixed surface/cutoff failures require controlled ablation.',
        'Float32 equivalence and the exact preliminary surface function remain unverified.',
        'Full GitHub census requires a committed revision and explicit MCSA_A6_DENSITY=1.']
    return result


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--scratch', required=True, type=Path)
    p.add_argument('--history', required=True, type=Path)
    p.add_argument('--official-root', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    args = p.parse_args()
    write(args.output, report(args.scratch, args.history, args.official_root))
