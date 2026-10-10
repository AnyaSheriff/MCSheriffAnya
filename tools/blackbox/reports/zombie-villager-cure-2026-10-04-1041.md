# zombie-villager-cure: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon zombie_villager $(3.5,0,3.5) {NoAI:1b,VillagerData:{type:"minecraft:taiga",profession:"minecraft:farmer",level:2},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:2,priceMultiplier:0.05f},{buy:{id:"minecraft:emerald",count:1},sell:{id:"minecraft:bread",count:6},maxUses:16,xp:1,priceMultiplier:0.05f}]}}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon zombie_villager $(3.5,0,3.5) {NoAI:1b,VillagerData:{type:"minecraft:taiga",profession:"minecraft:farmer",level:2},Offers:{Recipes:[{buy:{id:"minecraft:wheat",count:20},sell:{id:"minecraft:emerald",count:1},maxUses:16,xp:2,priceMultiplier:0.05f},{buy:{id:"minecraft:emerald",count:1},sell:{id:"minecraft:bread",count:6},maxUses:16,xp:1,priceMultiplier:0.05f}]}}** |
| +0 | сущность (3.5,0,3.5) zombie_villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,3.5) zombie_villager 0 3.5,0,3.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=20 15=1 20={"villagerType":6,"villagerProfession":5,"level":2} 21=true | свойства сущности 9=20 15=1 20={"villagerType":6,"villagerProfession":5,"level":2} 21=true |
| +0 | zombie_villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.23,"modifiers":[]}]} | zombie_villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.23,"modifiers":[]}]} |
| +0 | zombie_villager сдвиг 0,0,0                                    | zombie_villager сдвиг 0,0,0                                    |
| +0 | свойства сущности 9=20 15=1 20={"villagerType":6,"villagerProfession":5,"level":2} 21=true | свойства сущности 9=20 15=1 20={"villagerType":6,"villagerProfession":5,"level":2} 21=true |
| +0 | zombie_villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.23,"modifiers":[]}]} | zombie_villager entity_update_attributes {"properties":[{"key":"generic.scale","value":0.23,"modifiers":[]}]} |
| | **▶ cmd effect give @e[type=zombie_villager,distance=..48] minecraft:weakness 60** (первый отклик через 0 / 0 такт.) | **▶ cmd effect give @e[type=zombie_villager,distance=..48] minecraft:weakness 60** |
| +0 | свойства сущности 10=[{"type":"entity_effect","data":-12038840}] | свойства сущности 10=[{"type":"entity_effect","data":-12038840}] |
| +15 | zombie_villager событие 16                                     | zombie_villager событие 16                                     |
| +15 | свойства сущности 10=[{"type":"entity_effect","data":-14592}] 19=true | свойства сущности 10=[{"type":"entity_effect","data":-14592}] 19=true |
| +15 | инвентарь 36: golden_apple×3                                   | инвентарь 36: golden_apple×3                                   |
| +15 | звук (3.5,0,3.5) entity.zombie_villager.ambient 1.00 ~1.06     |   ← нет у нас                                                  |
| | **▶ cmd kill @e[type=zombie_villager,distance=..48]** (первый отклик через 0 / 0 такт.) | **▶ cmd kill @e[type=zombie_villager,distance=..48]** |
| +0 | zombie_villager damage_event {"sourceTypeId":19,"sourceCauseId":0,"sourceDirectId":0} | zombie_villager damage_event {"sourceTypeId":19,"sourceCauseId":0,"sourceDirectId":0} |
| +0 | звук (3.5,0,3.5) entity.zombie_villager.death 1.00 ~1.05       | звук (3.5,0,3.5) entity.zombie_villager.death 1.00 ~1.07       |
| +0 | zombie_villager событие 3                                      | zombie_villager событие 3                                      |
| +0 | zombie_villager сдвиг 0,0,0                                    | zombie_villager сдвиг 0,0,0                                    |
| +0 | свойства сущности 6=7 9=0                                      | свойства сущности 6=7 9=0                                      |
| +0 | zombie_villager скорость 0,0,0                                 | zombie_villager скорость 0,0,0                                 |
| +0 |                                                                | сущность (3.5,0,3.5) item 0  ← случайное, в допуске: добыча (у оригинала не было) |
| +0 |                                                                | свойства сущности 8={"itemCount":1,"itemId":1115,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}  ← случайное, в допуске: добыча (у оригинала не было) |
| +0 |                                                                | свойства сущности 8={"itemCount":1,"itemId":1115,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}  ← случайное, в допуске: добыча (у оригинала не было) |
| +19 | zombie_villager событие 60                                     | zombie_villager событие 60                                     |
| +19 | zombie_villager убран                                          | zombie_villager убран                                          |

Случайное, в допуске: 3 (входит в «Совпало»): добыча — 3.
Совпало: 21. Сдвинуто по тактам: 0. Нет у нас: 1. Лишнее у нас: 0.
