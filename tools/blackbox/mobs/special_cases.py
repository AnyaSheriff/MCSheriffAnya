"""Узкие парные опыты; это проверки наблюдений, не обещание совпадения AI.

reference_requirements содержит имена предикатов, которые runner обязан проверить
сначала на vanilla. Неизвестный предикат — INCONCLUSIVE, ложный на эталоне —
INVALID_REFERENCE. Одинаковые случайные траектории никогда не требуются.
"""
from copy import deepcopy


def _case(name, mob, family, nbt, environment, ticks, requirement, fields):
    return {
        "id": name, "mob": mob, "family": family, "nbt": nbt,
        "environment": environment,
        "phases": [{"id": "observe", "commands": [], "ticks": ticks}],
        "compare_fields": fields,
        "reference_requirements": ["spawn_seen", requirement],
        "policy": "predicate",
    }


def special_cases():
    """Возвращает независимые JSON-совместимые записи без общего mutable state."""
    result = []
    # Явно заданный Rotation проверяет передачу позы, не естественный поворот AI.
    for name, yaw, pitch in (("positive", 73, 17), ("negative", -73, -17),
                             ("halfturn", 179, 0), ("zero", 0, 0)):
        case = _case(f"special-pose-{name}", "cow", "protocol_pose",
                     f"NoAI:1b,NoGravity:1b,Rotation:[{yaw}.0f,{pitch}.0f]",
                     "dry", 5, "rotation_seen", ["spawn_yaw", "spawn_pitch"])
        case["policy"] = "exact"
        result.append(case)
    # Поднятие через summon, не /tp: первый пакет задаёт начало падения.
    for mob in ("cow", "pig", "sheep", "zombie"):
        for floating in (False, True):
            case = _case(f"special-gravity-{mob}-{'off' if floating else 'on'}",
                         mob, "gravity_control",
                         "NoAI:1b,Invulnerable:1b,NoGravity:" + ("1b" if floating else "0b"),
                         "air", 60, "stationary" if floating else "falls",
                         ["position"])
            result.append(case)
    # Вода и отсутствие гравитации разделены: сухая рыба не доказывает плавание.
    for mob in ("cod", "salmon", "pufferfish", "tropical_fish"):
        for active in (False, True):
            result.append(_case(
                f"special-water-{mob}-{'ai' if active else 'noai'}", mob,
                "fish_water_control", "Invulnerable:1b,NoGravity:1b,NoAI:" + ("0b" if active else "1b"),
                "water", 120, "moves" if active else "stationary", ["position"]))
        result.append(_case(f"special-dry-{mob}-floating", mob,
                            "fish_dry_control", "Invulnerable:1b,NoAI:1b,NoGravity:1b",
                            "air", 60, "stationary", ["position"]))
    # Полёт считается подтверждённым только при наблюдавшемся движении vanilla.
    for mob in ("happy_ghast", "ghast", "bat", "allay"):
        for active in (False, True):
            result.append(_case(
                f"special-flight-{mob}-{'ai' if active else 'noai'}", mob,
                "freeflight_control", "Invulnerable:1b,NoGravity:1b,NoAI:" +
                ("0b" if active else "1b"), "air", 160,
                "moves" if active else "stationary", ["position"]))
    return deepcopy(result)


SPECIAL_CASES = special_cases()
