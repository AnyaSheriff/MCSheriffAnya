import copy
import struct
import unittest

from thorns_natural_sample import NAME, binomial_bounds, damage, inspect
from smoke import varint


def event(name, at, payload):
    return {'name': name, 'at': at, 'hex': payload.hex()}


def hp(value, at):
    return event('update_health', at, struct.pack('>f', value)+varint(20)+struct.pack('>f', 5))


def slot(wear, at):
    # Собственная минимальная Slot777: предмет, damage и ThornsIII.
    stack = b''.join(varint(x) for x in [1, 123, 2, 0, 3, wear, 13, 1, 0, 3])
    return event('set_slot', at, b'\0\0'+struct.pack('>h', 6)+stack)


def hit(entity, kind, owner):
    return event('damage_event', 10.7, b''.join(varint(x) for x in [entity, kind, owner+1, owner+1])+b'\0')


def fixture():
    raw = {'scenario': NAME, 'protocol': 777, 'isolate_vitals': True, 'start': 10,
           'initial': [{'entity': entity, 'health': 20, 'health_receipt': hp(20, 9.9)} for entity in [5, 9]],
           'sent': [{'action': {'after': 12, 'attack': True, 'bot': 2}, 'at': 10.6}], 'clients': []}
    for side in [0, 1]:
        events = ([slot(0, 10.2)] if side == 0 else [])+[hit(5, 0, 9), hit(9, 1, 5), hp(19 if side == 0 else 17, 10.8)]
        if side == 0:
            events.append(slot(3, 10.9))
        raw['clients'].append({'events': events, 'failure': None, 'registry_names': {
            'minecraft:damage_type': ['minecraft:player_attack', 'minecraft:thorns'],
            'minecraft:enchantment': ['minecraft:thorns']}})
    return raw


class NaturalThornsTest(unittest.TestCase):
    def analyze(self, raw):
        return inspect(raw, {'levels': [3], 'smoke': True}, {'diamond_chestplate': 123})

    def test_actual_hp_both_owners_and_wear_required(self):
        result = self.analyze(fixture())
        self.assertEqual(result['natural_reflections'], 1)
        self.assertEqual(result['trials'][0]['reflected_damage'], 3)
        self.assertEqual(result['trials'][0]['wear'], 3)

    def test_missing_observer_or_wrong_source_cannot_pass(self):
        for change in ['missing', 'wrong-owner', 'wrong-source']:
            raw = fixture()
            if change == 'missing':
                raw['clients'][1]['events'].pop(1)
            else:
                raw['clients'][1]['events'][1] = hit(9, 0 if change == 'wrong-source' else 1, 9 if change == 'wrong-owner' else 5)
            with self.subTest(change=change), self.assertRaises(AssertionError):
                self.analyze(raw)

    def test_reflection_without_health_loss_or_correct_wear_fails(self):
        raw = fixture();raw['clients'][1]['events'][-1] = hp(20, 10.8)
        with self.assertRaises(AssertionError):
            self.analyze(raw)
        raw = fixture();raw['clients'][0]['events'][-1] = slot(1, 10.9)
        with self.assertRaises(AssertionError):
            self.analyze(raw)

    def test_missing_real_hit_initial_vitals_or_correct_gear_fails(self):
        for change in ['hit', 'vitals', 'gear']:
            raw = fixture()
            if change == 'hit':
                raw['sent'] = []
            elif change == 'vitals':
                raw['initial'][0]['health_receipt'] = hp(19, 9.9)
            else:
                raw['clients'][0]['events'][0] = slot(1, 10.2)
            with self.subTest(change=change), self.assertRaises(AssertionError):
                self.analyze(raw)

    def test_trailing_damage_bytes_and_broken_slot_rejected(self):
        value = hit(5, 0, 9);value['hex'] += '00'
        with self.assertRaises(AssertionError):
            damage(value, ['minecraft:player_attack', 'minecraft:thorns'])
        raw = fixture();raw['clients'][0]['events'][0]['hex'] += '00'
        with self.assertRaises(AssertionError):
            self.analyze(raw)

    def test_binomial_bounds_have_declared_tail_probability(self):
        import math
        n = 256;low, high = binomial_bounds(n)
        probability = lambda k: math.comb(n, k)*.45**k*.55**(n-k)
        self.assertLess(sum(probability(k) for k in range(low)), .0005)
        self.assertLess(sum(probability(k) for k in range(high+1, n+1)), .0005)
        self.assertGreaterEqual(sum(probability(k) for k in range(low+1)), .0005)
        self.assertGreaterEqual(sum(probability(k) for k in range(high, n+1)), .0005)
        self.assertLess(low, n*.45)
        self.assertGreater(high, n*.45)


if __name__ == '__main__':
    unittest.main()
