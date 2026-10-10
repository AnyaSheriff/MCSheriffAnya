# villager-levelup: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon villager $(3.5,0,3.5) {NoAI:1b,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:1},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:10,priceMultiplier:0.05f,rewardExp:1b}]}}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon villager $(3.5,0,3.5) {NoAI:1b,VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:1},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:10,priceMultiplier:0.05f,rewardExp:1b}]}}** |
| +0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,3.5) villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true |
| +0 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0                                           |
| +0 | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true | свойства сущности 9=20 15=1 19={"villagerType":2,"villagerProfession":5,"level":1} 20=true |
| +3 | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} | villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.5,"modifiers":[]}]} |
| +12 |                                                                | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.00  ← случайное, в допуске: голос жителя в покое (у оригинала не было) |
| +20 | окно 19                                                        | окно 19                                                        |
| +20 | содержимое окна 30:wheat×64 31:emerald×3                       | содержимое окна 30:wheat×64 31:emerald×3                       |
| +20 | робот: Merchant Offers level=1 xp=0 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=10 special=0 mult=0.05 demand=0] | робот: Merchant Offers level=1 xp=0 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=0/16 xp=10 special=0 mult=0.05 demand=0] |
| | **▶ select_trade 0** (первый отклик через 0 / 0 такт.) | **▶ select_trade 0** |
| +0 | звук (3.5,0,3.5) entity.villager.yes 1.00 ~0.84                |   ← случайное, в допуске: «да» жителя (у нас не было)          |
| +0 | ячейка 0: wheat×64                                             | ячейка 0: wheat×64                                             |
| +0 | ячейка 2: emerald×1                                            | ячейка 2: emerald×1                                            |
| +0 | ячейка 30: пусто                                               | ячейка 30: пусто                                               |
| | **▶ window_click {"slot":2,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":0}** |
| +0 | сущность (3.5,0.5,3.5) experience_orb 0                        | сущность (3.5,0.5,3.5) experience_orb 0                        |
| +0 | свойства сущности 8=10                                         | свойства сущности 8=10                                         |
| +0 | ячейка 0: wheat×44                                             | ячейка 0: wheat×44                                             |
| +0 | курсор: emerald×1                                              | курсор: emerald×1                                              |
| +0 | experience_orb сдвиг 0,0,0                                     | experience_orb сдвиг 0,0,0  ← порядок в такте другой           |
| +0 | свойства сущности 8=10                                         | свойства сущности 8=10  ← порядок в такте другой               |
| +0 |                                                                | звук (3.5,0,3.5) entity.villager.yes 1.00 ~1.04  ← случайное, в допуске: «да» жителя (у оригинала не было) |
| +5 | experience_orb collect {"pickupItemCount":1}                   | experience_orb collect {"pickupItemCount":1}  ← случайное, в допуске: ход и подбор шариков опыта |
| +5 | experience_orb убран                                           | experience_orb убран  ← случайное, в допуске: ход и подбор шариков опыта |
| +5 | робот: Set Experience bar=0.333 level=1 total=21               |   ← случайное, в допуске: опыт робота (у нас не было)          |
| +16 |                                                                | робот: Set Experience bar=0.333 level=1 total=10  ← случайное, в допуске: опыт робота (у оригинала не было) |
| +20 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0                                           |
| +80 | villager сдвиг 0,0,0                                           | villager сдвиг 0,0,0                                           |
| | **▶ close** (первый отклик через 0 / 0 такт.) | **▶ close** |
| +0 | инвентарь 36: wheat×44                                         | инвентарь 36: wheat×44                                         |
| +0 | инвентарь 37: emerald×4                                        | инвентарь 37: emerald×4                                        |
| +4 | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~0.87            | звук (3.5,0,3.5) entity.villager.ambient 1.00 ~1.12  ← случайное, в допуске: голос жителя в покое |
| +10 | окно 19                                                        | окно 19                                                        |
| +10 | содержимое окна 30:wheat×44 31:emerald×4                       | содержимое окна 30:wheat×44 31:emerald×4                       |
| +10 | робот: Merchant Offers level=1 xp=10 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=1/16 xp=10 special=0 mult=0.05 demand=0] | робот: Merchant Offers level=1 xp=10 regular=true restock=true [wheat×20+-→emerald×1 off=false uses=1/16 xp=10 special=0 mult=0.05 demand=0] |
| | **▶ interact** | **▶ interact** |

Случайное, в допуске: 8 (входит в «Совпало»): голос жителя в покое — 2; «да» жителя — 2; ход и подбор шариков опыта — 2; опыт робота — 2.
Совпало: 32. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
