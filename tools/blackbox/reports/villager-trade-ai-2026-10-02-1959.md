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
| +19 | villager событие 14                                            | villager событие 14  ← случайное, в допуске: занял рабочий блок |
| +40 | окно 19                                                        | окно 19                                                        |
| +40 | содержимое окна 30:wheat×64 31:emerald×3                       | содержимое окна 30:wheat×64 31:emerald×3                       |
| +40 | робот: Merchant Offers level=2 xp=20 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=1 special=0 mult=0.05 demand=0] | робот: Merchant Offers level=2 xp=20 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=1 special=0 mult=0.05 demand=0] |
| +44 |                                                                | звук (3.5,0,3.5) entity.villager.trade 1.00 ~1.15  ← случайное, в допуске: голос жителя в покое (у оригинала не было) |
| | **▶ select_trade 0** (первый отклик через 0 / 0 такт.) | **▶ select_trade 0** |
| +0 | звук (3.5,0,3.5) entity.villager.yes 1.00 ~0.96                |   ← случайное, в допуске: «да» жителя (у нас не было)          |
| +0 | ячейка 0: wheat×64                                             | ячейка 0: wheat×64                                             |
| +0 | ячейка 2: emerald×1                                            | ячейка 2: emerald×1                                            |
| +0 | ячейка 30: пусто                                               | ячейка 30: пусто                                               |
| | **▶ window_click {"slot":2,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":0}** |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=6                                          | свойства сущности 8=6                                          |
| +0 | ячейка 0: wheat×44                                             | ячейка 0: wheat×44                                             |
| +0 | курсор: emerald×1                                              | курсор: emerald×1                                              |
| +0 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле  ← порядок в такте другой        |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=6                                          | свойства сущности 8=6  ← порядок в такте другой                |
| +0 | villager событие 14                                            | villager событие 14  ← порядок в такте другой                  |
| +4 |                                                                | experience_orb collect {"pickupItemCount":1}  ← случайное, в допуске: ход и подбор шариков опыта (у оригинала не было) |
| +4 |                                                                | experience_orb убран  ← случайное, в допуске: ход и подбор шариков опыта (у оригинала не было) |
| +4 |                                                                | робот: Set Experience bar=0.857 level=0 total=6  ← случайное, в допуске: опыт робота (у оригинала не было) |
| | **▶ close** (первый отклик через 0 / 0 такт.) | **▶ close** |
| +0 | experience_orb скорость 0.009,0.007,0                          |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +0 | experience_orb сдвиг 184,-2020,-1024                           |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +0 | инвентарь 36: wheat×44                                         | инвентарь 36: wheat×44                                         |
| +0 | инвентарь 37: emerald×4                                        | инвентарь 37: emerald×4                                        |
| +20 | experience_orb скорость -0.001,0,0                             |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +20 | experience_orb сверка (3.535,0,3.25) скорость -0.001,0,0 поворот 111.207/0 на земле |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +40 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +40 | experience_orb скорость 0,0,0                                  |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +40 | experience_orb сдвиг -75,0,0 на земле                          |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +60 | experience_orb сдвиг -33,0,0 на земле                          |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |
| +89 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.97            | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.08  ← случайное, в допуске: голос жителя в покое |
| +100 | villager сдвиг 0,0,0 на земле                                  | villager сдвиг 0,0,0 на земле                                  |
| +100 | experience_orb сдвиг 0,0,0 на земле                            |   ← случайное, в допуске: ход и подбор шариков опыта (у нас не было) |

Случайное, в допуске: 15 (входит в «Совпало»): занял рабочий блок — 1; голос жителя в покое — 2; «да» жителя — 1; ход и подбор шариков опыта — 10; опыт робота — 1.
Совпало: 40. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
