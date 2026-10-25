#!/usr/bin/env python3
"""Compare two saved 26.3 worlds using only read-only region measurements.

Example (numpy and scipy required):
  python tools/measure/genaudit_263.py OUR_WORLD VANILLA_WORLD --plan PLAN.json --output REPORT

Writes REPORT.json and REPORT.md. Optional --cache stores resumable per-side
measurements outside the repository. Equal seeds and equal chunk coverage are
required. Missing coverage is an error unless --allow-incomplete is explicit.
Tolerances are audit triage thresholds, not a claim of statistical equivalence.
No game archives, code, data packs or network protocol implementations are read.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
import time

import numpy as np
from scipy import ndimage
from scipy.spatial import cKDTree

from genaudit_263_io import Reader, level, manifest, region_dir

SCHEMA = 1
DIMS = ('overworld', 'the_nether', 'the_end')
AIR = {'air', 'cave_air', 'void_air'}
FLUID = {'water', 'lava', 'bubble_column'}
GROUND = {'grass_block', 'dirt', 'coarse_dirt', 'rooted_dirt', 'podzol', 'mycelium',
          'mud', 'clay', 'sand', 'red_sand', 'sandstone', 'red_sandstone', 'gravel',
          'stone', 'granite', 'diorite', 'andesite', 'deepslate', 'tuff', 'calcite',
          'terracotta', 'snow_block', 'powder_snow', 'ice', 'packed_ice', 'blue_ice',
          'moss_block', 'pale_moss_block', 'netherrack', 'soul_sand', 'soul_soil',
          'basalt', 'blackstone', 'end_stone', 'bedrock', 'obsidian', 'sulfur', 'cinnabar'}
WATCH = {'short_grass', 'tall_grass', 'fern', 'large_fern', 'dead_bush', 'bush',
         'firefly_bush', 'leaf_litter', 'wildflowers', 'pink_petals', 'dandelion',
         'poppy', 'blue_orchid', 'allium', 'azure_bluet', 'oxeye_daisy', 'cornflower',
         'lily_of_the_valley', 'torchflower', 'wither_rose', 'sunflower', 'lilac',
         'rose_bush', 'peony', 'brown_mushroom', 'red_mushroom', 'sugar_cane',
         'pumpkin', 'melon', 'cactus', 'bamboo', 'kelp', 'kelp_plant', 'seagrass',
         'tall_seagrass', 'sea_pickle', 'chorus_plant', 'chorus_flower', 'spore_blossom',
         'hanging_roots', 'small_dripleaf', 'big_dripleaf', 'cave_vines',
         'cave_vines_plant', 'pointed_dripstone', 'dripstone_block', 'sculk',
         'sculk_sensor', 'sculk_shrieker', 'amethyst_block', 'budding_amethyst',
         'shelf_mushroom', 'red_shrub', 'campfire', 'soul_campfire', 'creaking_heart',
         'sulfur', 'potent_sulfur', 'cinnabar', 'bone_block', 'spawner', 'trial_spawner',
         'snow','snow_block','powder_snow','ice','packed_ice','blue_ice','mossy_cobblestone'}


def quantiles(values):
    values = np.asarray(values)
    return None if not values.size else {str(q): round(float(np.percentile(values, q)), 5)
                                        for q in (0, 5, 25, 50, 75, 95, 99, 100)}


def hist(values):
    keys, counts = np.unique(values, return_counts=True)
    return {str(int(k)): int(n) for k, n in zip(keys, counts)}


def add(counter, values, names=None):
    keys, counts = np.unique(values, return_counts=True)
    counter.update({names[int(k)] if names is not None else str(k): int(n)
                    for k, n in zip(keys, counts)})


def natural(name):
    return name in GROUND or name.endswith(('_ore', '_terracotta')) or name.startswith('raw_')


def feature(name):
    return (name in WATCH or name.endswith(('_log', '_wood', '_stem', '_hyphae', '_leaves'))
            or any(s in name for s in ('coral', 'mushroom', 'poplar', 'shrub', 'spike', 'sulfur', 'cinnabar',
                                      'tulip','eyeblossom','flower','sapling','vines','roots','fungus',
                                      'sprouts','azalea','dripleaf','moss','lily_pad','cocoa','sweet_berry')))


def requested(plan, dim):
    if plan is None:
        return None
    return {(x, z) for x0, z0, x1, z1 in plan['windows'].get(dim, [])
            for x in range(x0, x1+1) for z in range(z0, z1+1)}


def piece_boxes(reader,files,starts,present,dim):
    """Material spectra inside unioned piece boxes; terrain/air remain included."""
    index=defaultdict(list)
    quality=defaultdict(Counter)
    kinds={}
    for number,start in enumerate(starts or []):
        kind=structure_family(start['type']);boxes=start.get('boxes',[])
        quality[kind]['starts']+=1
        if not boxes:
            quality[kind]['without_exposed_boxes']+=1
            continue
        covered={(x,z) for x0,y0,z0,x1,y1,z1 in boxes for x in range(x0//16,x1//16+1) for z in range(z0//16,z1//16+1)}
        if not covered<=present:
            quality[kind]['clipped_by_sample']+=1
            continue
        quality[kind]['complete_instances']+=1
        kinds[number]=kind
        for box in boxes:
            x0,y0,z0,x1,y1,z1=box
            for x in range(x0//16,x1//16+1):
                for z in range(z0//16,z1//16+1):index[x,z].append((number,box))
    totals=defaultdict(Counter)
    wanted=set(index)
    for file in files:
        rx,rz=map(int,file.stem.split('.')[1:])
        if not any(x//32==rx and z//32==rz for x,z in wanted):continue
        for cx,cz,blocks,_,_ in reader.read(file,wanted):
            masks={}
            for number,box in index[cx,cz]:
                mask=masks.setdefault(number,np.zeros(blocks.shape,dtype=bool))
                x0,y0,z0,x1,y1,z1=box
                low=-64 if dim=='overworld' else 0
                high=319 if dim=='overworld' else 255
                y0,y1=max(y0,low),min(y1,high)
                if y0<=y1:
                    mask[y0+64:y1+65,max(z0-cz*16,0):min(z1-cz*16+1,16),max(x0-cx*16,0):min(x1-cx*16+1,16)]=True
            for number,mask in masks.items():add(totals[kinds[number]],blocks[mask],reader.names)
    return {'materials':dict(totals),'coverage':dict(quality),
            'definition':'Union of exposed piece boxes per instance; complete sampled instances only; includes natural terrain and air; not template attribution'}


def structure_family(name):
    for prefix in ('village','mineshaft','ruined_portal','shipwreck','ocean_ruin'):
        if name.startswith(prefix):return prefix
    return name


def layout_metrics(candidates,observed):
    """Compare eligible starts to persisted starts; does not assert placement."""
    groups=[]
    for records in (candidates,observed):
        group=defaultdict(set)
        for s in records or []:group[comparison_family(s['type'])].add(tuple(s['chunk']))
        groups.append(group)
    out={}
    for kind in groups[0].keys()|groups[1].keys():
        a,b=groups[0][kind],groups[1][kind]
        row={'ours_count':len(a),'vanilla_count':len(b),'coincident':len(a&b),
             'ours_coincident_percent':len(a&b)/len(a)*100 if a else None}
        arrays=[np.asarray(sorted(points),dtype=float).reshape(-1,2)*16 for points in (a,b)]
        for label,points in zip(('ours','vanilla'),arrays):
            row[label+'_nearest_spacing']=quantiles(cKDTree(points).query(points,k=2)[0][:,1]) if len(points)>1 else None
        row['ours_to_official_distance']=quantiles(cKDTree(arrays[1]).query(arrays[0])[0]) if a and b else None
        out[kind]=row
    return out


def comparison_family(name):
    return 'abandoned_camp' if name.startswith('abandoned_camp') else structure_family(name)


def analyze(world, plan, fingerprint, candidates=None):
    reader = Reader()
    result = {'level': level(world), 'manifest': fingerprint, 'dimensions': {}}
    for dim in DIMS:
        wanted = requested(plan, dim)
        folder = region_dir(world, dim)
        files = sorted(list(folder.glob('*.mca')) + list(folder.glob('*.mcs')))
        if not files:
            result['dimensions'][dim] = {'coverage': {'chunks': 0, 'requested': len(wanted) if wanted is not None else None},
                                         'unmeasured': 'No saved region files'}
            continue
        start = time.monotonic()
        counts, biomes3d, surfaces = Counter(), Counter(), Counter()
        topblocks, layers, feature_bio = defaultdict(Counter), defaultdict(Counter), defaultdict(Counter)
        height_bio, ores = defaultdict(Counter), defaultdict(Counter)
        terracotta_bands=defaultdict(Counter)
        depth = defaultdict(Counter)
        examples = defaultdict(list)
        tree_roots, tree_heights = Counter(), defaultdict(list)
        cave_shapes = Counter()
        cave_volumes, cave_extents = [], []
        objects=defaultdict(lambda:{'components':0,'edge_clipped_components':0,'volumes':[],'heights':[]})
        horizontal_logs=Counter()
        grass_cover_blocks=Counter()
        outside_height_nonair=0
        starts, structure_composition = [], defaultdict(Counter)
        predicted = candidates.get('dimensions',{}).get(dim,[]) if candidates else None
        if predicted is not None and wanted is not None:
            predicted=[s for s in predicted if tuple(s['chunk']) in wanted]
        candidate_at=defaultdict(list)
        for s in predicted or []:
            candidate_at[tuple(s['chunk'])].append(s['type'])
        candidate_composition=defaultdict(Counter)
        chunks = {}
        window_stats=[]
        for bounds in plan['windows'].get(dim,[]) if plan else []:
            window_stats.append({'bounds':bounds,'chunks':0,'columns':0,'blocks':Counter(),
                                 'surface_biomes':Counter(),'heights':Counter()})
        grass_above, grass_total, overhangs, transitions_total = 0, 0, 0, 0
        # Arrays retained per chunk are only surface data, never full block volumes.
        for file in files:
            for cx, cz, blocks, bios, root in reader.read(file, wanted):
                if (cx, cz) in chunks:
                    raise ValueError(f'Duplicate saved chunk {dim}/{cx}/{cz}')
                names = reader.names
                name_count = len(names)
                ground = np.asarray([natural(n) for n in names])[blocks]
                air = np.asarray([n in AIR for n in names])[blocks]
                water = np.asarray([n == 'water' for n in names])[blocks]
                lava = np.asarray([n == 'lava' for n in names])[blocks]
                if dim != 'overworld':
                    ground[:64]=False
                    ground[320:]=False
                    outside_height_nonair+=int((~air[:64]).sum()+(~air[320:]).sum())
                if dim == 'the_nether':
                    ground[129:] = False  # Highest natural floor at Y<=64; omit ceiling/roof.
                any_ground = ground.any(axis=0)
                top = np.where(any_ground, 383 - ground[::-1].argmax(axis=0), 0)
                ytop = np.where(any_ground,top-64,-64 if dim=='overworld' else -1)
                if dim!='overworld':
                    top=np.where(any_ground,top,64)
                zgrid, xgrid = np.indices((16, 16))
                bioarray = np.asarray(bios).reshape(96, 4, 4)
                surface_bio = bioarray[top//4, zgrid//4, xgrid//4]
                add(surfaces, surface_bio)
                local_counts=Counter()
                add(local_counts, blocks if dim=='overworld' else blocks[64:320], names)
                counts.update(local_counts)
                horizontal_logs.update(root.get('_audit_horizontal_logs',{}) if root else {})
                for window in window_stats:
                    wx0,wz0,wx1,wz1=window['bounds']
                    if wx0<=cx<=wx1 and wz0<=cz<=wz1:
                        window['chunks']+=1
                        window['columns']+=256
                        window['blocks'].update(local_counts)
                        add(window['surface_biomes'],surface_bio)
                        add(window['heights'],ytop)
                for kind in candidate_at.get((cx,cz),[]):
                    add(candidate_composition[kind],blocks,names)
                add(biomes3d, bioarray if dim=='overworld' else bioarray[16:80])
                for b in np.unique(surface_bio):
                    select = surface_bio == b
                    if len(examples['biome/'+str(b)])<3:
                        iz,ix=np.argwhere(select)[0]
                        examples['biome/'+str(b)].append([cx*16+int(ix),int(ytop[iz,ix]),cz*16+int(iz)])
                    add(height_bio[str(b)], ytop[select])
                    add(topblocks[str(b)], blocks[top[select], zgrid[select], xgrid[select]], names)
                    for offset in range(6):
                        valid = select & (top >= offset)
                        add(layers[f'{b}/-{offset}'], blocks[(top-offset)[valid], zgrid[valid], xgrid[valid]], names)
                # Exact counts by each 16-block band, with below-terrain denominators.
                for si in range(24) if dim=='overworld' else range(4,20):
                    y0 = si*16
                    section = blocks[y0:y0+16]
                    below = np.arange(y0, y0+16)[:, None, None] < top[None, :, :] - 5
                    band = str(y0-64)
                    d = depth[band]
                    d['volume'] += 4096
                    d['below_surface_volume'] += int(below.sum())
                    for key, mask in [('air', air), ('water', water), ('lava', lava)]:
                        d[key] += int(mask[y0:y0+16].sum())
                        d['underground_' + key] += int((mask[y0:y0+16] & below).sum())
                        ek='depth/'+band+'/'+key
                        if len(examples[ek])<3 and (mask[y0:y0+16]&below).any():
                            iy,iz,ix=np.argwhere(mask[y0:y0+16]&below)[0]
                            examples[ek].append([cx*16+int(ix),y0+int(iy)-64,cz*16+int(iz)])
                        if key != 'air':
                            # Fluid-to-air interface levels are observational proxies for aquifer levels.
                            above = air[min(y0+1,383):min(y0+17,384)]
                            if above.shape == section.shape:
                                for ly, n in enumerate((mask[y0:y0+16] & below & above).sum(axis=(1,2))):
                                    if n:
                                        depth[band][key + '_interface_y_' + str(y0+ly-64)] += int(n)
                    keys, ns = np.unique(section, return_counts=True)
                    for code, n in zip(keys, ns):
                        name = names[code]
                        if len(examples['block/'+name])<3:
                            iy,iz,ix=np.argwhere(section==code)[0]
                            examples['block/'+name].append([cx*16+int(ix),y0+int(iy)-64,cz*16+int(iz)])
                        if name=='terracotta' or name.endswith('_terracotta'):
                            b3=bioarray[si*4:si*4+4].repeat(4,0).repeat(4,1).repeat(4,2)
                            for b,n2 in zip(*np.unique(b3[section==code],return_counts=True)):
                                terracotta_bands[str(b)+'/'+name][band]+=int(n2)
                        if name.endswith('_ore') or name in ('ancient_debris', 'raw_copper_block', 'raw_iron_block'):
                            ores[name][band] += int(n)
                            if len(examples['ore/' + name]) < 3:
                                iy, iz, ix = np.argwhere(section == code)[0]
                                examples['ore/' + name].append([cx*16+int(ix), y0+int(iy)-64, cz*16+int(iz)])
                        if feature(name):
                            # 4x4x4 biome cells expanded only for sections with watched content.
                            b3 = bioarray[si*4:si*4+4].repeat(4,0).repeat(4,1).repeat(4,2)
                            for b, n2 in zip(*np.unique(b3[section == code], return_counts=True)):
                                feature_bio[str(b)][name] += int(n2)
                # Covered grass includes every non-air block, including plants (not a decay verdict).
                grass = blocks == reader.lookup.get('grass_block', -1)
                covered = grass[:-1] & ~air[1:]
                grass_total += int(grass.sum())
                grass_above += int(covered.sum())
                add(grass_cover_blocks,blocks[1:][covered],names)
                if covered.any() and len(examples['covered_grass']) < 12:
                    iy, iz, ix = np.argwhere(covered)[0]
                    examples['covered_grass'].append([cx*16+int(ix), int(iy)-64, cz*16+int(iz), names[blocks[iy+1,iz,ix]]])
                # Upper-terrain solid-air-solid transitions, excluding plants and buildings.
                transitions = ground[1:] & ~ground[:-1]
                upper = np.arange(1,384)[:,None,None] >= np.maximum(128, top[None,:,:]-32)
                spans = (transitions & upper).sum(axis=0)
                overhangs += int((spans >= 2).sum())
                transitions_total += int(spans.sum())
                # Trunk-base estimator: connected root columns, split by species and biome.
                for code in np.unique(blocks):
                    name = names[code]
                    if not name.endswith(('_log', '_stem')):
                        continue
                    log = blocks == code
                    bases = log[1:] & ~log[:-1]
                    near = (np.arange(1,384)[:,None,None] >= top[None,:,:]) & (np.arange(1,384)[:,None,None] <= top[None,:,:]+4)
                    roots2d = (bases & near).any(axis=0)
                    labels, n = ndimage.label(roots2d)
                    for label_id, sl in enumerate(ndimage.find_objects(labels), 1):
                        zz, xx = np.argwhere(labels[sl] == label_id)[0] + [sl[0].start, sl[1].start]
                        b = str(surface_bio[zz,xx])
                        key = b + '/' + name
                        tree_roots[key] += 1
                        yy = np.flatnonzero(log[:,zz,xx])
                        tree_heights[key].append(int(yy[-1]-yy[0]+1))
                # Cave components are chunk-clipped shape descriptors, not identified carver types.
                cave = air & (np.arange(384)[:,None,None] < top[None,:,:]-5)
                if dim!='overworld':
                    cave[:64]=False;cave[320:]=False
                labels, n = ndimage.label(cave)
                sizes = np.bincount(labels.ravel())
                for label_id, sl in enumerate(ndimage.find_objects(labels), 1):
                    volume = int(sizes[label_id])
                    extent = [s.stop-s.start for s in sl]
                    cave_volumes.append(volume)
                    cave_extents.append(max(extent))
                    shape = 'large_chamber_like' if volume >= 512 else 'thin_like' if min(extent) <= 2 else 'other'
                    cave_shapes[shape] += 1
                    if len(examples['cave/'+shape])<3:
                        iy,iz,ix=np.argwhere(labels[sl]==label_id)[0]+[s.start for s in sl]
                        examples['cave/'+shape].append([cx*16+int(ix),int(iy)-64,cz*16+int(iz)])
                    if sl[1].start == 0 or sl[1].stop == 16 or sl[2].start == 0 or sl[2].stop == 16:
                        cave_shapes['touches_chunk_edge'] += 1
                signatures={'amethyst_body':{'amethyst_block','budding_amethyst'},'bone_body':{'bone_block'},
                            'ice_body':{'packed_ice','blue_ice'},'mossy_boulder_or_building':{'mossy_cobblestone'},
                            'copper_ore_body':{'copper_ore','deepslate_copper_ore','raw_copper_block'},
                            'iron_ore_body':{'iron_ore','deepslate_iron_ore','raw_iron_block'}}
                for signature,mats in signatures.items():
                    codes=[reader.lookup[n] for n in mats if n in local_counts]
                    if not codes:
                        continue
                    lab,n=ndimage.label(np.isin(blocks,codes))
                    sizes=np.bincount(lab.ravel())
                    obj=objects[signature];obj['components']+=int(n)
                    for label_id,sl in enumerate(ndimage.find_objects(lab),1):
                        obj['volumes'].append(int(sizes[label_id]));obj['heights'].append(sl[0].stop-sl[0].start)
                        obj['edge_clipped_components']+=int(sl[1].start==0 or sl[1].stop==16 or sl[2].start==0 or sl[2].stop==16)
                # MCS does not serialize starts: unavailable, not zero structures.
                if root is not None:
                    for key, data in root.get('structures', {}).get('starts', {}).items():
                        if not isinstance(data, dict) or data.get('id', data.get('Id', 'INVALID')) in ('INVALID', 'minecraft:empty'):
                            continue
                        children = data.get('Children', data.get('children', []))
                        boxes = [p.get('BB', p.get('bounding_box')) for p in children]
                        starts.append({'type': key.removeprefix('minecraft:'), 'chunk': [cx,cz],
                                       'children': len(children), 'boxes': [b for b in boxes if b],
                                       'anchor_chunk_composition_only': True})
                        add(structure_composition[key.removeprefix('minecraft:')], blocks, names)
                chunks[cx,cz] = (ytop.astype(np.int16), surface_bio[2::4,2::4].copy())
            print(dim, file.name, len(chunks), f'{time.monotonic()-start:.1f}s', flush=True)
        if not chunks:
            result['dimensions'][dim] = {'coverage': {'chunks': 0, 'requested': len(wanted) if wanted is not None else None}}
            continue
        x0, z0 = map(min, zip(*chunks))
        x1, z1 = map(max, zip(*chunks))
        if (x1-x0+1)*(z1-z0+1)*256>20_000_000:
            raise ValueError('Disjoint windows span too much empty space: compare targets individually with --target-name')
        hmap = np.full(((z1-z0+1)*16, (x1-x0+1)*16), -32768, dtype=np.int16)
        bnames = sorted(surfaces)
        bindex = {b:i+1 for i,b in enumerate(bnames)}
        # Coasts and patches use 4-block cells, sampled at local (2,2).
        bmap = np.zeros(((z1-z0+1)*4, (x1-x0+1)*4), dtype=np.uint16)
        for (cx,cz),(h,b) in chunks.items():
            hmap[(cz-z0)*16:(cz-z0+1)*16,(cx-x0)*16:(cx-x0+1)*16] = h
            bmap[(cz-z0)*4:(cz-z0+1)*4,(cx-x0)*4:(cx-x0+1)*4] = np.vectorize(bindex.get)(b)
        valid = hmap != -32768
        jumps, boundary, interior = [], [], []
        jump_examples = []
        for axis in (0,1):
            a = np.diff(hmap.astype(np.int32), axis=axis)
            both = (valid[1:,:] & valid[:-1,:]) if axis == 0 else (valid[:,1:] & valid[:,:-1])
            at = np.indices(a.shape)[axis]
            border = (at+1)%16 == 0
            jumps.append(np.abs(a[both]))
            boundary.append(np.abs(a[both & border]))
            interior.append(np.abs(a[both & ~border]))
            for zz,xx in np.argwhere(both & (np.abs(a)>=16))[:12]:
                jump_examples.append([x0*16+int(xx),z0*16+int(zz),int(a[zz,xx]),'chunk_border' if border[zz,xx] else 'inside'])
        j, jb, ji = np.concatenate(jumps), np.concatenate(boundary), np.concatenate(interior)
        inner=valid[:-1,:-1]&valid[1:,:-1]&valid[:-1,1:]
        dx=np.diff(hmap.astype(np.int32),axis=1)[:-1,:]
        dz=np.diff(hmap.astype(np.int32),axis=0)[:,:-1]
        slopes=np.floor(np.sqrt(dx[inner]**2+dz[inner]**2)).astype(np.int32)
        peakmask=valid & (hmap>=128) & (hmap==ndimage.maximum_filter(hmap,size=33))
        peaklabels,peakcount=ndimage.label(peakmask)
        peak_examples=[]
        for sl in ndimage.find_objects(peaklabels)[:20]:
            zz,xx=sl[0].start,sl[1].start
            peak_examples.append([x0*16+xx,int(hmap[zz,xx]),z0*16+zz])
        patches, coast, river = {}, {}, {}
        observed = bmap != 0
        edge_distance = ndimage.distance_transform_edt(np.pad(observed,1))[1:-1,1:-1]*4
        ocean_codes = [bindex[b] for b in bnames if 'ocean' in b]
        ocean = np.isin(bmap, ocean_codes)
        land = observed & ~ocean
        # Missing cells are not treated as land; exclude distance censored by sample edge.
        distance = ndimage.distance_transform_edt(~land)*4
        for b in bnames:
            mask = bmap == bindex[b]
            lab,n = ndimage.label(mask)
            sizes = np.bincount(lab.ravel())[1:]*16
            touched = set(np.unique(lab[edge_distance<=4])) - {0}
            complete = [size for i,size in enumerate(sizes,1) if i not in touched]
            patches[b] = {'count': int(n), 'area_blocks_quantiles': quantiles(sizes),
                          'uncensored_count': len(complete), 'uncensored_area_quantiles': quantiles(complete)}
            if 'ocean' in b:
                sel = mask & (distance < edge_distance)
                coast[b] = {'n': int(sel.sum()), 'censored': int(mask.sum()-sel.sum()),
                            'distance_blocks': quantiles(distance[sel])}
            if b in ('river','frozen_river'):
                width = ndimage.distance_transform_edt(mask)*8
                sel = mask & (width/2 < edge_distance)
                river[b] = {'n':int(sel.sum()), 'twice_nearest_bank_distance': quantiles(width[sel]),
                            'definition':'4-block biome grid; local width proxy, not cross-section width'}
        spacing = {}
        for kind in sorted({s['type'] for s in starts}):
            pts = np.array([s['chunk'] for s in starts if s['type']==kind])*16
            distances = np.linalg.norm(pts[:,None]-pts[None,:],axis=2)
            np.fill_diagonal(distances,np.inf)
            spacing[kind] = {'count':len(pts),'nearest_observed_blocks':quantiles(distances.min(axis=1)) if len(pts)>1 else None}
        climate_share = Counter()
        for b,n in surfaces.items():
            climate = reader.climate.get(b)
            climate_share[str(climate) if climate else 'unknown'] += n
        box_stats=piece_boxes(reader,files,starts if files[0].suffix=='.mca' else predicted,set(chunks),dim)
        result['dimensions'][dim] = {
            'coverage': {'chunks':len(chunks),'requested':len(wanted) if wanted is not None else None,
                         'chunk_sha256':hashlib.sha256(json.dumps(sorted(chunks)).encode()).hexdigest(),
                         'missing_first20':sorted(wanted-set(chunks))[:20] if wanted is not None else [],
                         'bounds_chunks':[x0,z0,x1,z1]},
            'columns':int(valid.sum()), 'blocks':dict(counts), 'biomes_3d':dict(biomes3d),
            'outside_build_height_nonair_blocks':outside_height_nonair,
            'measured_y_range':[-64,319] if dim=='overworld' else [0,255],
            'height_definition':'Highest natural substrate; void=-64' if dim=='overworld' else 'Highest natural substrate at Y<=64; void=-1' if dim=='the_nether' else 'Highest natural substrate; void=-1',
            'windows':window_stats,
            'surface_biomes':dict(surfaces), 'nominal_climate_columns':dict(climate_share),
            'height_histogram':hist(hmap[valid]), 'height_quantiles':quantiles(hmap[valid]),
            'height_by_biome':dict(height_bio), 'jump_histogram':hist(j),
            'jump_quantiles':quantiles(j), 'border_jump_histogram':hist(jb),'inside_jump_histogram':hist(ji),
            'slope_magnitude_floor_histogram':hist(slopes),'slope_quantiles':quantiles(slopes),
            'local_peak_plateaus_ge128':int(peakcount),'peak_examples':peak_examples,
            'jump_ge16_share':float((j>=16).mean()) if j.size else None,
            'border_jump_ge16_share':float((jb>=16).mean()) if jb.size else None,
            'inside_jump_ge16_share':float((ji>=16).mean()) if ji.size else None,
            'overhang_proxy_columns':overhangs,'upper_solid_segments':transitions_total,
            'top_blocks_by_biome':dict(topblocks),'surface_layers':dict(layers),
            'covered_grass':grass_above,'grass_total':grass_total,
            'grass_cover_blocks':dict(grass_cover_blocks),
            'depth_bands':dict(depth),'ores_by_y16':dict(ores), 'features_by_biome':dict(feature_bio),
            'terracotta_bands_by_biome_y16':dict(terracotta_bands),
            'tree_base_estimates':dict(tree_roots), 'tree_vertical_spans':{k:quantiles(v) for k,v in tree_heights.items()},
            'horizontal_log_blocks':dict(horizontal_logs),
            'object_geometry_proxies':{k:{'components':v['components'],'edge_clipped_components':v['edge_clipped_components'],
                                          'volume_quantiles':quantiles(v['volumes']),'height_quantiles':quantiles(v['heights'])} for k,v in objects.items()},
            'cave_chunk_components':dict(cave_shapes),'cave_component_volumes':quantiles(cave_volumes),
            'cave_component_max_extent':quantiles(cave_extents),
            'patches':patches,'ocean_coast_distance':coast,'river_width_proxy':river,
            'structure_starts':starts if files[0].suffix=='.mca' else None,
            'structure_spacing':spacing if files[0].suffix=='.mca' else None,
            'structure_anchor_chunk_blocks':dict(structure_composition) if files[0].suffix=='.mca' else None,
            'generator_start_candidates':predicted,'candidate_anchor_chunk_blocks':dict(candidate_composition) if predicted is not None else None,
            'structure_piece_box_measurements':box_stats,
            'examples':dict(examples),'jump_examples':jump_examples,
            'limitations': ['Tree bases/spans are estimators; roots clipped at chunk borders; fallen/hollow trunks can bias them.',
                            'Cave shapes are chunk-clipped connected air components; no cheese/spaghetti/noodle attribution.',
                            'Coast/patch distances use the 4-block biome grid; edge censoring is recorded.',
                            'Aquifer interfaces include underground lakes; source mechanism is not serialized.',
                            'Structure starts are not stored in MCS; null is unavailable, never zero.',
                            'Anchor chunk block composition includes natural terrain, not full structure composition.',
                            'Covered grass includes plants and transparent blocks; this count alone does not prove decay.',
                            'Nominal biome climate metadata is not the hidden sampled climate noise.']}
    return result


def batch(key):
    if 'structure' in key:
        metric=key.split('/')[2] if '/structure_box_composition/' in key else key.split('/')[-1]
        modules={'village':'village.rs','pillager_outpost':'pillager_outpost.rs','stronghold':'stronghold.rs',
                 'mineshaft':'mineshaft.rs','fortress':'nether_fortress.rs','bastion_remnant':'nether_bastion.rs',
                 'nether_fossil':'nether_fossil.rs','end_city':'end_city.rs','trial_chambers':'trial_chambers.rs',
                 'ancient_city':'ancient_city.rs','shipwreck':'shipwreck.rs','ocean_ruin':'ocean_ruin.rs',
                 'mansion':'mansion.rs','monument':'monument.rs','trail_ruins':'trail_ruins.rs',
                 'ruined_portal':'ruined_portal.rs','buried_treasure':'shipwreck.rs','abandoned_camp':'abandoned_camp.rs'}
        module=modules.get(metric,'temples.rs')
        return 'R-'+module.removesuffix('.rs'),module,'3–8 h per module'
    if key.startswith('the_nether/'): return 'N', 'nether.rs', '4–8 h'
    if key.startswith('the_end/'): return 'E', 'the_end.rs', '3–6 h'
    metric=key.split('/',1)[1]
    material=metric.split('/')[-1]
    if metric=='spawn_offset':return 'A','spawn_search.rs','12–24 h (shared terrain batch)'
    if metric.startswith(('surface_biomes/','biomes_3d/','nominal_climate_columns/','patches/','ocean_coast_distance/','river_width_proxy/')):
        return 'A', 'terrain.rs', '12–24 h'
    if metric.startswith(('height','jump','border_jump','inside_jump','overhang','slope','top/','layer/','covered_grass','mountain_','surface_band/')):
        return 'A', 'terrain.rs', '12–24 h'
    if metric.startswith('ore/') or material.endswith('_ore') or any(s in metric for s in ('ore_body','raw_iron_block','raw_copper_block','ancient_debris','amethyst_body')):
        return 'U', 'underground.rs', '8–16 h'
    if metric.startswith(('depth/','cave_')) or any(s in metric for s in ('sculk','dripstone','sulfur','cinnabar')):
        return 'U', 'underground_carvers.rs', '8–16 h'
    if any(s in metric for s in ('log','leaves','tree_')): return 'F-tree', 'trees.rs', '4–8 h'
    if any(s in metric for s in ('coral','kelp','seagrass','sea_pickle')): return 'F-ocean', 'ocean.rs', '3–6 h'
    if material in GROUND or material.endswith('_terracotta') or material in AIR|FLUID|{'snow'}:
        return 'A','terrain.rs','12–24 h'
    if material in {'trial_spawner','vault','tuff_bricks','polished_tuff','tuff_brick_stairs','tuff_brick_slab'}:
        return 'R-trial_chambers','trial_chambers.rs','3–8 h per module'
    if material=='spawner':return 'U','underground_dungeons.rs','8–16 h'
    return 'F', 'vegetation.rs', '6–12 h'


def comparison(ours, vanilla, allow_incomplete=False):
    rows = []
    coverage = []
    def emit(dim, key, a, b, unit, tolerance, weight=1, at=None, denominator=None):
        if a is None or b is None:
            return
        delta = a-b
        limit = tolerance
        verdict = 'outside' if abs(delta)>limit else 'within'
        group,module,estimate = batch(dim+'/'+key)
        # Ranking is triage: category weight times normalized effect; no significance claim.
        score = weight*abs(delta)/max(limit, 1e-9)
        rows.append({'dimension':dim,'metric':key,'ours':round(float(a),6),'vanilla':round(float(b),6),
                     'delta':round(float(delta),6),'unit':unit,'tolerance':limit,'verdict':verdict,
                     'score':round(score,4),'coordinate_examples':at or [],'batch':group,
                     'likely_module_guess':module,'codex_estimate':estimate,'denominator':denominator})
    if ours['level'].get('spawn') and vanilla['level'].get('spawn'):
        os,vs=ours['level']['spawn'],vanilla['level']['spawn']
        emit('overworld','spawn_offset',math.hypot(os[0]-vs[0],os[1]-vs[1]),0,'blocks from official spawn',16,8,at=[os,vs])
    for dim in DIMS:
        a,b = ours['dimensions'][dim],vanilla['dimensions'][dim]
        ca,cb = a['coverage'],b['coverage']
        ok = ca.get('chunks',0)>0 and ca.get('chunk_sha256')==cb.get('chunk_sha256')
        complete = all(c.get('requested') is None or c['chunks']==c['requested'] for c in (ca,cb))
        if ca.get('requested')==0 and cb.get('requested')==0:
            coverage.append({'dimension':dim,'ours':ca,'vanilla':cb,'same_chunks':True,'complete':True,'status':'not requested by this plan'})
            continue
        coverage.append({'dimension':dim,'ours':ca,'vanilla':cb,'same_chunks':ok,'complete':complete})
        if not ok or not complete:
            if not allow_incomplete:
                raise ValueError(f'{dim}: unequal/incomplete coverage; explicitly use --allow-incomplete for a provisional report')
            # Unequal denominators cannot establish matched-area generation discrepancies.
            if not ok:
                continue
        columns = a['columns']
        emit(dim,'outside_build_height_nonair',a['outside_build_height_nonair_blocks'],b['outside_build_height_nonair_blocks'],'blocks',0,5)
        for metric in ('surface_biomes','biomes_3d','nominal_climate_columns'):
            ac,bc = a[metric],b[metric]
            na,nb = sum(ac.values()),sum(bc.values())
            for key in sorted(ac.keys()|bc.keys()):
                emit(dim,metric+'/'+key,ac.get(key,0)/na*100,bc.get(key,0)/nb*100,'percentage points',1,4,denominator=[na,nb],
                     at=a['examples'].get('biome/'+key,b['examples'].get('biome/'+key,[])))
        for q in ('5','50','95','99','100'):
            emit(dim,'height/p'+q,a['height_quantiles'][q],b['height_quantiles'][q],'blocks',4,5,at=a['peak_examples'][:3])
            if a['slope_quantiles'] and b['slope_quantiles']:
                emit(dim,'slope/p'+q,a['slope_quantiles'][q],b['slope_quantiles'][q],'blocks per block',1,4,at=a['jump_examples'][:3])
        av=a['local_peak_plateaus_ge128']/columns*1000
        bv=b['local_peak_plateaus_ge128']/columns*1000
        emit(dim,'mountain_local_peaks_ge128',av,bv,'local peak plateaus per 1000 columns',max(.01,bv*.2),3,at=a['peak_examples'][:3])
        for metric in ('jump_ge16_share','border_jump_ge16_share','inside_jump_ge16_share'):
            if a[metric] is not None and b[metric] is not None:
                emit(dim,metric,a[metric]*100,b[metric]*100,'percentage points',0.25,6,at=a['jump_examples'][:3])
        for biome in sorted(a['height_by_biome'].keys() & b['height_by_biome'].keys()):
            def from_hist(h):
                return sum(int(y)*n for y,n in h.items())/sum(h.values())
            if min(sum(a['height_by_biome'][biome].values()),sum(b['height_by_biome'][biome].values()))>=256:
                emit(dim,'height_mean/'+biome,from_hist(a['height_by_biome'][biome]),from_hist(b['height_by_biome'][biome]),'blocks',5,4,at=a['examples'].get('biome/'+biome,[]))
                def percentile(h,q):
                    total=sum(h.values());n=0
                    for y,count in sorted(h.items(),key=lambda kv:int(kv[0])):
                        n+=count
                        if n>=total*q:return int(y)
                for q in (.95,1):
                    emit(dim,'height_biome/'+biome+'/p'+str(int(q*100)),percentile(a['height_by_biome'][biome],q),percentile(b['height_by_biome'][biome],q),
                         'Y blocks',5,3,at=a['examples'].get('biome/'+biome,[]))
        for metric in ('covered_grass','overhang_proxy_columns'):
            emit(dim,metric,a[metric]/columns*100,b[metric]/columns*100,'per 100 columns',1,2,
                 at=a['examples'].get('covered_grass',[])[:3] if metric=='covered_grass' else None)
        for category,denom in [('blocks',columns),('tree_base_estimates',columns),('horizontal_log_blocks',columns)]:
            ac,bc = a[category],b[category]
            for key in sorted(ac.keys()|bc.keys()):
                av,bv = ac.get(key,0)/denom*256,bc.get(key,0)/denom*256
                tolerance = max(0.1,abs(bv)*0.20)
                emit(dim,category+'/'+key,av,bv,'per chunk',tolerance,2,
                     at=a['examples'].get('block/'+key,a['examples'].get('ore/'+key,[])))
        for biome in sorted(a['top_blocks_by_biome'].keys() & b['top_blocks_by_biome'].keys()):
            for metric in ['top_blocks_by_biome']+['surface_layers']:
                for suffix in ([''] if metric=='top_blocks_by_biome' else [f'/-{i}' for i in range(1,6)]):
                    ac,bc = a[metric].get(biome+suffix,{}),b[metric].get(biome+suffix,{})
                    na,nb = sum(ac.values()),sum(bc.values())
                    if min(na,nb)<256:
                        continue
                    for key in ac.keys()|bc.keys():
                        emit(dim,('top/' if not suffix else 'layer/')+biome+suffix+'/'+key,
                             ac.get(key,0)/na*100,bc.get(key,0)/nb*100,'percentage points',5,3,denominator=[na,nb],at=a['examples'].get('biome/'+biome,[]))
        for biome in a['features_by_biome'].keys()|b['features_by_biome'].keys():
            ac,bc=a['features_by_biome'].get(biome,{}),b['features_by_biome'].get(biome,{})
            na,nb=a['biomes_3d'].get(biome,0)*64,b['biomes_3d'].get(biome,0)*64
            if min(na,nb)<65536:
                continue
            for key in ac.keys()|bc.keys():
                av,bv=ac.get(key,0)/na*1e6,bc.get(key,0)/nb*1e6
                emit(dim,'feature_biome/'+biome+'/'+key,av,bv,'per million biome blocks',max(5,bv*0.2),1,
                     at=a['examples'].get('block/'+key,b['examples'].get('block/'+key,[])),denominator=[na,nb])
        for key in a['tree_vertical_spans'].keys()&b['tree_vertical_spans'].keys():
            aq,bq=a['tree_vertical_spans'][key],b['tree_vertical_spans'][key]
            emit(dim,'tree_span/'+key+'/p50',aq['50'],bq['50'],'blocks (trunk span estimator)',2,2,
                 at=a['examples'].get('block/'+key.split('/')[-1],[]))
        for q in ('50','95','99'):
            aq,bq=a['cave_component_volumes'],b['cave_component_volumes']
            if aq and bq:
                emit(dim,'cave_component_volume/p'+q,aq[q],bq[q],'blocks (chunk clipped)',max(16,bq[q]*0.2),2)
        for key in ('large_chamber_like','thin_like','other'):
            av=a['cave_chunk_components'].get(key,0)/a['coverage']['chunks']
            bv=b['cave_chunk_components'].get(key,0)/b['coverage']['chunks']
            emit(dim,'cave_shape_proxy/'+key,av,bv,'chunk-clipped components per chunk',max(.1,bv*.2),2,at=a['examples'].get('cave/'+key,[]))
        for key in a['object_geometry_proxies'].keys()&b['object_geometry_proxies'].keys():
            aq=a['object_geometry_proxies'][key]['volume_quantiles']
            bq=b['object_geometry_proxies'][key]['volume_quantiles']
            if aq and bq:
                emit(dim,'object_geometry_proxy/'+key+'/volume_p95',aq['95'],bq['95'],'blocks (chunk clipped)',max(4,bq['95']*.2),1)
        for fluid in ('water','lava'):
            values=[]
            for side in (a,b):
                pairs=Counter()
                for band in side['depth_bands'].values():
                    for key,count in band.items():
                        prefix=fluid+'_interface_y_'
                        if key.startswith(prefix):pairs[int(key[len(prefix):])]+=count
                total=sum(pairs.values())
                median=None
                cumulative=0
                for y,count in sorted(pairs.items()):
                    cumulative+=count
                    if cumulative>=total/2:
                        median=y;break
                values.append(median)
            emit(dim,'depth/'+fluid+'_interface_median_y',*values,'Y blocks',4,2)
        if min(a['grass_total'],b['grass_total'])>=256:
            for key in a['grass_cover_blocks'].keys()|b['grass_cover_blocks'].keys():
                av=a['grass_cover_blocks'].get(key,0)/a['grass_total']*100
                bv=b['grass_cover_blocks'].get(key,0)/b['grass_total']*100
                emit(dim,'covered_grass_by/'+key,av,bv,'per 100 grass blocks',2,2,at=a['examples'].get('covered_grass',[])[:3])
        for band in sorted(a['depth_bands'].keys() & b['depth_bands'].keys(),key=int):
            ac,bc = a['depth_bands'][band],b['depth_bands'][band]
            for key in ('air','water','lava','underground_air','underground_water','underground_lava'):
                denom = 'below_surface_volume' if key.startswith('underground') else 'volume'
                na,nb = ac[denom],bc[denom]
                if min(na,nb)>0:
                    emit(dim,'depth/'+band+'/'+key,ac[key]/na*100,bc[key]/nb*100,'percentage points',2,3,denominator=[na,nb],
                         at=a['examples'].get('depth/'+band+'/'+key.removeprefix('underground_'),[]))
        for ore in a['ores_by_y16'].keys() | b['ores_by_y16'].keys():
            ac,bc=a['ores_by_y16'].get(ore,{}),b['ores_by_y16'].get(ore,{})
            for band in ac.keys()|bc.keys():
                av,bv=ac.get(band,0)/columns*256,bc.get(band,0)/columns*256
                emit(dim,'ore/'+ore+'/y'+band,av,bv,'per chunk',max(0.25,bv*0.20),2,at=a['examples'].get('ore/'+ore,[]))
        for key in a['terracotta_bands_by_biome_y16'].keys()|b['terracotta_bands_by_biome_y16'].keys():
            ac=a['terracotta_bands_by_biome_y16'].get(key,{})
            bc=b['terracotta_bands_by_biome_y16'].get(key,{})
            for band in ac.keys()|bc.keys():
                av,bv=ac.get(band,0)/columns*256,bc.get(band,0)/columns*256
                emit(dim,'surface_band/'+key+'/y'+band,av,bv,'blocks per sampled chunk',max(.25,bv*.2),1,
                     at=a['examples'].get('biome/'+key.split('/')[0],[]))
        for metric,field,tol in [('patches','uncensored_area_quantiles',64),('ocean_coast_distance','distance_blocks',16),('river_width_proxy','twice_nearest_bank_distance',8)]:
            for key in a[metric].keys()&b[metric].keys():
                aq,bq=a[metric][key][field],b[metric][key][field]
                if aq and bq:
                    emit(dim,metric+'/'+key+'/p50',aq['50'],bq['50'],'blocks²' if metric=='patches' else 'blocks',tol,3)
        if a.get('generator_start_candidates') is not None and b.get('structure_starts') is not None:
            family=comparison_family
            predicted=a['generator_start_candidates']
            actual=b['structure_starts']
            ac=Counter(family(s['type']) for s in predicted)
            bc=Counter(family(s['type']) for s in actual)
            supported={'village','pillager_outpost','desert_pyramid','jungle_pyramid','monument','mansion','ancient_city',
                       'trial_chambers','stronghold','mineshaft','ruined_portal','shipwreck','buried_treasure',
                       'ocean_ruin','trail_ruins','igloo','swamp_hut','fortress','bastion_remnant','end_city','nether_fossil'}
            if any(family(s['type'])=='abandoned_camp' for s in predicted):supported.add('abandoned_camp')
            for key in sorted((ac.keys()|bc.keys())&supported):
                coords=[[s['chunk'][0]*16+8,s['chunk'][1]*16+8] for s in predicted if family(s['type'])==key][:3]
                emit(dim,'structure_candidate_count/'+key,ac[key],bc[key],'eligible starts / saved official starts',max(1,bc[key]*0.25),3,at=coords)
            for key,layout in layout_metrics(predicted,actual).items():
                coords=[[s['chunk'][0]*16+8,s['chunk'][1]*16+8] for s in predicted if family(s['type'])==key][:3]
                emit(dim,'structure_candidate_coincident/'+key,layout['ours_coincident_percent'],100,'percent of eligible starts at observed official chunks',5,3,at=coords)
                aq,bq=layout['ours_nearest_spacing'],layout['vanilla_nearest_spacing']
                if aq and bq:
                    for q in ('50','95'):
                        emit(dim,'structure_candidate_spacing_p'+q+'/'+key,aq[q],bq[q],'blocks between nearest observed starts',max(32,bq[q]*.25),2,at=coords)
                cross=layout['ours_to_official_distance']
                if cross:
                    emit(dim,'structure_candidate_offset_p95/'+key,cross['95'],0,'blocks to nearest official start',32,2,at=coords)
        def combine_boxes(materials):
            out=defaultdict(Counter)
            for kind,counts in materials.items():out[comparison_family(kind)].update(counts)
            return out
        ac=combine_boxes(a['structure_piece_box_measurements']['materials']);bc=combine_boxes(b['structure_piece_box_measurements']['materials'])
        for kind in ac.keys()&bc.keys():
            na,nb=sum(ac[kind].values()),sum(bc[kind].values())
            if min(na,nb)<64:continue
            for material in ac[kind].keys()|bc[kind].keys():
                emit(dim,'structure_box_composition/'+kind+'/'+material,ac[kind].get(material,0)/na*100,bc[kind].get(material,0)/nb*100,
                     'percent of piece-box volume (includes terrain)',5,2,denominator=[na,nb])
        if dim=='the_end':
            for index,(aw,bw) in enumerate(zip(a['windows'],b['windows'])):
                if not aw['columns'] or not bw['columns']:
                    continue
                for block in ('end_stone','chorus_plant','chorus_flower','purpur_block','end_stone_bricks'):
                    av=aw['blocks'].get(block,0)/aw['columns']*256
                    bv=bw['blocks'].get(block,0)/bw['columns']*256
                    emit(dim,f'window{index}/blocks/{block}',av,bv,'per chunk',max(1,bv*.2),3)
    return {'schema':SCHEMA,'ours_level':ours['level'],'vanilla_level':vanilla['level'],
            'coverage':coverage,'rows':sorted(rows,key=lambda r:(r['verdict']!='outside',-r['score'],r['metric'])),
            'unmeasured':['Exact structures counts/spacing parity: MCS has no persisted start metadata.',
                          'Whole-structure block composition: anchor-chunk composition is a contaminated proxy.',
                          'Exact cheese/spaghetti/noodle identities and copper/iron vein generator attribution.',
                          'True tree species instance counts/heights and fallen-tree attribution: estimators only.',
                          'Features absent from either measured sample: absence is not proof of nonimplementation.',
                          'Named boulder/iceberg/hoodoo instance attribution: natural block geometry needs targeted follow-up.'],
            'tolerance_policy':'Descriptive audit thresholds: biome 1 pp; heights 4–5 blocks; jumps >=16 0.25 pp; surfaces 5 pp; depth 2 pp; feature/ore counts 20% with absolute floors. No statistical equivalence claim.',
            'rank_policy':'Weighted effect divided by descriptive tolerance; correlated rows are not independent bugs.',
            'statistics':{'ours':ours,'vanilla':vanilla}}


def markdown(report):
    lines=['# Перепись генерации 26.3','',
           'Числа получены только чтением сохранённых регионов. Это проверка выбранных мест, а не доказательство совпадения всей генерации.',
           '',f"Семя: `{report['ours_level']['seed']}`. В таблице сначала самые большие отклонения от заранее заданного допуска. Связанные строки могут иметь одну причину.",
           '', '| Место | Показатель | У нас | Оригинал | Допуск | Вердикт | Группа / файл (догадка) |',
           '|---|---|---:|---:|---:|---|---|']
    for r in report['rows']:
        if r['verdict']!='outside': continue
        lines.append(f"| {r['dimension']} | {r['metric']} ({r['unit']}) | {r['ours']:.5g} | {r['vanilla']:.5g} | {r['tolerance']:.5g} | вне допуска | {r['batch']} / {r['likely_module_guess']} |")
    lines += ['', 'Покрытие:', '']
    for c in report['coverage']:
        lines.append(f"- {c['dimension']}: чанков у нас {c['ours']['chunks']}, у оригинала {c['vanilla']['chunks']}; одинаковые места: {c['same_chunks']}; план выполнен: {c['complete']}.")
    lines += ['', 'Ограничения (не считать совпадением):', '']
    lines += ['- '+s for s in report['unmeasured']]
    lines += ['', 'Точные гистограммы, координаты примеров, пропущенные чанки, исходные файлы с SHA-256 и определения измерений находятся в соседнем JSON.', '']
    return '\n'.join(lines)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('ours'); p.add_argument('vanilla'); p.add_argument('--plan')
    p.add_argument('--output',required=True); p.add_argument('--cache')
    p.add_argument('--allow-incomplete',action='store_true')
    p.add_argument('--starts-executable',help='Optional helper linked against our release seedmap-core; requires --plan')
    p.add_argument('--starts-json',help='Previously saved output of the optional eligible-start adapter')
    p.add_argument('--target-name',help='One biome target from a --targets plan; prevents mixing biased samples')
    p.add_argument('--release-deps',help='Compile the eligible-start adapter with rustc against an existing cargo build --release deps directory')
    args=p.parse_args()
    plan=json.loads(Path(args.plan).read_text()) if args.plan else None
    if plan is None:
        saved_plan=Path(args.ours).parent/'generation-complete.json'
        if saved_plan.exists():
            plan=json.loads(saved_plan.read_text())
    if args.target_name:
        is_structure=args.target_name.startswith('structure:')
        field='structure_targets' if is_structure else 'biome_targets'
        attribute='structure' if is_structure else 'biome'
        target_name=args.target_name.removeprefix('structure:')
        target=next((t for t in plan.get(field,[]) if t[attribute]==target_name),None) if plan else None
        if not target or not target['bounds']:
            raise ValueError('Requested biome target was not located')
        plan=dict(plan,windows={d:[target['bounds']] if d==target['dimension'] else [] for d in DIMS})
    candidates=json.loads(Path(args.starts_json).read_text()) if args.starts_json else None
    if args.release_deps:
        if not plan:
            raise ValueError('--release-deps needs a recorded generation plan')
        deps=Path(args.release_deps).resolve()
        with tempfile.TemporaryDirectory(prefix='genaudit-starts-') as scratch:
            executable=Path(scratch)/'starts'
            from genaudit_263_build_starts import build
            build(deps,executable)
            temp_plan=Path(scratch)/'plan.json';temp_plan.write_text(json.dumps(plan))
            candidates=json.loads(subprocess.check_output([str(executable),str(temp_plan)],text=True))
    if args.starts_executable:
        if not args.plan:
            raise ValueError('--starts-executable requires explicit --plan')
        candidates=json.loads(subprocess.check_output([args.starts_executable,args.plan],text=True))
    ol,vl=level(args.ours),level(args.vanilla)
    if ol['seed']!=vl['seed']:
        raise ValueError(f"Seed mismatch: {ol['seed']} vs {vl['seed']}")
    if plan and ol['seed']!=plan['seed']:
        raise ValueError('Plan seed differs from world seed')
    if candidates and ol['seed']!=candidates['seed']:
        raise ValueError('Eligible-start adapter seed differs from world seed')
    sides=[]
    for label,world in [('ours',args.ours),('vanilla',args.vanilla)]:
        fp=manifest(world)
        source_hash=hashlib.sha256(Path(__file__).read_bytes()+Path(__file__).with_name('genaudit_263_io.py').read_bytes()).hexdigest()
        token=hashlib.sha256(json.dumps([SCHEMA,source_hash,fp,plan,candidates if label=='ours' else None],sort_keys=True).encode()).hexdigest()
        cached=Path(args.cache)/f'{label}-{token}.json' if args.cache else None
        if cached and cached.exists():
            data=json.loads(cached.read_text())
        else:
            data=analyze(world,plan,fp,candidates if label=='ours' else None)
            if cached:
                cached.parent.mkdir(parents=True,exist_ok=True)
                cached.write_text(json.dumps(data,ensure_ascii=False,separators=(',',':'))+'\n')
        sides.append(data)
    report=comparison(*sides,args.allow_incomplete)
    report['analyzer_source_sha256']=source_hash
    report['eligible_start_provenance']=candidates.get('method') if candidates else None
    target=Path(args.output); target.parent.mkdir(parents=True,exist_ok=True)
    Path(str(target)+'.json').write_text(json.dumps(report,ensure_ascii=False,allow_nan=False,separators=(',',':'))+'\n')
    Path(str(target)+'.md').write_text(markdown(report))
    print('Report:',str(target)+'.json',str(target)+'.md', 'outside:',sum(r['verdict']=='outside' for r in report['rows']))


if __name__=='__main__':
    main()
