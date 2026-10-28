"""Чистая проверка процедурной гипотезы слоёв End; не формула оригинала."""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path


def canonical_sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def layer_mask(q):
    """q=2R, целочисленная гипотеза ceil(R) и d²<=(R+1)²."""
    if not isinstance(q, int) or not 1 <= q <= 12:
        raise ValueError('q должен быть целым1..12')
    extent = (q+1)//2
    return frozenset((x, z) for x in range(-extent, extent+1) for z in range(-extent, extent+1)
                     if 4*(x*x+z*z) <= (q+2)*(q+2))


def check_shape(cells, hypothesis):
    """Все совместимые пути q, без выбора удобного состояния или правила stop."""
    actual = [tuple(c) for c in cells]
    errors = []
    if not actual or any(len(c) != 3 or any(type(v) is not int for v in c) for c in actual):
        return {'passed': False, 'errors': ['Нет корректных целочисленных клеток'], 'q_paths': []}
    if len(actual) != len(set(actual)):
        errors.append('Повтор клетки')
    columns, layers = defaultdict(set), defaultdict(set)
    for x, y, z in actual:
        columns[x,z].add(y)
        layers[y].add((x,z))
    if max(layers) != 0:
        errors.append('Наблюдённый верх не совпал с заданнымtop0')
    if any(ys != set(range(min(ys),1)) for ys in columns.values()):
        errors.append('Разрыв столбца или неплоский верх')
    layer_ys = sorted(layers, reverse=True)
    if layer_ys != list(range(0, min(layers)-1, -1)):
        errors.append('Пропущен вертикальный слой')
    q_values = hypothesis['q_values']
    states = [[q for q in q_values if layers[y] == layer_mask(q)] for y in layer_ys]
    if any(not values for values in states):
        errors.append('Есть слой вне замороженного семейства масок')
    paths = [[q] for q in (states[0] if states else []) if q in hypothesis['initial_q']]
    for allowed in states[1:]:
        paths = [path+[q] for path in paths for q in allowed if path[-1]-q in hypothesis['visible_decrements_q']]
    if not paths:
        errors.append('Нет совместимой последовательности q')
    return {'passed': not errors, 'errors': errors, 'volume': len(actual), 'layers': len(layers),
            'layer_y': layer_ys, 'layer_q_candidates': states, 'q_paths': paths,
            'stop_rule_tested': False, 'relative_cells_sha256': canonical_sha(sorted(set(actual)))}


def validate_dataset(dataset, hypothesis):
    observations = dataset['observations']
    identifiers = [o['plot_index'] for o in observations]
    expected = hypothesis['expected_plots_per_series']
    complete = sorted(identifiers) == list(range(expected))
    reports = [{'plot_index': o['plot_index'], **check_shape(o['cells'], hypothesis)} for o in observations]
    return {'schema': 1, 'hypothesis_sha256_canonical': canonical_sha(hypothesis),
            'dataset_sha256_canonical': canonical_sha(dataset), 'expected_plots': expected,
            'measured_plots': len(observations), 'complete': complete,
            'passed': complete and all(r['passed'] for r in reports),
            'failed_plots': [r['plot_index'] for r in reports if not r['passed']],
            'observed_layers': sum(r.get('layers',0) for r in reports), 'plots': reports,
            'limits': ['Проверены клетки и существование видимой последовательности, а не внутренний RNG оригинала.',
                       'Правило остановки, вероятности шагов и природное размещение не проверены.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--hypothesis', type=Path, required=True)
    parser.add_argument('--observations', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = validate_dataset(json.loads(args.observations.read_text()), json.loads(args.hypothesis.read_text()))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    if not result['passed']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
