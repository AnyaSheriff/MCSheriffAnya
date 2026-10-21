#!/usr/bin/env python3
"""Strict selected wool-slab checks against fixed observations AND same-runner parity."""
import argparse,hashlib,io,json,math,struct
from pathlib import Path
from smoke import read_varint

COLORS=['white','orange','magenta','light_blue','yellow','lime','pink','gray','light_gray','cyan','purple','blue','brown','green','red','black']
PINS={'protocol.json':'13a10695aa1ed3604ed674ce8fdc8a759f48118714eab98644e685a3d1960727','blocks.json':'776312e51207352734304a293d9300f6c69b4efa40f00ac574c99d1ca7889873','items.json':'f0d0e447dd0311b7322f4548971d6ebf6074b5842fd4c7b4e3d8698e00b5edcb','entities.json':'33c2d7569e10cb98d86240e8135396702edb92c98e6ccb1c18bf6a481f22e3d5'}
JAR='33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'

def require(condition,where):
 if not condition:raise AssertionError(where)

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--official',type=Path,nargs='+',required=True);p.add_argument('--core',type=Path,nargs='+',required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 if a.output.exists():raise SystemExit('fresh comparison output required; previous evidence retained')
 output={'scope':'all16 selected wool recipes/states/place/mining; independent fixed expectations plus normalized same-runner comparison','result':'FAIL'}
 try:
  for name,h in PINS.items():require(hashlib.sha256((a.data/name).read_bytes()).hexdigest()==h,('pin',name))
  items=json.loads((a.data/'items.json').read_text());byname={r['name']:r['id'] for r in items};names={r['id']:r['name'] for r in items};blocks={r['name']:r for r in json.loads((a.data/'blocks.json').read_text())}
  def load(paths,side):
   combined={'recipes':{},'blocks':{}};reports=[]
   for path in paths:
    r=json.loads(path.read_text());require(r.get('result')=='OBSERVATION_COMPLETE',(side,path,'not complete'));require(r.get('server_exit')==0 and r.get('group_gone') is True and r.get('port_free') is True,(side,path,'cleanup'))
    require(r['plan']['input_pins']==PINS,(side,'pins'));require(r['plan']['colors']==COLORS,(side,'all colors'))
    if side=='official':require(r['plan']['jar_sha1']==JAR and r['plan'].get('binary_sha256') is None,(side,'target'))
    else:require(r['plan']['jar_sha1'] is None and len(r['plan'].get('binary_sha256') or '')==64,(side,'target'))
    for section in ['recipes','blocks']:
     require(not (set(combined[section]) & set(r[section])),(side,'duplicate phase',section));combined[section].update(r[section])
    for key in ['recipe_survival_mode_events','tools_survival_mode_events']:
     if r.get('recipes' if key.startswith('recipe') else 'blocks'):
      modes=r.get(key,[]);require(len(modes)>=2 and modes[-1]=={'mode':0,'payload_hex':'0300000000'} and {'mode':1,'payload_hex':'033f800000'} in modes,(side,key,'survival packet'))
    reports.append({'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'runner_sha256':r['runner_sha256'],'helper_sha256':r['own_helper_sha256'],'binary_sha256':r['plan'].get('binary_sha256')})
   require(set(combined['recipes'])==set(COLORS) and set(combined['blocks'])==set(COLORS),(side,'missing colors/phases'))
   return combined,reports
  official,oruns=load(a.official,'official');core,cruns=load(a.core,'core');runs=oruns+cruns
  require(len({r['runner_sha256'] for r in runs})==1,'different runner SHA; old discovery is not replay parity')
  require(all(r['helper_sha256']==runs[0]['helper_sha256'] for r in runs),'helper files changed across observations')
  def normalize(r,side):
   normalized={};group_ids=set();checks=0
   for color in COLORS:
    slab=color+'_wool_slab';wool=color+'_wool';case=r['recipes'][color];rows=case['selected_entries'];require(len(rows)==1,(side,color,'one shaped entry'))
    row=rows[0];display=row['display'];ingredient={'type':6,'set':{'items':[byname[wool]]}}
    expected_display={'type':1,'width':3,'height':1,'ingredients':[ingredient]*3,'result':{'type':5,'stack':{'item':slab,'count':6}},'station':{'type':4,'item':byname['crafting_table']}}
    require(display==expected_display,(side,color,'recipe display'));require(row['group_raw']>0,(side,color,'group present'));group_ids.add(row['group_raw'])
    require(row['category']==0 and row['requirements']==[{'items':[byname[wool]]}]*3 and row['flags']==3,(side,color,'book metadata'))
    natural=case['natural_family_entries'];require(len(natural)==1 and natural[0]['display']==expected_display and natural[0]['display_id']==row['display_id'],(side,color,'own natural unlock only'))
    named=case['candidate_named_checks'];require(len(named)==2 and named[0]['candidate']=='minecraft:'+slab and named[0]['accepted_selected_target'] is True and len(named[0]['selected_reaward'])==1 and named[0]['selected_reaward'][0]['display_id']==row['display_id'],(side,color,'canonical shaped name'))
    require(named[1]['candidate']=='minecraft:'+slab+'_from_'+wool+'_stonecutting' and named[1]['accepted_selected_target'] is False and not named[1]['selected_reaward'],(side,color,'cut name not awarded'))
    for emitted in [row,natural[0],named[0]['selected_reaward'][0]]:
     require(emitted['group_raw']==row['group_raw'] and emitted['category']==0 and emitted['requirements']==row['requirements'] and emitted['flags']==3 and emitted['replace'] is False,(side,color,'consistent natural/named metadata'))
    require(case['stonecut_declaration']['status']=='absent',(side,color,'cut absent; inconclusive is not absence'))
    ops=case['craft_operations'];require(len(ops)==1 and ops[0]['status']=='observed',(side,color,'manual operation'));op=ops[0]
    ready=op['ready'];taken=op['taken'];expected_slots=[{'item':slab,'count':6}]+[{'item':wool,'count':1}]*3+[None]*42
    require(ready['slots']==expected_slots and ready['cursor'] is None,(side,color,'ready exact input/output inventory'))
    require(taken['slots']==[None]*46 and taken['cursor']=={'item':slab,'count':6},(side,color,'exact consumption/cursor'))
    block=r['blocks'][color];source=blocks[slab];expected_states={half+'/'+wet:source['minStateId']+index for index,(half,wet) in enumerate((h,w) for h in ['top','bottom','double'] for w in ['true','false'])}
    require(set(block['states'])==set(expected_states),(side,color,'six states'))
    for key,value in expected_states.items():require(block['states'][key]['state']==value and block['states'][key]['position']==[2,65,0],(side,color,key,'exact known state'))
    place=block['placement'];require(place['updates'] and place['updates'][-1]['state']==expected_states['bottom/false'] and place['held'] is None,(side,color,'survival placement consumes one'))
    tools=block['tools'];require(len(tools)==6 and {(t['half'],t['tool']) for t in tools}=={(half,tool) for half in ['bottom','double'] for tool in ['hand','shears','diamond_pickaxe']},(side,color,'six tool cases'))
    normalized_tools=[]
    for t in tools:
     half,tool=t['half'],t['tool'];require(t['status']=='observed' and t['air_observed'] is True,(side,color,half,tool,'air actual, no unknown-as-air'))
     require(any(type(e.get('state')) is int and e.get('state')==0 and e.get('position')==[2,65,0] for e in t.get('block_updates',[])),(side,color,half,tool,'actual air event'))
     expected_count=2 if half=='double' else 1;drops={}
     require(len(t['drops'])==1,(side,color,half,tool,'one observed output entity, preserve stack distribution'))
     for entity,stack in t['drops'].items():
      require(type(stack.get('count')) is int and stack['count']>0,(side,color,half,tool,'positive drop count'))
      fresh=[e for e in t.get('spawns',[]) if str(e.get('entity'))==str(entity)]
      require(len(fresh)==1,(side,color,half,tool,'exactly one fresh spawn per drop ID'))
      position=fresh[0].get('position',[])
      require(len(position)==3 and all(type(v) in [int,float] and math.isfinite(v) for v in position) and math.dist(position,[2.5,65.5,.5])<=1.0,(side,color,half,tool,'fresh drop near target',position))
     for stack in t['drops'].values():
      require(stack['item'] in names,(side,color,'unknown drop item'));key=names[stack['item']];drops[key]=drops.get(key,0)+stack['count']
     require(drops=={slab:expected_count},(side,color,half,tool,'drop type/count',drops))
     if tool=='hand':
      require(t['held_before'] is None,(side,color,half,'bare hand actually empty'))
      require(t['held_after'] is None or t['held_after']=={'item':byname[slab],'count':expected_count},(side,color,half,'only own drop pickup permitted'))
      wear=None
     else:
      require(t['held_before'] in [{'item':byname[tool],'count':1},{'item':byname[tool],'count':1,'damage':0}],(side,color,half,tool,'fresh actual tool'))
      require(t['held_after']=={'item':byname[tool],'count':1,'damage':1},(side,color,half,tool,'wear1'))
      wear=1
     actions=[x for x in t['actions'] if x['name']=='block_dig'];require(len(actions)==2,(side,color,half,tool,'actions'))
     statuses=[read_varint(io.BytesIO(bytes.fromhex(x['payload_hex'])).read) for x in actions];require(statuses==[0,3],(side,color,half,tool,'observed start/finish route'))
     normalized_tools.append({'half':half,'tool':tool,'air':True,'drop':drops,'drop_stacks':[{'item':slab,'count':expected_count}],'wear':wear})
    normalized[color]={'display':display,'group':'same-wool-family','category':0,'requirements':row['requirements'],'flags':3,'canonical_shaped':'minecraft:'+slab,'cut':'absent','ready_slots':ready['slots'],'taken_cursor':taken['cursor'],'states':expected_states,'placed':expected_states['bottom/false'],'tools':sorted(normalized_tools,key=lambda x:(x['half'],x['tool']))}
    checks+=5+6+1+6
   require(len(group_ids)==1,(side,'one wool recipe grouping partition'))
   return normalized,checks
  on,ochecks=normalize(official,'official');cn,cchecks=normalize(core,'core');require(on==cn,'normalized selected behavior differs')
  output.update({'result':'PASS','official_independent_checks':ochecks,'core_independent_checks':cchecks,'colors':16,'recipes':16,'state_checks':96,'placement_checks':16,'tool_cases':96,'runs':runs,'normalized':on,'limits':['local display/group/entity/window/sequence numbers excluded across servers','bare-hand own-drop pickup timing excluded after verified unique drop metadata','no runtime concrete-vs-wool group comparison; only within-wool grouping checked','no exact break-time, GUI, full packet-order, wet mining, fire or sculk claim']})
 except BaseException as error:output['error']=repr(error)
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(output,ensure_ascii=False,indent=2)+'\n');print(json.dumps({k:output.get(k) for k in ['result','error','colors','official_independent_checks','core_independent_checks']}));return 0 if output['result']=='PASS' else 1
if __name__=='__main__':raise SystemExit(main())
