# pvp-fire-aspect: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: attack** (первый отклик через 0 / 0 такт.) | **▶ 2: attack** |
| +0 | робот: Damage Event type=34 cause=entity direct=entity pos=none | робот: Damage Event type=34 cause=entity direct=entity pos=none |
| +0 | робот: Hurt Animation yaw=90                                   | робот: Hurt Animation yaw=90                                   |
| +0 | робот: скорость 0,0.361,0.4                                    | робот: скорость 0,0.361,0.4                                    |
| +0 | звук (3.5,0,1.5) entity.player.attack.sweep 1.00 ~1.00         | звук (3.5,0,1.5) entity.player.attack.sweep 1.00 ~1.00         |
| +0 | частица (3.5,0.9,2.5) sweep_attack 0 шт. скорость 0 (0,0,1)    | частица (3.5,0.9,2.5) sweep_attack 0 шт. скорость 0 (0,0,1)    |
| +0 | частица (3.5,0.9,3.5) damage_indicator 3 шт. скорость 0.2 (0.1,0,0.1) | частица (3.5,0.9,3.5) damage_indicator 3 шт. скорость 0.2 (0.1,0,0.1) |
| +0 | робот: other animation {"animation":0}                         | робот: other animation {"animation":0}  ← у нас позже на 1     |
| +0 | робот: Metadata 9=13                                           | робот: Metadata 9=13                                           |
| +0 | робот: Set Health health=13 food=20 saturation=5               | робот: Set Health health=13 food=20 saturation=5               |
| +0 | робот: other entity_equipment {"equipments":["0:diamond_sword×1 {damage=1 enchantments={\"enchantments\":[{\"id\":10,\"level\":1}]}}"]} | робот: other entity_equipment {"equipments":["0:diamond_sword×1 {damage=1 enchantments={\"enchantments\":[{\"id\":10,\"level\":1}]}}"]}  ← у нас позже на 1 |
| +1 | робот: Metadata 0=1                                            | робот: Metadata 0=1                                            |
| +2 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4,"operation":0}]}]}  ← случайное, в допуске: досыл атрибутов сущности |
| +20 | робот: Damage Event type=31 cause=0 direct=0 pos=none          | робот: Damage Event type=31 cause=0 direct=0 pos=none  ← у нас позже на 2 |
| +20 | робот: Set Health health=12 food=20 saturation=5               | робот: Set Health health=12 food=20 saturation=5  ← у нас позже на 2 |
| +21 | робот: Metadata 9=12                                           | робот: Metadata 9=12  ← у нас позже на 2                       |
| +21 | робот: скорость 0,-0.078,0                                     | робот: скорость 0,-0.078,0  ← у нас позже на 2                 |
| +40 | робот: Damage Event type=31 cause=0 direct=0 pos=none          | робот: Damage Event type=31 cause=0 direct=0 pos=none  ← у нас позже на 2 |
| +40 | робот: Set Health health=11 food=20 saturation=5               | робот: Set Health health=11 food=20 saturation=5  ← у нас позже на 2 |
| +41 | робот: Metadata 9=11                                           | робот: Metadata 9=11  ← у нас позже на 2                       |
| +41 | робот: скорость 0,-0.078,0                                     | робот: скорость 0,-0.078,0  ← у нас позже на 2                 |
| +60 | робот: Damage Event type=31 cause=0 direct=0 pos=none          | робот: Damage Event type=31 cause=0 direct=0 pos=none  ← у нас позже на 2 |
| +60 | робот: Set Health health=10 food=20 saturation=5               | робот: Set Health health=10 food=20 saturation=5  ← у нас позже на 2 |
| +61 | робот: Metadata 9=10                                           | робот: Metadata 9=10  ← у нас позже на 2                       |
| +61 | робот: скорость 0,-0.078,0                                     | робот: скорость 0,-0.078,0  ← у нас позже на 2                 |
| +80 | робот: Metadata 0=0                                            | робот: Metadata 0=0  ← у нас позже на 5                        |

Случайное, в допуске: 1 (входит в «Совпало»): досыл атрибутов сущности — 1.
Совпало: 10. Сдвинуто по тактам: 15. Нет у нас: 0. Лишнее у нас: 0.
