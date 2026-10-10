# trident-riptide-rain: оригинал против нас

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
| +1 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]}  ← случайное, в допуске: досыл атрибутов сущности |
| +1 | звук (3.5,0,2.125) block.stone.step 0.15 ~1.00                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +1 | робот: b: Metadata 6=4                                         | робот: b: Metadata 6=4                                         |
| +1 | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9000000953674316,"operation":0}]}] | робот: b: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9000000953674316,"operation":0}]}]  ← случайное, в допуске: досыл атрибутов сущности |
| +2 | звук (3.5,0,0.875) block.stone.step 0.15 ~1.00                 |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +4 | звук (3.5,0,-0.125) block.stone.step 0.15 ~1.00                |   ← случайное, в допуске: шаги роботов (у нас не было)         |
| +20 | робот: other entity_metadata {"metadata":["6=0","8=0"]}        | робот: other entity_metadata {"metadata":["6=0","8=0"]}  ← у нас позже на 1 |
| +20 | робот: b: Metadata 6=0 8=0                                     | робот: b: Metadata 6=0 8=0                                     |

Случайное, в допуске: 5 (входит в «Совпало»): досыл атрибутов сущности — 2; шаги роботов — 3.
Совпало: 15. Сдвинуто по тактам: 2. Нет у нас: 0. Лишнее у нас: 0.
