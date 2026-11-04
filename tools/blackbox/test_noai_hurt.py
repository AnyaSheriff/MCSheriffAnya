"""Малые offline-проверки нового измерения; без Minecraft/Cargo/сети."""
import copy
import json
from pathlib import Path
import unittest
import tempfile
from unittest.mock import patch
from noai_hurt_audit import audit, audit_capture, compatible_sound_position, expected_actions, SCENARIOS, SPAWN, ATTACK, END, RECORD, SOUND
from noai_hurt_ci import verdict
from record777 import validate


class FixtureDecoder:
    def read(self,stream,kind):
        return json.loads(stream.read())


def event(tick,name,value):
    raw=(10000+tick).to_bytes(8,'big',signed=True) if name=='update_time' else json.dumps(value).encode()
    return {'at':100+tick*.05,'name':name,'hex':raw.hex()}


def fixture():
    events=[]
    for entity,x in [(51,3.5),(52,7.5)]:
        events.append(event(SPAWN,'spawn_entity',{'entityId':entity,'type':30,'x':100+x,'y':-59,'z':101.5}))
        events.append(event(SPAWN,'entity_metadata',{'entityId':entity,'metadata':[{'key':15,'value':1},{'key':9,'value':10.}]}))
    events.extend([event(ATTACK,'damage_event',{'entityId':51,'sourceTypeId':34,'sourceCauseId':2,'sourceDirectId':2}),
                   event(ATTACK,'sound_effect',{'sound':{'data':{'soundName':'minecraft:entity.cow.hurt','fixedRange':None}},'x':828,'y':-472,'z':812,'soundCategory':'neutral','volume':.4,'pitch':1.}),
                   event(ATTACK,'entity_metadata',{'entityId':51,'metadata':[{'key':9,'value':9.}]}),
                   event(ATTACK,'entity_velocity',{'entityId':51,'velocity':{'x':0,'y':.4,'z':-.4}})])
    for tick,x in [(150,3.5),(160,7.5),(290,3.5),(310,7.5)]:
        events.append(event(tick,'sound_effect',{'sound':{'data':{'soundName':SOUND,'fixedRange':None}},'x':int((100+x)*8),'y':-472,'z':812,'soundCategory':'neutral','volume':.4,'pitch':1.}))
    events.extend([event(420,'update_time',{}),event(END,'entity_destroy',{'entityIds':[51,52]})])
    return {'protocol':777,'start':100,'origin':[100,-59,100],'initial':[{'entity':1}],
            'sent':[{'at':100+row['after']*.05,'action':row} for row in expected_actions()],
            'clients':[{'failure':None,'events':sorted(events,key=lambda row:row['at'])}]}


