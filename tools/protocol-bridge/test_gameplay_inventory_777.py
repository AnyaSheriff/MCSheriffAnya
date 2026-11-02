"""Проверки декодера доказательств; не заменяют свежий серверный запуск."""
import io
import copy
import struct
import unittest
from probe_gameplay_inventory_777 import Probe, click_payload, predicted_pickup_payload, predict_pickup
from smoke import varint, read_varint
from compare_gameplay_inventory_777 import compare, STAGES, CHECKS


class EvidenceTests(unittest.TestCase):
    def client(self):
        client = Probe.__new__(Probe)
        client.version = 777
        client.names = {1:'stone'}
        client.inventory = {'window':0, 'revision':2, 'slots':[None]*46, 'cursor':None}
        client.observed = []
        client.wire = []
        client.xp = None
        client.ground_items = set()
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

    def test_predicted_pickup_has_changed_empty_slot_and_hashed_cursor(self):
        for revision in (17, -1):
            data = io.BytesIO(predicted_pickup_payload(revision, 513))
            self.assertEqual(read_varint(data.read), 0)
            self.assertEqual(read_varint(data.read), revision & 0xffffffff)
            self.assertEqual(struct.unpack('>hb', data.read(3)), (9,0))
            self.assertEqual(read_varint(data.read), 0)  # normal pickup
            self.assertEqual(read_varint(data.read), 1)  # changed slots
            self.assertEqual(struct.unpack('>h', data.read(2))[0], 9)
            self.assertEqual(data.read(1), b'\0')  # predicted empty slot9
            self.assertEqual(data.read(1), b'\1')  # present hashed cursor
            self.assertEqual(read_varint(data.read), 513)
            self.assertEqual(read_varint(data.read), 3)
            self.assertEqual(read_varint(data.read), 0)  # component hashes
            self.assertEqual(read_varint(data.read), 0)  # removed components
            self.assertEqual(data.read(), b'')

    def test_prediction_moves_three_once_and_rejects_second_application(self):
        client = self.client()
        client.inventory['slots'][9] = {'item':'stone', 'count':3}
        predict_pickup(client)
        self.assertIsNone(client.inventory['slots'][9])
        self.assertEqual(client.inventory['cursor'], {'item':'stone', 'count':3})
        with self.assertRaises(AssertionError):
            predict_pickup(client)

    def test_ground_item_spawn_and_destroy_evidence(self):
        client = self.client()
        client.observe_packet('spawn_entity', varint(123)+bytes(16)+varint(72))
        self.assertEqual(client.ground_items, {123})
        client.observe_packet('spawn_entity', varint(124)+bytes(16)+varint(1))
        self.assertEqual(client.ground_items, {123})
        client.observe_packet('entity_destroy', varint(1)+varint(123))
        self.assertFalse(client.ground_items)
        self.assertEqual([row['name'] for row in client.wire], ['spawn_entity','spawn_entity','entity_destroy'])

    def test_click_is_real_shift_and_stale_state_is_signed_varint(self):
        self.assertEqual(click_payload(2,36), b'\0\2\0\x24\0\1\0\0')
        self.assertEqual(click_payload(-1,5,0), b'\0\xff\xff\xff\xff\x0f\0\5\0\0\0\0')


class ComparisonGuards(unittest.TestCase):
    def reports(self):
        # Синтетическая фикстура проверяет отказы сравнения, не поведение игры.
        report = {'protocol':777, 'target':'official', 'exit_code':0, 'checks':list(CHECKS),
                  'observe_only':False, 'checks_kind':'asserted-invariants',
                  'jar_sha1':'33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c', 'target_sha256':'a'*64,
                  'packets':[{'direction':d, 'name':n} for d,n in [('out','window_click'), ('out','client_command'),
                             ('in','window_items'), ('in','respawn'), ('in','experience')]],
                  'snapshots':[{'stage':s, 'packet_index':5, 'ground_item_entities':[],
                                'inventory':{'window':0,'revision':1,'slots':[None]*46,'cursor':None},
                                'xp':{'progress':.125,'level':17,'total':403}} for s in STAGES]}
        other=copy.deepcopy(report);other['target']='core'
        return report,other

    def test_comparison_ignores_only_state_id_not_items_cursor_or_experience(self):
        a,b=self.reports();b['snapshots'][0]['inventory']['revision']=999
        self.assertEqual(compare(a,b,'a'*64)['status'],'match')
        b['snapshots'][0]['inventory']['slots'][9]={'item':'stone','count':1}
        b['snapshots'][1]['inventory']['cursor']={'item':'stone','count':2}
        b['snapshots'][-1]['xp']['level']=0
        self.assertEqual([x['field'] for x in compare(a,b,'a'*64)['mismatches']],['slots','cursor','xp'])

    def test_comparison_rejects_missing_coverage_raw_evidence_and_failed_probe(self):
        for mutate in [lambda b:b['snapshots'].pop(), lambda b:b['checks'].pop(),
                       lambda b:b['packets'].pop(), lambda b:b.update(error='failed'),
                       lambda b:b.update(observe_only=True, checks_kind='observed-actions-only'),
                       lambda b:b.update(behavior_failures=['wrong items']),
                       lambda b:b['snapshots'][0].update(ground_item_entities=[7])]:
            a,b=self.reports();mutate(b)
            with self.assertRaises(ValueError): compare(a,b,'a'*64)

    def test_comparison_rejects_wrong_runtime_identity(self):
        a,b=self.reports()
        with self.assertRaises(ValueError): compare(a,b,'b'*64)


if __name__ == '__main__':
    unittest.main()
