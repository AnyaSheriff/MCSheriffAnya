import struct
import unittest
from probe_persistence_777 import Probe, compare, snapshot
from smoke import varint


class PersistenceTests(unittest.TestCase):
    def probe(self):
        p = object.__new__(Probe)
        p.wire=[];p.effects={};p.xp=None;p.health=None
        return p

    def test_xp_missing_is_not_zero(self):
        p=self.probe()
        self.assertIsNone(snapshot(p,'a','s')['xp'])
        p.observe_packet('experience',struct.pack('>f',0)+varint(0)+varint(7))
        self.assertEqual(p.xp,{'progress':0.0,'level':0,'total':7})
        self.assertEqual(snapshot(p,'a','s')['xp_packets'],1)

    def test_effect_flags_infinite_and_removal(self):
        p=self.probe()
        p.observe_packet('entity_effect',varint(4)+varint(1)+varint(2)+varint(-1)+b'\x05')
        self.assertEqual(p.effects,{'1':{'amplifier':2,'duration':-1,'flags':5}})
        p.observe_packet('remove_entity_effect',varint(4)+varint(1))
        self.assertEqual(p.effects,{})

    def test_strict_comparator_keeps_missing_packet_failure(self):
        p=self.probe();a=snapshot(p,'fresh','a')
        p.observe_packet('experience',struct.pack('>f',0)+b'\0\0')
        b=snapshot(p,'fresh','b')
        failures=compare({'snapshots':[b]},{'snapshots':[a]})
        self.assertEqual(len(failures),2)

    def test_comparator_duration_and_flags_exact(self):
        p=self.probe();p.effects={'1':{'duration':100,'amplifier':0,'flags':1}}
        a=snapshot(p,'effects','a');p.effects['1']['duration']=99
        b=snapshot(p,'effects','b')
        self.assertTrue(compare({'snapshots':[a]},{'snapshots':[b]}))


if __name__=='__main__':unittest.main()
