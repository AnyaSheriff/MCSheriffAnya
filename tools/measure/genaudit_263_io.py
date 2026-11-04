"""Read-only adapters for measured worlds, never game archives or data packs."""
import ctypes
import ctypes.util
import gzip
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import zlib

import numpy as np
import census_263 as c

REPO = Path(__file__).resolve().parents[2]


def level(path):
    raw = (Path(path) / 'level.dat').read_bytes()
    if raw.startswith(b'\x1f\x8b'):
        raw = gzip.decompress(raw)
        root, _ = c.nbt(raw, 3 + int.from_bytes(raw[1:3], 'big'), 10)
        d = root.get('Data', root)
        settings = d.get('WorldGenSettings', {})
        spawn = d.get('spawn', {})
        seed = settings.get('seed', d.get('RandomSeed'))
        evidence = Path(path).parent / 'observed-seed.json'
        if seed is None and evidence.exists():
            seed = json.loads(evidence.read_text())['seed']
        return {'seed': seed,
                'spawn': [d.get('SpawnX', spawn.get('pos', [0, 0, 0])[0]),
                          d.get('SpawnZ', spawn.get('pos', [0, 0, 0])[-1])],
                'version': d.get('Version'), 'data_version': d.get('DataVersion')}
    fields = {k.strip(): v.strip() for k,v in (line.split('=', 1) for line in raw.decode().splitlines() if '=' in line)}
    spawn = fields.get('spawn', '0 64 0').split()
    return {'seed': int(fields['seed']), 'spawn': [math.floor(float(spawn[0])), math.floor(float(spawn[2]))],
            'version': 'MCSA custom level.dat'}


def registry():
    """Our checked-in catalog is authoritative for internal MCS state numbers."""
    text = (REPO / 'src/blocks_table.rs').read_text().split('pub const BLOCK_NAMES:', 1)[1].split('];', 1)[0]
    ranges = [(int(a), int(b), n) for a, b, n in re.findall(r'\((\d+), (\d+), "([^"]+)"\)', text)]
    text = (REPO / 'tools/protocol-bridge/src/content.rs').read_text()
    ranges += [(int(a), int(b), n) for n, a, b in re.findall(
        r'name: "([^"]+)"[^\n]*?first_state: (\d+), last_state: (\d+)', text)]
    terrain = (REPO / 'src/world/terrain.rs').read_text()
    names = dict(re.findall(r'Biome::(\w+) => "([^"]+)"', terrain))
    biomes = []
    for group in ('ALL', 'NETHER', 'END', 'PREVIEW'):
        part = terrain.split('pub const ' + group + ':', 1)[1].split('];', 1)[0]
        biomes += [names[n] for n in re.findall(r'Biome::(\w+)', part)]
        if group == 'ALL':
            # The one custom biome occupies a persisted slot between ALL and NETHER.
            custom = re.search(r'pub const AUTUMN_FOREST: &str = "([^"]+)"', terrain)
            biomes.append(custom[1] if custom else 'autumn_forest')
    climate = {n: [float(t), float(h)] for n, t, h in re.findall(
        r'"([^"]+)" => \((-?[\d.]+), (-?[\d.]+)\)', terrain)}
    return ranges, biomes, climate


def region_dir(world, dimension):
    world = Path(world)
    candidates = [world / 'dimensions/minecraft' / dimension / 'region']
    if dimension == 'overworld':
        candidates += [world / 'region', world]
    else:
        legacy = 'DIM-1' if dimension == 'the_nether' else 'DIM1'
        candidates += [world / legacy / 'region', world / dimension / 'region']
    return next((p for p in candidates if p.is_dir() and (list(p.glob('*.mca')) or list(p.glob('*.mcs')))), candidates[0])


