"""Отрицательные проверки ограниченного NoAI аудита; без игры, Cargo и сети."""
import copy
import io
import json
from pathlib import Path
import unittest
import tempfile
from unittest.mock import patch
from noai_ambient_audit import audit_capture, audit, sound_name, WINDOWS, SCENARIOS, SOUND
from analyze777 import Decoder
from noai_ambient_ci import verdict


class FixtureDecoder:
    def read(self, stream, kind):
        return json.loads(stream.read())


def event(tick, name, value):
    return {'at':100+tick*.05,'name':name,'hex':json.dumps(value).encode().hex()}


def fixture(silent=False, voices=True):
    events=[];sent=[]
    for index,(a,b) in enumerate(WINDOWS):
        entity=1000+index
        sent.extend([{'at':100+a*.05,'action':{'after':a,'cmd':'summon cow $(4.5,0,3.5) {NoAI:1b,Age:0'+(',Silent:1b' if silent else '')+'}'}},
                     {'at':100+b*.05,'action':{'after':b,'cmd':'kill @e[type=cow,distance=..48]'}}])
        events.append(event(a,'spawn_entity',{'entityId':entity,'type':42,'x':104.5,'y':-59,'z':103.5,'velocity':{'x':0,'y':0,'z':0},'yaw':0,'pitch':0,'headPitch':0}))
        events.append(event(a,'entity_metadata',{'entityId':entity,'metadata':[{'key':15,'value':1}]+([{'key':4,'value':True}] if silent else [])}))
        events.append(event(a+60,'rel_entity_move',{'entityId':entity,'move':{'steps':[{'delta':[0,0,0]}]}}))
        if voices:
            events.append(event(a+80,'sound_effect',{'sound':{'soundId':450},'soundCategory':'neutral','x':836,'y':-472,'z':828,'volume':.4,'pitch':1.1,'seed':123}))
    events.append(event(1890,'update_time',{}))
    return {'scenario':SCENARIOS[int(silent)],'protocol':777,'origin':[100,-59,100],'start':100,
            'clients':[{'events':events,'failure':None}], 'sent':sent}


