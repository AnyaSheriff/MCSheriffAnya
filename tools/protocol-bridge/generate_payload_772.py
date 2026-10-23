#!/usr/bin/env python3
"""Отдельные таблицы772 из закреплённых JSON; живой допуск проверяется отдельно."""
import argparse,json,hashlib
from pathlib import Path
from generate_client_chain import HERE,load,locked
from generate_registry_26_2 import mapper,block_states
from generate_static_776 import direct_mapping


def generate(docs,baseline,client=772):
    old=docs[client]; newer=docs[773]
    if client==771:assert docs[771]['protocol']['types']==docs[772]['protocol']['types']
    a=old['protocol']['types'];b=newer['protocol']['types']
    shared=a['SlotComponent'][1][1]['type'][1]['fields'];target=b['SlotComponent'][1][1]['type'][1]['fields']
    changed={k for k in shared if shared[k]!=target.get(k)}
    assert changed=={'entity_data','block_entity_data','profile','lodestone_tracker'}
    assert all(a[k]==b[k] for k in ['Slot','UntrustedSlot','UntrustedSlotComponent'])
    text=f'// Закреплённые JSON{client}; generate_payload_{client}.py. Это не живое доказательство.\n'
    canonical=mapper(baseline['protocol']['types']['entityMetadataEntry'][1][1]['type']);serializers=mapper(a['entityMetadataEntry'][1][1]['type'])
    for name,x,y in [('SERIALIZERS_FROM',serializers,canonical),('SERIALIZERS_TO',canonical,serializers)]:text+=f'pub(crate) const {name}: &[u16] = &{list(direct_mapping(x,y))};\n'
    lookup={r['name']:r for r in old['entities']}
    text+='pub(crate) const LAYOUTS: &[(u32, &[&str])] = &[\n'
    for r in baseline['entities']:
        if r['name'] in lookup:text+=f'({r["id"]}, &{json.dumps(lookup[r["name"]]["metadataKeys"])}),\n'
    text+='];\n'
    text+='pub(crate) const ITEM_NAMES: &[&str] = &'+json.dumps([r['name'] for r in old['items']])+';\n'
    # Явные визуальные замены; серверные предметы/состояния не меняются.
    def replacement(name):
        if name in ('golden_dandelion','potted_golden_dandelion'):return name.replace('golden_','')
        if name=='music_disc_lava_chicken':return 'music_disc_13'
        if name=='iron_chain':return 'chain'
        if name=='copper_nugget':return 'iron_nugget'
        if name=='copper_golem_spawn_egg':return 'iron_golem_spawn_egg'
        if name.endswith('_shelf'):return 'bookshelf'
        if name=='netherite_horse_armor':return 'diamond_horse_armor'
        if name in ('nautilus_spawn_egg','zombie_nautilus_spawn_egg'):return 'squid_spawn_egg'
        if name=='camel_husk_spawn_egg':return 'camel_spawn_egg'
        if name=='parched_spawn_egg':return 'skeleton_spawn_egg'
        if name.endswith('_spear'):return name.replace('_spear','_sword').replace('copper_','iron_')
        if name.endswith('_nautilus_armor'):
            metal=name.removesuffix('_nautilus_armor');return ('diamond' if metal=='netherite' else 'iron' if metal=='copper' else metal)+'_horse_armor'
        if 'lightning_rod' in name:return 'lightning_rod'
        if 'copper_golem_statue' in name:return name.replace('copper_golem_statue','cut_copper')
        for end,sub in [('copper_bars','iron_bars'),('copper_chain','chain'),('copper_chest','chest'),('copper_lantern','lantern'),('copper_wall_torch','wall_torch'),('copper_torch','torch')]:
            if name.endswith(end):return sub
        if name.startswith('copper_'):return name.replace('copper_','iron_',1)
        raise AssertionError(name)
    lowitems={r['name']:r['id'] for r in old['items']};itemviews=[];policies={}
    for r in baseline['items']:
        if r['name'] not in lowitems:
            name=replacement(r['name']);assert name in lowitems,(r['name'],name)
            itemviews.append((r['id'],lowitems[name]));policies[r['name']]=name
    text+='pub(crate) const ITEM_VIEWS: &[(u32,u32)] = &'+str(itemviews)+';\n'
    canonical={r['name']:r['id'] for r in baseline['entities']};lowentities={r['name']:r['id'] for r in old['entities']}
    views={'nautilus':'squid','zombie_nautilus':'squid','camel_husk':'camel','parched':'skeleton','copper_golem':'iron_golem','mannequin':'armor_stand'}
    text+='pub(crate) const ENTITY_VIEWS: &[(u32,u32,u32)] = &'+str([(canonical[n],canonical[t],lowentities[t]) for n,t in views.items()])+';\n'
    lowblocks={r['name']:r for r in old['blocks']};lowstates=block_states(old['blocks']);highstates=block_states(baseline['blocks']);stateviews=[];blockviews=[]
    for r in baseline['blocks']:
        if r['name'] not in lowblocks:
            sub=replacement(r['name']);assert sub in lowblocks,(r['name'],sub)
            blockviews.append((r['id'],lowblocks[sub]['id']));policies[r['name']]=sub
    for (name,props),number in highstates.items():
        if (name,props) in lowstates:continue
        sub=policies.get(name,name);oldprops=dict(props)
        target=lowblocks[sub];allowed={s['name']:s for s in target['states']};newprops={k:v for k,v in oldprops.items() if k in allowed}
        # Статуи: четыре позы имеют четыре явно разных медных вида.
        if 'copper_golem_statue' in name:
            sub={'standing':'cut_copper','sitting':'exposed_cut_copper','running':'weathered_cut_copper','star':'oxidized_cut_copper'}[oldprops['copper_golem_pose']];newprops={}
        key=(sub,tuple(sorted(newprops.items())))
        if key not in lowstates:
            # Прежняя замена состояния dry_vegetation уже закреплена774.
            if name=='note_block' and oldprops['instrument'].startswith('trumpet'):newprops['instrument']='harp'
            key=(sub,tuple(sorted(newprops.items())))
        assert key in lowstates,(name,props,key)
        stateviews.append((number,lowstates[key]))
    # Единственное переименование chain→iron_chain: обратный ввод не является
    # восстановлением исчезнувшего медного предмета из его заменителя.
    highitems={r['name']:r['id'] for r in baseline['items']};highblocks={r['name']:r['id'] for r in baseline['blocks']}
    text+='pub(crate) const INPUT_ITEMS: &[(u32,u32)] = &'+str([(lowitems['chain'],highitems['iron_chain'])])+';\n'
    text+='pub(crate) const INPUT_BLOCKS: &[(u32,u32)] = &'+str([(lowblocks['chain']['id'],highblocks['iron_chain'])])+';\n'
    text+='pub(crate) const INPUT_STATES: &[(u32,u32)] = &'+str([(number,highstates[('iron_chain',props)]) for (name,props),number in lowstates.items() if name=='chain'])+';\n'
    assert all(rows==sorted(rows) for rows in [itemviews,blockviews,stateviews])
    text+='pub(crate) const REMOVED_BE_VIEWS: &[(u32,u32)] = &'+str([(40,lowblocks['bookshelf']['defaultState']),(48,lowblocks['cut_copper']['defaultState'])])+';\n'
    text+='pub(crate) const BLOCK_VIEWS: &[(u32,u32)] = &'+str(blockviews)+';\n'
    text+='pub(crate) const STATE_VIEWS: &[(u32,u32)] = &'+str(stateviews)+';\n'
    lowparticles=mapper(a['Particle'][1][0]['type']);highparticles=mapper(baseline['protocol']['types']['Particle'][1][0]['type'])
    particleviews={'copper_fire_flame':'flame','pause_mob_growth':'smoke','reset_mob_growth':'happy_villager'}
    assert set(particleviews)==highparticles.keys()-lowparticles.keys()
    text+='pub(crate) const PARTICLE_VIEWS: &[(u32,u32)] = &'+str([(highparticles[n],lowparticles[t]) for n,t in particleviews.items()])+';\n'
    report={'client':client,'changed_components':sorted(changed),'item_views':policies,'item_count':len(itemviews),'block_count':len(blockviews),'state_count':len(stateviews),'entity_views':views,'particle_views':particleviews,'live_admission':'separate checklist required'}
    return text,json.dumps(report,indent=2)+'\n'


