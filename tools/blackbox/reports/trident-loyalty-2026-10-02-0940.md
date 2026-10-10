# trident-loyalty: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | сущность (3.5,1.52,-0.5) trident 2632 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0 | сущность (3.5,1.52,-0.5) trident 1610 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0  ← случайное, в допуске: номер бросившего и поворот трезубца |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true                                 |
| +0 | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} |
| +0 | робот: b: Slot 36 пусто                                        | робот: b: Slot 36 пусто                                        |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}  ← порядок в такте другой |
| +0 | trident сдвиг 0,0,0 поворот -1/0                               | trident сдвиг 0,0,0 поворот -1/0  ← порядок в такте другой     |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true  ← порядок в такте другой       |
| +0 | робот: other entity_equipment {"equipments":["0:пусто"]}       | робот: other entity_equipment {"equipments":["0:пусто"]}  ← порядок в такте другой |
| +1 | робот: Damage Event type=45 cause=entity direct=entity pos=none | робот: Damage Event type=45 cause=entity direct=entity pos=none |
| +1 | робот: Hurt Animation yaw=90.731                               | робот: Hurt Animation yaw=80.755  ← случайное, в допуске: угол ушиба от трезубца |
| +1 | звук (3.375,1.625,3.125) item.trident.hit 1.00 ~1.00           | звук (3.375,1.625,3.125) item.trident.hit 1.00 ~1.00           |
| +1 | робот: Set Health health=12 food=20 saturation=5               | робот: Set Health health=12 food=20 saturation=5               |
| +2 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]}  ← случайное, в допуске: досыл атрибутов сущности |
| +2 | trident скорость 0,-0.046,-0.024                               | trident скорость 0,-0.046,-0.025  ← случайное, в допуске: скорость трезубца после отскока |
| +2 | trident сдвиг -193,-18,15155 поворот 120/0                     | trident сдвиг -200,-13,15155 поворот 124/0  ← случайное, в допуске: наклон трезубца при броске |
| +2 | робот: Metadata 9=12                                           | робот: Metadata 9=12                                           |
| +2 | робот: скорость -0.003,0.275,0.218                             | робот: скорость 0.035,0.275,0.216  ← случайное, в допуске: горизонтальная отдача от трезубца |
| +2 | звук (3.375,1.625,3.125) item.trident.return 10.00 ~1.00       | звук (3.375,1.625,3.125) item.trident.return 10.00 ~1.00  ← порядок в такте другой |
| +3 | trident скорость 0.002,-0.041,-0.122                           | trident скорость 0.002,-0.041,-0.122                           |
| +3 | trident сдвиг 6,-156,-504 поворот 96/-3                        | trident сдвиг 7,-157,-505 поворот 99/-3  ← случайное, в допуске: сдвиг трезубца по z после отскока |
| +3 | свойства сущности 8=2                                          | свойства сущности 8=2                                          |
| +8 | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} |
| +8 | робот: Collect count=1                                         | робот: Collect count=1  ← порядок в такте другой               |
| +8 | trident collect {"pickupItemCount":1}                          | trident collect {"pickupItemCount":1}  ← порядок в такте другой |
| +8 | trident убран                                                  | trident убран  ← порядок в такте другой                        |
| +9 | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]}  ← у нас раньше на 1 |
| +10 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} |

Случайное, в допуске: 7 (входит в «Совпало»): номер бросившего и поворот трезубца — 1; угол ушиба от трезубца — 1; досыл атрибутов сущности — 1; скорость трезубца после отскока — 1; наклон трезубца при броске — 1; горизонтальная отдача от трезубца — 1; сдвиг трезубца по z после отскока — 1.
Совпало: 27. Сдвинуто по тактам: 1. Нет у нас: 0. Лишнее у нас: 0.
