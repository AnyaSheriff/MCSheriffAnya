"""Собирает таблицы для входа с Bedrock Edition (src/bedrock/).

Источники — только данные, не код:
- minecraft-data, data/bedrock/1.26.10: blockStates.json (все состояния блоков
  Bedrock по порядку сетевых номеров), items.json (предметы и их номера);
- minecraft-data, data/bedrock/1.26.0: blocksJ2B.json (состояние Java →
  состояние Bedrock);
- tools/data/blocks.json — наши блоки Java (те же, что для blocks_table.rs);
- minecraft.wiki, страница «Biome/ID», таблица «Bedrock Biome IDs» — номера
  биомов Bedrock.

Файлы minecraft-data берутся из ~/.cache/mcsheriffanya/minecraft-data/bedrock
(скачать: см. tools/README.md), страница вики — из ~/.cache/mcsheriffanya/wiki.

Выход:
- src/bedrock/java_to_bedrock.bin — на каждый номер состояния Java два байта
  (little-endian): сетевой номер состояния Bedrock;
- src/bedrock/item_registry.bin — готовое тело пакета Item Registry;
- src/bedrock/creative_content.bin — готовое тело пакета Creative Content:
  предметы Java 26.1.2 (tools/data/items.json) в их порядке, которые есть и
  у Bedrock — по имени предмета или по имени блока из blocksJ2B;
- src/bedrock/items_to_java.bin — предмет Bedrock → предмет Java: по три
  числа little-endian на запись (номер Bedrock i16, метаданные u16, номер
  Java u16), по возрастанию первых двух;
- src/bedrock/items_to_bedrock.bin — на каждый номер предмета Java восемь
  байт little-endian: номер предмета Bedrock (i16, 0 — нет такого),
  метаданные (u16) и номер блока (i32);
- src/bedrock/creative_items.bin — номер предмета Java (u16) для каждой
  записи творческого инвентаря по порядку;
- src/bedrock/tables.rs — номера биомов и прочие мелкие таблицы.
"""

import json
import pathlib
import re
import struct

ROOT = pathlib.Path(__file__).resolve().parent.parent
CACHE = pathlib.Path.home() / ".cache" / "mcsheriffanya"
BEDROCK = CACHE / "minecraft-data" / "bedrock"
OUT = ROOT / "src" / "bedrock"


def java_states():
    """Номер состояния Java → (имя, свойства) — как считает blocks.rs."""
    blocks = json.load(open(ROOT / "tools" / "data" / "blocks.json"))
    table = {}

    for block in blocks:
        states = block.get("states", [])
        values = []

        for state in states:
            if state["type"] == "bool":
                values.append(["true", "false"])
            elif state["type"] == "int":
                values.append(state.get("values") or [str(v) for v in range(state["num_values"])])
            else:
                values.append(state["values"])

        count = 1
        for v in values:
            count *= len(v)

        for index in range(count):
            props = {}
            rest = index

            for state, options in reversed(list(zip(states, values))):
                props[state["name"]] = str(options[rest % len(options)])
                rest //= len(options)

            table[block["minStateId"] + index] = (block["name"], props)

    return table


def parse_state(text):
    name, _, props = text.partition("[")
    name = name.replace("minecraft:", "")
    props = props.rstrip("]")
    result = {}

    for pair in filter(None, props.split(",")):
        key, _, value = pair.partition("=")
        result[key] = value

    return name, result


def normalize(value):
    if value in ("true", "false"):
        return 1 if value == "true" else 0
    try:
        return int(value)
    except ValueError:
        return value


def bedrock_key(name, props):
    return (name, tuple(sorted((k, normalize(str(v))) for k, v in props.items())))


