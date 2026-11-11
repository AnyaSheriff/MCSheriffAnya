"""Настоящие payload/отсчёты фазы игрока до старта и привязка к оригиналу777."""
import math
import re
import struct
_NAME = 'entity_update_attributes'
_HEX64 = re.compile(r'[0-9a-f]{64}')
_KEYS_C = ('source', 'period', 'residue', 'sample_tick', 'desired_phase', 'attacker')
_KEYS_ROW = ('at', 'name', 'hex')

def _fail(msg):
    raise ValueError("fire_stacking_phase: " + msg)


def _exact(obj, keys, what):
    if not isinstance(obj, dict) or set(obj) != set(keys):
        _fail("схема " + what)


def _num(v):
    # только конечные числа времени (bool числом не считается)
    if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
        _fail("ожидалось конечное число времени")
    return float(v)


def _int(v, what):
    if isinstance(v, bool) or not isinstance(v, int):
        _fail("ожидалось целое: " + what)
    return v


def _varint(data, pos):
    # беззнаковый varint: максимум 5 байт, значение < 2**31
    value = 0
    for i in range(5):
        if pos >= len(data):
            _fail("varint обрезан")
        b = data[pos]
        pos += 1
        value |= (b & 0x7F) << (7 * i)
        if not b & 0x80:
            if value >= 2 ** 31:
                _fail("varint слишком велик")
            return value, pos
    _fail("varint длиннее 5 байт")


def _decode(hex_text, attribute_ids, owner_ids):
    # строгий полный разбор груза: разрешённые атрибуты, точный EOF, без хвостов
    if not isinstance(hex_text, str) or len(hex_text) % 2 or \
            any(ch not in "0123456789abcdefABCDEF" for ch in hex_text):
        _fail("некорректный hex квитанции")
    data, pos = bytes.fromhex(hex_text), 0
    entity, pos = _varint(data, pos)
    if entity < 1 or entity not in owner_ids:
        _fail("чужая сущность в пакете атрибутов")
    count, pos = _varint(data, pos)
    if not 1 <= count <= len(attribute_ids):
        _fail("число атрибутов вне 1..len(attribute_ids)")
    seen, primary = set(), False
    for _ in range(count):
        aid, pos = _varint(data, pos)
        if aid not in attribute_ids or aid in seen:
            _fail("запрещённый или дублированный атрибут")
        seen.add(aid)
        primary = primary or aid == attribute_ids[0]
        if pos + 8 > len(data):
            _fail("обрезано поле base")
        base, pos = struct.unpack_from(">d", data, pos)[0], pos + 8
        if not math.isfinite(base):
            _fail("base не конечен")
        mods, pos = _varint(data, pos)
        if not 0 <= mods <= 16:
            _fail("число модификаторов вне 0..16")
        for _ in range(mods):
            nlen, pos = _varint(data, pos)
            if not 1 <= nlen <= 512 or pos + nlen > len(data):
                _fail("длина имени модификатора")
            try:
                data[pos:pos + nlen].decode("utf-8")
            except UnicodeDecodeError:
                _fail("имя модификатора не utf-8")
            pos += nlen
            if pos + 9 > len(data):
                _fail("обрезаны amount/operation")
            amount, pos = struct.unpack_from(">d", data, pos)[0], pos + 8
            if not math.isfinite(amount) or data[pos] > 2:
                _fail("amount не конечен либо операция вне 0..2")
            pos += 1
    if pos != len(data):
        _fail("хвост после полезного груза")
    if not primary:
        _fail("первичный атрибут отсутствует")



def _required(obj, keys, what):
    if not isinstance(obj, dict) or not set(keys) <= set(obj):
        _fail('отсутствуют обязательные поля: ' + what)


