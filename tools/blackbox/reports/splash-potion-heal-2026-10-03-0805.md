# splash-potion-heal: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ use_item** (первый отклик через 0 / 0 такт.) | **▶ use_item** |
| +0 | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.36         | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.34         |
| +0 | сущность (3.5,1.52,3.5) splash_potion 1125 3.5,1.52,3.5 yaw=-52 pitch=63 head=0 | сущность (3.5,1.52,3.5) splash_potion 1391 3.5,1.52,3.5 yaw=10 pitch=62 head=0  ← случайное, в допуске: номер бросившего и поворот зелья |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} |
| +0 | инвентарь все:                                                 | инвентарь все:                                                 |
| +0 | splash_potion сдвиг 0,0,0                                      | splash_potion сдвиг 0,0,0                                      |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} |
| +3 |                                                                | блок (5,-2,-1) dirt  ← лишнее                                  |
| +10 | splash_potion скорость -0.006,-0.025,0.002                     | splash_potion скорость 0.005,-0.026,0.021  ← случайное, в допуске: скорость зелья на сверке |
| +10 | splash_potion сдвиг -253,8418,83 поворот -52/37                | splash_potion сдвиг 231,8369,880 поворот 10/35  ← случайное, в допуске: сдвиг и поворот зелья на сверке |
| +13 |                                                                | блок (4,-2,-3) dirt  ← лишнее                                  |
| +14 |                                                                | блок (0,-2,-1) dirt  ← лишнее                                  |
| +15 | блок (6,-2,-6) dirt                                            |   ← нет у нас                                                  |
| +16 | событие мира (3,2,3) 2007/-515037                              | событие мира (3,2,3) 2007/-515037  ← у нас позже на 1          |
| +16 | splash_potion убран                                            | splash_potion убран  ← у нас позже на 1                        |
| +16 | робот: Set Health health=14 food=20 saturation=5               | робот: Set Health health=14 food=20 saturation=5  ← у нас позже на 1 |
| +17 | робот: Metadata 9=14                                           | робот: Metadata 9=14  ← у нас позже на 1                       |
| +18 | блок (3,-2,-4) dirt                                            |   ← нет у нас                                                  |
| +26 | блок (6,-2,0) dirt                                             |   ← нет у нас                                                  |
| +35 |                                                                | блок (0,-2,-4) dirt  ← лишнее                                  |

Случайное, в допуске: 3 (входит в «Совпало»): номер бросившего и поворот зелья — 1; скорость зелья на сверке — 1; сдвиг и поворот зелья на сверке — 1.
Совпало: 8. Сдвинуто по тактам: 4. Нет у нас: 3. Лишнее у нас: 4.
