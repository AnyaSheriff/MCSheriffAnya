"""Отрицательные проверки барьера; без сервера и сети."""
import unittest
from probe_startup_auth_777 import parse_list_count, validate_boundary, validate_default_dialog


class AuthBoundaryGuards(unittest.TestCase):
    def test_console_list_exact_formats_and_player_counts(self):
        for text, expected in [('There are 0 of a max of 20 players online: ', 0),
                               ('There are 1 of a max of 20 players online: Alice', 1),
                               ('There are 2 of a max of 20 players online: Alice, Bob_2', 2),
                               ('Игроков на сервере: 0 из 20', 0),
                               ('Игроков на сервере: 1 из 20: Alice', 1)]:
            for prefix in ['', '[15:22:11] [Server thread/INFO]: ']:
                with self.subTest(text=text, prefix=prefix):
                    self.assertEqual(parse_list_count(prefix+text), expected)

    def test_console_list_rejects_nonzero_and_unrelated_or_malformed_text(self):
        for text in ['There are 1 of a max of 20 players online: Alice', 'Игроков на сервере: 1 из 20: Alice']:
            with self.subTest(text=text), self.assertRaises(AssertionError):
                validate_boundary(0, parse_list_count(text), {}, {})
        for text in ['0', 'There are 0', 'prefix There are 0 of a max of 20 players online: ',
                     'There are 0 of a max of 20 players online: Alice',
                     'There are 1 of a max of 20 players online: ',
                     'There are 00 of a max of 20 players online: ',
                     'There are 2 of a max of 20 players online: Alice',
                     'Игроков на сервере: 0 из 20: Alice', 'Игроков на сервере: 0 из 20 trailing']:
            with self.subTest(text=text):
                self.assertIsNone(parse_list_count(text))

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
