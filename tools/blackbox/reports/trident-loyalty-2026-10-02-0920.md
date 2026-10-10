# trident-loyalty: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | сущность (3.5,1.52,-0.5) trident 2402 3.5,1.52,-0.5 yaw=0 pitch=-1 head=0 | сущность (3.5,1.52,-0.5) trident 1282 3.5,1.52,-0.5 yaw=0 pitch=0 head=0  ← случайное, в допуске: номер бросившего и поворот трезубца |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true                                 |
| +0 | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} | trident entity_sound_effect {"sound":"minecraft:item.trident.throw","soundCategory":"player","volume":1,"pitch":1} |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}              |
| +0 | trident сдвиг 0,0,0 поворот 0/-1                               | trident сдвиг 0,0,0 поворот 0/0  ← случайное, в допуске: наклон трезубца при броске; порядок в такте другой |
| +0 | свойства сущности 11=2 12=true                                 | свойства сущности 11=2 12=true  ← порядок в такте другой       |
| +0 | робот: other entity_equipment {"equipments":["0:пусто"]}       | робот: other entity_equipment {"equipments":["0:пусто"]}       |
| +0 | робот: b: Slot 36 пусто                                        | робот: b: Slot 36 пусто                                        |
| +1 | робот: Damage Event type=45 cause=entity direct=entity pos=none | робот: Damage Event type=45 cause=entity direct=entity pos=none |
| +1 | робот: Hurt Animation yaw=89.934                               | робот: Hurt Animation yaw=95.897  ← случайное, в допуске: угол ушиба от трезубца |
| +1 | звук (3.5,1.5,3.125) item.trident.hit 1.00 ~1.00               | звук (3.5,1.625,3.125) item.trident.hit 1.00 ~1.00  ← случайное, в допуске: точка удара трезубца |
| +1 | робот: Set Health health=12 food=20 saturation=5               | робот: Set Health health=12 food=20 saturation=5               |
| +2 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[]}]} |
| +2 | trident скорость 0,-0.044,-0.024                               | trident скорость 0,-0.045,-0.024  ← случайное, в допуске: скорость трезубца после отскока; порядок в такте другой |
| +2 | trident сдвиг 18,-175,15155 поворот 121/-1                     | trident сдвиг 127,-76,15155 поворот -125/-1  ← случайное, в допуске: наклон трезубца при броске; порядок в такте другой |
| +2 | робот: Metadata 9=12                                           | робот: Metadata 9=12  ← порядок в такте другой                 |
| +2 | робот: скорость 0,0.275,0.218                                  | робот: скорость -0.022,0.275,0.217  ← случайное, в допуске: горизонтальная отдача от трезубца; порядок в такте другой |
| +2 | звук (3.5,1.5,3.125) item.trident.return 10.00 ~1.00           | звук (3.5,1.625,3.125) item.trident.return 10.00 ~1.00  ← случайное, в допуске: точка удара трезубца; порядок в такте другой |
| +3 | trident скорость 0,-0.037,-0.122                               | trident скорость -0.001,-0.04,-0.122  ← случайное, в допуске: скорость трезубца после отскока |
| +3 | trident сдвиг -1,-138,-503 поворот 97/-3                       | trident сдвиг -4,-150,-504 поворот -100/-3  ← случайное, в допуске: сдвиг трезубца по z после отскока |
| +3 | свойства сущности 8=2                                          | свойства сущности 8=2                                          |
| +8 | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} | робот: b: Slot 36 trident×1 {damage=1 enchantments={"enchantments":[{"id":19,"level":2}]}} |
| +8 | робот: Collect count=1                                         | робот: Collect count=1                                         |
| +8 | trident collect {"pickupItemCount":1}                          | trident collect {"pickupItemCount":1}                          |
| +8 | trident убран                                                  | trident убран                                                  |
| +9 | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:trident×1 {damage=1 enchantments={\"enchantments\":[{\"id\":19,\"level\":2}]}}"]}  ← у нас раньше на 1 |
| +10 | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} | робот: other entity_update_attributes {"properties":[{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.9,"operation":0}]}]} |
| +36 | блок (1,-2,-6) dirt                                            |   ← нет у нас                                                  |

Случайное, в допуске: 10 (входит в «Совпало»): номер бросившего и поворот трезубца — 1; наклон трезубца при броске — 2; угол ушиба от трезубца — 1; точка удара трезубца — 2; скорость трезубца после отскока — 2; горизонтальная отдача от трезубца — 1; сдвиг трезубца по z после отскока — 1.
Совпало: 27. Сдвинуто по тактам: 1. Нет у нас: 1. Лишнее у нас: 0.
