"""Проверки оболочек на искусственных примерах, без генератора мира."""
import unittest
from underground_features import components, encode, decode
from underground_comparison import dungeon_summary

class Components(unittest.TestCase):
    def test_negative_coordinates_and_vertical_boundaries(self):
        for pos in [(-8704,-64,8703),(0,319,-1),(8192,0,-8192)]:
            self.assertEqual(decode(encode(*pos)),pos)

    def test_adjacent_chunks_connect_but_diagonals_do_not(self):
        cells = {encode(-1,16,15):0,encode(0,16,15):2,
                 encode(0,16,16):3,encode(1,17,16):1}
        found = components(cells)
        self.assertEqual(sorted(c['total'] for c in found),[1,3])
        self.assertEqual(sum(c['blocks']['budding_amethyst'] for c in found),1)

    def test_calcite_only_deposits_are_not_geodes(self):
        self.assertEqual(components({encode(0,0,0):2,encode(1,0,0):3}),[])

    def test_dungeon_species_and_negative_height_bins(self):
        rows = [dict(pos=[0,-1,0],entity='minecraft:zombie'),
                dict(pos=[0,0,0],entity='skeleton'),
                dict(pos=[0,10,0],entity='minecraft:silverfish'),
                dict(pos=[0,20,0],entity=None)]
        summary = dungeon_summary(rows)
        self.assertEqual(summary['count'],2)
        self.assertEqual(summary['histogram'],{-16:1,0:1})

if __name__ == '__main__':
    unittest.main()
