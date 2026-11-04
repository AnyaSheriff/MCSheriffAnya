"""Synthetic raw-packet negative controls; no sockets, pinned tables or servers."""
import copy,unittest
from compare_wool_sculk_777 import review,normalized,provenance
from probe_wool_sculk_777 import TYPES,LINES,pack_position,varint,HASHES,JAR_SHA1
class EvidenceReviewTests(unittest.TestCase):
 def report(self):
  self.states={};cases=[];controls=[]
  def event(pos,name,properties):
   decoded=dict(name=name,properties=properties);state=next((i for i,v in self.states.items() if v==decoded),len(self.states)+1);self.states[state]=decoded
   return dict(name='block_change',position=pos,state=state,payload_hex=(pack_position(pos)+varint(state)).hex(),at=1.)
  def case(label,kind,active,source,sensor,final,**extra):
   events=[] if final is None else [event(source,final['name'],final['properties'])]
   if active:events.append(event(sensor,'sculk_sensor',dict(sculk_sensor_phase='active',power='8',waterlogged='false')))
   activations=[dict(e,properties=self.states[e['state']]['properties']) for e in events if e['position']==sensor]
   updates=[dict(e,decoded=self.states[e['state']]) for e in events if e['position']==source]
   return dict(label=label,kind=kind,active=active,activations=activations,source=source,sensor=sensor,selected_events=events,final_source_updates=updates,initial='fixture',observation_seconds=5.5 if extra.get('action')=='break' else 3.5,**extra)
  for material in TYPES:
   for action in ['place','break']:
    label='emission/'+material['label']+'/'+action;active=material['label']=='stone'
    if action=='place':final=dict(name=material['block'],properties={} if 'half' not in material else dict(type=material['half'],waterlogged=str(material['wet'] and material['half']!='double').lower()))
    else:final=dict(name='water' if material.get('wet') else 'air',properties=dict(level='0') if material.get('wet') else {})
    cases.append(case(label,'emission',active,[2,65,0],[6,65,0],final,material=material,action=action))
    if material['label'] in ['stone','full-wool']:controls.append(dict(label=label,matched=True,expected_active=active,observed_active=active))
  for line in LINES:
   for barrier in [dict(label='air',block='air')]+TYPES+[dict(label='no-event',block='air')]:
    label='occlusion/'+line['name']+'/'+barrier['label'];active=barrier['label'] not in ['full-wool','no-event'];final=None if barrier['label']=='no-event' else dict(name='lever',properties=dict(face='floor',facing='north',powered='true'))
    cases.append(case(label,'occlusion',active,line['source'],line['sensor'],final,barrier=barrier,geometry=line))
    if barrier['label'] in ['air','stone','full-wool','no-event']:controls.append(dict(label=label,matched=True,expected_active=active,observed_active=active))
  return dict(result='OBSERVED',server_exit=0,group_gone=True,port_free=True,survival_confirmed_by_packet=True,mode_events=[dict(mode=0)],cases=cases,controls=controls)
 def check(self,report):return review(report,self.states)
 def test_wrong_or_missing_provenance_rejected(self):
  helper={name:'a'*64 for name in ['probe_gameplay.py','probe_white_concrete_slab.py','generate_tables.py','smoke.py','observe.py']}
  official=dict(jar_sha1=JAR_SHA1,binary_sha256=None,pins=HASHES,runner_sha256='b'*64,helper_sha256=helper)
  core=dict(official,jar_sha1=None,binary_sha256='c'*64);provenance(official,core,'c'*64)
  for field in ['jar','binary','pins','runner','helperkeys','helperempty']:
   off=copy.deepcopy(official);ours=copy.deepcopy(core)
   if field=='jar':off['jar_sha1']='0'*40
   elif field=='binary':ours['binary_sha256']='d'*64
   elif field=='pins':off['pins']['blocks.json']='e'*64
   elif field=='runner':off['runner_sha256']=ours['runner_sha256']=''
   elif field=='helperkeys':off['helper_sha256'].pop('observe.py');ours['helper_sha256'].pop('observe.py')
   else:off['helper_sha256']['observe.py']=ours['helper_sha256']['observe.py']=''
   with self.assertRaises(AssertionError):provenance(off,ours,'c'*64)
 def test_good_controls_are_required_before_slab_inference(self):
  good=self.report();self.assertEqual(len(self.check(good)),36)
  for label in ['emission/stone/place','occlusion/upper/air','occlusion/lower/no-event']:
   bad=copy.deepcopy(good);case=next(c for c in bad['cases'] if c['label']==label);case['active']=not case['active']
   with self.assertRaises(AssertionError):self.check(bad)
 def test_invalid_sensor_mode_cleanup_missing_raw_event_rejected(self):
  for change in ['power','mode','group','case','sensor-position','missing-source','forged-activation','raw-state']:
   bad=self.report()
   if change=='power':
    sensor=bad['cases'][0]['activations'][0];sensor['properties']['power']='0';self.states[sensor['state']]['properties']['power']='0'
   elif change=='mode':bad['mode_events'][-1]['mode']=1
   elif change=='group':bad['group_gone']=False
   elif change=='case':bad['cases'][-1]['label']='invented-case'
   elif change=='sensor-position':bad['cases'][0]['sensor']=[7,65,0]
   elif change=='missing-source':bad['cases'][4]['selected_events']=[];bad['cases'][4]['final_source_updates']=[]
   elif change=='forged-activation':bad['cases'][0]['selected_events']=bad['cases'][0]['selected_events'][:1]
   else:bad['cases'][0]['selected_events'][0]['state']=999
   with self.assertRaises(AssertionError):self.check(bad)
 def test_wrong_final_shape_water_name_and_break_replacement_rejected_independently(self):
  for label,field,value in [('emission/top-true/place','type','bottom'),('emission/top-true/place','waterlogged','false'),('emission/double-true/place','waterlogged','true'),('emission/top-false/place','name','stone'),('emission/top-true/break','name','air')]:
   bad=self.report();case=next(c for c in bad['cases'] if c['label']==label);final=case['final_source_updates'][-1];state=final['state']
   # Mutate both summary and supplied state interpretation: semantic checks must still reject.
   if field=='name':self.states[state]['name']=value
   else:self.states[state]['properties'][field]=value
   final['decoded']=copy.deepcopy(self.states[state])
   with self.assertRaises(AssertionError):self.check(bad)
 def test_normalization_preserves_final_shape_water_ignores_packet_format_time(self):
  good=self.report();case=next(c for c in good['cases'] if c['label']=='emission/top-true/place');same=copy.deepcopy(case);same['final_source_updates'][0]['payload_hex']='different-packet';same['at']=99
  self.assertEqual(normalized(case),normalized(same));same['final_source_updates'][0]['decoded']['properties']['waterlogged']='false';self.assertNotEqual(normalized(case),normalized(same))
if __name__=='__main__':unittest.main()
