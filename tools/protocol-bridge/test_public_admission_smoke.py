"""Проверки положительного PlayerInfo receipt; без сервера и сети."""
import unittest
from probe_public_776 import add_player_info
from smoke import varint


class PlayerInfoReceipts(unittest.TestCase):
    def snapshot(self):
        name=b'Mcsa769'
        return bytes([13,1])+bytes([37])*16+varint(len(name))+name+bytes([0,0,1])

    def test_actual_writer_layout_requires_complete_uuid_name_mode_and_listed(self):
        self.assertEqual(add_player_info(self.snapshot()),[dict(uuid='25'*16,name='Mcsa769',properties=0,mode=0,listed=1)])

    def test_truncated_trailing_wrong_flag_mode_listed_and_utf8_refuse(self):
        good=self.snapshot()
        for size in range(len(good)):
            with self.subTest(size=size),self.assertRaises((AssertionError,ValueError,EOFError)):
                add_player_info(good[:size])
        cases=[good+b'\0',bytes([4])+good[1:],good[:-2]+bytes([4,1]),good[:-1]+bytes([2]),
               good[:19]+bytes([255])+good[20:]]
        for value in cases:
            with self.subTest(value=value.hex()),self.assertRaises((AssertionError,ValueError,EOFError)):
                add_player_info(value)

    def test_bad_properties_count_and_signature_do_not_become_valid_receipts(self):
        prefix=self.snapshot()[:-3]
        for value in [prefix+varint(257),prefix+bytes([1,1])+b'x'+bytes([1])+b'y'+bytes([2])]:
            with self.assertRaises((AssertionError,ValueError,EOFError)):
                add_player_info(value)
