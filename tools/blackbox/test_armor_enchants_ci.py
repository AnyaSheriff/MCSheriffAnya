"""Отрицательные контроли: отсутствие удара, износа/эффекта не считается успехом."""
import copy
import json
import struct
import unittest

import armor_enchants_ci as a
from smoke import varint
from test_defense_ci import packet

ITEMS = {'iron_chestplate': 100, 'diamond_chestplate': 101}


def hp(value, tick):
    return packet('update_health', struct.pack('>f', value)+b'\x14'+struct.pack('>f', 5), tick)


def damage(entity=2, source=34, cause=4, tick=8):
    return packet('damage_event', b''.join(varint(v) for v in [entity, source, cause, cause])+b'\0', tick)


def chest(value, tick, item=100):
    stack = b'\0' if value is None else b'\1'+varint(item)+b'\1\0\3'+varint(value)
    return packet('set_player_inventory', varint(38)+stack, tick)


def fixture(name):
    raw = {'scenario':name, 'protocol':777, 'isolate_vitals':True, 'start':10,
           'initial':[{'entity':i, 'health':20, 'health_receipt':hp(20, -1)} for i in [2, 3]],
           'sent':[{'action':{'attack':True, 'bot':2}}],
           'clients':[{'events':[], 'failure':None}, {'events':[hp(20, 0)], 'failure':None}]}
    events = raw['clients'][0]['events']
    events.append(damage())
    if name == a.NAMES[0]: events.append(hp(10, 8))
    elif name == a.NAMES[1]:
        events.extend([hp(13, 8), damage(source=20, cause=0, tick=28), hp(12, 28)])
        for client in raw['clients']:
            client['events'].extend([packet('entity_metadata', bytes([2, 0, 0, 1, 255]), 8),
                                     packet('entity_metadata', bytes([2, 0, 0, 0, 255]), 88)])
    elif name in a.NAMES[2:5]:
        side = 0 if name == a.NAMES[4] else 1
        amplifier = 0 if name == a.NAMES[3] else 1
        raw['clients'][side]['events'].append(packet('entity_effect', bytes([2+side, 1, amplifier])+varint(60)+b'\6', 1))
        events.append(hp(7 if name == a.NAMES[2] else 17 if name == a.NAMES[3] else 15.8, 8))
        if name == a.NAMES[3]: raw['clients'][side]['events'].append(packet('remove_entity_effect', b'\3\1', 61))
        if name == a.NAMES[4]: events.extend([damage(tick=28), hp(11.6, 28)])
    elif name == a.NAMES[5]:
        events.extend([hp(19.5, 8), chest(3, 8, 101)])
        raw['clients'][1]['events'].extend([damage(entity=3, source=43, cause=3), hp(17, 8)])
    else:
        raw['sent'].append({'action':{'attack':True, 'bot':2}})
        events.extend([hp(13.7 if name == a.NAMES[6] else 13, 8), damage(tick=28), hp(7.4 if name == a.NAMES[6] else 6, 28)])
        if name == a.NAMES[6]: events.extend([chest(200, 1), chest(201, 8), chest(202, 28)])
        else: events.extend([chest(239, 1), chest(None, 8), packet('entity_status', struct.pack('>iB', 2, 50), 8)])
    for client in raw['clients']: client['events'].sort(key=lambda e:e['at'])
    return raw


