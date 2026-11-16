"""Синтетические проверки валидности записи и сравнения сетевых HP."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from analyze_allay_regen import analyze, main


def fixture():
    cases, rows = [], []
    for age in (2, 7, 12):
        for amount in (1, 4):
            ident = f'allay.regen-age{age:02d}-damage{amount:02d}'
            phases = [dict(id='before', ticks=age - 2), dict(id='hurt', ticks=0)]
            phases += [dict(id=f'after_{i:02d}', ticks=1) for i in range(1, 41)]
            cases.append(dict(id=ident, damage_age=age, damage_amount=amount, phases=phases))
            row = dict(id=ident, entity=7, status='RECORDED', initial=dict(health=20, entity=7), phases=[])
            for i, phase in enumerate(phases):
                hp = 20 if i == 0 else 20 - amount if i < 6 else 20
                row['phases'].append(dict(id=phase['id'], snapshot=dict(health=hp, entity=7), events=[]))
            rows.append(row)
    report = dict(cases=rows, cleanup=dict(server_exit=0, forced=False, group_gone=True, port_free=True))
    return report, copy.deepcopy(report), cases


class AnalyzeTests(unittest.TestCase):
    def setUp(self):
        self.reference, self.candidate, self.cases = fixture()

    def result(self):
        return analyze(self.reference, self.candidate, self.cases)

    def invalid(self):
        result = self.result()
        self.assertFalse(result['recording_valid'])
        self.assertFalse(result['exact_received_health_match'])
        self.assertTrue(result['errors'])

    def test_success_shape_and_controlled_ages(self):
        result = self.result()
        self.assertTrue(result['recording_valid'])
        self.assertTrue(result['exact_received_health_match'])
        self.assertEqual(len(result['cases']), 6)
        side = result['cases'][2]['reference']
        self.assertEqual(side['samples'][:3], [dict(phase='initial', entity_tick=2, health=20), dict(phase='before', entity_tick=7, health=20), dict(phase='hurt', entity_tick=7, health=19)])
        self.assertEqual(side['samples'][-1]['entity_tick'], 47)
        self.assertEqual(side['healing_transitions'], [dict(phase='after_05', entity_tick=12, **{'from': 19, 'to': 20})])

    def test_exact_difference_is_valid_baseline(self):
        self.candidate['cases'][0]['phases'][6]['snapshot']['health'] = 19
        result = self.result()
        self.assertTrue(result['recording_valid'])
        self.assertFalse(result['exact_received_health_match'])
        self.assertEqual([r['id'] for r in result['differences']], [self.cases[0]['id']])

    def test_healing_magnitude_not_prescribed(self):
        self.candidate['cases'][1]['phases'][6]['snapshot']['health'] = 18
        self.assertTrue(self.result()['recording_valid'])
        transitions = self.result()['cases'][1]['candidate']['healing_transitions']
        self.assertEqual([(t['from'], t['to']) for t in transitions], [(16, 18), (18, 20)])

    def test_missing_health(self):
        del self.candidate['cases'][0]['phases'][8]['snapshot']['health']
        self.invalid()

    def test_invalid_health_types_and_bounds(self):
        for value in (float('nan'), float('inf'), True, False, '20', -1, 21, None):
            with self.subTest(value=value):
                self.candidate['cases'][0]['phases'][8]['snapshot']['health'] = value
                self.invalid()

    def test_invalid_initial_health(self):
        self.candidate['cases'][0]['initial']['health'] = 19
        self.invalid()

    def test_missing_damage(self):
        for phase in self.candidate['cases'][0]['phases']:
            phase['snapshot']['health'] = 20
        self.invalid()

    def test_damage_can_arrive_after_hurt(self):
        self.candidate['cases'][0]['phases'][1]['snapshot']['health'] = 20
        self.assertTrue(self.result()['recording_valid'])

    def test_damage_larger_than_requested(self):
        self.candidate['cases'][0]['phases'][2]['snapshot']['health'] = 18
        self.invalid()

    def test_unrecovered_health(self):
        self.candidate['cases'][0]['phases'][-1]['snapshot']['health'] = 19
        self.invalid()

    def test_cleanup_fields_and_strict_types(self):
        for field, value in [('server_exit', 1), ('server_exit', False), ('forced', True), ('forced', 0), ('group_gone', False), ('port_free', False), ('stop_error', 'timeout')]:
            with self.subTest(field=field, value=value):
                self.candidate['cleanup'] = dict(server_exit=0, forced=False, group_gone=True, port_free=True)
                self.candidate['cleanup'][field] = value
                self.invalid()

    def test_missing_duplicate_reordered_cases(self):
        original = copy.deepcopy(self.candidate['cases'])
        for rows in (original[:-1], original + [original[0]], list(reversed(original))):
            self.candidate['cases'] = rows
            self.invalid()

    def test_missing_duplicate_reordered_phases(self):
        original = copy.deepcopy(self.candidate['cases'][0]['phases'])
        for rows in (original[:-1], original + [original[0]], list(reversed(original))):
            self.candidate['cases'][0]['phases'] = rows
            self.invalid()

    def test_subject_decode_error(self):
        self.candidate['cases'][0]['phases'][0]['events'] = [dict(entity=7, status='decode_error: bad packet')]
        self.invalid()

    def test_unparsed_subject_teleport(self):
        self.candidate['cases'][0]['phases'][0]['events'] = [dict(entity=7, status='unparsed_relative_teleport')]
        self.invalid()

    def test_other_entity_decode_error_does_not_reject_subject(self):
        self.candidate['cases'][0]['phases'][0]['events'] = [dict(entity=999, status='decode_error')]
        self.assertTrue(self.result()['recording_valid'])

    def test_missing_events(self):
        del self.candidate['cases'][0]['phases'][0]['events']
        self.invalid()

    def test_failed_case_or_side(self):
        self.candidate['cases'][0]['status'] = 'ERROR'
        self.invalid()
        self.candidate['cases'][0]['status'] = 'RECORDED'
        self.candidate['error'] = 'timeout'
        self.invalid()

    def test_definition_container_and_rejected_ticks(self):
        self.assertTrue(analyze(self.reference, self.candidate, {'cases': self.cases})['recording_valid'])
        self.cases[0]['phases'][1]['ticks'] = 1
        self.invalid()

    def test_definition_rejects_wrong_age_duplicate_order(self):
        original = copy.deepcopy(self.cases)
        for cases in (original[:-1], original + [original[0]], list(reversed(original))):
            self.cases = cases
            self.invalid()
        self.cases = original
        self.cases[0]['damage_age'] = True
        self.invalid()

    def test_cli_baseline_require_match_and_invalid(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            ref, cand, cases, output = [root / name for name in ('ref.json', 'cand.json', 'cases.json', 'out.json')]
            ref.write_text(json.dumps(self.reference))
            cases.write_text(json.dumps(self.cases))
            self.candidate['cases'][0]['phases'][6]['snapshot']['health'] = 19
            cand.write_text(json.dumps(self.candidate))
            args = ['analyze', '--reference', str(ref), '--candidate', str(cand), '--cases-file', str(cases), '--output', str(output)]
            with patch('sys.argv', args):
                self.assertEqual(main(), 0)
            self.assertFalse(json.loads(output.read_text())['exact_received_health_match'])
            with patch('sys.argv', args + ['--require-match']):
                self.assertEqual(main(), 1)
            self.candidate['cleanup']['forced'] = True
            cand.write_text(json.dumps(self.candidate))
            with patch('sys.argv', args):
                self.assertEqual(main(), 1)


if __name__ == '__main__':
    unittest.main()
