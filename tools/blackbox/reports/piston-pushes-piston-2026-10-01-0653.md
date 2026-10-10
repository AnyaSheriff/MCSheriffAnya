# piston-pushes-piston: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -5,0,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -5,0,0 minecraft:redstone_block** |
| +0 | блок (-5,0,0) redstone_block                                   | блок (-5,0,0) redstone_block                                   |
| +0 | звук (-3.5,0.5,0.5) block.piston.extend 0.50 ~0.80             | звук (-3.5,0.5,0.5) block.piston.extend 0.50 ~0.82             |
| +0 | действие (-4,0,0) piston 0/5                                   | действие (-4,0,0) piston 0/5                                   |
| +1 | блок (-4,0,0) piston[extended=true,facing=east]                | блок (-4,0,0) piston[extended=true,facing=east]                |
| +3 | блок (-2,0,0) piston[extended=false,facing=north] [пачкой]     | блок (-2,0,0) piston[extended=false,facing=north] [пачкой]     |
| +3 | блок (-3,0,0) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (-3,0,0) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | звук (-1.5,0.5,0.5) block.piston.extend 0.50 ~0.65             | звук (-1.5,0.5,0.5) block.piston.extend 0.50 ~0.74             |
| +3 | действие (-2,0,0) piston 0/2                                   | действие (-2,0,0) piston 0/2                                   |
| +4 | блок (-2,0,0) piston[extended=true,facing=north]               | блок (-2,0,0) piston[extended=true,facing=north]               |
| +6 | блок (-2,0,-1) piston_head[facing=north,short=false,type=normal] | блок (-2,0,-1) piston_head[facing=north,short=false,type=normal] |
| | **▶ set -5,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -5,0,0 minecraft:air** |
| +0 | блок (-5,0,0) air                                              | блок (-5,0,0) air                                              |
| +0 | звук (-3.5,0.5,0.5) block.piston.contract 0.50 ~0.61           | звук (-3.5,0.5,0.5) block.piston.contract 0.50 ~0.81           |
| +0 | действие (-4,0,0) piston 1/5                                   | действие (-4,0,0) piston 1/5                                   |
| +1 | блок (-3,0,0) air                                              | блок (-3,0,0) air                                              |
| +3 | блок (-4,0,0) piston[extended=false,facing=east]               | блок (-4,0,0) piston[extended=false,facing=east]               |
| | **▶ set -2,0,1 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,1 minecraft:air** |
| +0 | блок (-2,0,1) air                                              | блок (-2,0,1) air                                              |
| +0 | звук (-1.5,0.5,0.5) block.piston.contract 0.50 ~0.69           | звук (-1.5,0.5,0.5) block.piston.contract 0.50 ~0.79           |
| +0 | действие (-2,0,0) piston 1/2                                   | действие (-2,0,0) piston 1/2                                   |
| +1 | блок (-2,0,-1) air                                             | блок (-2,0,-1) air                                             |
| +3 | блок (-2,0,0) piston[extended=false,facing=north]              | блок (-2,0,0) piston[extended=false,facing=north]              |

Совпало: 20. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