def main():
    states = json.load(open(BEDROCK / "1.26.10" / "blockStates.json"))
    by_key = {}
    first_of = {}

    for index, state in enumerate(states):
        props = {k: v["value"] for k, v in state["states"].items()}
        by_key.setdefault(bedrock_key(state["name"], props), index)
        first_of.setdefault(state["name"], index)

    java = java_states()
    total = max(java) + 1
    mapping = [0] * total
    exact = by_name = missing = 0
    air = first_of["air"]
    missing_names = set()

    for state_id in range(total):
        name, props = java.get(state_id, ("air", {}))
        key = (name, tuple(sorted(props.items())))
        found = LOOKUP.get(key)

        if found is not None:
            bname, bprops = found
            index = by_key.get(bedrock_key(bname, bprops))

            if index is None:
                index = first_of.get(bname)
                by_name += 1
            else:
                exact += 1
        else:
            index = first_of.get(name)

            if index is None:
                missing += 1
                missing_names.add(name)
                index = first_of["stone"] if name != "air" else air
            else:
                by_name += 1

        mapping[state_id] = index

    with open(OUT / "java_to_bedrock.bin", "wb") as out:
        for value in mapping:
            out.write(struct.pack("<H", value))

    print(f"состояний Java {total}: точно {exact}, по имени {by_name}, нет у Bedrock {missing}")
    print("нет у Bedrock:", sorted(missing_names)[:40])

    write_items()
    write_creative(mapping, states)
    write_tables(len(states))


LOOKUP = {}


def prepare_lookup():
    j2b = json.load(open(BEDROCK / "1.26.0" / "blocksJ2B.json"))

    for java, bedrock in j2b.items():
        name, props = parse_state(java)
        LOOKUP[(name, tuple(sorted(props.items())))] = parse_state(bedrock)


# --- NBT в сетевом виде Bedrock: little-endian, длины и int — varint. ---

TAGS = {"end": 0, "byte": 1, "short": 2, "int": 3, "long": 4, "float": 5, "double": 6, "byteArray": 7,
        "string": 8, "list": 9, "compound": 10, "intArray": 11, "longArray": 12}


def varint(value):
    out = bytearray()
    value &= 0xFFFFFFFFFFFFFFFF
    while True:
        if value < 0x80:
            out.append(value)
            return bytes(out)
        out.append((value & 0x7F) | 0x80)
        value >>= 7


def zigzag32(value):
    return varint(((value << 1) ^ (value >> 31)) & 0xFFFFFFFF)


def zigzag64(value):
    return varint(((value << 1) ^ (value >> 63)) & 0xFFFFFFFFFFFFFFFF)


def nbt_string(text):
    data = text.encode()
    return varint(len(data)) + data


def nbt_payload(kind, value):
    if kind == "byte":
        return struct.pack("<b", value if value < 128 else value - 256)
    if kind == "short":
        return struct.pack("<h", value)
    if kind == "int":
        return zigzag32(value)
    if kind == "long":
        return zigzag64(value if isinstance(value, int) else (value[0] << 32 | (value[1] & 0xFFFFFFFF)))
    if kind == "float":
        return struct.pack("<f", value)
    if kind == "double":
        return struct.pack("<d", value)
    if kind == "string":
        return nbt_string(value)
    if kind == "list":
        inner = value["type"]
        items = value["value"]
        out = bytes([TAGS[inner]]) + zigzag32(len(items))
        for item in items:
            out += nbt_payload(inner, item)
        return out
    if kind == "compound":
        out = b""
        for name, entry in value.items():
            out += bytes([TAGS[entry["type"]]]) + nbt_string(name) + nbt_payload(entry["type"], entry["value"])
        return out + b"\x00"
    if kind == "byteArray":
        return zigzag32(len(value)) + bytes(v & 0xFF for v in value)
    if kind == "intArray":
        return zigzag32(len(value)) + b"".join(zigzag32(v) for v in value)
    if kind == "longArray":
        return zigzag32(len(value)) + b"".join(zigzag64(v) for v in value)
    raise ValueError(kind)


def nbt_root(tag):
    return bytes([TAGS[tag["type"]]]) + nbt_string(tag.get("name", "")) + nbt_payload(tag["type"], tag["value"])


