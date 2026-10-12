#!/usr/bin/env python3
"""Повторная перепись: только сохранённые region/entities, без сервера и jar."""
import argparse
import collections
import functools
import json
import math
from pathlib import Path
import struct
import zlib
from nether_census import nbt, unpack
from structures_audit import biome, inside, window


def windows(halo=0):
    for gx in (-1, 0, 1):
        for gz in (-1, 0, 1):
            h = (64 if gx == gz == 0 else 32) + halo
            yield gx * 512, gz * 512, h


def expected(halo=0):
    return {(cx, cz) for x, z, h in windows(halo)
            for cx in range(x-h, x+h) for cz in range(z-h, z+h)}


def decode(data, index):
    if len(data) < 8192:
        raise ValueError('short region header')
    offset = int.from_bytes(data[index*4:index*4+3], 'big') * 4096
    sectors = data[index*4+3]
    if not offset:
        return None
    if offset < 8192 or offset + 5 > len(data):
        raise ValueError('invalid record offset')
    length = struct.unpack_from('>I', data, offset)[0]
    if length < 1 or length + 4 > sectors*4096 or offset + 4 + length > len(data):
        raise ValueError('truncated record')
    if data[offset+4] != 2:
        raise ValueError('unsupported compression')
    raw = zlib.decompress(data[offset+5:offset+4+length])
    if raw[0] != 10:
        raise ValueError('non-compound root')
    root, end = nbt(raw, 3 + struct.unpack_from('>H', raw, 1)[0], 10)
    if end != len(raw):
        raise ValueError('trailing NBT bytes')
    return root


