"""Проверки декодера доказательств; не заменяют свежий серверный запуск."""
import struct
import unittest
from probe_gameplay_inventory_777 import Probe, click_payload
from smoke import varint


class EvidenceTests(unittest.TestCase):
    def client(self):
        client = Probe.__new__(Probe)
        client.version = 777
        client.names = {1:'stone'}
        client.inventory = {'window':0, 'revision':2, 'slots':[None]*46, 'cursor':None}
        client.observed = []
        client.wire = []
        client.xp = None
        return client

    def test_player_inventory_hotbar_armor_offhand_indices(self):
        client = self.client()
        for source, destination in [(0,36), (8,44), (9,9), (35,35), (36,8), (39,5), (40,45)]:
            client.observe_packet('set_player_inventory', varint(source)+b'\1\1\0\0')
            self.assertEqual(client.inventory['slots'][destination], {'item':'stone', 'count':1})
            self.assertEqual(client.wire[-1]['payload'], (varint(source)+b'\1\1\0\0').hex())

    def test_xp_preserves_float_level_and_total(self):
        client = self.client()
        payload = struct.pack('>f', .125)+varint(17)+varint(403)
        client.observe_packet('experience', payload)
        self.assertEqual(client.xp, {'progress':.125, 'level':17, 'total':403})
        self.assertEqual(client.wire[-1]['payload'], payload.hex())

    def test_click_is_real_shift_and_stale_state_is_signed_varint(self):
        self.assertEqual(click_payload(2,36), b'\0\2\0\x24\0\1\0\0')
        self.assertEqual(click_payload(-1,5,0), b'\0\xff\xff\xff\xff\x0f\0\5\0\0\0\0')


if __name__ == '__main__':
    unittest.main()
