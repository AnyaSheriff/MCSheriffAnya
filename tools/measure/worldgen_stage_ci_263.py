#!/usr/bin/env python3
"""GitHub-only stage experiment using a NEW build; old frozen baseline guard unchanged."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.request

import worldgen_exact_263 as exact
import worldgen_exact_ci_263 as baseline
import worldgen_stage_263 as stage

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load_records(world, wanted):
    reader = exact.Reader()
    region = exact.region_dir(world, 'overworld')
    records = {}
    files = [p for p in sorted(region.iterdir()) if p.suffix in ('.mca', '.mcs')]
    before = exact.fingerprints(files)
    for path in files:
        for cx, cz, blocks, biomes, _ in reader.read(path, wanted):
            if (cx, cz) in records or (cx, cz) not in wanted:
                raise ValueError('duplicate/unrequested saved chunk')
            records[cx, cz] = exact.chunk_data(reader, blocks, biomes)
    if records.keys() != wanted or before != exact.fingerprints(files):
        raise ValueError('incomplete or changing saved coverage')
    return records


def validate_build(receipt, head, binary, trace):
    if (receipt.get('source') != head or receipt.get('elf_sha256') != sha(binary)
            or receipt.get('trace_sha256') != sha(trace)
            or receipt.get('cargo_lock_sha256') != sha(ROOT / 'Cargo.lock')):
        raise ValueError('new source/build/trace identity mismatch')
    if not Path(binary).read_bytes().startswith(b'\x7fELF'):
        raise ValueError('new local GitHub build must be ELF')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'trace', 'build-receipt', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('MCSA_A6_DENSITY') != '0':
        raise SystemExit('GitHub only; explicit default density MCSA_A6_DENSITY=0 required')
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if os.environ.get('GITHUB_SHA') != head:
        raise ValueError('workflow source identity mismatch')
    subprocess.run(['git', 'diff', '--exit-code', 'HEAD'], cwd=ROOT, check=True)
    binary, trace = args.binary.resolve(), args.trace.resolve()
    build = json.loads(args.build_receipt.read_text())
    validate_build(build, head, binary, trace)
    traces = stage.read_traces(trace, require_early_stages=True)
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    pin = json.loads((ROOT / 'tools/blackbox/ci/sources-777.json').read_text())['official_jar']
    with urllib.request.urlopen(pin['url'], timeout=120) as response:
        jar = response.read(pin['bytes'] + 1)
    if len(jar) != pin['bytes'] or hashlib.sha256(jar).hexdigest() != pin['sha256']:
        raise ValueError('pinned official JAR identity mismatch')
    inputs = {'source': head, 'build': build, 'jar_pin': pin, 'seeds': stage.SEEDS,
              'holdout_seeds': stage.SEEDS[2:], 'trace_chunks': stage.TRACE_CHUNKS,
              'saved_chunks': baseline.CHUNKS, 'density_env': '0',
              'scope': 'New own source build, ordinary saved output, read-only own stages; official final saves only.'}
    (output / 'inputs.json').write_text(json.dumps(inputs, indent=2) + '\n')
    results = []
    with tempfile.TemporaryDirectory(prefix='mcsa-stage-', dir=os.environ['RUNNER_TEMP']) as temporary:
        temporary = Path(temporary)
        jarpath = temporary / 'official.jar'; jarpath.write_bytes(jar)
        for i, seed in enumerate(stage.SEEDS):
            evidence = output / f'seed-{i + 1}'; evidence.mkdir()
            scratch = temporary / f'seed-{i + 1}'; scratch.mkdir()
            plan = {'version': '26.3', 'seed': seed, 'dimension': 'overworld',
                    'chunks': [list(c) for c in baseline.CHUNKS]}
            (evidence / 'plan.json').write_text(json.dumps(plan, indent=2) + '\n')
            vanilla = baseline.generate('vanilla', seed, jarpath, scratch, evidence, 27860)
            ours = baseline.generate('ours', seed, binary, scratch, evidence, 27861)
            report = exact.compare_worlds(ours, vanilla, plan)
            (evidence / 'comparison.json').write_text(json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False) + '\n')
            if not report['recording_valid']:
                raise ValueError('invalid paired comparison: ' + repr(report['errors']))
            diagnostic = report['height_argmax_diagnostics']
            if (diagnostic['abs_error_max'] != report['top_non_air_abs_error_max']
                    or diagnostic['abs_error_sum'] != report['top_non_air_abs_error_sum']
                    or diagnostic['compared_columns'] != report['totals']['top_non_air_heights']['compared']):
                raise ValueError('full argmax does not reconcile with exact gate')
            wanted = set(stage.TRACE_CHUNKS)
            saved = load_records(ours, wanted)
            selected = [r['column'] for r in diagnostic['neighbors'] if r['in_plan']]
            links = [stage.link_trace(traces[seed, coord], saved[coord], selected) for coord in sorted(wanted)]
            stage_report = {'schema': 1, 'seed': seed, 'holdout': seed in stage.SEEDS[2:],
                            'own_stage_semantics': 'Initial fill includes inline cave sampling. Separate snapshots record ore-host preparation, ores, and underground digging before the decorated base.',
                            'official_observation': 'Only final ordinary saved blocks/biomes; no official internal-stage data.',
                            'argmax_column_in_trace': tuple(diagnostic['selected_column'][j] // 16 for j in range(2)) in wanted,
                            'all_trace_chunks_final_saved_equal': True, 'traced_chunks': links}
            (evidence / 'stage-link.json').write_text(json.dumps(stage_report, ensure_ascii=False, indent=2, allow_nan=False) + '\n')
            results.append({'seed': seed, 'holdout': seed in stage.SEEDS[2:], 'recording_valid': True,
                            'parity': report['parity'], 'argmax': diagnostic['selected_column'],
                            'height_max': diagnostic['abs_error_max'], 'tie_count': diagnostic['tie_count'],
                            'own_observer_identity': True, 'final_saved_equal': True,
                            'argmax_column_in_trace': stage_report['argmax_column_in_trace']})
    (output / 'summary.json').write_text(json.dumps({'source': head, 'results': results,
        'generator_fix': False, 'scope': 'Diagnostic stage experiment, not full-generator parity.'}, indent=2) + '\n')


if __name__ == '__main__':
    main()
