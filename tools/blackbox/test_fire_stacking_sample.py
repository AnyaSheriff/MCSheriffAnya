"""Негативные контроли собственных сырых пакетов; никакого Minecraft-процесса."""
import copy
import json
from pathlib import Path
import struct
import unittest

import fire_stacking_sample as sample
import fire_stacking_ci as driver
from smoke import varint

ITEMS = {'diamond_helmet': 100, 'diamond_chestplate': 101, 'diamond_leggings': 102,
         'diamond_boots': 103, 'diamond_sword': 104}
CASE = {'name': 'pvp-fire-stack-chest4-777', 'levels': [0, 4, 0, 0]}


def packet(name, payload, at):
    return {'name': name, 'hex': payload.hex(), 'at': at}


def hp(value, at):
    return packet('update_health', struct.pack('>f', value) + b'\x14' + struct.pack('>f', 5), at)


def stack(item, enchant=None):
    result = b'\1' + varint(ITEMS[item]) + bytes([int(enchant is not None), 0])
    if enchant is not None:
        result += b'\x0d\1' + varint(enchant[0]) + varint(enchant[1])
    return result


def fixture(burn=True):
    registry = {'minecraft:damage_type': ['minecraft:player_attack', 'minecraft:on_fire'],
                'minecraft:enchantment': ['minecraft:fire_protection', 'minecraft:fire_aspect']}
    raw = {'scenario': CASE['name'], 'protocol': 777, 'isolate_vitals': True, 'start': 10.,
           'initial': [{'entity': eid, 'health': 20., 'health_receipt': hp(20., 9.5)} for eid in [2, 3]],
           'clients': [{'events': [], 'failure': None, 'registry_names': copy.deepcopy(registry)} for _ in range(2)],
           'sent': [{'at': 11., 'action': {'after': 20, 'attack': True, 'bot': 2}}]}
    for (item, owner, wire), level in zip(sample.GEAR, CASE['levels']):
        value = stack(item, (0, level) if level else None)
        raw['clients'][0]['events'].append(packet('set_slot', b'\0\1' + struct.pack('>h', owner) + value, 10.1))
        raw['clients'][1]['events'].append(packet('entity_equipment', varint(2) + bytes([wire]) + value, 10.2))
    sword = stack('diamond_sword', (1, 1))
    raw['clients'][1]['events'].append(packet('set_slot', b'\0\1' + struct.pack('>h', 36) + sword, 10.1))
    raw['clients'][0]['events'].append(packet('entity_equipment', varint(3) + b'\0' + sword, 10.2))
    for client in raw['clients']:
        client['events'].append(packet('damage_event', b'\2\0\4\4\0', 11.02))
        if burn:
            client['events'].extend([packet('entity_metadata', b'\2\0\0\1\xff', 11.04),
                                     packet('damage_event', b'\2\1\0\0\0', 12.02),
                                     packet('entity_metadata', b'\2\0\0\0\xff', 12.6)])
    raw['clients'][0]['events'].append(hp(18., 11.03))
    if burn:
        raw['clients'][0]['events'].append(hp(17.5, 12.03))
    for client in raw['clients']:
        client['events'].sort(key=lambda row: row['at'])
    return raw


class RawGuards(unittest.TestCase):
    def test_actual_complete_worn_slots_sword_both_observers(self):
        for burn in [True, False]:
            result = sample.inspect(fixture(burn), CASE, ITEMS)
            self.assertEqual(result['evidence']['gear_verified'], [True, True])
            self.assertTrue(all(result['actual_pre_attack_receipts']))

    def test_missing_owner_and_observer_gear_or_weapon_rejected(self):
        for observer, name in [(0, 'set_slot'), (1, 'entity_equipment'),
                               (1, 'set_slot'), (0, 'entity_equipment')]:
            raw = fixture()
            raw['clients'][observer]['events'] = [e for e in raw['clients'][observer]['events'] if e['name'] != name]
            with self.assertRaises(AssertionError):
                sample.inspect(raw, CASE, ITEMS)

    def test_late_gear_and_wrong_enchantment_rejected(self):
        raw = fixture()
        for e in raw['clients'][1]['events']:
            if e['name'] == 'entity_equipment':
                e['at'] = 11.01
        raw['clients'][1]['events'].sort(key=lambda e: e['at'])
        with self.assertRaises(AssertionError):
            sample.inspect(raw, CASE, ITEMS)
        raw = fixture()
        case = copy.deepcopy(CASE)
        case['levels'][1] = 3
        with self.assertRaises(AssertionError):
            sample.inspect(raw, case, ITEMS)

    def test_real_initial_health_and_no_failure_required(self):
        for modify in [lambda r: r['initial'][0].pop('health_receipt'),
                       lambda r: r['clients'][1].update(failure='socket closed'),
                       lambda r: r.update(sent=[])]:
            raw = fixture()
            modify(raw)
            with self.assertRaises((AssertionError, KeyError)):
                sample.inspect(raw, CASE, ITEMS)

    def test_equipment_truncation_tail_duplicate_and_invalid_slot(self):
        for payload in [b'\2', b'\2\6\0', b'\2\0\0\0', b'\2\x80\0\0\0']:
            with self.assertRaises((AssertionError, EOFError)):
                sample.equipment(packet('entity_equipment', payload, 1))

    def test_effect_or_death_invalidates_burn_trial(self):
        for event in [packet('entity_effect', b'', 10.5), packet('entity_status', struct.pack('>iB', 2, 3), 11.5)]:
            raw = fixture()
            raw['clients'][0]['events'].append(event)
            with self.assertRaises(AssertionError):
                sample.inspect(raw, CASE, ITEMS)

    def test_frozen_cases_equal_base_armor_and_no_new_masks(self):
        root = Path(__file__).resolve().parents[2]
        config = json.loads(driver.CONFIG.read_text())
        original = json.loads((root / 'tools/blackbox/scenarios/pvp-fire-protection-iv-777.json').read_text())
        self.assertEqual(len(config['cases']), 6)
        for row in config['cases']:
            path = root / 'tools/blackbox/scenarios' / (row['name'] + '.json')
            self.assertEqual(driver.ci.sha(path), row['scenario_sha256'])
            scenario = json.loads(path.read_text())
            self.assertEqual(scenario['random'], original['random'])
            self.assertEqual(scenario['actions'][-1], {'after': 20, 'attack': True, 'bot': 2})
            for item, _, _ in sample.GEAR:
                self.assertEqual(sum(('with minecraft:' + item) in a.get('cmd', '') for a in scenario['actions']), 1)


if __name__ == '__main__':
    unittest.main()
