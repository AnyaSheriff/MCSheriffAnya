"""Диагностика скачков Энда по сохранённым мирам; только чтение.

Пример: python tools/measure/genaudit_263_end_edges.py WORLD --plan PLAN --output OUT
main/outer/gap определяются радиусом середины ребра: <=160, >=945, остальное.
Все пары включают пустоту (-1), но исключают отсутствующие чанки.
"""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import numpy as np


def diagnose(chunks):
    """Высоты [z,x], 16×16; каждое соседнее ребро учитывается ровно один раз."""
    rows = defaultdict(lambda: [0, 0])
    for heights in chunks.values():
        h = np.asarray(heights)
        if h.shape != (16, 16) or not np.issubdtype(h.dtype, np.integer) or (h < -1).any():
            raise ValueError('Ожидаются целые высоты 16×16, пустота=-1')
    for (cx, cz), heights in sorted(chunks.items()):
        h = np.asarray(heights)
        for axis, neighbor in [(1, (cx+1, cz)), (0, (cx, cz+1))]:
            if axis == 1:
                pairs = [(h[:, :-1], h[:, 1:], False)]
                if neighbor in chunks:
                    pairs.append((h[:, -1:], np.asarray(chunks[neighbor])[:, :1], True))
            else:
                pairs = [(h[:-1, :], h[1:, :], False)]
                if neighbor in chunks:
                    pairs.append((h[-1:, :], np.asarray(chunks[neighbor])[:1, :], True))
            for a, b, border in pairs:
                zz, xx = np.indices(a.shape)
                if border:
                    if axis == 1: xx += 15
                    else: zz += 15
                radius = np.hypot(cx*16+xx+(0.5 if axis == 1 else 0),
                                  cz*16+zz+(0.5 if axis == 0 else 0))
                zones = {'all': np.ones(a.shape, bool), 'main': radius <= 160,
                         'outer': radius >= 945, 'gap': (radius > 160) & (radius < 945)}
                kinds = {'all': np.ones(a.shape, bool), 'land_void': (a == -1) ^ (b == -1),
                         'land_land': (a >= 0) & (b >= 0), 'void_void': (a == -1) & (b == -1)}
                jumps = np.abs(a.astype(np.int64)-b.astype(np.int64)) >= 16
                for zone, mask in zones.items():
                    for kind, selection in kinds.items():
                        selected = mask & selection
                        row = rows[(zone, 'border' if border else 'interior', kind)]
                        row[0] += int(selected.sum())
                        row[1] += int((selected & jumps).sum())
    return [{'zone': z, 'location': loc, 'kind': k, 'denominator': n,
             'jump_ge16_count': count, 'share': count/n if n else None}
            for (z, loc, k), (n, count) in sorted(rows.items())]


def load_world(world, plan=None):
    # Те же Reader и определение natural, что у замороженного анализатора.
    from genaudit_263 import natural
    from genaudit_263_io import Reader, region_dir
    wanted = None if plan is None else {(x,z) for x0,z0,x1,z1 in plan['windows']['the_end']
                                       for x in range(x0,x1+1) for z in range(z0,z1+1)}
    reader = Reader()
    chunks, sources = {}, []
    folder = region_dir(world, 'the_end')
    files = sorted(list(folder.glob('*.mcs')) + list(folder.glob('*.mca')))
    if not files:
        raise ValueError('Нет сохранённых регионов Энда: ' + str(folder))
    for file in files:
        sources.append({'path': str(file), 'sha256': hashlib.sha256(file.read_bytes()).hexdigest()})
        for cx, cz, blocks, _, _ in reader.read(file, wanted):
            if (cx,cz) in chunks:
                raise ValueError('Повтор чанка')
            ground = np.asarray([natural(n) for n in reader.names])[blocks]
            ground[:64] = False
            ground[320:] = False
            chunks[cx,cz] = np.where(ground.any(axis=0), 383-ground[::-1].argmax(axis=0)-64, -1)
    return {'schema': 1, 'threshold_blocks': 16,
            'height_definition': 'Как frozen analyzer: highest natural substrate в Y=0..255; void=-1',
            'zone_definition': 'Радиус середины ребра: main<=160, outer>=945, gap между ними; координатные окна задаёт plan',
            'share_definition': 'jump_ge16_count/denominator внутри данной строки; не доля от всех рёбер',
            'plan_sha256': hashlib.sha256(json.dumps(plan, sort_keys=True).encode()).hexdigest() if plan is not None else None,
            'world': str(world), 'chunks': len(chunks), 'requested': len(wanted) if wanted is not None else None,
            'missing_chunks': sorted(wanted-chunks.keys()) if wanted is not None else None,
            'sources': sources, 'rows': diagnose(chunks)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world', type=Path)
    parser.add_argument('--plan', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    plan = json.loads(args.plan.read_text()) if args.plan else None
    result = load_world(args.world, plan)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')


if __name__ == '__main__':
    main()
