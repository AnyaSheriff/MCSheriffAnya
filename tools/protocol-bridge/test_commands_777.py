"""Независимые проверки сетевой записи и сохранения до разбора."""
import json
from pathlib import Path
import tempfile
import unittest
from probe_commands_777 import Probe, tree


class CommandFixtureTests(unittest.TestCase):
    def test_nonop_packet_matches_decoded_fixture(self):
        root = Path(__file__).parent / 'testdata/commands-777'
        fixture = json.loads((root / 'official.json').read_text())
        self.assertEqual(tree((root / 'nonop.bin').read_bytes()), fixture['roles']['nonop']['tree'])

    def test_runtime_nonop_nodes_match_independent_recording(self):
        root = Path(__file__).resolve().parents[2]
        actual = json.loads((root / 'src/network/play/command_tree_nonop.json').read_text())
        recorded = json.loads((root / 'tools/protocol-bridge/testdata/commands-777/official.json').read_text())
        self.assertEqual(actual, recorded['roles']['nonop']['tree'])

    def test_nonop_stays_inside_native_775_parser_set(self):
        payload = (Path(__file__).parent / 'testdata/commands-777/nonop.bin').read_bytes()
        nodes = tree(payload)['nodes']
        # Числовые ID из закреплённой protocol-26.1.json: integer/string,
        # entity/message/objective/int_range; новых777-разборщиков здесь нет.
        native = {3, 5, 6, 20, 24, 39}
        self.assertTrue(all(node.get('parser') in native for node in nodes if 'parser' in node))
        self.assertTrue(all(node['flags'] & ~31 == 0 for node in nodes))

    def test_op4_packets_and_runtime_projection_for_both_protocols(self):
        from generate_command_fixtures import project, encode
        root = Path(__file__).resolve().parents[2]
        for protocol, suffix in ((777, ''), (775, 'native-775')):
            folder = root / 'tools/protocol-bridge/testdata/commands-777' / suffix
            observed = json.loads((folder / 'official.json').read_text())['roles']['op4']['tree']
            self.assertEqual(tree((folder / 'op4.bin').read_bytes()), observed)
            expected = project(observed)
            actual = json.loads((root / f'src/network/play/command_tree_op4_{protocol}.json').read_text())
            self.assertEqual(actual.pop('protocol'), protocol)
            self.assertEqual(actual, expected)
            self.assertEqual(encode(actual), (folder / 'op4-projected.bin').read_bytes())
            if protocol == 775:
                self.assertTrue(all(n.get('parser', 0) <= 56 for n in actual['nodes']))

    def test_extended_parser_59_does_not_desynchronize_nodes(self):
        payload = bytes([2, 0, 1, 1, 6, 0, 1, ord('x'), 59, 0])
        decoded = tree(payload)
        self.assertEqual(decoded['nodes'][1]['parser'], 59)
        self.assertEqual(decoded['nodes'][1]['properties'], '')
        self.assertEqual(decoded['root'], 0)
        with self.assertRaises(AssertionError):
            tree(payload + b'\x00')

    def test_recorder_saves_raw_packet_without_parsing_it(self):
        with tempfile.TemporaryDirectory(prefix='command-recorder-test-') as directory:
            probe = Probe.__new__(Probe)
            probe.output = Path(directory)
            probe.trees = []
            payload = b'\x01\x02\x00\x01x\x7f'
            probe.observe_packet('declare_commands', payload)
            self.assertEqual((probe.output / 'tree-0.bin').read_bytes(), payload)
            self.assertEqual(probe.trees, [payload])


if __name__ == '__main__':
    unittest.main()
