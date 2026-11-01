"""Ложный loaded/unloaded след не должен пройти приёмку."""
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from tnt_chunk_lifecycle_ci import aligned, clean_shutdown, experiment, records, schedule_due, validate


def fixture(prime=65):
    rows = []
    unload = (prime // 100 + 1) * 100
    reload = prime + 100
    for tick in range((prime // 20 + 1) * 20, ((reload + 19) // 20) * 20 + 1, 20):
        loaded = tick <= unload or tick >= reload
        # В этом искусственном следе зажигание немедленное; проверяются разности.
        fuse = 80 - sum(t <= unload or t >= reload for t in range(prime, tick))
        sample = {'id': 17, 'position': [8.5, 5.0, 8.5], 'velocity': [0.0, 0.0, 0.0],
                  'fuse': fuse, 'chunk': [0, 0], 'loaded': loaded}
        stage = {'total': 1, 'loaded': int(loaded), 'omitted': 0, 'samples': [sample]}
        rows.append({'tick': tick, 'dimension': 'minecraft:overworld',
                     'sampling': 'one_step_every_20_ticks', 'budget_exhausted': False,
                     'world_lock_wait_us': 0, 'item_stage_elapsed_us': 1,
                     'before': copy.deepcopy(stage), 'after_step': copy.deepcopy(stage),
                     'exploding': 0, 'sampled_fuses_after_burn': [[17, fuse - int(loaded)]]})
    return rows


class LifecycleGuards(unittest.TestCase):
    def test_valid_phase_edges_and_middle(self):
        for prime in (61, 65, 75, 265):
            proof = validate(fixture(prime), prime)
            self.assertGreaterEqual(proof['unloaded_samples'], 2)
            self.assertEqual(proof['reload_due'], prime + 100)

    def test_incomplete_trace_cannot_pass(self):
        for rows in ([], fixture()[:2], fixture()[:4], fixture()[2:]):
            with self.subTest(length=len(rows)), self.assertRaises(ValueError):
                validate(rows, 65)
        validate(fixture()[:1], 65, complete=False)

    def test_false_positive_mutations_fail(self):
        changes = [
            lambda r: r[1].update(tick=80),
            lambda r: r.pop(2),
            lambda r: r[2]['before']['samples'][0].update(id=99),
            lambda r: r[2]['before']['samples'][0].update(loaded=True),
            lambda r: r[2]['after_step']['samples'][0]['position'].__setitem__(0, 8.6),
            lambda r: r[3]['before']['samples'][0]['velocity'].__setitem__(1, -0.1),
            lambda r: r[2].update(sampled_fuses_after_burn=[[17, 43]]),
            lambda r: r[-1].update(sampled_fuses_after_burn=[[17, r[-1]['before']['samples'][0]['fuse']]]),
            lambda r: r[2]['before'].update(omitted=1),
            lambda r: r[0].update(budget_exhausted=True),
            lambda r: r[-1]['before']['samples'][0].update(position=[float('nan'), 0, 0]),
            lambda r: r[0].update(exploding=1),
            lambda r: r[0].update(dimension='minecraft:the_nether'),
        ]
        for i, change in enumerate(changes):
            rows = fixture()
            change(rows)
            with self.subTest(change=i), self.assertRaises(ValueError):
                validate(rows, 65)

    def test_cross_sample_fuse_drop_while_unloaded_fails(self):
        rows = fixture()
        for stage in ('before', 'after_step'):
            rows[3][stage]['samples'][0]['fuse'] -= 1
        rows[3]['sampled_fuses_after_burn'][0][1] -= 1
        with self.assertRaises(ValueError):
            validate(rows, 65)

    def test_phase_guard_and_exact_acknowledgment(self):
        self.assertTrue(aligned(265, 4))
        for due, now in [(240, 4), (280, 4), (265, 200), (265.0, 4)]:
            self.assertFalse(aligned(due, now))
        text = 'Функция «probe:prime» запланирована через 261 тактов, на такт 265'
        self.assertEqual(schedule_due(text, 'probe:prime', 261), 265)
        for name, delay in [('probe:noop', 261), ('probe:prime', 260)]:
            with self.assertRaises(ValueError):
                schedule_due(text, name, delay)

    def test_cleanup_requires_stop_eof_zero_and_no_survivors(self):
        proc = Mock()
        for method, code, gone in [('already-exited', 0, True), ('stop-and-eof', 1, True),
                                   ('stop-and-eof', 0, False), ('kill-after-term-timeout', -9, True)]:
            with patch('tnt_diagnostics_ci.stop_process', return_value={
                    'method': method, 'exit_code': code, 'group_gone': gone}):
                with self.assertRaises(RuntimeError):
                    clean_shutdown(proc, {'pid': 123})
        with patch('tnt_diagnostics_ci.stop_process', return_value={
                'method': 'stop-and-eof', 'exit_code': 0, 'group_gone': True}):
            self.assertEqual(clean_shutdown(proc, {'pid': 123})['exit_code'], 0)

    def test_fresh_world_is_untouched_until_server_ready(self):
        # Сервер видит действительно новый мир, функции доступны перед первой командой.
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            runtime = output / 'dummy-runtime'
            runtime.write_text('not executable; Popen is mocked')
            proc = Mock()
            proc.pid = 123
            proc.poll.return_value = None

            def launch(*args, **kwargs):
                self.assertFalse((kwargs['cwd'] / 'world').exists())
                kwargs['stdout'].write('Готово\n')
                kwargs['stdout'].flush()
                return proc

            def first_command(text):
                self.assertEqual(text, 'tick freeze\n')
                fixture = output / 'lifecycle/world/datapacks/probe/data/probe/function'
                self.assertEqual(sorted(p.name for p in fixture.iterdir()),
                                 ['noop.mcfunction', 'prime.mcfunction', 'reload.mcfunction'])
                raise RuntimeError('intentional mocked command boundary')

            proc.stdin.write.side_effect = first_command
            with patch('tnt_chunk_lifecycle_ci.subprocess.Popen', side_effect=launch), \
                    patch('tnt_chunk_lifecycle_ci.process_identity', return_value={'pid': 123}), \
                    patch('tnt_chunk_lifecycle_ci.stop_process', return_value={
                        'method': 'already-exited', 'exit_code': 0, 'group_gone': True}), \
                    patch('tnt_chunk_lifecycle_ci.group_exists', return_value=False), \
                    patch('tnt_chunk_lifecycle_ci.ports_free', return_value={'26152': True}):
                with self.assertRaisesRegex(RuntimeError, 'intentional mocked command boundary'):
                    experiment(runtime, output)
            import json
            receipt = json.loads((output / 'lifecycle/receipt.json').read_text())
            self.assertEqual(receipt['fixture_preparation'], 'after-server-ready-before-first-command')
            self.assertEqual(len(receipt['fixture_sha256']), 3)

    def test_bad_json_is_failure(self):
        with self.assertRaises(ValueError):
            records('MCSA_TNT_DIAGNOSTICS {broken}')


if __name__ == '__main__':
    unittest.main()
