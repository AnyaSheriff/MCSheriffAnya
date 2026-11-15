import contextlib
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from compare_parallel_alerts import copy_evidence, selected_cases, side_command
from run_side import candidate_settings, parse_args, parallel_flags_receipt


class AlertsProbeTests(unittest.TestCase):
    def test_exact_existing_twelve_not_filtered_by_candidate_support(self):
        cases = selected_cases()
        self.assertEqual(len(cases), 12)
        self.assertEqual(len({c['id'] for c in cases}), 12)
        self.assertEqual({c['mob'] for c in cases}, {'bee', 'dolphin'})
        self.assertTrue(all(c['reference_requirements'] and c['compare_fields'] for c in cases))

    def test_combined_argument_rejected_for_reference(self):
        args = ['--side', 'reference', '--output', 'own-out', '--cases-file', 'own-cases',
                '--data', 'own-data', '--source', 'own-head', '--port', '27820',
                '--jar', 'opaque-own-jar', '--jar-sha256', 'sha', '--combined-parallel']
        with patch.dict(os.environ, GITHUB_ACTIONS='true'), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                parse_args(args)
            candidate = args.copy()
            candidate[candidate.index('reference')] = 'candidate'
            candidate[candidate.index('--jar')] = '--binary'
            candidate[candidate.index('--jar-sha256')] = '--binary-sha256'
            self.assertTrue(parse_args(candidate).combined_parallel)

    def test_default_config_exact_and_combined_only_adds_two_flags(self):
        expected = ('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\n'
                    'auth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
        self.assertEqual(parallel_flags_receipt(False), {})
        self.assertEqual(parallel_flags_receipt(True), {'candidate_parallel_flags': {
            'parallel-dimension-ticks': True, 'experimental-parallel-mobs': True}})
        self.assertEqual(candidate_settings(), expected)
        self.assertEqual(candidate_settings(True), expected + 'parallel-dimension-ticks=true\nexperimental-parallel-mobs=true\n')

    def test_command_enables_only_candidate_and_ports_are_isolated(self):
        common = (Path('own-out'), Path('own-inputs'), Path('own-cases'), 'own-head', Path('own-elf'), 'elf-sha', 'jar-sha')
        reference = side_command('reference', *common)
        candidate = side_command('candidate', *common)
        self.assertNotIn('--combined-parallel', reference)
        self.assertIn('--combined-parallel', candidate)
        self.assertEqual(reference[reference.index('--port') + 1], '27820')
        self.assertEqual(candidate[candidate.index('--port') + 1], '27821')

    def test_evidence_does_not_copy_world_or_binary_or_jar(self):
        with tempfile.TemporaryDirectory() as d:
            source, output = Path(d) / 'source', Path(d) / 'output'
            (source / 'world').mkdir(parents=True)
            for name in ['side.json', 'server.log', 'commands.jsonl', 'runtime-elf', 'server.jar', 'world/side.json']:
                (source / name).write_bytes(b'own synthetic evidence')
            copy_evidence(source, output)
            self.assertEqual({str(p.relative_to(output)) for p in output.rglob('*') if p.is_file()},
                             {'side.json', 'server.log', 'commands.jsonl'})


if __name__ == '__main__':
    unittest.main()
