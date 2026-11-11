import unittest,struct
import health
class HealthProbeTests(unittest.TestCase):
 def row(self,health_value):return dict(name='update_health',hex=(struct.pack('>f',health_value)+b'\x14'+struct.pack('>f',5.)).hex(),at=1.)
 def test_exact_float_packet_proofs(self):
  for value in [20.,19.5,13.375]:self.assertEqual(health.health_proof([self.row(value)],value)['health'],value)
 def test_latest_wins_and_stale_health_fails(self):
  with self.assertRaises(AssertionError):health.health_proof([self.row(20.),self.row(19.5)],20.)
 def test_unknown_trailing_or_short_packet_rejected(self):
  for payload in ['','00',self.row(20.)['hex']+'00']:
   with self.assertRaises((AssertionError,struct.error,IndexError,TypeError)):health.health_proof([dict(name='update_health',hex=payload,at=1.)],20.)
 def test_nonfinite_rejected(self):
  for value in [float('nan'),float('inf')]:
   with self.assertRaises(AssertionError):health.health_proof([self.row(value)],value)
 def test_missing_health_rejected(self):
  with self.assertRaises(AssertionError):health.health_proof([],20.)
 def test_fixed_finite_plan_and_counter_readback(self):
  self.assertEqual([s['health'] for s in health.HEALTH_STATES],[20.,19.5,13.375]);self.assertEqual(len(health.HEALTH_QUERIES),12)
  for holder in ['raw','success','scaled','missing_result','missing_success']:
   index=next(i for i,q in enumerate(health.HEALTH_QUERIES) if q.startswith('execute store') and 'score '+holder+' ' in q)
   self.assertEqual(health.HEALTH_QUERIES[index+1],f'scoreboard players get {holder} MCSAHealth')
 def test_actual_survival_mode_and_abilities(self):
  mode=dict(name='game_state_change',hex=(b'\x03'+struct.pack('>f',0.)).hex());ability=dict(name='abilities',hex=(b'\x00'+struct.pack('>ff',.05,.1)).hex())
  self.assertEqual(health.survival_proof([mode,ability])['mode'],mode)
  creative=dict(ability,hex='0d'+ability['hex'][2:])
  with self.assertRaises(LookupError):health.survival_proof([mode,creative])
 def test_last_mode_and_packet_types_matter(self):
  mode=dict(name='game_state_change',hex=(b'\x03'+struct.pack('>f',1.)).hex());ability=dict(name='abilities',hex=(b'\x00'+struct.pack('>ff',.05,.1)).hex())
  with self.assertRaises(AssertionError):health.survival_proof([mode,ability])
  with self.assertRaises(LookupError):health.survival_proof([])
if __name__=='__main__':unittest.main()
