#!/usr/bin/env python3
"""Разбор собственных776-записей, не parser Minecraft реализации.

Проверяем обе гипотезы SpawnInfo по известным командам/режимам.
Счётчик атрибутов выводится из наблюдений, а не ожидаемого числа40.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import uuid


class Reader:
    def __init__(self, data):
        self.data = io.BytesIO(data)

    def take(self, count):
        if not 0 <= count <= 2_097_151:
            raise ValueError("Неверная длина")
        result = self.data.read(count)
        if len(result) != count:
            raise ValueError("Оборванный пакет")
        return result

    def uint(self):
        result = 0
        for shift in range(0, 35, 7):
            byte = self.take(1)[0]
            if shift == 28 and byte & 0xF0:
                raise ValueError("Переполненный VarInt")
            result |= (byte & 127) << shift
            if byte < 128:
                return result
        raise ValueError("Неверный VarInt")

    def count(self, limit):
        value = self.uint()
        if value > limit:
            raise ValueError("Слишком много элементов")
        return value

    def string(self):
        return self.take(self.count(32767)).decode()

    def boolean(self):
        value = self.take(1)[0]
        if value > 1:
            raise ValueError("Неверный Boolean")
        return bool(value)

    def end(self):
        if self.data.read(1):
            raise ValueError("Лишние байты")


def success(payload):
    data = Reader(payload)
    player = uuid.UUID(bytes=data.take(16))
    name = data.string()
    if name != "McsaProbe":
        raise ValueError("Чужой профиль")
    properties = data.count(1024)
    for _ in range(properties):
        data.string(); data.string()
        if data.boolean():
            data.string()
    session = uuid.UUID(bytes=data.take(16))
    data.end()
    if session.int == 0 or session == player:
        raise ValueError("Session UUID отсутствует или равен UUID игрока")
    return {"player_uuid": str(player), "session_uuid": str(session),
            "username": name, "properties": properties}


def spawn_info(data, encoding):
    result = {"dimension_type": data.uint(), "dimension": data.string()}
    data.take(8)  # хеш seed собственного временного мира
    mode = data.uint() if encoding == "optional_varint" else data.take(1)[0]
    raw = data.uint() if encoding == "optional_varint" else data.take(1)[0]
    if encoding == "optional_varint":
        previous = None if raw == 0 else raw - 1
    else:
        previous = None if raw == 255 else raw
    if mode > 3 or (previous is not None and previous not in range(4)):
        raise ValueError("Неверный игровой режим")
    result.update(mode=mode, previous_mode=previous, previous_raw=raw)
    data.boolean(); data.boolean()
    if data.boolean():
        data.string(); data.take(8)
    data.uint(); data.uint()  # portal cooldown; signed sea-level представлен u32
    return result


def game_packet(body, packet, encoding):
    data = Reader(body)
    packet_id = data.uint()
    if packet == "login":
        data.take(4); data.boolean()
        for _ in range(data.count(1024)):
            data.string()
        for _ in range(3):
            data.uint()
        for _ in range(3):
            data.boolean()
    result = spawn_info(data, encoding)
    if packet == "login":
        result["online_mode"] = data.boolean()
        result["enforces_secure_chat"] = data.boolean()
        if result["online_mode"]:
            raise ValueError("Проба требует online-mode=false")
    elif packet == "respawn":
        flags = data.take(1)[0]
        if flags & ~3:
            raise ValueError("Неверные Respawn flags")
        result["flags"] = flags
    else:
        raise ValueError("Неизвестный пакет")
    data.end()
    result["packet_id"] = packet_id
    return result


def attribute_slot(payload, component=16):
    data = Reader(payload)
    if data.uint() != 1:
        raise ValueError("Неверное число предметов")
    item = data.uint()
    if [data.uint(), data.uint(), data.uint(), data.uint()] != [1, 0, component, 1]:
        raise ValueError("Ожидался один явно заданный attribute_modifiers")
    attribute = data.uint()
    if data.string() != "mcsa:probe" or struct.unpack(">d", data.take(8))[0] != 1.0:
        raise ValueError("Чужой modifier")
    if [data.uint(), data.uint(), data.uint()] != [0, 1, 0]:
        raise ValueError("Не совпали operation/mainhand/display")
    data.end()
    return item, attribute


def analyze(login, sessions, attributes):
    if login.get("handshake_protocol") != 776 or login.get("server_exit") != 0:
        raise ValueError("Неуспешная Login-проба776")
    if sessions.get("version") != 776 or sessions.get("exit_code") != 0:
        raise ValueError("Неуспешная Respawn-проба776")
    if attributes.get("protocol") != 776 or attributes.get("exit_code") != 0:
        raise ValueError("Неуспешная Attribute-проба776")
    hashes = {report["jar_sha1"] for report in [login, sessions, attributes]}
    if len(hashes) != 1:
        raise ValueError("Пробы разных JAR")
    packets = [p for p in login["packets"] if "test_profile_payload_hex" in p]
    if len(packets) != 1:
        raise ValueError("Требуется ровно один Login Success")
    profile = success(bytes.fromhex(packets[0]["test_profile_payload_hex"]))
    expected = [
        ("initial", "login", sessions["login_initial_hex"], 1, None),
        ("reconnect_after_survival", "login", sessions["login_after_switch_hex"], 0, 1),
    ]
    respawns = sessions["respawns"]
    if len(respawns) != 4:
        raise ValueError("Требуются четыре Respawn")
    for entry, mode, previous in zip(respawns, [0, 1, 3, 2], [1, 0, 1, 3]):
        expected.append((entry["dimension"], "respawn", entry["body"], mode, previous))
    hypotheses = {}
    failures = {}
    for encoding in ["legacy_byte", "optional_varint"]:
        try:
            values = []
            for label, packet, raw, mode, previous in expected:
                result = game_packet(bytes.fromhex(raw), packet, encoding)
                if (result["mode"], result["previous_mode"]) != (mode, previous):
                    raise ValueError(f"{label}: режимы не соответствуют командам")
                values.append({"case": label, **result})
            hypotheses[encoding] = values
        except ValueError as error:
            failures[encoding] = str(error)
    if len(hypotheses) != 1:
        raise ValueError(f"SpawnInfo гипотеза неоднозначна/неподдержана: {failures}")
    suggestions = attributes["attribute_suggestions"]
    observed = {}
    item_ids = set()
    for case in attributes["cases"]:
        item, attribute = attribute_slot(bytes.fromhex(case["slot_hex"]))
        if case["name"] in observed or attribute in observed.values():
            raise ValueError("Повторяющийся атрибут/ID")
        observed[case["name"]] = attribute
        item_ids.add(item)
    if len(set(suggestions)) != len(suggestions) or set(observed) != set(suggestions) or len(item_ids) != 1:
        raise ValueError("Не все предложенные атрибуты наблюдены")
    return {
        "protocol": 776, "jar_sha1": next(iter(hashes)), "login_success": profile,
        "spawn_info_encoding": next(iter(hypotheses)), "cases": next(iter(hypotheses.values())),
        "rejected_spawn_info_hypotheses": failures,
        "attribute_count": len(observed), "attribute_ids": observed,
        "attribute_ids_dense_from_zero": set(observed.values()) == set(range(len(observed))),
        "scope": "Observed offline Login/Join/Respawn and attributes only; online=true and full gameplay not verified",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["login", "sessions", "attributes", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    inputs = {name: getattr(args, name).read_bytes() for name in ["login", "sessions", "attributes"]}
    result = analyze(*(json.loads(inputs[name]) for name in ["login", "sessions", "attributes"]))
    result["input_sha256"] = {name: hashlib.sha256(body).hexdigest() for name, body in inputs.items()}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"spawn_info": result["spawn_info_encoding"],
                      "attributes": result["attribute_count"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
