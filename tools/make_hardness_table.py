#!/usr/bin/env python3
"""Собирает src/hardness_table.rs: какие блоки ломаются мгновенно и какие
не ломаются вовсе.

Источник — tools/data/blocks.json (minecraft-data), поле hardness:
0 — блок ломается с первого удара (трава, цветы, факелы, провод), и клиент
в выживании шлёт тогда только «начал копать», без «докопал»; -1 или пусто —
блок не ломается в выживании (бедрок, барьер, порталы).
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
DATA = ROOT / "tools" / "data" / "blocks.json"
OUTPUT = ROOT / "src" / "hardness_table.rs"


def ranges(blocks):
    return sorted((block["minStateId"], block["maxStateId"]) for block in blocks)


def rust(name, doc, spans):
    lines = [f"/// {doc}", f"pub const {name}: &[(i32, i32)] = &["]
    lines += [f"    ({low}, {high})," for low, high in spans]
    lines.append("];")
    return "\n".join(lines)


def main():
    blocks = json.loads(DATA.read_text())
    instant = [b for b in blocks if b.get("hardness") == 0 and b["name"] != "air"]
    unbreakable = [b for b in blocks if b.get("hardness") in (None, -1)]

    text = "\n\n".join([
        "// Создано tools/make_hardness_table.py из minecraft-data — не править руками.",
        rust("INSTANT", "Состояния блоков, которые ломаются с первого удара.", ranges(instant)),
        rust("UNBREAKABLE", "Состояния блоков, которые в выживании не ломаются.", ranges(unbreakable)),
    ]) + "\n"
    OUTPUT.write_text(text)
    print(f"{OUTPUT}: мгновенных {len(instant)}, неразрушимых {len(unbreakable)}")


main()
