"""Семантика проекции и кодирование действий без сторонних реализаций."""
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from analyze777 import Decoder, project_record
from record777 import make_recorder, validate, window_click_payload, pack_interact_location, velocity
from smoke import read_varint


class HarnessTests(unittest.TestCase):
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
