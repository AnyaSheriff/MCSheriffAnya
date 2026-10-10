# villager-trade-ai: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon villager $(3.5,0,3.5) {Xp:20,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:2},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:1,priceMultiplier:0.05f,rewardExp:1b}]}}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon villager $(3.5,0,3.5) {Xp:20,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:2},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:1,priceMultiplier:0.05f,rewardExp:1b}]}}** |
| +0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=20 19={"villagerType":2,"villagerProfession":5,"level":2} 20=true | свойства сущности 9=20 19={"villagerType":2,"villagerProfession":5,"level":2} 20=true |
| +0 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0                                           |
| +0 | свойства сущности 9=20 19={"villagerType":2,"villagerProfession":5,"level":2} 20=true | свойства сущности 9=20 19={"villagerType":2,"villagerProfession":5,"level":2} 20=true |
| +3 | villager скорость 0,-0.078,0                                   | villager скорость 0,-0.078,0                                   |
| +3 | villager сверка (3.5,0,3.5) скорость 0,-0.078,0 поворот 0/0 на земле | villager сверка (3.5,0,3.5) скорость 0,-0.078,0 поворот 0/0 на земле |
| +3 | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} |
| +10 | villager событие 14                                            |   ← нет у нас                                                  |
| +26 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.98            | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.06  ← случайное, в допуске: голос жителя в покое |
| +40 | окно 19                                                        | окно 19                                                        |
| +40 | содержимое окна 30:wheat×64 31:emerald×3                       | содержимое окна 30:wheat×64 31:emerald×3                       |
| +40 | робот: Merchant Offers level=2 xp=20 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=1 special=0 mult=0.05 demand=0] | робот: Merchant Offers level=2 xp=20 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=1 special=0 mult=0.05 demand=0] |
| | **▶ select_trade 0** (первый отклик через 0 / 0 такт.) | **▶ select_trade 0** |
| +0 | звук (3.5,0,3.5) entity.villager.yes 1.00 ~1.12                | звук (3.5,0,3.5) entity.villager.yes 1.00 ~1.02  ← случайное, в допуске: «да» жителя |
| +0 | ячейка 0: wheat×64                                             | ячейка 0: wheat×64                                             |
| +0 | ячейка 2: emerald×1                                            | ячейка 2: emerald×1                                            |
| +0 | ячейка 30: пусто                                               | ячейка 30: пусто                                               |
| | **▶ window_click {"slot":2,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":0}** |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=5                                          | свойства сущности 8=5                                          |
| +0 | ячейка 0: wheat×44                                             | ячейка 0: wheat×44                                             |
| +0 | курсор: emerald×1                                              | курсор: emerald×1                                              |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=5                                          | свойства сущности 8=5  ← порядок в такте другой                |
| +0 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле  ← порядок в такте другой        |
| +0 | villager событие 14                                            | villager событие 14  ← порядок в такте другой                  |
| +10 |                                                                | experience_orb collect {"pickupItemCount":1}  ← случайное, в допуске: ход и подбор шариков опыта (у оригинала не было) |
| +10 |                                                                | experience_orb убран  ← случайное, в допуске: ход и подбор шариков опыта (у оригинала не было) |
| +10 |                                                                | робот: Set Experience bar=0.714 level=0 total=5  ← случайное, в допуске: опыт робота (у оригинала не было) |
| | **▶ close** (первый отклик через 0 / 0 такт.) | **▶ close** |
| +0 | experience_orb скорость 0.003,0,0                              |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +0 | experience_orb сверка (3.471,0,3.25) скорость 0.003,0,0 поворот 260.331/0 на земле |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +0 | инвентарь 36: wheat×44                                         | инвентарь 36: wheat×44                                         |
| +0 | инвентарь 37: emerald×4                                        | инвентарь 37: emerald×4                                        |
| +20 | experience_orb скорость 0,0,0                                  |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +20 | experience_orb сдвиг 78,0,0 на земле                           |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +40 | experience_orb сдвиг 5,0,0 на земле                            |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +40 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +85 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.09            |   ← случайное, в допуске: голос жителя в покое (у нас не было) |
| +100 | experience_orb сдвиг 0,0,0 на земле                            |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +100 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |

Случайное, в допуске: 12 (входит в «Совпало»): голос жителя в покое — 2; «да» жителя — 1; ход и подбор шариков опыта — 8; опыт робота — 1.
Совпало: 37. Сдвинуто по тактам: 0. Нет у нас: 1. Лишнее у нас: 0.
