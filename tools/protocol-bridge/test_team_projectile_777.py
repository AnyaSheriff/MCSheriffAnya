#!/usr/bin/env python3
"""Отказ от неполной/подменённой записи и пустых положительных контролей."""
import copy
import io
import json
from pathlib import Path
import struct
import unittest

import probe_team_projectile_777 as probe
from smoke import varint


def packet(name,payload=b'',direction='in',time=0):
    return {'name':name,'payload':payload.hex(),'direction':direction,'time':time}


def text(value):
    value=value.encode()
    return varint(len(value))+value


def team(name,mode,flags=None,members=()):
    payload=text(name)+bytes([mode])
    if mode in (0,2): payload+=b'\x08\0\x01t'+b'\x08\0\0'*2+b'\0\0\0'+bytes([flags])
    if mode in (0,3,4): payload+=varint(len(members))+b''.join(text(m) for m in members)
    return packet('teams',payload)


def health(value,time=0):
    return packet('update_health',struct.pack('>f',value)+varint(20)+struct.pack('>f',5),time=time)


def fixture(target='official',self_denied_hit=True):
    report={'schema':probe.SCHEMA,'criteria':copy.deepcopy(probe.CRITERIA),'target':target,'protocol':777,
            'team_layout_sha256':probe.TEAM_LAYOUT_SHA256,
            'target_sha256':'a'*64,'jar_sha1':probe.JAR_SHA1 if target=='official' else None,
            'server_exit':0,'server_log_sha256':'b'*64,'entity_kinds':{'arrow':1,'trident':2},
            'status':'PASS','cases':[],'sessions':{},'behavior_failures':[]}
    for role,name,entity,uuid in [('shooter',probe.NAMES[0],3,'61'*16),('victim',probe.NAMES[1],4,'62'*16)]:
        report['sessions'][role]={'name':name,'uuid':uuid,'packets':[
            packet('login',struct.pack('>i',entity)),health(20),
            team('mcsa_a',0,0),team('mcsa_b',0,0)]}
    for index,case in enumerate(probe.CASES):
        mode=case.split('-',1)[1]
        for session in report['sessions'].values():
            session['packets'] += [health(20),team('mcsa_a',2,int(mode.endswith('allowed'))),
                                   team('mcsa_a',3,members=[probe.NAMES[0]]),
                                   team('mcsa_b' if mode=='different-denied' else 'mcsa_a',3,members=[probe.NAMES[1]])]
        row={'case':case,'ranges':{role:[len(session['packets']),None] for role,session in report['sessions'].items()}}
        shooter=report['sessions']['shooter']['packets']
        victim=report['sessions']['victim']['packets']
        stamp=index*100+1
        shooter += [packet('use_item',b'\0\1'+struct.pack('>ff',0,-90 if '-self-' in case else 0),'out',stamp),
                    packet('block_dig',bytes([probe.RELEASE_USE])+b'\0'*9+b'\2','out',stamp+1.2)]
        kind=1 if case.startswith('bow-') else 2
        velocity=bytes.fromhex('4bf97f8bfddd') if '-self-' in case else b'\0'
        spawn=varint(100+index)+bytes(16)+varint(kind)+struct.pack('>ddd',.5,66.5,-5.5)+velocity+bytes(3)+b'\3'
        shooter.append(packet('spawn_entity',spawn,time=stamp+1.21))
        victim.append(packet('tick_end',direction='out',time=stamp))
        if '-self-' in case:
            for height,offset in [(130,1.25),(107,1.3)]:
                payload=varint(100+index)+b'\0'+struct.pack('>dddffB',.5,height,-5.5,0,0,0)
                shooter.append(packet('sync_entity_position',payload,time=stamp+offset))
            forecast=probe.predict_self_aim([.5,66.5,-5.5],probe.lpvec(io.BytesIO(velocity)))
            x,_,z=forecast['crossing']
            shooter.append(packet('teleport_confirm',b'\1'+struct.pack('>dddff',x,65,z,0,-90),'out',stamp+1.4))
            row['self_aim']={'projectile':100+index,'forecast':forecast,'player_position':[x,65,z]}
        hit=not case.endswith('same-denied') and (not case.endswith('self-denied') or self_denied_hit)
        if hit:
            recipient=shooter if '-self-' in case else victim
            entity=3 if '-self-' in case else 4
            recipient += [health(14,stamp+1.5),packet('damage_event',varint(entity)+b'\1\4'+varint(101+index)+b'\0',time=stamp+1.5)]
        for packets in (shooter,victim):
            packets.append(packet('tick_end',direction='out',time=stamp+12))
        for role,session in report['sessions'].items(): row['ranges'][role][1]=len(session['packets'])
        row['summary']=probe.summarize(report,row)
        report['cases'].append(row)
    return report


