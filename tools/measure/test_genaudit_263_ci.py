"""Bounded CI wrapper tests; never run Cargo or a Minecraft server."""
import copy
import gzip
import importlib.util
import json
from pathlib import Path
import tempfile
import os
import signal
import subprocess
import sys
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('ci', HERE/'genaudit_263_ci.py')
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)


class CensusCI(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.frozen = ci.reference()
        cls.report = cls.frozen['cases']['owner']['baseline_report']

    def test_frozen_two_seed_full_coverage(self):
        self.assertEqual(sum(sum(row['ours']['chunks'] for row in case['baseline_report']['coverage'])
                             for case in self.frozen['cases'].values()), 82282)

    def test_frozen_statistics_reproduce_original_reports(self):
        sys.path.insert(0, str(HERE))
        import genaudit_263 as analyzer
        for label, case in self.frozen['cases'].items():
            with self.subTest(label=label):
                baseline = case['baseline_report']
                rebuilt = analyzer.comparison(baseline['statistics']['ours'],
                                              baseline['statistics']['vanilla'])
                self.assertEqual(ci.canonical_sha(rebuilt['rows']), ci.canonical_sha(baseline['rows']))
                self.assertEqual(ci.canonical_sha(rebuilt['coverage']), ci.canonical_sha(baseline['coverage']))

    def test_cleanup_only_owned_process_group(self):
        # A sleeping child is enough to exercise actual POSIX process group cleanup.
        child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'],
                                 start_new_session=True)
        ci.GROUPS.add(child.pid)
        try:
            self.assertTrue(ci.alive(child.pid))
            ci.cleanup()
            self.assertFalse(ci.alive(child.pid))
            self.assertFalse(ci.GROUPS)
            self.assertGreater(os.getpid(), 0)
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            ci.GROUPS.discard(child.pid)

    def test_same_baseline_has_no_regressions(self):
        result = ci.compare(self.report, self.report)
        self.assertEqual(result['counts']['common_rows'], len(self.report['rows']))
        self.assertEqual(result['counts'].get('regressions', 0), 0)

    def test_changed_reference_rejected(self):
        other = copy.deepcopy(self.report)
        other['statistics']['vanilla']['manifest'] = []
        with self.assertRaises(AssertionError):
            ci.compare(self.report, other)

    def test_incomplete_coverage_rejected(self):
        other = copy.deepcopy(self.report)
        other['coverage'][0]['ours']['chunks'] -= 1
        with self.assertRaises(AssertionError):
            ci.coverage(other)
        with self.assertRaises(AssertionError):
            ci.compare(self.report, other)

    def test_new_removed_rows_are_separate(self):
        other = copy.deepcopy(self.report)
        removed = other['rows'].pop(0)
        new = copy.deepcopy(removed)
        new['metric'] = 'new_ci_test_metric'
        new['verdict'] = 'outside'
        other['rows'].append(new)
        result = ci.compare(self.report, other)
        self.assertEqual(result['counts']['new_rows'], 1)
        self.assertEqual(result['counts']['removed_rows'], 1)
        self.assertEqual(result['counts'].get('resolved', 0), 0)
        self.assertEqual(result['new_rows'], [new])
        self.assertEqual(result['removed_rows'], [removed])

    def test_fixture_corruption_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'fixture.json.gz'
            path.write_bytes(gzip.compress(b'{}', mtime=0))
            path.with_suffix('').write_text(json.dumps({'fixture_sha256':'wrong','bytes':path.stat().st_size}))
            with self.assertRaisesRegex(AssertionError, 'Compressed reference'):
                ci.reference(path)

    def test_artifact_hash_and_commit_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            for name in ('mcsheriffanya','genaudit-starts','Cargo.lock'):
                (folder/name).write_text(name)
            with patch.object(ci, 'head', return_value='a'*40):
                ci.bundle(folder)
                ci.verify_bundle(folder)
                (folder/'genaudit-starts').write_text('changed')
                with self.assertRaisesRegex(AssertionError, 'Build artifact changed'):
                    ci.verify_bundle(folder)
            with patch.object(ci, 'head', return_value='b'*40):
                with self.assertRaisesRegex(AssertionError, 'another commit'):
                    ci.verify_bundle(folder)


if __name__ == '__main__':
    unittest.main()
