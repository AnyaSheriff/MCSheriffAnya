#!/usr/bin/env python3
"""Кандидатные таблицы 775↔776 из разрешённых JSON, без включения runtime.

--download наполняет отдельный кэш, --check проверяет воспроизводимость.
Атрибуты сохраняются как непроверенные данные, не как рабочее соответствие ID.
"""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
CANDIDATE = ("DallasCarraher/minecraft-data", "924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2")
BASELINE = ("PrismarineJS/minecraft-data", "23303d2350177f2084f94cd38e5d7c8e54dfc8d3")
ENTITIES = ("PrismarineJS/minecraft-data", "58d24389309b26db59454cf8b7813ad688beeb55")
DEFAULT_OUTPUT = Path(__file__).with_name("registry-candidate-26.2.json")
LOCAL_LOCKS = {
    "tools/data/blocks.json": "573e172619f3c1ca64e15d07799ab646e1fd9ad19d759d9ceca65ac4fc7440a9",
    "tools/data/items.json": "0d085dbf8a6fa129b94f696b77286d65ce7b04476eab398492558ab03cc1c45e",
}
REMOTE_LOCKS = {
    f"{CANDIDATE[1]}/data/dataPaths.json": "d03e54fb162884979381f29a01ef3c0557d75b452a74c4e19af7c56e8791b15e",
    f"{CANDIDATE[1]}/data/pc/26.2/protocol.json": "a6c9bd7240f74b94eb0d9b1497294e4cfaa4bb02c0023817c952429149e2d50c",
    f"{CANDIDATE[1]}/data/pc/26.2/version.json": "e4a731efdc228a6dafd61eb842e8eb76f9d6b766979254d743e161b86d7c1d0c",
    f"{BASELINE[1]}/data/pc/26.1/protocol.json": "0bb4f020fa45cc6629748f09546ec2bfd645f1843cccac9da2cab9d0539ab7e3",
    f"{ENTITIES[1]}/data/pc/26.1/entities.json": "e65d88a15fac5fc090cb2555c8cddaa80d9daf8c554d252680cec60a1d6eefc3",
}
ALLOWED = {"blocks", "items", "entities", "particles", "sounds", "attributes", "protocol", "version"}


def sha(body):
    return hashlib.sha256(body).hexdigest()


class Sources:
    def __init__(self, cache, download, locked):
        self.cache, self.download, self.locked = cache, download, locked
        self.manifest = {}

    def read(self, origin, path):
        repo, commit = origin
        match = re.fullmatch(r"data/pc/26\.[12]/([A-Za-z]+)\.json", path)
        if path != "data/dataPaths.json" and (not match or match[1] not in ALLOWED):
            raise ValueError(f"Запрещён источник: {path}")
        key = f"{commit}/{path}"
        target = self.cache / key
        url = f"https://raw.githubusercontent.com/{repo}/{commit}/{path}"
        if not target.exists():
            if not self.download:
                raise ValueError(f"Нет кэша {key}; требуется --download")
            with urllib.request.urlopen(url, timeout=45) as response:
                body = response.read(8 * 1024 * 1024 + 1)
            if len(body) > 8 * 1024 * 1024:
                raise ValueError("Слишком большой JSON")
            json.loads(body)
            expected = self.locked.get(key, {}).get("sha256", REMOTE_LOCKS.get(key))
            if expected and sha(body) != expected:
                raise ValueError(f"Изменился закреплённый источник {key}")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
        body = target.read_bytes()
        record = {"url": url, "bytes": len(body), "sha256": sha(body)}
        if key in REMOTE_LOCKS and record["sha256"] != REMOTE_LOCKS[key]:
            raise ValueError(f"Неверный SHA256 {key}")
        if key in self.locked and record != self.locked[key]:
            raise ValueError(f"Изменился источник {key}")
        self.manifest[key] = record
        return json.loads(body)

    def local(self, path):
        body = (ROOT / path).read_bytes()
        if sha(body) != LOCAL_LOCKS[path]:
            raise ValueError(f"Изменился внутренний baseline {path}")
        self.manifest[path] = {"bytes": len(body), "sha256": sha(body)}
        return json.loads(body)


