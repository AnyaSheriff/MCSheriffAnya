"""Сырые частные776-записи: измерения, книга, пароль и сохранённые отказы."""
import io
import json
from pathlib import Path
import unittest

from probe_step1_dimensions_recipes_776 import recipe
from probe_white_concrete_slab import uint

ROOT = Path(__file__).parent / 'testdata/step1-774-776'


class Continued776Tests(unittest.TestCase):
    def test_both_primaries_have_real_portal_roundtrip_and_complete_books(self):
        for primary, folder, minimum in [(775, 'dimensions-recipes-v3-776-primary775', 1400),
                                          (777, 'dimensions-recipes-v2-776-primary777', 1600)]:
            with self.subTest(primary=primary):
                d = json.loads((ROOT / folder / 'result.json').read_text())
                self.assertEqual((d['status'], d['server_exit'], d['primary']), ('pass', 0, primary))
                self.assertFalse(d['public_admission'])
                self.assertEqual([v['name'] for v in d['dimensions']],
                                 ['minecraft:the_nether', 'minecraft:overworld'])
                self.assertGreater(sum(len(b['entries']) for b in d['books']), minimum)
                self.assertTrue(d['ghosts'])
                self.assertGreaterEqual(len(d['acks']), 3)
                self.assertEqual(d['crafted_cursor']['count'], 4)
                self.assertEqual(sum(v['count'] for v in d['crafted_inventory'].values() if v), 1)
                packets = d['packets']
                respawns = [i for i, row in enumerate(packets) if row['name'] == 'respawn']
                self.assertEqual(len(respawns), 2)
                for start, end in zip(respawns, respawns[1:] + [len(packets)]):
                    self.assertIn('map_chunk', [row['name'] for row in packets[start:end]])
                    self.assertIn('position', [row['name'] for row in packets[start:end]])
                # Прямые execute-in/tp не заменяют требуемое прохождение портала.
                self.assertFalse(any(command.startswith('execute in') for command in d['commands']))
                log = (ROOT / folder / 'server.log').read_text()
                self.assertIn('перешёл в minecraft:the_nether', log)
                self.assertIn('перешёл в minecraft:overworld', log)

    def test_password_barrier_retries_before_real_join_on_both_primaries(self):
        for primary in (775, 777):
            d = json.loads((ROOT / f'password-dialog-776-primary{primary}' / 'result.json').read_text())
            self.assertEqual((d['status'], d['server_exit']), ('pass', 0))
            self.assertEqual([p['registered'] for p in d['auth_prompts']], [False, True, True])
            self.assertNotEqual(d['auth_prompts'][1]['sha256'], d['auth_prompts'][2]['sha256'])
            self.assertEqual(d['checks'].count('password-clears-dialog-before-finish'), 2)
            self.assertIn('password-relog-wrong-rejected-correct-enters', d['checks'])
            self.assertFalse(d['public_admission'])

    def test_observed_ghost_is_fully_decoded_and_truncation_is_rejected(self):
        for folder in ('dimensions-recipes-v3-776-primary775', 'dimensions-recipes-v2-776-primary777'):
            d = json.loads((ROOT / folder / 'result.json').read_text())
            raw = next(bytes.fromhex(row['hex']) for row in d['packets'] if row['name'] == 'craft_recipe_response')
            stream = io.BytesIO(raw)
            self.assertEqual(uint(stream), 0)
            self.assertEqual(recipe(stream), d['ghosts'][0]['display'])
            self.assertEqual(stream.read(), b'')
            for end in range(1, len(raw)):
                stream = io.BytesIO(raw[:end])
                uint(stream)
                with self.assertRaises((AssertionError, ValueError, EOFError, KeyError)):
                    recipe(stream)


if __name__ == '__main__':
    unittest.main()
