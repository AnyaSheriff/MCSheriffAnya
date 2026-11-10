import copy,json,unittest
from pathlib import Path
from unittest.mock import patch
from witch_bounded_audit import inspect

class FakeDecoder:
    def __init__(self, _): pass
    def read(self, stream, _): return json.loads(stream.read())

def event(name,value,t):
    return {'name':name,'at':100+t*.05,'hex':json.dumps(value).encode().hex()}

def fixture(kind='near'):
    end=610 if kind=='noai' else 410
    x={'near':102.5,'far':109.5,'wall':107.5,'creative':107.5,'noai':107.5}[kind]
    name=f'witch-bounded-{kind}-1';mode=1 if kind=='creative' else 0
    meta=[{'key':9,'value':26.0}]+([{'key':15,'value':1}] if kind=='noai' else [])
    spawn=lambda ident,typ,owner:{'entityId':ident,'type':typ,'x':x,'y':-59.,'z':100.5,'objectData':owner}
    events=[event('spawn_entity',spawn(20,148,0),10),event('entity_metadata',{'entityId':20,'metadata':meta},11),event('update_time',{},end),event('entity_destroy',{'entityIds':[20]},end)]
    if kind in ('near','far'):
        for i,potion,effect in [(30,28,2002),(31,26,2007)]:
            t=50+(i-30)*60
            events.extend([event('spawn_entity',spawn(i,108,20),t),event('entity_metadata',{'entityId':i,'metadata':[{'key':8,'value':{'itemCount':1,'itemId':500,'components':[{'type':'potion_contents','data':{'potionId':potion}}]}}]},t),event('world_event',{'effectId':effect},t+5),event('entity_destroy',{'entityIds':[i]},t+5)])
        events += [event('entity_effect',{'entityId':10,'effectId':18},55),event('damage_event',{'entityId':10,'sourceTypeId':23,'sourceCauseId':21,'sourceDirectId':32},115)]
    actions=json.loads((Path(__file__).parent/'scenarios'/f'{name}.json').read_text())['actions']
    ready=event('update_health',{'health':20.},-3)
    return {'protocol':777,'scenario':name,'isolate_vitals':True,'start':100.,'origin':[100,-59,100],
       'initial':[{'entity':10,'health':20.,'position':[100.5,-59.,100.5,0.,0.],'health_receipt':ready}],
       'clients':[{'failure':None,'events':events,'outbound':[],'moves':[{'at':100.,'position':{'x':100.5,'y':-59.,'z':100.5}},{'at':100+end*.05,'position':{'x':100.5,'y':-59.,'z':100.5}}],'preroll':[event('game_state_change',{'reason':'change_game_mode','gameMode':mode},-5),event('entity_effect',{'entityId':10,'effectId':10,'amplifier':4,'duration':5000},-4),ready]}],
       'sent':[{'at':100.5,'action':actions[0]},{'at':100+end*.05,'action':actions[1]}]}

class Guards(unittest.TestCase):
    def setUp(self):
        self.decoder=patch('witch_bounded_audit.Decoder',FakeDecoder);self.decoder.start();self.addCleanup(self.decoder.stop)
        self.hp=patch('witch_bounded_audit.health_packet',return_value=20.);self.hp.start();self.addCleanup(self.hp.stop)
        self.types={'witch':148,'splash_potion':108,'lingering_potion':109,'splash_item':500}
    def check(self,r):return inspect(r,{},self.types,r['scenario'])
    def test_positive_requires_real_separate_packets(self):
        row=self.check(fixture());self.assertTrue(row['poison_effect']);self.assertTrue(row['harming_cause_direct'])
        r=fixture();r['clients'][0]['events']=[x for x in r['clients'][0]['events'] if x['name']!='damage_event'];self.assertFalse(self.check(r)['harming_cause_direct'])
    def test_wrong_owner_is_rejected(self):
        r=fixture();e=next(x for x in r['clients'][0]['events'] if x['name']=='spawn_entity' and json.loads(bytes.fromhex(x['hex']))['type']==108)
        v=json.loads(bytes.fromhex(e['hex']));v['objectData']=99;e['hex']=json.dumps(v).encode().hex()
        with self.assertRaises(ValueError):self.check(r)
    def test_no_mode_and_no_hp_are_rejected(self):
        r=fixture();r['clients'][0]['preroll']=[]
        with self.assertRaises(ValueError):self.check(r)
        r=fixture();r['initial'][0]['health_receipt']=None
        with self.assertRaises(ValueError):self.check(r)
    def test_wall_bypass_and_noai_missing_are_rejected(self):
        r=fixture('wall');e=next(x for x in r['clients'][0]['events'] if x['name']=='spawn_entity');v=json.loads(bytes.fromhex(e['hex']));v['z']=110.;e['hex']=json.dumps(v).encode().hex()
        with self.assertRaises(ValueError):self.check(r)

    def test_wall_v2_enclosure_rejects_escape(self):
        r=fixture('wall');r['scenario']='witch-bounded-wall-v2';self.check(r)
        r['clients'][0]['events'].insert(2,event('rel_entity_move',{'entityId':20,'move':{'steps':[{'delta':[0,0,18432]}]}},30))
        with self.assertRaises(ValueError):self.check(r)
        r=fixture('noai');e=next(x for x in r['clients'][0]['events'] if x['name']=='entity_metadata');v=json.loads(bytes.fromhex(e['hex']));v['metadata']=[x for x in v['metadata'] if x['key']!=15];e['hex']=json.dumps(v).encode().hex()
        with self.assertRaises(ValueError):self.check(r)
    def test_mode_switch_dead_witch_and_drinking_attack_rejected(self):
        r=fixture();r['clients'][0]['events'].append(event('game_state_change',{'reason':'change_game_mode','gameMode':1},80))
        with self.assertRaises(ValueError):self.check(r)
        r=fixture();r['clients'][0]['events'].append(event('entity_metadata',{'entityId':20,'metadata':[{'key':9,'value':0.}]},100))
        with self.assertRaises(ValueError):self.check(r)
        r=fixture();r['clients'][0]['events'].append(event('entity_metadata',{'entityId':20,'metadata':[{'key':17,'value':True}]},40))
        with self.assertRaises(ValueError):self.check(r)
    def test_resistance_and_player_movement_are_required(self):
        r=fixture();r['clients'][0]['preroll']=[x for x in r['clients'][0]['preroll'] if x['name']!='entity_effect']
        with self.assertRaises(ValueError):self.check(r)
        r=fixture();r['clients'][0]['moves'][-1]['position']['x']=105.0
        with self.assertRaises(ValueError):self.check(r)

    def test_early_removal_and_incomplete_window_rejected(self):
        r=fixture();r['clients'][0]['events'].append(event('entity_destroy',{'entityIds':[20]},100))
        with self.assertRaises(ValueError):self.check(r)
        r=fixture();r['clients'][0]['events']=[x for x in r['clients'][0]['events'] if x['name']!='update_time']
        with self.assertRaises(ValueError):self.check(r)

    def test_preparation_death_is_not_observation_death(self):
        r=fixture();r['clients'][0]['preroll'].insert(0,event('update_health',{'health':0.},-10))
        self.check(r)
        r['clients'][0]['events'].append(event('update_health',{'health':0.},100))
        with self.assertRaises(ValueError):self.check(r)
        r=fixture();r['clients'][0]['preroll'].append(event('update_health',{'health':0.},-1))
        with self.assertRaises(ValueError):self.check(r)

if __name__=='__main__':unittest.main()
