"""Возраст урона и пошаговая запись здоровья: это наблюдения сети, не внутренний такт лечения."""

def allay_cases():
    cases = []
    for age in (2, 7, 12):
        for amount in (1, 4):
            phases = [
                {"id": "before", "commands": [], "ticks": age - 2},
                {
                    "id": "hurt",
                    "commands": ["damage {target} %d minecraft:generic" % amount],
                    "ticks": 0,
                },
            ]
            for i in range(1, 41):
                phases.append({"id": "after_%02d" % i, "commands": [], "ticks": 1})
            cases.append(
                {
                    "id": "allay.regen-age%02d-damage%02d" % (age, amount),
                    "mob": "allay",
                    "family": "regeneration",
                    "nbt": "NoAI:1b,NoGravity:1b,PersistenceRequired:1b,Rotation:[37.0f,0.0f],Health:20.0f",
                    "environment": "air",
                    "phases": phases,
                    "compare_fields": ["health"],
                    "reference_requirements": ["spawn_observed"],
                    "policy": "exact",
                    "damage_age": age,
                    "damage_amount": amount,
                }
            )
    return cases