class AuditTests(unittest.TestCase):
    def run_capture(self, raw, silent=False):
        return audit_capture(raw,FixtureDecoder(),42,{'450':SOUND},silent)

    def mutate(self, name, callback):
        raw=fixture()
        e=next(e for e in raw['clients'][0]['events'] if e['name']==name)
        value=json.loads(bytes.fromhex(e['hex']));callback(value)
        e['hex']=json.dumps(value).encode().hex()
        return raw

    def test_real_decoder_inline_data_holder_and_registry_id_are_distinct(self):
        # Схема ItemSoundHolder/ItemSoundEvent из закреплённого JSON777;
        # байты inline-заголовка взяты из фактического пакета candidate.
        protocol={'types':{
            'ItemSoundHolder':['registryEntryHolder',{'baseName':'soundId','otherwise':{'name':'data','type':'ItemSoundEvent'}}],
            'ItemSoundEvent':['container',[{'name':'soundName','type':'string'},{'name':'fixedRange','type':['option','f32']}]],
        },'play':{'toClient':{'types':{}}}}
        decoder=Decoder(protocol)
        stream=io.BytesIO(bytes.fromhex('001c6d696e6563726166743a656e746974792e636f772e616d6269656e7400'))
        literal=decoder.read(stream,'ItemSoundHolder')
        self.assertEqual(stream.read(),b'')
        self.assertEqual(literal,{'data':{'soundName':SOUND,'fixedRange':None}})
        self.assertEqual(sound_name(literal,{'0':'minecraft:ambient.cave'}),SOUND)
        indexed=decoder.read(io.BytesIO(b'\xc3\x03'),'ItemSoundHolder')
        self.assertEqual(indexed,{'soundId':450})
        self.assertEqual(sound_name(indexed,{'450':SOUND}),SOUND)
        self.assertIsNone(sound_name(indexed,{}))
        for malformed in [None,{}, {'soundId':True},{'soundId':-1},{'soundId':'450'},
                          {'data':None},{'data':{'soundName':42}},{'data':{'soundName':''}},
                          {'sound':{'soundName':SOUND}},{'soundId':450,'data':{'soundName':SOUND}}]:
            with self.subTest(holder=malformed),self.assertRaises(ValueError):
                sound_name(malformed,{'450':SOUND})

    def test_inline_ambient_keeps_all_capture_guards_and_unknown_id_is_not_ambient(self):
        raw=fixture()
        for entry in raw['clients'][0]['events']:
            if entry['name']=='sound_effect':
                value=json.loads(bytes.fromhex(entry['hex']))
                value['sound']={'data':{'soundName':SOUND,'fixedRange':None}}
                entry['hex']=json.dumps(value).encode().hex()
        report=self.run_capture(raw)
        self.assertEqual(report['errors'],[])
        self.assertEqual([w['sound_count'] for w in report['windows']],[1,1,1])
        first=next(entry for entry in raw['clients'][0]['events'] if entry['name']=='sound_effect')
        value=json.loads(bytes.fromhex(first['hex']));value['volume']=.5
        first['hex']=json.dumps(value).encode().hex()
        self.assertIn('ambient volume/pitch',self.run_capture(raw)['errors'])
        unknown=self.mutate('sound_effect',lambda value:value.update(sound={'soundId':999999}))
        self.assertEqual([w['sound_count'] for w in self.run_capture(unknown)['windows']],[0,1,1])
        malformed=self.mutate('sound_effect',lambda value:value.update(sound={'data':{'soundName':None}}))
        with self.assertRaises(ValueError):
            self.run_capture(malformed)

    def test_fixed_three_windows_keep_every_sound_timestamp(self):
        report=self.run_capture(fixture())
        self.assertEqual(report['errors'],[])
        self.assertEqual([w['sound_count'] for w in report['windows']],[1,1,1])
        self.assertEqual(report['windows'][0]['sounds'][0]['at'],105.5)
        self.assertEqual(report['windows'][0]['sounds'][0]['value']['seed'],123)

    def test_silent_control_requires_metadata_and_zero_sound(self):
        self.assertEqual(self.run_capture(fixture(True,False),True)['errors'],[])
        self.assertIn('Silent Cow emitted ambient',self.run_capture(fixture(True,True),True)['errors'])
        raw=fixture(True,False)
        for e in raw['clients'][0]['events']:
            if e['name']=='entity_metadata':
                v=json.loads(bytes.fromhex(e['hex']));v['metadata']=v['metadata'][:1];e['hex']=json.dumps(v).encode().hex()
        self.assertIn('Silent metadata missing',self.run_capture(raw,True)['errors'])

    def test_movement_outside_pad_cannot_disappear(self):
        raw=self.mutate('rel_entity_move',lambda v:v['move']['steps'][0].update(delta=[32767,0,0]))
        self.assertIn('cow relative movement',self.run_capture(raw)['errors'])
        raw=fixture();raw['clients'][0]['events'].append(event(100,'entity_teleport',{'entityId':1000,'x':9000,'y':-59,'z':103.5,'flags':{},'yaw':0,'pitch':0}))
        self.assertIn('cow absolute movement',self.run_capture(raw)['errors'])

    def test_nonzero_velocity_rotation_volume_pitch_and_position_fail(self):
        cases=[('sound_effect',lambda v:v.update(volume=.5),'ambient volume/pitch'),
               ('sound_effect',lambda v:v.update(pitch=float('nan')),'ambient volume/pitch'),
               ('sound_effect',lambda v:v.update(x=9999),'ambient position'),
               ('spawn_entity',lambda v:v.update(yaw=1),'cow spawn rotation/velocity')]
        for name,change,error in cases:
            with self.subTest(error=error):
                self.assertIn(error,self.run_capture(self.mutate(name,change))['errors'])

    def test_missing_window_noai_and_incomplete_capture_fail(self):
        raw=fixture();raw['clients'][0]['events']=[e for e in raw['clients'][0]['events'] if e['name']!='spawn_entity']
        self.assertIn('each window requires exactly one Cow spawn',self.run_capture(raw)['errors'])
        raw=self.mutate('entity_metadata',lambda v:v.update(metadata=[]))
        self.assertIn('NoAI metadata missing',self.run_capture(raw)['errors'])
        raw=fixture();raw['clients'][0]['events'][-1]['at']=100+1869*.05
        self.assertIn('capture ends before final 600-tick window ends',self.run_capture(raw)['errors'])

    def test_changed_actions_or_attacks_fail(self):
        raw=fixture();raw['sent'][0]['action']['after']=31
        self.assertIn('sent actions differ from fixed bounded series',self.run_capture(raw)['errors'])
        raw=fixture();raw['clients'][0]['outbound']=[{'name':'attack'}]
        self.assertIn('unexpected outbound interaction/attack',self.run_capture(raw)['errors'])

    def test_random_mismatch_is_diagnostic_but_warning_decode_failure_block(self):
        strict={'status':'mismatch-open','errors':[],'comparisons':[{'warnings':[],'decode_errors':[[],[]]}]}
        scoped={'status':'scoped-pass'}
        self.assertEqual(verdict(strict,scoped,0,0),'scoped-pass')
        for field,value in [('warnings',['unverified']),('decode_errors',[[{'error':'bad'}],[]])]:
            bad=copy.deepcopy(strict);bad['comparisons'][0][field]=value
            self.assertEqual(verdict(bad,scoped,0,0),'infrastructure-failed')
        self.assertEqual(verdict(strict,scoped,1,0),'infrastructure-failed')
        self.assertEqual(verdict(strict,{'status':'inconclusive'},0,0),'inconclusive')

    def test_noai_clear_or_premature_removal_fails(self):
        raw=self.mutate('entity_metadata',lambda v:v['metadata'].append({'key':15,'value':0}))
        self.assertIn('NoAI metadata disabled',self.run_capture(raw)['errors'])
        raw=fixture();raw['clients'][0]['events'].append(event(200,'entity_destroy',{'entityIds':[1000]}))
        self.assertIn('Cow removed before observation window ends',self.run_capture(raw)['errors'])

    def test_late_spawn_stepped_motion_and_positive_silent_fail(self):
        raw=fixture()
        for e in raw['clients'][0]['events']:
            if e['name']=='spawn_entity':e['at']+=599*.05
        self.assertIn('each window requires exactly one Cow spawn',self.run_capture(raw)['errors'])
        raw=fixture();raw['clients'][0]['events'].append(event(100,'sync_entity_position',{'entityId':1000,'positionType':'stepped','steps':[{'x':104.5,'y':-59,'z':103.5,'yaw':90,'pitch':25,'dx':1}]}))
        self.assertIn('cow stepped rotation/velocity',self.run_capture(raw)['errors'])
        raw=self.mutate('entity_metadata',lambda v:v['metadata'].append({'key':4,'value':True}))
        self.assertIn('positive Cow unexpectedly Silent',self.run_capture(raw)['errors'])

    def test_voice_before_noai_or_spawn_fails(self):
        raw=fixture()
        for e in raw['clients'][0]['events']:
            if e['name']=='sound_effect':e['at']-=80*.05+.001
        self.assertIn('ambient before Cow spawn/NoAI metadata',self.run_capture(raw)['errors'])
        raw=fixture()
        for e in raw['clients'][0]['events']:
            if e['name']=='entity_metadata':e['at']+=100*.05
        self.assertIn('ambient before Cow spawn/NoAI metadata',self.run_capture(raw)['errors'])
        raw=fixture();raw['sent'][0]['at']+=.5
        self.assertIn('scheduled command delivery late/missing timestamp',self.run_capture(raw)['errors'])

    def test_whole_series_requires_positive_both_sides_and_original_zero_is_inconclusive(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder=Path(temporary);data=folder/'data';data.mkdir()
            (data/'protocol.json').write_text('{}')
            (data/'entities.json').write_text(json.dumps([{'id':42,'name':'cow'}]))
            (folder/'sound-observations.json').write_text(json.dumps({'sounds':{'450':SOUND}}))
            def write(official,ours):
                for name in SCENARIOS:
                    for side in ('official','ours'):
                        silent=name.endswith('-silent')
                        raw=fixture(silent,False if silent else official if side=='official' else ours)
                        (folder/f'{name}-{side}.json').write_text(json.dumps(raw))
            with patch('noai_ambient_audit.Decoder',return_value=FixtureDecoder()):
                for official,ours,status in [(True,True,'scoped-pass'),(True,False,'failed'),(False,True,'inconclusive'),(False,False,'inconclusive')]:
                    write(official,ours)
                    self.assertEqual(audit(folder,data)['status'],status)

    def test_wire_shaped_stepped_absolute_movement_and_top_rotation_fail(self):
        raw=fixture();raw['clients'][0]['events'].append(event(100,'sync_entity_position',{'entityId':1000,'positionType':'stepped','steps':[{'x':9000,'y':-59,'z':103.5,'tickOffset':3}],'yaw':0,'pitch':0}))
        self.assertIn('cow absolute movement',self.run_capture(raw)['errors'])
        raw=fixture();raw['clients'][0]['events'].append(event(100,'sync_entity_position',{'entityId':1000,'positionType':'stepped','steps':[{'x':104.5,'y':-59,'z':103.5,'tickOffset':3}],'yaw':90,'pitch':0}))
        self.assertIn('cow rotation',self.run_capture(raw)['errors'])

    def test_sound_after_cow_removal_cannot_be_attributed(self):
        raw=fixture()
        events=raw['clients'][0]['events'];events.insert(3,event(100,'entity_destroy',{'entityIds':[1000]}))
        self.assertIn('ambient before Cow spawn/NoAI metadata',self.run_capture(raw)['errors'])

    def test_checked_scenarios_are_fixed_no_attack_series(self):
        root=Path(__file__).parent
        for silent,name in enumerate(SCENARIOS):
            scenario=json.loads((root/'scenarios'/f'{name}.json').read_text())
            self.assertEqual(scenario['record_ticks'],1890)
            self.assertEqual(scenario['actions'],[e['action'] for e in fixture(bool(silent))['sent']])
            self.assertFalse(any(a.get('attack') or a.get('interact') for a in scenario['actions']))


if __name__=='__main__':
    unittest.main()
