"""Собственный офлайн-декодер original NBT + Slot (протокол 777, 26.3 custom-data).

Только stdlib, никакого внешнего кода и никакого core/Cargo. Все 12 типов NBT;
границы: глубина 32, бюджет узлов 65536, совокупный размер 65535. Строковые
фикстуры только ASCII (не-ASCII отклоняется явно); скаляры со знаком точные;
float/double хранят сырой bit-hex и конечное значение; массивы — типизированные
списки целых; список — {element_type, values} с сохранением порядка; поля
compound — {type, value}. custom_data_raw_hex сохраняется байт в байт.
"""
import math
import struct

MAX_DEPTH = 32
MAX_NODES = 65536
MAX_TOTAL = 65535


def _exact(b, n, what):
    # Точное чтение n байт: усечение потока — явный отказ.
    v = b.read(n)
    if len(v) != n:
        raise AssertionError('NBT усечён: %s требует %d байт' % (what, n))
    return v


def read_varint(read):
    """Минимальный собственный varint (только stdlib): знаковый 32-бит."""
    value = 0
    shift = 0
    size = 0
    while True:
        byte = read(1)
        if not byte or len(byte) != 1:
            raise AssertionError('varint усечён')
        size += 1
        if size > 5:
            raise AssertionError('varint длиннее 5 байт')
        if size == 5 and byte[0] & 0xf0:
            raise AssertionError('varint выходит за 32 бита')
        value |= (byte[0] & 0x7f) << shift
        if not byte[0] & 0x80:
            break
        shift += 7
    if value >= 1 << 31:
        value -= 1 << 32
    return value


class _Ctx(object):
    __slots__ = ('nodes', 'start')

    def __init__(self, start):
        self.nodes = 0
        self.start = start


def _node(ctx, b, depth, extra=0):
    """Один ставка тега: бюджет узлов, глубина, совокупный объём."""
    ctx.nodes += 1
    if ctx.nodes > MAX_NODES:
        raise AssertionError('бюджет узлов NBT исчерпан (65536)')
    if depth > MAX_DEPTH:
        raise AssertionError('глубина NBT превышена (' + str(MAX_DEPTH) + ')')
    if b.tell() - ctx.start + extra > MAX_TOTAL:
        raise AssertionError('совокупный объём NBT превышен (' + str(MAX_TOTAL) + ')')


def _string(b, ctx):
    # Строковый тег/ключ compound: длина unsigned short, тело в UTF-8.
    length = struct.unpack('>H', _exact(b, 2, 'длина строки'))[0]
    if b.tell() - ctx.start + length > MAX_TOTAL:
        raise AssertionError('совокупный объём NBT превышен: строка')
    raw = _exact(b, length, 'строка')
    value = raw.decode('utf8', 'strict')
    if not value.isascii() or '\0' in value:
        raise AssertionError('не-ASCII строка отклонена')
    return value


def _payload(b, kind, depth, ctx):
    """Один тег NBT: возвращает {type, value} (для 5/6 ещё raw_hex)."""
    _node(ctx, b, depth)
    if kind == 0:
        return {'type': 0, 'value': None}
    if kind == 1:
        v = _exact(b, 1, 'BYTE')[0]
        return {'type': 1, 'value': v - 256 if v >= 128 else v}
    if kind == 2:
        return {'type': 2, 'value': struct.unpack('>h', _exact(b, 2, 'SHORT'))[0]}
    if kind == 3:
        return {'type': 3, 'value': struct.unpack('>i', _exact(b, 4, 'INT'))[0]}
    if kind == 4:
        return {'type': 4, 'value': struct.unpack('>q', _exact(b, 8, 'LONG'))[0]}
    if kind == 5:
        raw = _exact(b, 4, 'FLOAT')
        value = struct.unpack('>f', raw)[0]
        if not math.isfinite(value):
            raise AssertionError('FLOAT не конечен')
        return {'type': 5, 'value': value, 'raw_hex': raw.hex()}
    if kind == 6:
        raw = _exact(b, 8, 'DOUBLE')
        value = struct.unpack('>d', raw)[0]
        if not math.isfinite(value):
            raise AssertionError('DOUBLE не конечен')
        return {'type': 6, 'value': value, 'raw_hex': raw.hex()}
    if kind == 7:
        return {'type': 7, 'value': _array(b, 1, '>b', ctx)}
    if kind == 8:
        return {'type': 8, 'value': _string(b, ctx)}
    if kind == 9:
        elem = _exact(b, 1, 'LIST element_type')[0]
        if elem not in range(13):
            raise AssertionError('LIST содержит неизвестный тип элемента %d' % elem)
        length = struct.unpack('>i', _exact(b, 4, 'LIST длина'))[0]
        if length < 0:
            raise AssertionError('отрицательная длина LIST')
        if elem == 0 and length:
            raise AssertionError('LIST с элементами TAG_End')
        if length > MAX_NODES - ctx.nodes:
            raise AssertionError('длина LIST превышает бюджет узлов')
        values = [_payload(b, elem, depth + 1, ctx) for _ in range(length)]
        return {'type': 9, 'element_type': elem, 'values': values}
    if kind == 10:
        return {'type': 10, 'value': _fields(b, depth + 1, ctx)}
    if kind == 11:
        return {'type': 11, 'value': _array(b, 4, '>i', ctx)}
    if kind == 12:
        return {'type': 12, 'value': _array(b, 8, '>q', ctx)}
    raise AssertionError('неизвестный тип NBT %d' % kind)


