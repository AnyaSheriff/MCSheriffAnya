"""Офлайн-тесты собственного кодека collection_slots и чистых классификаторов
захвата. Никаких серверов и импортов репозитория наверху."""
import io
import math
import unittest

import collection_slots as cs
from collections_probe import (classify_case, classify_response, exact_marker,
                              inventory_snapshot, observer_map, stale_new_marker,
                              finalize_case, through_receipt)


def varint(value):
    out = bytearray()
    while True:
        part = value & 0x7f
        value >>= 7
        if value:
            out.append(part | 0x80)
        else:
            out.append(part)
            return bytes(out)


def tag(kind, payload):
    return cs.decode_tag(io.BytesIO(payload), kind)


class NbtTest(unittest.TestCase):
    def test_all_twelve_types(self):
        cases = [
            (1, b'\x7f', {'type': 1, 'value': 127}),
            (2, b'\x7f\xff', {'type': 2, 'value': 32767}),
            (3, b'\x7f\xff\xff\xff', {'type': 3, 'value': 2147483647}),
            (3, b'\x80\x00\x00\x00', {'type': 3, 'value': -2147483648}),
            (4, b'\x7f\xff\xff\xff\xff\xff\xff\xff', {'type': 4, 'value': 9223372036854775807}),
            (4, b'\x80\x00\x00\x00\x00\x00\x00\x00', {'type': 4, 'value': -9223372036854775808}),
            (5, b'\x3f\xc0\x00\x00', {'type': 5, 'value': 1.5, 'raw_hex': '3fc00000'}),
            (6, b'\x3f\xf8\x00\x00\x00\x00\x00\x00', {'type': 6, 'value': 1.5,
                                                      'raw_hex': '3ff8000000000000'}),
            (7, b'\x00\x00\x00\x02\xff\x01', {'type': 7, 'value': [-1, 1]}),
            (8, b'\x00\x03ABC', {'type': 8, 'value': 'ABC'}),
            (9, b'\x03\x00\x00\x00\x02\x00\x00\x00\x01\xff\xff\xff\xff',
                {'type': 9, 'element_type': 3,
                 'values': [{'type': 3, 'value': 1}, {'type': 3, 'value': -1}]}),
            (10, b'\x01\x00\x01a\x05\x00', {'type': 10, 'value': {'a': {'type': 1, 'value': 5}}}),
            (11, b'\x00\x00\x00\x02\x00\x00\x00\x01\xff\xff\xff\xff', {'type': 11, 'value': [1, -1]}),
            (12, b'\x00\x00\x00\x01\x80\x00\x00\x00\x00\x00\x00\x00',
                 {'type': 12, 'value': [-9223372036854775808]}),
        ]
        for kind, raw, expected in cases:
            self.assertEqual(tag(kind, raw), expected)

    def test_kind_byte_is_read_from_stream(self):
        self.assertEqual(cs.decode_tag(io.BytesIO(b'\x01\x7f')), {'type': 1, 'value': 127})

    def test_float_negative_zero_bits(self):
        for kind, raw, hexs in [(5, b'\x80\x00\x00\x00', '80000000'),
                                (6, b'\x80\x00\x00\x00\x00\x00\x00\x00', '8000000000000000')]:
            value = tag(kind, raw)
            self.assertEqual(value['value'], -0.0)
            self.assertLess(math.copysign(1, value['value']), 0)
            self.assertEqual(value['raw_hex'], hexs)

    def test_nested_and_empty_containers(self):
        self.assertEqual(tag(9, b'\x00\x00\x00\x00\x00'), {'type': 9, 'element_type': 0, 'values': []})
        self.assertEqual(tag(9, b'\x09\x00\x00\x00\x01\x01\x00\x00\x00\x00'),
                         {'type': 9, 'element_type': 9,
                          'values': [{'type': 9, 'element_type': 1, 'values': []}]})
        self.assertEqual(tag(7, b'\x00\x00\x00\x00'), {'type': 7, 'value': []})
        self.assertEqual(tag(11, b'\x00\x00\x00\x00'), {'type': 11, 'value': []})
        self.assertEqual(tag(12, b'\x00\x00\x00\x00'), {'type': 12, 'value': []})

    def test_non_ascii_rejected(self):
        with self.assertRaises(AssertionError):
            tag(8, b'\x00\x02\xc3\xa9')

    def test_truncation_rejected(self):
        for kind, raw in [(1, b''), (2, b'\x7f'), (3, b'\x01\x02'), (4, b'\x01' * 7),
                          (5, b'\x3f\xc0'), (6, b'\x3f' * 6), (8, b'\x00\x05ab')]:
            with self.assertRaises(AssertionError):
                tag(kind, raw)

    def test_bad_lengths_rejected(self):
        with self.assertRaises(AssertionError):
            tag(7, b'\xff\xff\xff\xff')               # отрицательная длина массива
        with self.assertRaises(AssertionError):
            tag(9, b'\x02\xff\xff\xff\xff')           # отрицательная длина списка
        with self.assertRaises(AssertionError):
            tag(9, b'\x00\x00\x00\x01')               # LIST из TAG_End
        with self.assertRaises(AssertionError):
            tag(7, b'\x00\x01\x00\x00')               # длина больше лимита объёма
        with self.assertRaises(AssertionError):
            cs.read_varint(io.BytesIO(b'\x80\x80\x80\x80\x80\x80').read)

    def test_depth_limit(self):
        deep = b'\x0a\x00\x00' * 40 + b'\x00' * 41
        with self.assertRaises(AssertionError):
            tag(10, deep)
        ok = b'\x0a\x00\x00' * 32 + b'\x00' * 33
        self.assertEqual(tag(10, ok)['type'], 10)

    def test_total_and_node_budget(self):
        huge = b'\x02\x11\x11\x02' * 70000             # список пустых compound
        with self.assertRaises(AssertionError):
            tag(9, b'\x0a\x00\x00\x11\x11' + huge[:800000])
        with self.assertRaises(AssertionError):
            cs.decode_custom_data(io.BytesIO(b'\x0a' + b'\x05' * 80000))

    def test_custom_data_marker(self):
        fields = cs.decode_custom_data(io.BytesIO(b'\x0a\x08\x00\x06marker\x00\x03NEW\x00'))
        self.assertEqual(fields, {'marker': {'type': 8, 'value': 'NEW'}})
        with self.assertRaises(AssertionError):
            cs.decode_custom_data(io.BytesIO(b'\x08\x00\x06marker\x00\x03NEW\x00'))

    def test_duplicate_keys_rejected(self):
        with self.assertRaises(AssertionError):
            tag(10, b'\x01\x00\x01a\x05\x01\x00\x01a\x07\x00')

    def test_varint(self):
        for raw, value in [(b'\x00', 0), (b'\xac\x02', 300),
                           (b'\xff\xff\xff\xff\x07', 2147483647),
                           (b'\xff\xff\xff\xff\x0f', -1)]:
            self.assertEqual(cs.read_varint(io.BytesIO(raw).read), value)
        with self.assertRaises(AssertionError):
            cs.read_varint(io.BytesIO(b'\xff\xff\xff\xff\x1f').read)


