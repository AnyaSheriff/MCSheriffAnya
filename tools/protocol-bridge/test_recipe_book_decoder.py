import collections
import json
from pathlib import Path
import unittest
from recipe_book_decoder import Decoder

ROOT = Path(__file__).parent / 'testdata/recipe-book-777'
NAMES = {int(k): v for k, v in json.loads((ROOT / 'items.json').read_text()).items()}
OFFICIAL = (ROOT / 'official-all.bin').read_bytes()


class RecipeDecoderTests(unittest.TestCase):
    def test_official_all_recipes(self):
        decoder = Decoder(OFFICIAL, NAMES)
        rows, replace = decoder.book()
        self.assertEqual(len(rows), 1720)
        self.assertFalse(replace)
        self.assertEqual(collections.Counter(row['recipe']['kind'] for row in rows).keys(), {0, 1, 2, 3, 4})
        self.assertIn(10, {kind for start, end, kind in decoder.offsets})
        self.assertIn(8, {kind for start, end, kind in decoder.offsets})

    def test_every_official_entry_as_singleton_and_truncated(self):
        rows, _ = Decoder(OFFICIAL, NAMES).book()
        for row in rows:
            body = b'\x01' + OFFICIAL[row['start']:row['end']] + b'\x00'
            with self.subTest(ident=row['id']):
                self.assertEqual(len(Decoder(body, NAMES).book()[0]), 1)
                with self.assertRaises((ValueError, EOFError)):
                    Decoder(body[:-1], NAMES).book()

    def test_invalid_identifier_reproduces_client_error_shape(self):
        with self.assertRaises(UnicodeDecodeError):
            Decoder(b'\x00\x02\xaf\x01', NAMES).idset()

    def test_trailing_and_invalid_item_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'trailing'):
            Decoder(b'\x00\x00\x00', NAMES).book()
        with self.assertRaisesRegex(ValueError, 'unknown item'):
            Decoder(b'\x04\x00', NAMES).slot()


if __name__ == '__main__':
    unittest.main()
