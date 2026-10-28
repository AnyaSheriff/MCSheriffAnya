"""Опыты Энда 2×2 и измеренной формы в CI; эталон и анализатор неизменны."""
import argparse
import copy
import ctypes
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import socket
import signal
import struct

import numpy as np
import sys

import genaudit_263_ci as ci

REPO = Path(__file__).resolve().parents[2]
SOURCE = Path('src/world/the_end.rs')
CONTROL = REPO / 'testdata/world-profile/genaudit-26.3/end-control-pass3.json.gz'
VARIANTS = ('A', 'B', 'C', 'D', 'E')
MODEL_INPUTS = (Path('src/world/end_island_shapes_26_3.rs'),
                Path('testdata/world-profile/genaudit-26.3/end-island-shapes-26.3.json'),
                Path('testdata/world-profile/genaudit-26.3/end-island-shapes-observations-26.3.json'))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def baseline():
    return subprocess.check_output(['git', 'show', 'HEAD:' + str(SOURCE)], cwd=REPO)


def variant_source(original, variant):
    require(variant in VARIANTS, 'Неизвестный вариант')
    text = original.decode()
    if variant in ('C', 'D'):
        old = 'const ISLETS: f64 = 0.225;'
        require(text.count(old) == 1, 'Изменился исходный вес шума')
        text = text.replace(old, 'const ISLETS: f64 = 0.3;')
        old = 'self.islands.octaves(fx / 190.0 + 0.37, fz / 190.0 + 0.71, 3) * (28.0 / 31.0) + ISLETS * self.islets.octaves(fx / 55.0 + 0.19, fz / 55.0 + 0.83, 1)'
        new = 'self.islands.octaves(fx / 190.0 + 0.37, fz / 190.0 + 0.71, 5) + ISLETS * self.islets.octaves(fx / 55.0 + 0.19, fz / 55.0 + 0.83, 2)'
        require(text.count(old) == 1, 'Изменилась исходная формула шума')
        text = text.replace(old, new)
    if variant in ('B', 'D'):
        start = '    fn small_island_columns(&self, x: i32, z: i32) -> Vec<(i32, i32)> {'
        end = '\n    fn main_column('
        require(text.count(start) == text.count(end) == 1, 'Изменилась функция мелких островков')
        before, tail = text.split(start)
        _, after = tail.split(end)
        text = before + '    fn small_island_columns(&self, _x: i32, _z: i32) -> Vec<(i32, i32)> {\n        Vec::new()\n    }\n' + end + after
    if variant == 'E':
        old = '''                    let d2 = (x - ix).pow(2) + (z - iz).pow(2);
                    if d2 > radius * radius {
                        continue;
                    }
                    let mut bottom = top;
                    for layer in 1..radius {
                        if d2 <= (radius - layer).pow(2) {
                            bottom = top - layer;
                        }
                    }
                    result.push((bottom, top));'''
        new = '''                    // Отдельный поток формы не сдвигает размещение второго островка.
                    let model = self.dice(ix, iz).below(observed_islands::model_count(radius) as i32) as u32;
                    if let Some((bottom, observed_top)) = observed_islands::column(radius, model, x - ix, z - iz) {
                        result.push((top + bottom, top + observed_top));
                    }'''
        require(text.count(old) == 1, 'Изменилась исходная геометрия островков')
        text = '#[path = "end_island_shapes_26_3.rs"]\nmod observed_islands;\n\n' + text.replace(old, new)
    return text.encode()


def source_provenance(variant):
    original = baseline()
    patched = variant_source(original, variant)
    inputs = {}
    if variant == 'E':
        for path in MODEL_INPUTS:
            committed = subprocess.check_output(['git', 'show', 'HEAD:' + str(path)], cwd=REPO)
            require((REPO/path).read_bytes() == committed, 'Изменён или не добавлен в commit вход модели: ' + str(path))
            inputs[str(path)] = digest(committed)
    return {'base_head': ci.head(), 'variant': variant, 'base_source_sha256': digest(original),
            'source_sha256': digest(patched),
            'model_inputs_sha256': inputs,
            # Канонический патч описывает точную замену одного файла, включая A.
            'patch_sha256': ci.canonical_sha({'path': str(SOURCE), 'before': digest(original), 'after': digest(patched)})}


