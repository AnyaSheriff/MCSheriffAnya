"""Собственный ограниченный наблюдатель пакетов 777; неизвестное не считается успехом."""
import copy
import io
import math
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'protocol-bridge'))
from probe_gameplay import Client, connect


class Reader:
    def __init__(self, payload): self.data = io.BytesIO(payload)
    def exact(self, n):
        if not 0 <= n <= 1048576: raise ValueError('length bound')
        value = self.data.read(n)
        if len(value) != n: raise ValueError('truncated payload')
        return value
    def unpack(self, fmt): return struct.unpack('>' + fmt, self.exact(struct.calcsize('>' + fmt)))
    def uint(self, limit=5):
        value = 0
        for i in range(limit):
            b = self.exact(1)[0]; value |= (b & 127) << (7*i)
            if b < 128:
                if value >= 1 << (32 if limit == 5 else 64): raise ValueError('varint overflow')
                return value
        raise ValueError('varint bound')
    def boolean(self):
        b = self.exact(1)[0]
        if b > 1: raise ValueError('invalid boolean')
        return bool(b)
    def floats(self, fmt):
        values = list(self.unpack(fmt))
        if not all(math.isfinite(v) for v in values): raise ValueError('nonfinite value')
        return values
    def end(self):
        if self.data.read(1): raise ValueError('trailing bytes')
    def lpvec(self):
        first = self.exact(1)[0]
        if first == 0: return [0., 0., 0.]
        rest = self.exact(5)
        packed = first | rest[0] << 8 | int.from_bytes(rest[1:], 'big') << 16
        scale = (packed & 3) + (self.uint() << 2 if packed & 4 else 0)
        return [(((packed >> (3+15*i)) & 32767)/32766*2-1)*scale for i in range(3)]
    def nbt(self, tag=None, depth=0):
        # Пропуск только сетевого NBT, с пределами размера и вложенности.
        if depth > 32: raise ValueError('NBT depth')
        if tag is None: tag = self.exact(1)[0]
        sizes = {0:0, 1:1, 2:2, 3:4, 4:8, 5:4, 6:8}
        if tag in sizes: self.exact(sizes[tag])
        elif tag == 8: self.exact(self.unpack('H')[0])
        elif tag in (7, 11, 12): self.exact(self.unpack('i')[0] * {7:1,11:4,12:8}[tag])
        elif tag == 9:
            child = self.exact(1)[0]; count = self.unpack('i')[0]
            if not 0 <= count <= 65536: raise ValueError('NBT list bound')
            for _ in range(count): self.nbt(child, depth+1)
        elif tag == 10:
            for _ in range(65536):
                child = self.exact(1)[0]
                if child == 0: return
                self.exact(self.unpack('H')[0]); self.nbt(child, depth+1)
            raise ValueError('NBT compound bound')
        else: raise ValueError('unknown NBT tag')


def angle(byte): return byte * 360 / 256


