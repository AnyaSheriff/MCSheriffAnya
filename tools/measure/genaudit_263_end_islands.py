"""Read-only детектор изолированных маленьких островков в сохранённом Энде.

WORLD --bounds CX0 CZ0 CX1 CZ1 [--bounds ...] --output REPORT.json
Опционально --observed-bank JSON (наблюдённые cells либо модельные columns).
Без --include-cells сохраняются только признаки, SHA клеток и сводки.
Компонента Endstone не доказывает, что это отдельная feature оригинала.
"""
import argparse
from collections import Counter, defaultdict
from dataclasses import dataclass
import hashlib
import json
from numbers import Integral
from pathlib import Path
import time

import numpy as np
from scipy import ndimage

METHOD = '4-связные 2Dкомпоненты Endstoneколонок; bbox<=13×13; верхY55..70; полные3Dклетки только кандидатов'
LIMITATIONS = [
    'Компонента не гарантирует feature: мелкий фрагмент шумового рельефа может пройти фильтр.',
    'Соединённые с большими островами, наложенные поXY и обрезанные формы исключены; это не частота всех feature.',
    'Биом центра чанка наY64 — proxy eligibility; правило оригинала не доказано.',
    'Числитель и знаменатель rates используют один chunkcenterbiomeY64; нужны целый чанк и одночанковый ореол внутри окна.',
    'Связность4соседа: диагональные контакты рассматриваются отдельно.',
    'Точное совпадение с ограниченным банком наблюдений не доказывает полноту банка или естественное распределение.',
]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def canonical_sha(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()


def coordinate_cells(cells):
    result = []
    for cell in cells:
        require(len(cell) == 3 and all(isinstance(v, Integral) for v in cell), 'Клетка должна содержать3целых координаты')
        result.append(tuple(int(v) for v in cell))
    require(len(result) == len(set(result)), 'Повтор клетки')
    require(bool(result), 'Пустая геометрия')
    return frozenset(result)


@dataclass
class ShapeBank:
    exact: dict
    top_masks: dict
    metadata: dict


def bank_from_data(data):
    """Чистое чтение собственных observations/cells либо models/columns."""
    require(data.get('schema') == 1, 'Неизвестная схема банка')
    observations = data.get('observations', data.get('models'))
    require(isinstance(observations, list) and bool(observations), 'Нет наблюдённых форм')
    exact, masks = {}, {}
    for index, observation in enumerate(observations):
        if 'cells' in observation:
            cells = coordinate_cells(observation['cells'])
        else:
            columns = observation.get('columns', [])
            require(bool(columns), 'Нет столбцов модели')
            generated = []
            for col in columns:
                require(len(col) == 4 and all(isinstance(v, Integral) for v in col), 'Неверный столбец банка')
                x, z, bottom, top = col
                require(-6 <= bottom <= top == 0, 'Столбец вне наблюдённых высот')
                generated.extend((x, y, z) for y in range(bottom, top + 1))
            cells = coordinate_cells(generated)
        radius = max(max(abs(x), abs(z)) for x, _, z in cells)
        require(radius in (4, 5, 6) and observation['radius'] == radius, 'Неверный класс банка')
        columns = defaultdict(set)
        for x, y, z in cells:
            columns[x, z].add(y)
        require(all(ys == set(range(min(ys), 1)) and min(ys) >= -6 for ys in columns.values()), 'Банк не имеет непрерывных столбцов доtop0')
        mask = frozenset((x, z) for x, y, z in cells if y == 0)
        require(radius not in masks or masks[radius] == mask, 'Противоречивые верхние маски класса')
        masks[radius] = mask
        exact.setdefault(cells, []).append(observation.get('plot_index', index))
    return ShapeBank(exact, masks, {'models': len(observations), 'distinct_forms': len(exact),
                                   'canonical_data_sha256': canonical_sha(data)})


def load_bank(path):
    path = Path(path)
    raw = path.read_bytes()
    bank = bank_from_data(json.loads(raw))
    bank.metadata.update(path=str(path.resolve()), sha256=hashlib.sha256(raw).hexdigest())
    return bank


def checked_bounds(bounds):
    require(len(bounds) == 4 and all(isinstance(v, Integral) for v in bounds), 'Нужны4целых границы чанков')
    x0, z0, x1, z1 = map(int, bounds)
    require(x0 <= x1 and z0 <= z1, 'Обратные границы окна')
    return x0, z0, x1, z1


def component_candidates(top, bottom, biomes, bounds):
    """Чистая классификация2D; нет чтения файлов и подмены отсутствующих чанков."""
    x0, z0, x1, z1 = checked_bounds(bounds)
    shape = ((z1-z0+1)*16, (x1-x0+1)*16)
    top, bottom, biomes = np.asarray(top), np.asarray(bottom), np.asarray(biomes)
    require(top.shape == bottom.shape == biomes.shape == shape, 'Размер карт не соответствует окну')
    require(np.issubdtype(top.dtype, np.integer) and np.issubdtype(bottom.dtype, np.integer), 'Высоты должны быть целыми')
    require(bool((bottom[top >= 0] <= top[top >= 0]).all()), 'Низ выше верха')
    labels, count = ndimage.label(top >= 0)
    candidates, excluded = [], Counter()
    for ident, slices in enumerate(ndimage.find_objects(labels), 1):
        if slices is None:
            continue
        sz, sx = slices
        width, height = sx.stop-sx.start, sz.stop-sz.start
        if sx.start == 0 or sz.start == 0 or sx.stop == shape[1] or sz.stop == shape[0]:
            excluded['edge_clipped'] += 1
            continue
        if width > 13 or height > 13:
            excluded['large_or_connected'] += 1
            continue
        selected = labels[slices] == ident
        tops = top[slices][selected]
        if not 55 <= tops.max() <= 70:
            excluded['top_outside55_70'] += 1
            continue
        xx, zz = (sx.start+sx.stop-1)//2, (sz.start+sz.stop-1)//2
        wx, wz = x0*16+xx, z0*16+zz
        candidates.append({'component_id': ident,
            'bounds': [x0*16+sx.start, z0*16+sz.start, x0*16+sx.stop-1, z0*16+sz.stop-1],
            'center_proxy': [wx, int(tops.max()), wz], 'bbox_size': [width, height],
            'columns': int(selected.sum()), 'top_range': [int(tops.min()), int(tops.max())],
            'bottom_range': [int(bottom[slices][selected].min()), int(bottom[slices][selected].max())],
            'center_biome_y64': str(biomes[zz, xx]),
            'center_chunk_center_biome_y64': str(biomes[(wz//16-z0)*16+8, (wx//16-x0)*16+8]),
            'center_chunk_has_full_halo': x0 < wx//16 < x1 and z0 < wz//16 < z1})
    eligible = Counter(str(biomes[(cz-z0)*16+8, (cx-x0)*16+8])
                       for cx in range(x0+1, x1) for cz in range(z0+1, z1))
    return {'labels': labels, 'components': int(count), 'candidates': candidates,
            'excluded_components': dict(excluded), 'eligible_proxy_halo_chunks_by_center_biome_y64': dict(eligible)}


def classify_cells(cells, center, bank=None, include_cells=False):
    """Чистая проверка полной3Dформы, включая разрывы в столбцах."""
    actual = coordinate_cells(cells)
    require(len(center) == 3 and all(isinstance(v, Integral) for v in center), 'Неверный центр')
    cx, cy, cz = center
    relative = frozenset((x-cx, y-cy, z-cz) for x, y, z in actual)
    require(max(y for _, y, _ in relative) == 0, 'ЦентрY должен совпадать с максимальным верхом')
    columns = defaultdict(set)
    for x, y, z in relative:
        columns[x, z].add(y)
    continuous = all(ys == set(range(min(ys), 1)) for ys in columns.values())
    flat = all(max(ys) == 0 for ys in columns.values())
    radius = max(max(abs(x), abs(z)) for x, _, z in relative)
    top_mask = frozenset((x, z) for x, y, z in relative if y == 0)
    mask_match = top_mask == bank.top_masks.get(radius) if bank is not None else None
    property_match = continuous and flat and mask_match if bank is not None else None
    exact = bank.exact.get(relative, []) if bank is not None else None
    result = {'volume': len(actual), 'relative_layer_counts': dict(Counter(y for _, y, _ in relative)),
              'all_columns_contiguous_to_top': continuous, 'flat_top': flat, 'radius_proxy': radius,
              'top_mask_matches_placed_class': mask_match, 'matches_observed_shape_properties': property_match,
              'exact_placed_model_plots': exact, 'exact_placed_model_match': bool(exact) if exact is not None else None,
              'relative_cells_sha256': canonical_sha(sorted(relative))}
    if include_cells:
        result.update(cells=sorted(actual), relative_cells=sorted(relative))
    return result


def proxy_rates(candidates, eligible, comparison_available=None):
    """Тот же биом центра ЧАНКА в числителе и знаменателе; не featureчастота."""
    counts, matched, distinct = Counter(), Counter(), defaultdict(set)
    has_comparison = (bool(candidates) and all(c['matches_observed_shape_properties'] is not None for c in candidates)) if comparison_available is None else comparison_available
    for candidate in candidates:
        if not candidate['center_chunk_has_full_halo']:
            continue
        biome = candidate['center_chunk_center_biome_y64']
        require(eligible.get(biome, 0) > 0, 'Кандидат с ореолом не входит в знаменатель')
        counts[biome] += 1
        if candidate['matches_observed_shape_properties']:
            matched[biome] += 1
            x, _, z = candidate['center_proxy']
            distinct[biome].add((x//16, z//16))
    return [{'center_chunk_center_biome_y64': b, 'chunk_denominator': n,
             'isolated_candidate_count': counts[b],
             'shape_property_candidate_count': matched[b] if has_comparison else None,
             'distinct_center_chunks_with_property_candidate': len(distinct[b]) if has_comparison else None,
             'property_candidates_per_chunk': matched[b]/n if n and has_comparison else None}
            for b, n in sorted(eligible.items())]


def source_files(folder, wanted):
    files = []
    for rx, rz in sorted({(x//32, z//32) for x, z in wanted}):
        for suffix in ('mca', 'mcs'):
            path = folder / f'r.{rx}.{rz}.{suffix}'
            if path.is_file():
                files.append(path)
    return files


def window_summary(candidates, comparison_available):
    matching = [c for c in candidates if c['matches_observed_shape_properties']]
    return {'candidates': len(candidates),
            'exact_model_matches': sum(c['exact_placed_model_match'] is True for c in candidates) if comparison_available else None,
            'property_matches': len(matching) if comparison_available else None,
            'distinct_property_shapes': len({c['relative_cells_sha256'] for c in matching}) if comparison_available else None,
            'radius_histogram': dict(Counter(c['radius_proxy'] for c in candidates)),
            'property_radius_histogram': dict(Counter(c['radius_proxy'] for c in matching)) if comparison_available else None,
            'candidate_center_biomes_y64': dict(Counter(c['center_biome_y64'] for c in candidates))}


def analyze_window(world, bounds, reader, bank=None, include_cells=False):
    from genaudit_263_io import region_dir
    bounds = checked_bounds(bounds)
    x0, z0, x1, z1 = bounds
    shape = ((z1-z0+1)*16, (x1-x0+1)*16)
    top, bottom = np.full(shape, -1, np.int16), np.full(shape, -1, np.int16)
    biomes = np.full(shape, 'unknown', dtype=object)
    wanted = {(x, z) for x in range(x0, x1+1) for z in range(z0, z1+1)}
    folder = region_dir(world, 'the_end')
    sources, seen = [], set()
    for file in source_files(folder, wanted):
        raw = file.read_bytes()
        sources.append({'path': str(file.resolve()), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()})
        for cx, cz, blocks, bios, _ in reader.read(file, wanted):
            require((cx, cz) not in seen, 'Повтор сохранённого чанка')
            seen.add((cx, cz))
            mask = blocks == reader.lookup['end_stone']
            exists = mask.any(0)
            z, x = (cz-z0)*16, (cx-x0)*16
            top[z:z+16, x:x+16] = np.where(exists, 383-mask[::-1].argmax(0)-64, -1)
            bottom[z:z+16, x:x+16] = np.where(exists, mask.argmax(0)-64, -1)
            quart_biomes = np.asarray(bios).reshape(96, 4, 4)[32]
            biomes[z:z+16, x:x+16] = np.repeat(np.repeat(quart_biomes, 4, 0), 4, 1)
    require(seen == wanted, f'Неполное покрытие: {len(seen)}/{len(wanted)}, первые пропуски {sorted(wanted-seen)[:20]}')
    components = component_candidates(top, bottom, biomes, bounds)
    labels, candidates = components.pop('labels'), components['candidates']
    needed = {(x//16, z//16) for c in candidates
              for x in range(c['bounds'][0], c['bounds'][2]+1) for z in range(c['bounds'][1], c['bounds'][3]+1)}
    ids, cells, reread = {c['component_id'] for c in candidates}, defaultdict(list), set()
    for file in source_files(folder, needed):
        for cx, cz, blocks, _, _ in reader.read(file, needed):
            require((cx, cz) not in reread, 'Повтор чанка во втором чтении')
            reread.add((cx, cz))
            for yi, lz, lx in np.argwhere(blocks == reader.lookup['end_stone']):
                ident = int(labels[(cz-z0)*16+int(lz), (cx-x0)*16+int(lx)])
                if ident in ids:
                    cells[ident].append((cx*16+int(lx), int(yi)-64, cz*16+int(lz)))
    require(reread == needed, 'Потерян чанк во втором чтении')
    for candidate in candidates:
        candidate.update(classify_cells(cells[candidate['component_id']], candidate['center_proxy'], bank, include_cells))
    # Нельзя подписывать отчёт хешем файлов, изменившихся во время измерения.
    for source in sources:
        require(hashlib.sha256(Path(source['path']).read_bytes()).hexdigest() == source['sha256'], 'Мир изменён во время чтения')
    return {'bounds_chunks': list(bounds), 'chunks': len(seen), 'requested_chunks': len(wanted), 'missing_chunks': [],
            'chunk_sha256': canonical_sha(sorted(seen)), 'sources': sources,
            'second_read_candidate_chunks': len(needed), **components,
            'summary': window_summary(candidates, bank is not None),
            'censored_proxy_rates': proxy_rates(candidates, components['eligible_proxy_halo_chunks_by_center_biome_y64'], bank is not None)}


def analyze_world(world, bounds, observed_bank=None, include_cells=False):
    """Публичный API для локального read-only и A/E CI; не запускает процессы."""
    from genaudit_263_io import Reader, level
    start = time.process_time()
    world = Path(world)
    windows = [checked_bounds(window) for window in bounds]
    require(bool(windows), 'Нет окон')
    for i, (x0, z0, x1, z1) in enumerate(windows):
        for a0, b0, a1, b1 in windows[:i]:
            require(x1 < a0 or a1 < x0 or z1 < b0 or b1 < z0, 'Перекрывающиеся окна удваивают наблюдения')
    bank = load_bank(observed_bank) if observed_bank is not None else None
    reader = Reader()
    entries = [analyze_window(world, b, reader, bank, include_cells) for b in windows]
    candidates = [c for entry in entries for c in entry['candidates']]
    total = sum(w['chunks'] for w in entries)
    return {'schema': 1, 'world': str(world.resolve()), 'level': level(world), 'method': METHOD,
            'requested_chunks': total, 'chunks': total, 'coverage': {'expected':total,'actual':total,'complete':True},
            'limitations': LIMITATIONS, 'bank': bank.metadata if bank is not None else None,
            'raw_cells_included': include_cells, 'windows': entries,
            'summary': {'chunks': sum(w['chunks'] for w in entries), **window_summary(candidates, bank is not None)},
            'cpu_seconds': round(time.process_time()-start, 3),
            'detector_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world', type=Path)
    parser.add_argument('--bounds', type=int, nargs=4, action='append', required=True)
    parser.add_argument('--observed-bank', type=Path)
    parser.add_argument('--include-cells', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = analyze_world(args.world, args.bounds, args.observed_bank, args.include_cells)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps(result['summary'], ensure_ascii=False))


if __name__ == '__main__':
    main()
