"""Регрессии полного списка ID палитры: не только используемые клетки."""
import io
import struct
import unittest
from probe_chunk_palettes import palette
from smoke import varint


class PaletteIds(unittest.TestCase):
    def test_sulfur_id_requires_the_67_entry_configuration(self):
        self.assertEqual(palette(io.BytesIO(b'\0' + varint(66)), 64, 1, 3, 7, range(67)), 1)
        with self.assertRaisesRegex(ValueError, 'unknown palette id 66'):
            palette(io.BytesIO(b'\0' + varint(66)), 64, 1, 3, 7, range(65))

    def test_unused_linear_palette_id_is_checked(self):
        data = b'\1\2\0' + varint(66) + bytes(8)
        with self.assertRaisesRegex(ValueError, 'unknown palette id 66'):
            palette(io.BytesIO(data), 64, 1, 3, 7, range(65))

    def test_invalid_indirect_index_is_rejected(self):
        data = b'\2\3\0\1\2' + struct.pack('>2Q', 3, 0)
        with self.assertRaisesRegex(ValueError, 'invalid palette index'):
            palette(io.BytesIO(data), 64, 1, 3, 7, range(67))

    def test_block_registry_is_checked(self):
        with self.assertRaisesRegex(ValueError, 'unknown palette id 40000'):
            palette(io.BytesIO(b'\0' + varint(40000)), 4096, 4, 8, 16, range(36000))

    def test_direct_biomes_and_truncation_are_checked(self):
        words = [66] + [0] * 7
        data = b'\7' + struct.pack('>8Q', *words)
        self.assertEqual(palette(io.BytesIO(data), 64, 1, 3, 7, range(67)), 64)
        with self.assertRaisesRegex(ValueError, 'unknown direct id 66'):
            palette(io.BytesIO(data), 64, 1, 3, 7, range(65))
        with self.assertRaisesRegex(ValueError, 'truncated palette'):
            palette(io.BytesIO(data[:-1]), 64, 1, 3, 7, range(67))


if __name__ == '__main__':
    unittest.main()
