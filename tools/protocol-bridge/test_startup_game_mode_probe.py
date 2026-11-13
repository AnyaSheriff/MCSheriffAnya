"""Проверки декодера и сравнения по реальным Login/Abilities оригинала777."""
import copy
import json
import struct
import unittest
from pathlib import Path

import probe_startup_game_mode_777 as probe


class StartupModeProbeTests(unittest.TestCase):
    def setUp(self):
        self.reference = json.loads((Path(__file__).parent / 'testdata/startup-game-mode-original-777.json').read_text())

    def test_all_measured_startup_and_saved_observations_decode_from_wire(self):
        self.assertEqual(len(probe.compare_reports(self.reference, self.reference)), 17)
        expected = {'survival': 0, 'creative': 1, 'adventure': 2, 'spectator': 3,
                    'missing': 0, 'number': 1, 'uppercase': 0, 'unknown': 0,
                    'empty': 0, 'zero': 0, 'two': 2, 'three': 3, 'out-of-range': 0, 'negative': 0}
        self.assertEqual({r['label']: r['fresh']['mode'] for r in self.reference['cases']}, expected)
        saved = self.reference['cases'][0]
        self.assertEqual([saved[k]['mode'] for k in ['saved_after_restart', 'fresh_after_restart', 'fresh_after_runtime_override']], [2, 1, 3])

    def test_old_creative_default_cannot_pass_survival(self):
        candidate = copy.deepcopy(self.reference)
        candidate['cases'][0]['fresh'] = copy.deepcopy(candidate['cases'][1]['fresh'])
        with self.assertRaises(AssertionError):
            probe.compare_reports(candidate, self.reference)

    def test_capabilities_difference_cannot_pass_same_mode(self):
        candidate = copy.deepcopy(self.reference)
        value = candidate['cases'][1]['fresh']
        value['abilities']['flags'] = 1
        for packet in value['packets']:
            if packet['packet'] == 'abilities':
                packet['hex'] = '01' + packet['hex'][2:]
        with self.assertRaises(AssertionError):
            probe.compare_reports(candidate, self.reference)

    def test_missing_duplicate_and_unbacked_summary_rejected(self):
        for kind in ['missing', 'duplicate', 'summary', 'no-wire', 'failed']:
            with self.subTest(kind=kind):
                candidate = copy.deepcopy(self.reference)
                if kind == 'missing': candidate['cases'].pop()
                elif kind == 'duplicate': candidate['cases'].append(candidate['cases'][0])
                elif kind == 'summary': candidate['cases'][0]['fresh']['mode'] = 1
                elif kind == 'no-wire': candidate['cases'][0]['fresh']['packets'] = []
                else: candidate['status'] = 'failed'
                with self.assertRaises(AssertionError):
                    probe.compare_reports(candidate, self.reference)

    def test_truncated_mode_prefix_and_abilities_rejected(self):
        payload = bytes.fromhex(self.reference['cases'][0]['fresh']['packets'][0]['hex'])
        # Последний байт префикса — подтверждённый survival gamemode.
        # Находим границу префикса по первой успешной длине.
        success = next(i for i in range(len(payload) + 1) if self.decodes(payload[:i]))
        for cut in range(success):
            with self.subTest(cut=cut), self.assertRaises((ValueError, EOFError, IndexError, struct.error)):
                probe.login_mode(payload[:cut])
        for size in [0, 1, 8, 10]:
            with self.assertRaises(ValueError): probe.ability_values(bytes(size))

    @staticmethod
    def decodes(payload):
        try: probe.login_mode(payload)
        except (ValueError, EOFError, IndexError): return False
        return True


if __name__ == '__main__':
    unittest.main()
