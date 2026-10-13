"""Synthetic census fixtures; no server or external implementation."""
import contextlib
import io
import sys
import unittest
from unittest.mock import patch

with patch.object(sys, 'argv', ['biome_census.py', 'unused']):
    import biome_census


class BiomeCensusTests(unittest.TestCase):
    def rows(self, cells):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            biome_census.report(cells)
        return [line.split('\t') for line in output.getvalue().splitlines()]

    def test_four_neighbours_and_disconnected_windows(self):
        rows = self.rows({(0, 0): ('plains', 64), (1, 0): ('plains', 65),
                          (2, 1): ('plains', 66), (8192, 0): ('plains', 67),
                          (0, 1): ('river', 60)})
        patches = {r[1]: r[2:] for r in rows if r[0] == 'patch'}
        self.assertEqual(patches['plains'][:4], ['3', '1', '1', '2'])
        shares = {r[1]: float(r[2]) for r in rows if r[0] == 'share'}
        self.assertEqual(shares, {'plains': 0.8, 'river': 0.2})

    def test_area_weighting_uses_cells_not_number_of_patches(self):
        cells = {(x, 0): ('ocean', 40) for x in range(10)}
        cells[(20, 0)] = ('ocean', 42)
        rows = self.rows(cells)
        self.assertIn(['patch_area', 'ocean', '160', '160'], rows)
        self.assertIn(['patch_summary', 'all_surface', '2', '5.500', '10', '1'], rows)

    def test_signed_coordinates_and_shared_border(self):
        rows = self.rows({(-1, -1): ('beach', -10), (0, -1): ('ocean', -20)})
        self.assertIn(['pair', 'beach', 'ocean', '1.0000'], rows)
        self.assertIn(['height', 'ocean', '-20', '-20', '-20', '-20', '-20'], rows)


if __name__ == '__main__':
    unittest.main()
