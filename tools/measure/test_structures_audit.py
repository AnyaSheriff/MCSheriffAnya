"""Проверки геометрии переписи без процесса Minecraft."""
import unittest
from structures_audit import window, biome, inside


class AuditTests(unittest.TestCase):
    def test_windows(self):
        self.assertEqual(window(-64,-64),'0,0')
        self.assertEqual(window(63,63),'0,0')
        self.assertIsNone(window(64,0))
        self.assertEqual(window(480,480),'1,1')
        self.assertIsNone(window(544,512))
        self.assertIsNone(window(100,100))

    def test_signed_section_and_quart_cell(self):
        root={'sections':[{'Y':255,'biomes':{'palette':['minecraft:deep_dark']}}]}
        self.assertEqual(biome(root,-1,-1,-1),'deep_dark')
        self.assertEqual(biome(root,0,0,0),'unknown')

    def test_bounds_are_inclusive(self):
        self.assertTrue(inside([-1,-4,2],[-1,-4,2,3,6,7]))
        self.assertFalse(inside([-2,-4,2],[-1,-4,2,3,6,7]))


if __name__=='__main__': unittest.main()
