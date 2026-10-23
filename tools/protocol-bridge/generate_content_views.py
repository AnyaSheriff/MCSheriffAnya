#!/usr/bin/env python3
"""Явная таблица визуальных замен775 из закреплённых JSON."""
from pathlib import Path
import argparse,json,re
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--data",type=Path,required=True)
args=parser.parse_args()
root=Path('.')
p=root/'tools/protocol-bridge/src/content.rs';s=p.read_text()
rows=[]
for line in re.findall(r'(?:SlabDescriptor|ConcreteSlab) \{[^}]+\}',s.split('impl SlabDescriptor')[0]):
 if 'SlabDescriptor {' not in line and 'ConcreteSlab {' not in line: continue
 m=re.search(r'form: BlockForm::(\w+), name: "([^"]*)"',line)
 if not m or not re.search(r'input_name: "([^"]*)"',line): continue
 d=dict(re.findall(r'(\w+): (-?\d+)',line));d.update(form=m[1],name=m[2]);d['input_name']=re.search(r'input_name: "([^"]*)"',line)[1];rows.append(d)
bp=Path('tools/data/blocks.json');ip=Path('tools/data/items.json')
blocks={b['name']:b for b in json.loads(bp.read_text())};items={i['name']:i for i in json.loads(ip.read_text())}
newblocks={b['name']:b for b in json.loads((args.data/'blocks.json').read_text())}
def props(b,off):
 out={}
 for st in reversed(b['states']):
  vals=st.get('values', ['true','false'] if st['type']=='bool' else list(map(str,range(st['num_values']))))
  out[st['name']]=vals[off%len(vals)];off//=len(vals)
 return out
policy=[];generated=[]
for r in rows:
 name=r['name'];form=r['form'];base=r['input_name']
 if name.startswith('poplar_') or 'poplar_' in name or name.startswith('stripped_poplar'): base=base if base in blocks or base in items else name.replace('poplar','oak')
 elif form=='Slab':base='quartz_slab'
 elif form=='Stairs':base='quartz_stairs'
 elif form=='Wall':base='stone_brick_wall'
 elif name=='straw_bed':base='white_bed'
 elif name.endswith('_cushion'):base=name.replace('_cushion','_carpet')
 elif name.endswith('_map'):base='filled_map'
 elif name=='music_disc_bounce':base='music_disc_pigstep'
 elif name=='sulfur_cube_bucket':base='slime_ball'
 elif name=='sulfur_cube_spawn_egg':base='slime_spawn_egg'
 elif name=='shelf_mushroom':base='brown_mushroom'
 elif name=='red_shrub':base='dead_bush'
 elif form=='Spike':base='pointed_dripstone'
 else:base='stone'
 if name=='poplar_boat':base='oak_boat'
 if name=='poplar_chest_boat':base='oak_chest_boat'
 first=int(r['first_state']);last=int(r['last_state']);block=int(r['block']);item=int(r['item'])
 b=blocks.get(base);nb=newblocks.get(name);states=[]
 if block>=0:
  assert b and nb,(name,base)
  options=[props(b,i) for i in range(b['maxStateId']-b['minStateId']+1)]
  for off in range(last-first+1):
   pr=props(nb,off);matches=[i for i,v in enumerate(options) if all(pr.get(k)==val for k,val in v.items())]
   states.append(b['minStateId']+matches[0] if matches else b['defaultState'])
  # Все общие свойства формы сохранены; особые растения и мощная сера имеют явную потерю отображения.
  if form in ('Slab','Stairs','Wall','Wood'):assert len(states)==b['maxStateId']-b['minStateId']+1,(name,base)
 bi=b['id'] if b else 0;ii=items[base]['id'] if item>=0 else 0
 policy.append(dict(name=name,substitute=base,item=ii if item>=0 else None,block=bi if block>=0 else None,first_state=first,states=states))
 generated.append('    View { item: %s, block: %s, first: %s, last: %s, item775: %s, block775: %s, states: &[%s] },'%(item,block,first,last,ii,bi,','.join(map(str,states))))
Path('src/gameplay_catalog/client-775-views.json').write_text(json.dumps({'source':'tools/data/{blocks,items}.json; pinned candidate-json/blocks.json','policy':'Только клиентское отображение; обратное восстановление подлинности по заменителю запрещено. [проверить] визуальную геометрию растений и сущностей графическим клиентом.','entries':policy},ensure_ascii=False,separators=(',', ':'))+'\n')
Path('tools/protocol-bridge/src/content_views.rs').write_text('//! Явные визуальные замены775; источник: src/gameplay_catalog/client-775-views.json.\nuse crate::ids::RegistryKind;\nstruct View { item: i32, block: i32, first: i32, last: i32, item775: u32, block775: u32, states: &\'static [u32] }\nstatic VIEWS: &[View] = &[\n'+'\n'.join(generated)+'\n];\npub(crate) fn substitute(kind: RegistryKind, id: u32) -> Option<u32> {\n    if kind == RegistryKind::Entity { return match id { 157 => Some(90), 158 => Some(91), 159 => Some(1), 160 => Some(117), _ => None }; }\n    VIEWS.iter().find_map(|r| match kind {\n        RegistryKind::Item if r.item >= 0 && id == r.item as u32 => Some(r.item775),\n        RegistryKind::Block if r.block >= 0 && id == r.block as u32 => Some(r.block775),\n        RegistryKind::BlockState if r.block >= 0 && (r.first as u32..=r.last as u32).contains(&id) => r.states.get((id-r.first as u32) as usize).copied(),\n        _ => None,\n    })\n}\n')
# Сущности выбираются строго по закреплённым именам.
en={name:i for i,name in enumerate(re.findall(r'\("([^"]*)",',Path('tools/protocol-bridge/src/entity_layouts.rs').read_text()))}
p=Path('tools/protocol-bridge/src/content_views.rs');x=p.read_text();x=x.replace('157 => Some(90), 158 => Some(91), 159 => Some(1), 160 => Some(117)',f'157 => Some({en["oak_boat"]}), 158 => Some({en["oak_chest_boat"]}), 159 => Some({en["armor_stand"]}), 160 => Some({en["slime"]})');p.write_text(x)

p=Path("tools/protocol-bridge/src/content_views.rs");p.write_text(p.read_text()+'\n#[cfg(test)]\n#[path = "content_views_tests.rs"]\nmod tests;\n')

view_path=Path('src/gameplay_catalog/client-775-views.json')
policy=json.loads(view_path.read_text())
policy['entities']=[{'name':name,'canonical':canonical,'substitute':base,'id':en[base]} for name,canonical,base in [('poplar_boat',157,'oak_boat'),('poplar_chest_boat',158,'oak_chest_boat'),('cushion',159,'armor_stand'),('sulfur_cube',160,'slime')]]
view_path.write_text(json.dumps(policy,ensure_ascii=False,separators=(',',':'))+'\n')
