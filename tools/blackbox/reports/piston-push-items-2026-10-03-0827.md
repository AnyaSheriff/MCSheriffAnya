# piston-push-items: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -1,0,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:redstone_block** |
| +0 | блок (-1,0,0) redstone_block                                   | блок (-1,0,0) redstone_block                                   |
| +0 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.71              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.74              |
| +0 | действие (0,0,0) piston 0/5                                    | действие (0,0,0) piston 0/5                                    |
| +1 | блок (0,0,0) piston[extended=true,facing=east]                 | блок (0,0,0) piston[extended=true,facing=east]                 |
| +3 | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  |
| | **▶ set -1,0,-2 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,-2 minecraft:redstone_block** |
| +0 | блок (-1,0,-2) redstone_block                                  | блок (-1,0,-2) redstone_block                                  |
| +0 | звук (0.5,0.5,-1.5) block.piston.extend 0.50 ~0.69             | звук (0.5,0.5,-1.5) block.piston.extend 0.50 ~0.61             |
| +0 | действие (0,0,-2) piston 0/5                                   | действие (0,0,-2) piston 0/5                                   |
| +1 | блок (0,0,-2) piston[extended=true,facing=east]                | блок (0,0,-2) piston[extended=true,facing=east]                |
| +3 | блок (1,0,-2) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (1,0,-2) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | блок (2,0,-2) stone [пачкой]                                   | блок (2,0,-2) stone [пачкой]                                   |
| +9 | предмет сдвиг 2601,0,0                                         | предмет сдвиг 2601,0,0  ← у нас раньше на 1                    |
| +11 | предмет сдвиг 2601,0,0                                         | предмет сдвиг 2601,0,0  ← у нас раньше на 1                    |
| | **▶ set -1,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:air** |
| +0 | блок (-1,0,0) air                                              | блок (-1,0,0) air                                              |
| +0 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.66            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.61            |
| +0 | действие (0,0,0) piston 1/5                                    | действие (0,0,0) piston 1/5                                    |
| +1 | блок (1,0,0) air                                               | блок (1,0,0) air                                               |
| +3 | блок (0,0,0) piston[extended=false,facing=east]                | блок (0,0,0) piston[extended=false,facing=east]                |
| | **▶ set -1,0,-2 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,-2 minecraft:air** |
| +0 | блок (-1,0,-2) air                                             | блок (-1,0,-2) air                                             |
| +0 | звук (0.5,0.5,-1.5) block.piston.contract 0.50 ~0.71           | звук (0.5,0.5,-1.5) block.piston.contract 0.50 ~0.83           |
| +0 | действие (0,0,-2) piston 1/5                                   | действие (0,0,-2) piston 1/5                                   |
| +1 | блок (1,0,-2) air                                              | блок (1,0,-2) air                                              |
| +3 | блок (0,0,-2) piston[extended=false,facing=east]               | блок (0,0,-2) piston[extended=false,facing=east]               |

Совпало: 21. Сдвинуто по тактам: 2. Нет у нас: 0. Лишнее у нас: 0.
