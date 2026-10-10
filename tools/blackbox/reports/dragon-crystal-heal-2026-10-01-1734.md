# dragon-crystal-heal: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon ender_dragon $(0.5,6,0.5) {NoAI:1b}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon ender_dragon $(0.5,6,0.5) {NoAI:1b}** |
| +0 | сущность (0.5,6,0.5) ender_dragon 0 0.5,6,0.5 yaw=0 pitch=0 head=0 | сущность (0.5,6,0.5) ender_dragon 0 0.5,6,0.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=200 15=1                                   | свойства сущности 9=200 15=1                                   |
| +0 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0                                       |
| +0 | свойства сущности 9=200 15=1                                   | свойства сущности 9=200 15=1                                   |
| | **▶ cmd summon end_crystal $(3.5,0,0.5)** (первый отклик через 0 / 0 такт.) | **▶ cmd summon end_crystal $(3.5,0,0.5)** |
| +0 | сущность (3.5,0,0.5) end_crystal 0 3.5,0,0.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,0.5) end_crystal 0 3.5,0,0.5 yaw=0 pitch=0 head=0 |
| +0 | end_crystal сдвиг 0,0,0                                        | end_crystal сдвиг 0,0,0                                        |
| +4 | звук (0.5,6,0.5) entity.ender_dragon.ambient 5.00 ~1.04        | звук (0.5,6,0.5) entity.ender_dragon.ambient 5.00 ~0.83  ← случайное, в допуске: голос дракона в покое |
| +40 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0                                       |
| +60 |                                                                | end_crystal сдвиг 0,0,0  ← лишнее                              |
| | **▶ attack** (первый отклик через 0 / 0 такт.) | **▶ attack** |
| +0 | end_crystal убран                                              | end_crystal убран                                              |
| +1 | ender_dragon damage_event {"sourceTypeId":35,"sourceCauseId":"entity","sourceDirectId":"entity"} | ender_dragon damage_event {"sourceTypeId":35,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← у нас раньше на 1 |
| +1 | звук (0.5,6,0.5) entity.ender_dragon.hurt 5.00 ~1.10           | звук (0.5,6,0.5) entity.ender_dragon.hurt 5.00 ~1.02  ← у нас раньше на 1 |
| +1 | взрыв (3.5,0,0.5) сила 6 блоков 1512 explosion_emitter entity.generic.explode толчок есть дым [poof:0.5/1x1 smoke:1/1x1] | взрыв (3.5,0,0.5) сила 6 блоков 1505 explosion_emitter entity.generic.explode толчок есть дым [poof:0.5/1x1 smoke:1/1x1]  ← у нас раньше на 1 |
| +1 | звук (0.5,0,0.5) entity.player.attack.strong 1.00 ~1.00        | звук (0.5,0,0.5) entity.player.attack.strong 1.00 ~1.00  ← у нас раньше на 1 |
| +1 | ender_dragon скорость -0.101,0.429,0                           | ender_dragon скорость -0.101,0.429,0  ← у нас раньше на 1      |
| +1 | свойства сущности 9=192.07778930664062                         | свойства сущности 9=192.07778930664062  ← у нас раньше на 1    |
| +1 | ender_dragon скорость -0.101,0.429,0                           | ender_dragon скорость -0.101,0.429,0  ← у нас раньше на 1      |
| +20 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0                                       |

Случайное, в допуске: 1 (входит в «Совпало»): голос дракона в покое — 1.
Совпало: 10. Сдвинуто по тактам: 7. Нет у нас: 0. Лишнее у нас: 1.