def prepare(variant):
    original = baseline()
    require((REPO / SOURCE).read_bytes() == original, 'Рабочий End уже изменён')
    source_provenance(variant)  # Модель E должна быть committed и неизменной ещё до Cargo.
    (REPO / SOURCE).write_bytes(variant_source(original, variant))


def bundle(folder, variant):
    folder = Path(folder)
    data = source_provenance(variant)
    require(digest((REPO / SOURCE).read_bytes()) == data['source_sha256'], 'Собирается другой исходник End')
    changed = subprocess.check_output(['git','diff','--name-only','HEAD'], cwd=REPO, text=True).splitlines()
    require(set(changed) <= {str(SOURCE)}, 'Другие tracked исходники изменены при сборке')
    names = ('mcsheriffanya', 'genaudit-starts', 'Cargo.lock')
    data.update({'analyzer_source_sha256': ci.analyzer_sha(), 'reference_sha256': ci.sha(ci.REFERENCE),
                 'control_sha256': ci.sha(CONTROL), 'files': {n: ci.sha(folder/n) for n in names}})
    ci.write(folder/'end-build.json', data)
    return data


def verify_bundle(folder, variant):
    folder = Path(folder)
    data = json.loads((folder/'end-build.json').read_text())
    for key, value in source_provenance(variant).items():
        require(data.get(key) == value, 'Другой вариант/исходник/commit: ' + key)
    for key, value in [('analyzer_source_sha256', ci.analyzer_sha()),
                       ('reference_sha256', ci.sha(ci.REFERENCE)), ('control_sha256', ci.sha(CONTROL))]:
        require(data.get(key) == value, 'Изменён вход: ' + key)
    require(set(data['files']) == {'mcsheriffanya', 'genaudit-starts', 'Cargo.lock'}, 'Неполный комплект')
    for name, value in data['files'].items():
        require(ci.sha(folder/name) == value, 'Повреждён артефакт: ' + name)
    return data


def end_plan(plan):
    result = copy.deepcopy(plan)
    result['windows'] = {d: result['windows'][d] if d == 'the_end' else [] for d in ci.EXPECTED}
    coords = {(x,z) for x0,z0,x1,z1 in result['windows']['the_end']
              for x in range(x0,x1+1) for z in range(z0,z1+1)}
    require(len(coords) == 2803, 'План Энда неполный')
    return result


def coverage(report):
    require(bool(report['rows']), 'Пустой набор метрик')
    rows = report['coverage']
    require(len(rows) == 1 and rows[0]['dimension'] == 'the_end', 'Требуется только Энд')
    row = rows[0]
    require(row['complete'] and row['same_chunks'], 'Неполное/разное покрытие')
    for side in ('ours', 'vanilla'):
        item = row[side]
        require(item['chunks'] == item['requested'] == 2803, 'Не все2803чанка')
        require(not item.get('missing_first20'), 'Пропущены чанки')
    require(row['ours']['chunk_sha256'] == row['vanilla']['chunk_sha256'], 'Разные координаты')
    require(all(r['dimension'] == 'the_end' for r in report['rows']), 'Чужое измерение в строках')


def control(label):
    meta = json.loads(CONTROL.with_suffix('').read_text())
    require(meta['fixture_sha256'] == ci.sha(CONTROL), 'Повреждён контроль A')
    require(meta['bytes'] == CONTROL.stat().st_size, 'Другой размер контроля')
    data = json.loads(gzip.decompress(CONTROL.read_bytes()))
    require(data['schema'] == 1 and data['analyzer_source_sha256'] == ci.analyzer_sha(), 'Другой анализатор контроля')
    require(set(data['cases']) == set(ci.SEEDS), 'Неполный набор семян контроля')
    case = data['cases'][label]
    require(ci.canonical_sha(case) == meta['cases'][label]['case_sha256_canonical'], 'Изменён case контроля')
    require(case['seed'] == ci.SEEDS[label], 'Другое семя контроля')
    require(case['analyzer_source_sha256'] == ci.analyzer_sha(), 'Другой анализатор case')
    coverage(case)
    for side in ('ours', 'vanilla'):
        require(set(case['statistics'][side]['dimensions']) == {'the_end'}, 'Чужое измерение контроля')
        require(case['statistics'][side]['level']['seed'] == case['seed'], 'Семя статистики контроля')
    return case


