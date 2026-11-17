"""Receipt gate: успешный cargo exit с отсутствующим тестом не является proof."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('bossbar_ci_subject', Path(__file__).with_name('bossbar_ci.py'))
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class ActualRuntimeResult(unittest.TestCase):
    def test_actual_exact_success(self):
        for name in ci.TESTS:
            ci.verify_runtime_log(name, f'test {name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 3999 filtered out; finished in 0.1s\n')

    def test_zero_tests_is_rejected(self):
        with self.assertRaises(ValueError):
            ci.verify_runtime_log(ci.TESTS[0], 'test result: ok. 0 passed; 0 failed; 0 ignored; 4000 filtered out; finished in 0.0s\n')

    def test_ignored_or_other_test_is_rejected(self):
        for name in ci.TESTS:
            for status in ['ignored', 'FAILED']:
                with self.assertRaises(ValueError):
                    ci.verify_runtime_log(name, f'test {name} ... {status}\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n')
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(name, 'test plugins::tests::other ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n')

    def test_bad_summary_is_rejected_even_with_ok_line(self):
        for summary in ['0 passed; 0 failed; 1 ignored;', '1 passed; 1 failed; 0 ignored;', '1 passed; 0 failed; 1 ignored;']:
            with self.assertRaises(ValueError):
                ci.verify_runtime_log(ci.TESTS[0], f'test {ci.TESTS[0]} ... ok\ntest result: ok. {summary}\n')