def named(entries):
    result = {}
    ids = set()
    for entry in entries:
        name, number = entry["name"], entry["id"]
        if name in result or number in ids or not isinstance(number, int) or number < 0:
            raise ValueError("Повторяющиеся или неверные имена/ID")
        result[name] = number
        ids.add(number)
    return result


def mapper(value):
    if value[0] != "mapper":
        raise ValueError("Ожидается JSON mapper")
    entries = [{"name": name, "id": int(number, 0) if number.startswith("0x") else int(number)}
               for number, name in value[1]["mappings"].items()]
    return named(entries)


def block_states(blocks):
    result = {}
    for block in blocks:
        properties = []
        for prop in block["states"]:
            values = prop.get("values")
            if values is None:
                if prop["type"] == "bool":
                    values = ["true", "false"]
                elif prop["type"] == "int":
                    values = list(range(prop["num_values"]))
                else:
                    raise ValueError(f"Неизвестное состояние {block['name']}/{prop['name']}")
            if len(values) != prop["num_values"]:
                raise ValueError("Неверное число значений свойства")
            properties.append([(prop["name"], str(value)) for value in values])
        combinations = list(itertools.product(*properties))
        if len(combinations) != block["maxStateId"] - block["minStateId"] + 1:
            raise ValueError("Неверный диапазон block states")
        for offset, props in enumerate(combinations):
            # Состояния сравниваются по всем свойствам, не только имени блока.
            key = (block["name"], tuple(sorted(props)))
            if key in result:
                raise ValueError("Повторяющееся состояние")
            result[key] = block["minStateId"] + offset
    if len(set(result.values())) != len(result):
        raise ValueError("Пересекающиеся ID состояний")
    return result


def label(key):
    if isinstance(key, str):
        return key
    name, props = key
    return name + ("[" + ",".join(f"{k}={v}" for k, v in props) + "]" if props else "")


def comparison(old, new):
    common = old.keys() & new.keys()
    forward = [-1] * (max(old.values()) + 1)
    backward = [-1] * (max(new.values()) + 1)
    for name in common:
        forward[old[name]] = new[name]
        backward[new[name]] = old[name]
    for name in common:
        if backward[forward[old[name]]] != old[name]:
            raise ValueError("Нарушено обратное соответствие")
    return {
        "baseline_count": len(old), "candidate_count": len(new),
        "matched": len(common), "renumbered": sum(old[n] != new[n] for n in common),
        "baseline_ids_present": sorted(old.values()), "candidate_ids_present": sorted(new.values()),
        "forward_775_to_776": forward, "backward_776_to_775": backward,
        "added": [{"name": label(n), "id": new[n]} for n in sorted(new.keys() - old.keys())],
        "removed": [{"name": label(n), "id": old[n]} for n in sorted(old.keys() - new.keys())],
    }


def records_delta(old, new):
    a = {entry["name"]: entry for entry in old}
    b = {entry["name"]: entry for entry in new}
    def different(left, right, key):
        return (key in left) != (key in right) or left.get(key) != right.get(key)
    return {
        "added": [b[n] for n in sorted(b.keys() - a.keys())],
        "removed": [a[n] for n in sorted(a.keys() - b.keys())],
        "changed_shared_fields": [
            {"name": n, "fields": {
                key: {"baseline": a[n].get(key), "candidate": b[n].get(key),
                      "baseline_present": key in a[n], "candidate_present": key in b[n]}
                for key in sorted(a[n].keys() | b[n].keys())
                if key != "id" and different(a[n], b[n], key)}}
            for n in sorted(a.keys() & b.keys())
            if any(key != "id" and different(a[n], b[n], key)
                   for key in a[n].keys() | b[n].keys())
        ],
    }


