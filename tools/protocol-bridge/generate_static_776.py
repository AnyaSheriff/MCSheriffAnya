#!/usr/bin/env python3
"""Прямые статические ID775↔776↔777; не codec и не runtime admission.

Только закреплённые локальные JSON. Никаких загрузок/чужих реализаций.
u16::MAX зарезервирован под MissingTarget. Источники с holes отклоняются:
доказанная плотность ID позволяет отличать UnknownId до чтения таблицы.
"""
import argparse
import hashlib
import json
from pathlib import Path

from generate_registry_26_2 import CANDIDATE, LOCAL_LOCKS, Sources, block_states, mapper, named

KINDS = ["Item", "Block", "BlockState", "Entity", "Component", "Particle"]
VERSIONS = {775: "V26_1_2", 776: "V26_2", 777: "V26_3"}
PAIRS = [(775, 776), (776, 775), (776, 777), (777, 776)]
MISSING = 65535


def dense_count(values):
    numbers = list(values.values())
    if not numbers or any(type(n) is not int or n < 0 or n >= MISSING for n in numbers):
        raise ValueError("ID должен быть целым в0..65534;65535 зарезервирован")
    if len(set(numbers)) != len(numbers):
        raise ValueError("Повторяющийся ID")
    if set(numbers) != set(range(max(numbers) + 1)):
        raise ValueError("Неплотные source IDs: holes требуют отдельной таблицы присутствия")
    return len(numbers)


def direct_mapping(source, target):
    count = dense_count(source)
    dense_count(target)
    result = [MISSING] * count
    for name, number in source.items():
        result[number] = target.get(name, MISSING)
    return tuple(result)


def load_versions(root, data777, cache776):
    provenance = json.loads((root / "tools/protocol-bridge/candidate-sources-26.3.json").read_text())
    source_manifest = {}

    def old_or_new(filename):
        record = next(entry for entry in provenance["files"] + provenance["baseline_files"]
                      if entry["file"] == filename)
        body = (data777 / filename).read_bytes()
        if len(body) != record["bytes"] or hashlib.sha256(body).hexdigest() != record["sha256"]:
            raise ValueError(f"Изменился закреплённый источник {filename}")
        source_manifest[record["url"]] = {"sha256": record["sha256"], "bytes": len(body)}
        return json.loads(body)

    def local(filename):
        body = (root / filename).read_bytes()
        if hashlib.sha256(body).hexdigest() != LOCAL_LOCKS[filename]:
            raise ValueError(f"Изменился внутренний baseline {filename}")
        source_manifest[filename] = {"sha256": LOCAL_LOCKS[filename], "bytes": len(body)}
        return json.loads(body)

    locked = json.loads((root / "tools/protocol-bridge/registry-candidate-26.2.json").read_text())["sources"]
    sources = Sources(cache776, False, locked)
    docs = {
        775: {"blocks": local("tools/data/blocks.json"), "items": local("tools/data/items.json"),
              "entities": old_or_new("entities-26.1.json"), "protocol": old_or_new("protocol-26.1.json")},
        776: {kind: sources.read(CANDIDATE, f"data/pc/26.2/{kind}.json")
              for kind in ["blocks", "items", "entities", "protocol"]},
        777: {kind: old_or_new(kind + ".json") for kind in ["blocks", "items", "entities", "protocol"]},
    }
    for entry in sources.manifest.values():
        source_manifest[entry["url"]] = {"sha256": entry["sha256"], "bytes": entry["bytes"]}
    versions = {}
    for version, doc in docs.items():
        versions[version] = {
            "Item": named(doc["items"]), "Block": named(doc["blocks"]),
            "BlockState": block_states(doc["blocks"]), "Entity": named(doc["entities"]),
            "Component": mapper(doc["protocol"]["types"]["SlotComponentType"]),
            "Particle": mapper(doc["protocol"]["types"]["Particle"][1][0]["type"]),
        }
        for values in versions[version].values():
            dense_count(values)
    return versions, source_manifest


