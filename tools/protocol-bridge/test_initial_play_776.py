"""Offline parser tests; синтетические кадры не доказывают принятие клиентом."""
import copy
import json
import math
import struct
import unittest
from unittest.mock import MagicMock, patch
import probe_initial_play_776 as probe
from probe_registry_client import varint


def synthetic():
    entries = [{'name': name, 'body_hex': bytes(varint(probe.IDS[name])).hex()} for name in probe.SEQUENCE]
    for entry, (x, z) in zip(entries[4:13], ((x, z) for x in range(-1, 2) for z in range(-1, 2))):
        entry['body_hex'] = (b'\x2d' + struct.pack('>ii', x, z)).hex()
    entries[13]['body_hex'] = (b'\x48\x01' + struct.pack('>ddddddffI', .5, 65., .5, 0., 0., 0., 0., 0., 0)).hex()
    return {'schema': 1, 'native_protocol': 775, 'target_protocol': 776, 'payload_protocol': 776, 'online_mode': False,
            'teleport_id': 1, 'expected_pose': dict(x=.5, y=65., z=.5, yaw=0., pitch=0.), 'packets': entries}


class InitialTests(unittest.TestCase):
    def test_contract_and_nine_chunks(self):
        data = synthetic()
        parsed, bodies, pose = probe.fixture(json.dumps(data))
        self.assertEqual(parsed, data)
        self.assertEqual(len(bodies), 21)
        self.assertEqual(pose, (.5, 65., .5, 0., 0.))

    def test_wrong_version_order_id_and_chunks(self):
        base = synthetic()
        for mutation in [
            lambda d: d.update(target_protocol=777),
            lambda d: d.pop('payload_protocol'),
            lambda d: d['packets'].reverse(),
            lambda d: d['packets'][0].update(body_hex='32'),
            lambda d: d['packets'][5].update(body_hex=d['packets'][4]['body_hex']),
            lambda d: d.update(online_mode=True),
        ]:
            data = copy.deepcopy(base); mutation(data)
            with self.assertRaises(ValueError):
                probe.fixture(json.dumps(data))

    def test_bad_pose_relative_flags_and_truncation(self):
        base = synthetic()
        for mutation in [
            lambda d: d['expected_pose'].update(y=64.),
            lambda d: d['expected_pose'].update(y=math.inf),
            lambda d: d.update(teleport_id=2),
            lambda d: d['packets'][13].update(body_hex=d['packets'][13]['body_hex'][:-2] + '01'),
            lambda d: d['packets'][13].update(body_hex='4801'),
        ]:
            data = copy.deepcopy(base); mutation(data)
            with self.assertRaises((ValueError, struct.error)):
                probe.fixture(json.dumps(data))

    def test_short_ack_never_invents_pose(self):
        result = probe.read_play(b'\x00\x01', 1, (None,) * 5)
        self.assertEqual(result['teleport_ack'], 1)
        self.assertNotIn('pose', result)
        for body in [b'\x00', b'\x00\x02', b'\x00\x01\x00', b'\x00\x01' + bytes(28)]:
            with self.assertRaises((ValueError, EOFError)):
                probe.read_play(body, 1, (None,) * 5)

    def test_loaded_and_exact_pong(self):
        self.assertTrue(probe.read_play(b'\x2c', 1, ())['loaded'])
        self.assertEqual(probe.read_play(b'\x2d' + struct.pack('>i', probe.PING), 1, ())['pong'], probe.PING)
        for body in [b'\x2c\x00', b'\x2d' + bytes(4), b'\x2d' + struct.pack('>i', probe.PING) + b'\x00']:
            with self.assertRaises(ValueError):
                probe.read_play(body, 1, ())

    def test_ground_flag_without_position_remains_unknown(self):
        result = probe.read_play(b'\x21\x01', 1, (None,) * 5)
        self.assertTrue(result['ground'])
        self.assertEqual(result['pose'], (None,) * 5)

    def test_complete_and_partial_movement(self):
        pose = (.5, 65., .5, 0., 0.)
        body = b'\x1f' + struct.pack('>dddffB', *pose, 3)
        result = probe.read_play(body, 1, (None,) * 5)
        self.assertEqual(result['pose'], pose)
        self.assertTrue(result['ground'])
        result = probe.read_play(b'\x20' + struct.pack('>ffB', 90., -30., 1), 1, pose)
        self.assertEqual(result['pose'], (.5, 65., .5, 90., -30.))
        result = probe.read_play(b'\x1e' + struct.pack('>dddB', 1., 65., 2., 0), 1, pose)
        self.assertEqual(result['pose'], (1., 65., 2., 0., 0.))
        self.assertFalse(result['ground'])

    def test_movement_flags_nonfinite_tail_and_truncation(self):
        bodies = [b'\x21\x04', b'\x21\x01\x00', b'\x1f',
                  b'\x1e' + struct.pack('>dddB', math.nan, 65., .5, 1),
                  b'\x20' + struct.pack('>ffB', math.inf, 0., 1)]
        for body in bodies:
            with self.subTest(body=body), self.assertRaises((ValueError, EOFError)):
                probe.read_play(body, 1, (None,) * 5)

    def test_late_ack_cannot_reuse_stale_ground_state(self):
        report = dict(teleport_ack=True, player_loaded=True, post_play_pong=True, grounded=True)
        self.assertTrue(probe.play_complete(report, 10., 9.))
        self.assertFalse(probe.play_complete(report, 10., 8.))
        self.assertFalse(probe.play_complete(report, 10., None))
        for key in report:
            changed = dict(report); changed[key] = False
            self.assertFalse(probe.play_complete(changed, 10., 9.))

    def test_socket_pause_is_allowed_until_overall_deadline(self):
        connection = MagicMock()
        expected = object()
        with patch.object(probe.time, 'monotonic', return_value=5.), patch.object(
                probe, 'packet', return_value=expected):
            self.assertIs(probe.next_play_packet(connection, 45.), expected)
        connection.settimeout.assert_called_once_with(40.)
        with patch.object(probe.time, 'monotonic', return_value=46.), self.assertRaises(TimeoutError):
            probe.next_play_packet(connection, 45.)
        with patch.object(probe.time, 'monotonic', return_value=5.), patch.object(
                probe, 'packet', side_effect=probe.socket.timeout()) as read, self.assertRaises(probe.socket.timeout):
            probe.next_play_packet(connection, 45.)
        read.assert_called_once()


if __name__ == '__main__':
    unittest.main()
