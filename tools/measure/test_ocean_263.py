"""Проверки знаменателей и явного отсутствия морских растений."""
import unittest
import contextlib
import io
import json
from pathlib import Path
import tempfile
from unittest.mock import patch

from ocean_263 import compare, coral_components, main


class OceanComparison(unittest.TestCase):
    def test_density_uses_own_biome_volume(self):
        a = {'biome_blocks': {'ocean': 1_000_000}, 'counts': {'ocean': {'kelp': 100}}}
        b = {'biome_blocks': {'ocean': 2_000_000}, 'counts': {'ocean': {'kelp': 200}}}
        row = next(r for r in compare(a, b) if r['metric'].endswith('/kelp'))
        self.assertEqual(row['ours'], row['vanilla'])
        self.assertFalse(row['outside'])

    def test_frozen_zero_rows_are_retained(self):
        a = {'biome_blocks': {'frozen_ocean': 1_000_000}, 'counts': {}}
        rows = compare(a, a)
        self.assertEqual(len(rows), 25)
        self.assertTrue(all(r['ours'] == r['vanilla'] == 0 for r in rows))

    def test_census_small_samples_are_flagged_and_not_hidden(self):
        a = {'biome_blocks': {'ocean': 65536}, 'counts': {'ocean': {'kelp': 10}}}
        b = {'biome_blocks': {'ocean': 65536}, 'counts': {}}
        row = next(r for r in compare(a, b) if r['metric'].endswith('/kelp'))
        self.assertTrue(row['outside'])
        self.assertTrue(row['small_sample'])

    def test_missing_population_is_a_failure(self):
        a = {'biome_blocks': {'ocean': 1_000_000}, 'counts': {}}
        b = {'biome_blocks': {'ocean': 1_000_000}, 'counts': {'ocean': {'kelp': 100}}}
        row = next(r for r in compare(a, b) if r['metric'].endswith('/kelp'))
        self.assertTrue(row['outside'])

    def test_diagonal_branches_form_one_component(self):
        shape = coral_components({'tube': {(5, 40, 5), (6, 41, 5), (7, 42, 5)}}, {(0, 0)})
        hist = shape['by_kind']['tube']['histograms']
        self.assertEqual(hist['blocks'], {'3': 1})
        self.assertEqual(hist['span_y'], {'3': 1})

    def test_unknown_diagonal_border_is_censored(self):
        shape = coral_components({'tube': {(15, 40, 15)}}, {(0, 0), (1, 0), (0, 1)})
        self.assertEqual(shape['by_kind']['tube']['clipped'], 1)
        self.assertEqual(shape['by_kind']['tube']['histograms']['blocks'], {})

    def test_target_reference_does_not_import_neighbor_biomes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            measured = root/'measured.json'
            measured.write_text(json.dumps({'statistics': {'seed': 1, 'chunks': 1,
                'biome_blocks': {'forest': 1000000}, 'counts': {'forest': {'kelp': 1}}}}))
            def report(volume, counts):
                return {'vanilla_level': {'seed': 1, 'version': {}}, 'statistics': {
                    'vanilla': {'dimensions': {'overworld': {'biomes_3d': volume,
                        'features_by_biome': counts}}}}}
            primary = root/'owner.json'
            primary.write_text(json.dumps(report({'ocean': 10000}, {'ocean': {'kelp': 10}})))
            target = root/'biome-cold_ocean.json'
            target.write_text(json.dumps(report({'cold_ocean': 10000, 'forest': 10000}, {
                'cold_ocean': {'kelp': 20}, 'forest': {'kelp': 100}})))
            output = root/'result.json'
            with patch('sys.argv', ['ocean_263.py', '--measured', str(measured),
                    '--official-reports', str(primary), str(target), '--output', str(output)]):
                with contextlib.redirect_stdout(io.StringIO()):
                    main()
            density = json.loads(output.read_text())['density_statistics']
            self.assertEqual(set(density['biome_blocks']), {'ocean', 'cold_ocean'})
            self.assertNotIn('forest', density['counts'])


if __name__ == '__main__':
    unittest.main()
