#!/usr/bin/env python3
"""Точные JSON-схемы выбранных клиентов. Это инвентаризация, не список поддержки.

Только minecraft-data JSON по неизменяемым commit URL; чужой код не используется.
--download заполняет отдельный кэш, --check сравнивает результат побайтно.
Пример: python3 tools/protocol-bridge/generate_priority_layouts.py \
    --cache /path/to/priority-json --download --check
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import urllib.request


CANDIDATE = ("DallasCarraher/minecraft-data", "924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2")
LEGACY = ("PrismarineJS/minecraft-data", "201ca802a1b2c968b02e7fa5b31dd74e8bff3d8b")
BASELINE = ("PrismarineJS/minecraft-data", "23303d2350177f2084f94cd38e5d7c8e54dfc8d3")
VERSIONS = {"1.12.2": 340, "1.16.5": 754, "1.17": 755, "1.20.1": 763,
            "1.21.1": 767, "1.21.11": 774, "26.1": 775, "26.3": 777}
DEFAULT_OUTPUT = Path(__file__).parent / "testdata/priority-protocol-layouts.json"
PLAY_NAMES = set("""
login respawn keep_alive start_configuration configuration_acknowledged player_loaded
map_chunk update_light unload_chunk chunk_biomes chunk_batch_start chunk_batch_finished
chunk_batch_received block_change multi_block_change tile_entity_data
update_view_position update_view_distance simulation_distance
position position_look look flying teleport_confirm player_rotation vehicle_move
player_input steer_vehicle steer_boat abilities entity_action block_dig block_place use_item
spawn_entity spawn_entity_experience_orb spawn_entity_weather spawn_entity_living
spawn_entity_painting named_entity_spawn entity entity_destroy destroy_entity
entity_metadata entity_velocity entity_equipment entity_teleport sync_entity_position
rel_entity_move entity_move_look entity_look entity_head_rotation entity_status
entity_update_attributes entity_effect remove_entity_effect set_passengers attach_entity
collect use_entity attack animation arm_animation set_projectile_power
window_items set_slot set_cursor_item set_player_inventory window_click set_creative_slot
open_window open_horse_window close_window transaction held_item_slot set_slot_state
select_bundle_item craft_progress_bar craft_recipe_request craft_recipe_response
unlock_recipes declare_recipes recipe_book recipe_book_add recipe_book_remove
recipe_book_settings displayed_recipe enchant_item trade_list select_trade
""".split())


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


class Sources:
    def __init__(self, cache, download, locked):
        self.cache, self.download, self.locked = cache, download, locked
        self.manifest = {}

    def read(self, origin, path):
        repo, commit = origin
        if not re.fullmatch(r"[0-9a-f]{40}", commit):
            raise ValueError("Требуется полный commit SHA")
        # Не допускаем чтение исходников, произвольных файлов или выхода из кэша.
        if path != "data/dataPaths.json" and not re.fullmatch(
                r"data/pc/(?:1\.[0-9]+(?:\.[0-9]+)?|26\.[13])/(?:protocol|version)\.json", path):
            raise ValueError(f"Не разрешён JSON-путь: {path}")
        key = f"{commit}/{path}"
        url = f"https://raw.githubusercontent.com/{repo}/{commit}/{path}"
        target = self.cache / key
        if not target.exists():
            if not self.download:
                raise ValueError(f"Нет {target}; запусти с --download")
            with urllib.request.urlopen(url, timeout=45) as response:
                body = response.read(8 * 1024 * 1024 + 1)
            if len(body) > 8 * 1024 * 1024:
                raise ValueError(f"Слишком большой JSON: {url}")
            json.loads(body)  # Не сохраняем страницу ошибки вместо данных.
            if key in self.locked and digest(body) != self.locked[key]["sha256"]:
                raise ValueError(f"SHA256 скачанного источника не совпал: {key}")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
        body = target.read_bytes()
        entry = {"url": url, "sha256": digest(body), "bytes": len(body)}
        if key in self.locked and entry != self.locked[key]:
            raise ValueError(f"Изменился закреплённый источник: {key}")
        self.manifest[key] = entry
        return json.loads(body), key


def references(schema):
    """Имена типов, а не совпавшие имена полей/значений mapper/compareTo."""
    if isinstance(schema, str):
        if schema != "native":
            yield schema
        return
    if not isinstance(schema, list) or len(schema) != 2 or not isinstance(schema[0], str):
        raise ValueError(f"Неизвестная форма типа: {schema!r}")
    name, args = schema
    yield name
    if name == "option":
        yield from references(args)
    else:
        yield from argument_references(args)


def argument_references(args):
    if isinstance(args, list):
        for value in args:
            yield from argument_references(value)
    elif isinstance(args, dict):
        for key, value in args.items():
            if key in ("type", "countType", "default"):
                yield from references(value)
            elif key == "fields":
                for field in value.values():
                    yield from references(field)
            elif key not in ("mappings", "flags"):
                yield from argument_references(value)


def dependency_types(schema, types):
    pending = list(references(schema))
    found, native, external = {}, set(), set()
    while pending:
        name = pending.pop()
        if name in found or name in native or name in external:
            continue
        if name not in types:
            external.add(name)
            continue
        if types[name] == "native":
            native.add(name)
            continue
        found[name] = types[name]
        pending.extend(references(types[name]))
    return found, sorted(native), sorted(external)


def packet_table(types):
    fields = types["packet"]
    if fields[0] != "container":
        raise ValueError("Неизвестная таблица пакетов")
    names = next(x["type"] for x in fields[1] if x.get("name") == "name")
    params = next(x["type"] for x in fields[1] if x.get("name") == "params")
    if names[0] != "mapper" or params[0] != "switch":
        raise ValueError("Неизвестный формат packet mapper/switch")
    result = {}
    for number, name in names[1]["mappings"].items():
        value = params[1]["fields"][name]
        result[name] = (int(number, 0), types.get(value, value) if isinstance(value, str) else value)
    return result


def build_report(sources):
    paths, paths_key = sources.read(CANDIDATE, "data/dataPaths.json")
    schemas, type_sets, versions = {}, {}, []

    def intern(schema):
        key = digest(canonical(schema))[:16]
        if key in schemas and schemas[key] != schema:
            raise ValueError("Коллизия внутреннего идентификатора схемы")
        schemas[key] = schema
        return key

    for version, expected_number in VERSIONS.items():
        route = paths["pc"][version]
        meta, meta_key = sources.read(LEGACY if version == "1.12.2" else CANDIDATE,
                                      f'data/{route["version"]}/version.json')
        if meta["minecraftVersion"] != version or meta["version"] != expected_number:
            raise ValueError(f"Не совпала точная версия: {version}: {meta}")
        origin = LEGACY if version == "1.12.2" else BASELINE if version == "26.1" else CANDIDATE
        protocol, protocol_key = sources.read(origin, f'data/{route["protocol"]}/protocol.json')
        packets = {}
        for stage in ("handshaking", "login", "configuration", "play"):
            for direction in ("toClient", "toServer"):
                local = protocol.get(stage, {}).get(direction, {}).get("types", {})
                if "packet" not in local:
                    continue
                types = {**protocol["types"], **local}
                for name, (number, schema) in packet_table(local).items():
                    if stage == "play" and name not in PLAY_NAMES:
                        continue
                    definitions, native, external = dependency_types(schema, types)
                    bindings = {key: intern(value) for key, value in sorted(definitions.items())}
                    type_set = {"bindings": bindings, "native_types": native,
                                "external_native_types": external}
                    set_key = digest(canonical(type_set))[:16]
                    if set_key in type_sets and type_sets[set_key] != type_set:
                        raise ValueError("Коллизия внутреннего идентификатора набора типов")
                    type_sets[set_key] = type_set
                    packets[f"{stage}/{direction}/{name}"] = {
                        "id": number, "id_hex": f"0x{number:02x}",
                        "schema": intern(schema), "type_set": set_key,
                    }
        versions.append({"minecraft_version": version, "protocol": expected_number,
                         "protocol_schema_version": route["protocol"].split("/")[-1],
                         "version_source": meta_key, "protocol_source": protocol_key,
                         "configuration_stage": "configuration" in protocol,
                         "packets": packets})

    baseline = next(v for v in versions if v["protocol"] == 775)["packets"]
    for version in versions:
        for name, packet in version["packets"].items():
            old = baseline.get(name)
            if old is None:
                packet["vs_775"] = {"counterpart": "absent_same_name"}
            else:
                packet["vs_775"] = {
                    "id_changed": packet["id"] != old["id"],
                    "schema_changed": packet["schema"] != old["schema"],
                    "dependency_types_changed": packet["type_set"] != old["type_set"],
                }
        version["absent_selected_775_packets"] = sorted(set(baseline) - set(version["packets"]))

    return {
        "format": 1,
        "purpose": "JSON wire inventory only; no runtime support or black-box parity claim",
        "baseline_protocol": 775,
        "internal_references": "First 16 hex digits of SHA256 with collision rejection; source hashes are full SHA256",
        "priority_protocols": [340, 754, 755, 763, 767, 774],
        "data_paths_source": paths_key,
        "comparison": "Exact JSON schema and transitive named types, by state/direction/name; "
                      "not semantic equivalence. External/native codec implementations are not read. "
                      "Registry ID and opaque payload semantics require separate verification.",
        "sources": sources.manifest,
        "versions": versions,
        "schemas": schemas,
        "type_sets": type_sets,
    }


def encode_report(report):
    # По одной компактной записи на схему/пакет: без многократного копирования Slot/NBT.
    lines = ["{"]
    for key in sorted(report):
        value = report[key]
        if key in ("schemas", "type_sets", "sources"):
            lines.append(f"  {json.dumps(key)}: {{")
            entries = [f"    {json.dumps(k)}: {canonical(v).decode()}" for k, v in sorted(value.items())]
            lines.append(",\n".join(entries))
            lines.append("  },")
        elif key == "versions":
            lines.append('  "versions": [')
            entries = []
            for version in value:
                header = {k: v for k, v in version.items() if k != "packets"}
                entry = "    {\n"
                entry += "".join(f"      {json.dumps(k)}: {canonical(v).decode()},\n"
                                 for k, v in sorted(header.items()))
                entry += '      "packets": {\n'
                entry += ",\n".join(f"        {json.dumps(k)}: {canonical(v).decode()}"
                                     for k, v in sorted(version["packets"].items()))
                entry += "\n      }\n    }"
                entries.append(entry)
            lines.append(",\n".join(entries))
            lines.append("  ],")
        else:
            lines.append(f"  {json.dumps(key)}: {canonical(value).decode()},")
    lines[-1] = lines[-1].removesuffix(",")
    return ("\n".join(lines) + "\n}\n").encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--download", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    locked = json.loads(args.output.read_bytes())["sources"] if args.output.exists() else {}
    if args.check and not args.output.exists():
        raise ValueError("--check требует существующий отчёт")
    report = build_report(Sources(args.cache, args.download, locked))
    output = encode_report(report)
    if args.check:
        if args.output.read_bytes() != output:
            raise ValueError("Отчёт отличается от повторно построенного")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(output)
    print(json.dumps({"result": "verified" if args.check else "generated",
                      "sources": len(report["sources"]), "schemas": len(report["schemas"]),
                      "bytes": len(output), "sha256": digest(output),
                      "packets": {v["minecraft_version"]: len(v["packets"]) for v in report["versions"]}}))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError) as error:
        print(f"Ошибка: {error}", file=sys.stderr)
        sys.exit(1)
