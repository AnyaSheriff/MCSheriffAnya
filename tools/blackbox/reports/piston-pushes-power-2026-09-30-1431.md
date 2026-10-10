# piston-pushes-power: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -5,0,-2 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -5,0,-2 minecraft:redstone_block** |
| +0 | блок (-5,0,-2) redstone_block                                  | блок (-5,0,-2) redstone_block                                  |
| +0 | звук (-3.5,0.5,-1.5) block.piston.extend 0.50 ~0.72            | звук (-3.5,0.5,-1.5) block.piston.extend 0.50 ~0.84            |
| +0 | действие (-4,0,-2) piston 0/5                                  | действие (-4,0,-2) piston 0/5                                  |
| +1 | блок (-4,0,-2) piston[extended=true,facing=east]               | блок (-4,0,-2) piston[extended=true,facing=east]               |
| +3 | блок (-2,0,-3) redstone_lamp[lit=true] [пачкой]                | блок (-2,0,-3) redstone_lamp[lit=true] [пачкой]                |
| +3 | блок (-2,0,-2) redstone_block [пачкой]                         | блок (-2,0,-2) redstone_block [пачкой]                         |
| +3 | блок (-3,0,-2) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (-3,0,-2) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | звук (-1.5,0.5,-0.5) block.piston.extend 0.50 ~0.67            | звук (-1.5,0.5,-0.5) block.piston.extend 0.50 ~0.69            |
| +3 | действие (-2,0,-1) piston 0/3                                  | действие (-2,0,-1) piston 0/3                                  |
| +4 | блок (-2,0,-1) piston[extended=true,facing=south]              | блок (-2,0,-1) piston[extended=true,facing=south]              |
| +6 | блок (-2,0,0) piston_head[facing=south,short=false,type=normal] | блок (-2,0,0) piston_head[facing=south,short=false,type=normal] |
| | **▶ set -5,0,-2 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -5,0,-2 minecraft:air** |
| +0 | блок (-5,0,-2) air                                             | блок (-5,0,-2) air                                             |
| +0 | звук (-3.5,0.5,-1.5) block.piston.contract 0.50 ~0.71          | звук (-3.5,0.5,-1.5) block.piston.contract 0.50 ~0.82          |
| +0 | действие (-4,0,-2) piston 1/5                                  | действие (-4,0,-2) piston 1/5                                  |
| +1 | блок (-3,0,-2) air                                             | блок (-3,0,-2) air                                             |
| +3 | блок (-4,0,-2) piston[extended=false,facing=east]              | блок (-4,0,-2) piston[extended=false,facing=east]              |

Совпало: 16. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
