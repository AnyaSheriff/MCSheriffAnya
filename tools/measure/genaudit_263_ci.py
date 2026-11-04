#!/usr/bin/env python3
"""CI orchestration of the unchanged saved-world analyzer and frozen pass2 statistics.

No official world, game archive, account credentials or external server is used.
Generated worlds stay in the runner temporary directory and are never artifacts.
"""
import argparse
import collections
import ctypes
import gzip
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[2]
REFERENCE = REPO / 'testdata/world-profile/genaudit-26.3/ci-reference-pass2.json.gz'
EXPECTED = {'overworld': 37249, 'the_nether': 1089, 'the_end': 2803}
SEEDS = {'owner': 1792932773345722119, 'second': 100554032945340}
GROUPS = set()


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def canonical_sha(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                    separators=(',', ':')).encode()).hexdigest()


def write(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, allow_nan=False,
                                    separators=(',', ':')) + '\n')


def analyzer_sha():
    folder = REPO / 'tools/measure'
    return hashlib.sha256((folder / 'genaudit_263.py').read_bytes() +
                          (folder / 'genaudit_263_io.py').read_bytes()).hexdigest()


def coverage(report):
    assert {r['dimension'] for r in report['coverage']} == set(EXPECTED)
    assert len(report['coverage']) == 3
    for row in report['coverage']:
        assert row['complete'] and row['same_chunks'], 'Incomplete coverage'
        for side in ('ours', 'vanilla'):
            item = row[side]
            assert item['chunks'] == item['requested'] == EXPECTED[row['dimension']]


def reference(path=REFERENCE):
    metadata = json.loads(Path(path).with_suffix('').read_text())
    assert sha(path) == metadata['fixture_sha256'], 'Compressed reference changed'
    assert Path(path).stat().st_size == metadata['bytes']
    data = json.loads(gzip.decompress(Path(path).read_bytes()))
    assert data['schema'] == 1
    assert data['analyzer_source_sha256'] == analyzer_sha(), 'Frozen analyzer changed'
    assert set(data['cases']) == set(SEEDS)
    for label, case in data['cases'].items():
        assert case['seed'] == case['plan']['seed'] == SEEDS[label]
        assert case['plan_sha256'] == metadata['cases'][label]['plan_sha256_canonical']
        assert case['baseline_report_sha256'] == metadata['cases'][label]['report_sha256_canonical']
        assert canonical_sha(case['plan']) == case['plan_sha256'], 'Plan changed'
        baseline = case['baseline_report']
        assert canonical_sha(baseline) == case['baseline_report_sha256'], 'Baseline changed'
        assert baseline['analyzer_source_sha256'] == data['analyzer_source_sha256']
        for side in ('ours', 'vanilla'):
            assert baseline['statistics'][side]['level']['seed'] == case['seed']
        coverage(baseline)
        # Validate exact coordinate count, not just the stated report denominator.
        for dim, bounds in case['plan']['windows'].items():
            coords = {(x, z) for x0,z0,x1,z1 in bounds
                      for x in range(x0,x1+1) for z in range(z0,z1+1)}
            assert len(coords) == EXPECTED[dim]
    return data


def category(row):
    if row['dimension'] == 'the_nether':
        return 'N'
    if row['dimension'] == 'the_end':
        return 'E'
    batch = row['batch']
    return 'R' if batch.startswith('R-') else {'F-tree': 'T', 'F-ocean': 'O'}.get(batch, batch)