def compare_rows(before, after):
    key = lambda r: (r['dimension'], r['metric'], r['unit'])
    a, b = {key(r): r for r in before}, {key(r): r for r in after}
    require(len(a) == len(before) and len(b) == len(after), 'Повтор метрики')
    common = a.keys() & b.keys()
    require(all(a[k]['vanilla'] == b[k]['vanilla'] and a[k]['tolerance'] == b[k]['tolerance'] for k in common), 'Изменился эталон/допуск')
    return {'new_rows': [b[k] for k in sorted(b.keys()-a.keys())],
            'removed_rows': [a[k] for k in sorted(a.keys()-b.keys())],
            'new_failure': [b[k] for k in sorted(common) if a[k]['verdict'] != 'outside' and b[k]['verdict'] == 'outside'],
            'resolved': [b[k] for k in sorted(common) if a[k]['verdict'] == 'outside' and b[k]['verdict'] != 'outside'],
            'worsened': [{'before': a[k], 'after': b[k]} for k in sorted(common)
                        if abs(b[k]['ours']-b[k]['vanilla']) > abs(a[k]['ours']-a[k]['vanilla']) + 1e-6]}


def control_validation(expected, report, variant):
    """Отказ контроля A — результат измерения, который надо сохранить."""
    if variant != 'A':
        return {'required':False,'passed':None,'reason':'Факторный вариант; точное совпадение A не требуется'}
    key = lambda r: (r['dimension'],r['metric'],r['unit'])
    old = {key(r):r for r in expected['rows']}
    new = {key(r):r for r in report['rows']}
    if old.keys() != new.keys() or len(old) != len(expected['rows']) or len(new) != len(report['rows']):
        return {'required':True,'passed':False,'reason':'A изменил набор метрик'}
    fields = ('ours','vanilla','tolerance','verdict')
    matches = all(all(old[k][f] == new[k][f] for f in fields) for k in old)
    return {'required':True,'passed':matches,'reason':'A воспроизвёл контроль' if matches else 'A не воспроизвёл контроль'}


def save_measurement(output, report, edges, summary, provenance, expected, variant):
    validation = control_validation(expected,report,variant)
    summary = dict(summary,control_validation=validation)
    provenance = dict(provenance,control_validation=validation)
    ci.write(output/'report.json',report)
    ci.write(output/'edges.json',edges)
    ci.write(output/'summary.json',summary)
    provenance.update(report_sha256=ci.sha(output/'report.json'),edges_sha256=ci.sha(output/'edges.json'))
    ci.write(output/'provenance.json',provenance)
    # Никогда не выдаём несовпавший A за успех, но доказательства уже на диске.
    require(not validation['required'] or validation['passed'],validation['reason'])


def central_digest(chunks):
    """Проекция625чанков: signedLE32(cx,cz), затем signedLE16Y[z,x]."""
    wanted = {(x,z) for x in range(-12,13) for z in range(-12,13)}
    require(set(chunks) == wanted, 'Центральная проекция должна содержать все625чанков')
    hasher = hashlib.sha256()
    for (cx,cz), heights in sorted(chunks.items()):
        value = np.asarray(heights)
        require(value.shape == (16,16) and np.issubdtype(value.dtype,np.integer), 'Неверная проекция высот')
        require(bool(((value >= -1) & (value <= 255)).all()), 'Высоты вне диапазона Энда')
        hasher.update(struct.pack('<ii',cx,cz))
        hasher.update(value.astype('<i2').tobytes(order='C'))
    return hasher.hexdigest()


