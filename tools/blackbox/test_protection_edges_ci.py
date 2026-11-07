"""Отрицательные проверки новой записи: чары/владелец/реальный урон обязательны."""
import copy
import json
import struct
import unittest
from pathlib import Path

import protection_edges_ci as p
from smoke import varint
from test_armor_enchants_ci import hp, damage
from test_defense_ci import packet

ITEMS={'diamond_chestplate':100}


def armor(wear,tick,ench=None):
    components=b'\3'+varint(wear)
    if ench is not None:components+=b'\x0d\1'+varint(ench[0])+varint(ench[1])
    slot=b'\1'+varint(100)+varint(1+(ench is not None))+b'\0'+components
    return packet('set_player_inventory',varint(38)+slot,tick)


def fixture(name,reflection=False):
    fire=name in p.NAMES[:2];protected=name==p.NAMES[1];broken=name==p.NAMES[3]
    registries={'minecraft:damage_type':['minecraft:player_attack','minecraft:on_fire','minecraft:thorns'],
                'minecraft:enchantment':['minecraft:thorns','minecraft:fire_protection']}
    raw={'scenario':name,'protocol':777,'isolate_vitals':True,'start':10,
         'initial':[{'entity':i,'health':20,'health_receipt':hp(20,-1)} for i in [2,3]],
         'sent':[{'action':{'attack':True,'bot':2}}],
         'clients':[{'events':[],'failure':None,'registry_names':copy.deepcopy(registries)} for _ in range(2)]}
    wear=527 if broken else 0;ench=(1,4) if protected else None if fire else (0,7)
    es=raw['clients'][0]['events'];es.extend([armor(wear,1,ench),damage(source=0,tick=8),hp(19 if not fire else 15,8)])
    if fire:
        es.append(armor(1,8,ench))
        if protected:es.extend([damage(source=1,cause=0,tick=28),hp(14.32,28)])
        else:
            for tick,value in [(28,14),(48,13),(68,12)]:es.extend([damage(source=1,cause=0,tick=tick),hp(value,tick)])
        for client in raw['clients']:
            client['events'].extend([packet('entity_metadata',bytes([2,0,0,1,255]),9),
                                     packet('entity_metadata',bytes([2,0,0,0,255]),40 if protected else 88)])
    elif broken:
        es.append(packet('set_player_inventory',varint(38)+b'\0',8))
        for client in raw['clients']:client['events'].append(packet('entity_status',struct.pack('>iB',2,50),8))
    else:es.append(armor(3,8,ench))
    if not fire and (not broken or reflection):raw['clients'][1]['events'].extend([damage(entity=3,source=2,cause=3,tick=8),hp(17,8)])
    if not raw['clients'][1]['events']:raw['clients'][1]['events'].append(hp(20,0))
    for client in raw['clients']:client['events'].sort(key=lambda e:e['at'])
    return raw


def rows():
    return [{'scenario':name,'side':side,'status':'pass','observed':p.inspect(fixture(name),ITEMS)}
            for name in p.NAMES for side in ['official','ours']]


