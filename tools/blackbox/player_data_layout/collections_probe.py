#!/usr/bin/env python3
"""Однократный finite ORIGINAL-only захват custom-data коллекций (26.3, 777).

Никакого core/Cargo/ретраев/фолбэков старта: один запуск, один исход.
Свой откалиброванный код переиспользуется как замороженные хелперы (lazy
import calibrate/as ref внутри capture), причём replay инвентаря/экипировки
работает НОВЫМ кодеком collection_slots без monkeypatch старых proof-функций.
"""
import argparse
import hashlib
import io
import json
import os
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tarfile
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).parent
FIXTURE_FLAGS = 'white-list=false\ngamemode=creative\n'
PINS = BASE / 'newcollection-pins.json'
CASES = BASE / 'collection-cases.json'
CRITERIA = BASE / 'collection-criteria.json'
WORKFLOW = '.github/workflows/custom-data-collections-original.yml'
ACCEPTED_REPLY = 'Replaced 1 slot(s) on blackbox with [Stone]'

from collection_slots import SlotDecoder, read_varint  # noqa: E402 — наш новый кодек


# ---------- чистые классификаторы (тестируются офлайн) ----------

def classify_response(reply):
    """Только точное 'Replaced 1 slot(s)...' — accepted; непустой ответ без
    префикса 'Replaced' — rejected-observed; всё остальное — invalid."""
    if reply == ACCEPTED_REPLY:
        return 'accepted'
    if reply and not reply.startswith('Replaced'):
        return 'rejected-observed'
    return 'invalid'


def exact_marker(item, kind, case_id):
    """Точный typed-маркер {type:8, value:'KIND_id'} и ничего ужерного в нём;
    CONTROL-маркер никогда не принимается за NEW (защита от ложного успеха)."""
    if not isinstance(item, dict):
        return False
    data = item.get('custom_data')
    keys = {'marker'} if kind == 'CONTROL' else {'marker', 'payload'}
    if kind not in ('CONTROL', 'NEW') or not isinstance(data, dict) or set(data) != keys:
        return False
    marker = data['marker']
    return marker == {'type': 8, 'value': '%s_%s' % (kind, case_id)}


def stale_new_marker(item, case_id):
    """Чужой NEW-маркер (утечка состояния между делами) — немедленный отказ."""
    if not isinstance(item, dict):
        return False
    data = item.get('custom_data')
    marker = data.get('marker') if isinstance(data, dict) else None
    return (isinstance(marker, dict) and marker.get('type') == 8
            and isinstance(marker.get('value'), str)
            and marker['value'].startswith('NEW_')
            and marker['value'] != 'NEW_' + case_id)


def classify_case(response_class, evidence_kept):
    """Исход дела валиден только при согласованности класса ответа и
    фактического байт-точного доказательства."""
    if response_class == 'accepted' and evidence_kept is True:
        return 'accepted'
    if response_class == 'rejected-observed' and evidence_kept is True:
        return 'rejected-observed'
    return 'invalid'


# ---------- офлайн-реплей на новом кодеке ----------

def inventory_snapshot(events, items, menu_slot):
    """Фактическая реплика полного 46-слотового инвентаря и последующих слотов."""
    parser = SlotDecoder(items)
    slots = {}
    seed = False
    receipts = []
    for e in events:
        b = io.BytesIO(bytes.fromhex(e['hex']))
        name = e['name']
        if name == 'window_items':
            window = read_varint(b.read)
            revision = read_varint(b.read)
            n = read_varint(b.read)
            values = [parser.slot(b) for _ in range(n)]
            cursor = parser.slot(b)
            assert not b.read()
            if window == 0:
                assert n == 46, 'фактический full46 обязателен'
                slots = dict(enumerate(values))
                seed = True
                receipts.append(dict(at=e['at'], kind='full46', revision=revision, cursor=cursor))
        elif name == 'set_player_inventory':
            slot = menu_slot(read_varint(b.read))
            value = parser.slot(b)
            assert not b.read()
            if seed:
                slots[slot] = value
                receipts.append(dict(at=e['at'], kind='slot', slot=slot, value=value))
        elif name == 'set_slot':
            window = read_varint(b.read)
            revision = read_varint(b.read)
            sidx = struct.unpack('>h', b.read(2))[0]
            value = parser.slot(b)
            assert not b.read()
            if window == 0 and seed:
                slots[sidx] = value
                receipts.append(dict(at=e['at'], kind='slot', slot=sidx, value=value))
    assert seed and len(slots) == 46, 'нет полного 46-слотового seed инвентаря'
    return dict(slots=[slots[i] for i in range(46)], receipts=receipts)


