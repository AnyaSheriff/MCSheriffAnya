# crossbow-piercing: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| +5 | звук (3.5,0,-0.5) item.crossbow.loading_start 0.50 ~1.00       | звук (3.5,0,-0.5) item.crossbow.loading_start 0.50 ~1.20       |
| +13 | звук (3.5,0,-0.5) item.crossbow.loading_middle 0.50 ~1.00      | звук (3.5,0,-0.5) item.crossbow.loading_middle 0.50 ~0.90      |
| +25 | звук (3.5,0,-0.5) item.crossbow.loading_end 1.00 ~0.99         | звук (3.5,0,-0.5) item.crossbow.loading_end 1.00 ~0.94         |
| +25 | робот: other entity_equipment {"equipments":["0:crossbow×1 {charged_projectiles={\"projectiles\":[{\"itemId\":896,\"itemCount\":1,\"addedComponentCount\":0,\"removedComponentCount\":0,\"components\":[],\"removeComponents\":[]}]} enchantments={\"enchantments\":[{\"id\":25,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:crossbow×1 {charged_projectiles={\"projectiles\":[{\"itemId\":896,\"itemCount\":1,\"addedComponentCount\":0,\"removedComponentCount\":0,\"components\":[],\"removeComponents\":[]}]} enchantments={\"enchantments\":[{\"id\":25,\"level\":2}]}}"]} |
| +26 | робот: b: Slot 36 crossbow×1 {charged_projectiles={"projectiles":[{"itemId":896,"itemCount":1,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}]} enchantments={"enchantments":[{"id":25,"level":2}]}} | робот: b: Slot 36 crossbow×1 {charged_projectiles={"projectiles":[{"itemId":896,"itemCount":1,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}]} enchantments={"enchantments":[{"id":25,"level":2}]}} |
| +26 | робот: b: Slot 37 arrow×4                                      | робот: b: Slot 37 arrow×4                                      |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}              |
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | звук (3.5,0,-0.5) item.crossbow.shoot 1.00 ~1.00               | звук (3.5,0,-0.5) item.crossbow.shoot 1.00 ~1.00               |
| +0 | сущность (3.5,1.52,-0.5) arrow 676 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0 | сущность (3.5,1.52,-0.5) arrow 804 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0  ← случайное, в допуске: номер стрелка и поворот стрелы |
| +0 | свойства сущности 8=1 9=2                                      | свойства сущности 8=1 9=2                                      |
| +0 | arrow сдвиг 0,0,0 поворот -1/0                                 | arrow сдвиг 0,0,0 поворот -1/0                                 |
| +0 | свойства сущности 8=1 9=2                                      | свойства сущности 8=1 9=2                                      |
| +0 | робот: other entity_equipment {"equipments":["0:crossbow×1 {damage=1 enchantments={\"enchantments\":[{\"id\":25,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:crossbow×1 {damage=1 enchantments={\"enchantments\":[{\"id\":25,\"level\":2}]}}"]} |
| +1 | робот: Damage Event type=0 cause=entity direct=entity pos=none | робот: Damage Event type=0 cause=entity direct=entity pos=none |
| +1 | робот: Hurt Animation yaw=90.632                               | робот: Hurt Animation yaw=89.554  ← случайное, в допуске: угол ушиба от стрелы |
| +1 | звук (3.375,1.625,3.125) item.crossbow.hit 1.00 ~1.18          | звук (3.375,1.625,3.125) item.crossbow.hit 1.00 ~1.00  ← случайное, в допуске: точка попадания стрелы |
| +1 | робот: Set Health health=9 food=20 saturation=5                | робот: Set Health health=10 food=20 saturation=5  ← случайное, в допуске: урон критической стрелы |
| +2 | arrow скорость -0.034,-0.091,3.118                             | arrow скорость -0.002,-0.084,3.106  ← случайное, в допуске: скорость стрелы после пробития |
| +2 | arrow сдвиг -286,-135,25933 поворот -1/-1                      | arrow сдвиг -16,-79,25832 поворот -1/-2  ← случайное, в допуске: сдвиг стрелы после пробития |
| +2 | робот: Metadata 9=9                                            | робот: Metadata 9=10  ← случайное, в допуске: урон критической стрелы |
| +2 | робот: скорость -0.002,0.275,0.218                             | робот: скорость 0.002,0.275,0.218  ← случайное, в допуске: горизонтальная отдача от стрелы |
| +11 | arrow скорость 0,0,0                                           |   ← случайное, в допуске: остановка пробившей стрелы (у нас не было) |

Случайное, в допуске: 9 (входит в «Совпало»): номер стрелка и поворот стрелы — 1; угол ушиба от стрелы — 1; точка попадания стрелы — 1; урон критической стрелы — 2; скорость стрелы после пробития — 1; сдвиг стрелы после пробития — 1; горизонтальная отдача от стрелы — 1; остановка пробившей стрелы — 1.
Совпало: 23. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