def packed(longs, bits, count):
    words = np.asarray(longs, dtype=np.int64).view(np.uint64)
    indices = np.arange(count)
    return ((words[indices // (64 // bits)] >> ((indices % (64 // bits)) * bits).astype(np.uint64)) & ((1 << bits) - 1)).astype(np.uint16)


def container(raw, pos, count):
    kind = raw[pos]
    pos += 1
    if kind == 0:
        return np.zeros(count, dtype=np.uint16), pos
    if kind == 1:
        return np.full(count, int.from_bytes(raw[pos:pos+2], 'big'), dtype=np.uint16), pos + 2
    if kind not in (2, 3):
        raise ValueError(f'Unknown MCS container {kind}')
    n = count if kind == 3 else int.from_bytes(raw[pos:pos+2], 'big')
    pos += 0 if kind == 3 else 2
    palette = np.frombuffer(raw, dtype='>u2', count=n, offset=pos).astype(np.uint16)
    pos += n * 2
    if kind == 3:
        return palette, pos
    bits = (n - 1).bit_length()
    bits = 4 if bits == 3 else 8 if 5 <= bits <= 7 else bits
    size = (count * bits + 7) // 8
    # MCS uses contiguous little-endian packing, unlike Anvil's padded longs.
    values = np.frombuffer(raw[pos:pos+size], dtype=np.uint8)
    indices = np.arange(count) * bits
    result = np.zeros(count, dtype=np.uint16)
    for bit in range(bits):
        at = indices + bit
        result |= (((values[at // 8] >> (at % 8)) & 1).astype(np.uint16) << bit)
    return palette[result], pos + size


class Reader:
    def __init__(self):
        ranges, self.biomes, self.climate = registry()
        self.names = ['air'] + sorted({n for _, _, n in ranges} - {'air'})
        self.lookup = {n: i for i, n in enumerate(self.names)}
        self.states = np.full(65536, 65535, dtype=np.uint16)
        for a, b, n in ranges:
            self.states[a:b+1] = self.lookup[n]
        self.horizontal_states=np.zeros(65536,dtype=bool)
        for block in json.loads((REPO/'tools/data/blocks.json').read_text()):
            if not block['name'].endswith(('_log','_wood','_stem','_hyphae')):
                continue
            properties=block.get('states',[])
            axis=next((s for s in properties if s['name']=='axis'),None)
            if axis and axis.get('values')==['x','y','z'] and block['maxStateId']-block['minStateId']==2:
                self.horizontal_states[block['minStateId']]=True
                self.horizontal_states[block['maxStateId']]=True
        for a,b,name in ranges:
            if name.startswith('poplar_') and name.endswith(('_log','_wood')) and b-a==2:
                self.horizontal_states[a]=True
                self.horizontal_states[b]=True
        self.lib = ctypes.CDLL(ctypes.util.find_library('zstd'))
        self.lib.ZSTD_decompress.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
        self.lib.ZSTD_decompress.restype = ctypes.c_size_t
        self.dest = ctypes.create_string_buffer(64 << 20)

    def code(self, name):
        name = name.removeprefix('minecraft:')
        if name not in self.lookup:
            self.lookup[name] = len(self.names)
            self.names.append(name)
        return self.lookup[name]

    def read(self, file, wanted=None):
        data = file.read_bytes()
        is_mcs = file.suffix == '.mcs'
        if is_mcs and data[:5] != b'MCS\x01\x01':
            raise ValueError(f'Unsupported MCS header: {file}')
        rx, rz = map(int, file.stem.split('.')[1:])
        for slot in range(1024):
            cx, cz = rx * 32 + slot % 32, rz * 32 + slot // 32
            if wanted is not None and (cx, cz) not in wanted:
                continue
            if is_mcs:
                off, size = struct.unpack_from('>II', data, 8 + slot * 8)
                if not size:
                    continue
                record = data[off * 256:off * 256 + size]
                crc, n = struct.unpack_from('>II', record)
                if n != len(record) - 8 or zlib.crc32(record[8:]) != crc:
                    raise ValueError(f'Invalid MCS CRC: {file}:{slot}')
                size = self.lib.ZSTD_decompress(self.dest, len(self.dest), record[8:], n)
                if size > len(self.dest):
                    raise ValueError(f'Invalid MCS compression: {file}:{slot}')
                raw = ctypes.string_at(self.dest, size)
                if raw[0] not in (4, 5):
                    raise ValueError(f'Unknown MCS chunk version {raw[0]}')
                pos = 1
                sections = []
                horizontal={}
                for _ in range(24):
                    values, pos = container(raw, pos, 4096)
                    ids = self.states[values]
                    if np.any(ids == 65535):
                        raise ValueError(f'Unknown internal states: {np.unique(values[ids == 65535])}')
                    sections.append(ids.reshape(16, 16, 16))
                    for code,count in zip(*np.unique(ids[self.horizontal_states[values]],return_counts=True)):
                        name=self.names[code]
                        horizontal[name]=horizontal.get(name,0)+int(count)
                bios = []
                for _ in range(24):
                    values, pos = container(raw, pos, 64)
                    bios.append([self.biomes[i] for i in values])
                yield cx, cz, np.concatenate(sections), bios, {'_mcs':True,'_audit_horizontal_logs':horizontal}
            else:
                off = int.from_bytes(data[slot*4:slot*4+3], 'big')
                if not off:
                    continue
                pos = off * 4096
                size = int.from_bytes(data[pos:pos+4], 'big')
                kind = data[pos+4]
                body = data[pos+5:pos+4+size]
                if kind == 2:
                    raw = zlib.decompress(body)
                elif kind == 1:
                    raw = gzip.decompress(body)
                elif kind == 3:
                    raw = body
                else:
                    raise ValueError(f'Unsupported Anvil compression {kind}: {file}:{slot}')
                root, _ = c.nbt(raw, 3 + int.from_bytes(raw[1:3], 'big'), 10)
                if root.get('Status') not in ('full', 'minecraft:full'):
                    continue
                if (root['xPos'], root['zPos']) != (cx, cz):
                    raise ValueError(f'Anvil coordinate mismatch: {file}:{slot}')
                blocks = np.zeros((384, 16, 16), dtype=np.uint16)
                bios = [['unknown'] * 64 for _ in range(24)]
                horizontal={}
                for sec in root.get('sections', []):
                    si = sec['Y'] + 4
                    if not 0 <= si < 24:
                        continue
                    bs = sec.get('block_states')
                    if bs:
                        palette = [self.code(p if isinstance(p, str) else p.get('id') or p.get('Name') or p.get('name') or p.get('')) for p in bs['palette']]
                        ids = np.zeros(4096, dtype=np.uint16) if len(palette) == 1 else packed(bs['data'], max(4, (len(palette)-1).bit_length()), 4096)
                        blocks[si*16:si*16+16] = np.asarray(palette, dtype=np.uint16)[ids].reshape(16, 16, 16)
                        for pi,p in enumerate(bs['palette']):
                            props=(p.get('properties') or p.get('Properties') or {}) if isinstance(p,dict) else {}
                            name=self.names[palette[pi]]
                            if name.endswith(('_log','_wood','_stem','_hyphae')) and props.get('axis') in ('x','z'):
                                horizontal[name]=horizontal.get(name,0)+int((ids==pi).sum())
                    if 'biomes' in sec:
                        bios[si] = [n.removeprefix('minecraft:') for n in c.biome_cells(sec)]
                root['_audit_horizontal_logs']=horizontal
                yield cx, cz, blocks, bios, root


def manifest(world):
    out = []
    for dim in ('overworld', 'the_nether', 'the_end'):
        folder = region_dir(world, dim)
        for file in sorted(list(folder.glob('*.mca')) + list(folder.glob('*.mcs'))):
            out.append({'path': str(file.resolve()), 'size': file.stat().st_size,
                        'sha256': hashlib.sha256(file.read_bytes()).hexdigest()})
    return out
