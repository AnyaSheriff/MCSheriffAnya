# splash-potion-feet: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ use_item** (первый отклик через 0 / 0 такт.) | **▶ use_item** |
| +0 | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.40         | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.44         |
| +0 | сущность (3.5,1.52,3.5) splash_potion 1121 3.5,1.52,3.5 yaw=23 pitch=-64 head=0 | сущность (3.5,1.52,3.5) splash_potion 1384 3.5,1.52,3.5 yaw=-22 pitch=-63 head=0  ← случайное, в допуске: номер бросившего и поворот зелья |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":13,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":13,"customEffects":[]}}],"removeComponents":[]} |
| +0 | инвентарь все:                                                 | инвентарь все:                                                 |
| +0 | splash_potion сдвиг 0,0,0                                      | splash_potion сдвиг 0,0,0                                      |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":13,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":13,"customEffects":[]}}],"removeComponents":[]} |
| +1 | блок (7,-2,1) dirt                                             |   ← нет у нас                                                  |
| +2 | робот: Effect id=0 amp=0 dur=3600 flags=14                     | робот: Effect id=0 amp=0 dur=3600 flags=14                     |
| +2 | событие мира (3,0,3) 2002/-13374465                            | событие мира (3,0,3) 2002/-13374465                            |
| +2 | splash_potion убран                                            | splash_potion убран                                            |
| +3 | блок (0,-2,0) dirt                                             |   ← нет у нас                                                  |
| +3 | робот: Metadata 10=[{"type":"entity_effect","data":-13374465}] | робот: Metadata 10=[{"type":"entity_effect","data":-13374465}] |
| +3 | робот: Attributes [{"key":"generic.scale","value":0.10000000149011612,"modifiers":[{"uuid":"minecraft:effect.speed","amount":0.20000000298023224,"operation":2}]}] | робот: Attributes [{"key":"generic.scale","value":0.10000000149011612,"modifiers":[{"uuid":"minecraft:effect.speed","amount":0.20000000298023224,"operation":2}]}] |
| +28 |                                                                | блок (-2,-2,-6) dirt  ← лишнее                                 |
| +39 | блок (1,-2,-3) dirt                                            |   ← нет у нас                                                  |
| +44 |                                                                | блок (-1,-2,-1) dirt  ← лишнее                                 |
| +47 |                                                                | блок (3,-2,0) dirt  ← лишнее                                   |
| +49 | блок (3,-2,1) dirt                                             |   ← нет у нас                                                  |
| +54 |                                                                | блок (7,-2,0) dirt  ← лишнее                                   |

Случайное, в допуске: 1 (входит в «Совпало»): номер бросившего и поворот зелья — 1.
Совпало: 11. Сдвинуто по тактам: 0. Нет у нас: 4. Лишнее у нас: 4.
