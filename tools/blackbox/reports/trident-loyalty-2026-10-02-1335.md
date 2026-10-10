# trident-loyalty: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | сущность (3.5,1.52,-0.5) trident 1713 3.5,1.52,-0.5 yaw=0 pitch=-1 head=0 | сущность (3.5,1.52,-0.5) trident 1359 3.5,1.52,-0.5 yaw=0 pitch=-1 head=0  ← случайное, в допуске: номер бросившего и поворот трезубца |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true                                 |
| +0 | робот: b: Slot 36 пусто                                        | робот: b: Slot 36 пусто                                        |
| +0 | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1}  ← порядок в такте другой |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}  ← порядок в такте другой |
| +0 | trident сдвиг 0,0,0 поворот 0/-1                               | trident сдвиг 0,0,0 поворот 0/-1  ← порядок в такте другой     |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true  ← порядок в такте другой       |
| +0 | робот: other entity_equipment {"equipments":["0:пусто"]}       | робот: other entity_equipment {"equipments":["0:пусто"]}  ← порядок в такте другой |
| +1 | робот: Damage Event type=45 cause=entity direct=entity pos=none | робот: Damage Event type=45 cause=entity direct=entity pos=none |
| +1 | робот: Hurt Animation yaw=89.441                               | робот: Hurt Animation yaw=99.439  ← случайное, в допуске: угол ушиба от трезубца |
| +1 | звук (3.5,1.5,3.125) item.trident.hit 1.00 ~1.00               | звук (3.5,1.5,3.125) item.trident.hit 1.00 ~1.00               |
| +1 | робот: Set Health health=12 food=20 saturation=5               | робот: Set Health health=12 food=20 saturation=5               |
| +2 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]}  ← случайное, в допуске: досыл атрибутов сущности |
| +2 | trident скорость 0,-0.043,-0.025                               | trident скорость 0,-0.044,-0.024  ← случайное, в допуске: скорость трезубца после отскока |
| +2 | trident сдвиг 148,-207,15155 поворот 127/-1                    | trident сдвиг 204,-180,15155 поворот -126/-1  ← случайное, в допуске: наклон трезубца при броске |
| +2 | робот: Metadata 9=12                                           | робот: Metadata 9=12                                           |
| +2 | робот: скорость 0.002,0.275,0.218                              | робот: скорость -0.036,0.275,0.215  ← случайное, в допуске: горизонтальная отдача от трезубца |
| +2 | звук (3.5,1.5,3.125) item.trident.return 10.00 ~1.00           | звук (3.5,1.5,3.125) item.trident.return 10.00 ~1.00  ← порядок в такте другой |
| +3 | trident скорость -0.001,-0.037,-0.122                          | trident скорость -0.002,-0.037,-0.122  ← случайное, в допуске: скорость трезубца после отскока |
| +3 | trident сдвиг -5,-133,-504 поворот 102/-3                      | trident сдвиг -7,-137,-504 поворот -100/-3  ← случайное, в допуске: сдвиг трезубца по z после отскока |
| +3 | свойства сущности 8=2                                          | свойства сущности 8=2                                          |
| +8 | робот: Collect count=1                                         | робот: Collect count=1                                         |
| +8 | trident collect {"pickupItemCount":1}                          | trident collect {"pickupItemCount":1}                          |
| +8 | trident убран                                                  | trident убран                                                  |
| +8 | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} |
| +9 | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]}  ← у нас раньше на 1 |
| +10 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} |

Случайное, в допуске: 8 (входит в «Совпало»): номер бросившего и поворот трезубца — 1; угол ушиба от трезубца — 1; досыл атрибутов сущности — 1; скорость трезубца после отскока — 2; наклон трезубца при броске — 1; горизонтальная отдача от трезубца — 1; сдвиг трезубца по z после отскока — 1.
Совпало: 27. Сдвинуто по тактам: 1. Нет у нас: 0. Лишнее у нас: 0.