def central_height(world):
    from genaudit_263 import natural
    from genaudit_263_io import Reader, region_dir
    wanted = {(x,z) for x in range(-12,13) for z in range(-12,13)}
    reader = Reader()
    chunks = {}
    folder = region_dir(world,'the_end')
    for file in sorted(folder.glob('*.mcs')):
        for cx,cz,blocks,_,_ in reader.read(file,wanted):
            require((cx,cz) not in chunks, 'Повтор центрального чанка')
            ground = np.asarray([natural(n) for n in reader.names])[blocks]
            ground[:64] = False
            ground[320:] = False
            chunks[cx,cz] = np.where(ground.any(axis=0),383-ground[::-1].argmax(axis=0)-64,-1)
    return central_digest(chunks)


def island_windows(plan):
    """Два внешних окна того же плана; центральный остров не входит в выборку."""
    expected = [[-12, -12, 12, 12], [112, 112, 144, 144], [-144, 112, -112, 144]]
    actual = plan['windows']['the_end']
    require(sorted(actual) == sorted(expected), 'Изменились окна выборки малых островков')
    return expected[1:]


def measure_islands(world, plan, output):
    from genaudit_263_end_islands import analyze_world
    source = REPO / 'tools/measure/genaudit_263_end_islands.py'
    bank = REPO / MODEL_INPUTS[2]
    reference_path = REPO/'testdata/world-profile/genaudit-26.3/end-islands-official-reference.json'
    reference = json.loads(reference_path.read_text())
    require(reference['detector_source_sha256'] == ci.sha(source)
            and reference['bank_sha256'] == ci.sha(bank), 'Эталон островков измерен другим детектором/банком')
    windows = island_windows(plan)
    report = analyze_world(world, windows, observed_bank=bank)
    ci.write(output/'islands.json', report)
    require(report['requested_chunks'] == report['chunks'] == 2178
            and report['coverage'] == {'expected': 2178, 'actual': 2178, 'complete': True},
            'Неполная выборка изолированных островков')
    require(report['level']['seed'] == plan['seed'], 'Другое семя диагностики островков')
    # Это отдельная диагностика: старый анализатор и его допуски не меняются.
    path = output/'provenance.json'
    provenance = json.loads(path.read_text())
    provenance['island_diagnostic'] = {
        'source_sha256': ci.sha(source),
        'reader_source_sha256': ci.sha(REPO/'tools/measure/genaudit_263_io.py'),
        'observed_bank_sha256': ci.sha(bank),
        'official_reference_sha256': ci.sha(reference_path),
        'report_sha256': ci.sha(output/'islands.json'),
        'windows': windows,
        'requested_chunks': 2178,
    }
    ci.write(path, provenance)