class World:
    def __init__(self, world, dimension):
        self.base = Path(world)/'dimensions/minecraft'/dimension
        self.dimension = dimension

    @functools.lru_cache(maxsize=4)
    def region(self, folder, rx, rz):
        p = self.base/folder/f'r.{rx}.{rz}.mca'
        return p.read_bytes() if p.exists() else None

    def read(self, cx, cz, folder='region'):
        data = self.region(folder, cx//32, cz//32)
        return decode(data, cx%32+cz%32*32) if data else None

    @functools.lru_cache(maxsize=128)
    def chunk(self, cx, cz):
        try:
            root = self.read(cx, cz)
        except (ValueError, IndexError, struct.error, zlib.error):
            return None
        return root if valid(root, cx, cz) is None else None

    @functools.lru_cache(maxsize=512)
    def section(self, cx, sy, cz):
        root = self.chunk(cx, cz)
        if root is None:
            return None
        for s in root.get('sections', []):
            if signed(s['Y']) == sy:
                bs = s.get('block_states')
                if not bs:
                    return None
                pal = [p['Name'].removeprefix('minecraft:') for p in bs['palette']]
                ids = [0]*4096 if len(pal) == 1 else unpack(bs['data'], max(4, (len(pal)-1).bit_length()), 4096)
                return pal, ids
        return None

    def block(self, x, y, z):
        s = self.section(x//16, y//16, z//16)
        return s[0][s[1][y%16*256+z%16*16+x%16]] if s else None

    def height(self, x, z):
        root = self.chunk(x//16, z//16)
        hm = root.get('Heightmaps', {}).get('OCEAN_FLOOR') if root else None
        if not hm:
            return None
        # Нижняя граница высоты записана в yPos; Энд/Незер начинаются с нуля.
        return unpack(hm, 9, 256)[z%16*16+x%16] + root.get('yPos', -4 if self.dimension == 'overworld' else 0)*16 - 1


def signed(y):
    return y-256 if y > 127 else y


def valid(root, cx, cz):
    if root is None:
        return 'missing'
    if root.get('DataVersion', 4790) != 4790:
        return 'unexpected DataVersion:' + str(root.get('DataVersion'))
    if root.get('Status') not in ('full', 'minecraft:full'):
        return 'status:' + str(root.get('Status'))
    if (root.get('xPos'), root.get('zPos')) != (cx, cz):
        return 'coordinate mismatch'
    if not root.get('sections') or not root.get('Heightmaps', {}).get('OCEAN_FLOOR'):
        return 'missing sections/heightmap'
    bottom = root.get('yPos', 0)
    expected_sections = set(range(bottom, 20 if bottom == -4 else 16))
    present = {signed(s['Y']) for s in root['sections'] if s.get('block_states')}
    if not expected_sections <= present:
        return 'missing block-state sections'
    for sec in root['sections']:
        for name, count, minimum in (('block_states', 4096, 4), ('biomes', 64, 1)):
            value = sec.get(name)
            if not value: continue
            pal = value.get('palette', [])
            if not pal: return 'empty ' + name + ' palette'
            if len(pal) > 1:
                bits=max(minimum, (len(pal)-1).bit_length()); per=64//bits
                if len(value.get('data', [])) != (count+per-1)//per:
                    return 'invalid ' + name + ' packed length'
    if len(root['Heightmaps']['OCEAN_FLOOR']) != 37:
        return 'invalid heightmap packed length'
    return None


def box_chunks(box):
    return {(x, z) for x in range(box[0]//16, box[3]//16+1)
            for z in range(box[2]//16, box[5]//16+1)}


def piece_record(p):
    pool = p.get('pool_element', {})
    return dict(box=p['BB'], name=pool.get('location', p.get('Template', p.get('id', 'unknown'))),
                rotation=p.get('Rot', p.get('rotation', p.get('O'))),
                projection=pool.get('projection'), ground_delta=p.get('ground_level_delta', 0),
                origin=[p.get('PosX'), p.get('PosY'), p.get('PosZ')])


def census(world, dimension):
    w = World(world, dimension)
    area, halo = expected(), expected(8)
    full, issues, starts, bes, entities = set(), [], [], [], []
    exposure = collections.Counter()
    references = collections.Counter()
    # Сортировка по регионам ограничивает память/повторное чтение.
    for cx, cz in sorted(halo, key=lambda p: (p[0]//32, p[1]//32, p[1], p[0])):
        try:
            root = w.read(cx, cz)
            issue = valid(root, cx, cz)
        except (ValueError, IndexError, struct.error, zlib.error) as e:
            root, issue = None, str(e)
        if issue:
            issues.append(dict(chunk=[cx, cz], window=window(cx, cz), reason=issue))
            continue
        full.add((cx, cz))
        if (cx, cz) in area:
            exposure[biome(root, cx*16+8, 64 if dimension == 'overworld' else 32, cz*16+8)] += 1
        for kind, refs in root.get('structures', {}).get('References', {}).items():
            references[kind] += len(refs)
        for kind, s in root.get('structures', {}).get('starts', {}).items():
            ps = [piece_record(p) for p in s.get('Children', []) if 'BB' in p]
            if not ps or s.get('id') in ('INVALID', 'minecraft:invalid'):
                continue
            box = [min(p['box'][i] for p in ps) for i in range(3)] + [max(p['box'][i] for p in ps) for i in range(3, 6)]
            starts.append(dict(kind=kind.removeprefix('minecraft:'), chunk=[cx, cz], window=window(cx, cz),
                               box=box, pieces=ps, biome=biome(root, cx*16+8, ps[0]['box'][1], cz*16+8)))
        for be in root.get('block_entities', []):
            bes.append({k: be[k] for k in ('id', 'x', 'y', 'z', 'LootTable', 'SpawnData', 'SpawnPotentials') if k in be})
    entity_issues = []
    for cx, cz in sorted(full, key=lambda p: (p[0]//32, p[1]//32, p[1], p[0])):
        try:
            root = w.read(cx, cz, 'entities')
        except (ValueError, IndexError, struct.error, zlib.error) as e:
            entity_issues.append(dict(chunk=[cx, cz], reason=str(e)))
            continue
        for e in (root or {}).get('Entities', []):
            entities.append({k:e[k] for k in ('id', 'Pos', 'VillagerData', 'Age', 'PersistenceRequired', 'variant', 'CatType', 'Item', 'AttachFace', 'Facing', 'TileX', 'TileY', 'TileZ', 'HomePosX', 'HomePosY', 'HomePosZ', 'LootTable') if k in e})
    by_be, by_e = collections.defaultdict(list), collections.defaultdict(list)
    for be in bes:
        by_be[be['x']//16, be['z']//16].append(be)
    for e in entities:
        if len(e.get('Pos', [])) == 3:
            by_e[math.floor(e['Pos'][0])//16, math.floor(e['Pos'][2])//16].append(e)
    for s in starts:
        cs = box_chunks(s['box'])
        s['complete_bounds'] = cs <= full
        s['missing_bounds_chunks'] = sorted(cs-full)
        s['block_entities'] = [b for c in cs for b in by_be[c] if inside([b['x'], b['y'], b['z']], s['box'])]
        s['entities'] = [dict(e) for c in cs for e in by_e[c] if inside(e['Pos'], s['box'])]
        for e in s['entities']:
            e['piece_membership'] = [i for i, p in enumerate(s['pieces']) if inside(e['Pos'], p['box'])]
    return dict(dimension=dimension, full_chunks=sorted(full), core_chunks=len(full & area),
                chunks_by_window=dict(collections.Counter(window(*c) for c in full & area)),
                issues=issues, entity_issues=entity_issues, biome_exposure=exposure, references=references,
                starts=starts, block_entities=bes, entities=entities)



def compatible_loot(kind, table):
    table=table.removeprefix('minecraft:')
    if kind.startswith('village_'):return table.startswith('chests/village/')
    if kind.startswith('ruined_portal'):return table=='chests/ruined_portal'
    if kind.startswith('mineshaft'):return table=='chests/abandoned_mineshaft'
    if kind.startswith('shipwreck'):return table.startswith('chests/shipwreck_')
    if kind.startswith('ocean_ruin'):return table.startswith('chests/underwater_ruin_') or table=='archaeology/'+kind
    if kind=='trial_chambers':return '/trial_chambers/' in table
    if kind=='bastion_remnant':return table.startswith('chests/bastion_')
    if kind=='fortress':return table=='chests/nether_bridge'
    if kind=='end_city':return table=='chests/end_city_treasure'
    if kind=='stronghold':return table.startswith('chests/stronghold_')
    if kind=='ancient_city':return table.startswith('chests/ancient_city')
    if kind=='trail_ruins':return table.startswith('archaeology/trail_ruins_')
    if kind=='desert_pyramid':return table in ('chests/desert_pyramid','archaeology/desert_pyramid')
    return table=='chests/'+kind


def attach_loot(data):
    # BB пирамиды не включает подземную сокровищницу: измеренные сундуки
    # на 11 блоков ниже BB. Не приписываем соседней шахте сундуки данжа.
    for s in data['starts']:
        records=list(s['block_entities'])
        if s['kind']=='desert_pyramid':
            b=s['box']
            records=[q for q in data['block_entities'] if b[0]<=q['x']<=b[3] and b[2]<=q['z']<=b[5]]
        s['compatible_loot']=[q for q in records if compatible_loot(s['kind'],q.get('LootTable',''))]
        s['compatible_loot'] += [q for q in s['entities'] if compatible_loot(s['kind'],q.get('LootTable',''))]
        s['loot_outside_saved_box']=[q for q in s['compatible_loot'] if 'x' in q and not inside([q['x'],q['y'],q['z']],s['box'])]
    return data

def geometry(world, data):
    w = World(world, data['dimension'])
    for si, s in enumerate(data['starts']):
        if s['window'] is None:
            continue
        for p in s['pieces']:
            b = p['box']; y = b[1] + p['ground_delta']
            x, z = (b[0]+b[3])//2, (b[2]+b[5])//2
            surface = w.height(x, z)
            p['datum_y'] = y
            p['surface'] = surface
            p['depth'] = surface-y if surface is not None else None
            p['datum_sea_delta'] = y - (63 if data['dimension']=='overworld' else 32 if data['dimension']=='the_nether' else 0)
            # Датум BB не всегда пол: сохраняем блоки, не объявляем воздух дефектом.
            p['base_probes'] = [dict(pos=[px,y,pz], at=w.block(px,y,pz), below=w.block(px,y-1,pz))
                                for px in sorted({b[0], x, b[3]}) for pz in sorted({b[2], z, b[5]})]
            if p['projection'] == 'terrain_matching':
                continue
            rings = {}
            for d in (1, 4, 8, 12, 16):
                ps = [(b[0]-d,z), (b[3]+d,z), (x,b[2]-d), (x,b[5]+d)]
                rings[d] = [None if (h := w.height(px,pz)) is None else h-y for px,pz in ps]
            p['ring_height_minus_datum'] = rings
        if si % 100 == 0:
            print('geometry', data['dimension'], si, flush=True)
    return data


def main():
    a = argparse.ArgumentParser(); a.add_argument('world'); a.add_argument('output'); a.add_argument('--dimension', default='overworld'); a.add_argument('--geometry', action='store_true')
    args = a.parse_args()
    data = attach_loot(census(args.world, args.dimension))
    print(args.dimension, data['core_chunks'], collections.Counter(s['kind'] for s in data['starts'] if s['window']), 'issues', len(data['issues']), flush=True)
    Path(args.output).write_text(json.dumps(data, separators=(',', ':'), sort_keys=True)+'\n')
    if args.geometry:
        geometry(args.world, data)
        Path(args.output).write_text(json.dumps(data, separators=(',', ':'), sort_keys=True)+'\n')

if __name__ == '__main__': main()
