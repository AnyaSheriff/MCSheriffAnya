"""Peaceful не принимается за успешный тест при отсутствии самого взрыва."""
import copy,json,unittest
from pathlib import Path
from unittest.mock import patch
import armor_difficulty_sample as probe

class DifficultyControls(unittest.TestCase):
 def test_known_peaceful_requires_actual_explosion_not_empty_record(self):
  raw={'protocol':777,'isolate_vitals':True,'initial':[],'clients':[]}
  with self.assertRaises(AssertionError):probe.harmless(raw,{}, {})
 def test_normalized_nonpeaceful_keeps_source_name_and_difficulty_command(self):
  raw={'scenario':probe.name('easy'),'commands':[{'command':'difficulty easy'}]}
  with patch.object(probe.armor,'inspect',return_value=[{'trial':0,'cause':7,'direct':8,'victim_damage':5.}]) as inner:
   rows=probe.inspect(raw,'easy',{},{});self.assertEqual(rows,[{'trial':0,'victim_damage':5.}]);self.assertEqual(inner.call_args.args[0]['scenario'],probe.armor.name('blast'));self.assertEqual(raw['scenario'],probe.name('easy'))
  raw['commands']=[]
  with self.assertRaises(AssertionError):probe.inspect(raw,'easy',{}, {})
