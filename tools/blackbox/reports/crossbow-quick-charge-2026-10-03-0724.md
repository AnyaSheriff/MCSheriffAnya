# crossbow-quick-charge: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| +3 | звук (3.5,0,-0.5) item.crossbow.quick_charge_2 0.50 ~1.00      | звук (3.5,0,-0.5) item.crossbow.quick_charge_2 0.50 ~0.99      |
| +15 | звук (3.5,0,-0.5) item.crossbow.loading_end 1.00 ~0.97         | звук (3.5,0,-0.5) item.crossbow.loading_end 1.00 ~0.88         |
| +15 | робот: other entity_equipment {"equipments":["0:crossbow×1 {charged_projectiles={\"projectiles\":[{\"itemId\":896,\"itemCount\":1,\"addedComponentCount\":0,\"removedComponentCount\":0,\"components\":[],\"removeComponents\":[]}]} enchantments={\"enchantments\":[{\"id\":30,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:crossbow×1 {charged_projectiles={\"projectiles\":[{\"itemId\":896,\"itemCount\":1,\"addedComponentCount\":0,\"removedComponentCount\":0,\"components\":[],\"removeComponents\":[]}]} enchantments={\"enchantments\":[{\"id\":30,\"level\":2}]}}"]} |
| +16 | робот: b: Slot 36 crossbow×1 {charged_projectiles={"projectiles":[{"itemId":896,"itemCount":1,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}]} enchantments={"enchantments":[{"id":30,"level":2}]}} | робот: b: Slot 36 crossbow×1 {charged_projectiles={"projectiles":[{"itemId":896,"itemCount":1,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}]} enchantments={"enchantments":[{"id":30,"level":2}]}} |
| +16 | робот: b: Slot 37 arrow×4                                      | робот: b: Slot 37 arrow×4                                      |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}              |
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | звук (3.5,0,-0.5) item.crossbow.shoot 1.00 ~1.00               | звук (3.5,0,-0.5) item.crossbow.shoot 1.00 ~1.00               |
| +0 | сущность (3.5,1.52,-0.5) arrow 683 3.5,1.52,-0.5 yaw=0 pitch=0 head=0 | сущность (3.5,1.52,-0.5) arrow 812 3.5,1.52,-0.5 yaw=0 pitch=-1 head=0  ← случайное, в допуске: номер стрелка и поворот стрелы |
| +0 | робот: other entity_equipment {"equipments":["0:crossbow×1 {damage=1 enchantments={\"enchantments\":[{\"id\":30,\"level\":2}]}}"]} | робот: other entity_equipment {"equipments":["0:crossbow×1 {damage=1 enchantments={\"enchantments\":[{\"id\":30,\"level\":2}]}}"]} |
| +1 | робот: Damage Event type=0 cause=entity direct=entity pos=none | робот: Damage Event type=0 cause=entity direct=entity pos=none |
| +1 | робот: Hurt Animation yaw=89.813                               | робот: Hurt Animation yaw=98.703  ← случайное, в допуске: угол ушиба от стрелы |
| +1 | звук (3.5,1.625,3.125) item.crossbow.hit 1.00 ~1.11            | звук (3.5,1.5,3.125) item.crossbow.hit 1.00 ~1.00  ← случайное, в допуске: точка попадания стрелы |
| +1 | робот: Set Health health=12 food=20 saturation=5               | робот: Set Health health=13 food=20 saturation=5  ← случайное, в допуске: урон критической стрелы |
| +2 | робот: Metadata 9=12 12=1                                      | робот: Metadata 9=13 12=1  ← случайное, в допуске: урон критической стрелы |
| +2 | робот: скорость 0.001,0.275,0.218                              | робот: скорость -0.033,0.275,0.216  ← случайное, в допуске: горизонтальная отдача от стрелы |

Случайное, в допуске: 6 (входит в «Совпало»): номер стрелка и поворот стрелы — 1; угол ушиба от стрелы — 1; точка попадания стрелы — 1; урон критической стрелы — 2; горизонтальная отдача от стрелы — 1.
Совпало: 16. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
