"""Export every outside-tolerance row from completed census reports, without worlds.

Usage: python genaudit_263_catalog.py AUDIT/reports --output discrepancies.json
Rows retain the report's ranked order within each independently sampled case.
Metric names and units are interned to keep the regression reference small.
"""
import argparse
import hashlib
import json
from pathlib import Path


def export(root):
    catalog = {}
    cases = []
    hashes = set()
    missing = []
    inputs = [('core', root / (name + '.json')) for name in ('owner', 'second')]
    for group in ('targets', 'camps'):
        for item in json.loads((root / group / 'index.json').read_text()):
            if item['status'] == 'measured':
                # Resolve against the supplied root, even after moving the archive.
                inputs.append((group, root / group / Path(item['report']).name))
            elif item['status'] == 'not located by official server':
                missing.append([group, item['category'], item['name']])
            else:
                raise ValueError(f'Unfinished case: {item}')
    for group, path in inputs:
        report = json.loads(path.read_text())
        if report['ours_level']['seed'] != report['vanilla_level']['seed']:
            raise ValueError(f'Different seeds: {path}')
        coverage = []
        for c in report['coverage']:
            if not c['complete'] or not c['same_chunks']:
                raise ValueError(f'Incomplete coverage: {path}')
            v = c['ours']
            coverage.append([c['dimension'], v['chunks'], v.get('chunk_sha256'), v.get('bounds_chunks')])
        hashes.add(report['analyzer_source_sha256'])
        rows = []
        for r in report['rows']:
            if r['verdict'] != 'outside':
                continue
            key = (r['dimension'], r['metric'], r['unit'])
            catalog.setdefault(key, len(catalog))
            rows.append([catalog[key], r['ours'], r['vanilla'], r['tolerance']])
        cases.append({'report': group + '/' + path.name,
                      'report_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                      'seed': report['ours_level']['seed'], 'coverage': coverage, 'rows': rows})
    if len(hashes) != 1:
        raise ValueError('Reports come from different analyzer versions')
    return {'schema': 1, 'analyzer_source_sha256': next(iter(hashes)),
            'metric_columns': ['dimension', 'metric', 'unit'],
            'row_columns': ['metric_index', 'ours', 'vanilla', 'tolerance'],
            'coverage_columns': ['dimension', 'paired_chunks', 'chunk_sha256', 'bounds_chunks'],
            'verdict': 'outside; abs(ours-vanilla)>tolerance; correlated rows are not independent bugs',
            'order': 'cases separate; rows ranked by original report score; see full report for coordinates, denominators and limitations',
            'not_located': missing, 'metrics': list(catalog), 'cases': cases}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('reports', type=Path)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    data = export(args.reports)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(data, ensure_ascii=False, allow_nan=False, separators=(',', ':')) + '\n')
    print(f"{len(data['cases'])} cases; {sum(len(c['rows']) for c in data['cases'])} outside rows; {args.output.stat().st_size} bytes")


if __name__ == '__main__':
    main()
