import copy
import io
import json
from pathlib import Path
import struct
import sys
import unittest

from crossbow_unbreaking_ci import ROOT, require_statistical_pass, validate_readiness
from crossbow_unbreaking_probe import CRITERIA_SHA256, criteria, statistics


def rows_for_counts(counts=(32, 24, 18, 54)):
    def control(i):
        return dict(trial=i, control=True, category='plain_fresh_control', emitted=3,
                    after=dict(item='crossbow', count=1, damage=3))
    rows = [control(0), control(1)]
    for category, count in zip(criteria()['categories'], counts):
        for _ in range(count):
            rows.append(dict(trial=len(rows), control=False, category=category))
    return rows + [control(130), control(131)]


def readiness_fixture():
    sys.path.insert(0, str(ROOT / 'tools/protocol-bridge'))
    from smoke import varint
    pose = [103.5, -59., 103.5, 0., -20.]
    wire = lambda name, at, payload: dict(name=name, at=at, hex=payload.hex())
    return dict(start=0., causal_menu_readiness=True,
                sent=[dict(at=1., action=dict(cmd='tp $(bot) $(3.5,0,3.5) 0 -20')),
                      dict(at=2., action=dict(after=10, click=[5, 0, 3])),
                      dict(at=2.2, action=dict(close=True)),
                      dict(at=3., action=dict(use=True)),
                      dict(at=4., action=dict(after=28, click=[5, 0, 3])),
                      dict(at=4.2, action=dict(close=True))],
                commands=[dict(command='tp blackbox 103.5 -59 103.5 0 -20', fresh_tp_ack=dict(id=1, at=1.2, pose=pose)),
                          dict(readiness='actual-open-window-and-full-items', action_after=10, received_at=2.1, window=7),
                          dict(readiness='actual-open-window-and-full-items', action_after=28, received_at=4.1, window=8),
                          dict(command='kill @e[type=arrow]', via='OWN isolated RCON', issued_at=.5,
                               all_old_ids_gone_at=.8, old_arrow_ids_before=[[42], [42]], old_arrow_ids_still_tracked=[[], []])],
                clients=[dict(events=[dict(name='window_items', at=2.1), dict(name='window_items', at=4.1),
                                      wire('entity_destroy', .7, varint(1) + varint(42))],
                              wire_sent=[wire('teleport_confirm', 1.1, varint(1) + struct.pack('>dddff', *pose)),
                                         wire('close_window', 2.21, varint(7)),
                                         wire('use_item', 3.01, varint(0) + varint(1) + struct.pack('>ff', 0., -20.)),
                                         wire('close_window', 4.21, varint(8))]),
                         dict(events=[wire('entity_destroy', .7, varint(1) + varint(42))])])


class UnbreakingFixedSampleTests(unittest.TestCase):
    def test_original_primary_is_immutable(self):
        import hashlib
        config = json.loads((ROOT / 'tools/blackbox/ci/crossbow-ub128.json').read_text())
        raw = (ROOT / config['primary_reference']).read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), config['primary_reference_sha256'])
        primary = json.loads(raw)
        self.assertEqual(primary['statistics']['counts'], dict(one_arrow_then_break=29, two_arrows_then_break=19,
                                                              three_arrows_then_break=19, three_arrows_survived=61))
        self.assertEqual(primary['fixed_probability_criteria_sha256'], CRITERIA_SHA256)

    def test_declared_counts_pass_and_known_bad_baseline_fails(self):
        good = statistics(rows_for_counts())
        require_statistical_pass(dict(official=good, ours=copy.deepcopy(good)))
        bad = statistics(rows_for_counts((0, 0, 82, 46)))
        self.assertFalse(bad['passed'])
        with self.assertRaises(AssertionError):
            require_statistical_pass(dict(official=good, ours=bad))

    def test_sample_size_and_all_four_fresh_controls_are_mandatory(self):
        with self.assertRaises(AssertionError):
            statistics(rows_for_counts()[:-1])
        changed = rows_for_counts()
        changed[0]['after'] = None
        with self.assertRaises(AssertionError):
            statistics(changed)

    def test_good_readiness_requires_actual_ack_menu_close_and_old_ID_removal(self):
        self.assertEqual(validate_readiness(readiness_fixture(), expected_trials=1)['actual_TP_ACK_wire'], 1)

    def test_missing_or_corrupted_ack_is_rejected(self):
        changed = readiness_fixture()
        changed['clients'][0]['wire_sent'][0]['hex'] += '00'
        with self.assertRaises(AssertionError):
            validate_readiness(changed, expected_trials=1)
        changed = readiness_fixture()
        changed['clients'][0]['wire_sent'] = changed['clients'][0]['wire_sent'][1:]
        with self.assertRaises(AssertionError):
            validate_readiness(changed, expected_trials=1)

    def test_close_before_full_menu_and_missing_old_ID_deletion_are_rejected(self):
        changed = readiness_fixture()
        changed['sent'][2]['at'] = 2.05
        with self.assertRaises(AssertionError):
            validate_readiness(changed, expected_trials=1)
        changed = readiness_fixture()
        changed['clients'][1]['events'] = []
        with self.assertRaises(AssertionError):
            validate_readiness(changed, expected_trials=1)


if __name__ == '__main__':
    unittest.main()