def execute(label, variant, build, output, scratch):
    from genaudit_263_io import level, manifest
    from genaudit_263_end_edges import load_world
    import genaudit_263 as analyzer
    require(label in ci.SEEDS, 'Неизвестное семя')
    frozen = ci.reference()
    case = frozen['cases'][label]
    receipt = verify_bundle(build, variant)
    expected = control(label)
    require(expected['statistics']['vanilla']['dimensions']['the_end'] == case['baseline_report']['statistics']['vanilla']['dimensions']['the_end'], 'Контроль и frozen official различаются')
    for port, kind in ((26280, socket.SOCK_STREAM), (26281, socket.SOCK_DGRAM)):
        with socket.socket(socket.AF_INET, kind) as sock:
            sock.bind(('0.0.0.0', port))
    require(not scratch.exists() and not scratch.resolve().is_relative_to(REPO.resolve()), 'Нужен новый scratch вне repo')
    scratch.mkdir(parents=True)
    output.mkdir(parents=True, exist_ok=True)
    worlds = scratch/'worlds'
    worlds.mkdir()
    plan = end_plan(case['plan'])
    plan_path = worlds/f'{label}-plan.json'
    ci.write(plan_path, plan)
    metadata = scratch/'seed-metadata'
    metadata.mkdir()
    x,z = plan['center']
    (metadata/'level.dat').write_text(f'seed={case["seed"]}\nspawn={x} 64 {z}\n')
    for name in ('mcsheriffanya', 'genaudit-starts'):
        (build/name).chmod(0o555)
    ci.run([sys.executable, REPO/'tools/measure/genaudit_263_generate.py', '--output', worlds,
            '--owner-world', metadata, '--owner-seed', ci.SEEDS['owner'], '--second-seed', ci.SEEDS['second'],
            '--jar', '/unused-official-jar', '--ours', build/'mcsheriffanya', '--only', 'ours', '--label', label], output/'generation.log')
    world = worlds/f'{label}-ours/world'
    require(json.loads((world.parent/'generation-complete.json').read_text()) == plan, 'Генерация не завершена')
    require(level(world)['seed'] == case['seed'], 'Другое семя мира')
    require(json.loads(plan_path.read_text()) == plan, 'План изменён генератором')
    candidates = json.loads(subprocess.check_output([str(build/'genaudit-starts'), str(plan_path)], text=True))
    require(candidates['seed'] == case['seed'], 'Другое семя helper')
    saved_dims = analyzer.DIMS
    try:
        analyzer.DIMS = ('the_end',)
        ours = analyzer.analyze(world, plan, manifest(world), candidates)
        official = copy.deepcopy(case['baseline_report']['statistics']['vanilla'])
        official['dimensions'] = {'the_end': official['dimensions']['the_end']}
        report = analyzer.comparison(ours, official)
    finally:
        analyzer.DIMS = saved_dims
    # comparison добавляет spawn_offset вне DIMS; он не относится к опыту.
    report['rows'] = [r for r in report['rows'] if r['dimension'] == 'the_end']
    report['analyzer_source_sha256'] = ci.analyzer_sha()
    coverage(report)
    require(expected['seed'] == case['seed'], 'Другое семя контроля')
    require(expected['statistics']['vanilla']['dimensions']['the_end'] == official['dimensions']['the_end'], 'Контроль и frozen official различаются')
    summary = compare_rows(expected['rows'], report['rows'])
    center_hash = central_height(world)
    ci.write(output/'central-height.json', {'chunks':625,'central_height_sha256':center_hash,
             'definition':'sorted(cx,cz); signedLE32 coordinates + C-order[z,x] signedLE16 naturalY; void=-1'})
    save_measurement(output, report, load_world(world, plan), summary,
            {'label': label, 'variant': variant, 'seed': case['seed'], 'build': receipt,
             'original_plan_sha256': case['plan_sha256'], 'derived_plan_sha256': ci.canonical_sha(plan),
             'manifest': ours['manifest'], 'coverage': report['coverage'], 'central_height_sha256':center_hash},
            expected, variant)
    measure_islands(world, plan, output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--prepare-variant', choices=VARIANTS)
    modes.add_argument('--bundle', action='store_true')
    modes.add_argument('--execute', action='store_true')
    parser.add_argument('--variant', choices=VARIANTS)
    parser.add_argument('--label', choices=tuple(ci.SEEDS))
    for name in ('build', 'output', 'scratch'):
        parser.add_argument('--'+name, type=Path)
    args = parser.parse_args()
    if args.prepare_variant:
        prepare(args.prepare_variant)
    elif args.bundle:
        require(args.build is not None and args.variant is not None, 'Нужны build/variant')
        bundle(args.build, args.variant)
    else:
        require(all(x is not None for x in (args.label,args.variant,args.build,args.output,args.scratch)), 'Неполные параметры execute')
        require(ctypes.CDLL(None).prctl(36,1,0,0,0) == 0, 'Не удалось включить Linux child subreaper')
        signals = (signal.SIGINT,signal.SIGTERM,signal.SIGHUP)
        def interrupted(number, _frame):
            raise InterruptedError(f'Signal {number}')
        for sig in signals:
            signal.signal(sig,interrupted)
        try:
            execute(args.label,args.variant,args.build.resolve(),args.output.resolve(),args.scratch.resolve())
        finally:
            for sig in signals:
                signal.signal(sig,signal.SIG_IGN)
            ci.cleanup()


if __name__ == '__main__':
    main()
