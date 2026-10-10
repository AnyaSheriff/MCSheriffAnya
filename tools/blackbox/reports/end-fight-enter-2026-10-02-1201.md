# end-fight-enter: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd execute in minecraft:the_end run tp $(bot) 0.5 80.0 0.5** (первый отклик через 0 / 0 такт.) | **▶ cmd execute in minecraft:the_end run tp $(bot) 0.5 80.0 0.5** |
| +0 | робот: Respawn dim=minecraft:the_end mode=creative prev=0 death=minecraft:overworld 0,0,0 portal=0 copy=3 | робот: Respawn dim=minecraft:the_end mode=creative prev=0 death=minecraft:overworld 0,0,0 portal=0 copy=3 |
| +0 | робот: Entity Event status=28                                  | робот: Entity Event status=28                                  |
| +0 | робот: сверка места (-99.5,139,-99.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} | робот: сверка места (-99.5,139,-99.5) {"_value":24,"x":false,"y":false,"z":false,"yaw":true,"pitch":true,"dx":false,"dy":false,"dz":false,"yawDelta":false} |
| +0 | робот: Abilities flags=13 fly=0.05 fov=0.1                     | робот: Abilities flags=13 fly=0.05 fov=0.1                     |
| +0 | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":-60}} | робот: Spawn Position {"dimensionName":"minecraft:overworld","location":{"x":0,"z":0,"y":63}}  ← случайное, в допуске: высота точки возрождения |
| +0 | робот: Game Event reason=level_chunks_load_start value=0       | робот: Game Event reason=level_chunks_load_start value=0       |
| +0 | робот: set_ticking_state {"tick_rate":20,"is_frozen":false}    |   ← нет у нас                                                  |
| +0 | робот: step_tick {"tick_steps":0}                              |   ← нет у нас                                                  |
| +0 | робот: Held Slot 0                                             | робот: Held Slot 0                                             |
| +0 | робот: Set Health health=20 food=20 saturation=6               | робот: Set Health health=20 food=20 saturation=20  ← случайное, в допуске: здоровье и опыт после смены |
| +0 | робот: Set Experience bar=0 level=0 total=0                    | робот: Set Experience bar=0 level=0 total=0  ← случайное, в допуске: здоровье и опыт после смены |
| +1 | робот: Boss Bar action=0 health=1 title={"translate":"entity.minecraft.ender_dragon"} color=0 dividers=0 flags=6 | робот: Boss Bar action=0 health=1 title={"translate":"entity.minecraft.ender_dragon"} color=0 dividers=0 flags=6  ← случайное, в допуске: дракон кружит |

Случайное, в допуске: 4 (входит в «Совпало»): высота точки возрождения — 1; здоровье и опыт после смены — 2; дракон кружит — 1.
Совпало: 10. Сдвинуто по тактам: 0. Нет у нас: 2. Лишнее у нас: 0.