ITEMS = [{'id': 5, 'name': 'stone'}]
MARKER_NBT = b'\x0a\x08\x00\x06marker\x00\x03NEW\x00'
MARKER_HEX = MARKER_NBT.hex()


def slot_bytes(custom=MARKER_NBT, kinds=(0,), trailing=b''):
    out = varint(1) + varint(5) + varint(len(kinds)) + varint(0)
    for kind in kinds:
        out += varint(kind)
        if kind == 0:
            out += custom
        elif kind == 3:
            out += varint(7)
        elif kind == 13:
            out += varint(1) + varint(7) + varint(3)
    return out + trailing


class SlotTest(unittest.TestCase):
    def _slot(self, raw):
        stream = io.BytesIO(raw)
        value = cs.SlotDecoder(ITEMS).slot(stream)
        if stream.read():
            raise AssertionError('trailing bytes после отдельного слота')
        return value

    def test_known777_item_marker(self):
        slot = self._slot(slot_bytes())
        self.assertEqual(slot['item'], 'stone')
        self.assertEqual(slot['count'], 1)
        self.assertEqual(slot['custom_data'], {'marker': {'type': 8, 'value': 'NEW'}})
        self.assertEqual(slot['custom_data_raw_hex'], MARKER_HEX)

    def test_damage_and_enchantments(self):
        custom = b'\x0a\x08\x00\x06marker\x00\x02AB\x00'
        slot = self._slot(slot_bytes(custom=custom, kinds=(3, 13)))
        self.assertEqual(slot['damage'], 7)
        self.assertEqual(slot['enchantments'], [[7, 3]])

    def test_empty_slot(self):
        self.assertIsNone(self._slot(varint(0)))

    def test_unknown_component_rejected(self):
        with self.assertRaises(AssertionError):
            self._slot(slot_bytes(kinds=(99,)))

    def test_duplicate_component_rejected(self):
        with self.assertRaises(AssertionError):
            self._slot(slot_bytes(kinds=(3, 3)))

    def test_removed_component_rejected(self):
        raw = varint(1) + varint(5) + varint(0) + varint(1) + varint(0)
        with self.assertRaises(AssertionError):
            self._slot(raw)

    def test_count_bound(self):
        with self.assertRaises(AssertionError):
            self._slot(varint(100) + varint(5) + varint(0) + varint(0))

    def test_trailing_rejected(self):
        with self.assertRaises(AssertionError):
            self._slot(slot_bytes(trailing=b'\xff'))

    def test_unknown_item_rejected(self):
        with self.assertRaises(AssertionError):
            self._slot(varint(1) + varint(4) + varint(0) + varint(0))


