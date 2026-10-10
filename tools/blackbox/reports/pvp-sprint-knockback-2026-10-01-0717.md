# pvp-sprint-knockback: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: control forward=true** | **▶ 2: control forward=true** |
| | **▶ 2: control sprint=true** (первый отклик через 0 / 0 такт.) | **▶ 2: control sprint=true** |
| +0 | робот: other entity_metadata {"metadata":["0=8"]}              | робот: other entity_metadata {"metadata":["0=8"]}              |
| +0 | робот: other entity_update_attributes {"properties":[{"key":"generic.scale","value":0.1,"modifiers":[{"uuid":"minecraft:sprinting","amount":0.3,"operation":2}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.scale","value":0.1,"modifiers":[{"uuid":"minecraft:sprinting","amount":0.3,"operation":2}]}]} |
| | **▶ 2: attack** (первый отклик через 0 / 0 такт.) | **▶ 2: attack** |
| +0 | звук (3.5,0,2.25) entity.player.attack.knockback 1.00 ~1.00    | звук (3.5,0,2.25) entity.player.attack.knockback 1.00 ~1.00    |
| +0 | робот: Damage Event type=34 cause=entity direct=entity pos=none | робот: Damage Event type=34 cause=entity direct=entity pos=none |
| +0 | робот: Hurt Animation yaw=90                                   | робот: Hurt Animation yaw=90                                   |
| +0 | робот: скорость 0,0.4,0.7                                      | робот: скорость 0,0.4,0.7                                      |
| +0 | звук (3.5,0,2.25) entity.player.attack.strong 1.00 ~1.00       | звук (3.5,0,2.25) entity.player.attack.strong 1.00 ~1.00       |
| +0 | частица (3.5,0.9,3.5) damage_indicator 3 шт. скорость 0.2 (0.1,0,0.1) | частица (3.5,0.9,3.5) damage_indicator 3 шт. скорость 0.2 (0.1,0,0.1) |
| +0 | робот: other animation {"animation":0}                         | робот: other animation {"animation":0}                         |
| +0 | робот: other entity_metadata {"metadata":["0=0"]}              | робот: other entity_metadata {"metadata":["0=0"]}              |
| +0 | робот: other entity_update_attributes {"properties":[{"key":"generic.scale","value":0.1,"modifiers":[]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.scale","value":0.1,"modifiers":[]}]} |
| +0 | робот: Metadata 9=13                                           | робот: Metadata 9=13                                           |
| +0 | робот: Set Health health=13 food=20 saturation=5               | робот: Set Health health=13 food=20 saturation=5               |
| +0 | робот: other entity_equipment {"equipments":["0:diamond_sword×1 {damage=1}"]} | робот: other entity_equipment {"equipments":["0:diamond_sword×1 {damage=1}"]} |
| +1 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4,"operation":0}]}]} |
| | **▶ 2: control forward=false** | **▶ 2: control forward=false** |
| | **▶ 2: control sprint=false** | **▶ 2: control sprint=false** |
| +0 | звук (3.5,0,2.875) block.stone.step 0.15 ~1.00                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +11 | звук (3.5,0,3) block.stone.step 0.15 ~1.00                     |   ← случайное, в допуске: шаги роботов (у нас не было)         |

Случайное, в допуске: 2 (входит в «Совпало»): шаги роботов — 2.
Совпало: 17. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
