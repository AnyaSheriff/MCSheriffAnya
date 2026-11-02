import json
import struct
import unittest
from probe_persistence_777 import Probe, compare, snapshot, evidence_failures, duration_failure, player_metadata
from smoke import varint


class PersistenceTests(unittest.TestCase):
    def test_actual_core_initial_hand_metadata_and_invalid_arm(self):
        entity, fields = player_metadata(bytes.fromhex('010000000614000f2a0110007fff'))
        self.assertEqual(entity,1)
        self.assertEqual(fields,{0:0,6:0,15:1,16:127})
        with self.assertRaises(AssertionError):
            player_metadata(bytes.fromhex('010f2a02ff'))

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
        p=self.probe();a=snapshot(p,'fresh-login','a')
        p.observe_packet('experience',struct.pack('>f',0)+b'\0\0')
        b=snapshot(p,'fresh-login','b')
        failures=compare({'snapshots':[b]},{'snapshots':[a]})
        self.assertEqual(len(failures),2)

    def test_order_and_duplicates_rejected(self):
        p=self.probe();a=snapshot(p,'a','s');b=snapshot(p,'b','s')
        self.assertTrue(compare({'snapshots':[a,b]},{'snapshots':[b,a]}))
        self.assertTrue(compare({'snapshots':[a,a]},{'snapshots':[a,a]}))

    def test_empty_or_failed_evidence_rejected(self):
        self.assertTrue(evidence_failures({},official=True))
        self.assertTrue(evidence_failures({'target':'core','observe_only':True}))

    def test_semantic_flags_and_exact_login_flags(self):
        p=self.probe();p.effects={'1':{'duration':100,'amplifier':0,'flags':14}}
        a=snapshot(p,'effects-before','a');p.effects['1']['flags']=6
        b=snapshot(p,'effects-before','b')
        self.assertFalse(compare({'snapshots':[a]},{'snapshots':[b]}))
        a['stage']=b['stage']='effects-relogin';a['xp_packets']=b['xp_packets']=1
        self.assertTrue(compare({'snapshots':[a]},{'snapshots':[b]}))

    def test_offline_seconds_are_excluded_from_duration_budget(self):
        report={'sessions':[{'connect_started':0,'closed_at':1},{'connect_started':7,'closed_at':8}]}
        before={'effects':{'0':{'duration':600}},'effect_receipts':{'0':{'time':0.5}}}
        after={'effects':{'0':{'duration':580}},'effect_receipts':{'0':{'time':7.5}}}
        self.assertFalse(duration_failure(report,before,after,'0'))
        after['effects']['0']['duration']=600
        self.assertTrue(duration_failure(report,before,after,'0'), 'reset to original600 must fail')
        after['effects']['0']['duration']=460
        self.assertTrue(duration_failure(report,before,after,'0'))
        after['effects']['0']['duration']=601
        self.assertTrue(duration_failure(report,before,after,'0'))

    def test_raw_and_snapshot_timestamps_are_bounded(self):
        packet=lambda at:{'direction':'out','name':'tick_end','payload':'','time':at}
        report={'target':'core','observe_only':False,'protocol':777,'server_exits':[0,0],
                'sessions':[{'name':'s','connect_started':1.0,'closed_at':4.0,'packets':[packet(3.0),packet(2.0)]}],
                'snapshots':[]}
        self.assertTrue(any('packet timestamp' in x for x in evidence_failures(report)))
        report['sessions'][0]['packets']=[packet(2.0)]
        p=self.probe();value=snapshot(p,'fresh-login','s')
        value.update(packet_index=1,time=5.0)
        report['snapshots']=[value]
        self.assertTrue(any('snapshot timestamp' in x for x in evidence_failures(report)))
        report['sessions'][0]['packets']=[packet(0.5)]
        self.assertTrue(any('packet timestamp' in x for x in evidence_failures(report)))

    def test_actual_official_absorption_metadata(self):
        self.assertEqual(player_metadata(bytes.fromhex('03110340000000ff')),(3,{17:2.0}))
        p=self.probe();p.entity_id=3;p.absorption=None
        p.observe_packet('entity_metadata',bytes.fromhex('03110340000000ff'))
        self.assertEqual(p.absorption,2.0)
        p.observe_packet('entity_metadata',bytes.fromhex('04110340800000ff'))
        self.assertEqual(p.absorption,2.0)
        with self.assertRaises(AssertionError):
            player_metadata(bytes.fromhex('0311037fc00000ff'))


if __name__=='__main__':unittest.main()
