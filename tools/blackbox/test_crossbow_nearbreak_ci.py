import copy
import json
from pathlib import Path
import unittest

from crossbow_nearbreak_ci import ROOT, validate_rows, digest


class FrozenCrossbowControls(unittest.TestCase):
    def setUp(self):
        self.config = json.loads((ROOT / 'tools/blackbox/ci/crossbow-nearbreak-777.json').read_text())
        self.fixtures = []
        for spec in self.config['probes']:
            path = ROOT / spec['expected']
            self.assertEqual(digest(path), spec['expected_sha256'])
            self.fixtures.append(json.loads(path.read_text())['rows'])

    def test_original_16_and_20_are_accepted_without_rewriting(self):
        self.assertEqual([len(rows) for rows in self.fixtures], [16, 20])
        for rows in self.fixtures:
            validate_rows(copy.deepcopy(rows), rows)

    def test_missing_trial_is_rejected(self):
        for rows in self.fixtures:
            with self.assertRaises(AssertionError):
                validate_rows(rows[:-1], rows)

    def test_extra_projectile_after_break_is_rejected(self):
        for rows in self.fixtures:
            changed = copy.deepcopy(rows)
            row = next(r for r in changed if r['after'] is None and len(r['emitted']) < 3)
            row['emitted'].append('arrow')
            with self.assertRaises(AssertionError):
                validate_rows(changed, rows)

    def test_surviving_fresh_wear_is_required(self):
        for rows in self.fixtures:
            changed = copy.deepcopy(rows)
            row = next(r for r in changed if r['initial_damage'] == 0)
            row['after']['damage'] += 1
            with self.assertRaises(AssertionError):
                validate_rows(changed, rows)

    def test_break_receipt_is_required_on_both_observers(self):
        for rows in self.fixtures:
            changed = copy.deepcopy(rows)
            next(r for r in changed if r['after'] is None)['statuses_by_observer'][1] = []
            with self.assertRaises(AssertionError):
                validate_rows(changed, rows)

    def test_broken_weapon_cannot_be_replaced_by_unobserved_item(self):
        for rows in self.fixtures:
            changed = copy.deepcopy(rows)
            next(r for r in changed if r['after'] is None)['after'] = dict(item='crossbow', count=1, damage=464)
            with self.assertRaises(AssertionError):
                validate_rows(changed, rows)


if __name__ == '__main__':
    unittest.main()