def build(sources):
    paths = sources.read(CANDIDATE, "data/dataPaths.json")["pc"]["26.2"]
    candidate = {}
    for kind in sorted(ALLOWED):
        path = paths[kind]
        if path != "pc/26.2":
            raise ValueError(f"Неожиданный источник26.2 {kind}: {path}")
        candidate[kind] = sources.read(CANDIDATE, f"data/{path}/{kind}.json")
    if candidate["version"]["version"] != 776 or candidate["version"]["minecraftVersion"] != "26.2":
        raise ValueError("Неверная версия")
    baseline = {
        "blocks": sources.local("tools/data/blocks.json"),
        "items": sources.local("tools/data/items.json"),
        "entities": sources.read(ENTITIES, "data/pc/26.1/entities.json"),
        "protocol": sources.read(BASELINE, "data/pc/26.1/protocol.json"),
    }
    registries = {name: comparison(named(baseline[kind]), named(candidate[kind]))
                  for name, kind in [("Block", "blocks"), ("Item", "items"), ("Entity", "entities")]}
    registries["BlockState"] = comparison(block_states(baseline["blocks"]), block_states(candidate["blocks"]))
    old_types, new_types = baseline["protocol"]["types"], candidate["protocol"]["types"]
    for name, old, new in [
        ("Particle", old_types["Particle"][1][0]["type"], new_types["Particle"][1][0]["type"]),
        ("Component", old_types["SlotComponentType"], new_types["SlotComponentType"]),
    ]:
        registries[name] = comparison(mapper(old), mapper(new))
    particle_json = named(candidate["particles"])
    particle_protocol = mapper(new_types["Particle"][1][0]["type"])
    # Не скрываем противоречия двух кандидатных таблиц.
    particle_disagreements = [
        {"name": n, "particles_json": particle_json.get(n), "protocol_json": particle_protocol.get(n)}
        for n in sorted(particle_json.keys() | particle_protocol.keys())
        if particle_json.get(n) != particle_protocol.get(n)]
    attribute_fields = candidate["protocol"]["play"]["toClient"]["types"]["packet_entity_update_attributes"][1]
    attributes = mapper(attribute_fields[1]["type"][1]["type"][1][0]["type"])
    return {
        "format": 1, "baseline_protocol": 775, "candidate_protocol": 776,
        "status": "candidate JSON only; no runtime admission or primary gameplay profile",
        "sources": sources.manifest,
        "mapping_contract": "Exact name and block-state-property equality only; -1 means unavailable. Sparse IDs require presence lists. Candidate attribute IDs are deliberately excluded.",
        "registries": registries,
        "content_records": {kind: records_delta(baseline[kind], candidate[kind])
                            for kind in ["blocks", "items", "entities"]},
        "candidate_only": {
            "sounds": candidate["sounds"],
            "particles_json": candidate["particles"],
            "particle_table_disagreements": particle_disagreements,
            "attributes": {
                "runtime_mapping_available": False,
                "protocol_mapper_count": len(attributes),
                "attributes_json_count": len(candidate["attributes"]),
                "attributes_json_has_explicit_ids": any("id" in entry for entry in candidate["attributes"]),
                "reason": "The protocol candidate repeats the known-inaccurate 777 list of 31 attributes; separate official776 observations are required. attributes.json is also unverified.",
                "protocol_mapper": attributes, "attributes_json": candidate["attributes"],
            },
        },
        "unverified": [
            "All776 registry IDs and state numbering need ordinary official server/client observations.",
            "JSON content changes do not establish implementation of776 mechanics or primary profile.",
            "Dynamic registry NBT, block-entity IDs and palette width require separate observations.",
            "No aliases are inferred for renamed particles; unmatched names remain unavailable.",
            "Changed record fields include renumbered references such as drops/internalId; raw numeric differences are not automatically gameplay differences.",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--download", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    previous = json.loads(args.output.read_text()) if args.output.exists() else {}
    sources = Sources(args.cache, args.download, previous.get("sources", {}))
    report = build(sources)
    body = (json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode()
    if args.check:
        if not args.output.exists() or args.output.read_bytes() != body:
            raise SystemExit("Отчёт не воспроизводится")
    else:
        args.output.write_bytes(body)
    print(json.dumps({"sha256": sha(body), "sources": len(sources.manifest),
                      "counts": {k: [v["baseline_count"], v["candidate_count"], v["matched"]]
                                 for k, v in report["registries"].items()},
                      "attribute_runtime_mapping": False}, ensure_ascii=False))


if __name__ == "__main__":
    main()