def build(versions, sources):
    counts = {kind: {version: dense_count(versions[version][kind]) for version in VERSIONS}
              for kind in KINDS}
    pool, rows, routes = {}, [], []
    for kind in KINDS:
        for source, target in PAIRS:
            values = direct_mapping(versions[source][kind], versions[target][kind])
            reverse = direct_mapping(versions[target][kind], versions[source][kind])
            for number, mapped in enumerate(values):
                if mapped != MISSING and reverse[mapped] != number:
                    raise ValueError("Необратимое точное соответствие")
            symbol = f"{kind.upper()}_{source}_TO_{target}"
            reused = values in pool
            if reused:
                symbol = pool[values]
            else:
                pool[values] = symbol
            routes.append((kind, source, target, symbol))
            rows.append({"registry": kind, "from": source, "to": target, "symbol": symbol,
                         "source_count": len(values), "mapped": sum(v != MISSING for v in values),
                         "reused_existing_array": reused, "raw_u16_bytes": len(values) * 2})
    text = "// Сгенерировано generate_static_776.py; закреплённые JSON в static-776-report.json.\n"
    text += "// Только ID; форматы packet/item payload и admission776 остаются закрыты.\n"
    text += "// Все source ID проверены плотными0..count: holes НЕ считаются MissingTarget.\n"
    text += "use crate::{JavaVersion, ids::RegistryKind};\n\n"
    text += "pub const MISSING: u16 = u16::MAX;\n\n"
    text += "pub fn source_count(kind: RegistryKind, version: JavaVersion) -> Option<u32> {\n    match (kind, version) {\n"
    for kind in KINDS:
        for version, label in VERSIONS.items():
            text += f"        (RegistryKind::{kind}, JavaVersion::{label}) => Some({counts[kind][version]}),\n"
    text += "        _ => None,\n    }\n}\n\n"
    text += "pub fn table(kind: RegistryKind, from: JavaVersion, to: JavaVersion) -> Option<&'static [u16]> {\n    match (kind, from, to) {\n"
    for kind, source, target, symbol in routes:
        text += f"        (RegistryKind::{kind}, JavaVersion::{VERSIONS[source]}, JavaVersion::{VERSIONS[target]}) => Some(&{symbol}),\n"
    text += "        _ => None,\n    }\n}\n\n"
    for values, symbol in pool.items():
        text += f"static {symbol}: [u16; {len(values)}] = [\n"
        for start in range(0, len(values), 24):
            text += "    " + ", ".join(map(str, values[start:start + 24])) + ",\n"
        text += "];\n\n"
    text = text.rstrip("\n") + "\n"
    examples = []
    for kind, name in [("Block", "sulfur"), ("Item", "sulfur_cube_bucket"),
                       ("Item", "music_disc_bounce"), ("Entity", "sulfur_cube"),
                       ("Component", "sulfur_cube_content"), ("Component", "map_color"),
                       ("Component", "swing_animation"), ("Particle", "sulfur_cube_goo"),
                       ("Particle", "trial_spawner_detection"),
                       ("Particle", "trial_spawner_detected_player")]:
        examples.append({"registry": kind, "name": name,
                         "ids": {str(v): versions[v][kind].get(name) for v in VERSIONS}})
    state = ("cinnabar", ())
    examples.append({"registry": "BlockState", "name": "cinnabar",
                     "ids": {str(v): versions[v]["BlockState"].get(state) for v in VERSIONS}})
    report = {
        "format": 1, "sources": sources, "counts": counts, "routes": rows,
        "new_unique_arrays": len(pool), "raw_static_u16_bytes": sum(len(a) * 2 for a in pool),
        "rust_source_sha256": hashlib.sha256(text.encode()).hexdigest(),
        "missing_target_sentinel": MISSING, "source_presence": "dense_count verified; reject holes/maxID>=65535 before generation",
        "examples": examples,
        "scope": "Candidate static IDs only; Attribute uses separate observed mapping; existing775↔777 tables unchanged; packet codecs/admission not enabled.",
    }
    return text, json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--data777", type=Path, required=True)
    parser.add_argument("--cache776", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    rust = args.output or args.root / "tools/protocol-bridge/src/generated_static_776.rs"
    report = args.report or args.root / "tools/protocol-bridge/static-776-report.json"
    versions, sources = load_versions(args.root, args.data777, args.cache776)
    bodies = build(versions, sources)
    for path, body in zip([rust, report], bodies):
        if args.check:
            if not path.exists() or path.read_text() != body:
                raise SystemExit(f"Не воспроизводится {path}")
        else:
            path.write_text(body)
    print(json.dumps({"raw_u16_bytes": json.loads(bodies[1])["raw_static_u16_bytes"],
                      "source_sha256": hashlib.sha256(bodies[0].encode()).hexdigest()}))


if __name__ == "__main__":
    main()
