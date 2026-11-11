"""Положительные доказательства реальной схемы Recorder и строгие отказы."""
import copy
import hashlib
import json
import struct
import unittest
from fire_stacking_phase import inspect_phase

# Синтетический охранный пример формата Recorder, не реальная запись.
BASE = {'scenario': 'pvp-fire-stack-plain-777',
 'protocol': 777,
 'start': 2.0,
 'clock': 0.0,
 'origin': [100, -59, 100],
 'initial': [{'entity': 17, 'health': 20.0, 'entities': []},
             {'entity': 23, 'health': 20.0, 'entities': []}],
 'isolate_vitals': True,
 'sent': [{'at': 2.0,
           'tick': 0,
           'action': {'after': 0, 'cmd': 'give attack sword'},
           'pose': [0, 0, 0]},
          {'at': 3.0,
           'tick': 20,
           'action': {'after': 20, 'attack': True, 'bot': 2},
           'pose': [0, 0, 0]}],
 'attribute_phase_samples': [[], [1.9, 3.1]],
 'attribute_phase_receipts': [[],
                              [{'at': 1.9,
                                'name': 'entity_update_attributes',
                                'hex': '170105401000000000000000'},
                               {'at': 3.1,
                                'name': 'entity_update_attributes',
                                'hex': '170105401000000000000000'}]],
 'clients': [{'events': [], 'failure': None},
             {'events': [{'at': 2.5, 'name': 'entity_metadata', 'hex': '17000000ff'},
                         {'at': 3.10001,
                          'name': 'entity_update_attributes',
                          'hex': '170105401000000000000000'}],
              'failure': None}],
 'phase_alignment': {'declared_policy': 'all',
                     'initial_start_tick': 39,
                     'reference_sha256': None,
                     'verified': True,
                     'constraints': [{'source': 'player_attributes',
                                      'period': 2,
                                      'residue': 0,
                                      'sample_tick': 38,
                                      'desired_phase': 0,
                                      'attacker': 1}],
                     'policy': 'all',
                     'observed_constraints': [],
                     'applied_constraints': [{'source': 'player_attributes',
                                              'period': 2,
                                              'residue': 0,
                                              'sample_tick': 38,
                                              'desired_phase': 0,
                                              'attacker': 1}],
                     'current_origin_tick': None,
                     'final_start_tick': 40}}

class ActualShape(unittest.TestCase):
    def test_real_recorder_shape_not_pre_post_arrays(self):
        result = inspect_phase(copy.deepcopy(BASE), (5, 35))
        self.assertEqual(result['pre_samples_count'], 1)

    def test_reference_uses_file_hash_and_actual_entity_ids(self):
        reference = copy.deepcopy(BASE)
        ours = copy.deepcopy(BASE)
        digest = hashlib.sha256(json.dumps(reference).encode()).hexdigest()
        ours['phase_alignment']['reference_sha256'] = digest
        result = inspect_phase(ours, (5, 35), reference, digest)
        self.assertEqual(result['actual_post_attack_count'], 1)

    def test_other_participant_is_not_attacker_phase(self):
        raw = copy.deepcopy(BASE)
        for r in raw['attribute_phase_receipts'][1]:
            r['hex'] = '11' + r['hex'][2:]
        raw['clients'][1]['events'][-1]['hex'] = raw['attribute_phase_receipts'][1][-1]['hex']
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_false_sample_tick_same_parity_still_rejected(self):
        raw = copy.deepcopy(BASE)
        for c in [raw['phase_alignment']['constraints'][0], raw['phase_alignment']['applied_constraints'][0]]:
            c['sample_tick'] += 2
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_final_start_must_actually_satisfy_constraint(self):
        raw = copy.deepcopy(BASE)
        raw['attribute_phase_samples'][1][0] = 1.95
        raw['attribute_phase_receipts'][1][0]['at'] = 1.95
        for c in [raw['phase_alignment']['constraints'][0], raw['phase_alignment']['applied_constraints'][0]]:
            c['sample_tick'], c['residue'] = 39, 1
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_only_post_start_samples_do_not_establish_preparation(self):
        raw = copy.deepcopy(BASE)
        raw['attribute_phase_samples'][1].pop(0)
        raw['attribute_phase_receipts'][1].pop(0)
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_empty_constraints_vacuous_true_rejected(self):
        raw = copy.deepcopy(BASE)
        raw['phase_alignment']['constraints'] = []
        raw['phase_alignment']['applied_constraints'] = []
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_post_start_event_is_real_and_same_payload(self):
        raw = copy.deepcopy(BASE)
        raw['clients'][1]['events'][-1]['hex'] += '00'
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_sample_receipt_time_binding_required(self):
        raw = copy.deepcopy(BASE)
        raw['attribute_phase_samples'][1][0] += .0001
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_reference_digest_mismatch_rejected(self):
        raw = copy.deepcopy(BASE)
        raw['phase_alignment']['reference_sha256'] = 'a' * 64
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35), copy.deepcopy(BASE), 'b' * 64)

    def test_payload_is_complete_finite_and_contains_primary_speed(self):
        original = bytes.fromhex(BASE['attribute_phase_receipts'][1][0]['hex'])
        malformed = [original[:-1], original + b'\0', b'\x80' * 5,
                     b'\x17\1\x23' + struct.pack('>d', 4.) + b'\0',
                     b'\x17\1\5' + struct.pack('>d', float('nan')) + b'\0']
        for payload in malformed:
            raw = copy.deepcopy(BASE)
            raw['attribute_phase_receipts'][1][0]['hex'] = payload.hex()
            with self.assertRaises(ValueError):
                inspect_phase(raw, (5, 35))

    def test_boolean_phase_and_wrong_attacker_index_rejected(self):
        for field, value in [('desired_phase', True), ('attacker', 23)]:
            raw = copy.deepcopy(BASE)
            for c in [raw['phase_alignment']['constraints'][0], raw['phase_alignment']['applied_constraints'][0]]:
                c[field] = value
            with self.assertRaises(ValueError):
                inspect_phase(raw, (5, 35))

    def test_missing_post_attack_receipt_and_event_rejected(self):
        raw = copy.deepcopy(BASE)
        raw['attribute_phase_samples'][1].pop()
        raw['attribute_phase_receipts'][1].pop()
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))

    def test_final_time_and_reference_desired_phase_must_be_real(self):
        raw = copy.deepcopy(BASE)
        raw['start'] += .05
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35))
        raw = copy.deepcopy(BASE)
        raw['phase_alignment']['reference_sha256'] = 'a' * 64
        for c in [raw['phase_alignment']['constraints'][0], raw['phase_alignment']['applied_constraints'][0]]:
            c['desired_phase'] = 1
        with self.assertRaises(ValueError):
            inspect_phase(raw, (5, 35), copy.deepcopy(BASE), 'a' * 64)


if __name__ == '__main__':
    unittest.main()
