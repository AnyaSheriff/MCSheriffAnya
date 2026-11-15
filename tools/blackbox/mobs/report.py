#!/usr/bin/env python3
"""Честное сравнение наблюдений: пустые записи никогда не становятся совпадением."""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
try:
    from .catalog import catalog, PLANNED_BEHAVIORS
except ImportError:
    from catalog import catalog, PLANNED_BEHAVIORS

POSITION_TOL = 1 / 4096 + 1e-8
ANGLE_TOL = 360 / 256 + 1e-8
HEALTH_TOL = 1e-4
FIELDS = {'type', 'position', 'rotation', 'health', 'removed', 'damage', 'status',
          'spawn_yaw', 'spawn_pitch'}
ALIASES = {'spawn_seen': 'spawn_observed', 'rotation_seen': 'rotation_seen'}
REQUIREMENTS = {'spawn_observed', 'rotation_seen', 'rotation_changed', 'position_changed',
                'removal_observed', 'respawn_observed', 'health_decreased',
                'damage_event_observed', 'death_event_observed', 'stationary', 'falls',
                'moves', 'yaw_changes'}
CLEANUP = dict(server_exit=0, forced=False, group_gone=True, port_free=True)


class InvalidObservation(ValueError):
    pass


def number(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise InvalidObservation('отсутствующее или нечисловое конечное значение')
    return value


def vector(value):
    if not isinstance(value, (tuple, list)) or len(value) != 3:
        raise InvalidObservation('нет трёх координат')
    return [number(x) for x in value]


def angle_distance(a, b):
    return abs((number(a) - number(b) + 180) % 360 - 180)


def snapshots(row):
    return [row.get('initial')] + [p.get('snapshot') for p in row.get('phases', [])]


def subject_events(row, events=None):
    initial = row.get('initial') or {}
    uid = initial.get('uuid')
    events = row.get('events', []) if events is None else events
    ids = {row.get('entity'), initial.get('entity')}
    for event in row.get('events', []):
        state = event.get('state') or {}
        if uid and state.get('uuid') == uid:
            ids.add(event.get('entity'))
    ids.discard(None)
    return [e for e in events if e.get('entity') in ids or ids.intersection(e.get('entities', []))]


def effects(row, events=None):
    selected = subject_events(row, events)
    return {'damage': sorted(e['name'] for e in selected
                             if e.get('name') in ('damage_event', 'hurt_animation')),
            'status': sorted(e['entity_status'] for e in selected
                             if e.get('name') == 'entity_status' and 'entity_status' in e),
            'removed': any(e.get('name') == 'entity_destroy' for e in selected)}


def structural(row, case):
    if not isinstance(row.get('initial'), dict) or not row.get('events'):
        raise InvalidObservation('пустая начальная запись или лента событий')
    phases = row.get('phases', [])
    if [p.get('id') for p in phases] != [p['id'] for p in case['phases']]:
        raise InvalidObservation('неполные, повторные или переставленные фазы')
    if any(not isinstance(p.get('snapshot'), dict) or not isinstance(p.get('events'), list) for p in phases):
        raise InvalidObservation('нет снимка или событий фазы')
    if row.get('mob') != case['mob'] or row['initial'].get('entity_name') != case['mob']:
        raise InvalidObservation('неверный моб в записи')
    if not any(e.get('name') == 'spawn_entity' for e in subject_events(row)):
        raise InvalidObservation('нет пакета появления испытуемого')


def requirement(name, row):
    states = snapshots(row)
    first = states[0]
    # Промежуточные пакеты сохраняют движение с возвратом в начальную точку.
    observed = states + [e['state'] for e in subject_events(row) if isinstance(e.get('state'), dict)]
    selected = subject_events(row)
    effect = effects(row)
    if name == 'spawn_observed':
        return bool(first.get('entity_name')) and any(e.get('name') == 'spawn_entity' for e in selected)
    if name == 'rotation_seen':
        number(first.get('yaw')); number(first.get('pitch')); return True
    if name in ('rotation_changed', 'yaw_changes'):
        yaw = number(first.get('yaw'))
        threshold = 10 if name == 'yaw_changes' else ANGLE_TOL
        return any(angle_distance(yaw, s['yaw']) >= threshold for s in observed[1:] if 'yaw' in s)
    if name in ('position_changed', 'stationary', 'moves', 'falls'):
        start = vector(first.get('position'))
        positions = [vector(s.get('position')) for s in observed[1:] if 'position' in s]
        distances = [math.dist(start,p) for p in positions]
        if name == 'stationary': return all(d <= .08 + 1e-10 for d in distances)
        if name == 'falls': return any(start[1] - p[1] >= 1 - 1e-10 for p in positions)
        if name == 'moves': return any(d >= .25 - 1e-10 for d in distances)
        return any(d > POSITION_TOL for d in distances)
    if name == 'health_decreased':
        health = number(first.get('health'))
        return any(number(s.get('health')) < health - HEALTH_TOL for s in states[1:])
    if name == 'damage_event_observed': return bool(effect['damage']) or 2 in effect['status']
    if name == 'death_event_observed': return 3 in effect['status']
    if name == 'removal_observed': return effect['removed']
    if name == 'respawn_observed':
        removed = False
        for event in selected:
            if event.get('name') == 'entity_destroy': removed = True
            if removed and event.get('name') == 'spawn_entity': return True
        return False
    raise KeyError(name)


def field_value(field, state, row, phase=None):
    if field in ('damage', 'status'):
        return effects(row, phase['events'] if phase else []) [field]
    if field in ('spawn_yaw', 'spawn_pitch'):
        return number(row['initial'].get(field.removeprefix('spawn_')))
    if field == 'type':
        value = state.get('entity_name')
        if not isinstance(value, str) or not value: raise InvalidObservation('нет имени типа')
        return value
    if field == 'position': return vector(state.get('position'))
    if field == 'rotation': return [number(state.get('yaw')), number(state.get('pitch'))]
    if field == 'health': return number(state.get('health'))
    if field == 'removed':
        value = state.get('removed')
        if not isinstance(value, bool): raise InvalidObservation('нет признака удаления')
        return value
    raise KeyError(field)


def equal_field(field, a, b):
    if field == 'position': return all(abs(x-y) <= POSITION_TOL for x,y in zip(a,b))
    if field == 'rotation': return all(angle_distance(x,y) <= ANGLE_TOL for x,y in zip(a,b))
    if field in ('spawn_yaw', 'spawn_pitch'): return angle_distance(a,b) <= ANGLE_TOL
    if field == 'health': return abs(a-b) <= HEALTH_TOL
    return a == b


def compare_case(case, reference, candidate):
    result = dict(id=case['id'], mob=case['mob'], family=case['family'], status='MATCH', reasons=[], limitations=[])
    def fail(status, message):
        result.update(status=status, reasons=[message]); return result
    fields = case.get('compare_fields', [])
    requirements = [ALIASES.get(r,r) for r in case.get('reference_requirements', [])]
    if not fields or not requirements or case.get('policy') not in ('exact', 'predicate'):
        return fail('UNSUPPORTED_CHECK', 'неполный договор проверки')
    unknown = (set(fields)-FIELDS) | (set(requirements)-REQUIREMENTS)
    if unknown: return fail('UNSUPPORTED_CHECK', 'неизвестные проверки: '+','.join(sorted(unknown)))
    if reference is None: return fail('HARNESS_ERROR', 'нет записи эталона')
    if reference.get('status') != 'RECORDED': return fail('HARNESS_ERROR', 'ошибка выполнения эталона: '+str(reference.get('error')))
    for label,row in (('reference',reference), ('candidate',candidate)):
        if row:
            events = subject_events(row)
            if any(e.get('status', '').startswith(('decode_error','unparsed_relative_teleport')) for e in events):
                return fail('HARNESS_ERROR', 'неполная расшифровка пакетов испытуемого')
            if any(e.get('status') == 'unparsed_metadata' for e in events):
                result['limitations'].append(label+': неразобранный остаток metadata; проверены только запрошенные доступные поля')
    # Сначала проверяем применимость эталона; кандидат не может скрыть пустой опыт.
    try:
        structural(reference, case)
        reference_values = [[field_value(f,s,reference,phase) for f in fields]
                            for s,phase in [(reference['initial'],None)] +
                            [(p['snapshot'],p) for p in reference['phases']]]
        failed = [r for r in requirements if not requirement(r,reference)]
        if failed:
            stochastic = bool(set(failed) & {'moves','yaw_changes'})
            return fail('INCONCLUSIVE' if stochastic else 'INVALID_REFERENCE', 'эталон не показал: '+','.join(failed))
    except (InvalidObservation, KeyError, TypeError, ValueError) as error:
        return fail('INVALID_REFERENCE', str(error))
    if candidate is None: return fail('MISSING_CANDIDATE', 'нет записи кандидата')
    if candidate.get('status') != 'RECORDED':
        return fail('MISSING_CANDIDATE', 'кандидат не записан: '+str(candidate.get('error')))
    try:
        structural(candidate, case)
        candidate_values = [[field_value(f,s,candidate,phase) for f in fields]
                            for s,phase in [(candidate['initial'],None)] +
                            [(p['snapshot'],p) for p in candidate['phases']]]
        failed = [r for r in requirements if not requirement(r,candidate)]
        if failed: return fail('DIFFERENT', 'кандидат не показал: '+','.join(failed))
        differences = []
        for index,(a,b) in enumerate(zip(reference_values,candidate_values)):
            for f,x,y in zip(fields,a,b):
                # В случайном AI-опыте сравнивается движение, а не случайная траектория.
                if case['policy'] == 'predicate' and f == 'position': continue
                if not equal_field(f,x,y): differences.append(f'фаза {index}: {f}: {x!r} != {y!r}')
        if differences: result.update(status='DIFFERENT', reasons=differences)
    except (InvalidObservation, KeyError, TypeError, ValueError) as error:
        return fail('DIFFERENT', 'неполная запись кандидата: '+str(error))
    return result


def compare_reports(reference, candidate, cases):
    errors = []
    def index(rows, label):
        ids = [r.get('id') for r in rows]
        if None in ids or len(ids) != len(set(ids)): errors.append(label+': отсутствующие/повторные id')
        return {r.get('id'):r for r in rows}
    expected = index(cases,'cases')
    refs = index(reference.get('cases', []),'reference')
    candidates = index(candidate.get('cases', []),'candidate')
    for label,side in [('reference',reference),('candidate',candidate)]:
        if side.get('cleanup') != CLEANUP: errors.append(label+': остановка/освобождение ресурсов не подтверждены')
        if side.get('error'): errors.append(label+': '+str(side['error']))
        if not side.get('source') or not side.get('identities'): errors.append(label+': нет source/identities')
    for key in ('protocol.json','entities.json','cases_sha256'):
        a = reference.get('identities',{}).get(key); b = candidate.get('identities',{}).get(key)
        if not a or a != b: errors.append('не совпадают identities: '+key)
    if reference.get('source') != candidate.get('source'): errors.append('разные source')
    if set(refs)-set(expected) or set(candidates)-set(expected): errors.append('лишние id записи')
    results = [compare_case(case,refs.get(case['id']),candidates.get(case['id'])) for case in cases]
    if errors:
        for row in results:
            row['ungated_status'] = row['status'];row['status']='HARNESS_ERROR';row['reasons'] += errors
    counts = dict(Counter(r['status'] for r in results))
    executed = {(r['mob'],r['family']) for r in results if r['status'] in ('MATCH','DIFFERENT')}
    matched = {(r['mob'],r['family']) for r in results if r['status'] == 'MATCH'}
    return dict(errors=errors, counts=counts, cases=results,
                limitations=sorted({x for r in results for x in r.get('limitations', [])}),
                coverage=dict(catalog_mobs=len(catalog()), planned_pairs=len(catalog()) * len(PLANNED_BEHAVIORS),
                              implemented_cases=len(expected),
                              implemented_pairs=len({(c['mob'],c['family']) for c in cases}),
                              executed_pairs=len(executed), matched_pairs=len(matched)),
                gate_pass=bool(results) and all(r['status']=='MATCH' for r in results))


def markdown(report):
    lines = ['# Наблюдения мобов', '', 'Совпадение подтверждено только для MATCH.', '',
             '| Результат | Сценарии |', '|---|---:|']
    lines += [f'| {key} | {value} |' for key,value in sorted(report['counts'].items())]
    lines += ['', 'Ограничения: ' + ('; '.join(report.get('limitations', [])) or 'не заявлены'), '']
    lines += ['', '| Сценарий | Результат | Причина |', '|---|---|---|']
    for row in report['cases']:
        lines.append('| '+row['id']+' | '+row['status']+' | '+'; '.join(row['reasons']).replace('|','/')+' |')
    return '\n'.join(lines)+'\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('reference','candidate','cases-file','output'): parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--gate',action='store_true')
    args = parser.parse_args()
    report = compare_reports(json.loads(args.reference.read_text()),json.loads(args.candidate.read_text()),
                             json.loads(args.cases_file.read_text()))
    args.output.mkdir(parents=True,exist_ok=True)
    (args.output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    (args.output/'report.md').write_text(markdown(report))
    if args.gate and not report['gate_pass']: raise SystemExit(1)


if __name__ == '__main__': main()
