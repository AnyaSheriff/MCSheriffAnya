"""Pure parser tests and selected packet replay; never launches a server.

Synthetic payload controls below check parsers only, not new runtime evidence.
"""
import io
import json
import math
from pathlib import Path
import struct
import unittest

import probe_wool_steps_777 as probe


def uint(value):
    result = bytearray()
    while True:
        byte = value & 127
        value >>= 7
        result.append(byte | (128 if value else 0))
        if not value:
            return bytes(result)


def metadata(payload, *, at=2):
    return [dict(direction='in', name='entity_metadata', hex=payload.hex(), at=at)]


class WoolStepsParsers(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.selected = json.loads((Path(__file__).parent / 'observations/wool-steps-selected.json').read_text())

    def test_server_position_accepts_finite_three_component_vectors(self):
        self.assertEqual(probe.parse_entity_data('Pos', '[0.5d, 65.0625d, -2d]'), [.5, 65.0625, -2])
        self.assertEqual(probe.parse_entity_data('Pos', '[1e1, -.5, +2]'), [10, -.5, 2])

    def test_server_position_rejects_wrong_shape_nonfinite_or_tail(self):
        for value in ['[1,2]', '[1,2,3,4]', '[NaNd,2d,3d]', '[1e999,2,3]', '[1,2,3] junk', '']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                probe.parse_entity_data('Pos', value)

    def test_ground_has_no_truthy_or_missing_default(self):
        for text, expected in [('1b', True), ('true', True), ('0b', False), ('false', False)]:
            self.assertIs(probe.parse_entity_data('OnGround', text), expected)
        for value in ['', '2b', '1', 'TRUE', 'false junk']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                probe.parse_entity_data('OnGround', value)

    def test_selected_actual_observer_packets_recover_each_recorded_path(self):
        # Replay only committed own selected packets; no local full-report path.
        rows = self.selected['observer_raw_movement']
        entity = self.selected['actor_entity_id']
        spawn = next(row for row in rows if row['name'] == 'spawn_entity')
        data = io.BytesIO(bytes.fromhex(spawn['hex']))
        self.assertEqual(probe.uint31(data), entity)
        probe.exact(data, 16)
        player_kind = probe.uint31(data)
        events = probe.observe_actor(rows, player_kind)
        for case in self.selected['cases']:
            with self.subTest(case=case['label']):
                recorded = case['observer_path']
                actual = [event for event in events if case['start'] <= event['at'] <= recorded[-1]['at']]
                self.assertEqual(actual, recorded)
                self.assertTrue(all(abs(row['position'][1] - case['feet']) <= .03 for row in actual))
                self.assertTrue(all(abs(a-b) <= .04 for a,b in zip(actual[-1]['position'], case['after']['position'])))

    def test_actual_relative_packets_reject_truncation_and_trailing_bytes(self):
        checked = 0
        for row in self.selected['observer_raw_movement']:
            if row['name'] not in ('rel_entity_move', 'entity_move_look'):
                continue
            data = io.BytesIO(bytes.fromhex(row['hex']))
            probe.uint31(data)
            payload = data.read()
            probe.decode_movement(row['name'], io.BytesIO(payload))
            for malformed in [payload[:-1], payload+b'\0']:
                with self.assertRaises(ValueError):
                    probe.decode_movement(row['name'], io.BytesIO(malformed))
            checked += 1
        self.assertGreater(checked, 100)

    def test_relative_step_counts_and_varints_are_bounded(self):
        for payload in [uint(1025 << 1), uint(3)+uint(1), b'\xff\xff\xff\xff\x0f', b'\x80'*6]:
            with self.subTest(payload=payload.hex()), self.assertRaises(ValueError):
                probe.decode_movement('rel_entity_move', io.BytesIO(payload))
        ground, rows = probe.decode_movement('rel_entity_move', io.BytesIO(uint(3)+uint(2)+struct.pack('>hhh', 1, -2, 3)))
        self.assertTrue(ground)
        self.assertEqual(rows, [dict(delta=[1, -2, 3], tick_offset=2)])

    def test_sync_linear_and_stepped_synthetic_parser_controls(self):
        linear = uint(0)+struct.pack('>dddffB', 1, 65, 2, 0, 0, 1)
        stepped = uint(1)+uint(1)+struct.pack('>ddd', 1, 65, 2)+uint(2)+struct.pack('>ffB', 0, 0, 0)
        self.assertEqual(probe.decode_movement('sync_entity_position', io.BytesIO(linear)),
                         (True, [dict(position=[1,65,2], tick_offset=0)]))
        self.assertEqual(probe.decode_movement('sync_entity_position', io.BytesIO(stepped)),
                         (False, [dict(position=[1,65,2], tick_offset=2)]))

    def test_sync_rejects_bad_tag_count_nonfinite_ground_and_tail(self):
        valid = uint(0)+struct.pack('>dddffB', 1, 65, 2, 0, 0, 1)
        bad = [uint(2), uint(1)+uint(0), uint(1)+uint(1025), uint(1)+b'\xff\xff\xff\xff\x0f',
               uint(1)+uint(1)+b'\0'*4, valid[:-1], valid+b'\0',
               uint(0)+struct.pack('>dddffB', math.nan, 65, 2, 0, 0, 1),
               uint(0)+struct.pack('>dddffB', 1, 65, 2, math.inf, 0, 1),
               uint(0)+struct.pack('>dddffB', 1, 65, 2, 0, 0, 2)]
        for payload in bad:
            with self.subTest(payload=payload.hex()), self.assertRaises(ValueError):
                probe.decode_movement('sync_entity_position', io.BytesIO(payload))

    def test_sneak_metadata_requires_selected_flags_and_pose(self):
        for flags, pose in [(2, 5), (0, 0)]:
            values, evidence = probe.metadata_sneak(metadata(bytes([4,0,0,flags,6,20,pose,255])), 4, 1)
            self.assertEqual(values, dict(flags=flags, pose=pose))
            self.assertEqual(len(evidence), 1)
        # An unrelated entity or stale metadata never proves requested state.
        self.assertEqual(probe.metadata_sneak(metadata(bytes([4,0,0,2,6,20,5,255])), 5, 1), ({}, []))
        self.assertEqual(probe.metadata_sneak(metadata(bytes([4,0,0,2,6,20,5,255]), at=0), 4, 1), ({}, []))
        self.assertEqual(probe.metadata_sneak([], 4, 1), ({}, []))

    def test_metadata_rejects_wrong_serializers_duplicates_missing_bytes_and_tail(self):
        bad = [bytes([4,0,0,2,0,0,2,255]), bytes([4,0,1,2,255]), bytes([4,6,0,5,255]),
               bytes([4,0,0,2,255,0]), bytes([4,6,20]), bytes([4,6,99,255]),
               bytes([4,3,8,2,255]), bytes([4,0,0,2])]
        for payload in bad:
            with self.subTest(payload=payload.hex()), self.assertRaises(ValueError):
                probe.metadata_sneak(metadata(payload), 4, 1)


if __name__ == '__main__':
    unittest.main()
