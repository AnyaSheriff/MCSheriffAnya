# villager-trade: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon villager $(3.5,0,3.5) {NoAI:1b,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:1},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:2,priceMultiplier:0.05f,rewardExp:1b},{buy:{id:"minecraft:emerald",count:1},sell:{id:"minecraft:bread",count:6},maxUses:16,xp:1,priceMultiplier:0.05f,rewardExp:1b}]}}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon villager $(3.5,0,3.5) {NoAI:1b,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:1},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:2,priceMultiplier:0.05f,rewardExp:1b},{buy:{id:"minecraft:emerald",count:1},sell:{id:"minecraft:bread",count:6},maxUses:16,xp:1,priceMultiplier:0.05f,rewardExp:1b}]}}** |
| +0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true |
| +0 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0                                           |
| +0 | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true |
| +3 | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} |
| +3 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.91            |   ← случайное, в допуске: голос жителя в покое (у нас не было) |
| +20 | окно 19                                                        | окно 19                                                        |
| +20 | содержимое окна 30:wheat×64 31:emerald×3                       | содержимое окна 30:wheat×64 31:emerald×3                       |
| +20 | робот: Merchant Offers level=1 xp=0 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=2 special=0 mult=0.05 demand=0] [emerald×1+-→bread×6 off=false uses=0/16 xp=1 special=0 mult=0.05 demand=0] | робот: Merchant Offers level=1 xp=0 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=2 special=0 mult=0.05 demand=0] [emerald×1+-→bread×6 off=false uses=0/16 xp=1 special=0 mult=0.05 demand=0] |
| | **▶ select_trade 0** (первый отклик через 0 / 0 такт.) | **▶ select_trade 0** |
| +0 | звук (3.5,0,3.5) entity.villager.yes 1.00 ~1.01                | звук (3.5,0,3.5) entity.villager.yes 1.00 ~0.97  ← случайное, в допуске: «да» жителя |
| +0 | ячейка 0: wheat×64                                             | ячейка 0: wheat×64                                             |
| +0 | ячейка 2: emerald×1                                            | ячейка 2: emerald×1                                            |
| +0 | ячейка 30: пусто                                               | ячейка 30: пусто                                               |
| | **▶ window_click {"slot":2,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":1}** |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=4                                          | свойства сущности 8=5  ← случайное, в допуске: очки шарика опыта |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=5                                          | свойства сущности 8=3  ← случайное, в допуске: очки шарика опыта |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=5                                          | свойства сущности 8=4  ← случайное, в допуске: очки шарика опыта |
| +0 | ячейка 0: wheat×4                                              | ячейка 0: wheat×4                                              |
| +0 | ячейка 2: пусто                                                | ячейка 2: пусто                                                |
| +0 | ячейка 31: emerald×6                                           | ячейка 31: emerald×6                                           |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=5                                          | свойства сущности 8=5  ← порядок в такте другой                |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=5                                          | свойства сущности 8=3  ← случайное, в допуске: очки шарика опыта; порядок в такте другой |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=4                                          | свойства сущности 8=4  ← порядок в такте другой                |
| +4 |                                                                | робот: Set Experience bar=0.714 level=0 total=5  ← случайное, в допуске: опыт робота (у оригинала не было) |
| +6 | experience_orb collect {"pickupItemCount":1}                   | experience_orb collect {"pickupItemCount":1}  ← случайное, в допуске: ход и подбор шариков опыта |
| +6 | experience_orb убран                                           | experience_orb убран  ← случайное, в допуске: ход и подбор шариков опыта |
| +6 | робот: Set Experience bar=0.714 level=0 total=31               |   ← случайное, в допуске: опыт робота (у нас не было)          |
| +8 | experience_orb collect {"pickupItemCount":1}                   | experience_orb collect {"pickupItemCount":1}                   |
| +8 | experience_orb убран                                           | experience_orb убран                                           |
| +8 | робот: Set Experience bar=0.222 level=1 total=35               |   ← случайное, в допуске: опыт робота (у нас не было)          |
| +8 |                                                                | робот: Set Experience bar=0.222 level=1 total=9  ← случайное, в допуске: опыт робота (у оригинала не было) |
| | **▶ select_trade 1** (первый отклик через 0 / 0 такт.) | **▶ select_trade 1** |
| +0 | ячейка 0: emerald×6                                            | ячейка 0: emerald×6                                            |
| +0 | ячейка 2: bread×6                                              | ячейка 2: bread×6                                              |
| +0 | ячейка 31: пусто                                               | ячейка 31: пусто                                               |
| +0 | ячейка 38: wheat×4                                             | ячейка 38: wheat×4                                             |
| +0 | experience_orb collect {"pickupItemCount":1}                   | experience_orb collect {"pickupItemCount":1}  ← случайное, в допуске: ход и подбор шариков опыта |
| +0 | experience_orb убран                                           | experience_orb убран  ← случайное, в допуске: ход и подбор шариков опыта |
| +0 | робот: Set Experience bar=0.778 level=1 total=40               |   ← случайное, в допуске: опыт робота (у нас не было)          |
| +2 |                                                                | робот: Set Experience bar=0.556 level=1 total=12  ← случайное, в допуске: опыт робота (у оригинала не было) |
| | **▶ window_click {"slot":2,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":0}** |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=3                                          | свойства сущности 8=3                                          |
| +0 | ячейка 0: emerald×5                                            | ячейка 0: emerald×5                                            |
| +0 | курсор: bread×6                                                | курсор: bread×6                                                |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=3                                          | свойства сущности 8=3  ← порядок в такте другой                |
| +0 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0  ← порядок в такте другой                 |
| +6 | experience_orb collect {"pickupItemCount":1}                   | experience_orb collect {"pickupItemCount":1}  ← случайное, в допуске: ход и подбор шариков опыта |
| +6 | experience_orb убран                                           | experience_orb убран  ← случайное, в допуске: ход и подбор шариков опыта |
| +6 | робот: Set Experience bar=0.091 level=2 total=43               |   ← случайное, в допуске: опыт робота (у нас не было)          |
| +9 |                                                                | робот: Set Experience bar=0.889 level=1 total=15  ← случайное, в допуске: опыт робота (у оригинала не было) |
| | **▶ close** (первый отклик через 0 / 0 такт.) | **▶ close** |
| +0 | инвентарь 36: bread×6                                          | инвентарь 36: bread×6                                          |
| +0 | инвентарь 37: emerald×5                                        | инвентарь 37: emerald×5                                        |

Случайное, в допуске: 20 (входит в «Совпало»): голос жителя в покое — 1; «да» жителя — 1; очки шарика опыта — 4; ход и подбор шариков опыта — 6; опыт робота — 8.
Совпало: 57. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