def _validate(raw, attribute_ids):
    _required(raw, ('protocol', 'start', 'clock', 'initial', 'sent', 'clients',
                    'attribute_phase_samples', 'attribute_phase_receipts', 'phase_alignment'), 'raw')
    if raw['protocol'] != 777:
        _fail('нужен protocol777')
    start, clock = _num(raw['start']), _num(raw['clock'])
    initial = raw['initial']
    if not isinstance(initial, list) or len(initial) != 2:
        _fail('нужны два участника')
    owners = []
    for item in initial:
        _required(item, ('entity',), 'initial')
        entity = _int(item['entity'], 'entity')
        if entity <= 0:
            _fail('неверный entity')
        owners.append(entity)
    if owners[0] == owners[1]:
        _fail('участники должны различаться')
    sent = raw['sent']
    if not isinstance(sent, list) or any(not isinstance(v, dict) or not isinstance(v.get('action'), dict) for v in sent):
        _fail('неверные отправленные действия')
    attacks = [v for v in sent if v['action'].get('attack')]
    if len(attacks) != 1 or attacks[0]['action'] != {'attack': True, 'bot': 2, 'after': 20}:
        _fail('нужен один удар второго игрока на20')
    if attacks[0]['action']['attack'] is not True:
        _fail('attack должен быть bool')
    attack_at = _num(attacks[0].get('at'))
    if not start + .95 <= attack_at <= start + 1.25:
        _fail('поздний либо преждевременный удар')
    samples, receipts = raw['attribute_phase_samples'], raw['attribute_phase_receipts']
    if any(not isinstance(v, list) or len(v) != 2 or any(not isinstance(rows, list) for rows in v)
           for v in (samples, receipts)):
        _fail('списки должны быть по клиентам, а не до/после')
    for side, (times, rows) in enumerate(zip(samples, receipts)):
        actual_times = []
        for row in rows:
            _exact(row, _KEYS_ROW, 'receipt')
            when = _num(row['at'])
            if row['name'] != _NAME:
                _fail('неверное имя пакета')
            _decode(row['hex'], attribute_ids, {owners[side]})
            actual_times.append(when)
        numbers = [_num(v) for v in times]
        if actual_times != numbers or any(a >= b for a, b in zip(numbers, numbers[1:])):
            _fail('настоящие квитанции должны совпадать с упорядоченными отсчётами')
    pre = [r for r in receipts[1] if r['at'] < start]
    post = [r for r in receipts[1] if r['at'] >= start]
    if not pre or not any(r['at'] > attack_at for r in post):
        _fail('нет своего пакета до старта либо после настоящего удара')
    align = raw['phase_alignment']
    _required(align, ('policy', 'declared_policy', 'verified', 'observed_constraints',
                      'constraints', 'applied_constraints', 'initial_start_tick',
                      'final_start_tick', 'reference_sha256'), 'phase_alignment')
    if (align['policy'] != 'all' or align['declared_policy'] != 'all'
            or align['verified'] is not True or align['observed_constraints'] != []):
        _fail('фаза не проверена либо исключены ограничения')
    cons = align['constraints']
    if not isinstance(cons, list) or len(cons) != 1 or align['applied_constraints'] != cons:
        _fail('нужно одно действительно применённое ограничение')
    c = cons[0]
    _exact(c, _KEYS_C, 'constraint')
    if c['source'] != 'player_attributes' or _int(c['period'], 'period') != 2 or _int(c['attacker'], 'attacker') != 1:
        _fail('нужна двухтактная фаза клиента1')
    residue, desired = _int(c['residue'], 'residue'), _int(c['desired_phase'], 'desired_phase')
    if residue not in (0, 1) or desired not in (0, 1):
        _fail('неверная чётность')
    tick = math.floor((pre[-1]['at'] - clock) * 20)
    if _int(c['sample_tick'], 'sample_tick') != tick or residue != (tick - 20 + desired) % 2:
        _fail('ограничение не выведено из настоящего последнего пакета')
    lo = _int(align['initial_start_tick'], 'initial_start_tick')
    hi = _int(align['final_start_tick'], 'final_start_tick')
    if lo < 0 or not lo <= hi <= lo + 1 or abs((start - clock) * 20 - hi) > 1e-5 or hi % 2 != residue:
        _fail('реальный старт не удовлетворяет ограничению')
    clients = raw['clients']
    if not isinstance(clients, list) or len(clients) != 2:
        _fail('нужны оба клиента')
    _required(clients[1], ('events',), 'attacker client')
    events = clients[1]['events']
    if not isinstance(events, list) or any(not isinstance(e, dict) for e in events):
        _fail('нет событий атакующего')
    index = 0
    for row in post:
        while index < len(events):
            event = events[index]
            index += 1
            if event.get('name') != _NAME or event.get('hex') != row['hex']:
                continue
            when = _num(event.get('at'))
            if row['at'] <= when <= row['at'] + .05:
                break
        else:
            _fail('послестартовый отсчёт не связан с настоящим пакетом журнала')
    return c, start, pre, post, attack_at


def inspect_phase(raw, attribute_ids, reference=None, reference_hash=None):
    """Проверка именно реальной схемы Recorder; родительские поля не теряются."""
    if (not isinstance(attribute_ids, (list, tuple)) or not attribute_ids
            or any(type(v) is not int or v < 0 for v in attribute_ids)
            or len(set(attribute_ids)) != len(attribute_ids)):
        _fail('неверные закреплённые идентификаторы атрибутов')
    c, start, pre, post, attack_at = _validate(raw, attribute_ids)
    if reference is None:
        if raw['phase_alignment']['reference_sha256'] is not None or c['desired_phase'] != 0:
            _fail('исходная запись должна иметь собственную фазу без ссылки')
    else:
        if not isinstance(reference_hash, str) or not _HEX64.fullmatch(reference_hash):
            _fail('нужен SHA256 байтов настоящего файла оригинала')
        if raw['phase_alignment']['reference_sha256'] != reference_hash:
            _fail('ссылка не совпала с файлом оригинала')
        rc, rstart, _, rpost, rattack = _validate(reference, attribute_ids)
        if reference['phase_alignment']['reference_sha256'] is not None or rc['desired_phase'] != 0:
            _fail('неверная исходная фаза оригинала')
        first = next(r for r in rpost if r['at'] > rstart + 1.)
        if first['at'] < rattack:
            _fail('отсчёт оригинала опередил настоящий удар')
        before = math.floor((first['at'] - reference['clock']) * 20)
        desired = (round((rstart - reference['clock']) * 20) + 20 - before) % 2
        if c['desired_phase'] != desired:
            _fail('фаза отличается от вычисленной по оригиналу')
    return {'pre_samples_count': len(pre), 'sample_tick': c['sample_tick'],
            'residue': c['residue'], 'desired_phase': c['desired_phase'],
            'actual_post_attack_count': sum(r['at'] > attack_at for r in post)}
