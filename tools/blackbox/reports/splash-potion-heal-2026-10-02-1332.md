# splash-potion-heal: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ use_item** (первый отклик через 1 / 0 такт.) | **▶ use_item** |
| +0 | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.42         | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.37         |
| +0 | сущность (3.5,1.52,3.5) splash_potion 1688 3.5,1.52,3.5 yaw=5 pitch=63 head=0 | сущность (3.5,1.52,3.5) splash_potion 1323 3.5,1.52,3.5 yaw=-88 pitch=63 head=0  ← случайное, в допуске: номер бросившего и поворот зелья |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} |
| +0 | инвентарь все:                                                 | инвентарь все:                                                 |
| +0 | splash_potion сдвиг 0,0,0                                      | splash_potion сдвиг 0,0,0                                      |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} |
| +1 | блок (4,-2,-1) dirt                                            |   ← нет у нас                                                  |
| +9 | блок (8,-2,-3) dirt                                            |   ← нет у нас                                                  |
| +9 | splash_potion скорость 0.001,-0.017,0.006                      | splash_potion скорость -0.009,-0.026,-0.006  ← у нас позже на 1 |
| +9 | splash_potion сдвиг 36,8730,268 поворот 5/39                   | splash_potion сдвиг -365,8345,-238 поворот -88/37  ← у нас позже на 1 |
| +10 | блок (1,-2,0) dirt                                             |   ← нет у нас                                                  |
| +14 | блок (2,-2,-5) dirt                                            |   ← нет у нас                                                  |
| +17 | событие мира (3,2,3) 2007/-515037                              | событие мира (3,2,3) 2007/-515037                              |
| +17 | splash_potion убран                                            | splash_potion убран                                            |
| +17 | робот: Set Health health=14 food=20 saturation=5               | робот: Set Health health=14 food=20 saturation=5               |
| +17 | робот: Metadata 9=14                                           | робот: Metadata 9=14  ← у нас позже на 1                       |
| +26 | блок (-1,-2,-4) dirt                                           |   ← нет у нас                                                  |

Случайное, в допуске: 1 (входит в «Совпало»): номер бросившего и поворот зелья — 1.
Совпало: 9. Сдвинуто по тактам: 3. Нет у нас: 5. Лишнее у нас: 0.
