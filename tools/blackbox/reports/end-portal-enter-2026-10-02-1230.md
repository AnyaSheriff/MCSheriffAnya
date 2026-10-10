# end-portal-enter: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd tp $(bot) $(8.5,0.3,4.5)** (первый отклик через 0 / 1 такт.) | **▶ cmd tp $(bot) $(8.5,0.3,4.5)** |
| +0 | робот: сверка места (8.5,0.3,4.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} | робот: сверка места (8.5,0.3,4.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} |
| +1 | робот: Respawn dim=minecraft:the_end mode=creative prev=0 death=minecraft:overworld 0,0,0 portal=10 copy=3 | робот: Respawn dim=minecraft:the_end mode=creative prev=0 death=minecraft:overworld 0,0,0 portal=10 copy=3 |
| +1 | робот: Entity Event status=28                                  | робот: Entity Event status=28                                  |
| +1 | робот: сверка места (0.5,108,-99.5) {"_value":496,"x":false,"y":false,"z":false,"yaw":false,"pitch":true,"dx":true,"dy":true,"dz":true,"yawDelta":true} | робот: сверка места (0.5,108,-99.5) {"_value":496,"x":false,"y":false,"z":false,"yaw":false,"pitch":true,"dx":true,"dy":true,"dz":true,"yawDelta":true} |
| +1 | робот: Abilities flags=13 fly=0.05 fov=0.1                     | робот: Abilities flags=13 fly=0.05 fov=0.1                     |
| +1 | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":-60}} | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":63}}  ← случайное, в допуске: смена порталом ждёт движения |
| +1 | робот: Game Event reason=level_chunks_load_start value=0       | робот: Game Event reason=level_chunks_load_start value=0       |
| +1 | робот: Held Slot 0                                             | робот: Held Slot 0                                             |
| +1 | робот: Set Health health=20 food=20 saturation=9               | робот: Set Health health=20 food=20 saturation=20  ← случайное, в допуске: смена порталом ждёт движения |
| +1 | робот: Set Experience bar=0 level=0 total=0                    | робот: Set Experience bar=0 level=0 total=0                    |
| +2 | робот: set_ticking_state {"tick_rate":20,"is_frozen":false}    | робот: set_ticking_state {"tick_rate":20,"is_frozen":false}    |
| +2 | робот: step_tick {"tick_steps":0}                              | робот: step_tick {"tick_steps":0}                              |
| +9 | робот: Boss Bar action=0 health=1 title={"translate":"entity.minecraft.ender_dragon"} color=0 dividers=0 flags=6 | робот: Boss Bar action=0 health=1 title={"translate":"entity.minecraft.ender_dragon"} color=0 dividers=0 flags=6 |

Случайное, в допуске: 2 (входит в «Совпало»): смена порталом ждёт движения — 2.
Совпало: 13. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