def replay(events):
    return inventory_snapshot(events, ITEMS, lambda code: code)


class ReplayTest(unittest.TestCase):
    def test_replay_46_slots(self):
        events = [dict(name='window_items', at=0, hex=(varint(0) + varint(1) + varint(46)
                       + varint(0) * 46 + varint(0)).hex())]
        snap = replay(events)
        self.assertEqual(len(snap['slots']), 46)
        self.assertTrue(all(s is None for s in snap['slots']))
        self.assertEqual(observer_map([], ITEMS, 1), {})
        self.assertEqual(equipment_free(), [])


def equipment_free():
    from collections_probe import equipment_snapshot
    return equipment_snapshot([dict(name='other', at=0, hex='')], ITEMS)


class ClassifierTest(unittest.TestCase):
    def test_exact_replaced_only(self):
        self.assertEqual(classify_response('Replaced 1 slot(s) on blackbox with [Stone]'), 'accepted')
        self.assertEqual(classify_response('Replaced 2 slot(s) on blackbox with [Stone]'), 'invalid')
        self.assertEqual(classify_response('Could not replace item'), 'rejected-observed')
        self.assertEqual(classify_response(''), 'invalid')
        self.assertEqual(classify_response(None), 'invalid')

    def test_marker_kinds_prevent_control_false_positive(self):
        control = {'item': 'stone', 'count': 1,
                   'custom_data': {'marker': {'type': 8, 'value': 'CONTROL_x'}},
                   'custom_data_raw_hex': MARKER_HEX}
        fresh = {'item': 'stone', 'count': 1,
                 'custom_data': {'marker': {'type': 8, 'value': 'NEW_x'},
                                 'payload': {'type': 9, 'element_type': 0, 'values': []}},
                 'custom_data_raw_hex': MARKER_HEX}
        self.assertTrue(exact_marker(control, 'CONTROL', 'x'))
        self.assertFalse(exact_marker(control, 'NEW', 'x'))
        self.assertTrue(exact_marker(fresh, 'NEW', 'x'))
        self.assertFalse(exact_marker({'custom_data': {'marker': {'type': 8, 'value': 'NEW_x'}}}, 'NEW', 'x'))
        self.assertFalse(exact_marker(fresh, 'CONTROL', 'other'))
        self.assertFalse(exact_marker(None, 'NEW', 'x'))
        self.assertTrue(stale_new_marker({'custom_data': {'marker': {'type': 8, 'value': 'NEW_y'}}}, 'x'))
        self.assertFalse(stale_new_marker(fresh, 'x'))

    def test_classify_case(self):
        self.assertEqual(classify_case('accepted', True), 'accepted')
        self.assertEqual(classify_case('rejected-observed', True), 'rejected-observed')
        self.assertEqual(classify_case('accepted', False), 'invalid')
        self.assertEqual(classify_case('invalid', True), 'invalid')


