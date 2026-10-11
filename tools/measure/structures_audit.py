#!/usr/bin/env python3
"""Перепись мира 26.1.2 только чтением, через существующий разбор NBT.
Сервер, jar, протокол, наборы данных и шаблоны не используются.
"""
import argparse
import collections
import json
import math
import os
import struct
import zlib
from nether_census import nbt, unpack


def window(cx, cz):
    for gx in (-1, 0, 1):
        for gz in (-1, 0, 1):
            half = 64 if gx == gz == 0 else 32
            if gx*512-half <= cx < gx*512+half and gz*512-half <= cz < gz*512+half:
                return f'{gx},{gz}'
    return None


def records(folder, all_chunks=False):
    if not os.path.isdir(folder):
        return
    for filename in sorted(os.listdir(folder)):
        if not filename.endswith('.mca'):
            continue
        _, rx, rz, _ = filename.split('.')
        rx, rz = int(rx), int(rz)
        if not all_chunks and not any(window(rx*32+x, rz*32+z) for x in (0,31) for z in (0,31)):
            continue
        with open(os.path.join(folder, filename), 'rb') as f:
            data = f.read()
        for i in range(1024):
            cx, cz = rx*32+i%32, rz*32+i//32
            if not all_chunks and window(cx, cz) is None:
                continue
            offset = int.from_bytes(data[i*4:i*4+3], 'big')*4096
            if not offset:
                continue
            length = struct.unpack_from('>i', data, offset)[0]
            if data[offset+4] != 2:
                raise ValueError(f'unsupported compression: {filename}/{i}')
            raw = zlib.decompress(data[offset+5:offset+4+length])
            name_len = struct.unpack_from('>H', raw, 1)[0]
            root, _ = nbt(raw, 3+name_len, 10)
            yield cx, cz, root


def biome(root, x, y, z):
    for s in root.get('sections', []):
        sy = s['Y'] if s['Y'] < 128 else s['Y']-256
        if sy == y//16 and 'biomes' in s:
            b = s['biomes']; p = b['palette']
            ids = [0]*64 if len(p) == 1 else unpack(b['data'], max(1,(len(p)-1).bit_length()),64)
            return p[ids[(y%16//4)*16+(z%16//4)*4+x%16//4]].removeprefix('minecraft:')
    return 'unknown'


def inside(pos, box):
    return all(box[i] <= pos[i] <= box[i+3] for i in range(3))


def census(world, dimension, all_chunks=False):
    base = os.path.join(world,'dimensions/minecraft',dimension)
    starts, bes, entities = [], [], []
    chunks = collections.Counter(); biomes = collections.Counter()
    feature_blocks = collections.Counter(); feature_examples = {}
    for cx, cz, root in records(os.path.join(base,'region'), all_chunks):
        if root.get('Status') not in ('full','minecraft:full'):
            continue
        chunks[window(cx,cz) or 'outside'] += 1
        biomes[biome(root,cx*16+8,64 if dimension=='overworld' else 32,cz*16+8)] += 1
        for kind, start in root.get('structures',{}).get('starts',{}).items():
            if start.get('id') in ('INVALID','minecraft:invalid'):
                continue
            pieces = start.get('Children',[])
            boxes = [p['BB'] for p in pieces if 'BB' in p]
            if not boxes:
                continue
            box = [min(b[i] for b in boxes) for i in range(3)]+[max(b[i] for b in boxes) for i in range(3,6)]
            py = boxes[0][1]
            hm = root.get('Heightmaps',{}).get('OCEAN_FLOOR',[])
            floor = unpack(hm,9,256)[8*16+8] + (-64 if dimension=='overworld' else 0)-1 if hm else None
            names = collections.Counter(p.get('pool_element',{}).get('location',p.get('Template',p.get('id','unknown'))) for p in pieces)
            starts.append(dict(kind=kind.removeprefix('minecraft:'),chunk=[cx,cz],window=window(cx,cz),box=box,pieces=len(pieces),piece_types=dict(names),biome=biome(root,cx*16+8,py,cz*16+8),surface=floor,first_piece_floor=py,boxes=boxes))
        for be in root.get('block_entities',[]):
            bes.append({k:be[k] for k in ('id','x','y','z','LootTable','SpawnData') if k in be})
        for sec in root.get('sections',[]):
            bs=sec.get('block_states',{}); palette=bs.get('palette',[])
            names=[p['Name'].removeprefix('minecraft:') for p in palette]
            wanted={'budding_amethyst','bone_block','end_gateway'} & set(names)
            if not wanted: continue
            ids=[0]*4096 if len(names)==1 else unpack(bs['data'],max(4,(len(names)-1).bit_length()),4096)
            sy=sec['Y'] if sec['Y']<128 else sec['Y']-256
            for index,v in enumerate(ids):
                if names[v] in wanted:
                    feature_blocks[names[v]]+=1
                    feature_examples.setdefault(names[v],[cx*16+index%16,sy*16+index//256,cz*16+(index//16)%16])
    for _,_,root in records(os.path.join(base,'entities'), all_chunks):
        for e in root.get('Entities',[]):
            entities.append({'id':e['id'].removeprefix('minecraft:'),'pos':e.get('Pos',[]),'persistent':e.get('PersistenceRequired',0)})
    for s in starts:
        boxes=s.pop('boxes'); nearby=[be for be in bes if inside([be['x'],be['y'],be['z']],s['box'])]
        es=[e for e in entities if len(e['pos'])==3 and inside(e['pos'],s['box'])]
        s['block_entities']=dict(collections.Counter(be['id'].removeprefix('minecraft:') for be in nearby))
        s['loot_tables']=dict(collections.Counter(be['LootTable'] for be in nearby if 'LootTable' in be))
        s['entities']=dict(collections.Counter(e['id'] for e in es))
        s['complete_bounds']=all(window(x//16,z//16)==s['window'] for x in (s['box'][0],s['box'][3]) for z in (s['box'][2],s['box'][5]))
    return dict(dimension=dimension,chunks=dict(chunks),biomes_y64_or32=dict(biomes),starts=starts,feature_blocks=dict(feature_blocks),feature_examples=feature_examples,block_entities=bes,entity_totals=dict(collections.Counter(e['id'] for e in entities)))


def main():
    p=argparse.ArgumentParser(); p.add_argument('world'); p.add_argument('output'); p.add_argument('--dimension',default='overworld')
    p.add_argument('--all', action='store_true', help='Biased examples only; no area-frequency inference')
    args=p.parse_args()
    result=census(args.world,args.dimension,args.all)
    with open(args.output,'w') as f: json.dump(result,f,indent=2,sort_keys=True)
    print(args.dimension, sum(result['chunks'].values()), collections.Counter(s['kind'] for s in result['starts']), flush=True)

if __name__=='__main__': main()
