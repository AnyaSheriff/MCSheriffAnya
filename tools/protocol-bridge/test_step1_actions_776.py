"""Сверяем разрешённые узкие случаи с сырыми ответами оригинала776."""
import json
from pathlib import Path
import unittest

from probe_white_concrete_slab import pack_position


class ObservedActions776Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).parent / 'testdata/step1-774-776/official-actions-776'
        cls.report = json.loads((cls.root / 'result.json').read_text())

    def test_finish_two_changes_authored_stone_to_air_and_other_candidates_do_not(self):
        report = self.report
        self.assertEqual(report['protocol'], 776)
        self.assertEqual(report['jar_sha256'], 'cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5')
        air = (pack_position((2, 65, 0)) + b'\0').hex()
        for candidate in report['candidates']:
            changes = [r['hex'] for r in candidate['records'] if r['name'] == 'block_change']
            self.assertEqual(air in changes, candidate['end'] == 2)
        self.assertIn('ACTION_2_AIR', (self.root / 'server.log').read_text())
        sent = report['sessions'][0]['sent']
        self.assertTrue(any(v['name'] == 'block_dig' and v['hex'] ==
                            (b'\x00' + pack_position((2, 65, 0)) + b'\x01\x04').hex() for v in sent))
        self.assertTrue(any(v['name'] == 'block_dig' and v['hex'] ==
                            (b'\x02' + pack_position((2, 65, 0)) + b'\x01\x05').hex() for v in sent))

    def test_relogged_chest_one_has_empty_network_nbt_and_observer_failure_is_retained(self):
        self.assertEqual(self.report['sessions'][1]['chests'], [{'kind': 1, 'nbt': None, 'hex': '00'}])
        self.assertEqual(self.report['status'], 'failed')
        self.assertIn('unexpected block entity in test chunk', self.report['error'])
        self.assertIn(self.report['server_exit'], (143, -15))


if __name__ == '__main__':
    unittest.main()
