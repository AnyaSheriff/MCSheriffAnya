import unittest
from thorns_unbreaking_regression import check_semantics


class UnbreakingRegressionTest(unittest.TestCase):
    def row(self, reflection, health, broken):
        return {'reflection': reflection, 'victim_hp': health,
                'event_order': [{'incoming_index': 10, 'break_index': broken}]}

    def test_break_before_damage_cannot_reflect(self):
        check_semantics([self.row(False, 19., 5)])
        with self.assertRaises(AssertionError):
            check_semantics([self.row(True, 19., 5)])

    def test_break_after_reflection_and_survival_keep_thorns(self):
        check_semantics([self.row(True, 19.304000854492188, 12), self.row(True, 19.304000854492188, None)])
        with self.assertRaises(AssertionError):
            check_semantics([self.row(False, 19.304000854492188, None)])

    def test_health_cannot_hide_missing_break_event(self):
        with self.assertRaises(AssertionError):
            check_semantics([self.row(True, 19., None)])


if __name__ == '__main__':
    unittest.main()
