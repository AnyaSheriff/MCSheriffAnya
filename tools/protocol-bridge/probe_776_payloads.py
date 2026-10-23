"""Ограниченные проверки собственных776 случаев. Не общий protocol client."""
import io
import math
import struct
from pathlib import Path
import hashlib
import json

from probe_block_entities import BLOCKS, exact, nbt
from smoke import read_varint
from generate_registry_26_2 import CANDIDATE, DEFAULT_OUTPUT, LOCAL_LOCKS, Sources
from generate_tables import states


def table(cache, name):
    if cache.name in ('771','772','773'):
        locks=json.loads((Path(__file__).resolve().parent/'client-chain-sources.json').read_text())['versions'][cache.name]['files']
        record=locks[name];raw=(cache/(name+'.json')).read_bytes()
        assert len(raw)==record['bytes'] and hashlib.sha256(raw).hexdigest()==record['sha256']
        return json.loads(raw)
    if (cache / 'protocol.json').is_file():
        manifest = json.loads((DEFAULT_OUTPUT.parent / 'step1-sources-774.json').read_text())
        if name in manifest:
            raw=(cache / (name+'.json')).read_bytes()
            assert hashlib.sha256(raw).hexdigest()==manifest[name]['sha256'] and len(raw)==manifest[name]['bytes']
            return json.loads(raw)
    locked = json.loads(DEFAULT_OUTPUT.read_text())['sources']
    source = Sources(cache, False, locked)
    return source.read(CANDIDATE, 'data/pc/26.2/' + name + '.json')


def authored_states(cache):
    # Только устойчивые строительные блоки и axis logs. Без NBT, жидкости,
    # gravity/redstone/растений. Множество ограничено общими775/776 именами.
    middle = states(table(cache, 'blocks'))
    root = DEFAULT_OUTPUT.parents[2]
    path = root / 'tools/data/blocks.json'
    raw = path.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == LOCAL_LOCKS['tools/data/blocks.json']
    old = states(json.loads(raw))
    suffixes = ('_planks', '_wool', '_terracotta', '_concrete', '_ore', '_bricks',
                '_stained_glass', '_log', '_wood', '_stem', '_hyphae', '_block')
    plain = {'granite','polished_granite','diorite','polished_diorite','andesite','polished_andesite',
             'cobblestone','mossy_cobblestone','obsidian','crying_obsidian','glass','sandstone',
             'chiseled_sandstone','cut_sandstone','smooth_sandstone','red_sandstone',
             'chiseled_red_sandstone','cut_red_sandstone','smooth_red_sandstone','tuff','calcite',
             'netherrack','terracotta','bricks','blackstone','polished_blackstone','smooth_stone'}
    excluded_entities = set(BLOCKS) | {'test_instance_block', 'test_block'}
    selected = [(key, number) for key, number in middle.items()
                if key in old and key[0] not in excluded_entities
                and (key[0].endswith(suffixes) or key[0] in plain)
                and 'coral' not in key[0] and key[0] not in ('moss_block', 'pale_moss_block', 'snow_block')
                and all(name == 'axis' for name, _ in key[1])]
    selected.sort(key=lambda row: row[1])
    assert len(selected) >= 299, len(selected)
    stone = middle['stone', ()]
    expected = [stone] * 4096
    commands = ['fill -16 96 -16 -1 111 -1 minecraft:stone']
    for i, ((name, props), number) in enumerate(selected[:299]):
        state = name + ('[' + ','.join(k + '=' + v for k, v in props) + ']' if props else '')
        commands.append(f'setblock {-16 + i % 16} {96 + i // 256} {-16 + (i // 16) % 16} minecraft:{state}')
        expected[i] = number
    assert len(set(expected)) == 300
    return commands, expected