class TeamProjectileTests(unittest.TestCase):
    def test_survival_starts_after_safe_teleport_and_healing(self):
        setup=probe.world_setup_commands()
        floor=next(i for i,cmd in enumerate(setup) if cmd.startswith('fill '))
        for name in probe.NAMES:
            self.assertLess(setup.index('gamemode creative '+name),floor)
        for case in probe.CASES:
            commands=probe.case_setup_commands(case)
            for name in probe.NAMES:
                creative=commands.index('gamemode creative '+name)
                teleport=next(i for i,cmd in enumerate(commands) if cmd.startswith('tp '+name+' '))
                heal=commands.index('effect give '+name+' instant_health 1 8 true')
                survival=commands.index('gamemode survival '+name)
                self.assertLess(creative,teleport)
                self.assertLess(teleport,heal)
                self.assertLess(heal,survival)

    def test_native_pre_shot_damage_is_decoded_as_environmental(self):
        # Failed core41990613917: ID1, type22, no cause/direct; HP19.
        state=probe.State({})
        state.entity=1
        state.accept(packet('damage_event',bytes.fromhex('0116000000')))
        state.accept(packet('update_health',bytes.fromhex('419800001440a00000')))
        self.assertEqual(state.health,19)
        self.assertEqual(state.events[0],{'name':'damage_event','source':22,'cause':0,'direct':0})

    def test_self_forecast_corrects_known_raw_input_miss(self):
        # Core41996104828: actual lpVec3 spawn; старый TP под y107 дал
        # driftX>.3. Прогноз используется ТОЛЬКО для ввода, hit требует raw.
        velocity=probe.lpvec(io.BytesIO(bytes.fromhex('4bf97f8bfddd')))
        forecast=probe.predict_self_aim([.5,66.52000000000001,-5.5],velocity)
        self.assertEqual(forecast['tick'],103)
        self.assertEqual(forecast['crossing'][1],66)
        actual_landing=[-1.9852215107505236,65.05015343356142,-6.132422743192036]
        old_aim=[-1.6674705809556611,65,-6.083469752516296]
        self.assertGreater(abs(actual_landing[0]-old_aim[0]),.3)
        for axis in (0,2): self.assertLess(abs(actual_landing[axis]-forecast['crossing'][axis]),.06)

    def test_self_forecast_rejects_nonfinite_invalid_speed_and_budget(self):
        position=[.5,66.52,-5.5]
        for bad in ([float('nan'),3,0],[0,float('inf'),0],[0,-3,0],[0,0,0],[0,5,0],[.3,3,0],[0,3]):
            with self.subTest(bad=bad),self.assertRaises(AssertionError):
                probe.predict_self_aim(position,bad)
        for budget in (0,201,True):
            with self.assertRaisesRegex(AssertionError,'budget'):
                probe.predict_self_aim(position,[0,3,0],max_ticks=budget)
        with self.assertRaisesRegex(AssertionError,'exceeded'):
            probe.predict_self_aim(position,[0,3,0],max_ticks=2)
        with self.assertRaisesRegex(AssertionError,'floor'):
            probe.predict_self_aim([100,66.52,-5.5],[0,3,0])
        with self.assertRaisesRegex(AssertionError,'floor'):
            probe.predict_self_aim([.5,66.52,2],[0,3,0])

    def test_backstop_is_behind_target_and_within_relative_move_range(self):
        self.assertIn('fill -8 65 2 8 69 2 stone',probe.world_setup_commands())
        self.assertNotIn('fill -8 65 24 8 69 24 stone',probe.world_setup_commands())
        self.assertLess(2-(-5.5),8)
        self.assertGreater(2-(.5+.3),1)
        for case in probe.CASES[:6]:
            self.assertIn('tp McsaArcher 0.5 65 -5.5 0 0',probe.case_setup_commands(case))
            self.assertIn('tp McsaTarget 0.5 65 0.5 180 0',probe.case_setup_commands(case))

    def test_existing_own_native_team_layout_evidence(self):
        evidence=json.loads(probe.TEAM_LAYOUT_FILE.read_text())
        state=probe.State({})
        for row in evidence['cases']:
            state.accept(packet('teams',bytes.fromhex(row['modern'])))
        self.assertEqual(state.teams,{})

    def test_actual_41989305234_native_team_add(self):
        # Только искусственное имя команды; original raw сохранён в FAILED
        # official41989305234. Никаких пользовательских данных в fixture.
        payload=bytes.fromhex('066d6373615f61000800066d6373615f610800000800000000000300')
        state=probe.State({})
        state.accept(packet('teams',payload))
        self.assertEqual(state.teams,{'mcsa_a':{'flags':3,'members':[]}})
        for size in range(len(payload)):
            with self.subTest(size=size),self.assertRaises((AssertionError,EOFError,IndexError,ValueError)):
                probe.State({}).accept(packet('teams',payload[:size]))
        with self.assertRaisesRegex(AssertionError,'trailing'):
            probe.State({}).accept(packet('teams',payload+b'\0'))

    def test_release_enum_matches_own_actual_gui_observation(self):
        evidence=json.loads(Path(probe.__file__).with_name('testdata').joinpath('actual777-use-action-observations.json').read_text())
        self.assertEqual(evidence['protocol'],777)
        self.assertEqual(probe.RELEASE_USE,evidence['statuses']['release_use'])

    def test_complete_capture_and_comparison(self):
        official=fixture()
        core=fixture('core')
        probe.validate(official,'official')
        probe.validate(core,'core')
        self.assertEqual(probe.compare(official,core),[])

    def test_self_denied_is_observed_not_presumed(self):
        official=fixture(self_denied_hit=False)
        probe.validate(official,'official')
        actual=fixture('core')
        self.assertTrue(any('bow-self-denied' in row for row in probe.compare(official,actual)))

    def test_malformed_damage_is_rejected(self):
        state=probe.State({'arrow':1,'trident':2})
        state.entity=4
        for payload in (b'',b'\4\1',b'\4\1\1\1\2',b'\4\1\1\1\0\0'):
            with self.subTest(payload=payload),self.assertRaises((AssertionError,EOFError,IndexError,ValueError)):
                state.accept(packet('damage_event',payload))

    def test_truncated_team_is_rejected(self):
        value=team('mcsa_a',0,0,members=['x'])
        value['payload']=value['payload'][:-2]
        with self.assertRaises((AssertionError,EOFError,ValueError)):
            probe.State({}).accept(value)

    def test_empty_control_cannot_pass(self):
        report=fixture()
        row=report['cases'][1]
        packets=report['sessions']['victim']['packets']
        start,stop=row['ranges']['victim']
        for index in range(start,stop):
            if packets[index]['name'] in ('update_health','damage_event'):
                packets[index]=packet('tick_end',direction='out',time=packets[index]['time'])
        # Recompute honest summaries: even an internally consistent empty
        # control must fail semantic validation.
        for row in report['cases']: row['summary']=probe.summarize(report,row)
        report['behavior_failures']=probe.behavior_failures(report)
        report['status']='FAIL'
        self.assertTrue(any('positive control' in value for value in report['behavior_failures']))
        with self.assertRaises(AssertionError): probe.validate(report,'official')

    def test_missing_spawn_cannot_be_called_a_miss(self):
        report=fixture()
        row=report['cases'][0]
        start,stop=row['ranges']['shooter']
        packets=report['sessions']['shooter']['packets']
        for index in range(start,stop):
            if packets[index]['name']=='spawn_entity': packets[index]=packet('tick_end',direction='out',time=packets[index]['time'])
        with self.assertRaisesRegex(AssertionError,'spawn'): probe.summarize(report,row)

    def test_unrelated_damage_cannot_satisfy_control(self):
        report=fixture()
        row=report['cases'][1]
        start,stop=row['ranges']['victim']
        packets=report['sessions']['victim']['packets']
        for index in range(start,stop):
            if packets[index]['name']=='damage_event':
                packets[index]=packet('damage_event',b'\4\1\4'+varint(999)+b'\0',time=packets[index]['time'])
        with self.assertRaisesRegex(AssertionError,'unrelated damage'):
            probe.summarize(report,row)

    def test_forged_summary_rejected(self):
        report=fixture()
        report['cases'][1]['summary']['minimum_health']=20
        with self.assertRaisesRegex(AssertionError,'raw'): probe.validate(report,'official')

    def test_missing_duplicate_and_reordered_stages_rejected(self):
        original=fixture()
        variants=[original['cases'][:-1],original['cases']+[original['cases'][0]],list(reversed(original['cases']))]
        for rows in variants:
            report=copy.deepcopy(original);report['cases']=rows
            with self.assertRaises(AssertionError): probe.validate(report,'official')

    def test_missing_team_receipt_rejected(self):
        report=fixture()
        report['sessions']['victim']['packets'][2]=packet('tick_end',direction='out')
        with self.assertRaises(AssertionError): probe.validate(report,'official')

    def test_short_observation_rejected(self):
        report=fixture()
        row=report['cases'][0]
        for role in report['sessions']:
            start,stop=row['ranges'][role]
            for p in report['sessions'][role]['packets'][start:stop]: p['time']=1
        with self.assertRaisesRegex(AssertionError,'short'): probe.summarize(report,row)

    def test_truncated_nonzero_velocity_rejected(self):
        state=probe.State({})
        with self.assertRaises((ValueError,AssertionError)):
            state.accept(packet('entity_velocity',b'\4\1'))

    def test_self_aim_requires_raw_teleport_acknowledgement(self):
        report=fixture()
        row=report['cases'][-1]
        row['self_aim']['player_position'][0]=5
        with self.assertRaisesRegex(AssertionError,'self aim'): probe.summarize(report,row)

    def test_self_forecast_cannot_be_forged(self):
        report=fixture()
        row=report['cases'][-1]
        row['self_aim']['forecast']['crossing'][0]+=1
        with self.assertRaisesRegex(AssertionError,'forecast'): probe.summarize(report,row)

    def test_self_aim_rejects_second_or_late_teleport(self):
        for mode in ('second','late','after-hit'):
            report=fixture()
            row=report['cases'][-1]
            start,stop=row['ranges']['shooter']
            packets=report['sessions']['shooter']['packets']
            selected=packets[start:stop]
            teleport=next(p for p in selected if p['name']=='teleport_confirm')
            if mode=='second':
                # Вместо последнего tick_end: диапазон остаётся прежним.
                end_time=selected[-1]['time']
                selected[-1]=copy.deepcopy(teleport)
                selected[-1]['time']=end_time
            elif mode=='late':
                spawn=next(p for p in selected if p['name']=='spawn_entity')
                teleport['time']=spawn['time']+1.1
            else:
                for p in selected:
                    if p['name'] in ('damage_event','update_health'): p['time']=teleport['time']-.01
            selected.sort(key=lambda p:p['time'])
            packets[start:stop]=selected
            reason={'second':'exact self teleport','late':'not early','after-hit':'precede actual damage'}[mode]
            with self.subTest(mode=mode),self.assertRaisesRegex(AssertionError,reason): probe.summarize(report,row)

    def test_prediction_cannot_replace_a_real_positive_self_hit(self):
        report=fixture()
        row=report['cases'][-1]
        start,stop=row['ranges']['shooter']
        packets=report['sessions']['shooter']['packets']
        for i in range(start,stop):
            if packets[i]['name'] in ('damage_event','update_health'):
                packets[i]=packet('tick_end',direction='out',time=packets[i]['time'])
        row['summary']=probe.summarize(report,row)
        self.assertFalse(row['summary']['hit'])
        self.assertEqual(row['summary']['minimum_health'],20)
        self.assertIn('bow-self-allowed: positive control did not genuinely hit',probe.behavior_failures(report))

    def test_legacy_release_status_cannot_count_as_777_shot(self):
        report=fixture()
        row=report['cases'][0]
        start,stop=row['ranges']['shooter']
        for p in report['sessions']['shooter']['packets'][start:stop]:
            if p['name']=='block_dig': p['payload']='05'+p['payload'][2:]
        with self.assertRaisesRegex(AssertionError,'use/release'): probe.summarize(report,row)

    def test_777_relative_steps_and_truncation(self):
        state=probe.State({})
        state.positions[7]=[0.,65.,0.]
        state.accept(packet('rel_entity_move',b'\7\1'+struct.pack('>hhh',0,0,8192)))
        self.assertEqual(state.positions[7],[0.,65.,2.])
        # Два шага с относительными tick offsets, ground1.
        payload=b'\7\5\1'+struct.pack('>hhh',0,0,4096)+b'\2'+struct.pack('>hhh',0,-4096,4096)
        state.accept(packet('rel_entity_move',payload))
        self.assertEqual(state.positions[7],[0.,64.,4.])
        self.assertEqual(len(state.events[-1]['positions']),2)
        with self.assertRaises((AssertionError,EOFError)):
            state.accept(packet('rel_entity_move',payload[:-1]))




