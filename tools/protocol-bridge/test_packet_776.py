"""Схемы сравниваются по точному имени и конкретной паре, без alias версии."""
import unittest
from generate_packet_776 import make_route


class PacketTests(unittest.TestCase):
    def test_same_id_does_not_alias_spectator_names(self):
        self.assertEqual(make_route({'spectate_entity': 0}, {'spectator_action': 0},
                                    {'spectate_entity': 'same'}, {'spectator_action': 'same'}),
                         [(0, 'spectate_entity', None, False)])

    def test_same_name_does_not_imply_equal_payload(self):
        self.assertEqual(make_route({'join': 0}, {'join': 0}, {'join': 'old'}, {'join': 'mid'}),
                         [(0, 'join', 0, False)])

    def test_same_layout_can_move_packet_id(self):
        self.assertEqual(make_route({'a': 0, 'b': 1}, {'b': 0, 'a': 1},
                                    {'a': 'a', 'b': 'b'}, {'a': 'a', 'b': 'b'}),
                         [(0, 'a', 1, True), (1, 'b', 0, True)])

    def test_equality_is_for_selected_pair(self):
        names = {'join': 0}
        old, mid, new = {'join': 'old'}, {'join': 'new'}, {'join': 'new'}
        self.assertFalse(make_route(names, names, old, mid)[0][3])
        self.assertTrue(make_route(names, names, mid, new)[0][3])
        self.assertFalse(make_route(names, names, old, new)[0][3])

    def test_holes_and_sentinel_rejected_on_both_sides(self):
        for malformed in [{'a': 1}, {'a': 0, 'b': 2}, {'a': 65535}, {'a': -1}, {'a': True}]:
            for source, target in [(malformed, {'a': 0}), ({'a': 0}, malformed)]:
                with self.assertRaises(ValueError):
                    make_route(source, target, {'a': 'a', 'b': 'b'}, {'a': 'a', 'b': 'b'})

    def test_missing_schema_is_not_equal_by_default(self):
        for source, target in [({}, {'a': 'a'}), ({'a': 'a'}, {})]:
            with self.assertRaises(ValueError):
                make_route({'a': 0}, {'a': 0}, source, target)


if __name__ == '__main__':
    unittest.main()
