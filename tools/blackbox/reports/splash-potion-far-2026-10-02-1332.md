# splash-potion-far: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ use_item** (первый отклик через 0 / 0 такт.) | **▶ use_item** |
| +0 | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.35         | звук (3.5,0,3.5) entity.splash_potion.throw 0.50 ~0.38         |
| +0 | сущность (3.5,1.52,3.5) splash_potion 1680 3.5,1.52,3.5 yaw=127 pitch=13 head=0 | сущность (3.5,1.52,3.5) splash_potion 1309 3.5,1.52,3.5 yaw=127 pitch=13 head=0  ← случайное, в допуске: номер бросившего и поворот зелья |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":37,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":37,"customEffects":[]}}],"removeComponents":[]} |
| +0 | инвентарь все:                                                 | инвентарь все:                                                 |
| +0 | splash_potion сдвиг 0,0,0                                      | splash_potion сдвиг 0,0,0                                      |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":37,"customEffects":[]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1292,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"potion_contents","data":{"potionId":37,"customEffects":[]}}],"removeComponents":[]} |
| +10 | splash_potion скорость 0.002,-0.325,-0.432                     | splash_potion скорость 0.001,-0.322,-0.437  ← случайное, в допуске: скорость зелья на сверке |
| +10 | splash_potion сдвиг 87,-4471,-18536 поворот 127/-13            | splash_potion сдвиг 47,-4336,-18735 поворот 127/-13  ← случайное, в допуске: сдвиг и поворот зелья на сверке |
| +13 | событие мира (3,-1,-3) 2002/-12038840                          | событие мира (3,-1,-3) 2002/-12038840                          |
| +13 | splash_potion убран                                            | splash_potion убран                                            |

Случайное, в допуске: 3 (входит в «Совпало»): номер бросившего и поворот зелья — 1; скорость зелья на сверке — 1; сдвиг и поворот зелья на сверке — 1.
Совпало: 10. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
