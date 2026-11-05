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
        spawn=varint(100+index)+bytes(16)+varint(kind)+struct.pack('>ddd',.5,66.5,-5.5)+b'\0'+bytes(3)+b'\3'
        shooter.append(packet('spawn_entity',spawn,time=stamp+1.21))
        victim.append(packet('tick_end',direction='out',time=stamp))
        if '-self-' in case:
            for height,offset in [(130,1.25),(107,1.3)]:
                payload=varint(100+index)+b'\0'+struct.pack('>dddffB',.5,height,-5.5,0,0,0)
                shooter.append(packet('sync_entity_position',payload,time=stamp+offset))
            shooter.append(packet('teleport_confirm',b'\1'+struct.pack('>dddff',.5,65,-5.5,0,-90),'out',stamp+1.4))
            row['self_aim']={'projectile':100+index,'observed_position':[.5,107.,-5.5],'player_position':[.5,65,-5.5]}
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
    def test_self_aim_uses_early_sample_from_failed_native_trajectory(self):
        # Свежий failed official41989796183, только авторские координаты:
        # tracker раз в1с. Поздний y72 нельзя выбрать вместо раннего y107.
        positions=[[.5,66.52000000327826,-5.5],
                   [.30107313463590846,111.81257956445874,-4.927018978045748],
                   [.13836944003341664,130.64839389828137,-4.458373734787939],
                   [.005312799408416641,127.84492710140637,-4.075072953537939],
                   [-.10355128483755745,107.3428349962047,-3.7615549118764235],
                   [-.192575743535565,72.36463455795393,-3.5051324058949067],
                   [-.156198790410565,65.04993729232893,-3.5156304527699067]]
        aim=probe.self_aim_position(positions)
        self.assertEqual(aim,positions[4])
        self.assertLess(abs(positions[-1][0]-aim[0]),.3)
        self.assertLess(abs(positions[-1][2]-aim[2]),.3)
        self.assertIsNone(probe.self_aim_position(positions[:4]))
        self.assertIsNone(probe.self_aim_position(positions[5:]))

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


if __name__=='__main__': unittest.main()