class Guards(unittest.TestCase):
    def test_eight_distinct_response_shapes(self):
        for name in a.NAMES:
            with self.subTest(name=name): a.inspect(fixture(name), ITEMS)

    def test_initial_hp_must_have_wire_receipt_before_actions(self):
        for alter in [lambda r:r['initial'][0].pop('health_receipt'),
                      lambda r:r['initial'][1]['health_receipt'].update(at=11),
                      lambda r:r['initial'][0].update(health=19)]:
            raw=fixture(a.NAMES[0]); alter(raw)
            with self.assertRaises((AssertionError, KeyError)): a.inspect(raw, ITEMS)

    def test_each_case_needs_real_attack_damage_owner_and_health(self):
        for name in a.NAMES:
            for kind in ['action', 'owner', 'health', 'disconnect']:
                raw=fixture(name)
                if kind=='action': raw['sent']=[]
                elif kind=='owner':
                    for e in raw['clients'][0]['events']:
                        if e['name']=='damage_event': e['hex']=damage(cause=6)['hex']
                elif kind=='health': raw['clients'][0]['events']=[e for e in raw['clients'][0]['events'] if e['name']!='update_health']
                else: raw['clients'][1]['failure']='closed'
                with self.subTest(name=name,kind=kind), self.assertRaises(AssertionError): a.inspect(raw, ITEMS)

    def test_missing_effect_expiry_and_wrong_recipient_fail(self):
        for name in a.NAMES[2:5]:
            raw=fixture(name)
            for client in raw['clients']: client['events']=[e for e in client['events'] if e['name']!='entity_effect']
            with self.subTest(name=name), self.assertRaises(AssertionError): a.inspect(raw, ITEMS)
        raw=fixture(a.NAMES[3]);raw['clients'][1]['events']=[e for e in raw['clients'][1]['events'] if e['name']!='remove_entity_effect']
        with self.assertRaises(AssertionError): a.inspect(raw, ITEMS)

    def test_fire_must_damage_and_extinguish_for_both_observers(self):
        for side in [0, 1]:
            raw=fixture(a.NAMES[1]);raw['clients'][side]['events']=[e for e in raw['clients'][side]['events'] if not (e['name']=='entity_metadata' and e['at']>14)]
            with self.subTest(side=side),self.assertRaises(AssertionError): a.inspect(raw, ITEMS)

    def test_thorns_needs_return_owner_real_hp_and_armor_wear(self):
        for missing in ['damage_event','update_health']:
            raw=fixture(a.NAMES[5]);raw['clients'][1]['events']=[e for e in raw['clients'][1]['events'] if e['name']!=missing]
            with self.assertRaises(AssertionError): a.inspect(raw, ITEMS)
        raw=fixture(a.NAMES[5]);raw['clients'][0]['events']=[e for e in raw['clients'][0]['events'] if e['name']!='set_player_inventory']
        with self.assertRaises(AssertionError): a.inspect(raw, ITEMS)

    def test_break_needs_gear_before_first_hit_empty_slot_break_event_and_more_damage(self):
        for kind in ['initial','empty','status','damage']:
            raw=fixture(a.NAMES[7]);es=raw['clients'][0]['events']
            if kind=='initial': es[:]=[e for e in es if e!=chest(239,1)]
            if kind=='empty': es[:]=[e for e in es if e!=chest(None,8)]
            if kind=='status': es[:]=[e for e in es if e['name']!='entity_status']
            if kind=='damage': es[-1]=hp(7.4,28)
            with self.subTest(kind=kind),self.assertRaises(AssertionError): a.inspect(raw,ITEMS)

    def test_nonbreaking_wear_cannot_disappear(self):
        raw=fixture(a.NAMES[6]);raw['clients'][0]['events'].append(chest(None, 30))
        with self.assertRaises(AssertionError): a.inspect(raw,ITEMS)

    def test_corrupt_inventory_and_health_rejected(self):
        for payload in [b'\x26\1',b'\x26\1\x64\1\0\x7f',b'\x26\0\0']:
            with self.assertRaises((AssertionError,EOFError)): a.inventory_changes(packet('set_player_inventory',payload))
        for value in [float('nan'),float('inf'),0,21]:
            with self.assertRaises(AssertionError): a.health_packet(hp(value,1))

    def test_native_setup_and_old_suites_are_preserved(self):
        import record777
        self.assertEqual(json.loads(a.CONFIG.read_text())['scenarios'],a.NAMES)
        for name in a.NAMES:
            s=json.loads((a.ci.ROOT/f'tools/blackbox/scenarios/{name}.json').read_text());record777.validate(s)
            self.assertEqual(s['native_phase'],'all');self.assertNotIn('!syncattrs',s['setup']);self.assertFalse(s.get('attr_phase',False))
        self.assertEqual(len(json.loads(a.ci.SOURCES.read_text())['scenarios']),15)
        sources=a.ci.helper_sources(entries=[a.Path(a.__file__)])
        self.assertIn(a.ci.ROOT/'tools/blackbox/defense_ci.py',sources)

    def test_six_native_copies_keep_old_actions_and_random_rules(self):
        for name in a.NAMES[:6]:
            old=json.loads((a.ci.ROOT/f'tools/blackbox/scenarios/{name[:-4]}.json').read_text())
            new=json.loads((a.ci.ROOT/f'tools/blackbox/scenarios/{name}.json').read_text())
            self.assertEqual(new['actions'],old['actions']);self.assertEqual(new['random'],old['random'])
            self.assertEqual(new['setup'],[c for c in old['setup'] if c!='!syncattrs'])

    def test_effect_late_or_for_wrong_player_fails(self):
        for wrong in ['late','recipient']:
            raw=fixture(a.NAMES[2]);event=next(e for e in raw['clients'][1]['events'] if e['name']=='entity_effect')
            if wrong=='late':event['at']=11
            else:event['hex']='02'+event['hex'][2:]
            with self.subTest(wrong=wrong),self.assertRaises(AssertionError):a.inspect(raw,ITEMS)


if __name__=='__main__':unittest.main()
