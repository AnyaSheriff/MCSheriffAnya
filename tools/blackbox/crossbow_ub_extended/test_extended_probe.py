import ast,copy,hashlib,json,math,os,tempfile,types,unittest
from pathlib import Path
import extended_probe as p
R=Path(os.environ['MCSA_EXTENDED_PROBE_REPO'])
def fixture(group,emitted=1,survived=False,control=False):
 c=p.cases(group)[0 if control else 2];enchant={'minecraft:unbreaking':3} if c['unbreaking'] else {}
 if c['natural']:enchant['minecraft:multishot']=1
 charges=[dict(item='arrow',count=1)]*3
 initial=dict(item='crossbow',count=1,damage=c['damage'],named_enchantments=enchant,projectiles=[] if c['natural'] else charges)
 loaded=dict(initial,projectiles=charges)
 final=dict(initial,damage=3 if control else 464,projectiles=[]) if survived else None
 status=48 if c['hand'] else 47
 e=dict(initial=initial,loaded=loaded,final=final,actual_hands=[c['hand']]*(2 if c['natural'] else 1),actual_yaw_pitch=[(0.,-20.)]*(2 if c['natural'] else 1),menu_count=3 if c['natural'] else 2,actual_ack=True,old_ids_gone_before_first_use=True,offhand_echo_before_fire=True,offhand_echo_before_load=True,ammo=[3,2,2] if c['natural'] else[None,None],release_status=6 if c['natural'] else None,held_seconds=1.5,no_spawn_during_loading=True,no_break_before_fire=True,emitted_ids=[[(i,'arrow') for i in range(emitted)]]*2,break_status=[[],[]] if survived else[[status],[status]])
 return c,e