def compare(before, after):
    assert before['analyzer_source_sha256'] == after['analyzer_source_sha256'], 'Analyzer changed'
    assert before['statistics']['vanilla']['manifest'] == after['statistics']['vanilla']['manifest'], 'Reference changed'
    for report in (before, after):
        assert len(report['coverage']) == 3
        for coverage in report['coverage']:
            assert coverage['complete'] and coverage['same_chunks'], 'Incomplete coverage'
            expected = {'overworld': 37249, 'the_nether': 1089, 'the_end': 2803}[coverage['dimension']]
            for side in ('ours', 'vanilla'):
                assert coverage[side]['chunks'] == coverage[side]['requested'] == expected
    key = lambda r: (r['dimension'], r['metric'], r['unit'])
    old = {key(r): r for r in before['rows']}
    new = {key(r): r for r in after['rows']}
    assert len(old) == len(before['rows']) and len(new) == len(after['rows'])
    outside = lambda r: r['verdict'] == 'outside'
    counters = collections.Counter()
    details = {'regressions': [], 'worsened_within_tolerance': [], 'new_rows': [], 'removed_rows': []}
    for k, row in new.items():
        prev = old.get(k)
        if prev is None:
            counters['new_rows'] += 1
            counters['new_outside'] += outside(row)
            details['new_rows'].append(row)
            continue
        assert abs(prev['vanilla'] - row['vanilla']) < 1e-5, ('Reference metric changed', k)
        counters['common_rows'] += 1
        counters['common_outside_before'] += outside(prev)
        counters['common_outside_after'] += outside(row)
        counters['resolved'] += outside(prev) and not outside(row)
        counters['new_failure'] += not outside(prev) and outside(row)
        delta = abs(row['ours'] - row['vanilla']) - abs(prev['ours'] - prev['vanilla'])
        if delta > max(1e-6, row['tolerance'] * 1e-4):
            field = 'regressions' if outside(row) else 'worsened_within_tolerance'
            counters[field] += 1
            details[field].append({'before': prev, 'after': row, 'absolute_error_increase': delta})
    for k in old.keys() - new.keys():
        counters['removed_rows'] += 1
        counters['removed_outside'] += outside(old[k])
        details['removed_rows'].append(old[k])
    categories = {}
    for cat in ('A', 'U', 'T', 'F', 'O', 'R', 'N', 'E'):
        categories[cat] = {name: sum(outside(r) for r in rows if category(r) == cat)
                           for name, rows in [('before', old.values()), ('after', new.values())]}
    details['regressions'].sort(key=lambda r: -(r['after']['score'] - r['before']['score']))
    return {'counts': dict(counters), 'categories': categories, 'coverage': after['coverage'], **details}



def alive(group):
    try:
        os.killpg(group, 0)
        return True
    except ProcessLookupError:
        return False


def cleanup():
    for sig, seconds in ((signal.SIGINT, 20), (signal.SIGTERM, 10), (signal.SIGKILL, 5)):
        current = [g for g in GROUPS if alive(g)]
        for group in current:
            try:
                os.killpg(group, sig)
            except ProcessLookupError:
                pass
        end = time.monotonic() + seconds
        while any(alive(g) for g in current) and time.monotonic() < end:
            try:
                while os.waitpid(-1, os.WNOHANG)[0]:
                    pass
            except ChildProcessError:
                pass
            time.sleep(.1)
    assert not any(alive(g) for g in GROUPS), 'Owned descendants remain'
    GROUPS.clear()


