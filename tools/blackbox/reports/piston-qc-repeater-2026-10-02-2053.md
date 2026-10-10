# piston-qc-repeater: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 0,2,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 0,2,0 minecraft:redstone_block** |
| +0 | блок (0,2,0) redstone_block                                    | блок (0,2,0) redstone_block                                    |
| | **▶ set 0,0,-3 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 0,0,-3 minecraft:redstone_block** |
| +0 | блок (0,0,-3) redstone_block                                   | блок (0,0,-3) redstone_block                                   |
| +1 | блок (0,0,-2) repeater[delay=1,facing=north,locked=false,powered=true] | блок (0,0,-2) repeater[delay=1,facing=north,locked=false,powered=true] |
| +1 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.81              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.84              |
| +1 | действие (0,0,0) piston 0/5                                    | действие (0,0,0) piston 0/5                                    |
| +2 | блок (0,0,0) piston[extended=true,facing=east]                 | блок (0,0,0) piston[extended=true,facing=east]                 |
| +4 | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  |
| | **▶ set 0,2,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 0,2,0 minecraft:air** |
| +0 | блок (0,2,0) air                                               | блок (0,2,0) air                                               |
| | **▶ set 0,0,-3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 0,0,-3 minecraft:air** |
| +0 | блок (0,0,-3) air                                              | блок (0,0,-3) air                                              |
| +1 | блок (0,0,-2) repeater[delay=1,facing=north,locked=false,powered=false] | блок (0,0,-2) repeater[delay=1,facing=north,locked=false,powered=false] |
| +1 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.65            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.72            |
| +1 | действие (0,0,0) piston 1/5                                    | действие (0,0,0) piston 1/5                                    |
| +2 | блок (1,0,0) air                                               | блок (1,0,0) air                                               |
| +4 | блок (0,0,0) piston[extended=false,facing=east]                | блок (0,0,0) piston[extended=false,facing=east]                |

Совпало: 14. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
