"""Отрицательные wire-контроли щита: ложный PASS без урона/use/износа запрещён."""
import copy
import json
import struct
import unittest
from pathlib import Path
import defense_ci as d
from smoke import varint


def packet(name, payload, tick=1):
    return {'name':name,'hex':payload.hex(),'at':10+tick*.05}


def fixture(name):
    active = packet('entity_metadata', varint(2)+bytes([8,0,3,255]))
    x={'scenario':name,'protocol':777,'isolate_vitals':True,'start':10,
       'initial':[{'entity':2,'health':20},{'entity':3,'health':20}],
       'sent':[{'action':{'use':True,'hand':1}},{'action':{'attack':True,'bot':2}}],
       'clients':[{'events':[active],'failure':None},{'events':[active],'failure':None}]}
    events=x['clients'][0]['events']
    wear=8 if name==d.NAMES[0] else 18 if name==d.NAMES[2] else 0
    if wear:events.append(packet('set_player_inventory',varint(40)+b'\1'+varint(100)+b'\1\0\3'+varint(wear)))
    if name!=d.NAMES[0]:
        events.append(packet('update_health',struct.pack('>f',13)+b'\x14'+struct.pack('>f',5),45))
        events.append(packet('damage_event',b'\2\x22\4\4\0',45))
    if name in [d.NAMES[2],'pvp-shield']:
        group=b'minecraft:shield';events.append(packet('set_cooldown',varint(len(group))+group+varint(100),12))
    if name==d.NAMES[2]:
        group=b'minecraft:shield';events.append(packet('set_cooldown',varint(len(group))+group+b'\0',111))
        events.append(packet('entity_metadata',varint(2)+bytes([8,0,3,255]),130))
    return x


class SemanticTests(unittest.TestCase):
    def test_all_six_real_response_shapes_pass(self):
        for name in d.NAMES:
            with self.subTest(name=name):d.inspect(fixture(name),100)

    def test_missing_initial_health_inactive_hand_and_missing_actions_fail(self):
        for alter in [lambda r:r['initial'][0].pop('health'),lambda r:r.update(sent=[]),
                      lambda r:r['clients'][0].update(events=r['clients'][0]['events'][1:]) or r['clients'][1].update(events=[]),
                      lambda r:r.update(protocol=775),lambda r:r['clients'][1].update(failure='closed')]:
            r=fixture(d.NAMES[0]);alter(r)
            with self.subTest(alter=alter),self.assertRaises(AssertionError):d.inspect(r,100)

    def test_wrong_front_wear_or_sword_cooldown_fail(self):
        for damage in [0,7,9]:
            r=fixture(d.NAMES[0]);r['clients'][0]['events'][1]['hex']=(varint(40)+b'\1'+varint(100)+b'\1\0\3'+varint(damage)).hex()
            with self.subTest(damage=damage),self.assertRaises(AssertionError):d.inspect(r,100)
        r=fixture(d.NAMES[0]);r['clients'][0]['events'].append(fixture(d.NAMES[2])['clients'][0]['events'][-3])
        with self.assertRaises(AssertionError):d.inspect(r,100)

    def test_rear_needs_actual_damage_from_right_player(self):
        for field,value in [('name','unrelated'),('hex','0222050500')]:
            r=fixture(d.NAMES[1]);r['clients'][0]['events'][-1][field]=value
            with self.subTest(field=field),self.assertRaises(AssertionError):d.inspect(r,100)

    def test_recovery_rejects_damage_after_reactivation_wrong_cooldown_and_missing_reuse(self):
        for alter in [lambda es:es[3].update(at=17.3),lambda es:es.pop(),
                      lambda es:es[-3].update(hex=(b'\x10minecraft:shield'+varint(99)).hex()),
                      lambda es:es.append(packet('damage_event',b'\2\x22\4\4\0',145))]:
            r=fixture(d.NAMES[2]);alter(r['clients'][0]['events'])
            with self.subTest(alter=alter),self.assertRaises((AssertionError,EOFError)):d.inspect(r,100)

    def test_empty_armor_positive_control_fails(self):
        for name in d.NAMES[-2:]:
            r=fixture(name);r['clients'][0]['events']=r['clients'][0]['events'][:1]
            with self.subTest(name=name),self.assertRaises(AssertionError):d.inspect(r,100)

    def test_slot_truncation_trailing_unknown_component_fail(self):
        for raw in [b'\x28\1',b'\x28\1\x64\1\0\x7f',b'\x28\1\x64\1\0\3\x08\0']:
            with self.subTest(raw=raw),self.assertRaises((AssertionError,EOFError)):d.shield_damages(packet('set_player_inventory',raw),100)

    def test_suite_and_original_combat_scope_are_separate(self):
        config=json.loads(d.CONFIG.read_text());self.assertEqual(config['scenarios'],d.NAMES)
        old=json.loads(d.ci.SOURCES.read_text());self.assertEqual(len(old['scenarios']),15)
        self.assertNotEqual(old['scenarios'],d.NAMES)
        sources=d.ci.helper_sources(entries=[Path(d.__file__)])
        self.assertIn(d.ci.ROOT/'tools/protocol-bridge/probe_use_actions_runtime.py',sources)
        self.assertIn(d.ci.ROOT/'tools/protocol-bridge/probe_persistence_777.py',sources)

    def test_all_authored_setup_is_supported_by_native_recorder(self):
        import record777
        for name in d.NAMES:
            row=json.loads((d.ci.ROOT/f'tools/blackbox/scenarios/{name}.json').read_text())
            record777.validate(row)
            self.assertNotIn('!syncattrs',row['setup'])
            self.assertEqual(row.get('native_phase','all'),'all')
            self.assertFalse(row.get('attr_phase',False), 'legacy bot marker is not native alignment')

    def test_semantic_failure_never_hides_infrastructure_failure(self):
        for before,after in [('match','mismatch-open'),('mismatch-open','mismatch-open'),('infrastructure-failed','infrastructure-failed')]:
            result={'status':before};d.apply_semantic_result(result,{'status':'failed','error':'missing raw'})
            self.assertEqual(result,{'status':after,'semantic_error':'missing raw'})

    def test_expiry_is_checked_on_whole_observed_tick_grid(self):
        r=fixture(d.NAMES[2]);r['clients'][0]['events'][-2]['at']=10+112.01*.05
        d.inspect(r,100)
        r['clients'][0]['events'][-2]['at']=10+113*.05
        with self.assertRaises(AssertionError):d.inspect(r,100)


if __name__=='__main__':unittest.main()
