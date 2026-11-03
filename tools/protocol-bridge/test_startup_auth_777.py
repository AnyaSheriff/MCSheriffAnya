"""Отрицательные проверки барьера; без сервера и сети."""
import unittest
from probe_startup_auth_777 import validate_boundary


class AuthBoundaryGuards(unittest.TestCase):
    def test_unchanged_empty_and_existing_playerdata_are_allowed(self):
        for data in [{}, {'player.dat':'fixed-sha'}]:
            validate_boundary(0, 0, data, data)

    def test_join_listing_and_each_playerdata_change_fail(self):
        for args in [(1,0,{},{}), (0,1,{},{}), (0,0,{}, {'player':'new'}),
                     (0,0,{'player':'old'}, {'player':'new'}), (0,0,{'player':'old'}, {})]:
            with self.subTest(args=args), self.assertRaises(AssertionError):
                validate_boundary(*args)


if __name__ == '__main__': unittest.main()
