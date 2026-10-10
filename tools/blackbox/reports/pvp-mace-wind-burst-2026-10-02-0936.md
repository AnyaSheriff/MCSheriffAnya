# pvp-mace-wind-burst: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd tp $(bot2) $(3.5,4,1.5) 0 0** | **▶ cmd tp $(bot2) $(3.5,4,1.5) 0 0** |
| | **▶ 2: attack** (первый отклик через 0 / 0 такт.) | **▶ 2: attack** |
| +0 | робот: Damage Event type=26 cause=entity direct=entity pos=none | робот: Damage Event type=26 cause=entity direct=entity pos=none |
| +0 | робот: Hurt Animation yaw=90                                   | робот: Hurt Animation yaw=90                                   |
| +0 | робот: скорость 0,0.361,0.4                                    | робот: скорость 0,0.361,0.4                                    |
| +0 | звук (3.5,2,1.5) entity.player.attack.crit 1.00 ~1.00          | звук (3.5,2,1.5) entity.player.attack.crit 1.00 ~1.00          |
| +0 | робот: Animate anim=4                                          | робот: Animate anim=4                                          |
| +0 | звук (3.5,2,1.5) item.mace.smash_ground 1.00 ~1.00             | звук (3.5,2,1.5) item.mace.smash_ground 1.00 ~1.00             |
| +0 | событие мира (3,-1,3) 2013/750                                 | событие мира (3,-1,3) 2013/750                                 |
| +0 | робот: b: Velocity 0,0.01,0                                    | робот: b: Velocity 0,0.01,0  ← порядок в такте другой          |
| +0 | взрыв (3.5,1.89,1.5) сила 3.5 блоков 836 gust_emitter_large entity.wind_charge.wind_burst толчок есть дым [] | взрыв (3.5,1.79,1.5) сила 3.5 блоков 617 gust_emitter_large entity.wind_charge.wind_burst толчок есть дым []  ← случайное, в допуске: высота взрыва ветра |
| +0 | частица (3.5,0.9,3.5) damage_indicator 7 шт. скорость 0.2 (0.1,0,0.1) | частица (3.5,0.9,3.5) damage_indicator 7 шт. скорость 0.2 (0.1,0,0.1) |
| +0 | робот: other animation {"animation":0}                         | робот: other animation {"animation":0}                         |
| +0 | робот: Metadata 9=4.216925621032715                            |   ← нет у нас                                                  |
| +0 | робот: Set Health health=4.217 food=20 saturation=5            | робот: Set Health health=4.217 food=20 saturation=5            |
| +0 | робот: other entity_equipment {"equipments":["0:mace×1 {damage=1 enchantments={\"enchantments\":[{\"id\":42,\"level\":1}]}}"]} | робот: other entity_equipment {"equipments":["0:mace×1 {damage=1 enchantments={\"enchantments\":[{\"id\":42,\"level\":1}]}}"]} |
| +0 |                                                                | робот: Metadata 9=4.216923713684082  ← лишнее                  |
| +1 | робот: Attributes [{"key":"generic.armor","value":0,"modifiers":[{"uuid":"minecraft:armor.leggings","amount":6,"operation":0},{"uuid":"minecraft:armor.chestplate","amount":8,"operation":0}]},{"key":"generic.armor_toughness","value":0,"modifiers":[{"uuid":"minecraft:armor.leggings","amount":2,"operation":0},{"uuid":"minecraft:armor.chestplate","amount":2,"operation":0}]}] | робот: Attributes [{"key":"generic.armor","value":0,"modifiers":[{"uuid":"minecraft:armor.leggings","amount":6,"operation":0},{"uuid":"minecraft:armor.chestplate","amount":8,"operation":0}]},{"key":"generic.armor_toughness","value":0,"modifiers":[{"uuid":"minecraft:armor.leggings","amount":2,"operation":0},{"uuid":"minecraft:armor.chestplate","amount":2,"operation":0}]}]  ← случайное, в допуске: досыл атрибутов сущности |
| +2 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-3.4,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-3.4,"operation":0}]}]}  ← случайное, в допуске: досыл атрибутов сущности |
| +2 | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-3.4000000953674316,"operation":0}]}] | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-3.4000000953674316,"operation":0}]}] |
| +31 | частица (3.5,0,1.5) block 200 шт. скорость 0.15 (0.3,0.3,0.3)  |   ← нет у нас                                                  |
| +31 | частица (3.5,0,1.5) block 90 шт. скорость 0.15 (0,0,0)         |   ← нет у нас                                                  |

Случайное, в допуске: 3 (входит в «Совпало»): высота взрыва ветра — 1; досыл атрибутов сущности — 2.
Совпало: 16. Сдвинуто по тактам: 0. Нет у нас: 3. Лишнее у нас: 1.
