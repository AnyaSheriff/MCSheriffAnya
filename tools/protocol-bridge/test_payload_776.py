"""Schema graph: транзитивные отличия не маскируются совпавшим именем."""
import unittest
import json
from pathlib import Path
from generate_payload_776 import graph


class PayloadGraphTests(unittest.TestCase):
    def test_transitive_type_changes_are_visible(self):
        self.assertNotEqual(graph({'a': 'b', 'b': 'varint'}, 'a'),
                            graph({'a': 'b', 'b': 'i32'}, 'a'))

    def test_recursive_types_terminate_and_keep_definitions(self):
        types = {'a': ['array', {'type': 'a', 'countType': 'varint'}]}
        self.assertEqual(graph(types, 'a')['references'], types)

    def test_only_named_typed_remapper_boundaries_are_exempt(self):
        self.assertEqual(graph({'SlotComponentType': 'old'}, 'SlotComponentType'),
                         graph({'SlotComponentType': 'new'}, 'SlotComponentType'))
        self.assertNotEqual(graph({'Slot': 'old'}, 'Slot'), graph({'Slot': 'new'}, 'Slot'))

    def test_numeric_limits_and_field_order_are_preserved(self):
        for a, b in [(10, 11), ('varint', 'u8'), ([1, 2], [2, 1])]:
            self.assertNotEqual(graph({}, a), graph({}, b))

    def test_final_gameplay_traces_keep_both_primaries_and_failed_pickup_race(self):
        root = Path(__file__).parent / 'testdata/step1-774-776'
        for primary, folder in [(775, 'payload-regression-v2-776-primary775'),
                                (777, 'payload-regression-776-primary777')]:
            data = json.loads((root / folder / 'result.json').read_text())
            self.assertEqual((data['status'], data['primary'], data['server_exit']), ('pass', primary, 0))
            self.assertEqual(len(data['checks']), 11)
            self.assertIn('process-restart-player-and-chest-world-persistence', data['checks'])
            commands = data['commands']
            self.assertLess(commands.index('kill @e[type=item]'), commands.index('give McsaProbe diamond 3'))
            self.assertFalse(data['public_admission'])
        failed = json.loads((root / 'payload-regression-776-primary775/result.json').read_text())
        self.assertEqual(failed['status'], 'failed')
        # Предмет подобрался после pickup, до deposit: это не потеря трёх алмазов.
        inventory = failed['sessions'][0]['inventory']
        self.assertEqual(inventory['37']['count'], 3)
        self.assertEqual(inventory['36']['count'], 1)


if __name__ == '__main__':
    unittest.main()
