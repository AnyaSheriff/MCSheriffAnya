# trident-loyalty: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | сущность (3.5,1.52,-0.5) trident 6874 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0 | сущность (3.5,1.52,-0.5) trident 476 3.5,1.52,-0.5 yaw=-1 pitch=-1 head=0  ← случайное, в допуске: номер бросившего и поворот трезубца |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true                                 |
| +0 | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} |
| +0 | trident сдвиг 0,0,0 поворот -1/0                               |   ← нет у нас                                                  |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true                                 |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}              |
| +0 | робот: other entity_equipment {"equipments":["0:пусто"]}       | робот: other entity_equipment {"equipments":["0:пусто"]}       |
| +0 | робот: b: Slot 36 пусто                                        | робот: b: Slot 36 пусто                                        |
| +0 |                                                                | trident сдвиг 0,0,0 поворот -1/-1  ← лишнее                    |
| +1 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]} |
| +1 | робот: Damage Event type=45 cause=entity direct=entity pos=none | робот: Damage Event type=45 cause=entity direct=entity pos=none  ← порядок в такте другой |
| +1 | робот: Hurt Animation yaw=90.012                               | робот: Hurt Animation yaw=83.331  ← случайное, в допуске: угол ушиба от трезубца; порядок в такте другой |
| +1 | звук (3.375,1.625,3.125) item.trident.hit 1.00 ~1.00           | звук (3.375,1.5,3.125) item.trident.hit 1.00 ~1.00  ← случайное, в допуске: точка удара трезубца; порядок в такте другой |
| +1 | робот: Set Health health=12 food=20 saturation=5               | робот: Set Health health=12 food=20 saturation=5  ← порядок в такте другой |
| +2 | trident скорость 0,-0.048,-0.025                               | trident скорость 0,-0.045,-0.025  ← случайное, в допуске: скорость трезубца после отскока |
| +2 | trident сдвиг -3,86,15155 поворот -124/0                       |   ← нет у нас                                                  |
| +2 | робот: Metadata 9=12                                           | робот: Metadata 9=12                                           |
| +2 | робот: скорость 0,0.275,0.218                                  | робот: скорость 0.025,0.275,0.217  ← случайное, в допуске: горизонтальная отдача от трезубца |
| +2 | звук (3.375,1.625,3.125) item.trident.return 10.00 ~1.00       | звук (3.375,1.5,3.125) item.trident.return 10.00 ~1.00  ← случайное, в допуске: точка удара трезубца; порядок в такте другой |
| +2 |                                                                | trident сдвиг -144,-104,15155 поворот 127/8  ← лишнее          |
| +3 | trident скорость 0,-0.043,-0.122                               | trident скорость 0.001,-0.039,-0.123  ← случайное, в допуске: скорость трезубца после отскока |
| +3 | trident сдвиг 0,-169,-505 поворот -99/-3                       |   ← нет у нас                                                  |
| +3 | свойства сущности 8=2                                          | свойства сущности 8=2                                          |
| +3 |                                                                | trident сдвиг 5,-146,-505 поворот 127/8  ← лишнее              |
| +8 | робот: Collect count=1                                         | робот: Collect count=1                                         |
| +8 | trident collect {"pickupItemCount":1}                          | trident collect {"pickupItemCount":1}                          |
| +8 | trident убран                                                  | trident убран                                                  |
| +8 | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}}  ← порядок в такте другой |
| +9 | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]}  ← у нас позже на 6 |
| +11 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]}  ← случайное, в допуске: досыл атрибутов сущности |

Случайное, в допуске: 8 (входит в «Совпало»): номер бросившего и поворот трезубца — 1; угол ушиба от трезубца — 1; точка удара трезубца — 2; скорость трезубца после отскока — 2; горизонтальная отдача от трезубца — 1; досыл атрибутов сущности — 1.
Совпало: 24. Сдвинуто по тактам: 1. Нет у нас: 3. Лишнее у нас: 3.
