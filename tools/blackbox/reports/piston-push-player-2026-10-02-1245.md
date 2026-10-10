# piston-push-player: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 1,1,3 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 1,1,3 minecraft:redstone_block** |
| +0 | блок (1,1,3) redstone_block                                    | блок (1,1,3) redstone_block                                    |
| +0 | звук (2.5,1.5,3.5) block.piston.extend 0.50 ~0.76              | звук (2.5,1.5,3.5) block.piston.extend 0.50 ~0.77              |
| +0 | действие (2,1,3) piston 0/5                                    | действие (2,1,3) piston 0/5                                    |
| +1 | блок (2,1,3) piston[extended=true,facing=east]                 | блок (2,1,3) piston[extended=true,facing=east]                 |
| +3 | робот: сверка места (4.31,0,3.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false} | робот: сверка места (4.31,0,3.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false} |
| +3 | блок (3,1,3) piston_head[facing=east,short=false,type=normal]  | блок (3,1,3) piston_head[facing=east,short=false,type=normal]  ← порядок в такте другой |
| | **▶ set 1,1,3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 1,1,3 minecraft:air** |
| +0 | блок (1,1,3) air                                               | блок (1,1,3) air                                               |
| +0 | звук (2.5,1.5,3.5) block.piston.contract 0.50 ~0.70            | звук (2.5,1.5,3.5) block.piston.contract 0.50 ~0.83            |
| +0 | действие (2,1,3) piston 1/5                                    | действие (2,1,3) piston 1/5                                    |
| +1 | блок (3,1,3) air                                               | блок (3,1,3) air                                               |
| +3 | блок (2,1,3) piston[extended=false,facing=east]                | блок (2,1,3) piston[extended=false,facing=east]                |

Совпало: 11. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
