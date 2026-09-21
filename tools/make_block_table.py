#!/usr/bin/env python3
"""Собирает таблицы для src/blocks_table.rs.

Откуда взяты исходные списки — см. tools/README.md.

Соединяем два списка по имени: у блока и у его предмета имя совпадает
(«stone» и «stone»), поэтому предмет «камень» указывает на блок «камень»,
а у блока уже есть номер состояния по умолчанию — тот, который игра берёт,
когда игрок ставит блок в пустое место. Именно это число нужно серверу,
пока он не умеет считать состояние сам.

Предметы, у которых блока с таким именем нет (палка, меч, ведро), в таблицу
не попадают: их ставить некуда.

Вторая таблица — блоки, состояние которых зависит от того, КАК игрок поставил
блок: бревно ложится по грани, ступени поворачиваются по взгляду. Чтобы сервер
мог посчитать состояние сам, у таких блоков выписаны их свойства и порядок
значений — по ним номер состояния и складывается.

Третья — предметы, которыми в творческом режиме блоки не ломаются.

Четвёртая — имена блоков по номерам состояний: обратный поиск, когда номер
известен, а спросить надо про сам блок.

Пятая и шестая — полные кубы и блоки, у которых ящика нет вовсе. В исходных
данных у плит, ступеней, дверей и заборов стоит «block», хотя блок они
занимают не целиком, поэтому полные кубы приходится вычитать по именам.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
DATA = ROOT / "tools" / "data"
OUTPUT = ROOT / "src" / "blocks_table.rs"

# У признаков «да/нет» порядок значений такой: первым идёт «да». В исходных
# данных у таких свойств списка значений нет вовсе, поэтому берём его отсюда.
# Порядок проверен по умолчаниям блоков: у блока со свойством «залит водой»
# по умолчанию стоит «нет», и это сходится только с таким порядком.
BOOL_VALUES = ("true", "false")

# Предметы, которыми в творческом режиме блоки не ломаются. Точные имена
# перечислены в вики (страницы Breaking и Sword): у этих предметов в данных
# игры стоит признак «нельзя ломать в творческом», у всех остальных он
# по умолчанию включён. Мечи ловятся по окончанию имени, потому что их
# по семь штук на разные материалы.
CANNOT_BREAK_IN_CREATIVE = ("trident", "mace", "debug_stick")
CANNOT_BREAK_IN_CREATIVE_SUFFIXES = ("_sword",)

# Предметы, которыми блоки не ломаются вовсе, ни в творческом, ни в выживании.
# Копьё — оружие для удара, а не для копания: им нельзя сломать ни один блок.
CANNOT_BREAK_AT_ALL_SUFFIXES = ("_spear",)


def cannot_break_in_creative(name: str) -> bool:
    return name in CANNOT_BREAK_IN_CREATIVE or name.endswith(
        CANNOT_BREAK_IN_CREATIVE_SUFFIXES
    )


def cannot_break_at_all(name: str) -> bool:
    return name.endswith(CANNOT_BREAK_AT_ALL_SUFFIXES)


def family(block: dict) -> str | None:
    """К какому семейству относится блок — от этого зависит правило установки.

    Правило у каждого семейства своё, и общего среди них нет: ступени смотрят
    туда же, куда игрок, а печь или сундук — наоборот. Поэтому семейство
    определяется набором свойств блока, а не догадкой.

    Забор, стенка и панель устроены почти одинаково (соединяются с соседями),
    но соединяются с разным, поэтому это три разных семейства. Различить их
    по свойствам нельзя — свойства у них одни и те же, — и помогают только
    имена: панель кончается на _pane, у стенки есть свойство «столб» (up).
    """
    name = block["name"]
    properties = block["states"]
    names = {property_["name"] for property_ in properties}

    if "axis" in names:
        return "Pillar"
    if {"facing", "half", "shape"} <= names:
        return "Stairs"
    if {"facing", "half", "hinge"} <= names:
        return "Door"
    if {"facing", "in_wall"} <= names:
        return "Gate"
    if {"face", "facing"} <= names:
        return "Face"

    sides = {"north", "east", "south", "west"}

    # Соединяющиеся блоки узнаются по четырём сторонам «да/нет» и признаку
    # «залит водой». Без последнего под это правило попал бы красный провод:
    # у него стороны тоже есть, но значения другие, а воды не бывает.
    if sides <= names and "waterlogged" in names:
        if name.endswith("_pane") or "bars" in name:
            return "Pane"
        if "up" in names:
            return "Wall"
        return "Fence"

    if "type" in names and "facing" not in names:
        return "Slab"

    # Редстоун. Провод узнаётся по силе сигнала, факелы — по имени (у напольного
    # свойство одно, «горит»), повторитель и сравнитель — по задержке и режиму.
    if name == "redstone_wire":
        return "Wire"
    if name.endswith("torch"):
        return "Torch"
    if name in ("repeater", "comparator"):
        return "Diode"
    if name in ("piston", "sticky_piston"):
        return "Piston"

    # У остальных блоков со свойствами правила установки нет, но свойства
    # читать и менять всё равно надо: лампа, нажимная плита, люк.
    return "Other"


# Блоки, которые в данных помечены полным кубом, хотя занимают блок не целиком:
# плиты, ступени, двери, заборы. Нужны, чтобы забор соединялся только с полными
# кубами: в самих данных этого различия нет — там у всех стоит «block».
PARTIAL_SUFFIXES = (
    "_slab",
    "_stairs",
    "_door",
    "_trapdoor",
    "_fence",
    "_fence_gate",
    "_pane",
    "_wall",
    "_head",
    "_skull",
    "_bed",
    "_candle",
    "_carpet",
    "fence",
    "bars",
    "chain",
)

PARTIAL_NAMES = (
    "repeater",
    "comparator",
    "daylight_detector",
    "cake",
    "candle_cake",
    "brewing_stand",
    "cauldron",
    "lava_cauldron",
    "water_cauldron",
    "powder_snow_cauldron",
    "composter",
    "lectern",
    "grindstone",
    "stonecutter",
    "anvil",
    "chipped_anvil",
    "damaged_anvil",
    "hopper",
    "flower_pot",
    "daylight_detector",
    "calibrated_sculk_sensor",
    "sculk_sensor",
    "campfire",
    "soul_campfire",
    "conduit",
    "dragon_egg",
    "end_portal_frame",
    "decorated_pot",
    "bell",
    "piston_head",
    "snow",
    "powder_snow",
    "ladder",
    "cocoa",
)


def is_full_cube(block: dict) -> bool:
    """Занимает ли блок свой блок целиком."""
    if block.get("boundingBox") != "block":
        return False

    name = block["name"]

    return name not in PARTIAL_NAMES and not name.endswith(PARTIAL_SUFFIXES)


def ranges(blocks: list[dict]) -> list[tuple[int, int]]:
    """Номера состояний блоками подряд — так их короче хранить и искать."""
    found = sorted((block["minStateId"], block["maxStateId"]) for block in blocks)

    merged = []
    for low, high in found:
        if merged and low <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], max(merged[-1][1], high))
        else:
            merged.append((low, high))

    return merged


def values_of(property_: dict) -> tuple[str, ...]:
    """Значения свойства по порядку — так, как их нумерует игра."""
    if property_["type"] == "bool":
        return BOOL_VALUES

    return tuple(property_["values"])


def defaults(block: dict) -> list[str]:
    """Значения свойств у состояния по умолчанию.

    Номер состояния устроен как смешанная система счисления: свойства идут
    по порядку, и последнее меняется быстрее всех. Поэтому чтобы разложить
    номер на значения, идём по свойствам с конца, забирая остаток от деления.
    """
    offset = block["defaultState"] - block["minStateId"]

    values = []
    for property_ in reversed(block["states"]):
        options = values_of(property_)
        values.append(options[offset % len(options)])
        offset //= len(options)

    values.reverse()
    return values


# Проводимость — отдельное свойство блока, не связанное с прозрачностью:
# «whether or not you can see through the block does not determine whether
# a block can be powered» (minecraft.wiki, Conductivity). Полный куб —
# только первое приближение, а дальше идут списки исключений с той же страницы.

# Полные кубы, которые сигнал НЕ проводят («List of non-conductive blocks»).
NOT_CONDUCTIVE_NAMES = (
    "redstone_block",
    "observer",
    "piston",
    "sticky_piston",
    "moving_piston",
    "glowstone",
    "tnt",
    "dirt_path",
    "farmland",
    "enchanting_table",
    "composter",
    "decorated_pot",
    "hopper",
    "ice",
    "honey_block",
    "big_dripleaf",  # проводит только в Bedrock
)

NOT_CONDUCTIVE_SUFFIXES = (
    "_leaves",
    "_glass",
    "_copper_bulb",
    "copper_bulb",
    "copper_grate",
    "_slab",
    "_stairs",
)

# Не полные кубы (или помеченные прозрачными), которые сигнал всё же проводят
# («List of conductive blocks»).
CONDUCTIVE_NAMES = (
    "slime_block",
    "mangrove_roots",
    "barrier",
    "soul_sand",
    "soul_soil",
    "mud",
    "target",
)


def state_values(block: dict, state: int) -> dict[str, str]:
    """Значения свойств у этого состояния блока."""
    offset = state - block["minStateId"]

    values = {}
    for property_ in reversed(block.get("states") or []):
        options = values_of(property_)
        values[property_["name"]] = options[offset % len(options)]
        offset //= len(options)

    return values


def conductive_states(blocks: list[dict]) -> list[int]:
    """Номера состояний, которые проводят сигнал редстоуна."""
    found = []

    for block in blocks:
        name = block["name"]
        low, high = block["minStateId"], block["maxStateId"]

        # Двойная плита — полный блок и проводит, обычная — нет.
        if name.endswith("_slab"):
            found.extend(
                state
                for state in range(low, high + 1)
                if state_values(block, state).get("type") == "double"
            )
            continue

        if name in CONDUCTIVE_NAMES:
            found.extend(range(low, high + 1))
            continue

        if name in NOT_CONDUCTIVE_NAMES or name.endswith(NOT_CONDUCTIVE_SUFFIXES):
            continue

        if is_full_cube(block) and not block.get("transparent"):
            found.extend(range(low, high + 1))

    return sorted(found)


def state_ranges(states: list[int]) -> list[tuple[int, int]]:
    """Подряд идущие номера — одним промежутком."""
    merged: list[list[int]] = []

    for state in states:
        if merged and state <= merged[-1][1] + 1:
            merged[-1][1] = max(merged[-1][1], state)
        else:
            merged.append([state, state])

    return [(low, high) for low, high in merged]


def main() -> None:
    blocks = json.loads((DATA / "blocks.json").read_text(encoding="utf-8"))
    items = json.loads((DATA / "items.json").read_text(encoding="utf-8"))

    block_by_name = {block["name"]: block for block in blocks}

    # Предметы, чьё имя не совпадает с именем блока, который они ставят.
    # У большинства блоков предмет называется так же, а эти — исключения.
    ITEM_TO_BLOCK = {
        "redstone": "redstone_wire",
    }

    pairs = []
    fragile = []
    harmless = []
    for item in items:
        if cannot_break_at_all(item["name"]):
            harmless.append((item["id"], item["name"]))
        elif cannot_break_in_creative(item["name"]):
            fragile.append((item["id"], item["name"]))

        block = block_by_name.get(ITEM_TO_BLOCK.get(item["name"], item["name"]))
        if block is None:
            continue
        pairs.append((item["id"], block["defaultState"], block["name"]))

    oriented = []
    for block in blocks:
        properties = block.get("states")
        if not properties:
            continue

        kind = family(block)
        if kind is not None:
            oriented.append((block["name"], kind, block))

    pairs.sort()
    fragile.sort()
    harmless.sort()
    oriented.sort(key=lambda entry: entry[0])

    lines = [
        "// Таблицы блоков и предметов.",
        "//",
        "// Файл собран программой tools/make_block_table.py — править вручную",
        "// бессмысленно, при следующем запуске правки затрутся. Источник данных",
        "// описан в tools/README.md.",
        "",
        "use crate::blocks::{Family, Orientation, Property};",
        "",
        "/// Пары «номер предмета — состояние блока по умолчанию — имя блока».",
        "///",
        "/// Отсортированы по номеру предмета: по нему идёт поиск.",
        "pub const BLOCK_STATES: &[(i32, i32, &str)] = &[",
    ]

    for item_id, state, name in pairs:
        lines.append(f'    ({item_id}, {state}, "{name}"),')

    lines.append("];")
    lines.append("")
    lines.append("/// Блоки, состояние которых зависит от того, как игрок их поставил.")
    lines.append("///")
    lines.append("/// Отсортированы по имени: по нему идёт поиск. Свойства выписаны")
    lines.append("/// по порядку — так, как их нумерует игра, а `default` — значение")
    lines.append("/// свойства у состояния по умолчанию: его берут, когда правило")
    lines.append("/// установки про это свойство ничего не говорит.")
    lines.append("pub const ORIENTED: &[Orientation] = &[")

    for name, kind, block in oriented:
        block_defaults = defaults(block)

        lines.append("    Orientation {")
        lines.append(f'        name: "{name}",')
        lines.append(f'        min: {block["minStateId"]},')
        lines.append(f'        max: {block["maxStateId"]},')
        lines.append(f"        family: Family::{kind},")
        lines.append("        properties: &[")

        for property_, default in zip(block["states"], block_defaults):
            options = ", ".join(f'"{value}"' for value in values_of(property_))
            lines.append(
                f'            Property {{ name: "{property_["name"]}",'
                f' values: &[{options}], default: "{default}" }},'
            )

        lines.append("        ],")
        lines.append("    },")

    lines.append("];")
    lines.append("")
    lines.append("/// Предметы, которыми в творческом режиме блоки не ломаются.")
    lines.append("///")
    lines.append("/// Отсортированы по номеру предмета: по нему идёт поиск.")
    lines.append("pub const CANNOT_BREAK_IN_CREATIVE: &[i32] = &[")

    for item_id, name in fragile:
        lines.append(f"    {item_id}, // {name}")

    lines.append("];")
    lines.append("")
    lines.append("/// Предметы, которыми блоки не ломаются вовсе.")
    lines.append("///")
    lines.append("/// Отсортированы по номеру предмета: по нему идёт поиск. Копьём")
    lines.append("/// нельзя сломать блок ни в творческом режиме, ни в выживании.")
    lines.append("pub const CANNOT_BREAK_AT_ALL: &[i32] = &[")

    for item_id, name in harmless:
        lines.append(f"    {item_id}, // {name}")

    lines.append("];")
    lines.append("")
    lines.append("/// Имена блоков: номера состояний, от и до, и имя блока.")
    lines.append("///")
    lines.append("/// Отсортированы по первому номеру: по нему идёт поиск. Нужны, когда")
    lines.append("/// известен только номер состояния, а спросить надо про сам блок:")
    lines.append("/// например, что стоит у соседа — забор там или камень.")
    lines.append("pub const BLOCK_NAMES: &[(i32, i32, &str)] = &[")

    for block in sorted(blocks, key=lambda block: block["minStateId"]):
        low = block["minStateId"]
        high = block["maxStateId"]
        lines.append(f'    ({low}, {high}, "{block["name"]}"),')

    lines.append("];")
    lines.append("")
    lines.append("/// Блоки, занимающие свой блок целиком: номера состояний, от и до.")
    lines.append("///")
    lines.append("/// Нужны там, где важно, полный это куб или нет: забор соединяется")
    lines.append("/// только с полными, а дверь только на полный куб и ставится.")
    lines.append("pub const FULL_CUBES: &[(i32, i32)] = &[")

    for low, high in ranges([block for block in blocks if is_full_cube(block)]):
        lines.append(f"    ({low}, {high}),")

    lines.append("];")
    lines.append("")
    lines.append("/// Блоки, у которых ящика нет вовсе: трава, факел, вода.")
    lines.append("///")
    lines.append("/// Нужны для запрета ставить блок в игрока: с такими блоками")
    lines.append("/// столкнуться нельзя, значит и мешать они не могут.")
    lines.append("pub const EMPTY_BOX: &[(i32, i32)] = &[")

    for low, high in ranges(
        [block for block in blocks if block.get("boundingBox") == "empty"]
    ):
        lines.append(f"    ({low}, {high}),")

    lines.append("];")
    lines.append("")
    lines.append("/// Предметы, которых в стопку помещается меньше обычных 64.")
    lines.append("///")
    lines.append("/// Хранятся только они: у подавляющего большинства предметов стопка")
    lines.append("/// обычная, и перечислять их все незачем. Тройка — это диапазон")
    lines.append("/// номеров предметов и размер стопки у них.")
    lines.append("pub const SMALL_STACKS: &[(i32, i32, i32)] = &[")

    small = sorted(
        (item["id"], item["stackSize"])
        for item in items
        if item.get("stackSize", 64) != 64
    )

    start = None
    previous = None
    size = None
    for number, stack in small + [(None, None)]:
        if start is not None and (number != previous + 1 or stack != size):
            lines.append(f"    ({start}, {previous}, {size}),")
            start = None
        if number is None:
            break
        if start is None:
            start = number
            size = stack
        previous = number

    lines.append("];")
    lines.append("")
    lines.append("/// Что выпадает из блока, когда его сломали.")
    lines.append("///")
    lines.append("/// Тройка — это промежуток номеров состояний блока и номер предмета,")
    lines.append("/// который из него выпадает. Блоков, из которых не выпадает ничего")
    lines.append("/// (листва, стекло), здесь нет вовсе.")
    lines.append("pub const DROPS: &[(i32, i32, i32)] = &[")

    for block in sorted(blocks, key=lambda block: block["minStateId"]):
        drops = block.get("drops") or []
        if not drops:
            continue

        lines.append(
            f'    ({block["minStateId"]}, {block["maxStateId"]}, {drops[0]}),'
        )

    lines.append("];")
    lines.append("")
    lines.append("/// Чем надо ломать блок, чтобы из него что-то выпало.")
    lines.append("///")
    lines.append("/// Промежуток номеров состояний и список подходящих предметов. Блока")
    lines.append("/// здесь нет — значит, годится что угодно, хоть рука.")
    lines.append("pub const HARVEST_TOOLS: &[(i32, i32, &[i32])] = &[")

    for block in sorted(blocks, key=lambda block: block["minStateId"]):
        tools = block.get("harvestTools") or {}
        if not tools:
            continue

        numbers = ", ".join(str(number) for number in sorted(int(t) for t in tools))
        lines.append(
            f'    ({block["minStateId"]}, {block["maxStateId"]}, &[{numbers}]),'
        )

    lines.append("];")
    lines.append("")
    lines.append("/// Номера блоков — не состояний, а самих блоков.")
    lines.append("///")
    lines.append("/// Нужны там, где блок называется числом отдельно от состояния:")
    lines.append("/// например, в пакете о действии блока (ход поршня).")
    lines.append("///")
    lines.append("/// Отсортировано по имени: по нему идёт поиск.")
    lines.append("pub const BLOCK_IDS: &[(&str, i32)] = &[")

    for block in sorted(blocks, key=lambda block: block["name"]):
        lines.append(f'    ("{block["name"]}", {block["id"]}),')

    lines.append("];")
    lines.append("")
    lines.append("/// Номера и имена всех предметов.")
    lines.append("///")
    lines.append("/// Нужны там, где предмет называется словом: ведро, например, блока")
    lines.append("/// не даёт, и по таблице блоков его не найти.")
    lines.append("pub const ITEM_NAMES: &[(i32, &str)] = &[")

    for item in sorted(items, key=lambda item: item["id"]):
        lines.append(f'    ({item["id"]}, "{item["name"]}"),')

    lines.append("];")
    lines.append("")
    lines.append("/// Состояние по умолчанию для каждого блока — по имени.")
    lines.append("///")
    lines.append("/// Не то же, что начало промежутка состояний: у свойств-переключателей")
    lines.append("/// первым идёт значение «да», поэтому первым состоянием дёрна оказывается")
    lines.append("/// заснеженный, а по умолчанию он обычный. Ставить надо именно это.")
    lines.append("///")
    lines.append("/// Отсортировано по имени: по нему идёт поиск.")
    lines.append("pub const DEFAULT_STATES: &[(&str, i32)] = &[")

    for block in sorted(blocks, key=lambda block: block["name"]):
        lines.append(f'    ("{block["name"]}", {block["defaultState"]}),')

    lines.append("];")
    lines.append("")
    lines.append("/// Блоки, проводящие сигнал редстоуна.")
    lines.append("///")
    lines.append("/// Проводимость — своё свойство блока, и по виду его не угадать:")
    lines.append("/// светокамень и блок редстоуна — полные кубы, но сигнал не проводят,")
    lines.append("/// а слизь и корни мангра — проводят. Поэтому за основу взят полный")
    lines.append("/// куб, а поверх — списки исключений с вики (страница Conductivity).")
    lines.append("/// Двойная плита проводит, обычная — нет.")
    lines.append("pub const CONDUCTIVE: &[(i32, i32)] = &[")

    for low, high in state_ranges(conductive_states(blocks)):
        lines.append(f"    ({low}, {high}),")

    lines.append("];")
    lines.append("")

    OUTPUT.write_text("\n".join(lines), encoding="utf-8")

    kinds = {}
    for _, kind, _ in oriented:
        kinds[kind] = kinds.get(kind, 0) + 1

    full = sum(1 for block in blocks if is_full_cube(block))
    empty = sum(1 for block in blocks if block.get("boundingBox") == "empty")

    print(f"Записано пар: {len(pairs)} → {OUTPUT.relative_to(ROOT)}")
    print(f"Записано блоков с правилом установки: {len(oriented)} {kinds}")
    print(f"Записано предметов, не ломающих блоки в творческом: {len(fragile)}")
    print(f"Записано предметов, не ломающих блоки нигде: {len(harmless)}")
    print(f"Записано имён блоков: {len(blocks)}")
    print(f"Записано полных кубов: {full}, блоков без ящика: {empty}")


if __name__ == "__main__":
    main()
