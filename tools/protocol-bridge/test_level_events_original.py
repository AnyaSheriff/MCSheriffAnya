"""Чистые отрицательные проверки границ наблюдений, без сети/Minecraft."""
import struct
import unittest
from probe_level_events_original import event, validate
from probe_gameplay import packed


class Guards(unittest.TestCase):
    def test_exact_event_length_and_boolean(self):
        raw = struct.pack('>iQiB', 1001, 42, 0, 0)
        self.assertEqual(event(raw)['effect'], 1001)
        for bad in (raw[:-1], raw+b'\0', raw[:-1]+b'\2'):
            with self.assertRaises(ValueError): event(bad)

    def test_missing_wrong_phase_wrong_data_rejected(self):
        rows = [{'phase':p, 'name':'world_event', 'event':{'effect':e,'data':0,'packed_position':int.from_bytes(packed(40 if p in ('empty','item') else 42,100,40),'big')}} for p,e in [('empty',1001),('item',1000),('jukebox-start',1010),('jukebox-stop',1011)]]
        rows.append({'phase':'jukebox-start','name':'world_particles','note':True})
        validate(rows)
        for index in range(len(rows)):
            with self.assertRaises(ValueError): validate(rows[:index]+rows[index+1:])
        rows[0]['event']['data']=1
        with self.assertRaises(ValueError): validate(rows)
