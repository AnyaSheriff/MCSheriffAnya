#!/usr/bin/env python3
"""Замеры сохранённого мира 26.1.2: без сервера, jar и наборов данных."""
import argparse
import collections
import gzip
import json
from pathlib import Path
from structures_audit2 import World, expected
from structures_audit import biome, window
from nether_census import unpack


def read(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_text())


def fossils(world, data, output, asset):
    w = World(world, 'the_nether')
    variants = collections.defaultdict(collections.Counter)
    examples = {}
    heights = collections.Counter()
    materials = collections.Counter()
    for s in data['starts']:
        if s['kind'] != 'nether_fossil' or not s['complete_bounds']:
            continue
        p = s['pieces'][0]
        box = p['box']
        heights[box[1]] += 1
        # NONE исключает неоднозначность осей; все повороты учитываются в составе.
        cells = []
        for y in range(box[1], box[4]+1):
            for z in range(box[2], box[5]+1):
                for x in range(box[0], box[3]+1):
                    name = w.block(x,y,z)
                    if name in ('bone_block', 'coal_ore', 'coal_block'):
                        materials[name] += 1
                        root = w.chunk(x//16,z//16)
                        sec = next(t for t in root['sections'] if t['Y'] == y//16)
                        bs = sec['block_states']; pal = bs['palette']
                        ids = [0]*4096 if len(pal)==1 else unpack(bs['data'],max(4,(len(pal)-1).bit_length()),4096)
                        state = pal[ids[y%16*256+z%16*16+x%16]]
                        props = state.get('Properties',{})
                        text = name + ('['+','.join(f'{k}={v}' for k,v in sorted(props.items()))+']' if props else '')
                        cells.append((x-box[0],y-box[1],z-box[2],text))
        if p['rotation'] == 'NONE':
            key = (box[3]-box[0]+1,box[5]-box[2]+1,box[4]-box[1]+1,tuple(cells))
            variants[p['name']][key] += 1
            examples[p['name'],key] = s['chunk']
    lines = ['# Замер vanilla-struct/world 26.1.2: модальные целые образцы NONE, только кости.']
    summary = {}
    for name, choices in sorted(variants.items()):
        (width,length,height,cells), count = choices.most_common(1)[0]
        texts = sorted({c[3] for c in cells}); symbols = dict(zip(texts,'ABCDEFGHIJKLMNOPQRSTUVWXYZ'))
        lines += [f'template {name.split("/")[-1]} {width} {length} {height}', *[f'  {symbols[t]} {t}' for t in texts]]
        grid = {(x,y,z): symbols[t] for x,y,z,t in cells}
        for y in range(height):
            lines += [f'layer {y}', *[''.join(grid.get((x,y,z),'.') for x in range(width)) for z in range(length)]]
        summary[name] = dict(cells=len(cells), agreement=count, specimens=sum(choices.values()), distinct=len(choices), chunk=examples[name,(width,length,height,cells)])
    assert len(summary) == 14
    asset.write_text('\n'.join(lines)+'\n')
    output.write_text(json.dumps(dict(variants=summary,heights=heights,materials=materials),indent=2)+'\n')


def exposure(world, output):
    w = World(world,'overworld')
    counts = collections.defaultdict(lambda: collections.Counter())
    cave = collections.defaultdict(lambda: collections.Counter())
    for cx,cz in sorted(expected(),key=lambda p:(p[0]//32,p[1]//32,p[1],p[0])):
        r = w.chunk(cx,cz)
        if r is None: raise ValueError((cx,cz))
        # Одинаковая сетка 4×4 блоков в обоих мирах, на Y64 и Y−51.
        for dx in (2,6,10,14):
            for dz in (2,6,10,14):
                counts[window(cx,cz)][biome(r,cx*16+dx,64,cz*16+dz)] += 16
                cave[window(cx,cz)][biome(r,cx*16+dx,-51,cz*16+dz)] += 16
    output.write_text(json.dumps(dict(y64=counts,y_minus51=cave),indent=2)+'\n')


def inhabitants(data, output):
    rows=[]
    for s in data['starts']:
        if not s['kind'].startswith('village_') or s['window'] is None:continue
        entities=s['entities']; villagers=[e for e in entities if e['id']=='minecraft:villager']
        rows.append(dict(kind=s['kind'],chunk=s['chunk'],entities=collections.Counter(e['id'] for e in entities),
                         villagers=[{k:e.get(k) for k in ('VillagerData','Age','PersistenceRequired','piece_membership')} for e in villagers],
                         loot=[dict(table=b['LootTable'],pieces=[p['name'] for p in s['pieces'] if all(p['box'][i]<=b[k]<=p['box'][i+3] for i,k in enumerate(('x','y','z')))]) for b in s['block_entities'] if 'LootTable' in b]))
    output.write_text(json.dumps(rows,indent=2)+'\n')


def main():
    p=argparse.ArgumentParser();p.add_argument('mode',choices=['fossils','exposure','inhabitants']);p.add_argument('world',type=Path);p.add_argument('data',type=Path);p.add_argument('output',type=Path);p.add_argument('--asset',type=Path)
    a=p.parse_args()
    if a.mode=='fossils': fossils(a.world,read(a.data/'vanilla-the_nether.json.gz'),a.output,a.asset)
    elif a.mode=='exposure':exposure(a.world,a.output)
    else:inhabitants(read(a.data/'vanilla-overworld.json.gz'),a.output)

if __name__=='__main__': main()
