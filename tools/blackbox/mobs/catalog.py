"""Полный закреплённый список живых сущностей 777, без фильтра поддержки ядром."""

PIN_SHA256 = '33c2d7569e10cb98d86240e8135396702edb92c98e6ccb1c18bf6a481f22e3d5'
PIN_RECORD_COUNT = 161
# Имя, сетевой тип, индекс здоровья: извлечены только из разрешённой JSON-таблицы.
PINNED_LIVING = (
    ('allay', 2, 9),
    ('armadillo', 4, 9),
    ('armor_stand', 5, 9),
    ('axolotl', 7, 9),
    ('bat', 10, 9),
    ('bee', 11, 9),
    ('blaze', 14, 9),
    ('bogged', 16, 9),
    ('breeze', 17, 9),
    ('camel', 19, 9),
    ('camel_husk', 20, 9),
    ('cat', 21, 9),
    ('cave_spider', 22, 9),
    ('chicken', 26, 9),
    ('cod', 27, 9),
    ('copper_golem', 28, 9),
    ('cow', 30, 9),
    ('creaking', 31, 9),
    ('creeper', 32, 9),
    ('dolphin', 36, 9),
    ('donkey', 37, 9),
    ('drowned', 39, 9),
    ('elder_guardian', 41, 9),
    ('enderman', 42, 9),
    ('endermite', 43, 9),
    ('ender_dragon', 44, 9),
    ('evoker', 47, 9),
    ('fox', 55, 9),
    ('frog', 56, 9),
    ('ghast', 58, 9),
    ('happy_ghast', 59, 9),
    ('giant', 60, 9),
    ('glow_squid', 62, 9),
    ('goat', 63, 9),
    ('guardian', 64, 9),
    ('hoglin', 65, 9),
    ('horse', 67, 9),
    ('husk', 68, 9),
    ('illusioner', 69, 9),
    ('iron_golem', 71, 9),
    ('llama', 79, 9),
    ('magma_cube', 81, 9),
    ('mannequin', 84, 9),
    ('mooshroom', 87, 9),
    ('mule', 88, 9),
    ('nautilus', 89, 9),
    ('ocelot', 92, 9),
    ('panda', 97, 9),
    ('parched', 98, 9),
    ('parrot', 99, 9),
    ('phantom', 100, 9),
    ('pig', 101, 9),
    ('piglin', 102, 9),
    ('piglin_brute', 103, 9),
    ('pillager', 104, 9),
    ('polar_bear', 105, 9),
    ('pufferfish', 110, 9),
    ('rabbit', 111, 9),
    ('ravager', 112, 9),
    ('salmon', 113, 9),
    ('sheep', 114, 9),
    ('shulker', 115, 9),
    ('silverfish', 117, 9),
    ('skeleton', 118, 9),
    ('skeleton_horse', 119, 9),
    ('slime', 120, 9),
    ('sniffer', 122, 9),
    ('snow_golem', 124, 9),
    ('spider', 127, 9),
    ('squid', 130, 9),
    ('stray', 131, 9),
    ('strider', 132, 9),
    ('sulfur_cube', 133, 9),
    ('tadpole', 134, 9),
    ('trader_llama', 138, 9),
    ('tropical_fish', 140, 9),
    ('turtle', 141, 9),
    ('vex', 142, 9),
    ('villager', 143, 9),
    ('vindicator', 144, 9),
    ('wandering_trader', 145, 9),
    ('warden', 146, 9),
    ('witch', 148, 9),
    ('wither', 149, 9),
    ('wither_skeleton', 150, 9),
    ('wolf', 152, 9),
    ('zoglin', 153, 9),
    ('zombie', 154, 9),
    ('zombie_horse', 155, 9),
    ('zombie_nautilus', 156, 9),
    ('zombie_villager', 157, 9),
    ('zombified_piglin', 158, 9),
 )

WATER = frozenset("axolotl cod dolphin elder_guardian glow_squid guardian nautilus pufferfish salmon squid tadpole tropical_fish zombie_nautilus".split())
AIR = frozenset("allay bat bee blaze breeze ender_dragon ghast happy_ghast parrot phantom vex wither".split())
SPECIAL_DAMAGE = {
    "armor_stand": "Стойка имеет отдельные правила разрушения и не является обычным мобом.",
    "ender_dragon": "Дракон получает урон через отдельные части тела; обычный /damage не проверяет бой с ним.",
    "wither": "Начальная неуязвимость и отдельная последовательность смерти требуют специального сценария.",
    "creaking": "Урон и связь с сердцем требуют специального сценария.",
    "mannequin": "Манекен требует отдельной проверки применимости урона.",
}

def catalog(candidate_names=()):
    """Поддержка кандидатом — только диагностика; набор сущностей от неё не зависит."""
    supported = set(candidate_names)
    return [{"mob": name, "type_id": type_id, "health_index": health_index,
             "classification": "armor_stand" if name == "armor_stand" else "living_mob",
             "environment": "water" if name in WATER else "air" if name in AIR else "dry",
             "candidate_support": name in supported if candidate_names else None,
             "generic_damage_exclusion": SPECIAL_DAMAGE.get(name)}
            for name, type_id, health_index in PINNED_LIVING]

PLANNED_BEHAVIORS = ("movement_ai", "water_movement", "air_movement", "breeding",
                     "taming", "target_selection", "attack", "flee", "daylight",
                     "status_effect", "save_reload")

def coverage_matrix(cases):
    """Уникальные пары моб/поведение; наличие сценария ещё не означает успешный прогон."""
    rows = {(r["mob"], behavior): {"mob": r["mob"], "behavior": behavior,
            "status": "PLANNED", "case_ids": []}
            for r in catalog() for behavior in PLANNED_BEHAVIORS}
    for case in cases:
        key = (case["mob"], case["family"])
        row = rows.setdefault(key, {"mob": key[0], "behavior": key[1],
                                   "status": "IMPLEMENTED", "case_ids": []})
        row["case_ids"].append(case["id"])
    for r in catalog():
        if r["generic_damage_exclusion"]:
            for behavior in ("damage", "death"):
                rows[(r["mob"], behavior)] = {"mob": r["mob"], "behavior": behavior,
                    "status": "EXCLUDED_GENERIC", "case_ids": [],
                    "reason": r["generic_damage_exclusion"]}
    return [rows[k] for k in sorted(rows)]
