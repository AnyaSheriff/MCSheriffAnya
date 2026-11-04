#!/usr/bin/env python3
# Перепись только сохранённых region и собственных экспортов: биомы, берег, столбы, полосы.
import argparse, collections, csv, json, re
from pathlib import Path
import numpy as np
from scipy import ndimage as ndi
import biome_census as bc
import decoration_census_263 as dc

OCEANS={'ocean','deep_ocean','warm_ocean','lukewarm_ocean','deep_lukewarm_ocean','cold_ocean','deep_cold_ocean','frozen_ocean','deep_frozen_ocean'}
COLORS=['terracotta','orange_terracotta','yellow_terracotta','brown_terracotta','red_terracotta','white_terracotta','light_gray_terracotta']

GROUND=set(COLORS+['red_sand','red_sandstone','stone','sand','grass_block','dirt','coarse_dirt'])

def extract(region,output):
    count=collections.Counter();sites={}
    with output.open('w') as stream:
        out=csv.writer(stream);out.writerow(['x','z','biome','height'])
        for path in sorted(region.glob('*.mca')):
            for (x,z),(bio,h) in bc.chunk_cells(path).items():
                out.writerow([x*4+2,z*4+2,bio,h]);count[bio]+=1;sites.setdefault(bio,[x*4+2,z*4+2])
    print(json.dumps({'cells':dict(count),'first_sites':sites},indent=2))

def rows(path):
    with path.open() as stream:
        for row in csv.DictReader(stream):yield int(row['x']),int(row['z']),row['biome'],int(row['height'])

