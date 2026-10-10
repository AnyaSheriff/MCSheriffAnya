# nether-portal-enter: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd tp $(bot) $(6.5,1,2.5)** (первый отклик через 0 / 2 такт.) | **▶ cmd tp $(bot) $(6.5,1,2.5)**  ← отклик у нас позже на 2 |
| +0 | робот: сверка места (6.5,1,2.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} | робот: сверка места (6.5,1,2.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} |
| +2 | робот: Respawn dim=minecraft:the_nether mode=creative prev=0 death=minecraft:overworld 0,0,0 portal=10 copy=3 | робот: Respawn dim=minecraft:the_nether mode=creative prev=0 death=minecraft:overworld 0,0,0 portal=10 copy=3  ← у нас раньше на 1 |
| +2 | робот: Entity Event status=28                                  | робот: Entity Event status=28  ← у нас раньше на 1             |
| +2 | робот: сверка места (-76.5,91,-98.5) {"_value":504,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":true,"dy":true,"dz":true,"yawDelta":true} | робот: сверка места (-87.5,174,-87.5) {"_value":504,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":true,"dy":true,"dz":true,"yawDelta":true}  ← у нас раньше на 1 |
| +2 | робот: Abilities flags=13 fly=0.05 fov=0.1                     | робот: Abilities flags=13 fly=0.05 fov=0.1  ← у нас раньше на 1 |
| +2 | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":-60}} | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":63}}  ← у нас раньше на 1 |
| +2 | робот: Game Event reason=level_chunks_load_start value=0       | робот: Game Event reason=level_chunks_load_start value=0  ← у нас раньше на 1 |
| +2 | робот: Held Slot 0                                             | робот: Held Slot 0  ← у нас раньше на 1                        |
| +2 | робот: Set Health health=20 food=20 saturation=9               | робот: Set Health health=20 food=20 saturation=20  ← у нас раньше на 1 |
| +2 | робот: Set Experience bar=0 level=0 total=0                    | робот: Set Experience bar=0 level=0 total=0  ← у нас раньше на 1 |

Совпало: 1. Сдвинуто по тактам: 10. Нет у нас: 0. Лишнее у нас: 0.
