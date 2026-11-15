"""Лёгкие проверки измерителя; сервер и нативный код не запускаются."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

PATH = Path(__file__).with_name('check-adaptive-combined.py')
SPEC = importlib.util.spec_from_file_location('combined_probe', PATH)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class ProbeTests(unittest.TestCase):
    def test_time_reply_requires_one_actual_numeric_answer(self):
        self.assertEqual(PROBE.time_value('[12:00:00] Время: 1205\n[Server] MCSA_COMBINED_fresh_0001'), 1205)
        for value in ('Unknown command', 'Время: nope', 'Время: 1200\nВремя: 1205'):
            with self.assertRaises(ValueError):
                PROBE.time_value(value)

    def test_disabled_tree_detects_byte_change_and_extra_file(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d)
            (path / 'sentinel').write_bytes(b'keep')
            original = PROBE.tree(path)
            self.assertEqual(original, PROBE.tree(path))
            (path / 'sentinel').write_bytes(b'changed')
            self.assertNotEqual(original, PROBE.tree(path))
            (path / 'sentinel').write_bytes(b'keep')
            (path / 'new-file').write_bytes(b'')
            self.assertNotEqual(original, PROBE.tree(path))

    def test_local_execution_refused_before_output_or_binary_access(self):
        with tempfile.TemporaryDirectory() as d:
            output = Path(d) / 'must-not-exist'
            env = dict(os.environ, GITHUB_ACTIONS='false')
            run = subprocess.run([sys.executable, '-B', str(PATH), '--binary', '/nonexistent-own-test-binary',
                                  '--output', str(output)], env=env, text=True, capture_output=True)
            self.assertNotEqual(run.returncode, 0)
            self.assertIn('GitHub-only', run.stderr)
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
