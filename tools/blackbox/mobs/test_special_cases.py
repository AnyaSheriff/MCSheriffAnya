"""Проверка границ описаний; реальные результаты получает парный runner."""
import json
import unittest

from special_cases import special_cases


class SpecialCasesTests(unittest.TestCase):
    def test_unique_and_bounded(self):
        cases = special_cases()
        self.assertEqual(len(cases), 32)
        self.assertEqual(len({c['id'] for c in cases}), len(cases))
        self.assertLessEqual(sum(p['ticks'] for c in cases for p in c['phases']), 4000)
        for case in cases:
            self.assertEqual(json.loads(json.dumps(case)), case)
            self.assertIn(case['environment'], ('air', 'dry', 'water'))
            self.assertNotIn('{', case['nbt'])
            self.assertIn(case['policy'], ('exact', 'predicate'))
            self.assertEqual(case['reference_requirements'][0], 'spawn_seen')
            for phase in case['phases']:
                self.assertTrue(1 <= phase['ticks'] <= 160)
                self.assertEqual(phase['commands'], [])

    def test_random_motion_never_exact(self):
        for case in special_cases():
            if case['reference_requirements'][-1] == 'moves':
                self.assertEqual(case['policy'], 'predicate')
                control_id = case['id'].removesuffix('-ai') + '-noai'
                control = next(c for c in special_cases() if c['id'] == control_id)
                self.assertEqual(control['reference_requirements'][-1], 'stationary')
                self.assertEqual(control['environment'], case['environment'])

    def test_pose_is_explicit_and_not_ai_claim(self):
        cases = [c for c in special_cases() if c['family'] == 'protocol_pose']
        self.assertEqual(len(cases), 4)
        for case in cases:
            self.assertIn('Rotation:[', case['nbt'])
            self.assertIn('NoAI:1b', case['nbt'])
            self.assertEqual(case['policy'], 'exact')

    def test_independent_results(self):
        first = special_cases()
        first[0]['phases'][0]['commands'].append('changed')
        self.assertEqual(special_cases()[0]['phases'][0]['commands'], [])


if __name__ == '__main__':
    unittest.main()
