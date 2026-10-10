# piston-push-cart: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon minecart $(2.2,0,0.5)** (первый отклик через 0 / 0 такт.) | **▶ cmd summon minecart $(2.2,0,0.5)** |
| +0 | сущность (2.2,0,0.5) minecart 0                                | сущность (2.2,0,0.5) minecart 0                                |
| +0 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |
| | **▶ cmd summon minecart $(2.2,0,-2.5)** (первый отклик через 0 / 0 такт.) | **▶ cmd summon minecart $(2.2,0,-2.5)** |
| +0 | сущность (2.2,0,-2.5) minecart 0                               | сущность (2.2,0,-2.5) minecart 0                               |
| +0 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |
| +2 | вагонетка сдвиг 0,256,0                                        | вагонетка сдвиг 0,256,0                                        |
| +3 | вагонетка сдвиг 0,256,0                                        | вагонетка сдвиг 0,256,0                                        |
| | **▶ set -1,0,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:redstone_block** |
| +0 | блок (-1,0,0) redstone_block                                   | блок (-1,0,0) redstone_block                                   |
| +0 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.81              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.69              |
| +0 | действие (0,0,0) piston 0/5                                    | действие (0,0,0) piston 0/5                                    |
| +1 | блок (0,0,0) piston[extended=true,facing=east]                 | блок (0,0,0) piston[extended=true,facing=east]                 |
| +3 | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  |
| +3 | вагонетка сдвиг 1229,0,0                                       | вагонетка сдвиг 1229,0,0                                       |
| | **▶ set -2,0,-3 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,-3 minecraft:redstone_block** |
| +0 | блок (-2,0,-3) redstone_block                                  | блок (-2,0,-3) redstone_block                                  |
| +0 | звук (-0.5,0.5,-2.5) block.piston.extend 0.50 ~0.73            | звук (-0.5,0.5,-2.5) block.piston.extend 0.50 ~0.72            |
| +0 | действие (-1,0,-3) piston 0/5                                  | действие (-1,0,-3) piston 0/5                                  |
| +1 | блок (-1,0,-3) piston[extended=true,facing=east]               | блок (-1,0,-3) piston[extended=true,facing=east]               |
| +3 | блок (0,0,-3) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (0,0,-3) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | блок (1,0,-3) stone [пачкой]                                   | блок (1,0,-3) stone [пачкой]                                   |
| +4 | вагонетка сдвиг 1229,0,0                                       | вагонетка сдвиг 1229,0,0                                       |
| | **▶ set -1,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:air** |
| +0 | блок (-1,0,0) air                                              | блок (-1,0,0) air                                              |
| +0 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.66            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.65            |
| +0 | действие (0,0,0) piston 1/5                                    | действие (0,0,0) piston 1/5                                    |
| +1 | блок (1,0,0) air                                               | блок (1,0,0) air                                               |
| +3 | блок (0,0,0) piston[extended=false,facing=east]                | блок (0,0,0) piston[extended=false,facing=east]                |
| | **▶ set -2,0,-3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,-3 minecraft:air** |
| +0 | блок (-2,0,-3) air                                             | блок (-2,0,-3) air                                             |
| +0 | звук (-0.5,0.5,-2.5) block.piston.contract 0.50 ~0.69          | звук (-0.5,0.5,-2.5) block.piston.contract 0.50 ~0.84          |
| +0 | действие (-1,0,-3) piston 1/5                                  | действие (-1,0,-3) piston 1/5                                  |
| +1 | блок (0,0,-3) air                                              | блок (0,0,-3) air                                              |
| +3 | блок (-1,0,-3) piston[extended=false,facing=east]              | блок (-1,0,-3) piston[extended=false,facing=east]              |
| +15 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |
| +16 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |

Совпало: 31. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
