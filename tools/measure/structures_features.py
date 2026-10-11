#!/usr/bin/env python3
"""Замер фич чтением миров с выключенными строениями."""
import collections
import json
import sys
from village import World
from structures_audit import biome, unpack


def main():
    world, census, output = sys.argv[1:]
    w=World(world); data=json.load(open(census)); out=[]
    for be in data['block_entities']:
        if be['id'] not in ('minecraft:mob_spawner','minecraft:brushable_block'): continue
        x,y,z=be['x'],be['y'],be['z']; root=w.chunk(x//16,z//16)
        hm=root.get('Heightmaps',{}).get('OCEAN_FLOOR')
        surface=unpack(hm,9,256)[(z%16)*16+x%16]-65 if hm else None
        floor=w.block(x,y-1,z)
        out.append(dict(pos=[x,y,z],kind=be['id'],entity=be.get('SpawnData',{}).get('entity',{}).get('id'),biome=biome(root,x,y,z),surface=surface,depth=surface-y+1 if surface is not None else None,floor=floor,below_floor=w.block(x,y-2,z),loot=be.get('LootTable')))
    with open(output,'w') as f: json.dump(out,f,indent=2,sort_keys=True)
    print('records',len(out),collections.Counter(r['entity'] for r in out))

if __name__=='__main__': main()
