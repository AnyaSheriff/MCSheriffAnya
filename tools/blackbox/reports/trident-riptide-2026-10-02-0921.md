# trident-riptide: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| +0 | робот: b: Metadata 8=1                                         | робот: b: Metadata 8=1                                         |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | робот: b: Entity Sound minecraft:item.trident.riptide_3 cat=player vol=1 | робот: b: Entity Sound minecraft:item.trident.riptide_3 cat=player vol=1 |
| +0 | робот: other entity_sound_effect {"sound":"minecraft:item.trident.riptide_3","soundCategory":"player","volume":1,"pitch":1} | робот: other entity_sound_effect {"sound":"minecraft:item.trident.riptide_3","soundCategory":"player","volume":1,"pitch":1}  ← порядок в такте другой |
| +0 | робот: other entity_metadata {"metadata":["8=4"]}              | робот: other entity_metadata {"metadata":["8=4"]}  ← порядок в такте другой |
| +0 | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":32,\"level\":3}]}}"]} | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":32,\"level\":3}]}}"]}  ← порядок в такте другой |
| +0 | робот: b: Metadata 8=4                                         | робот: b: Metadata 8=4                                         |
| +0 | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":32,"level":3}]}} | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":32,"level":3}]}} |
| +1 | робот: other entity_metadata {"metadata":["6=4"]}              | робот: other entity_metadata {"metadata":["6=4"]}              |
| +1 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} |
| +1 | робот: b: Metadata 6=4                                         | робот: b: Metadata 6=4  ← порядок в такте другой               |
| +1 | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9000000953674316,"operation":0}]}] | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9000000953674316,"operation":0}]}]  ← случайное, в допуске: досыл атрибутов сущности |
| +11 | звук (3.5,0,-0.5) entity.player.splash 0.18 ~1.19              |   ← нет у нас                                                  |
| +11 | звук (3.5,0,0.5) entity.player.swim 0.17 ~0.93                 |   ← нет у нас                                                  |
| +11 | звук (3.5,0,0.5) block.stone.step 0.05 ~0.80                   |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +12 | робот: other entity_metadata {"metadata":["1=299"]}            |   ← нет у нас                                                  |
| +12 | звук (3.5,0,0.25) entity.player.swim 0.13 ~0.90                |   ← нет у нас                                                  |
| +12 | звук (3.5,0,0.25) block.stone.step 0.05 ~0.80                  |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +12 | робот: b: Metadata 1=299                                       |   ← нет у нас                                                  |
| +13 | робот: other entity_metadata {"metadata":["1=298"]}            |   ← нет у нас                                                  |
| +13 | робот: b: Metadata 1=298                                       |   ← нет у нас                                                  |
| +14 | робот: other entity_metadata {"metadata":["1=297"]}            |   ← нет у нас                                                  |
| +14 | робот: b: Metadata 1=297                                       |   ← нет у нас                                                  |
| +15 | робот: other entity_metadata {"metadata":["1=296"]}            |   ← нет у нас                                                  |
| +15 | звук (3.5,0,-0.125) entity.player.swim 0.07 ~1.17              |   ← нет у нас                                                  |
| +15 | звук (3.5,0,-0.125) block.stone.step 0.05 ~0.80                |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +15 | робот: b: Metadata 1=296                                       |   ← нет у нас                                                  |
| +16 | робот: other entity_metadata {"metadata":["1=295"]}            |   ← нет у нас                                                  |
| +16 | робот: b: Metadata 1=295                                       |   ← нет у нас                                                  |
| +17 | робот: other entity_metadata {"metadata":["1=294"]}            |   ← нет у нас                                                  |
| +17 | робот: b: Metadata 1=294                                       |   ← нет у нас                                                  |
| +18 | робот: other entity_metadata {"metadata":["1=293"]}            |   ← нет у нас                                                  |
| +18 | робот: b: Metadata 1=293                                       |   ← нет у нас                                                  |
| +19 | робот: other entity_metadata {"metadata":["1=292"]}            |   ← нет у нас                                                  |
| +19 | робот: b: Metadata 1=292                                       |   ← нет у нас                                                  |
| +20 | робот: other entity_metadata {"metadata":["1=291","6=0","8=0"]} |   ← нет у нас                                                  |
| +20 | робот: b: Metadata 1=291 6=0 8=0                               |   ← нет у нас                                                  |
| +20 |                                                                | робот: b: Metadata 6=0 8=0  ← лишнее                           |
| +21 | робот: other entity_metadata {"metadata":["1=295"]}            |   ← нет у нас                                                  |
| +21 | робот: b: Metadata 1=295                                       |   ← нет у нас                                                  |
| +21 |                                                                | робот: other entity_metadata {"metadata":["6=0","8=0"]}  ← лишнее |
| +22 | робот: other entity_metadata {"metadata":["1=299"]}            |   ← нет у нас                                                  |
| +22 | робот: b: Metadata 1=299                                       |   ← нет у нас                                                  |
| +23 | робот: other entity_metadata {"metadata":["1=300"]}            |   ← нет у нас                                                  |
| +23 | робот: b: Metadata 1=300                                       |   ← нет у нас                                                  |
| +26 | звук (3.5,0,-0.5) entity.player.swim 0.01 ~1.10                |   ← нет у нас                                                  |
| +26 | звук (3.5,0,-0.5) block.stone.step 0.05 ~0.80                  |   ← случайное, в допуске: шаги роботов (у нас не было)         |

Случайное, в допуске: 5 (входит в «Совпало»): досыл атрибутов сущности — 1; шаги роботов — 4.
Совпало: 16. Сдвинуто по тактам: 0. Нет у нас: 29. Лишнее у нас: 2.
