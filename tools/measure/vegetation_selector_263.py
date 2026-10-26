#!/usr/bin/env python3
"""Опад и полочные грибы изолированных деревьев естественного селектора.

Только чтение сохранённых region. Площадки созданы обычными командами
vegetation_plots_263.py --dappled-plots; содержимое архивов не требуется.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from census_263 import chunks as nbt_chunks, unpack
from genaudit_263_io import Reader, level, region_dir


def measure(world, plots, seed, plot_name='dappled_selector'):
    metadata = level(world)
    if metadata['seed'] != seed or not isinstance(metadata['version'], dict):
        raise ValueError('Нужен официальный мир с подтверждённым семенем')
    plot = plots[plot_name]
    bottom = plot['base_y']
    wanted = {(px//16, pz//16) for x, z in plot['points']
              for px in range(x-9, x+10) for pz in range(z-9, z+10)}
    reader = Reader()
    chunks, files, shelf_states = {}, [], {}
    for file in sorted(region_dir(world, 'overworld').glob('*.mca')):
        rx, rz = map(int, file.stem.split('.')[1:])
        if not any(x//32 == rx and z//32 == rz for x, z in wanted):
            continue
        files.append([file.name, hashlib.sha256(file.read_bytes()).hexdigest()])
        for cx, cz, blocks, _, _ in reader.read(file, wanted):
            chunks[cx, cz] = blocks
        for root in nbt_chunks(file):
            cx,cz = root['xPos'],root['zPos']
            if (cx,cz) not in wanted:
                continue
            for section in root.get('sections',[]):
                if section['Y']*16+15 < bottom:
                    continue
                container = section.get('block_states')
                if not container:
                    continue
                palette = container['palette']
                states = {}
                for index,block in enumerate(palette):
                    if isinstance(block,str):
                        block_name = block.split('[')[0]
                        properties = dict(v.split('=') for v in block.split('[')[1].rstrip(']').split(',')) if '[' in block else {}
                    else:
                        block_name = block.get('id',block.get('Name',block.get('','')))
                        properties = block.get('properties',block.get('Properties',{}))
                    if block_name.removeprefix('minecraft:') == 'shelf_mushroom':
                        states[index] = properties
                if not states:
                    continue
                ids = unpack(container['data'],max(4,(len(palette)-1).bit_length()),4096) if len(palette)>1 else [0]*4096
                for index,state in enumerate(ids):
                    if state in states:
                        shelf_states[cx*16+(index&15),section['Y']*16+(index>>8),cz*16+((index>>4)&15)] = states[state]
    if chunks.keys() != wanted:
        raise ValueError(f'Неполное покрытие: {len(chunks)}/{len(wanted)}')

    def name(x, y, z):
        return reader.names[int(chunks[x//16, z//16][y+64, z%16, x%16])]

    observations = []
    for x, z in plot['points']:
        counts = Counter()
        litter, shelf = [], []
        shelf_support = []
        base = name(x, bottom, z)
        trunk = 0
        while trunk < 31 and base.endswith('_log') and name(x, bottom+trunk, z) == base:
            trunk += 1
        for dx in range(-9, 10):
            for dz in range(-9, 10):
                for dy in range(31):
                    block = name(x+dx, bottom+dy, z+dz)
                    if block.endswith(('_log', '_leaves')) or block in ('leaf_litter', 'shelf_mushroom'):
                        counts[block] += 1
                    if block == 'leaf_litter':
                        litter.append([dx, dy, dz])
                    if block == 'shelf_mushroom':
                        shelf.append([dx, dy, dz])
                        supports = [(sx,sz) for sx,sz in ((0,1),(0,-1),(1,0),(-1,0))
                                    if name(x+dx+sx,bottom+dy,z+dz+sz).endswith('_log')]
                        shelf_support.append([dx,dy,dz,supports])
        observations.append({'origin': [x, bottom, z], 'base': base, 'trunk_height':trunk,
                             'counts': dict(counts), 'litter': litter, 'shelf': shelf,
                             'shelf_log_support':shelf_support,
                             'shelf_states':[[dx,dy,dz,shelf_states[x+dx,bottom+dy,z+dz]] for dx,dy,dz in shelf]})
    return {'seed': seed, 'level': metadata, 'files': files, 'plots': plots,
            'method': f'{len(plot["points"])} isolated {plot_name} placements on grass at y=200; zero random ticks; read-only region observations',
            'bases': dict(Counter(p['base'] for p in observations)), 'observations': observations}


def shelf_models(result):
    faces = {'north':0,'south':1,'west':2,'east':3}
    # Строковая запись состояния без свойств означает умолчания блока:
    # minecraft.wiki/w/Shelf_Mushroom — age=0, facing=north.
    return [[[x,y+1,z,int(state.get('age',0)),faces[state.get('facing','north')]] for x,y,z,state in p['shelf_states']]
            for p in result['observations'] if p['base']=='poplar_log' and p['trunk_height']>1]


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world', type=Path)
    parser.add_argument('--plots', type=Path, required=True)
    parser.add_argument('--seed', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--models', type=Path)
    parser.add_argument('--plot-name',default='dappled_selector',choices=['dappled_selector','spruce_configured'])
    args = parser.parse_args()
    result = measure(args.world, json.loads(args.plots.read_text()), args.seed,args.plot_name)
    args.output.write_text(json.dumps(result,separators=(',',':'))+'\n')
    if args.models:
        args.models.write_text(json.dumps(shelf_models(result),separators=(',',':'))+'\n')
    print(result['bases'])
    for base in result['bases']:
        rows = [p for p in result['observations'] if p['base'] == base]
        print(base, len(rows), 'litter', [len(p['litter']) for p in rows],
              'shelf', [len(p['shelf']) for p in rows])
