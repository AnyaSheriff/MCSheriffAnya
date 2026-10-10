# piston-push-cart: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon minecart $(2.2,0,0.5)** | **▶ cmd summon minecart $(2.2,0,0.5)** |
| +0 | сущность (2.2,0,0.5) minecart 0                                |   ← нет у нас                                                  |
| +0 | вагонетка сдвиг 0,0,0                                          |   ← нет у нас                                                  |
| | **▶ cmd summon minecart $(2.2,0,-2.5)** (первый отклик через 0 / 0 такт.) | **▶ cmd summon minecart $(2.2,0,-2.5)** |
| +0 | сущность (2.2,0,-2.5) minecart 0                               | сущность (2.2,0,-2.5) minecart 0                               |
| +0 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0  ← порядок в такте другой                |
| +0 |                                                                | сущность (2.2,0,0.5) minecart 0  ← лишнее                      |
| +0 |                                                                | вагонетка сдвиг 0,0,0  ← лишнее                                |
| +0 |                                                                | вагонетка сдвиг 0,0,0  ← лишнее                                |
| +2 | вагонетка сдвиг 0,256,0                                        | вагонетка сдвиг 0,256,0  ← у нас позже на 1                    |
| +3 | вагонетка сдвиг 0,256,0                                        | вагонетка сдвиг 0,256,0                                        |
| | **▶ set -1,0,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:redstone_block** |
| +0 | блок (-1,0,0) redstone_block                                   | блок (-1,0,0) redstone_block                                   |
| +0 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.62              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.62              |
| +0 | действие (0,0,0) piston 0/5                                    | действие (0,0,0) piston 0/5                                    |
| +1 | блок (0,0,0) piston[extended=true,facing=east]                 | блок (0,0,0) piston[extended=true,facing=east]                 |
| +3 | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  |
| +3 | вагонетка сдвиг 1229,0,0                                       | вагонетка сдвиг 1229,0,0  ← у нас позже на 1                   |
| | **▶ set -2,0,-3 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,-3 minecraft:redstone_block** |
| +0 | блок (-2,0,-3) redstone_block                                  | блок (-2,0,-3) redstone_block                                  |
| +0 | звук (-0.5,0.5,-2.5) block.piston.extend 0.50 ~0.76            | звук (-0.5,0.5,-2.5) block.piston.extend 0.50 ~0.84            |
| +0 | действие (-1,0,-3) piston 0/5                                  | действие (-1,0,-3) piston 0/5                                  |
| +1 | блок (-1,0,-3) piston[extended=true,facing=east]               | блок (-1,0,-3) piston[extended=true,facing=east]               |
| +3 | блок (0,0,-3) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (0,0,-3) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | блок (1,0,-3) stone [пачкой]                                   | блок (1,0,-3) stone [пачкой]                                   |
| +4 | вагонетка сдвиг 1229,0,0                                       | вагонетка сдвиг 1229,0,0                                       |
| | **▶ set -1,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:air** |
| +0 | блок (-1,0,0) air                                              | блок (-1,0,0) air                                              |
| +0 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.61            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.84            |
| +0 | действие (0,0,0) piston 1/5                                    | действие (0,0,0) piston 1/5                                    |
| +1 | блок (1,0,0) air                                               | блок (1,0,0) air                                               |
| +3 | блок (0,0,0) piston[extended=false,facing=east]                | блок (0,0,0) piston[extended=false,facing=east]                |
| | **▶ set -2,0,-3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,-3 minecraft:air** |
| +0 | блок (-2,0,-3) air                                             | блок (-2,0,-3) air                                             |
| +0 | звук (-0.5,0.5,-2.5) block.piston.contract 0.50 ~0.68          | звук (-0.5,0.5,-2.5) block.piston.contract 0.50 ~0.73          |
| +0 | действие (-1,0,-3) piston 1/5                                  | действие (-1,0,-3) piston 1/5                                  |
| +1 | блок (0,0,-3) air                                              | блок (0,0,-3) air                                              |
| +3 | блок (-1,0,-3) piston[extended=false,facing=east]              | блок (-1,0,-3) piston[extended=false,facing=east]              |
| +15 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0  ← у нас позже на 1                      |
| +16 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |

Совпало: 26. Сдвинуто по тактам: 3. Нет у нас: 2. Лишнее у нас: 3.
