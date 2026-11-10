"""Synthetic guard destruction only; no original/gameplay proof or local server."""
import copy
import json
import struct
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import tracker_ground_offset_777 as cal


class FakeDecoder:
    def __init__(self, protocol):
        pass
    def read(self, stream, kind):
        return json.load(stream)


def event(name,at,value):
    return dict(name=name,at=at,hex=json.dumps(value).encode().hex())


def receipt(command,at,response=''):
    return dict(command=command,at=at,ended_at=at+.001,response=response)


def fixture():
    fixed=cal.criteria();rows,plans=cal.schedule(fixed)
    start,end=1.,1.+len(rows)*.05
    prefix='synthetic_only'
    clients=[]
    for i,position in enumerate((cal.ACTOR,cal.OBSERVER)):
        tp=dict(teleportId=3,x=position[0],y=position[1],z=position[2],dx=0,dy=0,dz=0,flags={'x':False})
        c=dict(entity=i+1,health=20,failure=None,
               events=[event('login',0.,{}),event('position',.2,tp),
                       event('game_state_change',.3,dict(reason='change_game_mode',gameMode=0)),
                       event('abilities',.3,dict(flags=0))],
               outbound=[dict(name='teleport_confirm',at=.201,issued_at=.2005,hex=(cal.varint(3)+struct.pack('>dddff',*position,0,0)).hex())],
               acks=[dict(id=3,at=.201,pose=list(position)+[0,0])])
        c['events'] += [event('update_time',n+.5,dict(age=n*20)) for n in range(23)]
        clients.append(c)
    clients[1]['events'].append(event('spawn_entity',.4,dict(entityId=1,x=100.5,y=64,z=100.5)))
    geometry=[]
    for i,(x,y,z,state) in enumerate(cal.geometry_checks()):
        token=prefix+'_geometry_'+str(i);r=receipt(f'execute if block {x} {y} {z} minecraft:{state} run say {token}',.31+i*.001)
        geometry.append(r)
        for c in clients:c['events'].append(event('profileless_chat',.35+i*.001,dict(message=token,name='Rcon',type={'chatType':4},target=None)))
    frames=[]
    for row in rows:
        at=start+row['frame']*.05
        frames.append(dict(row,scheduled_at=at,entered_at=at+.0001))
        for i,c in enumerate(clients):
            pos=row['actor_move'] if i==0 else cal.OBSERVER if row['frame']%20==0 else None
            if pos is not None:
                c['outbound'].append(dict(name='position_look',issued_at=at+.001,at=at+.002,hex=struct.pack('>dddffB',*pos,0,0,1).hex()))
                if i==0:
                    clients[1]['events'].append(event('sync_entity_position',at+.01,dict(entityId=1,positionType='stepped',steps=[dict(x=pos[0],y=pos[1],z=pos[2],tickOffset=2)],onGround=True)))
            c['outbound'].append(dict(name='tick_end',issued_at=at+.003,at=at+.004,hex=''))
    queries=[]
    for q in plans:
        at=start+q['frame']*.05
        before=prefix+'_q'+str(q['index'])+'_before';after=prefix+'_q'+str(q['index'])+'_after'
        pos=rows[q['frame']]['expected_current']
        query=dict(q,scheduled_at=at,requested_at=at+.003,started_at=at+.004,ended_at=at+.02,before_token=before,after_token=after,
                   before=receipt('say '+before,at+.004),
                   Pos=receipt('data get entity blackbox Pos',at+.006,'blackbox has the following entity data: ['+', '.join(str(p)+'d' for p in pos)+']'),
                   OnGround=receipt('data get entity blackbox OnGround',at+.008,'blackbox has the following entity data: 1b'),
                   after=receipt('say '+after,at+.01))
        queries.append(query)
        for c in clients:
            c['events'] += [event('profileless_chat',at+.03,dict(message=before,name='Rcon',type={'chatType':4},target=None)),
                            event('profileless_chat',at+.04,dict(message=after,name='Rcon',type={'chatType':4},target=None))]
    final=prefix+'_final_live'
    for c in clients:
        c['events'].append(event('profileless_chat',end+.03,dict(message=final,name='Rcon',type={'chatType':4},target=None)))
        c['events'].sort(key=lambda e:e['at'])
    commands=[]
    for name in ('blackbox','blackboxb'):
        commands += [receipt('data get entity '+name+' Inventory',.7,name+' has the following entity data: []'),
                     receipt('data get entity '+name+' equipment',.71,'Found no elements matching equipment')]
    raw=dict(protocol=777,prefix=prefix,scope='NONCOMBAT_GROUND_ONLY_NOT_WIND_POSE_PROOF',start=start,end=end,
             frames=frames,clients=clients,queries=queries,commands=commands,geometry=geometry,
             initial_teleports=[receipt('tp actor',.19),receipt('tp observer',.19)],final_echo_token=final,final_echo=receipt('say '+final,end+.01))
    return raw,fixed


