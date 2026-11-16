"""Проверка полученных по сети HP Allay; внутренняя фаза лечения неизвестна."""
import argparse
import json
import math
from pathlib import Path


def _health(snapshot):
    value = snapshot.get('health') if isinstance(snapshot, dict) else None
    return value if type(value) in (int, float) and math.isfinite(value) and 0 <= value <= 20 else None


def analyze(reference, candidate, cases):
    errors, differences, rows = [], [], []
    definitions = cases.get('cases') if isinstance(cases, dict) else cases
    expected = [f'allay.regen-age{age:02d}-damage{amount:02d}' for age in (2, 7, 12) for amount in (1, 4)]
    if not isinstance(definitions, list):
        definitions = []
    if [c.get('id') if isinstance(c, dict) else None for c in definitions] != expected:
        errors.append('definitions: missing, duplicate, unexpected or out-of-order cases')
    for definition in definitions:
        if not isinstance(definition, dict):
            continue
        ident = definition.get('id')
        age, amount = definition.get('damage_age'), definition.get('damage_amount')
        if type(age) is not int or age not in (2, 7, 12) or type(amount) is not int or amount not in (1, 4) or ident != f'allay.regen-age{age:02d}-damage{amount:02d}':
            errors.append(f'{ident}: invalid definition age/amount')
            continue
        phases = ['before', 'hurt'] + [f'after_{i:02d}' for i in range(1, 41)]
        definition_phases = definition.get('phases')
        if not isinstance(definition_phases, list) or [p.get('id') if isinstance(p, dict) else None for p in definition_phases] != phases:
            errors.append(f'{ident}: invalid definition phases')
        else:
            expected_ticks = [age - 2, 0] + [1] * 40
            if any(type(p.get('ticks')) is not int or p['ticks'] != tick for p, tick in zip(definition_phases, expected_ticks)):
                errors.append(f'{ident}: invalid definition ticks')
        result = {'id': ident}
        for label, report in [('reference', reference), ('candidate', candidate)]:
            parsed = {'samples': [], 'healing_transitions': []}
            result[label] = parsed
            report_rows = report.get('cases', []) if isinstance(report, dict) else []
            matches = [r for r in report_rows if isinstance(r, dict) and r.get('id') == ident] if isinstance(report_rows, list) else []
            if len(matches) != 1:
                errors.append(f'{label}/{ident}: missing or duplicate case')
                continue
            row = matches[0]
            if row.get('status') != 'RECORDED' or row.get('error'):
                errors.append(f'{label}/{ident}: recording failed')
            recorded = row.get('phases', [])
            if not isinstance(recorded, list) or [p.get('id') if isinstance(p, dict) else None for p in recorded] != phases:
                errors.append(f'{label}/{ident}: missing, duplicate, unexpected or out-of-order phases')
                continue
            snapshots = [('initial', 2, row.get('initial'))] + [(p['id'], age if i < 2 else age + i - 1, p.get('snapshot')) for i, p in enumerate(recorded)]
            subject_ids = {s.get('entity') for _, _, s in snapshots if isinstance(s, dict) and type(s.get('entity')) is int}
            if type(row.get('entity')) is int:
                subject_ids.add(row['entity'])
            for phase in recorded:
                events = phase.get('events')
                if not isinstance(events, list):
                    errors.append(f'{label}/{ident}/{phase["id"]}: missing events')
                    continue
                for event in events:
                    if not isinstance(event, dict):
                        errors.append(f'{label}/{ident}/{phase["id"]}: invalid event')
                        continue
                    status = event.get('status')
                    if isinstance(status, str) and status.startswith(('decode_error', 'unparsed_relative_teleport')) and (event.get('entity') in subject_ids or event.get('entity') is None or not subject_ids):
                        errors.append(f'{label}/{ident}/{phase["id"]}: subject decode error')
            previous = None
            for phase, tick, snapshot in snapshots:
                health = _health(snapshot)
                parsed['samples'].append(dict(phase=phase, entity_tick=tick, health=health))
                if health is None:
                    errors.append(f'{label}/{ident}/{phase}: missing or invalid health')
                if previous is not None and health is not None and health > previous:
                    parsed['healing_transitions'].append({'phase': phase, 'entity_tick': tick, 'from': previous, 'to': health})
                previous = health
            sample = {s['phase']: s['health'] for s in parsed['samples']}
            if sample['initial'] != 20 or sample['before'] != 20:
                errors.append(f'{label}/{ident}: initial/before health must be 20')
            early = [sample[p] for p in ['hurt', 'after_01', 'after_02', 'after_03']]
            if any(h is not None and h < 20 - amount for h in early) or 20 - amount not in early:
                errors.append(f'{label}/{ident}: requested damage not observed')
            if sample['after_40'] != 20:
                errors.append(f'{label}/{ident}: health not restored by after_40')
        if result['reference']['samples'] != result['candidate']['samples']:
            differences.append({'id': ident, 'reference': result['reference']['samples'], 'candidate': result['candidate']['samples']})
        rows.append(result)
    for label, report in [('reference', reference), ('candidate', candidate)]:
        if not isinstance(report, dict):
            errors.append(f'{label}: invalid report')
            continue
        cleanup = report.get('cleanup')
        if not isinstance(cleanup, dict) or type(cleanup.get('server_exit')) is not int or cleanup.get('server_exit') != 0 or cleanup.get('forced') is not False or cleanup.get('group_gone') is not True or cleanup.get('port_free') is not True or cleanup.get('stop_error'):
            errors.append(f'{label}: invalid cleanup')
        recorded = report.get('cases')
        if not isinstance(recorded, list) or [c.get('id') if isinstance(c, dict) else None for c in recorded] != expected:
            errors.append(f'{label}: missing, duplicate, unexpected or out-of-order cases')
        if report.get('error'):
            errors.append(f'{label}: report error')
    return dict(recording_valid=not errors, exact_received_health_match=not errors and not differences, cases=rows, differences=differences, errors=errors,
                scope='Received network health at controlled entity ticks; not proof of internal healing phase.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('reference', 'candidate', 'cases-file'):
        parser.add_argument('--' + flag, required=True, type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--require-match', action='store_true')
    args = parser.parse_args()
    try:
        result = analyze(*(json.loads(path.read_text()) for path in (args.reference, args.candidate, args.cases_file)))
    except (OSError, ValueError) as error:
        result = dict(recording_valid=False, exact_received_health_match=False, cases=[], differences=[], errors=[str(error)])
    output = json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + '\n'
    if args.output:
        args.output.write_text(output)
    else:
        print(output, end='')
    return 0 if result['recording_valid'] and (not args.require_match or result['exact_received_health_match']) else 1


if __name__ == '__main__':
    raise SystemExit(main())
