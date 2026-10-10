# trident-riptide: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| +0 | робот: b: Metadata 8=1                                         | робот: b: Metadata 8=1  ← порядок в такте другой               |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | робот: b: Entity Sound minecraft:item.trident.riptide_3 cat=player vol=1 | робот: b: Entity Sound minecraft:item.trident.riptide_3 cat=player vol=1 |
| +0 | робот: other entity_sound_effect {"sound":"minecraft:item.trident.riptide_3","soundCategory":"player","volume":1,"pitch":1} | робот: other entity_sound_effect {"sound":"minecraft:item.trident.riptide_3","soundCategory":"player","volume":1,"pitch":1}  ← порядок в такте другой |
| +0 | робот: other entity_metadata {"metadata":["8=4"]}              | робот: other entity_metadata {"metadata":["8=4"]}  ← порядок в такте другой |
| +0 | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":32,\"level\":3}]}}"]} | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":32,\"level\":3}]}}"]}  ← порядок в такте другой |
| +0 | робот: b: Metadata 8=4                                         | робот: b: Metadata 8=4                                         |
| +0 | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":32,"level":3}]}} | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":32,"level":3}]}} |
| +1 | робот: other entity_metadata {"metadata":["6=4"]}              | робот: other entity_metadata {"metadata":["6=4"]}  ← у нас позже на 1 |
| +1 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} |
| +1 | робот: b: Metadata 6=4                                         | робот: b: Metadata 6=4  ← порядок в такте другой               |
| +1 | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9000000953674316,"operation":0}]}] | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9000000953674316,"operation":0}]}]  ← случайное, в допуске: досыл атрибутов сущности |
| +2 | звук (3.5,0,-0.5) entity.player.splash 0.22 ~0.71              |   ← нет у нас                                                  |
| +2 | звук (3.5,0,1.875) entity.player.swim 0.39 ~1.28               |   ← нет у нас                                                  |
| +2 | звук (3.5,0,1.875) block.stone.step 0.05 ~0.80                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +3 | робот: other entity_metadata {"metadata":["1=299"]}            |   ← нет у нас                                                  |
| +3 | звук (3.5,0,1.375) entity.player.swim 0.31 ~1.33               |   ← нет у нас                                                  |
| +3 | звук (3.5,0,1.375) block.stone.step 0.05 ~0.80                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +3 | робот: b: Metadata 1=299                                       |   ← нет у нас                                                  |
| +4 | робот: other entity_metadata {"metadata":["1=298"]}            |   ← нет у нас                                                  |
| +4 | звук (3.5,0,1) entity.player.swim 0.25 ~1.15                   |   ← нет у нас                                                  |
| +4 | звук (3.5,0,1) block.stone.step 0.05 ~0.80                     |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +4 | робот: b: Metadata 1=298                                       |   ← нет у нас                                                  |
| +5 | робот: other entity_metadata {"metadata":["1=297"]}            |   ← нет у нас                                                  |
| +5 | робот: b: Metadata 1=297                                       |   ← нет у нас                                                  |
| +6 | робот: other entity_metadata {"metadata":["1=296"]}            |   ← нет у нас                                                  |
| +6 | звук (3.5,0,0.5) entity.player.swim 0.16 ~1.09                 |   ← нет у нас                                                  |
| +6 | звук (3.5,0,0.5) block.stone.step 0.05 ~0.80                   |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +6 | робот: b: Metadata 1=296                                       |   ← нет у нас                                                  |
| +7 | робот: other entity_metadata {"metadata":["1=295"]}            |   ← нет у нас                                                  |
| +7 | робот: b: Metadata 1=295                                       |   ← нет у нас                                                  |
| +8 | робот: other entity_metadata {"metadata":["1=294"]}            |   ← нет у нас                                                  |
| +8 | звук (3.5,0,0.125) entity.player.swim 0.10 ~1.17               |   ← нет у нас                                                  |
| +8 | звук (3.5,0,0.125) block.stone.step 0.05 ~0.80                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +8 | робот: b: Metadata 1=294                                       |   ← нет у нас                                                  |
| +9 | робот: other entity_metadata {"metadata":["1=293"]}            |   ← нет у нас                                                  |
| +9 | робот: b: Metadata 1=293                                       |   ← нет у нас                                                  |
| +10 | робот: other entity_metadata {"metadata":["1=292"]}            |   ← нет у нас                                                  |
| +10 | робот: b: Metadata 1=292                                       |   ← нет у нас                                                  |
| +11 | робот: other entity_metadata {"metadata":["1=291"]}            |   ← нет у нас                                                  |
| +11 | звук (3.5,0,-0.25) entity.player.swim 0.05 ~1.28               |   ← нет у нас                                                  |
| +11 | звук (3.5,0,-0.25) block.stone.step 0.05 ~0.80                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +11 | робот: b: Metadata 1=291                                       |   ← нет у нас                                                  |
| +12 | робот: other entity_metadata {"metadata":["1=290"]}            |   ← нет у нас                                                  |
| +12 | робот: b: Metadata 1=290                                       |   ← нет у нас                                                  |
| +13 | робот: other entity_metadata {"metadata":["1=289"]}            |   ← нет у нас                                                  |
| +13 | робот: b: Metadata 1=289                                       |   ← нет у нас                                                  |
| +14 | робот: other entity_metadata {"metadata":["1=288"]}            |   ← нет у нас                                                  |
| +14 | робот: b: Metadata 1=288                                       |   ← нет у нас                                                  |
| +15 | робот: other entity_metadata {"metadata":["1=287"]}            |   ← нет у нас                                                  |
| +15 | робот: b: Metadata 1=287                                       |   ← нет у нас                                                  |
| +16 | робот: other entity_metadata {"metadata":["1=286"]}            |   ← нет у нас                                                  |
| +16 | робот: b: Metadata 1=286                                       |   ← нет у нас                                                  |
| +17 | робот: other entity_metadata {"metadata":["1=285"]}            |   ← нет у нас                                                  |
| +17 | робот: b: Metadata 1=285                                       |   ← нет у нас                                                  |
| +18 | робот: other entity_metadata {"metadata":["1=284"]}            |   ← нет у нас                                                  |
| +18 | робот: b: Metadata 1=284                                       |   ← нет у нас                                                  |
| +19 | робот: other entity_metadata {"metadata":["1=283"]}            |   ← нет у нас                                                  |
| +19 | робот: b: Metadata 1=283                                       |   ← нет у нас                                                  |
| +20 | робот: other entity_metadata {"metadata":["1=282","6=0","8=0"]} |   ← нет у нас                                                  |
| +20 | робот: b: Metadata 1=282 6=0 8=0                               |   ← нет у нас                                                  |
| +20 |                                                                | робот: b: Metadata 6=0 8=0  ← лишнее                           |
| +21 | робот: other entity_metadata {"metadata":["1=286"]}            |   ← нет у нас                                                  |
| +21 | робот: b: Metadata 1=286                                       |   ← нет у нас                                                  |
| +21 |                                                                | робот: other entity_metadata {"metadata":["6=0","8=0"]}  ← лишнее |
| +22 | робот: other entity_metadata {"metadata":["1=290"]}            |   ← нет у нас                                                  |
| +22 | робот: b: Metadata 1=290                                       |   ← нет у нас                                                  |
| +23 | робот: other entity_metadata {"metadata":["1=294"]}            |   ← нет у нас                                                  |
| +23 | робот: b: Metadata 1=294                                       |   ← нет у нас                                                  |
| +24 | робот: other entity_metadata {"metadata":["1=298"]}            |   ← нет у нас                                                  |
| +24 | робот: b: Metadata 1=298                                       |   ← нет у нас                                                  |
| +25 | робот: other entity_metadata {"metadata":["1=300"]}            |   ← нет у нас                                                  |
| +25 | робот: b: Metadata 1=300                                       |   ← нет у нас                                                  |

Случайное, в допуске: 7 (входит в «Совпало»): досыл атрибутов сущности — 1; шаги роботов — 6.
Совпало: 17. Сдвинуто по тактам: 1. Нет у нас: 53. Лишнее у нас: 2.
