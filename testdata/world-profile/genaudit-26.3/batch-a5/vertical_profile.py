#!/usr/bin/env python3
"""Профили готовых регионов по биому поверхности; архивы игры не читаются."""
import sys,json,argparse
from pathlib import Path
from collections import Counter,defaultdict
import numpy as np
from scipy import ndimage
sys.path.insert(0,str(Path(__file__).resolve().parents[4]/'tools/measure'))
from genaudit_263_io import Reader,region_dir
from genaudit_263 import natural,requested
HIGH=['jagged_peaks','frozen_peaks','stony_peaks','windswept_hills','windswept_gravelly_hills','windswept_forest','windswept_savanna','beach']
def profile(world,plan,output):
 r=Reader();want=requested(plan,'overworld');heights=defaultdict(list);rows={};examples=defaultdict(list);seen=set();n=0
 if not want:raise ValueError('Нужен непустой план выборки')
 for f in sorted(region_dir(world,'overworld').glob('*.mc*')):
  rx,rz=map(int,f.stem.split('.')[1:])
  if want is not None and not any(x//32==rx and z//32==rz for x,z in want):continue
  for cx,cz,blocks,bios,_ in r.read(f,want):
   if (cx,cz) in seen:raise ValueError(f'Повтор чанка {cx},{cz}')
   seen.add((cx,cz));n+=1
   ground=np.asarray([natural(name) for name in r.names])[blocks]
   top=383-ground[::-1].argmax(axis=0)
   bi=np.asarray(bios).reshape(96,4,4);zz,xx=np.indices(top.shape);bio=bi[top//4,zz//4,xx//4]
   air=np.asarray([name in ('air','cave_air','void_air') for name in r.names])[blocks]
   solid=~np.asarray([name in ('air','cave_air','void_air','water','lava','bubble_column') for name in r.names])[blocks]
   # Только связь с верхней гранью чанка: закрытые пещеры не считаются нависаниями.
   labels,_=ndimage.label(air[127:])
   sky_labels=np.unique(labels[-1]);sky_labels=sky_labels[sky_labels!=0]
   sky_air=np.isin(labels,sky_labels)
   for name in HIGH:
    mask=bio==name;count=int(mask.sum())
    if not count:continue
    heights[name].extend((top[mask]-64).tolist())
    row=rows.setdefault(name,{'columns':0,'solid_by_y':{},'air_below_top_by_y':{},'overhang_columns_above_sea':0,'sky_connected_overhang_columns_above_sea':0,'upper_runs':[]})
    row['columns']+=count
    for y in range(64,241,8):
     a=row['solid_by_y'].setdefault(str(y),[0,0]);a[0]+=int(solid[y+64][mask].sum());a[1]+=count
    for band in range(112,208,16):
     below=(np.arange(band+64,band+80)[:,None,None]<top[None,:,:]-5)&mask[None,:,:]
     a=row['air_below_top_by_y'].setdefault(str(band),[0,0]);a[0]+=int((air[band+64:band+80]&below).sum());a[1]+=int(below.sum())
    below=(np.arange(127,384)[:,None,None]<top[None,:,:]-5)&mask[None,:,:]
    row['overhang_columns_above_sea']+=int(np.any(air[127:]&below,axis=0).sum())
    row['sky_connected_overhang_columns_above_sea']+=int(np.any(sky_air&below,axis=0).sum())
    for z,x in np.argwhere(mask&(top>=224)):
     y=int(top[z,x]);start=y
     while start>127 and solid[start-1,z,x]:start-=1
     row['upper_runs'].append([start-64,y-64])
     if len(examples[name])<12:examples[name].append([cx*16+int(x),y-64,cz*16+int(z),start-64])
 if seen!=want:raise ValueError(f'Неполная выборка: {n} / {len(want) if want is not None else None}')
 for name,row in rows.items():
  row['height_quantiles_min_p50_p95_p99_max']=np.percentile(heights[name],[0,50,95,99,100]).tolist()
  v=row.pop('upper_runs');row['upper_run_count_above_y160']=len(v)
  row['upper_run_thickness_quantiles_min_p50_p95_max']=np.percentile([b-a+1 for a,b in v],[0,50,95,100]).tolist() if v else None
  row['examples']=examples[name]
 result={'chunks':n,'definition':'Surface biome of highest natural block; solid excludes air/water/lava. Raw gap proxy: any air above sea at least six below top, including enclosed caves. Sky-connected overhangs additionally require a six-neighbour air path to Y319 inside the chunk; paths crossing chunk faces are not followed, so frequency is a lower bound. Profiles aggregate per biome, never require matching positions. Bands match unchanged census. Upper-run thickness is truncated at Y63.','biomes':rows}
 Path(output).write_text(json.dumps(result,indent=2)+'\n');print(output,n,{k:v['height_quantiles_min_p50_p95_p99_max'] for k,v in rows.items()},flush=True)
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('world',type=Path)
 parser.add_argument('--plan',required=True,type=Path)
 parser.add_argument('--output',required=True,type=Path)
 args=parser.parse_args()
 profile(args.world,json.loads(args.plan.read_text()),args.output)
