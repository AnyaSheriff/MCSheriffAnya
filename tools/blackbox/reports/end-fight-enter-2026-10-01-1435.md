# end-fight-enter: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd execute in minecraft:the_end run tp $(bot) 0.5 80.0 0.5** (первый отклик через 0 / 0 такт.) | **▶ cmd execute in minecraft:the_end run tp $(bot) 0.5 80.0 0.5** |
| +0 | робот: Respawn dim=minecraft:the_end mode=creative prev=0 death=minecraft:overworld 3,0,3 portal=0 copy=3 |   ← нет у нас                                                  |
| +0 | робот: Entity Event status=28                                  |   ← нет у нас                                                  |
| +0 | робот: сверка места (-99.5,139,-99.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} |   ← нет у нас                                                  |
| +0 | робот: Abilities flags=13 fly=0.05 fov=0.1                     | робот: Abilities flags=13 fly=0.05 fov=0.1                     |
| +0 | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":-60}} |   ← нет у нас                                                  |
| +0 | робот: Game Event reason=level_chunks_load_start value=0       | робот: Game Event reason=level_chunks_load_start value=0       |
| +0 | робот: Held Slot 0                                             |   ← нет у нас                                                  |
| +0 |                                                                | робот: Respawn dim=minecraft:the_end mode=creative prev=255 death=none portal=0 copy=3  ← лишнее |
| +0 |                                                                | робот: сверка места (-99.5,139,-99.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false}  ← лишнее |
| +1 | робот: Set Health health=20 food=20 saturation=20              |   ← нет у нас                                                  |
| +1 | робот: Set Experience bar=0 level=0 total=5                    |   ← нет у нас                                                  |
| +5 | робот: Boss Bar action=0 health=1 title={"translate":"entity.minecraft.ender_dragon"} color=0 dividers=0 flags=6 | робот: Boss Bar action=0 health=1 title={"translate":"entity.minecraft.ender_dragon"} color=0 dividers=0 flags=6  ← случайное, в допуске: дракон кружит |

Случайное, в допуске: 1 (входит в «Совпало»): дракон кружит — 1.
Совпало: 3. Сдвинуто по тактам: 0. Нет у нас: 7. Лишнее у нас: 2.