def run(command, log):
    with Path(log).open('w') as stream:
        child = subprocess.Popen([str(v) for v in command], cwd=REPO, stdout=stream,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        GROUPS.add(child.pid)
        status = child.wait()
        assert status == 0, f'{Path(log).name}: exit {status}'
        assert not alive(child.pid), 'Unexpected surviving descendants'
        GROUPS.remove(child.pid)


def head():
    value = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    if os.environ.get('GITHUB_SHA'):
        assert value == os.environ['GITHUB_SHA'], 'Checkout differs from workflow commit'
    return value


def bundle(folder):
    folder = Path(folder)
    names = ('mcsheriffanya', 'genaudit-starts', 'Cargo.lock')
    write(folder / 'build.json', {'head': head(), 'analyzer_source_sha256': analyzer_sha(),
          'reference_sha256': sha(REFERENCE), 'files': {n: sha(folder/n) for n in names}})


def verify_bundle(folder):
    folder = Path(folder)
    data = json.loads((folder / 'build.json').read_text())
    assert data['head'] == head(), 'Runtime belongs to another commit'
    assert data['analyzer_source_sha256'] == analyzer_sha()
    assert data['reference_sha256'] == sha(REFERENCE)
    assert set(data['files']) == {'mcsheriffanya', 'genaudit-starts', 'Cargo.lock'}
    for name, digest in data['files'].items():
        assert sha(folder / name) == digest, f'Build artifact changed: {name}'
    return data


def execute(label, build, output, scratch):
    from genaudit_263_io import level, manifest
    import genaudit_263 as analyzer
    frozen = reference()
    case = frozen['cases'][label]
    build_info = verify_bundle(build)
    output.mkdir(parents=True, exist_ok=True)
    assert not scratch.exists(), 'Use a fresh temporary output'
    assert not scratch.resolve().is_relative_to(REPO.resolve()), 'Worlds must be outside repository'
    scratch.mkdir(parents=True)
    for port, kind in ((26280, socket.SOCK_STREAM), (26281, socket.SOCK_DGRAM)):
        with socket.socket(socket.AF_INET, kind) as sock:
            sock.bind(('0.0.0.0', port))
    worlds = scratch / 'worlds'
    worlds.mkdir()
    plan_path = worlds / f'{label}-plan.json'
    write(plan_path, case['plan'])
    metadata = scratch / 'seed-metadata'
    metadata.mkdir()
    x,z = case['plan']['center']
    (metadata/'level.dat').write_text(f"seed={SEEDS['owner']}\nspawn={x} 64 {z}\n")
    # Synthetic text metadata is only the generator's initial seed/spawn input.
    # The fixed plan above determines every requested chunk for either label.
    runtime = build/'mcsheriffanya'
    helper = build/'genaudit-starts'
    runtime.chmod(0o555)
    helper.chmod(0o555)
    run([sys.executable, REPO/'tools/measure/genaudit_263_generate.py',
         '--output', worlds, '--owner-world', metadata, '--owner-seed', SEEDS['owner'],
         '--second-seed', SEEDS['second'], '--jar', '/unused-official-jar',
         '--ours', runtime, '--only', 'ours', '--label', label], output/'generation.log')
    world = worlds/f'{label}-ours/world'
    assert json.loads((world.parent/'generation-complete.json').read_text()) == case['plan']
    assert level(world)['seed'] == case['seed']
    assert canonical_sha(json.loads(plan_path.read_text())) == case['plan_sha256']
    candidates = json.loads(subprocess.check_output([str(helper),str(plan_path)], text=True))
    assert candidates['seed'] == case['seed']
    ours = analyzer.analyze(world, case['plan'], manifest(world), candidates)
    report = analyzer.comparison(ours, case['baseline_report']['statistics']['vanilla'])
    report['analyzer_source_sha256'] = analyzer_sha()
    report['eligible_start_provenance'] = candidates.get('method')
    coverage(report)
    summary = compare(case['baseline_report'], report)
    write(output/f'{label}.json', report)
    (output/f'{label}.md').write_text(analyzer.markdown(report))
    write(output/f'{label}-summary.json', summary)
    write(output/'provenance.json', {'head': head(), 'label': label, 'seed': case['seed'],
          'pairs': sum(EXPECTED.values()), 'reference_sha256': sha(REFERENCE),
          'plan_sha256': case['plan_sha256'], 'baseline_report_sha256': case['baseline_report_sha256'],
          'report_sha256': sha(output/f'{label}.json'), 'build': build_info})
    (output/'summary.md').write_text(f"# {label}: {sum(EXPECTED.values())} пар чанков\n\n" +
        'Новые и исчезнувшие строки учитываются отдельно; это не исправления.\n\n```json\n' +
        json.dumps(summary['counts'],ensure_ascii=False,indent=2) + '\n```\n')
    print(json.dumps(summary['counts']), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build', type=Path, required=True)
    parser.add_argument('--bundle', action='store_true')
    parser.add_argument('--label', choices=SEEDS)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--scratch', type=Path)
    args = parser.parse_args()
    reference()
    if args.bundle:
        bundle(args.build)
        return
    assert args.label and args.output and args.scratch
    assert ctypes.CDLL(None).prctl(36,1,0,0,0) == 0  # Linux child subreaper.
    for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP):
        signal.signal(sig, lambda n,f: (_ for _ in ()).throw(InterruptedError(f'Signal {n}')))
    try:
        execute(args.label, args.build.resolve(), args.output.resolve(), args.scratch.resolve())
    finally:
        for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP):
            signal.signal(sig,signal.SIG_IGN)
        cleanup()


if __name__ == '__main__':
    main()
