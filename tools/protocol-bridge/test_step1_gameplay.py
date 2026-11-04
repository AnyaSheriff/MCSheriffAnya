import struct
import unittest
from unittest.mock import patch

from probe_step1_gameplay_776 import GameplayProbe
from smoke import varint


def client(version=776):
    # Минимальная собственная таблица: проверки наблюдателя не требуют кэша.
    table = {'types': {'packet': ['container', [
        {'type': ['mapper', {'mappings': {'0x00': 'teleport_confirm'}}]}]]}}
    protocol = {'play': {'toClient': table, 'toServer': table}}
    return GameplayProbe(None, protocol, 1, version)


class Step1GameplayTests(unittest.TestCase):
    def test_health_history_keeps_damage_when_last_sample_has_regenerated(self):
        probe = client()
        for value in (20., 19., 20.):
            probe.observe_packet('update_health', struct.pack('>f', value) + bytes([20]) + struct.pack('>f', 5.))
        self.assertEqual(probe.health, 20.)
        self.assertEqual(probe.health_events, [20., 19., 20.])
        self.assertEqual(len(probe.raw), 3)
        self.assertTrue(all('hex' in row for row in probe.raw))

    def test_chest_window_and_cursor_are_observed_separately_from_player_inventory(self):
        probe = client()
        probe.observe_packet('open_window', bytes([6, 2, 0]))
        slots = [b'\0'] * 63
        slots[55] = bytes([3]) + varint(926) + bytes([0, 0])
        probe.observe_packet('window_items', bytes([6, 1, 63]) + b''.join(slots) + b'\0')
        self.assertEqual((probe.window, probe.menu, probe.revision), (6, 2, 1))
        self.assertEqual(probe.slots[55], {'item': 926, 'count': 3})
        self.assertEqual(probe.inventory, {})
        probe.observe_packet('set_cursor_item', slots[55])
        self.assertEqual(probe.cursor, {'item': 926, 'count': 3})
        probe.observe_packet('set_slot', bytes([6, 2, 0, 0]) + slots[55])
        self.assertEqual(probe.slots[0], probe.cursor)
        probe.observe_packet('set_cursor_item', b'\0')
        self.assertIsNone(probe.cursor)

    def test_teleport_confirmation_uses_each_peers_actual_layout(self):
        pose = [1., 65., 2., 3., 4.]
        for version in (775, 776, 777):
            probe = client(version)
            with patch.object(probe, 'send') as send:
                probe.confirm_teleport(129, pose)
            expected = varint(129)
            if version == 777:
                expected += struct.pack('>dddff', *pose)
            send.assert_called_once_with('teleport_confirm', expected)
            self.assertEqual(probe.acks, [129])
