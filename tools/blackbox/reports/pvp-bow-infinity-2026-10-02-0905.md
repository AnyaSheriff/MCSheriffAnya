# pvp-bow-infinity: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ 2: use_item** (первый отклик через 0 / 0 такт.) | **▶ 2: use_item** |
| +0 | робот: other entity_metadata {"metadata":["8=1"]}              | робот: other entity_metadata {"metadata":["8=1"]}              |
| +0 | робот: b: Metadata 8=1                                         | робот: b: Metadata 8=1  ← порядок в такте другой               |
| | **▶ 2: release_use** (первый отклик через 0 / 0 такт.) | **▶ 2: release_use** |
| +0 | сущность (3.5,1.52,-0.5) arrow 2222 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0 | сущность (3.5,1.52,-0.5) arrow 996 3.5,1.52,-0.5 yaw=-1 pitch=0 head=0  ← случайное, в допуске: номер стрелка и поворот стрелы |
| +0 | звук (3.5,0,-0.5) entity.arrow.shoot 1.00 ~1.07                | звук (3.5,0,-0.5) entity.arrow.shoot 1.00 ~1.13  ← порядок в такте другой |
| +0 | робот: other entity_metadata {"metadata":["8=0"]}              | робот: other entity_metadata {"metadata":["8=0"]}              |
| +0 | робот: other entity_equipment {"equipments":["0:bow×1 {damage=1 enchantments={\"enchantments\":[{\"id\":16,\"level\":1}]}}"]} | робот: other entity_equipment {"equipments":["0:bow×1 {damage=1 enchantments={\"enchantments\":[{\"id\":16,\"level\":1}]}}"]} |
| +0 | робот: b: Metadata 8=0                                         | робот: b: Metadata 8=0                                         |
| +1 | робот: b: Game Event reason=play_arrow_hit_sound value=0       | робот: b: Game Event reason=play_arrow_hit_sound value=0       |
| +1 | робот: Damage Event type=0 cause=entity direct=entity pos=none | робот: Damage Event type=0 cause=entity direct=entity pos=none  ← порядок в такте другой |
| +1 | робот: Hurt Animation yaw=90.097                               | робот: Hurt Animation yaw=89.469  ← случайное, в допуске: угол ушиба от стрелы; порядок в такте другой |
| +1 | звук (3.375,1.625,3.125) entity.arrow.hit 1.00 ~1.26           | звук (3.375,1.5,3.125) entity.arrow.hit 1.00 ~1.00  ← случайное, в допуске: точка попадания стрелы; порядок в такте другой |
| +1 | робот: Set Health health=15 food=20 saturation=5               | робот: Set Health health=15 food=20 saturation=5  ← порядок в такте другой |
| +2 | робот: Metadata 9=15 12=1                                      | робот: Metadata 9=15 12=1                                      |
| +2 | робот: скорость 0,0.275,0.218                                  | робот: скорость 0.002,0.275,0.218  ← случайное, в допуске: горизонтальная отдача от стрелы |

Случайное, в допуске: 4 (входит в «Совпало»): номер стрелка и поворот стрелы — 1; угол ушиба от стрелы — 1; точка попадания стрелы — 1; горизонтальная отдача от стрелы — 1.
Совпало: 14. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
