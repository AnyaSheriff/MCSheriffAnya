"""Синтетические разрушения evidence; это НЕ original/gameplay proof."""
import copy
import io
import json
import struct
import unittest
from unittest.mock import patch

import wind_origin_original as pilot


class FakeDecoder:
    def __init__(self, protocol):
        pass

    def read(self, stream, kind):
        return json.load(stream)


def event(name, at, value):
    return dict(name=name, at=at, hex=json.dumps(value).encode().hex())


def fixture():
    actor, victim = 1, 2
    attack, end = 20.461, 24.6
    trial = dict(index=0, material='air', reset_at=10., heal_at=10.3, quiet_at=15., start_tp_at=20.,
                 attack_at=attack, end_at=end, cell_change_at=11., echo='unique_live',
                 victim_inventory_receipt={'response':'blackboxb has the following entity data: []'})
    clients = []
    for entity in (actor, victim):
        events = [event('login', 0., {}), event('game_state_change', 1., dict(reason='change_game_mode',gameMode=0)),
                  event('abilities', 1., dict(flags=0)), event('update_health',10.2,dict(health=199)), event('update_health',10.5,dict(health=200)),
                  event('block_change',11.1,dict(location=dict(x=100,y=63,z=100),type=0))]
        events += [event('update_time',float(t),dict(age=(t-10)*20)) for t in range(15,25)]
        events.append(event('system_chat',25.,dict(text='unique_live')))
        clients.append(dict(entity=entity, events=events, outbound=[], moves=[], acks=[], failure=None,
                            registries={'minecraft:damage_type':['minecraft:mace_smash'],
                                        'minecraft:enchantment':['minecraft:wind_burst']}))
    clients[0]['acks'] = [dict(at=20.001,id=3,pose=pilot.START+[0,0],motion=[0,0,0])]
    clients[0]['events'].append(dict(at=20.0005,name='position',hex=(pilot.varint(3)+struct.pack('>ddddddffI',*pilot.START,0,0,0,0,0,0)).hex()))
    position, motion = list(pilot.START), [0,0,0]
    for n in range(10):
        position, motion, grounded = pilot.move(position,motion,pilot.blocks('air'))
        clients[0]['moves'].append(dict(at=20.01+n*.05,position=position,motion=motion,onGround=grounded))
    clients[0]['moves'].append(dict(at=20.51,position=[100.5,63.32,100.5],motion=[0,-.7,0],onGround=False))
    clients[0]['outbound'] = [dict(at=20.001,name='teleport_confirm',hex=(pilot.varint(3)+struct.pack('>dddff',*pilot.START,0,0)).hex())]
    clients[0]['outbound'] += [dict(at=m['at'],name='position_look',hex=struct.pack('>dddffB',*m['position'],0,0,int(m['onGround'])).hex()) for m in clients[0]['moves'][:-1]]
    clients[0]['outbound'] += [dict(at=attack,name='attack',hex=pilot.varint(victim).hex()),
                              dict(at=attack+.0001,name='tick_end',hex='')]
    slot = dict(itemCount=1,itemId=10,components=[dict(type='enchantments',data=dict(enchantments=[dict(id=0,level=1)]))])
    menu = dict(items=[dict(itemCount=0)]*54+[slot]+[dict(itemCount=0)]*8,carriedItem=dict(itemCount=0))
    clients[0]['events'].append(event('window_items',12.,menu))
    clients[1]['events'] += [
        event('spawn_entity',1.,dict(entityId=actor,x=100.5,y=pilot.START[1],z=100.5)),
        event('sync_entity_position',20.48,dict(entityId=actor,positionType='linear',x=100.5,y=64.04,z=100.5,onGround=False)),
        event('damage_event',20.48,dict(entityId=victim,sourceTypeId=0,sourceCauseId=actor+1,sourceDirectId=actor+1)),
        event('sound_effect',20.48,dict(sound=dict(soundId=1))),
        event('update_health',20.49,dict(health=180))]
    clients[0]['events'].append(event('explosion',20.48,dict(center=dict(x=100.5,y=63.94,z=100.5),blockCount=0,sound=dict(soundId=2))))
    for client in clients:
        client['events'].sort(key=lambda e:e['at'])
    return dict(protocol=777,trial=trial,clients=clients,sounds={'1':pilot.SOUNDS[0],'2':pilot.SOUNDS[3]})