def _array(b, width, fmt, ctx):
    """Массив байтов/int/long: типизированный список целых со знаком."""
    length = struct.unpack('>i', _exact(b, 4, 'длина массива'))[0]
    if length < 0:
        raise AssertionError('отрицательная длина массива')
    _node(ctx, b, 0, length * width)
    raw = _exact(b, length * width, 'тело массива')
    return [x[0] for x in struct.iter_unpack(fmt, raw)] if length else []


def _fields(b, depth, ctx):
    """Поле compound: dict ключ -> {type, value}; дубликаты ключей отклоняются."""
    out = {}
    while True:
        kind = _exact(b, 1, 'код тега compound')[0]
        if kind == 0:
            return out
        if kind not in range(13):
            raise AssertionError('неизвестный тип NBT %d в compound' % kind)
        key = _string(b, ctx)
        if key in out:
            raise AssertionError('дубликат ключа NBT: ' + key)
        out[key] = _payload(b, kind, depth, ctx)


def decode_tag(b, kind=None):
    """Один тег NBT с границы потока; kind=0..12 (по умолчанию читается сам)."""
    ctx = _Ctx(b.tell())
    if kind is None:
        kind = _exact(b, 1, 'код тега')[0]
    if kind not in range(13):
        raise AssertionError('неизвестный тип NBT %d' % kind)
    out = _payload(b, kind, 0, ctx)
    if b.tell() - ctx.start > MAX_TOTAL:
        raise AssertionError('совокупный объём NBT превышен')
    return out


def decode_custom_data(b):
    """custom_data: маркер 0x0a + поля compound до TAG_End; возвращает поля."""
    ctx = _Ctx(b.tell())
    if _exact(b, 1, 'custom_data префикс')[0] != 10:
        raise AssertionError('custom_data требует TAG_Compound(10)')
    ctx.nodes += 1
    fields = _fields(b, 1, ctx)
    if b.tell() - ctx.start > MAX_TOTAL:
        raise AssertionError('совокупный объём NBT превышен')
    return fields


class SlotDecoder(object):
    """Слот инвентаря 777: count<=99; компоненты ТОЛЬКО 0(custom_data),
    3(damage), 13(enchantments); removed-компоненты не допускаются."""

    def __init__(self, items):
        # Фактическая известная таблица предметов 777 из items.json.
        self.names = {x['id']: x['name'] for x in items}

    def slot(self, b):
        count = read_varint(b.read)
        if count == 0:
            return None
        if not 0 < count <= 99:
            raise AssertionError('count слота вне 1..99: %d' % count)
        item = read_varint(b.read)
        if item not in self.names:
            raise AssertionError('неизвестный id предмета %d' % item)
        added = read_varint(b.read)
        removed = read_varint(b.read)
        if removed != 0:
            raise AssertionError('removed-компоненты не допускаются')
        if not 0 <= added <= 8:
            raise AssertionError('слишком много added-компонентов: %d' % added)
        out = {'item': self.names[item], 'count': count}
        seen = set()
        for _ in range(added):
            kind = read_varint(b.read)
            if kind in seen:
                raise AssertionError('дубликат компонента %d' % kind)
            seen.add(kind)
            if kind == 0:
                start = b.tell()
                out['custom_data'] = decode_custom_data(b)
                end = b.tell()
                out['custom_data_raw_hex'] = b.getvalue()[start:end].hex()
            elif kind == 3:
                out['damage'] = read_varint(b.read)
            elif kind == 13:
                n = read_varint(b.read)
                if not 0 <= n <= MAX_NODES:
                    raise AssertionError('длина enchantments вне бюджета')
                out['enchantments'] = [[read_varint(b.read), read_varint(b.read)]
                                       for _ in range(n)]
            else:
                raise AssertionError('неизвестный компонент слота %d (только 0,3,13)' % kind)
        return out
