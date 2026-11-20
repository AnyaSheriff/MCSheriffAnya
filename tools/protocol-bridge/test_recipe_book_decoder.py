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

    def test_775_initial_crafting_table_and_former_idset_builder(self):
        root = ROOT.parent / 'recipe-book-775'
        names = {int(k): v for k, v in json.loads((root / 'items.json').read_text()).items()}
        rows, _ = Decoder((root / 'initial-crafting-table.bin').read_bytes(), names, string_tags=True).book()
        self.assertEqual(rows[0]['id'], 171)
        self.assertEqual(rows[0]['recipe']['result'], [5, 'crafting_table', 1])
        self.assertTrue(all(slot[0] == 10 for slot in rows[0]['recipe']['ingredients']))
        with self.assertRaises((ValueError, UnicodeDecodeError)):
            Decoder((root / 'former-idset-builder.bin').read_bytes(), names, string_tags=True).book()

    def test_775_tag_is_string_and_777_tag_is_idset(self):
        self.assertEqual(Decoder(b'\x06\x05plank', NAMES, string_tags=True).slot(), [6, 'plank'])
        self.assertEqual(Decoder(b'\x06\x00\x05plank', NAMES).slot(), [6, {'tag': 'plank'}])

    def test_776_official_slot_tags_are_strings_inside_full_recipe(self):
        fixture = json.loads((ROOT.parent / 'recipe-book-tag-776.json').read_text())
        body = bytes.fromhex(fixture['body_hex'])
        items = {360: 'crafting_table'}
        rows, replace = Decoder(body, items, string_tags=True).book()
        self.assertFalse(replace)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['recipe']['ingredients'], [[6, 'minecraft:planks']] * 4)
        self.assertEqual(rows[0]['requirements'], [{'tag': 'minecraft:planks'}] * 4)
        with self.assertRaises(ValueError):
            Decoder(body, items).book()

    def test_trailing_and_invalid_item_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'trailing'):
            Decoder(b'\x00\x00\x00', NAMES).book()
        with self.assertRaisesRegex(ValueError, 'unknown item'):
            Decoder(b'\x04\x00', NAMES).slot()


if __name__ == '__main__':
    unittest.main()
