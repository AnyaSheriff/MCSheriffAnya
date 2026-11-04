"""Отрицательные проверки барьера; без сервера и сети."""
import unittest
from probe_startup_auth_777 import validate_boundary, validate_default_dialog


class AuthBoundaryGuards(unittest.TestCase):
    def test_show_dialog_777_rejects_clear_dialog_and_old_show_id(self):
        payload = b'mcsheriffanya:register password'
        validate_default_dialog(0x13, payload)
        for packet_id, data in [(0x12, payload), (0x12, b''), (0x11, payload), (0x13, b'wrong-dialog')]:
            with self.subTest(packet_id=packet_id, data=data), self.assertRaises(AssertionError):
                validate_default_dialog(packet_id, data)

    def test_unchanged_empty_and_existing_playerdata_are_allowed(self):
        for data in [{}, {'player.dat':'fixed-sha'}]:
            validate_boundary(0, 0, data, data)

    def test_join_listing_and_each_playerdata_change_fail(self):
        for args in [(1,0,{},{}), (0,1,{},{}), (0,0,{}, {'player':'new'}),
                     (0,0,{'player':'old'}, {'player':'new'}), (0,0,{'player':'old'}, {})]:
            with self.subTest(args=args), self.assertRaises(AssertionError):
                validate_boundary(*args)


if __name__ == '__main__': unittest.main()
