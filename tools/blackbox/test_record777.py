"""Семантика проекции и кодирование действий без сторонних реализаций."""
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from analyze777 import Decoder, project_record
from record777 import aim_at, dig_status, make_recorder, protocol_rotation, validate, window_click_payload, pack_interact_location, velocity
from smoke import read_varint
from types import SimpleNamespace
from record777 import run_scenario_sides, aligned_start


class RecordingSequenceTests(unittest.TestCase):
    def args(self, folder, **changes):
        args=SimpleNamespace(data=folder,output=folder,version=777,isolate_vitals=True,
                             parallel=False,resume=False,core_only=False,skip_recorded_core=False)
        for key,value in changes.items():setattr(args,key,value)
        (folder/'entities.json').write_text('[{"id":30,"name":"cow"}]')
        scenario=folder/'tools/blackbox/scenarios/probe.json';scenario.parent.mkdir(parents=True);scenario.write_text('{"name":"probe"}')
        patched=patch('record777.ROOT',folder);patched.start();self.addCleanup(patched.stop)
        import hashlib
        args.manifests=[{'protocol':777,'isolate_vitals':True,'recorder_sha256':'a'*64,
                         'scenario_sha256':{'probe':hashlib.sha256(scenario.read_bytes()).hexdigest()},
                         'data_sha256':{'entities.json':hashlib.sha256((folder/'entities.json').read_bytes()).hexdigest()}}]
        return args

    def raw(self, **changes):
        raw=dict(scenario='probe',protocol=777,isolate_vitals=True,start=10,clock=0,recorder_sha256='a'*64,
                 initial=[{'entity':1}],clients=[{'events':[{'name':'position'}],'failure':None}])
        raw.update(changes)
        return raw

    def test_real_sequencing_passes_only_successful_fresh_official_and_its_file_hash(self):
        import hashlib
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory);args=self.args(folder);calls=[]
            # Наличие старого файла не делает его опорой свежей последовательности.
            (folder/'probe-official.json').write_text(json.dumps(self.raw(start=-100)))
            def capture(*call,**kwargs):
                calls.append((call[0],call[7],kwargs['reference_sha256']))
                raw=self.raw();call[4].write_text(json.dumps(raw));return raw
            with patch('record777.record',side_effect=capture):
                run_scenario_sides(args,{'name':'probe'},[('official',None,1),('ours',None,2)],{},None)
            self.assertIsNone(calls[0][1]);self.assertEqual(calls[1][1]['start'],10)
            self.assertEqual(calls[1][2],hashlib.sha256((folder/'probe-official.json').read_bytes()).hexdigest())

    def test_failed_official_with_partial_file_never_starts_ours(self):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory);args=self.args(folder);calls=[]
            (folder/'probe-official.json').write_text(json.dumps(self.raw(start=-100)))
            def fail(*call,**kwargs):
                calls.append(call[0]);call[4].write_text(json.dumps(self.raw()))
                raise RuntimeError('late official cleanup failure')
            with patch('record777.record',side_effect=fail):
                with self.assertRaisesRegex(RuntimeError,'late official cleanup'):
                    run_scenario_sides(args,{'name':'probe'},[('official',None,1),('ours',None,2)],{},None)
            self.assertEqual(calls,[1])

    def test_parallel_both_sides_never_read_concurrent_reference(self):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory);args=self.args(folder,parallel=True)
            (folder/'probe-official.json').write_text('incomplete old JSON')
            def capture(*call,**kwargs):
                self.assertIsNone(call[7]);self.assertIsNone(kwargs['reference_sha256'])
                raw=self.raw();call[4].write_text(json.dumps(raw));return raw
            with patch('record777.record',side_effect=capture) as mocked:
                run_scenario_sides(args,{'name':'probe'},[('official',None,1),('ours',None,2)],{},None)
            self.assertEqual(mocked.call_count,2)

    def test_resume_and_core_only_require_complete_identical_saved_capture(self):
        for core_only in (False,True):
            with self.subTest(core_only=core_only),tempfile.TemporaryDirectory() as directory:
                folder=Path(directory);args=self.args(folder,resume=True,core_only=core_only)
                (folder/'probe-official.json').write_text(json.dumps(self.raw()))
                servers=[('ours',None,2)] if core_only else [('official',None,1),('ours',None,2)]
                def capture(*call,**kwargs):
                    self.assertEqual(call[7]['scenario'],'probe')
                    raw=self.raw();call[4].write_text(json.dumps(raw));return raw
                with patch('record777.record',side_effect=capture) as mocked:
                    run_scenario_sides(args,{'name':'probe'},servers,{},None,{'record_status':'complete'},args.manifests)
                    self.assertEqual(mocked.call_count,1)
                for previous,raw in [({'record_status':'failed'},self.raw()),
                                     ({'record_status':'complete'},self.raw(scenario='different')),
                                     ({'record_status':'complete'},self.raw(phase_alignment={'verified':False})),
                                     ({'record_status':'complete'},self.raw(clients=[{'events':[]}])),
                                     ({'record_status':'complete'},self.raw(clients=[{'events':[1],'failure':'disconnect'}]))]:
                    (folder/'probe-official.json').write_text(json.dumps(raw))
                    with patch('record777.record') as mocked:
                        with self.assertRaises(ValueError):
                            run_scenario_sides(args,{'name':'probe'},servers,{},None,previous,args.manifests)
                        mocked.assert_not_called()

    def test_saved_reference_rejects_changed_scenario_data_or_missing_old_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory);args=self.args(folder,resume=True,core_only=True)
            (folder/'probe-official.json').write_text(json.dumps(self.raw()))
            servers=[('ours',None,2)]
            with patch('record777.record') as mocked:
                with self.assertRaisesRegex(ValueError,'previous source/data manifest'):
                    run_scenario_sides(args,{'name':'probe'},servers,{},None,{'record_status':'complete'},[])
                for target in [folder/'entities.json',folder/'tools/blackbox/scenarios/probe.json']:
                    old=target.read_text();target.write_text(old+' ')
                    with self.assertRaisesRegex(ValueError,'previous source/data manifest'):
                        run_scenario_sides(args,{'name':'probe'},servers,{},None,{'record_status':'complete'},args.manifests)
                    target.write_text(old)
                mocked.assert_not_called()

    def test_skip_recorded_core_cannot_relabel_old_runtime_as_fresh(self):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory);args=self.args(folder,resume=True,core_only=True,skip_recorded_core=True)
            (folder/'probe-ours.json').write_text(json.dumps(self.raw()))
            (folder/'probe-official.json').write_text(json.dumps(self.raw()))
            with patch('record777.record') as mocked:
                with self.assertRaisesRegex(ValueError,'per-file runtime identity'):
                    run_scenario_sides(args,{'name':'probe'},[('ours',None,2)],{},None,{'record_status':'complete'},args.manifests)
                mocked.assert_not_called()

    def test_attribute_and_mob_alignment_is_joint_or_explicitly_fails(self):
        # Исходный+1 сохранил бы60цикл, но тихо испортил бы уже выбранную2фазу.
        mob={'source':'mob_tracking','period':60,'residue':1}
        with self.assertRaisesRegex(ValueError,'incompatible recording phases'):
            aligned_start(120,[{'source':'player_attributes','period':2,'residue':0},mob])
        self.assertEqual(aligned_start(120,[mob]),121)
        self.assertEqual(aligned_start(120,[{'source':'player_attributes','period':2,'residue':1},mob]),121)
        self.assertEqual(aligned_start(121,[{'period':2,'residue':0},{'period':60,'residue':0}]),180)
        self.assertEqual(aligned_start(120,[{'period':2,'residue':0},{'period':60,'residue':0}]),120)