def palette(data, count, minimum, maximum):
    width = exact(data, 1)[0]
    if width == 0:
        return width, [read_varint(data.read)] * count
    assert minimum <= width <= 16, ('invalid palette width', width)
    indirect = width <= maximum
    entries = []
    if indirect:
        size = read_varint(data.read)
        assert 0 < size <= 1 << width
        entries = [read_varint(data.read) for _ in range(size)]
    per_word = 64 // width
    words = [int.from_bytes(exact(data, 8), 'big') for _ in range((count + per_word - 1) // per_word)]
    values = []
    for word in words:
        for shift in range(min(per_word, count - len(values))):
            value = word >> (shift * width) & ((1 << width) - 1)
            if indirect:
                assert value < len(entries), 'palette index out of bounds'
                value = entries[value]
            values.append(value)
    return width, values


def authored_section(payload, expected):
    data = io.BytesIO(payload)
    result = None
    for index in range(24):
        start = data.tell()
        blocks_present, fluids_present = struct.unpack('>HH', exact(data, 4))
        assert blocks_present <= 4096 and fluids_present <= 4096
        width, blocks = palette(data, 4096, 4, 8)
        palette(data, 64, 1, 3)  # биомы лишь пропускаем, не записываем
        if index == 10:  # y96 при min_y=-64
            result = {'section_y': 96, 'width': width, 'blocks_present': blocks_present,
                      'fluids_present': fluids_present, 'section_hex': payload[start:data.tell()].hex(),
                      'unique_states': len(set(blocks)), 'expected_prefix': expected[:299],
                      'remaining_state': expected[-1]}
            assert width == 15, ('observed direct width differs', result)
            assert blocks_present == 4096 and fluids_present == 0
            differences = [(i, a, b) for i, (a, b) in enumerate(zip(expected, blocks)) if a != b]
            assert not differences, ('authored states changed', differences[:20])
    assert not data.read(), 'trailing chunk section bytes'
    return result


def metadata(body):
    data = io.BytesIO(body)
    read_varint(data.read)  # packet id
    entity = read_varint(data.read)
    fields = {}
    while True:
        index = exact(data, 1)[0]
        if index == 255:
            break
        assert index not in fields
        kind = read_varint(data.read)
        if kind == 0:
            value = struct.unpack('>b', exact(data, 1))[0]
        elif kind in (1, 19, 20, 23, 24):
            value = read_varint(data.read)
        elif kind == 3:
            value = struct.unpack('>f', exact(data, 4))[0]
            assert math.isfinite(value)
        elif kind == 8:
            value = exact(data, 1)[0]
            assert value in (0, 1)
        elif kind == 6:
            present = exact(data, 1)[0]
            assert present in (0, 1)
            value = nbt(data) if present else None
        elif kind == 11:
            present = exact(data, 1)[0]
            assert present in (0, 1)
            value = exact(data, 8).hex() if present else None
        elif kind == 17:
            assert read_varint(data.read) == 0, 'nonempty particles unexpected in authored entity'
            value = []
        else:
            raise AssertionError(('unexpected metadata serializer', index, kind))
        fields[index] = {'serializer': kind, 'value': value}
    assert not data.read(), 'trailing metadata bytes'
    return entity, fields


def motion(name, body):
    data = io.BytesIO(body)
    read_varint(data.read)
    entity = read_varint(data.read)
    if name in ('rel_entity_move', 'entity_move_look'):
        value = {'delta': list(struct.unpack('>hhh', exact(data, 6)))}
        if name == 'entity_move_look':
            value['angles'] = list(exact(data, 2))
        value['on_ground'] = exact(data, 1)[0]
    elif name == 'sync_entity_position':
        value = {'position': list(struct.unpack('>ddd', exact(data, 24))),
                 'velocity': list(struct.unpack('>ddd', exact(data, 24))),
                 'rotation': list(struct.unpack('>ff', exact(data, 8))),
                 'on_ground': exact(data, 1)[0]}
        assert all(math.isfinite(x) for k in ['position', 'velocity', 'rotation'] for x in value[k])
    else:
        raise AssertionError(('unexpected movement packet', name))
    assert value['on_ground'] in (0, 1) and not data.read(), 'movement layout differs'
    return {'entity': entity, **value}


def validate_slots(report, cases, cache):
    assert report['protocol'] == 776 and report['exit_code'] == 0
    items = {row['name']: row['id'] for row in table(cache, 'items')}
    assert len(report['cases']) == len(cases)
    result = []
    for record, case in zip(report['cases'], cases):
        assert record['name'] == case['name'] and record['command'] == case['command']
        data = io.BytesIO(bytes.fromhex(record['slot_hex']))
        count = read_varint(data.read)
        assert count == case['expected_count']
        if count == 0:
            assert not data.read()
            result.append({'name': case['name'], 'count': 0})
            continue
        item = read_varint(data.read)
        assert item == items[case['expected_item']]
        added, removed = read_varint(data.read), read_varint(data.read)
        assert added <= 16 and removed == 0
        values = {}
        for _ in range(added):
            kind = read_varint(data.read)
            assert kind not in values
            if kind in (3, 85):
                value = read_varint(data.read)
            elif kind == 6:
                value = nbt(data)
            elif kind == 40:
                value = [read_varint(data.read), read_varint(data.read)]
            elif kind == 45:
                value = struct.unpack('>i', exact(data, 4))[0]
            elif kind == 16:
                assert read_varint(data.read) == 1
                attribute = read_varint(data.read)
                name = exact(data, read_varint(data.read)).decode()
                amount = struct.unpack('>d', exact(data, 8))[0]
                operation, slot, display = [read_varint(data.read) for _ in range(3)]
                assert name == 'mcsa:probe' and amount == 1.0 and [operation, slot, display] == [0, 1, 0]
                value = attribute
            else:
                raise AssertionError(('unexpected authored slot component', case['name'], kind))
            values[str(kind)] = value
        assert not data.read(), 'slot trailing bytes'
        assert values == case['expected_components'], (case['name'], values, case['expected_components'])
        result.append({'name': case['name'], 'count': count, 'item_id': item, 'components': values})
    return result