class GroundOffsetGuards(unittest.TestCase):
    def analyze(self,raw,fixed):
        with patch.object(cal,'Decoder',FakeDecoder):return cal.analyze(raw,{},fixed)

    def test_complete_scaffold_remains_semantically_unresolved(self):
        raw,fixed=fixture();result=self.analyze(raw,fixed)
        self.assertEqual(result['status'],'COMPLETE_NONCOMBAT_OBSERVATIONS_OFFSET_SEMANTICS_UNRESOLVED')
        self.assertFalse(result['future_nonzero_Wind_pose_eligibility'])
        self.assertTrue(all(r['agrees'] for r in result['held_endpoint_observations']))
        self.assertIn('UNIDENTIFIABLE',result['interpolation_vs_sample_timing'])

    def test_fixed_schedule_and_authority_budget(self):
        fixed=cal.criteria();rows,queries=cal.schedule(fixed)
        self.assertEqual(len(rows),412);self.assertEqual(len(queries),11)
        for q in queries:
            room=(q['phase_end_frame']-q['frame'])*.05
            self.assertGreaterEqual(room+1e-9,1.2)
        moves=[r['actor_move'] for r in rows if r['actor_move'] is not None]
        self.assertTrue(all(p[1]==64 and 100.5<=p[0]<=102.5 for p in moves))
        self.assertLessEqual(max(abs(a[0]-b[0]) for a,b in zip(moves,moves[1:])),.125)

    def test_authority_parser_never_uses_local_pose(self):
        self.assertEqual(cal.parse_pos('blackbox has the following entity data: [100.5d, 64.0d, 100.5d]'),cal.ACTOR)
        for text in ['[100.5d,64d,100.5d]', 'other has the following entity data: [100.5d,64d,100.5d]',
                     'blackbox has the following entity data: [100.5d,64d]',
                     'blackbox has the following entity data: [NaN,64d,100.5d]',
                     'blackbox has the following entity data: [100.5d,64d,100.5d] trailing']:
            with self.assertRaises(AssertionError):cal.parse_pos(text)

    def test_missing_or_wrong_authority_is_rejected(self):
        raw,fixed=fixture()
        for mutate in [lambda r:r['queries'].pop(),
                       lambda r:r['queries'][0]['Pos'].update(response=''),
                       lambda r:r['queries'][0]['Pos'].update(response='blackbox has the following entity data: [101.5d,64d,100.5d]'),
                       lambda r:r['queries'][0]['OnGround'].update(response='blackbox has the following entity data: 0b'),
                       lambda r:r['queries'][0].update(started_at=2.),
                       lambda r:r['queries'][0].update(ended_at=10.)]:
            broken=copy.deepcopy(raw);mutate(broken)
            with self.assertRaises(AssertionError):self.analyze(broken,fixed)

    def test_noncombat_and_fresh_pose_guards(self):
        raw,fixed=fixture()
        for mutate in [lambda r:r['clients'][0]['outbound'].append(dict(name='attack',at=2.,hex='02')),
                       lambda r:r['clients'][0]['events'].append(event('explosion',2.,{})),
                       lambda r:r['clients'][0]['events'].append(event('damage_event',2.,{})),
                       lambda r:r['clients'][0]['acks'].clear(),
                       lambda r:r['clients'][0].update(failure='socket gone'),
                       lambda r:r['frames'].pop(),
                       lambda r:r['clients'][0]['events'].append(event('position',2.,{}))]:
            broken=copy.deepcopy(raw);mutate(broken)
            with self.assertRaises(AssertionError):self.analyze(broken,fixed)

    def test_cadence_missing_packet_and_final_liveness(self):
        raw,fixed=fixture()
        def late(r):
            e=next(e for e in r['clients'][0]['outbound'] if e['name']=='position_look')
            e['issued_at']+=.1
        def missing(r):
            i=next(i for i,e in enumerate(r['clients'][0]['outbound']) if e['name']=='position_look')
            r['clients'][0]['outbound'].pop(i)
        def no_final(r):
            token=r['final_echo_token'].encode().hex()
            r['clients'][1]['events']=[e for e in r['clients'][1]['events'] if not (e['name']=='profileless_chat' and token in e['hex'])]
        for mutate in (late,missing,no_final):
            broken=copy.deepcopy(raw);mutate(broken)
            with self.assertRaises(AssertionError):self.analyze(broken,fixed)

    def test_stale_wrong_missing_or_duplicate_phase_echo(self):
        value=dict(message='exact',name='Rcon',type={'chatType':4},target=None)
        decoder=FakeDecoder({});good=[event('profileless_chat',2.,value)]
        self.assertEqual(cal.phase_echo(good,decoder,'exact',1.)['index'],0)
        for events in [[],[event('profileless_chat',.5,value)],
                       [event('profileless_chat',2.,dict(value,message='wrong'))],
                       [event('profileless_chat',2.,dict(value,name='SomePlayer'))],
                       [event('profileless_chat',2.,dict(value,type={'chatType':0}))],good*2]:
            with self.assertRaises(AssertionError):cal.phase_echo(events,decoder,'exact',1.)
        raw,fixed=fixture();token=raw['queries'][0]['before_token']
        raw['clients'][1]['events']=[e for e in raw['clients'][1]['events'] if not (e['name']=='profileless_chat' and token.encode().hex() in e['hex'])]
        with self.assertRaises(AssertionError):self.analyze(raw,fixed)

    def test_emission_gap_uses_ages_not_arrival_intervals(self):
        a=dict(index=1,at=100,age_bracket={'after':100},offsets=[2])
        b=dict(index=2,at=100.001,age_bracket={'before':140},offsets=[2])
        self.assertTrue(cal.emission_gap_observation(a,b)['excludes_elapsed_emission_gap'])
        b.update(at=2000,age_bracket={'before':101})
        self.assertFalse(cal.emission_gap_observation(a,b)['excludes_elapsed_emission_gap'])
        b['age_bracket']['before']=None
        self.assertFalse(cal.emission_gap_observation(a,b)['excludes_elapsed_emission_gap'])

    def test_missing_ages_cannot_be_replaced_by_local_clock(self):
        raw,fixed=fixture();raw['clients'][1]['events']=[e for e in raw['clients'][1]['events'] if e['name']!='update_time']
        with self.assertRaises(AssertionError):self.analyze(raw,fixed)

    def test_nonzero_offset_preserved_not_current_pose_eligibility(self):
        events=[event('spawn_entity',0,dict(entityId=1,x=100.5,y=64,z=100.5)),
                event('rel_entity_move',1,dict(entityId=1,move={'onGround':True,'steps':[{'delta':[512,0,0],'tickOffset':2}]}))]
        rows=cal.tracker_steps(events,FakeDecoder({}),1)
        self.assertEqual(rows[-1]['positions'],[[100.625,64,100.5]])
        self.assertEqual(rows[-1]['offsets'],[2])
        with self.assertRaisesRegex(AssertionError,'without absolute base'):cal.tracker_steps(events[1:],FakeDecoder({}),1)

    def test_no_local_execution_or_automatic_retry(self):
        with patch.dict(cal.os.environ,{},clear=True):
            with self.assertRaisesRegex(AssertionError,'GitHub-only'):cal.run(None)
        with patch.dict(cal.os.environ,{'GITHUB_ACTIONS':'true','GITHUB_RUN_ATTEMPT':'2'},clear=True):
            with self.assertRaisesRegex(AssertionError,'retry'):cal.run(None)


if __name__=='__main__':unittest.main()