def write_items():
    items = json.load(open(BEDROCK / "1.26.10" / "items.json"))
    versions = {"legacy": 0, "data_driven": 1, "none": 2}
    body = varint(len(items))

    for item in items:
        name = "minecraft:" + item["name"]
        body += nbt_string(name)
        body += struct.pack("<h", item["id"])
        body += bytes([1 if item.get("version") == "data_driven" else 0])
        body += zigzag32(versions.get(item.get("version"), 2))
        body += nbt_root(item.get("nbt") or {"type": "compound", "name": "", "value": {}})

    (OUT / "item_registry.bin").write_bytes(body)
    print(f"предметов в реестре: {len(items)}, {len(body)} байт")


# Предметы-снаряжение: отдельная вкладка творческого инвентаря.
EQUIPMENT = re.compile(
    r"(_sword|_pickaxe|_axe|_shovel|_hoe|_spear|_helmet|_chestplate|_leggings|_boots|_horse_armor|_harness)$"
    r"|^(bow|crossbow|trident|shield|mace|arrow|spectral_arrow|elytra|turtle_helmet|wolf_armor)$"
)

# Блоки природы: земля, камни, руды, растения, деревья.
NATURE = re.compile(
    r"(dirt|grass|sand$|gravel|clay$|mud$|_log$|_stem$|_wood$|_hyphae$|leaves|sapling|propagule|_ore$|"
    r"flower|tulip|orchid|allium|bluet|daisy|poppy|dandelion|lilac|peony|rose_bush|sunflower|"
    r"fern|bush|vine|mushroom|fungus|roots|moss|coral|kelp|seagrass|cactus|sugar_cane|bamboo$|"
    r"pumpkin$|melon$|ice$|snow|dripleaf|azalea|lichen|sculk|amethyst|dripstone|petals|spore|"
    r"netherrack|soul_s|basalt|blackstone$|end_stone$|obsidian|nylium|wart_block|shroomlight|calcite|tuff$|"
    r"^stone$|^granite$|^diorite$|^andesite$|^deepslate$|mycelium|podzol|farmland|lily_pad|frogspawn|egg$)"
)


# Цвета по порядку Java; у Bedrock цвет кровати и флага — в метаданных
# предмета (minecraft.wiki, «Bed» и «Banner», раздел Data values): кровати —
# в этом же порядке, флаги — в обратном.
COLORS = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
    "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black",
]


def item_metadata(name):
    for index, color in enumerate(COLORS):
        if name == f"{color}_bed":
            return index
        if name == f"{color}_banner":
            return 15 - index
    return 0


def write_creative(mapping, states):
    java_items = json.load(open(ROOT / "tools" / "data" / "items.json"))
    java_blocks = {block["name"]: block for block in json.load(open(ROOT / "tools" / "data" / "blocks.json"))}
    bedrock_items = {item["name"]: item for item in json.load(open(BEDROCK / "1.26.10" / "items.json"))}

    groups = [(1, "construction"), (2, "nature"), (3, "equipment"), (4, "items")]
    entries = []
    skipped = []
    to_java = {}
    to_bedrock = {}
    creative_java = []

    for item in java_items:
        name = item["name"]
        block = java_blocks.get(name)
        runtime = mapping[block["defaultState"]] if block else 0
        bedrock = bedrock_items.get(name)

        # Имя предмета у Bedrock другое — берём имя блока, в который
        # переводится блок Java.
        if bedrock is None and block is not None:
            bedrock = bedrock_items.get(states[runtime]["name"])

        if bedrock is None or name == "air":
            skipped.append(name)
            continue

        if EQUIPMENT.search(name):
            group = 2
        elif block is None:
            group = 3
        elif NATURE.search(name):
            group = 1
        else:
            group = 0

        entries.append((bedrock["id"], item_metadata(name), runtime, group))
        creative_java.append(item["id"])
        to_bedrock[item["id"]] = (bedrock["id"], item_metadata(name), runtime)
        to_java.setdefault((bedrock["id"], item_metadata(name)), item["id"])

    body = varint(len(groups))

    for category, _ in groups:
        body += struct.pack("<i", category) + varint(0) + zigzag32(0)

    body += varint(len(entries))

    for index, (network_id, metadata, runtime, group) in enumerate(entries):
        # entry_id; ItemLegacy: номер, количество, метаданные, блок, пустые
        # дополнительные данные (без NBT, два пустых списка); вкладка.
        extra = struct.pack("<H", 0) + struct.pack("<i", 0) + struct.pack("<i", 0)
        body += varint(index + 1)
        body += zigzag32(network_id) + struct.pack("<H", 1) + varint(metadata) + zigzag32(runtime)
        body += varint(len(extra)) + extra
        body += varint(group)

    (OUT / "creative_content.bin").write_bytes(body)

    with open(OUT / "items_to_java.bin", "wb") as out:
        for (bedrock_id, metadata), java_id in sorted(to_java.items()):
            out.write(struct.pack("<hHH", bedrock_id, metadata, java_id))

    with open(OUT / "items_to_bedrock.bin", "wb") as out:
        for java_id in range(max(item["id"] for item in java_items) + 1):
            out.write(struct.pack("<hHi", *to_bedrock.get(java_id, (0, 0, 0))))

    with open(OUT / "creative_items.bin", "wb") as out:
        for java_id in creative_java:
            out.write(struct.pack("<H", java_id))
    print(f"творческий инвентарь: {len(entries)} предметов, пропущено {len(skipped)}: {skipped[:30]}")


