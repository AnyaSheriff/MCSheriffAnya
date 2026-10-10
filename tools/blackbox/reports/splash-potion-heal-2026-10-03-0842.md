# splash-potion-heal: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ use_item** (первый отклик через 0 / 0 такт.) | **▶ use_item** |
| +0 | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.37         | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.38         |
| +0 | сущность (3.5,1.52,3.5) splash_potion 1461 3.5,1.52,3.5 yaw=-40 pitch=63 head=0 | сущность (3.5,1.52,3.5) splash_potion 1868 3.5,1.52,3.5 yaw=-64 pitch=63 head=0  ← случайное, в допуске: номер бросившего и поворот зелья |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} |
| +0 | инвентарь все:                                                 | инвентарь все:                                                 |
| +0 | splash_potion сдвиг 0,0,0                                      | splash_potion сдвиг 0,0,0                                      |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":25,"customEffects":[]}}],"removeComponents":[]} |
| +10 | splash_potion скорость -0.005,-0.018,0.003                     | splash_potion скорость -0.007,-0.011,0  ← случайное, в допуске: скорость зелья на сверке |
| +10 | splash_potion сдвиг -204,8699,140 поворот -40/38               | splash_potion сдвиг -302,8998,2 поворот -64/40  ← случайное, в допуске: сдвиг и поворот зелья на сверке |
| +17 | событие мира (3,2,3) 2007/-515037                              | событие мира (3,2,3) 2007/-515037                              |
| +17 | splash_potion убран                                            | splash_potion убран                                            |
| +17 | робот: Set Health health=14 food=20 saturation=5               | робот: Set Health health=14 food=20 saturation=5               |
| +18 | робот: Metadata 9=14                                           | робот: Metadata 9=14                                           |

Случайное, в допуске: 3 (входит в «Совпало»): номер бросившего и поворот зелья — 1; скорость зелья на сверке — 1; сдвиг и поворот зелья на сверке — 1.
Совпало: 12. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
