"""Receipt gate: успешный cargo exit с отсутствующим тестом не является proof."""
import importlib.util
import json
import subprocess
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('bossbar_ci_subject', Path(__file__).with_name('bossbar_ci.py'))
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class ActualRuntimeResult(unittest.TestCase):
    @staticmethod
    def log(name, body='ok', summary='1 passed; 0 failed; 0 ignored;', count=1):
        return f'running {count} test\ntest {name} ... {body}\ntest result: ok. {summary} 0 measured; 3719 filtered out; finished in 2.39s\n'

    def test_actual_exact_success(self):
        for name in (*ci.TESTS, 'plugins::tests::own_mod_source_tests::translated_own_mod_source_lifecycle_events_and_isolation'):
            ci.verify_runtime_log(name, self.log(name))

    def test_recorded_ci_44059957194_interleaved_native_info(self):
        # Exact native test-output section, without GitHub's outer timestamps.
        log = '''running 1 test
test plugins::tests::translated_bukkit_bossbar_multiplayer_updates_and_cleanup ... [10:23:06] [Server thread/INFO]: [java-bossbar] HTTPS: запрещён (нет разрешённых хостов)
[10:23:06] [Server thread/INFO]: [hello] HTTPS: запрещён (нет разрешённых хостов)
[10:23:06] [Server thread/INFO]: [hello] hello включён
[10:23:06] [Server thread/INFO]: [hello] hello выключен
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3719 filtered out; finished in 2.39s
'''
        ci.verify_runtime_log(ci.TESTS[0], log)

    def test_missing_terminal_ok_is_rejected_even_with_name_and_summary(self):
        for body in ('', '[INFO] callback completed', '[INFO] ok', '[INFO] callback\nnot ok'):
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(ci.TESTS[0], self.log(ci.TESTS[0], body))

    def test_zero_tests_is_rejected(self):
        with self.assertRaises(ValueError):
            ci.verify_runtime_log(ci.TESTS[0], 'running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 4000 filtered out; finished in 0.0s\n')

    def test_ignored_or_other_test_is_rejected(self):
        for name in ci.TESTS:
            for status in ['ignored', 'FAILED', '[INFO] callback\nFAILED\nok', '[INFO] callback\nignored\nok']:
                with self.assertRaises(ValueError):
                    ci.verify_runtime_log(name, self.log(name, status))
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, self.log('plugins::tests::other'))

    def test_forged_or_duplicate_name_is_rejected(self):
        name = ci.TESTS[0]
        for other in (name + '_forged', 'fake::' + name, name + ' extra'):
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, self.log(other, '[INFO] callback\nok'))
        for body in ('[INFO] callback\ntest foreign::test ... ok\nok', 'ok\ntest ' + name + ' ... ok'):
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, self.log(name, body))
        with self.assertRaises(ValueError):
            ci.verify_runtime_log(name, 'test foreign::test ... ok\n' + self.log(name))

    def test_single_run_is_required(self):
        name = ci.TESTS[0]
        for log in (self.log(name).replace('running 1 test\n', ''), self.log(name, count=2), self.log(name) + self.log(name)):
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, log)

    def test_bad_summary_is_rejected_even_with_ok_line(self):
        for summary in ['0 passed; 0 failed; 1 ignored;', '1 passed; 1 failed; 0 ignored;', '1 passed; 0 failed; 1 ignored;', '11 passed; 0 failed; 0 ignored;', '1 passed; 0 failed; 0 ignored; 1 failed;']:
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(ci.TESTS[0], self.log(ci.TESTS[0], summary=summary))

    def test_failed_duplicate_or_missing_summary_is_rejected(self):
        name = ci.TESTS[0]
        valid = self.log(name)
        for log in (valid.replace('test result: ok.', 'test result: FAILED.'), valid + valid.splitlines()[-1] + '\n', valid[:valid.index('test result:')]):
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, log)

    def test_multiple_or_nonterminal_ok_is_rejected(self):
        name = ci.TESTS[0]
        for body in ('[INFO] callback\nok\nok', '[INFO] callback\nok\n[INFO] still running', 'ok\nFAILED'):
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, self.log(name, body))


    def test_failed_scenario_keeps_later_results_and_fails_gate(self):
        executed = []

        def fake_run(command, log):
            name = command[command.index('--bin') + 2]
            executed.append(name)
            if name == ci.TESTS[0]:
                log.write_text('running 1 test\n' + f'test {name} ... FAILED\n')
                raise subprocess.CalledProcessError(101, command)
            if name == ci.TESTS[1]:
                log.write_text('running 0 tests\n')
                return
            log.write_text(self.log(name))

        with tempfile.TemporaryDirectory() as folder, patch.object(ci, 'run', fake_run):
            with patch.dict(ci.os.environ, {'GITHUB_ACTIONS': 'true'}), patch.object(
                ci.sys, 'argv', ['bossbar_ci.py', 'runtime', '--output', folder]
            ):
                with self.assertRaises(SystemExit):
                    ci.main()
            receipt = json.loads((Path(folder) / 'runtime-receipt.json').read_text())
            self.assertEqual(executed, list(ci.TESTS))
            self.assertEqual(len(receipt['failures']), 2)
            for name, failure in zip(ci.TESTS, receipt['failures']):
                self.assertTrue(failure.startswith(name + ':'))
            self.assertTrue(all(f'runtime-{i}.log' in receipt['files'] for i in range(1, 4)))