def long_fixture(target='official', *, coordinates=True, stops=True, grounded=True, victim_stop=True, packet_ground=False, followup=True, critical=False):
    original=fixture(target)
    report=copy.deepcopy(original)
    report.update(schema=probe.LONG_SCHEMA,criteria=copy.deepcopy(probe.LONG_CRITERIA))
    for session in report['sessions'].values(): session['packets']=[]
    previous={role:0 for role in report['sessions']}
    for index,row in enumerate(report['cases']):
        for role,session in report['sessions'].items():
            start,stop=original['cases'][index]['ranges'][role]
            packets=original['sessions'][role]['packets']
            session['packets'].extend(copy.deepcopy(packets[previous[role]:start]))
            row['ranges'][role]=[len(session['packets']),None]
            chunk=copy.deepcopy(packets[start:stop])
            if row['case'].endswith('same-denied'):
                entity=100+index;stamp=chunk[-1]['time']-1
                additions=[]
                if role=='victim':
                    shot_start,shot_stop=original['cases'][index]['ranges']['shooter']
                    additions.append(copy.deepcopy(next(p for p in original['sessions']['shooter']['packets'][shot_start:shot_stop] if p['name']=='spawn_entity')))
                if coordinates:
                    additions.append(packet('sync_entity_position',varint(entity)+b'\0'+struct.pack('>dddffB',.5,65.5,23.95,0,0,packet_ground),time=stamp))
                if stops and (role!='victim' or victim_stop):
                    additions.append(packet('entity_velocity',varint(entity)+b'\0',time=stamp+.1))
                if grounded:
                    additions.append(packet('entity_metadata',varint(entity)+(bytes([8,0,int(critical)]) if row['case'].startswith('bow-') else b'')+bytes([10,8,1,255]),time=stamp+.2))
                if followup and coordinates:
                    additions.append(packet('entity_move_look',varint(entity)+b'\0'+struct.pack('>hhhBB',0,0,0,0,0),time=stamp+.3))
                chunk[-1:-1]=additions
            session['packets'].extend(chunk)
            row['ranges'][role][1]=len(session['packets'])
            previous[role]=stop
        row['summary']=probe.summarize(report,row)
    report['behavior_failures']=probe.behavior_failures(report)
    return report


