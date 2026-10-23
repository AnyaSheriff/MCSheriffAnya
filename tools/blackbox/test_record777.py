"""Семантика проекции и кодирование действий без сторонних реализаций."""
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from analyze777 import Decoder
from record777 import make_recorder, validate, window_click_payload
from smoke import read_varint


class HarnessTests(unittest.TestCase):
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

    def test_destroyed_entities_are_not_attack_targets(self):
        class Socket:
            def setsockopt(self,*args):pass
        protocol={'play':{direction:{'types':{'packet':['container',[{'type':['mapper',{'mappings':{}}]}]]}} for direction in ('toClient','toServer')}}
        client=make_recorder(Socket(),protocol,777)
        client.entities=[(12,1),(13,1)]
        client.observe_packet('entity_destroy',b'\x01\x0c')
        self.assertEqual(client.entities,[(13,1)])

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
            raw.write_text(json.dumps(payload))
            for flags,expected in (({},[]),({'track_self':True},['Set Health health=19 food=20 saturation=5']),({'track_second':True},['b: Set Health health=18 food=20 saturation=5'])):
                scenario.write_text(json.dumps({'second':True,**flags}))
                subprocess.run(['node',str(Path(__file__).parent/'replay777.js'),str(raw),str(scenario),str(root),str(output)],check=True)
                self.assertEqual([e['text'] for e in json.loads(output.read_text())['events'] if e['kind']=='sv'],expected)


if __name__=='__main__':unittest.main()
