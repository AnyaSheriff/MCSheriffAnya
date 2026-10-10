# villager-claim: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon villager $(3.5,0,3.5)** (первый отклик через 0 / 0 такт.) | **▶ cmd summon villager $(3.5,0,3.5)** |
| +0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=20 20=true                                 | свойства сущности 9=20 20=true                                 |
| +0 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0                                           |
| +0 | свойства сущности 9=20 20=true                                 | свойства сущности 9=20 20=true                                 |
| +3 | villager скорость 0,-0.078,0                                   | villager скорость 0,-0.078,0                                   |
| +3 | villager сверка (3.5,0,3.5) скорость 0,-0.078,0 поворот 0/0 на земле | villager сверка (3.5,0,3.5) скорость 0,-0.078,0 поворот 0/0 на земле |
| +3 | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} |
| | **▶ set 3,0,4 minecraft:composter** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,4 minecraft:composter** |
| +0 | блок (3,0,4) composter[level=0]                                | блок (3,0,4) composter[level=0]                                |
| +7 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.88            | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.16  ← случайное, в допуске: голос жителя в покое |
| +28 | villager событие 14                                            | villager событие 14  ← случайное, в допуске: занял рабочий блок |
| +29 | свойства сущности 19={"villagerType":2,"villagerProfession":5,"level":1} | свойства сущности 19={"villagerType":2,"villagerProfession":5,"level":1}  ← случайное, в допуске: занял рабочий блок |
| +40 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +100 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +128 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.01            | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.96  ← случайное, в допуске: голос жителя в покое |
| +160 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +220 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| | **▶ set 3,0,4 minecraft:glass** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,4 minecraft:glass** |
| +0 | блок (3,0,4) glass                                             | блок (3,0,4) glass                                             |
| +1 | свойства сущности 19={"villagerType":2,"villagerProfession":0,"level":1} | свойства сущности 19={"villagerType":2,"villagerProfession":0,"level":1} |
| +50 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +51 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.93            | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.00  ← случайное, в допуске: голос жителя в покое |
| +110 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |

Случайное, в допуске: 5 (входит в «Совпало»): голос жителя в покое — 3; занял рабочий блок — 2.
Совпало: 21. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
