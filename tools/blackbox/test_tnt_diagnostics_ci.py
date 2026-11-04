"""Отказы smoke-проверки не должны превращаться в успешную диагностику."""
import copy
import unittest
from unittest.mock import Mock, patch

from tnt_diagnostics_ci import clean_shutdown, records, validate


def fixture():
    rows = []
    for tick, fuse in [(20, 61), (40, 41)]:
        sample = {'id': 17, 'position': [8.5, 119.0, 8.5], 'velocity': [0.0, -0.1, 0.0],
                  'fuse': fuse, 'chunk': [0, 0], 'loaded': True}
        stage = {'total': 1, 'loaded': 1, 'omitted': 0, 'samples': [sample]}
        rows.append({'tick': tick, 'dimension': 'minecraft:overworld',
                     'sampling': 'one_step_every_20_ticks', 'budget_exhausted': False,
                     'world_lock_wait_us': 0, 'item_stage_elapsed_us': 1,
                     'before': copy.deepcopy(stage), 'after_step': copy.deepcopy(stage),
                     'exploding': 0, 'sampled_fuses_after_burn': [[17, fuse - 1]]})
    return rows


class AcceptanceGuards(unittest.TestCase):
    def test_valid_and_disabled_empty(self):
        validate(True, fixture())
        validate(False, [])

    def test_disabled_leak_and_missing_enabled_output_fail(self):
        for enabled, rows in [(False, fixture()), (True, []), (True, fixture()[:1])]:
            with self.subTest(enabled=enabled, count=len(rows)), self.assertRaises(ValueError):
                validate(enabled, rows)

    def test_stale_wrong_id_unloaded_and_no_burn_fail(self):
        mutations = [
            lambda rows: rows[1].update(tick=20),
            lambda rows: rows[1]['before']['samples'][0].update(id=99),
            lambda rows: rows[0]['after_step']['samples'][0].update(loaded=False),
            lambda rows: rows[0].update(sampled_fuses_after_burn=[[17, 61]]),
            lambda rows: rows[1]['before']['samples'][0].update(fuse=61),
            lambda rows: rows[0]['before']['samples'][0].update(position=[float('nan'), 0, 0]),
            lambda rows: rows[0]['before'].update(omitted=1),
            lambda rows: rows[0].update(budget_exhausted=True),
        ]
        for index, mutate in enumerate(mutations):
            rows = fixture()
            mutate(rows)
            with self.subTest(case=index), self.assertRaises(ValueError):
                validate(True, rows)

    def test_cleanup_escalation_or_surviving_group_cannot_pass(self):
        proc = Mock()
        identity = {'pid': 123}
        for method, code, gone in [
            ('term-after-stop-timeout', 0, True),
            ('kill-after-term-timeout', -9, True),
            ('stop-and-eof', 0, False),
            ('already-exited', 0, True),
            ('stop-and-eof', 1, True),
        ]:
            result = {'method': method, 'exit_code': code, 'group_gone': gone}
            with self.subTest(result=result), patch('tnt_diagnostics_ci.stop_process', return_value=result) as stop:
                with self.assertRaises(RuntimeError):
                    clean_shutdown(proc, identity)
                stop.assert_called_once_with(proc, identity=identity)
        result = {'method': 'stop-and-eof', 'exit_code': 0, 'group_gone': True}
        with patch('tnt_diagnostics_ci.stop_process', return_value=result):
            self.assertEqual(clean_shutdown(proc, identity), result)

    def test_malformed_diagnostic_json_is_failure(self):
        with self.assertRaises(ValueError):
            records('log MCSA_TNT_DIAGNOSTICS {bad}\n')


if __name__ == '__main__':
    unittest.main()