class Guards(unittest.TestCase):
    def test_all_four_shapes_and_no_reflection_from_broken_piece(self):
        for name in p.NAMES:p.inspect(fixture(name),ITEMS)
        with self.assertRaises(AssertionError):p.inspect(fixture(p.NAMES[3],True),ITEMS)
        self.assertEqual(p.cross_check(rows()),[])

    def test_hp_receipt_and_authored_attack_required(self):
        for modify in [lambda r:r.update(sent=[]),lambda r:r['initial'][1].pop('health_receipt'),
                       lambda r:r['clients'][0].update(failure='closed')]:
            raw=fixture(p.NAMES[0]);modify(raw)
            with self.assertRaises((AssertionError,KeyError)):p.inspect(raw,ITEMS)

    def test_wrong_enchantment_or_initial_wear_cannot_pass(self):
        for name in p.NAMES:
            raw=fixture(name);raw['clients'][0]['events'][0]=armor(123,1,None)
            with self.subTest(name=name),self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_gear_received_after_hit_fails(self):
        raw=fixture(p.NAMES[1]);raw['clients'][0]['events'][0]['at']=12
        with self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_wrong_registry_or_damage_owner_fails(self):
        for kind in ['registry','owner']:
            raw=fixture(p.NAMES[0])
            if kind=='registry':raw['clients'][0]['registry_names']['minecraft:damage_type'][0]='minecraft:magic'
            else:raw['clients'][0]['events'][1]=damage(source=0,cause=6,tick=8)
            with self.subTest(kind=kind),self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_fire_requires_real_damage_and_no_attributed_fake_fire(self):
        for kind in ['missing','wrong_cause']:
            raw=fixture(p.NAMES[1]);es=raw['clients'][0]['events'];event=next(e for e in es if e['name']=='damage_event' and e['at']>11)
            if kind=='missing':es.remove(event)
            else:event['hex']=damage(source=1,cause=4,tick=28)['hex']
            with self.subTest(kind=kind),self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_both_observers_must_see_extinction(self):
        for side in [0,1]:
            raw=fixture(p.NAMES[1]);raw['clients'][side]['events']=[e for e in raw['clients'][side]['events'] if not(e['name']=='entity_metadata' and e['at']>11)]
            with self.subTest(side=side),self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_incorrect_duration_rejected_without_new_random_exception(self):
        raw=fixture(p.NAMES[1]);event=next(e for e in raw['clients'][1]['events'] if e['name']=='entity_metadata' and e['at']>11);event['at']=15
        with self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_control_thorns_must_actually_reflect_and_wear(self):
        for kind in ['damage','wear']:
            raw=fixture(p.NAMES[2])
            if kind=='damage':raw['clients'][1]['events']=[hp(20,0)]
            else:raw['clients'][0]['events']=[e for e in raw['clients'][0]['events'] if e!=armor(3,8,(0,7))]
            with self.subTest(kind=kind),self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_break_requires_empty_slot_and_both_events(self):
        for side in [0,1]:
            raw=fixture(p.NAMES[3]);raw['clients'][side]['events']=[e for e in raw['clients'][side]['events'] if e['name']!='entity_status']
            with self.subTest(side=side),self.assertRaises(AssertionError):p.inspect(raw,ITEMS)

    def test_cross_side_reflection_difference_cannot_hide_behind_rng_mask(self):
        x=rows();row=next(r for r in x if r['scenario']==p.NAMES[3] and r['side']=='ours');row['observed']['reflected']=[{'side':1}];row['observed']['health'][1]=[20,17]
        self.assertTrue(p.cross_check(x))

    def test_exact_hp_and_relative_protection_have_independent_controls(self):
        x=rows();x[1]['observed']['health'][0][1]+=0.0000001;self.assertTrue(p.cross_check(x))
        x=rows()
        for row in x:
            if row['scenario']==p.NAMES[1]:row['observed']['health'][0][2]=14
        self.assertTrue(p.cross_check(x))

    def test_native_setup_keeps_prior_comparator_and_random_rules(self):
        import record777
        self.assertEqual(json.loads(p.CONFIG.read_text())['scenarios'],p.NAMES)
        for name in p.NAMES:
            s=json.loads((p.ci.ROOT/f'tools/blackbox/scenarios/{name}.json').read_text());record777.validate(s)
            base='pvp-fire-aspect-777' if name in p.NAMES[:2] else 'pvp-thorns-777'
            original=json.loads((p.ci.ROOT/f'tools/blackbox/scenarios/{base}.json').read_text())
            self.assertEqual(s['random'],original['random']);self.assertEqual(s['native_phase'],'all')
            self.assertNotIn('!syncattrs',s['setup']);self.assertFalse(s.get('attr_phase',False))
        self.assertIn(p.ci.ROOT/'tools/blackbox/combat_wire.py',p.ci.helper_sources(entries=[Path(p.__file__)]))


if __name__=='__main__':unittest.main()
