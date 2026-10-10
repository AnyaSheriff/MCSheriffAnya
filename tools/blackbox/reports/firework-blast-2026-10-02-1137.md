# firework-blast: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 3,-1,5** (первый отклик через 0 / 0 такт.) | **▶ click 3,-1,5** |
| +0 | сущность (3.5,0.15,5.5) firework_rocket 331 3.5,0.15,5.5 yaw=0 pitch=0 head=0 | сущность (3.5,0.15,5.5) firework_rocket 40 3.5,0.15,5.5 yaw=0 pitch=0 head=0  ← случайное, в допуске: номер пустившего ракету |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1243,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"fireworks","data":{"flightDuration":0,"explosions":[{"shape":"large_ball","colors":[16711680],"fadeColors":[255],"hasTrail":false,"hasTwinkle":true},{"shape":"burst","colors":[65280],"fadeColors":[],"hasTrail":true,"hasTwinkle":false}]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1243,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"fireworks","data":{"flightDuration":0,"explosions":[{"shape":"large_ball","colors":[16711680],"fadeColors":[255],"hasTrail":false,"hasTwinkle":true},{"shape":"burst","colors":[65280],"fadeColors":[],"hasTrail":true,"hasTwinkle":false}]}}],"removeComponents":[]} |
| +0 | блок (3,-1,5) stone                                            | блок (3,-1,5) stone                                            |
| +0 | блок (3,0,5) air                                               | блок (3,0,5) air                                               |
| +0 | firework_rocket сдвиг 0,0,0                                    | firework_rocket сдвиг 0,0,0  ← порядок в такте другой          |
| +0 | свойства сущности 8={"itemCount":1,"itemId":1243,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"fireworks","data":{"flightDuration":0,"explosions":[{"shape":"large_ball","colors":[16711680],"fadeColors":[255],"hasTrail":false,"hasTwinkle":true},{"shape":"burst","colors":[65280],"fadeColors":[],"hasTrail":true,"hasTwinkle":false}]}}],"removeComponents":[]} | свойства сущности 8={"itemCount":1,"itemId":1243,"addedComponentCount":1,"removedComponentCount":0,"components":[{"type":"fireworks","data":{"flightDuration":0,"explosions":[{"shape":"large_ball","colors":[16711680],"fadeColors":[255],"hasTrail":false,"hasTwinkle":true},{"shape":"burst","colors":[65280],"fadeColors":[],"hasTrail":true,"hasTwinkle":false}]}}],"removeComponents":[]}  ← порядок в такте другой |
| +0 | инвентарь 36: пусто                                            | инвентарь 36: пусто                                            |
| +0 | звук (3.375,0.25,5.375) entity.firework_rocket.launch 3.00 ~1.00 | звук (3.5,0.25,5.375) entity.firework_rocket.launch 3.00 ~1.00  ← случайное, в допуске: звук запуска по x/z; порядок в такте другой |
| +10 | firework_rocket скорость -0.003,0.45,-0.001                    | firework_rocket скорость 0.003,0.45,-0.006  ← случайное, в допуске: скорость ракеты вбок |
| +10 | firework_rocket сдвиг -70,10650,-23 поворот -69/56             | firework_rocket сдвиг 81,10650,-152 поворот 96/56  ← случайное, в допуске: сдвиг ракеты вбок и угол |
| +10 | firework_rocket событие 17                                     | firework_rocket событие 17  ← порядок в такте другой           |
| +10 | робот: Damage Event type=15 cause=entity direct=entity pos=none | робот: Damage Event type=15 cause=entity direct=entity pos=none  ← порядок в такте другой |
| +10 | робот: Hurt Animation yaw=90.587                               | робот: Hurt Animation yaw=89.304  ← случайное, в допуске: угол ушиба; порядок в такте другой |
| +10 | firework_rocket убран                                          | firework_rocket убран                                          |
| +10 | робот: Set Health health=14.903 food=20 saturation=5           | робот: Set Health health=14.868 food=20 saturation=5  ← случайное, в допуске: урон от взрыва |
| +11 | робот: Metadata 9=14.903149604797363                           | робот: Metadata 9=14.868364334106445  ← случайное, в допуске: урон от взрыва |
| +11 | робот: скорость 0.002,0.275,-0.218                             | робот: скорость -0.003,0.275,-0.218  ← случайное, в допуске: отдача вбок |

Случайное, в допуске: 8 (входит в «Совпало»): номер пустившего ракету — 1; звук запуска по x/z — 1; скорость ракеты вбок — 1; сдвиг ракеты вбок и угол — 1; угол ушиба — 1; урон от взрыва — 2; отдача вбок — 1.
Совпало: 17. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