class EvidenceGuards(unittest.TestCase):
    def check(self, raw):
        with patch.object(pilot,'Decoder',FakeDecoder):
            return pilot.audit(raw,{}, {10:'mace'}, {'air':0})

    def test_positive_scaffold_only(self):
        result = self.check(fixture())
        self.assertEqual(result['B_effect'],'EVENT_PRESENT_COUNT_ZERO')
        self.assertEqual(result['A_origin'],'EVENT_PRESENT_C_INSIDE_U')

    def test_independent_A_and_B_no_retime(self):
        raw=fixture()
        explosion=next(e for e in raw['clients'][0]['events'] if e['name']=='explosion')
        value=json.loads(bytes.fromhex(explosion['hex']));value['center']['y']=64.04
        explosion['hex']=json.dumps(value).encode().hex()
        result=self.check(raw)
        self.assertEqual(result['A_origin'],'INCONCLUSIVE_ORIGIN_GEOMETRY')
        self.assertEqual(result['B_effect'],'EVENT_PRESENT_COUNT_ZERO')

    def test_missing_mandatory_receipts_rejected(self):
        for name,side in [('login',0),('system_chat',0),('damage_event',1),('sound_effect',1),
                          ('update_health',1),('window_items',0),('sync_entity_position',1),
                          ('block_change',0),('abilities',0),('update_time',0),('explosion',0)]:
            with self.subTest(name=name):
                raw=fixture();raw['clients'][side]['events']=[e for e in raw['clients'][side]['events'] if e['name']!=name]
                with self.assertRaises(AssertionError):self.check(raw)

    def test_wrong_or_stale_ack_and_grounding_rejected(self):
        for mutate in [lambda r:r['clients'][0]['acks'].clear(),
                       lambda r:r['clients'][0]['acks'][0].update(at=19),
                       lambda r:r['clients'][0]['acks'][0].update(motion=[0,.01,0]),
                       lambda r:r['clients'][0]['moves'][-2].update(onGround=True),
                       lambda r:r['clients'][0].update(failure='socket gone'),
                       lambda r:r['clients'][0]['outbound'].reverse(),
                       lambda r:r['trial'].update(quiet_at=19),
                       lambda r:r.update(protocol=768)]:
            raw=fixture();mutate(raw)
            with self.assertRaises(AssertionError):self.check(raw)

    def test_nonzero_peer_offset_does_not_qualify(self):
        raw=fixture()
        e=next(e for e in raw['clients'][1]['events'] if e['name']=='sync_entity_position')
        v=json.loads(bytes.fromhex(e['hex']))
        v.update(positionType='stepped',steps=[dict(x=100.5,y=64.04,z=100.5,tickOffset=1)])
        e['hex']=json.dumps(v).encode().hex()
        with self.assertRaisesRegex(AssertionError,'INCONCLUSIVE_SERVER_POSE'):self.check(raw)
        v['steps'][0]['tickOffset']=0;e['hex']=json.dumps(v).encode().hex()
        self.assertEqual(self.check(raw)['status'],'qualified-observation')

    def test_coalesced_reset_health_does_not_qualify(self):
        raw=fixture()
        raw['clients'][1]['events']=[e for e in raw['clients'][1]['events'] if not (e['name']=='update_health' and e['at']==10.2)]
        with self.assertRaisesRegex(AssertionError,'INCONCLUSIVE_RESET_HP_RECEIPT'):self.check(raw)

    def test_feedback_ownership_and_wire_order(self):
        velocity=dict(entityId=1,velocity=dict(x=0,y=.01,z=0))
        blast=dict(playerKnockback=dict(x=0,y=1.2,z=0))
        reset=pilot.feedback([0,-.7,0],1,'entity_velocity',velocity)
        self.assertEqual(reset,[0,.01,0])
        self.assertAlmostEqual(pilot.feedback(reset,1,'explosion',blast)[1],1.21)
        impulse_first=pilot.feedback([0,-.7,0],1,'explosion',blast)
        self.assertAlmostEqual(pilot.feedback(impulse_first,1,'entity_velocity',velocity)[1],.01)
        velocity['entityId']=99
        self.assertEqual(pilot.feedback([0,-.7,0],1,'entity_velocity',velocity),[0,-.7,0])
        self.assertEqual(pilot.feedback([0,-.7,0],1,'explosion',{'playerKnockback':None}),[0,-.7,0])

    def test_centre_bounds(self):
        self.assertTrue(pilot.inside(dict(x=100.5,y=63.5,z=100.5)))
        self.assertFalse(pilot.inside(dict(x=100.5,y=64.,z=100.5)))
        self.assertFalse(pilot.inside(dict(x=100.,y=63.5,z=100.5)))

    def test_real_hole_and_solid_collision(self):
        p=pilot.START;v=[0,0,0]
        for _ in range(10):p,v,g=pilot.move(p,v,pilot.blocks('air'))
        self.assertAlmostEqual(p[1],64.04)
        self.assertFalse(g)
        air,_,ag=pilot.move(p,v,pilot.blocks('air'))
        stone,_,sg=pilot.move(p,v,pilot.blocks('stone'))
        self.assertLess(air[1],64)
        self.assertFalse(ag)
        self.assertEqual(stone[1],64)
        self.assertTrue(sg)

    def test_no_local_execution(self):
        with patch.dict(pilot.os.environ,{},clear=True):
            with self.assertRaisesRegex(AssertionError,'GitHub-only'):
                pilot.run(None)


if __name__=='__main__':unittest.main()
