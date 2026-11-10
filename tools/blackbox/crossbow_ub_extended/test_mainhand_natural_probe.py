import hashlib,json,os,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import mainhand_natural_original as n
import extended_probe as probe
R=Path(os.environ['MCSA_EXTENDED_PROBE_REPO'])
class NaturalOriginalTests(unittest.TestCase):
 def test_local_or_rerun_cannot_create_output(self):
  with tempfile.TemporaryDirectory() as directory:
   output=Path(directory)/'no-output'
   for env in [dict(GITHUB_ACTIONS='false',GITHUB_RUN_ATTEMPT='1'),dict(GITHUB_ACTIONS='true',GITHUB_RUN_ATTEMPT='2')]:
    with patch.dict(os.environ,env):
     with self.assertRaises(AssertionError):n.run(output,Path('no-zip'))
    self.assertFalse(output.exists())
 def test_frozen_mainhand_real_charge_schedule(self):
  pin=n.read(n.PIN);self.assertEqual(n.GROUP,'mainhand-natural');scenario=probe.scenario(n.GROUP,R)
  data=(json.dumps(scenario,ensure_ascii=False,indent=2)+'\n').encode();self.assertEqual(hashlib.sha256(data).hexdigest(),pin['scenario_sha256'])
  self.assertEqual(hashlib.sha256((probe.BASE/'criteria.json').read_bytes()).hexdigest(),pin['criteria_sha256'])
  self.assertEqual(sum(bool(a.get('use')) for a in scenario['actions']),264);self.assertEqual(sum(bool(a.get('release')) for a in scenario['actions']),132)
  self.assertEqual(sum('click' in a for a in scenario['actions']),396)
  self.assertFalse(any('window_click' in a or 'charged_projectiles' in a.get('cmd','') for a in scenario['actions']))
  for c in probe.cases(n.GROUP):
   actions=[a for a in scenario['actions'] if c['start']<=a['after']<c['start']+c['duration']];uses=[a for a in actions if a.get('use')];release=next(a for a in actions if a.get('release'))
   self.assertEqual([a['hand'] for a in uses],[0,0]);self.assertEqual(release['after']-uses[0]['after'],30)
 def test_synthetic_statistical_failure_is_frozen_and_cannot_be_overwritten(self):
  with tempfile.TemporaryDirectory() as directory:
   root=Path(directory);raw=root/'synthetic-only.json';raw.write_text('{"synthetic":true}');pin=n.read(n.PIN)
   observation=dict(statistics=dict(statistical_pass=False));capture=dict(source='synthetic-unit-not-runtime',requested_capture_sides=['official'],criteria_sha256=pin['criteria_sha256'],scenario_sha256=pin['scenario_sha256'],recorder_sha256=pin['recorder_sha256'],raw_sha256=n.sha(raw),lifecycle_clean=True,core_executed=False)
   output=root/'FIRST';n.freeze_first(output,raw,observation,capture,pin);self.assertFalse(n.read(output/'original-first-pin.json')['statistical_pass'])
   with self.assertRaises(AssertionError):n.freeze_first(output,raw,observation,capture,pin)
if __name__=='__main__':unittest.main()
