"""Проверки настоящих описаний, возраста урона и изоляции запуска."""
import contextlib
import copy
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from allay_cases import allay_cases
from analyze_allay_regen import analyze
from probe_allay_regeneration import main, side_command
from test_analyze_allay_regen import fixture


class AllayProbeTests(unittest.TestCase):
    def test_actual_factory_agrees_with_analyzer_contract(self):
        reference, candidate, definitions = fixture()
        cases = allay_cases()
        self.assertTrue(analyze(reference, candidate, cases)['recording_valid'])
        self.assertEqual([(c['damage_age'], c['damage_amount']) for c in cases],
                         [(age, amount) for age in (2, 7, 12) for amount in (1, 4)])
        self.assertEqual([c['id'] for c in cases], [c['id'] for c in definitions])

    def test_damage_commands_and_ages_include_two_setup_ticks(self):
        for case in allay_cases():
            phases = case['phases']
            self.assertEqual(2 + phases[0]['ticks'], case['damage_age'])
            self.assertEqual(phases[1]['ticks'], 0)
            self.assertEqual(phases[1]['commands'][0].format(target='@e[tag=own]'),
                             f'damage @e[tag=own] {case["damage_amount"]} minecraft:generic')
            self.assertEqual(sum(p['ticks'] for p in phases[2:]), 40)
            self.assertTrue(all(p['ticks'] == 1 and not p['commands'] for p in phases[2:]))
            self.assertIn('Health:20.0f', case['nbt'])
            self.assertIn('NoAI:1b', case['nbt'])

    def test_side_command_does_not_enable_unrelated_parallel_modes(self):
        common = (Path('own-out'), Path('own-in'), Path('own-cases'), 'own-sha', Path('own-elf'), 'elf-sha', 'jar-sha')
        ref = side_command('reference', *common)
        ours = side_command('candidate', *common)
        self.assertIn('--jar', ref)
        self.assertNotIn('--binary', ref)
        self.assertIn('--binary', ours)
        self.assertNotIn('--jar', ours)
        self.assertNotIn('--combined-parallel', ours)
        self.assertEqual(ref[ref.index('--port') + 1], '27920')
        self.assertEqual(ours[ours.index('--port') + 1], '27921')

    def test_local_runtime_rejected_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'must-not-exist'
            argv = ['probe', '--binary', 'does-not-exist', '--binary-source', 'own', '--output', str(output)]
            with patch.dict(os.environ, {'GITHUB_ACTIONS': 'false'}), patch('sys.argv', argv), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    main()
            self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