class LongRangeTrackingTests(unittest.TestCase):
    def test_far_input_is_separate_and_near_v3_keeps_original_wall(self):
        self.assertIn('fill -8 65 2 8 69 2 stone',probe.world_setup_commands())
        self.assertIn('fill -8 65 24 8 69 24 stone',probe.world_setup_commands(24))
        self.assertNotEqual(probe.SCHEMA,probe.LONG_SCHEMA)
        self.assertEqual(probe.CRITERIA['cases'],probe.LONG_CRITERIA['cases'])
        with self.assertRaisesRegex(AssertionError,'unregistered'):
            probe.world_setup_commands(12)

    def test_complete_far_capture_is_replayed_for_both_observers(self):
        official=long_fixture();core=long_fixture('core')
        probe.validate(official,'official');probe.validate(core,'core')
        self.assertEqual(probe.compare(official,core),[])
        for role in ('shooter','victim'):
            row=core['cases'][0]['summary']['tracking'][role]
            self.assertEqual(row['spawns'],1)
            self.assertTrue(row['zero_velocity'] and row['past_target'] and row['in_ground'])
            self.assertGreater(row['coordinate_updates'],0)

    def test_stop_cannot_invent_a_landing_position(self):
        core=long_fixture('core',coordinates=False)
        probe.validate(core,'core')
        for role in ('shooter','victim'):
            row=core['cases'][0]['summary']['tracking'][role]
            self.assertTrue(row['zero_velocity'])
            self.assertFalse(row['past_target'])
            self.assertEqual(row['coordinate_updates'],0)
            stops=[r for r in row['receipts'] if 'velocity' in r]
            self.assertEqual(stops[-1]['last_received_position'],[.5,66.5,-5.5])
        differences=probe.compare(long_fixture(),core)
        self.assertTrue(any('shooter tracking past_target' in x for x in differences))
        self.assertTrue(any('victim tracking past_target' in x for x in differences))

    def test_absent_ground_metadata_is_a_comparison_difference(self):
        core=long_fixture('core',grounded=False)
        self.assertTrue(any('tracking in_ground' in x for x in probe.compare(long_fixture(),core)))

    def test_missing_stop_control_in_either_observer_is_rejected(self):
        for kwargs in ({'stops':False},{'victim_stop':False}):
            with self.subTest(kwargs=kwargs),self.assertRaisesRegex(AssertionError,'spawn/stop control'):
                probe.validate(long_fixture('core',**kwargs),'core')

    def test_projectile_velocity_does_not_become_player_knockback(self):
        core=long_fixture('core')
        self.assertFalse(core['cases'][0]['summary']['velocity'])
        self.assertTrue(core['cases'][0]['summary']['tracking']['shooter']['zero_velocity'])

    def test_cross_profile_comparison_is_rejected(self):
        with self.assertRaisesRegex(AssertionError,'different input profiles'):
            probe.compare(fixture(),long_fixture('core'))

    def test_forged_tracking_summary_is_rejected(self):
        report=long_fixture('core')
        report['cases'][0]['summary']['tracking']['victim']['past_target']=False
        with self.assertRaisesRegex(AssertionError,'not derived from raw'):
            probe.validate(report,'core')

    def test_projectile_velocity_truncation_and_trailing_bytes_are_rejected(self):
        report=long_fixture('core');row=report['cases'][0]
        for bad in (b'',b'\0\0'):
            broken=copy.deepcopy(report)
            packets=broken['sessions']['shooter']['packets']
            start,stop=row['ranges']['shooter']
            p=next(p for p in packets[start:stop] if p['name']=='entity_velocity')
            p['payload']=(varint(100)+bad).hex()
            with self.subTest(bad=bad),self.assertRaises((AssertionError,EOFError,IndexError,ValueError)):
                probe.summarize(broken,broken['cases'][0])

    def test_ground_metadata_must_have_boolean_serializer(self):
        report=long_fixture('core');row=report['cases'][0]
        packets=report['sessions']['shooter']['packets']
        start,stop=row['ranges']['shooter']
        p=next(p for p in packets[start:stop] if p['name']=='entity_metadata')
        p['payload']=(varint(100)+bytes([10,0,1,255])).hex()
        with self.assertRaisesRegex(AssertionError,'must be boolean'):
            probe.summarize(report,row)


    def test_packet_ground_flag_is_separate_from_in_ground_metadata(self):
        row=long_fixture()['cases'][0]['summary']['tracking']['shooter']
        self.assertTrue(row['in_ground'])
        self.assertFalse(row['absolute_ground_true'])
        self.assertTrue(row['settled_followup'])

    def test_changed_absolute_ground_bit_is_a_comparison_difference(self):
        self.assertTrue(any('absolute_ground_true' in x for x in probe.compare(long_fixture(),long_fixture('core',packet_ground=True))))

    def test_missing_settled_followup_is_a_comparison_difference(self):
        self.assertTrue(any('settled_followup' in x for x in probe.compare(long_fixture(),long_fixture('core',followup=False))))

    def test_invalid_absolute_ground_boolean_is_rejected(self):
        report=long_fixture('core');row=report['cases'][0]
        packets=report['sessions']['shooter']['packets']
        start,stop=row['ranges']['shooter']
        p=next(p for p in packets[start:stop] if p['name']=='sync_entity_position')
        raw=bytes.fromhex(p['payload']);p['payload']=(raw[:-1]+b'\2').hex()
        with self.assertRaises(AssertionError):probe.summarize(report,row)


    def test_critical_reset_is_a_comparison_difference(self):
        self.assertTrue(any('last_critical' in x for x in probe.compare(long_fixture(),long_fixture('core',critical=True))))

    def test_critical_flag_metadata_requires_byte_serializer(self):
        report=long_fixture('core');row=report['cases'][0]
        packets=report['sessions']['shooter']['packets'];start,stop=row['ranges']['shooter']
        p=next(p for p in packets[start:stop] if p['name']=='entity_metadata')
        p['payload']=(varint(100)+bytes([8,8,0,10,8,1,255])).hex()
        with self.assertRaisesRegex(AssertionError,'flags must be byte'):
            probe.summarize(report,row)


if __name__=='__main__': unittest.main()
