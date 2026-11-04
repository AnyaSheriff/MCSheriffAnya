"""Pure selected packet replay/parser tests, not an extra server run."""
import copy
import io
import json
import math
from pathlib import Path
import struct
import unittest

import probe_player_floor_777 as probe


class PlayerFloorParsers(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.record = json.loads((Path(__file__).parent/'observations/player-floor-selected.json').read_text())
        cls.raw = cls.record['raw']

    def test_actual24_intrusion_wire_and_exact_server_readback(self):
        self.assertEqual(len(self.record['cases']), 24)
        for case in self.record['cases']:
            if case['kind'] != 'intrusion':
                continue
            with self.subTest(label=case['label']):
                packets = [r for r in self.raw if r['direction']=='out' and r['name']=='position'
                           and case['start'] <= r['at'] <= case['end']]
                self.assertEqual(len(packets), 1)
                self.assertEqual(bytes.fromhex(packets[0]['hex']), struct.pack('>dddB', *case['requested_endpoint'], case['flag']))
                fields = [r for r in self.record['server_fields_history'] if case['start'] <= r['at'] <= case['end']]
                self.assertEqual([r['field'] for r in fields], ['Pos', 'OnGround'])
                self.assertEqual(probe.known.parse_entity_data('Pos', fields[0]['raw']), case['after']['position'])
                self.assertIs(probe.known.parse_entity_data('OnGround', fields[1]['raw']), case['after']['ground'])
        tiny = [c for c in self.record['cases'] if c.get('nearcontact') and c['depth']==1e-7]
        self.assertEqual(len(tiny), 2)
        self.assertTrue(all(c['after']['position'][1] == 64.9999999 and c['after']['ground'] for c in tiny))

    def test_tiny_endpoint_labels_do_not_hide_acceptance_in_old_point02_tolerance(self):
        # Synthetic classification controls; no claim about unsampled thresholds.
        for depth in [1e-7, 1e-5, .0001, .001, .01, .0625]:
            before=[.5,65,.5]; requested=[.5,65-depth,.5]
            with self.subTest(depth=depth):
                self.assertEqual(probe.endpoint_observation(before,requested,requested), 'requested_endpoint_observed')
                self.assertEqual(probe.endpoint_observation(before,requested,before), 'endpoint_unchanged')

    def test_airborne_three_endpoints_remain_distinguishable(self):
        before=[.5,65.5,.5]; requested=[.5,64.875,.5]
        labels=[probe.endpoint_observation(before,requested,[.5,y,.5]) for y in [65.5,65,64.875]]
        self.assertEqual(len(set(labels)),3)

    def test_actual_teleports_have_exact_ack_and_separate_setup_windows(self):
        pairs=probe.teleport_evidence(self.raw,0,math.inf)
        self.assertEqual(len(pairs),45)
        self.assertTrue(all(pair['flags']&7==0 for pair in pairs))
        for case in self.record['cases']:
            with self.subTest(label=case['label']):
                self.assertEqual(probe.teleport_evidence(self.raw,case['start'],case['end']),case['teleports'])
                self.assertEqual(probe.teleport_evidence(self.raw,case['setup_start'],case['setup_end']),case['setup_teleports'])

    def test_missing_or_duplicate_actual_ack_is_not_accepted(self):
        ack=next(r for r in self.raw if r['direction']=='out' and r['name']=='teleport_confirm')
        for malformed in [[r for r in self.raw if r != ack], self.raw+[ack]]:
            with self.assertRaises(ValueError):
                probe.teleport_evidence(malformed,0,math.inf)

    def test_ack_position_difference_of_one_tenth_millionth_is_rejected(self):
        malformed=copy.deepcopy(self.raw)
        ack=next(r for r in malformed if r['direction']=='out' and r['name']=='teleport_confirm')
        source=io.BytesIO(bytes.fromhex(ack['hex']));probe.known.uint31(source);offset=source.tell()
        payload=bytes.fromhex(ack['hex']);x=struct.unpack('>d',payload[offset:offset+8])[0]
        ack['hex']=(payload[:offset]+struct.pack('>d',x+1e-7)+payload[offset+8:]).hex()
        with self.assertRaisesRegex(ValueError,'ACK pose mismatch'):
            probe.teleport_evidence(malformed,0,math.inf)

    def test_truncated_or_trailing_ack_is_rejected(self):
        for truncated in [False,True]:
            malformed=copy.deepcopy(self.raw)
            ack=next(r for r in malformed if r['direction']=='out' and r['name']=='teleport_confirm')
            ack['hex']=ack['hex'][:-2] if truncated else ack['hex']+'00'
            with self.subTest(truncated=truncated),self.assertRaises(ValueError):
                probe.teleport_evidence(malformed,0,math.inf)

    def test_relative_xyz_or_unknown_teleport_flags_are_not_guessed(self):
        for flags in [1,2,4,512]:
            malformed=copy.deepcopy(self.raw)
            row=next(r for r in malformed if r['direction']=='in' and r['name']=='position')
            payload=bytes.fromhex(row['hex']);row['hex']=(payload[:-4]+struct.pack('>I',flags)).hex()
            with self.subTest(flags=flags),self.assertRaises(ValueError):
                probe.teleport_evidence(malformed,0,math.inf)


    def test_actual_core24_same_wire_inputs_readback_and_correction_xyz(self):
        core=json.loads((Path(__file__).parent/'observations/player-floor-core-selected.json').read_text())
        self.assertEqual(len(core['cases']),24)
        self.assertTrue(core['cleanup']['clean'])
        for ordinary,ours in zip(self.record['cases'],core['cases']):
            with self.subTest(label=ordinary['label']):
                self.assertEqual(ordinary['label'],ours['label'])
                for key in ['before','requested_endpoint','after']:
                    self.assertEqual(ordinary[key],ours[key])
                def inputs(raw,case):
                    return [row['hex'] for row in raw if row['direction']=='out' and row['name']=='position'
                            and case['start']<=row['at']<=case['end']]
                self.assertEqual(inputs(self.raw,ordinary),inputs(core['raw'],ours))
                fields=[r for r in core['server_fields_history'] if ours['start']<=r['at']<=ours['end']]
                self.assertEqual([r['field'] for r in fields],['Pos','OnGround'])
                self.assertEqual(probe.known.parse_entity_data('Pos',fields[0]['raw']),ordinary['after']['position'])
                self.assertIs(probe.known.parse_entity_data('OnGround',fields[1]['raw']),ordinary['after']['ground'])
                old=probe.teleport_evidence(self.raw,ordinary['start'],ordinary['end'])
                new=probe.teleport_evidence(core['raw'],ours['start'],ours['end'])
                self.assertEqual(len(old),len(new))
                self.assertEqual([t['position'][:3] for t in old],[t['position'][:3] for t in new])
                # Preserve each side's own previous angles, without equating initial pitch.
                for case,targets in [(ordinary,old),(ours,new)]:
                    previous=case['setup_teleports'][-1]['position'][3:]
                    self.assertTrue(all(t['position'][3:]==previous for t in targets))


if __name__=='__main__':
    unittest.main()