class HurtTests(unittest.TestCase):
    def report(self,raw):
        return audit_capture(raw,FixtureDecoder(),30,{})

    def test_valid_measurement_retains_hurt_control_raw_and_does_not_claim_parity(self):
        report=self.report(fixture());self.assertEqual(report['errors'],[])
        self.assertEqual(report['cows']['hurt']['first_voice_after_actual_hurt_nominal_ticks'],40)
        self.assertEqual(report['cows']['control']['first_voice_after_actual_hurt_nominal_ticks'],50)
        self.assertEqual(len(report['cows']['hurt']['motion']),1)
        self.assertEqual(report['cows']['hurt']['intervoice_gaps_nominal_ticks'],[140])
        self.assertEqual(report['actual_hurt_nominal_tick'],ATTACK)

    def test_hurt_cow_movement_and_changed_sound_position_are_allowed(self):
        raw=fixture()
        raw['clients'][0]['events'].append(event(120,'sync_entity_position',{'entityId':51,'x':103.5,'y':-59.,'z':97.5,'yaw':0,'pitch':0}))
        for entry in raw['clients'][0]['events']:
            if entry['name']=='sound_effect' and entry['at']>100+ATTACK*.05:
                value=json.loads(bytes.fromhex(entry['hex']))
                if value['x']==828:
                    value['z']=780;entry['hex']=json.dumps(value).encode().hex()
        raw['clients'][0]['events'].sort(key=lambda row:row['at'])
        self.assertEqual(self.report(raw)['errors'],[])

    def test_unobserved_or_stale_hurt_sound_position_is_rejected(self):
        raw=fixture()
        for row in raw['clients'][0]['events']:
            if row['name']=='sound_effect' and row['at']==100+150*.05:
                value=json.loads(bytes.fromhex(row['hex']));value['x']=800000
                row['hex']=json.dumps(value).encode().hex()
        self.assertIn('Cow sound spatial attribution unknown',self.report(raw)['errors'])
        raw=fixture()
        raw['clients'][0]['events'].append(event(120,'sync_entity_position',{'entityId':51,'x':103.5,'y':-59.,'z':97.5,'yaw':0,'pitch':0}))
        raw['clients'][0]['events'].sort(key=lambda row:row['at'])
        self.assertIn('Cow sound spatial attribution unknown',self.report(raw)['errors'])

    def test_next_tracking_sample_only_within_three_ticks_and_ambiguity_rejected(self):
        for delay,valid in [(2,True),(4,False)]:
            raw=fixture()
            raw['clients'][0]['events'].append(event(150+delay,'sync_entity_position',{'entityId':51,'x':103.5,'y':-59.,'z':97.5,'yaw':0,'pitch':0}))
            for row in raw['clients'][0]['events']:
                if row['name']=='sound_effect' and row['at']>100+ATTACK*.05:
                    value=json.loads(bytes.fromhex(row['hex']))
                    if value['x']==828:
                        value['z']=780;row['hex']=json.dumps(value).encode().hex()
            raw['clients'][0]['events'].sort(key=lambda row:row['at'])
            report=self.report(raw)
            if valid:self.assertEqual(report['errors'],[])
            else:self.assertIn('Cow sound spatial attribution unknown',report['errors'])
        raw=fixture()
        raw['clients'][0]['events'].append(event(155,'sync_entity_position',{'entityId':51,'x':107.5,'y':-59.,'z':101.5,'yaw':0,'pitch':0}))
        raw['clients'][0]['events'].sort(key=lambda row:row['at'])
        self.assertIn('Cow sound spatial attribution ambiguous',self.report(raw)['errors'])

    def test_control_spawn_velocity_and_yaw_are_not_ignored(self):
        for field,value in [('yaw',1),('velocity',{'x':.01,'y':0,'z':0})]:
            raw=fixture()
            for row in raw['clients'][0]['events']:
                if row['name']=='spawn_entity':
                    decoded=json.loads(bytes.fromhex(row['hex']))
                    if decoded['entityId']==52:
                        decoded[field]=value;row['hex']=json.dumps(decoded).encode().hex()
            self.assertIn('control initial spawn velocity/rotation',self.report(raw)['errors'])

    def test_sound_quantization_accepts_only_adjacent_floor_or_trunc_bins(self):
        sample={'position':[-.02,0,0],'uncertainty':0}
        self.assertTrue(compatible_sound_position([-1,0,0],sample,[0,0,0]))
        self.assertTrue(compatible_sound_position([0,0,0],sample,[0,0,0]))
        self.assertFalse(compatible_sound_position([1,0,0],sample,[0,0,0]))
        sample={'position':[.125,0,0],'uncertainty':1/4096}
        self.assertTrue(compatible_sound_position([0,0,0],sample,[0,0,0]))
        self.assertTrue(compatible_sound_position([1,0,0],sample,[0,0,0]))
        self.assertFalse(compatible_sound_position([2,0,0],sample,[0,0,0]))

    def test_control_motion_damage_missing_hurt_and_early_removal_fail(self):
        for name,value,error in [
            ('entity_velocity',{'entityId':52,'velocity':{'x':.1,'y':0,'z':0}},'control velocity'),
            ('damage_event',{'entityId':52},'control accepted damage/hurt sound count'),
            ('entity_destroy',{'entityIds':[51]},'Cow removed before fixed pair ends')]:
            raw=fixture();raw['clients'][0]['events'].append(event(200,name,value))
            raw['clients'][0]['events'].sort(key=lambda row:row['at'])
            self.assertIn(error,self.report(raw)['errors'])
        raw=fixture();raw['clients'][0]['events']=[row for row in raw['clients'][0]['events'] if row['name']!='damage_event']
        self.assertIn('hurt accepted damage/hurt sound count',self.report(raw)['errors'])

    def test_absence_of_posthurt_voice_is_valid_censored_measurement(self):
        raw=fixture();raw['clients'][0]['events']=[row for row in raw['clients'][0]['events'] if row['name']!='sound_effect' or json.loads(bytes.fromhex(row['hex']))['sound']['data']['soundName']!=SOUND]
        report=self.report(raw);self.assertEqual(report['errors'],[])
        self.assertIsNone(report['cows']['hurt']['first_voice_after_actual_hurt_nominal_ticks'])
        self.assertEqual(report['cows']['hurt']['post_hurt_window_nominal_ticks'],300)

    def test_lost_noai_silent_and_health_death_fail(self):
        for key,value,error in [(15,0,'hurt NoAI disabled'),(4,True,'hurt unexpectedly Silent'),(9,0,'hurt Cow not alive')]:
            raw=fixture();raw['clients'][0]['events'].append(event(200,'entity_metadata',{'entityId':51,'metadata':[{'key':key,'value':value}]}))
            self.assertIn(error,self.report(raw)['errors'])

    def test_fixed_eight_scenarios_are_400_tick_pairs_with_one_first_target_fist_attack(self):
        root=Path(__file__).parent
        self.assertEqual(len(SCENARIOS),8)
        self.assertEqual(END-SPAWN,400);self.assertEqual(ATTACK-SPAWN,100)
        for name in SCENARIOS:
            scenario=json.loads((root/'scenarios'/f'{name}.json').read_text());validate(scenario)
            self.assertEqual(scenario['name'],name)
            self.assertEqual(scenario['actions'],expected_actions())
            self.assertEqual(scenario['record_ticks'],RECORD)
            self.assertIn('clear $(bot)',scenario['setup'])
            self.assertFalse(any('give ' in row for row in scenario['setup']))
            self.assertEqual([a for a in scenario['actions'] if a.get('attack')],[{'after':ATTACK,'attack':True,'which':0}])

    def test_whole_eight_pair_series_reports_validity_not_parity(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder=Path(temporary);data=folder/'data';data.mkdir()
            (data/'protocol.json').write_text('{}')
            (data/'entities.json').write_text(json.dumps([{'id':30,'name':'cow'}]))
            (folder/'sound-observations.json').write_text(json.dumps({'sounds':{}}))
            for name in SCENARIOS:
                for side in ('official','ours'):
                    raw=fixture();raw['scenario']=name
                    if side=='ours':
                        raw['clients'][0]['events']=[row for row in raw['clients'][0]['events'] if row['name']!='sound_effect' or json.loads(bytes.fromhex(row['hex']))['sound']['data']['soundName']!=SOUND]
                    (folder/f'{name}-{side}.json').write_text(json.dumps(raw))
            with patch('noai_hurt_audit.Decoder',return_value=FixtureDecoder()):
                report=audit(folder,data)
            self.assertEqual(report['status'],'observations-valid')
            self.assertEqual(report['errors'],[])
            self.assertEqual(len(report['captures']),16)
            self.assertIn('no parity',report['claim'])

    def test_wrong_damage_source_or_no_actual_health_loss_is_invalid(self):
        for source in [0,35]:
            raw=fixture()
            for row in raw['clients'][0]['events']:
                if row['name']=='damage_event':
                    row['hex']=json.dumps({'entityId':51,'sourceTypeId':source,'sourceCauseId':2,'sourceDirectId':2}).encode().hex()
            self.assertIn('accepted damage is not the observer fist attack',self.report(raw)['errors'])
        raw=fixture()
        for row in raw['clients'][0]['events']:
            if row['name']=='entity_metadata' and row['at']==100+ATTACK*.05:
                row['hex']=json.dumps({'entityId':51,'metadata':[{'key':9,'value':10.}]}).encode().hex()
        self.assertIn('hurt Cow health did not decrease',self.report(raw)['errors'])

    def test_strict_random_mismatch_is_diagnostic_but_transport_decode_errors_block(self):
        strict={'status':'mismatch-open','errors':[],'comparisons':[{'warnings':[],'decode_errors':[[],[]]}]}
        self.assertEqual(verdict(strict,{'status':'observations-valid'},0,0),'observations-valid')
        for key,value in [('warnings',['unverified']),('decode_errors',[[{'error':'bad'}],[]])]:
            bad=copy.deepcopy(strict);bad['comparisons'][0][key]=value
            self.assertEqual(verdict(bad,{'status':'observations-valid'},0,0),'infrastructure-failed')
        self.assertEqual(verdict(strict,{'status':'observations-valid'},1,0),'infrastructure-failed')
        self.assertEqual(verdict(strict,{'status':'observation-invalid'},0,0),'observation-invalid')


if __name__=='__main__':
    unittest.main()
