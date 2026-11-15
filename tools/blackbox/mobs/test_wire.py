"""Отрицательные пакеты и регрессии изменённых полей 777 без сервера."""
import math
import struct
import unittest
from wire import MobObserver, Reader


def observer():
    # Для офлайн-пакетов сетевые таблицы не нужны.
    obj = MobObserver.__new__(MobObserver)
    obj.phase='test'; obj.events=[]; obj.entities={}; obj.entity_types={}; obj.max_events=100; obj.counts={}
    return obj


class WireTests(unittest.TestCase):
    def test_spawn_zero_velocity_precedes_angles_777(self):
        o=observer()
        packet=b'\x07'+bytes(16)+b'\x03'+struct.pack('>ddd',1,64,-2)+b'\x00\x20\x40\x80\x00'
        o.observe_packet('spawn_entity',packet)
        s=o.snapshot(7)
        self.assertEqual([s['pitch'],s['yaw'],s['head_yaw']],[45,90,180])
        self.assertEqual(s['velocity'],[0,0,0]); self.assertEqual(o.events[-1]['status'],'decoded')
        for n in range(len(packet)):
            bad=observer();bad.observe_packet('spawn_entity',packet[:n])
            self.assertEqual(bad.events[-1]['status'],'decode_error',n)
            self.assertNotIn(7,bad.entities)

    def test_health_is_unknown_until_symbolic_metadata(self):
        o=observer();o.entities[7]={'entity':7,'entity_type':3}
        o.set_entity_types([{'id':3,'name':'test','metadataKeys':['shared_flags','air_supply']+['unused']*7+['health']}])
        self.assertNotIn('health',o.snapshot(7))
        o.observe_packet('entity_metadata',b'\x07\x09\x03'+struct.pack('>f',12.5)+b'\xff')
        self.assertEqual(o.snapshot(7)['health'],12.5)
        self.assertEqual(o.events[-1]['metadata'][0]['key'],'health')
        o.observe_packet('entity_metadata',b'\x07\x02\x7f\x00\xff')
        self.assertEqual(o.events[-1]['status'],'unparsed_metadata')
        self.assertEqual(o.events[-1]['unparsed']['remaining_hex'],'00ff')

    def test_stepped_sync_preserves_offsets_and_last_position(self):
        o=observer()
        packet=b'\x07\x01\x02'+struct.pack('>ddd',1,2,3)+b'\x02'+struct.pack('>ddd',4,5,6)+b'\x06'+struct.pack('>ff',90,10)+b'\x01'
        o.observe_packet('sync_entity_position',packet)
        self.assertEqual(o.snapshot(7)['position'],[4,5,6])
        self.assertEqual([s['tick_offset'] for s in o.events[-1]['steps']],[2,6])
        o.observe_packet('sync_entity_position',b'\x07\x02')
        self.assertEqual(o.events[-1]['status'],'decode_error')
        self.assertEqual(o.snapshot(7)['position'],[4,5,6])

    def test_delta777_has_properties_and_no_trailing_ground(self):
        o=observer();o.entities[7]={'entity':7,'position':[0,64,0]}
        o.observe_packet('entity_move_look',b'\x07\x01'+struct.pack('>hhh',4096,0,-2048)+b'\x80\x40')
        self.assertEqual(o.snapshot(7)['position'],[1,64,-.5])
        self.assertEqual(o.snapshot(7)['yaw'],180)
        o.observe_packet('rel_entity_move',b'\x07\x00'+struct.pack('>hhh',1,2,3)+b'\x01')
        self.assertEqual(o.events[-1]['status'],'decode_error')
        self.assertEqual(o.snapshot(7)['position'],[1,64,-.5])

    def test_guards_and_nonfinite(self):
        for raw in (b'\x80'*5,b'\xff\xff\xff\xff\x7f'):
            with self.assertRaises(ValueError):Reader(raw).uint()
        with self.assertRaises(ValueError): Reader(b'\x01').lpvec()
        with self.assertRaises(ValueError): Reader(struct.pack('>d',math.inf)).floats('d')
        with self.assertRaises(ValueError): Reader(b'\x02').boolean()
        o=observer();o.max_events=0
        with self.assertRaises(RuntimeError):o.observe_packet('entity_status',b'')

    def test_snapshot_removal_and_event_copy(self):
        o=observer();o.entities[7]={'entity':7,'position':[1,2,3]}
        o.observe_packet('entity_destroy',b'\x01\x07')
        self.assertTrue(o.snapshot(7)['removed'])
        snap=o.snapshot(7);snap['position'][0]=99
        self.assertEqual(o.snapshot(7)['position'][0],1)
        copy=o.marked_events(0);copy[0]['status']='changed'
        self.assertEqual(o.events[0]['status'],'decoded')


if __name__=='__main__':unittest.main()