class BarrierStateTest(unittest.TestCase):
    @staticmethod
    def stack(kind):
        text = (kind + '_x').encode('ascii')
        nbt = b'\x0a\x08\x00\x06marker' + len(text).to_bytes(2, 'big') + text
        if kind == 'NEW':
            nbt += b'\x09\x00\x07payload\x00\x00\x00\x00\x00'
        return slot_bytes(custom=nbt + b'\x00')

    def owner(self, kind, at):
        frame = varint(0) + varint(1) + varint(46)
        frame += b''.join(self.stack(kind) if i == 36 else varint(0) for i in range(46))
        return dict(name='window_items', at=at, hex=(frame + varint(0)).hex())

    def observer(self, kind, at):
        return dict(name='entity_equipment', at=at,
                    hex=(varint(1) + b'\x00' + self.stack(kind)).hex())

    def test_delayed_change_before_end_invalidates_rejected_result(self):
        control = self.owner('CONTROL', 1)
        before = replay([control])
        markers = [dict(name='system_chat', at=4, hex='01'),
                   dict(name='system_chat', at=4, hex='02')]
        events = [[control, self.owner('NEW', 3), markers[0]],
                  [self.observer('CONTROL', 1), self.observer('NEW', 3), markers[1]]]
        with self.assertRaises(AssertionError):
            finalize_case(before, events, dict(receipts=markers), ITEMS,
                          lambda code: code, 1, 'x', 'rejected-observed')

    def test_future_change_after_end_is_excluded(self):
        control = self.owner('CONTROL', 1)
        before = replay([control])
        markers = [dict(name='system_chat', at=4, hex='01'),
                   dict(name='system_chat', at=4, hex='02')]
        events = [[control, markers[0], self.owner('NEW', 5)],
                  [self.observer('CONTROL', 1), markers[1], self.observer('NEW', 5)]]
        after, _ = finalize_case(before, events, dict(receipts=markers), ITEMS,
                                 lambda code: code, 1, 'x', 'rejected-observed')
        self.assertEqual(after['slots'], before['slots'])

    def test_accepted_marker_must_still_exist_at_end(self):
        control = self.owner('CONTROL', 1)
        before = replay([control])
        markers = [dict(name='system_chat', at=4, hex='01'),
                   dict(name='system_chat', at=4, hex='02')]
        events = [[control, self.owner('NEW', 2), self.owner('CONTROL', 3), markers[0]],
                  [self.observer('NEW', 2), self.observer('CONTROL', 3), markers[1]]]
        with self.assertRaises(AssertionError):
            finalize_case(before, events, dict(receipts=markers), ITEMS,
                          lambda code: code, 1, 'x', 'accepted')

    def test_missing_or_duplicate_receipt_is_invalid(self):
        receipt = dict(name='system_chat', at=4, hex='01')
        for events in [[], [receipt, receipt]]:
            with self.assertRaises(AssertionError):
                through_receipt(events, receipt)


if __name__ == '__main__':
    unittest.main()