class MobObserver(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.phase = 'initial'; self.events = []; self.entities = {}
        self.entity_types = {}; self.max_events = 100000; self.counts = {}
    def set_entity_types(self, records): self.entity_types = {r['id']: r for r in records}
    def snapshot(self, entity_id): return copy.deepcopy(self.entities.get(entity_id))
    def marked_events(self, begin_index): return copy.deepcopy(self.events[begin_index:])
    def observe_packet(self, name, payload):
        relevant = {'spawn_entity','entity_metadata','rel_entity_move','entity_move_look',
                    'entity_look','entity_head_rotation','sync_entity_position','entity_teleport',
                    'entity_velocity','damage_event','entity_status','entity_destroy','hurt_animation',
                    'sound_effect','entity_sound_effect','named_sound_effect','world_particles'}
        if name not in relevant: return
        if len(self.events) >= self.max_events: raise RuntimeError('mob event budget exhausted')
        event = dict(index=len(self.events), phase=self.phase, name=name, payload_hex=payload.hex())
        self.counts[name] = self.counts.get(name, 0)+1
        try:
            self.decode(name, Reader(payload), event)
            event.setdefault('status', 'decoded')
        except (ValueError, struct.error, OverflowError, UnicodeError) as error:
            event.update(status='decode_error', error=str(error))
        self.events.append(event)
    def decode(self, name, r, e):
        if name in ('sound_effect','entity_sound_effect','named_sound_effect','world_particles'):
            e.update(status='counted_raw'); return
        if name == 'entity_destroy':
            count = r.uint()
            if count > 10000: raise ValueError('removal count bound')
            e['entities'] = [r.uint() for _ in range(count)]; r.end()
            for entity in e['entities']:
                if entity in self.entities: self.entities[entity]['removed'] = True
            return
        entity = r.unpack('i')[0] if name == 'entity_status' else r.uint()
        e['entity'] = entity
        state = copy.deepcopy(self.entities.get(entity, {'entity':entity}))
        if name == 'spawn_entity':
            state['uuid'] = r.exact(16).hex(); state['entity_type'] = r.uint()
            state['position'] = r.floats('ddd'); state['velocity'] = r.lpvec()
            pitch, yaw, head = r.exact(3)
            state.update(pitch=angle(pitch), yaw=angle(yaw), head_yaw=angle(head), object_data=r.uint(), removed=False)
            kind = self.entity_types.get(state['entity_type'])
            if kind: state['entity_name'] = kind['name']
        elif name == 'entity_velocity': state['velocity'] = r.lpvec()
        elif name in ('rel_entity_move','entity_move_look'):
            properties = r.uint(); count = properties >> 1
            if count > 1024: raise ValueError('relative steps bound')
            steps = []
            for _ in range(count or 1):
                tick = r.uint() if count else 0; delta = list(r.unpack('hhh'))
                steps.append(dict(delta=delta, tick_offset=tick))
                if 'position' in state:
                    state['position'] = [math.floor(x*4096+.5)/4096+d/4096 for x,d in zip(state['position'],delta)]
            e['steps'] = steps; state['on_ground'] = bool(properties & 1)
            if name == 'entity_move_look': state['yaw'], state['pitch'] = map(angle, r.exact(2))
        elif name == 'sync_entity_position':
            tag = r.uint()
            if tag not in (0,1): raise ValueError('unknown position type')
            count = r.uint() if tag else 1
            if not 1 <= count <= 1024: raise ValueError('absolute steps bound')
            steps = []
            for _ in range(count):
                pos = r.floats('ddd'); tick = r.uint() if tag else 0
                steps.append(dict(position=pos, tick_offset=tick))
            state['position'] = steps[-1]['position']; e['steps'] = steps
            state['yaw'], state['pitch'] = r.floats('ff'); state['on_ground'] = r.boolean()
        elif name == 'entity_teleport':
            pos = r.floats('ddd'); velocity = r.floats('ddd'); yaw,pitch = r.floats('ff')
            flags = r.unpack('I')[0]; ground = r.boolean(); e['flags'] = flags
            if flags:
                e.update(status='unparsed_relative_teleport', position=pos, velocity=velocity, yaw=yaw,pitch=pitch)
            else: state.update(position=pos,velocity=velocity,yaw=yaw,pitch=pitch,on_ground=ground)
        elif name == 'entity_look':
            state['yaw'], state['pitch'] = map(angle, r.exact(2)); state['on_ground'] = r.boolean()
        elif name == 'entity_head_rotation': state['head_yaw'] = angle(r.exact(1)[0])
        elif name == 'entity_status': e['entity_status'] = r.unpack('b')[0]
        elif name == 'hurt_animation': e['yaw'] = r.floats('f')[0]
        elif name == 'damage_event':
            e['source_type'],e['source_cause'],e['source_direct'] = r.uint(),r.uint(),r.uint()
            if r.boolean(): e['source_position'] = r.floats('ddd')
        elif name == 'entity_metadata':
            self.metadata(r, state, e)
            if e.get('status') == 'unparsed_metadata':
                self.entities[entity] = state; e['state'] = copy.deepcopy(state); return
        r.end(); self.entities[entity] = state; e['state'] = copy.deepcopy(state)
    def metadata(self, r, state, e):
        entries = []; e['metadata'] = entries
        keys = self.entity_types.get(state.get('entity_type'), {}).get('metadataKeys', [])
        for _ in range(256):
            index = r.exact(1)[0]
            if index == 255: return
            kind = r.uint(); start = r.data.tell(); value = None
            if kind == 0: value = r.unpack('b')[0]
            elif kind in (1,2): value = r.uint(10 if kind == 2 else 5)
            elif kind == 3: value = r.floats('f')[0]
            elif kind == 4: value = r.exact(r.uint()).decode('utf-8')
            elif kind == 5: r.nbt()
            elif kind == 6:
                if r.boolean(): r.nbt()
            elif kind == 8: value = r.boolean()
            elif kind in (9,39,40): value = r.floats('ffff' if kind == 40 else 'fff')
            elif kind == 10: value = r.unpack('q')[0]
            elif kind == 11:
                if r.boolean(): value = r.unpack('q')[0]
            elif kind in (12,14,15,19,20,21,22,23,24,25,26,27,28,29,30,31,32,35,36,37,38,42,43): value = r.uint()
            elif kind == 13:
                if r.boolean(): value = r.exact(16).hex()
            elif kind == 18: value = [r.uint(),r.uint(),r.uint()]
            else:
                e.update(status='unparsed_metadata', unparsed=dict(index=index,type=kind,remaining_hex=r.data.read().hex()))
                return
            key = keys[index] if index < len(keys) else None
            entries.append(dict(index=index,type=kind,key=key,value=value,raw_hex=r.data.getvalue()[start:r.data.tell()].hex()))
            expected = {'health':3, 'shared_flags':0, 'air_supply':1, 'no_gravity':8}
            if key in expected:
                if kind != expected[key]: raise ValueError('metadata symbolic type mismatch')
                state[key] = value
        raise ValueError('metadata entries bound')
