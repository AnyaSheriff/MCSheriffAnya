"""Границы полного каталога и обязательная наблюдаемость действий."""
import collections
import unittest

try:
    from .catalog import catalog, coverage_matrix, SPECIAL_DAMAGE, PINNED_LIVING
    from .common_cases import common_cases
except ImportError:
    from catalog import catalog, coverage_matrix, SPECIAL_DAMAGE, PINNED_LIVING
    from common_cases import common_cases


class CatalogTests(unittest.TestCase):
    def test_full_living_boundary(self):
        names = {r['mob'] for r in catalog()}
        self.assertEqual(len(names), 92)
        self.assertTrue({'mannequin', 'camel_husk', 'parched', 'zombie_nautilus',
                         'ender_dragon', 'wither', 'armor_stand'} <= names)
        self.assertTrue({'player', 'boat', 'end_crystal', 'shulker_bullet'}.isdisjoint(names))
        self.assertEqual(len(PINNED_LIVING), len(names))
        self.assertEqual([r['classification'] for r in catalog() if r['mob'] == 'armor_stand'],
                         ['armor_stand'])

    def test_candidate_support_never_filters(self):
        empty = catalog()
        partial = catalog(['cow'])
        self.assertEqual([r['mob'] for r in empty], [r['mob'] for r in partial])
        self.assertEqual(sum(r['candidate_support'] is True for r in partial), 1)
        self.assertEqual(len(partial), 92)

    def test_case_contract_and_bounds(self):
        cases = common_cases()
        self.assertEqual(len(cases), 542)
        self.assertEqual(len({c['id'] for c in cases}), len(cases))
        groups = collections.defaultdict(set)
        for c in cases:
            groups[c['mob']].add(c['family'])
            self.assertIn(c['environment'], ('dry', 'water', 'air'))
            self.assertIn(c['policy'], ('exact', 'predicate'))
            self.assertTrue(c['compare_fields'])
            self.assertTrue(c['reference_requirements'])
            self.assertEqual(len({p['id'] for p in c['phases']}), len(c['phases']))
            for phase in c['phases']:
                self.assertIsInstance(phase['commands'], list)
                self.assertGreaterEqual(phase['ticks'], 1)
                self.assertLessEqual(phase['ticks'], 40)
            self.assertIn('NoAI:1b', c['nbt'])
            self.assertIn('NoGravity:1b', c['nbt'])
        for families in groups.values():
            self.assertTrue({'controlled_spawn', 'turn', 'teleport', 'tracking_leave_return'} <= families)
        self.assertEqual(set(groups), {r['mob'] for r in catalog()})

    def test_actions_require_actual_effect(self):
        for case in common_cases():
            commands = [s for p in case['phases'] for s in p['commands']]
            req = set(case['reference_requirements'])
            family = case['family']
            if family == 'damage':
                self.assertTrue(any(s.startswith('damage ') for s in commands))
                self.assertTrue({'health_decreased', 'damage_event_observed'} <= req)
            if family == 'death':
                self.assertIn('kill {target}', commands)
                self.assertIn('death_event_observed', req)
            if family == 'tracking_leave_return':
                self.assertEqual(len(commands), 2)
                self.assertTrue({'removal_observed', 'respawn_observed'} <= req)
                self.assertNotIn('death_event_observed', req)

    def test_special_applicability_and_honest_matrix(self):
        cases = common_cases()
        for name in SPECIAL_DAMAGE:
            self.assertFalse(any(c['mob'] == name and c['family'] in ('damage', 'death') for c in cases))
        rows = coverage_matrix(cases)
        self.assertEqual(len(rows), len({(r['mob'], r['behavior']) for r in rows}))
        self.assertEqual(sum(r['status'] == 'IMPLEMENTED' for r in rows), 542)
        self.assertEqual(sum(r['status'] == 'PLANNED' for r in rows), 92 * 11)
        self.assertFalse(any(r['status'] == 'PASS' for r in rows))
        self.assertTrue(all(r.get('reason') for r in rows if r['status'] == 'EXCLUDED_GENERIC'))


if __name__ == '__main__':
    unittest.main()