def ocean(path,mask_path,output,physical=False):
    data=list(rows(path));mask=list(rows(mask_path)) if mask_path else data
    points={(x//4,z//4) for x,z,_,_ in mask};x0=min(x for x,z in points);z0=min(z for x,z in points)
    shape=(max(z for x,z in points)-z0+1,max(x for x,z in points)-x0+1)
    names=np.full(shape,'?',dtype='U32');known=np.zeros(shape,dtype=bool);heights=np.full(shape,-999,dtype=np.int16)
    for x,z,b,h in data:
        x,z=x//4,z//4
        if (x,z) in points:names[z-z0,x-x0]=b;known[z-z0,x-x0]=True;heights[z-z0,x-x0]=h
    assert int(known.sum())==len(points),'export does not cover the full reference mask'
    ocean_mask=np.isin(names,list(OCEANS));land=known & ~ocean_mask & ~np.isin(names,['river','frozen_river'])
    if physical:
        assert not np.any(heights[known]==-999),'missing terrain heights'
        land=known & (heights>=63)
    distance=ndi.distance_transform_edt(~land)*4
    # Неизвестное за краем снимка не считается сушей. Такие расстояния цензурируются.
    boundary=ndi.distance_transform_edt(np.pad(known,1))[1:-1,1:-1]*4
    definition='Euclidean distance between 4x4 cell centres to '+('columns with floor height >=63' if physical else 'non-ocean, non-river surface-biome cells')+'; unknown edges censored. Resolution 4 blocks.'
    out={'definition':definition, 'known_cells':int(known.sum()),'ocean_cells':int(ocean_mask.sum()),'biomes':{}}
    for b in sorted(OCEANS):
        take=names==b;d=distance[take];edge=boundary[take];exact=d<edge
        hist={str(limit):int(((d<limit)&(edge>=limit)).sum()) for limit in [16,32,64,128,256]}
        # Для долей расстояний используем одну и ту же полосу с известным окружением 256 блоков.
        interior=take & (boundary>256);vals=distance[interior]
        out['biomes'][b]={'cells':int(take.sum()),'area_share_all':float(take.sum()/max(1,known.sum())),'area_share_ocean':float(take.sum()/max(1,ocean_mask.sum())),'on_dry_land_columns':int((take&(heights>=63)).sum()),'exact_distances':int(exact.sum()),'censored':int((~exact).sum()),'distance_quantiles_exact':np.quantile(d[exact],[0,.1,.5,.9,1]).tolist() if exact.any() else [],'halo256_cells':int(interior.sum()),'halo256_below':{str(limit):int((vals<limit).sum()) for limit in [16,32,64,128,256]},'below_with_known_radius':hist}
    out['physical_land']=physical
    output.write_text(json.dumps(out,indent=2)+'\n')

def own_registry():
    names=['?']*65536
    text=Path('src/blocks_table.rs').read_text().split('pub const BLOCK_NAMES:')[1].split('];')[0]
    for lo,hi,name in re.findall(r'\((\d+), (\d+), "([^"]+)"\)',text):names[int(lo):int(hi)+1]=[name]*(int(hi)-int(lo)+1)
    for name,lo,hi in re.findall(r'name: "([^"]+)"[^\n]*?first_state: (\d+), last_state: (\d+)',Path('tools/protocol-bridge/src/content.rs').read_text()):names[int(lo):int(hi)+1]=[name]*(int(hi)-int(lo)+1)
    text=Path('src/world/terrain.rs').read_text();bios=[]
    for key in ['ALL','NETHER','END','PREVIEW']:
        bios+=re.findall(r'Biome::(\w+),?',text.split('pub const '+key+':')[1].split('];')[0])
    bios=[re.sub(r'(?<!^)(?=[A-Z])','_',b).lower() for b in bios]+['?']*64
    return {'blocks':names,'biomes':bios,'properties':[{}]*65536}

def badlands(source,output,want='eroded_badlands',plan=None,columns=None,opening_size=33):
    selected=None
    if plan:
        p=json.loads(plan.read_text())
        selected=set(map(tuple,p.get('ours_badlands_chunks',[])))
        for x,z,w,d in p.get('ours_badlands_windows',[]):
            selected.update((cx,cz) for cx in range(x,x+w) for cz in range(z,z+d))
    roots=dc.roots(source,own_registry() if list(source.glob('*.mcs')) else None)
    chunks=[root for root in roots if selected is None or (root['xPos'],root['zPos']) in selected]
    heights={};biomes={};band_starts={};colors=collections.Counter();runs=[];transitions=[];deepest=[];band_columns=0;layouts={}
    for root in chunks:
        secs={s['Y']:s for s in root['sections']};cache={}
        def at(x,y,z):
            s=secs.get(y//16)
            return 'air' if not s else s['palette'][s['ids'][(y%16)*256+z*16+x]]
        for z in range(16):
            for x in range(16):
                h=next((y for y in range(319,31,-1) if at(x,y,z) in GROUND),31)
                s=secs.get(h//16);bio=s['biomes'][((h%16)//4)*16+(z//4)*4+x//4] if s else '?'
                p=(root['xPos']*16+x,root['zPos']*16+z);heights[p]=h;biomes[p]=bio
                if bio!=want:continue
                ceiling=next((y for y in range(h,max(31,h-8),-1) if at(x,y,z) in COLORS),h)
                seq=[at(x,y,z) for y in range(ceiling,max(31,ceiling-64),-1)];span=0
                for name in seq:
                    if name not in COLORS:break
                    span+=1;colors[name]+=1
                deepest.append(span);layouts[p]=seq[:span];band_starts[p]=ceiling
                if span>=16:band_columns+=1
                if span>=8:
                    change=0;run=1
                    for a,b in zip(seq[:span],seq[1:span]):
                        if a==b:run+=1
                        else:runs.append(run);run=1;change+=1
                    runs.append(run);transitions.append(change/(span-1)*32)
    x0=min(x for x,z in heights);z0=min(z for x,z in heights);shape=(max(z for x,z in heights)-z0+1,max(x for x,z in heights)-x0+1)
    assert shape[0]*shape[1]<=64_000_000,'measure distant windows separately'
    h=np.zeros(shape,dtype=np.int16);valid=np.zeros(shape,bool);eroded=np.zeros(shape,bool)
    for (x,z),v in heights.items():h[z-z0,x-x0]=v;valid[z-z0,x-x0]=True;eroded[z-z0,x-x0]=biomes[x,z]==want
    opening=ndi.grey_opening(h,size=(opening_size,opening_size));prom=h-opening
    eligible=eroded & (ndi.distance_transform_edt(np.pad(valid,1))[1:-1,1:-1]>32)
    result={'band_min_layers':8,'opening_size':opening_size,'biome':want,'definition':'Terracotta: first band within 8 blocks below terrain, contiguous depth capped at 64; run lengths and transitions use columns with >=8 contiguous layers. Hoodoo proxy: configurable morphological opening of terrain height (opening_size); 8-connected prominence>=12 components, 4..1024 columns, peak>=16. 32-block observed halo excludes truncated windows.','chunks':len(chunks),'eroded_columns':int(eroded.sum()),'eligible_columns':int(eligible.sum()),'height_quantiles':np.quantile(h[eligible],[0,.5,.9,.99,1]).tolist() if eligible.any() else [],'prominence_quantiles':np.quantile(prom[eligible],[0,.5,.9,.99,1]).tolist() if eligible.any() else [],'terracotta_color_counts':dict(colors),'terracotta_depth_quantiles':np.quantile(deepest,[0,.5,.9,.99,1]).tolist() if deepest else [],'band_columns_ge16':band_columns,'band_run_quantiles':np.quantile(runs,[0,.5,.9,.99,1]).tolist() if runs else [],'transitions_per32_quantiles':np.quantile(transitions,[0,.5,.9,1]).tolist() if transitions else []}
    labels,n=ndi.label((prom>=12)&eligible,structure=np.ones((3,3)))
    areas=np.bincount(labels.ravel());tops=ndi.maximum(prom,labels,index=np.arange(1,n+1)) if n else []
    keep=[i for i,top in enumerate(tops,1) if 4<=areas[i]<=1024 and top>=16]
    result['spires']={'count':len(keep),'count_per10000_columns':len(keep)*10000/max(1,int(eligible.sum())),'area_columns':int(sum(areas[i] for i in keep)),'area_share':sum(int(areas[i]) for i in keep)/max(1,int(eligible.sum())),'peak_prominences':[int(tops[i-1]) for i in keep],'areas':[int(areas[i]) for i in keep]}
    output.write_text(json.dumps(result,indent=2)+'\n')
    if columns:
        with columns.open('w') as stream:
            out=csv.writer(stream);out.writerow(['x','z','biome','height','terracotta_start_y','terracotta_top_down'])
            for (x,z),height in heights.items():out.writerow([x,z,biomes[x,z],height,band_starts.get((x,z),''),'/'.join(layouts.get((x,z),[]))])

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['extract','ocean','badlands']);p.add_argument('source',type=Path);p.add_argument('output',type=Path);p.add_argument('--mask',type=Path);p.add_argument('--physical',action='store_true');p.add_argument('--biome',default='eroded_badlands');p.add_argument('--plan',type=Path);p.add_argument('--columns',type=Path);p.add_argument('--opening',type=int,default=33);a=p.parse_args()
    if a.mode=='extract':extract(a.source,a.output)
    elif a.mode=='ocean':ocean(a.source,a.mask,a.output,a.physical)
    else:badlands(a.source,a.output,a.biome,a.plan,a.columns,a.opening)
