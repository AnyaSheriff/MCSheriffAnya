# piston-immovable: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -6,0,1 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -6,0,1 minecraft:redstone_block** |
| +0 | блок (-6,0,1) redstone_block                                   | блок (-6,0,1) redstone_block                                   |
| | **▶ set -4,0,1 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -4,0,1 minecraft:redstone_block** |
| +0 | блок (-4,0,1) redstone_block                                   | блок (-4,0,1) redstone_block                                   |
| | **▶ set -2,0,1 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,1 minecraft:redstone_block** |
| +0 | блок (-2,0,1) redstone_block                                   | блок (-2,0,1) redstone_block                                   |
| | **▶ set 0,0,1 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 0,0,1 minecraft:redstone_block** |
| +0 | блок (0,0,1) redstone_block                                    | блок (0,0,1) redstone_block                                    |
| | **▶ set 2,0,1 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 2,0,1 minecraft:redstone_block** |
| +0 | блок (2,0,1) redstone_block                                    | блок (2,0,1) redstone_block                                    |
| | **▶ set 4,0,1 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 4,0,1 minecraft:redstone_block** |
| +0 | блок (4,0,1) redstone_block                                    | блок (4,0,1) redstone_block                                    |
| +0 | звук (4.5,0.5,0.5) block.piston.extend 0.50 ~0.80              | звук (4.5,0.5,0.5) block.piston.extend 0.50 ~0.82              |
| +0 | действие (4,0,0) piston 0/2                                    | действие (4,0,0) piston 0/2                                    |
| +1 | блок (4,0,0) piston[extended=true,facing=north]                | блок (4,0,0) piston[extended=true,facing=north]                |
| +3 | блок (4,0,-2) white_glazed_terracotta[facing=north] [пачкой]   | блок (4,0,-2) white_glazed_terracotta[facing=north] [пачкой]   |
| +3 | блок (4,0,-1) piston_head[facing=north,short=false,type=normal] [пачкой] | блок (4,0,-1) piston_head[facing=north,short=false,type=normal] [пачкой] |
| | **▶ set 4,0,1 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 4,0,1 minecraft:air** |
| +0 | блок (4,0,1) air                                               | блок (4,0,1) air                                               |
| +0 | звук (4.5,0.5,0.5) block.piston.contract 0.50 ~0.73            | звук (4.5,0.5,0.5) block.piston.contract 0.50 ~0.82            |
| +0 | действие (4,0,0) piston 1/2                                    | действие (4,0,0) piston 1/2                                    |
| +1 | блок (4,0,-1) air                                              | блок (4,0,-1) air                                              |
| +3 | блок (4,0,0) piston[extended=false,facing=north]               | блок (4,0,0) piston[extended=false,facing=north]               |

Совпало: 16. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