def main():
    p=argparse.ArgumentParser();p.add_argument('--client',type=int,choices=(771,772),default=772);p.add_argument('--cache',type=Path,required=True);p.add_argument('--cache774',type=Path,required=True);p.add_argument('--baseline',type=Path,required=True);p.add_argument('--check',action='store_true');args=p.parse_args()
    docs=load(args.cache,args.cache774);locks=json.loads((HERE/'candidate-sources-26.3.json').read_text());baseline={}
    for name in ['protocol','entities']:
        filename=name+'-26.1.json';record=next(r for r in locks['files']+locks['baseline_files'] if r['file']==filename);baseline[name]=locked(args.baseline,filename,record)
    from generate_registry_26_2 import LOCAL_LOCKS
    for name in ['items','blocks']:
        path='tools/data/'+name+'.json';raw=(HERE.parents[1]/path).read_bytes();assert hashlib.sha256(raw).hexdigest()==LOCAL_LOCKS[path];baseline[name]=json.loads(raw)
    text,report=generate(docs,baseline,args.client)
    for path,content in [(HERE/f'src/generated_payload_{args.client}.rs',text),(HERE/f'payload-{args.client}-report.json',report)]:
        if args.check:assert path.read_text()==content,path
        else:path.write_text(content)
    print(report)
if __name__=='__main__':main()
