"""Проверки сырого inventory echo, реального попадания и нормативного результата."""
import copy
import json
from pathlib import Path
import struct
import unittest
from unittest.mock import patch
import armor_sources_sample as probe
from smoke import varint


def event(name,at,body):return {'name':name,'at':at,'hex':body.hex()}
def hp(value,at):return event('update_health',at,struct.pack('>f',value)+varint(20)+struct.pack('>f',5))
def slot(number,at,count=1,enchant=False):
 data=b''.join(varint(v) for v in [count,123,2 if enchant else 1,0,3,431])
 if enchant:data+=b''.join(varint(v) for v in [13,1,0,3])
 return event('set_slot',at,b'\0\0'+struct.pack('>h',number)+data)
def fixture(enchant=False):
 raw={'scenario':probe.name('arrow'),'protocol':777,'isolate_vitals':True,'start':10.,
      'initial':[{'entity':v,'health':20.,'health_receipt':hp(20.,9.9)} for v in [5,9]],'clients':[],
      'sent':[{'at':10.5,'action':{'after':10,'use':True,'bot':2}}, {'at':11.3,'action':{'after':26,'release':True,'bot':2}},
              {'at':12.7,'action':{'after':54,'window_click':{'inventory':True,'slot':6,'mode':1}}}]}
 for side in [0,1]:
  events=[event('spawn_entity',11.35,varint(11)+bytes(16)+varint(99)),event('damage_event',11.4,b''.join(varint(v) for v in [5,0,10,12])+b'\0')]
  if side==0:events=[slot(6,10.2,enchant=enchant)]+events+[hp(15.,11.41),event('set_slot',12.71,b'\0\0'+struct.pack('>h',6)+b'\0'),slot(9,12.72,enchant=enchant)]
  raw['clients'].append({'preroll':[],'events':events,'failure':None,'registry_names':{'minecraft:damage_type':['minecraft:arrow'],'minecraft:enchantment':['minecraft:unbreaking']},'moves':[]})
 return raw


class ArmorSourceControls(unittest.TestCase):
 def inspect(self,raw,enchant=False):
  with patch.dict(probe.CASES,{'arrow':[('elytra',6,431,'unbreaking' if enchant else None)]}):
   return probe.inspect(raw,'arrow',{'elytra':123},{'arrow':99})
 def test_actual_echo_preserves_elytra_and_enchantment(self):
  for enchanted in [False,True]:self.assertEqual(self.inspect(fixture(enchanted),enchanted)[0]['final_damage'],431)
 def test_rejects_duplication_and_lost_enchantment(self):
  raw=fixture();raw['clients'][0]['events'][-1]=slot(9,12.72,count=2)
  with self.assertRaises(AssertionError):self.inspect(raw)
  raw=fixture(True);raw['clients'][0]['events'][-1]=slot(9,12.72)
  with self.assertRaises(AssertionError):self.inspect(raw,True)
 def test_rejects_missing_hit_health_or_echo(self):
  for name in ['damage_event','update_health','set_slot']:
   raw=fixture();raw['clients'][0]['events']=[v for v in raw['clients'][0]['events'] if v['name']!=name]
   with self.subTest(name=name),self.assertRaises((AssertionError,StopIteration)):self.inspect(raw)
 def test_rejects_missing_spawn_wrong_owner_and_truncated_damage(self):
  for mode in ['spawn','owner','payload']:
   raw=fixture();events=raw['clients'][1]['events']
   if mode=='spawn':events.pop(0)
   elif mode=='owner':events[1]=event('damage_event',11.4,b''.join(varint(v) for v in [5,0,6,12])+b'\0')
   else:events[1]['hex']+='00'
   with self.subTest(mode=mode),self.assertRaises(AssertionError):self.inspect(raw)
 def test_frozen_expected_results_reject_wrong_wear_health_and_source(self):
  expected=json.loads((Path(__file__).parent/'ci/armor-sources-777.json').read_text())['expected']
  for kind,rows in expected.items():
   probe.validate(kind,copy.deepcopy(rows),expected)
   for key,value in [('victim_damage',0.),('final_damage',999),('direct_roles',['missing','missing'])]:
    wrong=copy.deepcopy(rows);wrong[0][key]=value
    with self.subTest(kind=kind,key=key),self.assertRaises(AssertionError):probe.validate(kind,wrong,expected)
