# piston-push-items: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -1,0,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:redstone_block** |
| +0 | блок (-1,0,0) redstone_block                                   | блок (-1,0,0) redstone_block                                   |
| +0 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.63              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.81              |
| +0 | действие (0,0,0) piston 0/5                                    | действие (0,0,0) piston 0/5                                    |
| +1 | блок (0,0,0) piston[extended=true,facing=east]                 | блок (0,0,0) piston[extended=true,facing=east]                 |
| +3 | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  |
| | **▶ set -1,0,-2 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,-2 minecraft:redstone_block** |
| +0 | блок (-1,0,-2) redstone_block                                  | блок (-1,0,-2) redstone_block                                  |
| +0 | звук (0.5,0.5,-1.5) block.piston.extend 0.50 ~0.67             | звук (0.5,0.5,-1.5) block.piston.extend 0.50 ~0.77             |
| +0 | действие (0,0,-2) piston 0/5                                   | действие (0,0,-2) piston 0/5                                   |
| +1 | блок (0,0,-2) piston[extended=true,facing=east]                | блок (0,0,-2) piston[extended=true,facing=east]                |
| +3 | блок (1,0,-2) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (1,0,-2) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | блок (2,0,-2) stone [пачкой]                                   | блок (2,0,-2) stone [пачкой]                                   |
| +8 | предмет сдвиг 2601,0,0                                         | предмет сдвиг 2601,0,0                                         |
| +10 | предмет сдвиг 2601,0,0                                         | предмет сдвиг 2601,0,0                                         |
| | **▶ set -1,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:air** |
| +0 | блок (-1,0,0) air                                              | блок (-1,0,0) air                                              |
| +0 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.71            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.85            |
| +0 | действие (0,0,0) piston 1/5                                    | действие (0,0,0) piston 1/5                                    |
| +1 | блок (1,0,0) air                                               | блок (1,0,0) air                                               |
| +3 | блок (0,0,0) piston[extended=false,facing=east]                | блок (0,0,0) piston[extended=false,facing=east]                |
| | **▶ set -1,0,-2 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,-2 minecraft:air** |
| +0 | блок (-1,0,-2) air                                             | блок (-1,0,-2) air                                             |
| +0 | звук (0.5,0.5,-1.5) block.piston.contract 0.50 ~0.61           | звук (0.5,0.5,-1.5) block.piston.contract 0.50 ~0.71           |
| +0 | действие (0,0,-2) piston 1/5                                   | действие (0,0,-2) piston 1/5                                   |
| +1 | блок (1,0,-2) air                                              | блок (1,0,-2) air                                              |
| +3 | блок (0,0,-2) piston[extended=false,facing=east]               | блок (0,0,-2) piston[extended=false,facing=east]               |

Совпало: 23. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