class HarnessTests(unittest.TestCase):
    def test_stationary_client_keeps_twenty_tick_heartbeat_and_moves_immediately(self):
        class Socket:
            def setsockopt(self,*args):pass
        protocol={'play':{direction:{'types':{'packet':['container',[{'type':['mapper',{'mappings':{}}]}]]}} for direction in ('toClient','toServer')}}
        client=make_recorder(Socket(),protocol,777)
        client.position=[3.5,-59.,3.5,0.,0.]
        sent=[]
        with patch('record777.Client.send',lambda self,name,payload=b'':sent.append((name,payload))):
            for _ in range(21):client.send('tick_end')
            self.assertEqual(sum(n=='tick_end' for n,p in sent),21)
            self.assertEqual(sum(n=='position_look' for n,p in sent),2)
            client.position[0]+=1
            client.send('tick_end')
            self.assertEqual(sum(n=='position_look' for n,p in sent),3)

    def test_bot_radians_and_block_aim_use_protocol_degrees(self):
        import math
        self.assertEqual(protocol_rotation(0,0),[180.,0.])
        self.assertEqual(protocol_rotation(math.pi,-math.pi/2),[0.,90.])
        class Actor:
            position=[3.5,0.,3.5,180.,0.]
        actor=Actor()
        aim_at(actor,[3.5,1.62,4.5])
        self.assertEqual(actor.position[3:],[0.,0.])
        aim_at(actor,[3.5,.62,3.5])
        self.assertEqual(actor.position[3:],[0.,90.])

    def test_scenario_dig_actions_use_the_observed_wire_enum(self):
        self.assertEqual([dig_status(n,775) for n in range(8)],list(range(8)))
        self.assertEqual([dig_status(n,777) for n in range(8)],[0,2,3,4,5,6,7,8])
        for n in (-1,8):
            with self.assertRaises(ValueError):dig_status(n,777)

    def test_projection_keeps_the_original_bots_ten_tick_tail(self):
        with tempfile.TemporaryDirectory() as folder:
            data=Path(folder)
            for name in ('items','entities','blocks'):(data/f'{name}.json').write_text('[]')
            scenario=json.loads((Path(__file__).parent/'scenarios/pvp-fist.json').read_text())
            record={'initial':[{'entity':1},{'entity':2}],'clients':[{'events':[]},{'events':[]}],'start':0,'sent':[]}
            decoder=Decoder({'types':{},'play':{'toClient':{'types':{}},'toServer':{'types':{}}}})
            output=data/'events.json'
            self.assertEqual(project_record(record,decoder,scenario,data,output,{}),[])
            self.assertEqual(json.loads(output.read_text())['window']['last'],scenario['record_ticks']+10)

    def test_jukebox_projection_uses_the_advertised_song_name(self):
        from record777 import pack_position
        protocol={'types':{},'play':{'toClient':{'types':{'packet_world_event':['container',[
            {'name':'effectId','type':'i32'},{'name':'location','type':['bitfield',[
                {'name':'x','size':26,'signed':True},{'name':'z','size':26,'signed':True},{'name':'y','size':12,'signed':True}]]},
            {'name':'data','type':'i32'},{'name':'global','type':'bool'}]]}},'toServer':{'types':{}}}}
        with tempfile.TemporaryDirectory() as folder:
            data=Path(folder)
            for name in ('items','entities','blocks'):(data/f'{name}.json').write_text('[]')
            scenario=dict(name='jukebox-disc',record_ticks=1)
            def project(songs,index):
                payload=struct.pack('>i',1010)+pack_position([100,-59,100])+struct.pack('>i?',index,False)
                record={'protocol':777,'origin':[100,-59,100],'initial':[{'entity':1}], 'start':0,'sent':[],
                    'clients':[{'events':[dict(at=0,name='world_event',hex=payload.hex())],
                                'registry_names':{'minecraft:jukebox_song':songs}}]}
                output=data/'events.json'
                errors=project_record(record,Decoder(protocol),scenario,data,output,{})
                return errors,json.loads(output.read_text())['events']
            official=project(['minecraft:bounce','minecraft:cat'],1)
            core=project(['minecraft:cat','minecraft:bounce'],0)
            self.assertEqual(official,core)
            self.assertEqual(official[0],[])
            self.assertEqual(official[1][0]['data'],'song:minecraft:cat')
            self.assertNotEqual(official,project(['minecraft:cat','minecraft:bounce'],1))
            self.assertTrue(project(['minecraft:cat'],-1)[0])
            self.assertTrue(project(['minecraft:cat'],1)[0])
            self.assertTrue(project(None,0)[0])

    def test_optional_anonymous_nbt_end_and_compound(self):
        decoder=Decoder({'types':{},'play':{'toClient':{'types':{}}}})
        self.assertIsNone(decoder.read(io.BytesIO(b'\0'),'anonOptionalNbt'))
        self.assertEqual(decoder.read(io.BytesIO(b'\x0a\x01\0\x01x\x07\0'),'anonOptionalNbt'),{'x':7})

    def test_interact_location_matches_original_bot(self):
        x,y,z = velocity(io.BytesIO(pack_interact_location()))
        self.assertEqual((x,z),(0.,0.))
        self.assertAlmostEqual(y,.35,delta=1/16383)

    def test_packed_position_signed(self):
        decoder=Decoder({'types':{},'play':{'toClient':{'types':{}}}})
        schema=['bitfield',[{'name':'x','size':26,'signed':True},{'name':'z','size':26,'signed':True},{'name':'y','size':12,'signed':True}]]
        raw=(((-12)&0x3ffffff)<<38)|(108<<12)|((-59)&0xfff)
        self.assertEqual(decoder.read(io.BytesIO(raw.to_bytes(8,'big')),schema),{'x':-12,'z':108,'y':-59})

    def test_teleport_confirmation_version(self):
        class Socket:
            def setsockopt(self,*args):pass
        protocol={'play':{direction:{'types':{'packet':['container',[{'type':['mapper',{'mappings':{}}]}]]}} for direction in ('toClient','toServer')}}
        for version,length in ((775,1),(777,33)):
            client=make_recorder(Socket(),protocol,version);sent=[]
            client.send=lambda name,payload:sent.append((name,payload))
            client.confirm_teleport(1,[0.,0.,0.,0.,0.])
            self.assertEqual(len(sent[0][1]),length)

    def test_repeated_dead_health_does_not_queue_two_respawns(self):
        class Socket:
            def setsockopt(self,*args):pass
        protocol={'play':{direction:{'types':{'packet':['container',[{'type':['mapper',{'mappings':{}}]}]]}} for direction in ('toClient','toServer')}}
        client=make_recorder(Socket(),protocol,777);sent=[]
        client.send=lambda name,payload=b'':sent.append((name,payload))
        for health in (0.,0.,20.,0.):
            client.observe_packet('update_health',struct.pack('>f',health)+b'\x14'+struct.pack('>f',5.))
        self.assertEqual(sent,[('client_command',b'\0'),('client_command',b'\0')])

    def test_destroyed_entities_are_not_attack_targets(self):
        class Socket:
            def setsockopt(self,*args):pass
        protocol={'play':{direction:{'types':{'packet':['container',[{'type':['mapper',{'mappings':{}}]}]]}} for direction in ('toClient','toServer')}}
        client=make_recorder(Socket(),protocol,777)
        client.entities=[(12,1),(13,1)];client.targets=[12,13]
        client.observe_packet('entity_destroy',b'\x01\x0c')
        self.assertEqual(client.entities,[(13,1)])
        self.assertEqual(client.targets,[13])

    def test_window_scenario_and_revision_encoding(self):
        scenario=json.loads((Path(__file__).parent/'scenarios/shield-banner-craft.json').read_text())
        validate(scenario)
        payload=io.BytesIO(window_click_payload(7,300,{'slot':37,'button':1,'mode':0}))
        self.assertEqual((read_varint(payload.read),read_varint(payload.read)),(7,300))
        self.assertEqual(struct.unpack('>hb',payload.read(3)),(37,1))
        self.assertEqual(read_varint(payload.read),0)
        self.assertEqual(payload.read(),b'\0\0')

    def test_original_handler_flags_and_observer(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            for name in ('items','entities','blocks'):(root/f'{name}.json').write_text('[]')
            scenario=root/'scenario.json';raw=root/'raw.json';output=root/'events.json'
            payload={'initial':[{'entity':1},{'entity':2}],'start':0,'sent':[{'at':0,'tick':0,'action':{'attack':True}}],'last':40,
                     'packets':[{'client':i,'at':.05,'name':'update_health','value':{'health':19-i,'food':20,'foodSaturation':5}} for i in range(2)]}
            payload['packets'].append({'client':0,'at':.05,'name':'tile_entity_data','value':{'location':{'x':100,'y':-59,'z':100},'action':4,'nbtData':{'note':1}}})
            raw.write_text(json.dumps(payload))
            for flags,expected in (({},[]),({'track_self':True},['Set Health health=19 food=20 saturation=5']),({'track_second':True},['b: Set Health health=18 food=20 saturation=5'])):
                scenario.write_text(json.dumps({'second':True,**flags}))
                subprocess.run(['node',str(Path(__file__).parent/'replay777.js'),str(raw),str(scenario),str(root),str(output)],check=True)
                self.assertEqual([e['text'] for e in json.loads(output.read_text())['events'] if e['kind']=='sv'],expected)
                self.assertEqual([(e['type'],e['data']) for e in json.loads(output.read_text())['events'] if e['kind']=='block_entity'],[(4,'{"note":1}')])


if __name__=='__main__':unittest.main()