def equipment_snapshot(events, items):
    """Пакеты entity_equipment: предмет в слотах наблюдаемой сущности."""
    parser = SlotDecoder(items)
    out = []
    for e in events:
        if e['name'] != 'entity_equipment':
            continue
        b = io.BytesIO(bytes.fromhex(e['hex']))
        entity = read_varint(b.read)
        slots = []
        while True:
            tag = b.read(1)
            assert tag
            sidx = tag[0]
            slots.append(dict(slot=sidx & 127, item=parser.slot(b)))
            if not sidx & 128:
                break
        assert not b.read()
        out.append(dict(entity=entity, slots=slots, at=e['at'], hex=e['hex']))
    return out


def observer_map(events, items, entity):
    """Карта slot->item из видe наблюдателя для конкретной сущности."""
    observed = {}
    for packet in equipment_snapshot(events, items):
        if packet['entity'] == entity:
            for row in packet['slots']:
                observed[row['slot']] = row['item']
    return observed


def through_receipt(events, receipt):
    """Префикс реального потока до конкретного пакета маркера включительно."""
    positions = [i for i, event in enumerate(events) if event == receipt]
    assert len(positions) == 1, 'маркер отсутствует или неоднозначен в потоке'
    return events[:positions[0] + 1]


def snapshots_at_barrier(events, barrier, items, menu_slot, entity):
    assert len(events) == len(barrier['receipts']) == 2
    bounded = [through_receipt(rows, receipt)
               for rows, receipt in zip(events, barrier['receipts'])]
    return (inventory_snapshot(bounded[0], items, menu_slot),
            observer_map(bounded[1], items, entity),
            [equipment_snapshot(rows, items) for rows in bounded])


def finalize_case(before, events, barrier, items, menu_slot, entity, case_id, outcome):
    """Повторная проверка ПОСЛЕ END; предварительного снимка недостаточно."""
    after, observed, gear = snapshots_at_barrier(events, barrier, items, menu_slot, entity)
    slot = after['slots'][36]
    assert observed.get(0) == slot, 'состояние наблюдателя на END расходится'
    if outcome == 'accepted':
        assert (slot is not None and slot['item'] == 'stone' and slot['count'] == 1
                and exact_marker(slot, 'NEW', case_id)), 'NEW не подтверждён на END'
        expected = list(before['slots'])
        expected[36] = slot
        assert after['slots'] == expected, 'изменились другие ячейки'
    else:
        assert outcome == 'rejected-observed'
        assert after['slots'] == before['slots'], 'отклонение изменило инвентарь на END'
        assert exact_marker(slot, 'CONTROL', case_id), 'CONTROL не подтверждён на END'
    return after, gear


# ---------- запуск ----------

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def preflight(output):
    """Только GitHub Actions, попытка 1, чистая цель, все хэши frozen-данных."""
    assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('GITHUB_RUN_ATTEMPT') == '1'
    assert not Path(output).exists()
    pin = read(PINS)
    for name, digest in pin['source_files'].items():
        assert sha(ROOT / name) == digest, 'замороженный исход изменился: ' + name
    assert sha(CASES) == pin['cases_sha256'], 'кейсы не соответствуют закреплению'
    assert sha(CRITERIA) == pin['criteria_sha256'], 'критерии не соответствуют закреплению'
    return pin


