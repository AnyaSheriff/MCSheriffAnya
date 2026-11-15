"""Отрицательные записи обязаны ломать проверку, а не давать ложное совпадение."""
import copy
import unittest
try:
    from .report import compare_case, compare_reports, CLEANUP
except ImportError:
    from report import compare_case, compare_reports, CLEANUP


def fixture():
    case = dict(id='cow.turn',mob='cow',family='turn',policy='exact',
                phases=[dict(id='turned')], compare_fields=['position','rotation','health'],
                reference_requirements=['spawn_observed','rotation_changed'])
    first = dict(entity=10,entity_name='cow',uuid='subject',position=[4.5,101,.5],yaw=37,pitch=0,health=10.,removed=False)
    last = copy.deepcopy(first); last['yaw'] = 127
    spawn = dict(name='spawn_entity',entity=10,status='decoded',state=first)
    row = dict(id=case['id'],mob='cow',status='RECORDED',entity=10,initial=first,
               events=[spawn],phases=[dict(id='turned',snapshot=last,events=[])])
    return case,copy.deepcopy(row),copy.deepcopy(row)


class ReportTests(unittest.TestCase):
    def status(self, case, a, b): return compare_case(case,a,b)['status']

    def test_actual_match_and_wrong_turn(self):
        c,a,b=fixture(); self.assertEqual(self.status(c,a,b),'MATCH')
        b['phases'][0]['snapshot']['yaw']=37
        self.assertEqual(self.status(c,a,b),'DIFFERENT')

    def test_both_missing_health_not_match(self):
        c,a,b=fixture()
        for row in (a,b): row['initial'].pop('health')
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')

    def test_empty_trace_and_noop_reference(self):
        c,a,b=fixture();a['events']=[]
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')
        c,a,b=fixture();a['phases'][0]['snapshot']['yaw']=37
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')

    def test_wrong_symbolic_mob_is_not_match(self):
        c,a,b=fixture();a['initial']['entity_name']='pig';b['initial']['entity_name']='pig'
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')

    def test_unknown_predicate_or_field(self):
        for location in ('reference_requirements','compare_fields'):
            c,a,b=fixture();c[location].append('imaginary_check')
            self.assertEqual(self.status(c,a,b),'UNSUPPORTED_CHECK')

    def test_missing_candidate_and_decoder_failure(self):
        c,a,b=fixture();self.assertEqual(self.status(c,a,None),'MISSING_CANDIDATE')
        b['status']='ERROR';b['error']='missing subject'
        self.assertEqual(self.status(c,a,b),'MISSING_CANDIDATE')
        c,a,b=fixture();b['events'][0]['status']='decode_error'
        self.assertEqual(self.status(c,a,b),'HARNESS_ERROR')

    def test_circular_angle_and_position_precision(self):
        c,a,b=fixture();a['phases'][0]['snapshot']['yaw']=359.8;b['phases'][0]['snapshot']['yaw']=.2
        self.assertEqual(self.status(c,a,b),'MATCH')
        b['phases'][0]['snapshot']['position'][0]+=.001
        self.assertEqual(self.status(c,a,b),'DIFFERENT')

    def test_nan_and_incomplete_phases(self):
        for value in (float('nan'),float('inf'),None):
            c,a,b=fixture();a['initial']['health']=value
            self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')
        c,a,b=fixture();b['phases']=[]
        self.assertEqual(self.status(c,a,b),'DIFFERENT')
        a['phases']=[]
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')

    def test_predicate_motion_validates_reference_first(self):
        c,a,b=fixture();c.update(policy='predicate',compare_fields=['position'],reference_requirements=['spawn_seen','moves'])
        self.assertEqual(self.status(c,a,b),'INCONCLUSIVE')
        a['phases'][0]['snapshot']['position'][0]+=2
        self.assertEqual(self.status(c,a,b),'DIFFERENT')
        b['phases'][0]['snapshot']['position'][2]+=3
        self.assertEqual(self.status(c,a,b),'MATCH')

    def test_subject_only_death_and_removal(self):
        c,a,b=fixture();c.update(policy='predicate',compare_fields=['status','removed'],reference_requirements=['death_event_observed','removal_observed'])
        for row in (a,b):
            row['events'] += [dict(name='entity_status',entity=999,entity_status=3),dict(name='entity_destroy',entities=[999])]
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')
        for row in (a,b):
            row['events'] += [dict(name='entity_status',entity=10,entity_status=3),dict(name='entity_destroy',entities=[10])]
        self.assertEqual(self.status(c,a,b),'MATCH')

    def test_ai_thresholds_and_out_and_back(self):
        try:
            from .report import requirement
        except ImportError:
            from report import requirement
        c,a,b=fixture()
        for distance,moves,stationary in ((.079,False,True),(.08,False,True),(.081,False,False),
                                           (.2499,False,False),(.25,True,False)):
            a['phases'][0]['snapshot']['position']=[4.5+distance,101,.5]
            self.assertEqual(requirement('moves',a),moves)
            self.assertEqual(requirement('stationary',a),stationary)
        # Диагональное перемещение оценивается по евклидову расстоянию.
        a['phases'][0]['snapshot']['position']=[4.5+.18,101+.18,.5]
        self.assertTrue(requirement('moves',a))
        a['phases'][0]['snapshot']['position']=[4.5,101,.5]
        a['events'].append(dict(name='sync_entity_position',entity=10,
                               state=dict(a['initial'],position=[5,101,.5])))
        self.assertTrue(requirement('moves',a))
        self.assertFalse(requirement('stationary',a))
        for drop,expected in ((.999,False),(1,True)):
            a['phases'][0]['snapshot']['position']=[4.5,101-drop,.5]
            self.assertEqual(requirement('falls',a),expected)
        a['events']=a['events'][:1]
        for yaw,expected in ((46.99,False),(47,True)):
            a['phases'][0]['snapshot']['yaw']=yaw
            self.assertEqual(requirement('yaw_changes',a),expected)

    def test_duplicate_feedback_differs(self):
        c,a,b=fixture();c['compare_fields']=['damage','status']
        for row in (a,b):
            row['phases'][0]['events']=[dict(name='damage_event',entity=10),
                                      dict(name='entity_status',entity=10,entity_status=2)]
        self.assertEqual(self.status(c,a,b),'MATCH')
        b['phases'][0]['events'].append(dict(name='damage_event',entity=10))
        self.assertEqual(self.status(c,a,b),'DIFFERENT')
        b['phases'][0]['events']=copy.deepcopy(a['phases'][0]['events'])
        b['phases'][0]['events'].append(dict(name='entity_status',entity=10,entity_status=2))
        self.assertEqual(self.status(c,a,b),'DIFFERENT')

    def test_metadata_suffix_scoped_and_missing_prefix(self):
        c,a,b=fixture()
        for row in (a,b):
            row['events'].append(dict(name='entity_metadata',entity=10,status='unparsed_metadata',
                                     unparsed=dict(index=17,type=99)))
        result=compare_case(c,a,b)
        self.assertEqual(result['status'],'MATCH')
        self.assertEqual(len(result['limitations']),2)
        a['initial'].pop('health')
        self.assertEqual(self.status(c,a,b),'INVALID_REFERENCE')
        c,a,b=fixture();b['initial'].pop('health')
        b['events'].append(dict(name='entity_metadata',entity=10,status='unparsed_metadata',
                               unparsed=dict(index=8,type=99)))
        self.assertEqual(self.status(c,a,b),'DIFFERENT')

    def test_unrelated_decode_noise_ignored_subject_rejected(self):
        c,a,b=fixture()
        b['events'].append(dict(name='entity_metadata',entity=999,status='decode_error'))
        self.assertEqual(self.status(c,a,b),'MATCH')
        b['events'].append(dict(name='entity_teleport',entity=10,status='unparsed_relative_teleport'))
        self.assertEqual(self.status(c,a,b),'HARNESS_ERROR')

    def test_duplicate_missing_ids_cleanup_and_pins(self):
        c,a,b=fixture()
        def side(row):return dict(source='commit',identities={'protocol.json':'p','entities.json':'e','cases_sha256':'c'},cleanup=dict(CLEANUP),cases=[row])
        ref=side(a);candidate=side(b)
        self.assertTrue(compare_reports(ref,candidate,[c])['gate_pass'])
        candidate['cases']=[]
        self.assertEqual(compare_reports(ref,candidate,[c])['counts'],{'MISSING_CANDIDATE':1})
        candidate=side(b);candidate['cases'][0].pop('id')
        self.assertEqual(compare_reports(ref,candidate,[c])['counts'],{'HARNESS_ERROR':1})
        c,a,b=fixture()
        candidate=side(b);candidate['cases'].append(b)
        self.assertEqual(compare_reports(ref,candidate,[c])['counts'],{'HARNESS_ERROR':1})
        candidate=side(b);candidate['cleanup']['port_free']=False
        self.assertFalse(compare_reports(ref,candidate,[c])['gate_pass'])
        candidate=side(b);candidate['identities']['entities.json']='wrong'
        self.assertEqual(compare_reports(ref,candidate,[c])['counts'],{'HARNESS_ERROR':1})


if __name__ == '__main__':unittest.main()