# Имена биомов, которые у Bedrock другие.
BIOME_ALIASES = {
    "stony_shore": "stone_beach",
    "snowy_plains": "ice_plains",
    "ice_spikes": "ice_plains_spikes",
    "snowy_beach": "cold_beach",
    "snowy_taiga": "cold_taiga",
    "windswept_hills": "extreme_hills",
    "windswept_forest": "extreme_hills_plus_trees",
    "windswept_gravelly_hills": "extreme_hills_mutated",
    "windswept_savanna": "savanna_mutated",
    "old_growth_birch_forest": "birch_forest_mutated",
    "old_growth_pine_taiga": "mega_taiga",
    "old_growth_spruce_taiga": "redwood_taiga_mutated",
    "sparse_jungle": "jungle_edge",
    "badlands": "mesa",
    "eroded_badlands": "mesa_bryce",
    "wooded_badlands": "mesa_plateau_stone",
    "dark_forest": "roofed_forest",
    "mushroom_fields": "mushroom_island",
    "swamp": "swampland",
    "savanna_plateau": "savanna_plateau",
    "autumn_forest": "forest",
}


def write_tables(state_count):
    wiki = (CACHE / "wiki" / "Biome_ID.wiki").read_text(encoding="utf8")
    segment = wiki[wiki.find("bedrock biomes"):]
    ids = dict(re.findall(r"<code>([a-z_]+)</code>\s*\n\|\s*(\d+)", segment[:40000]))

    terrain = (ROOT / "src" / "world" / "terrain.rs").read_text(encoding="utf8")
    block = terrain[terrain.find("fn name(self)"):]
    block = block[:block.find("\n    }\n")]
    ours = re.findall(r'=> "([a-z_]+)"', block) + ["autumn_forest"]

    numbers = []
    names = []
    for name in ours:
        bedrock = BIOME_ALIASES.get(name, name)
        if bedrock not in ids:
            print("биома нет в таблице Bedrock:", name, "→", bedrock)
            bedrock = "plains"
        numbers.append(int(ids[bedrock]))
        names.append(bedrock)

    lines = [
        "// Создано tools/make_bedrock_tables.py — не править руками.",
        "",
        "/// Сколько состояний блоков у Bedrock 1.26.10.",
        f"pub const BLOCK_STATES: usize = {state_count};",
        "",
        "/// Номер биома Bedrock для каждого нашего: по порядку `Biome::ALL`, затем",
        "/// свои (`CUSTOM_BIOMES`).",
        f"pub const BIOMES: [u32; {len(numbers)}] = {numbers};",
        "",
        "/// Имена тех же биомов у Bedrock.",
        f"pub const BIOME_NAMES: [&str; {len(names)}] = [{', '.join(json.dumps(n) for n in names)}];",
        "",
    ]
    (OUT / "tables.rs").write_text("\n".join(lines), encoding="utf8")
    print(f"биомов: {len(numbers)}")


if __name__ == "__main__":
    prepare_lookup()
    main()