def archive_names(pin):
    """Архив собственных исходников: закреплённые файлы + новые файлы захвата
    + замороженные данные + НАШ путь workflow (старый путь-константа не в archive)."""
    names = list(pin['source_files'])
    for name in ['collection_slots.py', 'collections_probe.py', 'test_collections.py',
                 'collection-cases.json', 'collection-criteria.json', 'newcollection-pins.json']:
        path = BASE / name
        if path.is_file():
            assert path.is_relative_to(ROOT), 'собственный файл вне ROOT'
            names.append(str(path.relative_to(ROOT)))
    names.append(WORKFLOW)
    return sorted(set(names))


def capture(output):
    pin = preflight(output)
    signal.signal(signal.SIGALRM, lambda *_: (_ for _ in ()).throw(TimeoutError('finite capture deadline')))
    signal.alarm(300)
    output = Path(output).resolve()
    output.mkdir()
    work = output / 'original'
    work.mkdir()
    # Ленивые импорты собственного кода: офлайн-тесты не требуют репозитория.
    sys.path.insert(0, str(ROOT / 'tools/blackbox'))
    import record777 as own
    import combat_ci
    from rcon_transport import IsolatedRcon
    from combat_process import stop_process, ports_free, process_identity
    from probe_player_actions_runtime import menu_slot
    try:
        import calibrate as ref
    except ImportError:
        sys.path.insert(0, str(ROOT / 'tools/blackbox/player_data_layout'))
        import calibrate as ref
    cases = read(CASES)
    criteria = read(CRITERIA)
    assert cases['schema'] == criteria['schema'], 'frozen-данные не согласованы'
    assert criteria['original_only'] is True and criteria['max_seconds'] == 300
    assert criteria['ports'] == [26230, 26231]
    clients = []
    configurations = []
    join_proofs = []
    startup_attempts = []
    cleanup_control = None
    proc = None
    control = None
    log = None
    life = {}
    result = dict(status='INVALID', original_only=True, core_executed=False, compiled=False)
    commands = []
    collected = []
    try:
        result['source'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        for port in criteria['ports']:
            with socket.socket() as check:
                check.bind(('127.0.0.1', port))
        source = read(ROOT / 'tools/blackbox/ci/protection-edges-777.json')
        data = output / 'data'
        data.mkdir()
        combat_ci.fetch(source['official_jar'], work / 'server.jar')
        for entry in source['json_files']:
            if entry['file'] in ['protocol.json', 'items.json', 'entities.json', 'blocks.json']:
                combat_ci.fetch(entry, data / entry['file'])
        (work / 'eula.txt').write_text('eula=true\n')
        (work / 'server.properties').write_text(FIXTURE_FLAGS + 'server-ip=127.0.0.1\nserver-port=26230\n'
            'enable-rcon=true\nrcon.port=26231\nrcon.password=local-layout-fixture\nonline-mode=false\n'
            'enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n'
            'generate-structures=false\nspawn-protection=0\nallow-flight=true\nview-distance=2\n'
            'simulation-distance=2\nmax-tick-time=0\n')
        log = (work / 'server.log').open('w')
        argv = ['java', '-Xmx1G', '-XX:ActiveProcessorCount=2', '-jar', 'server.jar', 'nogui']
        proc = subprocess.Popen(argv, cwd=work, stdin=subprocess.DEVNULL, stdout=log,
                                stderr=subprocess.STDOUT, start_new_session=True)
        life['identity'] = process_identity(proc.pid)
        life['argv'] = argv
        deadline = time.monotonic() + 120
        startup_done = False
        while time.monotonic() < deadline:
            assert proc.poll() is None
            startup_done = 'Done (' in (work / 'server.log').read_text()
            if startup_done:
                break
            time.sleep(.1)
        assert startup_done, 'до готовность "Done" не достигнута'
        candidate = IsolatedRcon(26231, 'local-layout-fixture')
        cleanup_control = candidate
        try:
            startup_status = ref.semantic_startup(candidate, startup_done, lambda: ref.status_query(26230))
        except BaseException as error:
            startup_attempts.append(dict(error=repr(error), calls=candidate.calls))
            raise
        control = candidate
        write(output / 'startup-status.json', startup_status)
        protocol = read(data / 'protocol.json')
        items = read(data / 'items.json')
        for name in ['blackbox', 'blackboxb']:
            configuration = []
            def receive_configuration(sock, packet_id, payload):
                configuration.append(dict(id=packet_id, hex=payload.hex()))
                return False
            client = own.connect(26230, protocol, lambda s, p: ref.RawClient(s, p), version=777,
                                 username=name, configuration_handler=receive_configuration)
            client.start()
            clients.append(client)
            configurations.append(configuration)
        for client in clients:
            deadline = time.monotonic() + 5
            while True:
                assert client.failure is None
                try:
                    join_proofs.append(ref.creative_readiness(client.events, protocol))
                    break
                except ref.MissingJoinProof:
                    assert time.monotonic() < deadline, 'нет фактического Login/Abilities proof'
                    time.sleep(.01)
        nonce = 'RUN' + os.environ['GITHUB_RUN_ID']
        def command(text, role):
            row = dict(role=role, command=text, issued=time.monotonic())
            row['response'] = control.run(text)
            row['completed'] = time.monotonic()
            commands.append(row)
            return row
        for text in ref.SETUP:
            ref.validate_setup_reply(text, command(text, 'setup')['response'])
        entity = clients[0].entity
        for case in cases['cases']:
            case_id = case['id']
            clients[0].send('held_item_slot', struct.pack('>h', 0))
            control_command = ('item replace entity blackbox hotbar.0 with '
                               'minecraft:stone[custom_data={marker:"CONTROL_%s"}] 1' % case_id)
            candidate_command = ('item replace entity blackbox hotbar.0 with '
                                 'minecraft:stone[custom_data={marker:"NEW_%s",payload:%s}] 1'
                                 % (case_id, case['payload']))
            row_control = command(control_command, 'control')
            assert row_control['response'] == ACCEPTED_REPLY, 'CONTROL-команда не принята'
            limit = time.monotonic() + 3
            while True:
                before = inventory_snapshot(clients[0].events, items, menu_slot)
                observed = observer_map(clients[1].events, items, entity)
                slot = before['slots'][36]
                if (slot is not None and slot['item'] == 'stone' and slot['count'] == 1
                        and exact_marker(slot, 'CONTROL', case_id) and observed.get(0) == slot):
                    break
                assert time.monotonic() < limit, 'CONTROL-доказательство не подтверждено'
                time.sleep(.01)
            begin = ref.barrier(clients, control, nonce + '_CASE_' + case_id.replace('-', '_').upper() + '_BEGIN')
            before, observed, _ = snapshots_at_barrier(
                [c.events for c in clients], begin, items, menu_slot, entity)
            slot = before['slots'][36]
            assert (slot is not None and slot['item'] == 'stone' and slot['count'] == 1
                    and exact_marker(slot, 'CONTROL', case_id) and observed.get(0) == slot)
            control_raw = slot['custom_data_raw_hex']
            row_case = command(candidate_command, 'candidate')
            klass = classify_response(row_case['response'])
            if klass == 'invalid':
                raise AssertionError('кандидат %s: серый INVALID ответ %r' % (case_id, row_case['response']))
            limit = time.monotonic() + 3
            while True:
                after = inventory_snapshot(clients[0].events, items, menu_slot)
                observed = observer_map(clients[1].events, items, entity)
                slot_new = after['slots'][36]
                if klass == 'accepted':
                    if (slot_new is not None and slot_new['item'] == 'stone'
                            and slot_new['count'] == 1 and exact_marker(slot_new, 'NEW', case_id)
                            and observed.get(0) == slot_new):
                        outcome = 'accepted'
                        break
                    if stale_new_marker(slot_new, case_id):
                        raise AssertionError('утечка состояния: чужой NEW-маркер %r' % (slot_new,))
                elif (slot_new is not None and exact_marker(slot_new, 'CONTROL', case_id)
                        and slot_new.get('custom_data_raw_hex') == control_raw
                        and after['slots'] == before['slots']
                        and observed.get(0) == slot_new):
                    outcome = 'rejected-observed'
                    break
                assert time.monotonic() < limit, 'исход кандидата %s не подтверждён' % case_id
                time.sleep(.01)
            assert classify_case(klass, True) == outcome, 'несогласованный исход'
            end = ref.barrier(clients, control, nonce + '_CASE_' + case_id.replace('-', '_').upper() + '_END')
            after, case_equipment = finalize_case(
                before, [c.events for c in clients], end, items, menu_slot, entity, case_id, outcome)
            queries = [command(q, 'query') for q in cases['queries']]
            collected.append(dict(id=case_id, selected=0, response_class=klass, outcome=outcome,
                                  control=dict(command=control_command, response=row_control['response']),
                                  candidate=dict(command=candidate_command, response=row_case['response']),
                                  control_raw_hex=control_raw, before=before, after=after,
                                  begin=begin, end=end, queries=queries,
                                  equipment=case_equipment))
        result.update(status='COMPLETE-custom-data-collection-discovery-not-parity', cases=collected)
    except BaseException:
        (output / 'error.txt').write_text(traceback.format_exc())
        raise
    finally:
        signal.alarm(0)
        write(output / 'commands.json', commands)
        write(output / 'startup-attempts.json', startup_attempts)
        write(output / 'configuration.json', configurations)
        write(output / 'join-readiness.json', join_proofs)
        write(output / 'cases.json', cases)
        write(output / 'criteria.json', criteria)
        write(output / 'pins.json', pin)
        write(output / 'raw-clients.json',
              [dict(entity=c.entity, events=c.events, wire_sent=c.wire_sent, failure=c.failure) for c in clients])
        for client in clients:
            client.close()
        if proc is not None:
            life['cleanup_errors'] = []
            life['cleanup'] = stop_process(proc, ref.CleanupAdapter(cleanup_control, life['cleanup_errors'])
                                           if cleanup_control else None, identity=life['identity'])
        write(output / 'raw-rcon-frames.json', cleanup_control.calls if cleanup_control else [])
        used = ref.imported_own_files()
        missing = [n for n in used if n not in pin['source_files']]
        if missing:
            result.update(status='INVALID', unfrozen_imports=missing)
        write(output / 'own-import-closure.json', used)
        if log:
            log.close()
        life['ports_free'] = ports_free(criteria['ports'])
        if not all(life['ports_free'].values()) or (proc is not None and
                (life['cleanup']['exit_code'] != 0 or not life['cleanup']['group_gone'])):
            result.update(status='INVALID', cleanup_failed=True)
        with tarfile.open(output / 'own-source.tar.gz', 'w:gz') as archive:
            for name in archive_names(pin):
                archive.add(ROOT / name, arcname=name)
        write(output / 'lifecycle.json', life)
        write(output / 'result.json', result)
        ref.finalize_manifest(output)
    if result['status'].startswith('COMPLETE'):
        assert all(life['ports_free'].values()) and life['cleanup']['group_gone']
        first = output / 'FIRST-CUSTOM-DATA'
        first.mkdir(exist_ok=False)
        for name in ['result.json', 'commands.json', 'raw-clients.json', 'lifecycle.json',
                     'raw-rcon-frames.json', 'configuration.json', 'criteria.json', 'cases.json',
                     'pins.json', 'startup-status.json', 'own-import-closure.json', 'own-source.tar.gz']:
            shutil.copy2(output / name, first / name)
        ref.finalize_manifest(output)
    else:
        raise RuntimeError('технический INVALID сохранён; без повторов')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    capture(args.output.resolve())