class Tests(unittest.TestCase):
 def test_all_three_groups_four_categories_and_fresh_controls(self):
  for group in p.GROUPS:
   for n,survived,label in [(1,False,'one_arrow_then_break'),(2,False,'two_arrows_then_break'),(3,False,'three_arrows_then_break'),(3,True,'three_arrows_survived')]:
    c,e=fixture(group,n,survived);self.assertEqual(p.validate_case(c,e),label)
   c,e=fixture(group,3,True,True);self.assertEqual(p.validate_case(c,e),'plain_fresh_control')
 def test_negative_actual_hand_status_echo_ack_menu_and_cleanup(self):
  mutations={
   'wrong-hand':lambda e:e.update(actual_hands=[0]),
   'mainhand47-for-offhand48':lambda e:e.update(break_status=[[47],[47]]),
   'offhand-echo-missing':lambda e:e.update(offhand_echo_before_fire=False),
   'ack-missing':lambda e:e.update(actual_ack=False),
   'full-menu-missing':lambda e:e.update(menu_count=1),
   'oldID-still-live':lambda e:e.update(old_ids_gone_before_first_use=False),
   'wrong-angle':lambda e:e.update(actual_yaw_pitch=[(10.,-20.)]),
   'observer-spawn-missing':lambda e:e.update(emitted_ids=[[(1,'arrow')],[]]),
   'break-before-fire':lambda e:e.update(no_break_before_fire=False),
   'unbreaking-missing':lambda e:e['initial'].update(named_enchantments={}),
  }
  for label,mutation in mutations.items():
   with self.subTest(label=label):
    c,e=fixture('offhand-commandloaded');mutation(e)
    with self.assertRaises(AssertionError):p.validate_case(c,e)
 def test_negative_natural_charge_ammo_release_preloaded_and_early_emit(self):
  mutations={
   'wrong-charge-hand':lambda e:e.update(actual_hands=[0,1]),
   'missing-charge-echo':lambda e:e.update(offhand_echo_before_load=False),
   'too-short-hold':lambda e:e.update(held_seconds=.2),
   'missing-release':lambda e:e.update(release_status=None),
   'wrong-release-action':lambda e:e.update(release_status=5),
   'no-ammo-consumption':lambda e:e.update(ammo=[3,3,3]),
   'extra-ammo-consumption':lambda e:e.update(ammo=[3,1,1]),
   'preloaded-shortcut':lambda e:e['initial'].update(projectiles=[dict(item='arrow',count=1)]*3),
   'missing-intermediate-full-menu':lambda e:e.update(menu_count=2),
   'emission-during-charge':lambda e:e.update(no_spawn_during_loading=False),
   'no-multishot':lambda e:e['loaded'].update(named_enchantments={'minecraft:unbreaking':3}),
  }
  for label,mutation in mutations.items():
   with self.subTest(label=label):
    c,e=fixture('offhand-natural');mutation(e)
    with self.assertRaises(AssertionError):p.validate_case(c,e)
 def test_surviving_short_salvo_and_survivor_break_rejected(self):
  for group in p.GROUPS:
   c,e=fixture(group,2,True)
   with self.assertRaises(AssertionError):p.validate_case(c,e)
   c,e=fixture(group,3,True);e['break_status']=[[48],[48]]
   with self.assertRaises(AssertionError):p.validate_case(c,e)
 def test_family_bonferroni_and_exact_fixed_binomial_bounds(self):
  d=p.read(p.BASE/'criteria.json');self.assertEqual(d['per_tail_alpha'],.001/48)
  for entry in d['categories'].values():
   q=entry['probability_hypothesis'];mass=[math.comb(128,k)*q**k*(1-q)**(128-k) for k in range(129)];lo,hi=entry['inclusive_count_bounds'];a=d['per_tail_alpha']
   self.assertLessEqual(sum(mass[:lo]),a);self.assertGreater(sum(mass[:lo+1]),a)
   self.assertLessEqual(sum(mass[hi+1:]),a);self.assertGreater(sum(mass[hi:]),a)
 def test_statistics_fixed_n_controls_and_known_bad_baseline(self):
  def rows(counts):
   labels=[label for label,n in zip(p.read(p.BASE/'criteria.json')['categories'],counts) for _ in range(n)];i=iter(labels)
   return [dict(trial=k,control=k in [0,1,130,131],category='plain_fresh_control' if k in [0,1,130,131] else next(i)) for k in range(132)]
  good=rows([32,24,18,54]);self.assertTrue(p.statistics(good)['statistical_pass']);self.assertFalse(p.statistics(rows([0,0,82,46]))['statistical_pass'])
  with self.assertRaises(AssertionError):p.statistics(good[:-1])
  good[0]['category']='three_arrows_survived'
  with self.assertRaises(AssertionError):p.statistics(good)
 def test_scenarios_have_only_natural_charge_not_commandloaded_shortcut(self):
  for group in p.GROUPS:
   s=p.scenario(group,R);self.assertEqual(len(p.cases(group)),132)
   self.assertEqual(sum(bool(a.get('use')) for a in s['actions']),132*(2 if 'natural' in group else 1))
   if 'natural' in group:
    self.assertFalse(any('charged_projectiles' in a.get('cmd','') for a in s['actions']));self.assertEqual(sum(bool(a.get('release')) for a in s['actions']),132)
   self.assertEqual(sum(a.get('cmd')=='kill @e[type=arrow]' for a in s['actions']),132)
 def test_original_entry_rejects_local_execution_before_any_output(self):
  import original_first
  from unittest.mock import patch
  with tempfile.TemporaryDirectory() as d:
   output=Path(d)/'must-not-exist'
   with patch.dict(os.environ,{'GITHUB_ACTIONS':'false'}):
    with self.assertRaises(AssertionError):original_first.run(output,Path(d)/'no-binary')
   self.assertFalse(output.exists())
 def test_original_entry_rejects_rerun_before_any_output(self):
  import original_first
  from unittest.mock import patch
  with tempfile.TemporaryDirectory() as d:
   output=Path(d)/'must-not-exist'
   with patch.dict(os.environ,{'GITHUB_ACTIONS':'true','GITHUB_RUN_ATTEMPT':'2'}):
    with self.assertRaises(AssertionError):original_first.run(output,Path(d)/'no-binary')
   self.assertFalse(output.exists())
 def test_lifecycle_requires_one_official_no_core_and_all_four_ports_free(self):
  import original_first
  with tempfile.TemporaryDirectory() as d:
   pth=Path(d)/'lifecycle.json'
   life=dict(cleanup_errors=[],ports_free={str(p):True for p in range(26190,26194)},servers=[dict(label='official',port=26190,argv=['java','-Xmx1G','-XX:ActiveProcessorCount=2','-jar','server.jar','nogui'])],cleanup=[dict(exit_code=0,group_gone=True)])
   pth.write_text(json.dumps(life));original_first.validate_lifecycle(pth)
   life['servers'][0]['label']='ours';pth.write_text(json.dumps(life))
   with self.assertRaises(AssertionError):original_first.validate_lifecycle(pth)
 def test_auxiliary_swap55_is_retained_as_open_mismatch(self):
  report=p.report_auxiliary(dict(auxiliary_swap55=[[],[dict(at=1.,status=55)]]),dict(auxiliary_swap55=[[],[]]))
  self.assertEqual(report['original_counts'],[0,1]);self.assertEqual(report['candidate_counts'],[0,0]);self.assertTrue(report['status'].startswith('OPEN'))
 def test_original_pin_cannot_be_replaced(self):
  with tempfile.TemporaryDirectory() as d:
   with self.assertRaises(AssertionError):p.freeze_original('offhand-natural',None,None,None,Path(d))
 def test_proposed_recorder_guard_is_explicit_original_only_github_and_fixed_name(self):
  tree=ast.parse((R/'tools/blackbox/record777.py').read_text());node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='allow_causal_original_reference');ns={};exec(compile(ast.Module(body=[node],type_ignores=[]),'own-guard','exec'),ns)
  good=dict(causal_menu_readiness=True,official_only=True,version=777,resume=False,core_only=False,parallel=False,scenarios=['crossbow-ub-offhand-natural-777'])
  guard=ns['allow_causal_original_reference'];self.assertTrue(guard(types.SimpleNamespace(**good),'true'))
  for key,value in [('causal_menu_readiness',False),('official_only',False),('version',775),('resume',True),('core_only',True),('parallel',True),('scenarios',[]),('scenarios',['pvp-thorns-control-777'])]:
   self.assertFalse(guard(types.SimpleNamespace(**dict(good,**{key:value})),'true'))
  self.assertFalse(guard(types.SimpleNamespace(**good),'false'))
if __name__=='__main__':unittest.main()
