# piston-pushes-piston: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -5,0,0 minecraft:redstone_block** (первый отклик через 0 / 1 такт.) | **▶ set -5,0,0 minecraft:redstone_block** |
| +0 | блок (-5,0,0) redstone_block                                   | блок (-5,0,0) redstone_block                                   |
| +0 | звук (-3.5,0.5,0.5) block.piston.extend 0.50 ~0.73             | звук (-3.5,0.5,0.5) block.piston.extend 0.50 ~0.65             |
| +0 | действие (-4,0,0) piston 0/5                                   | действие (-4,0,0) piston 0/5                                   |
| +1 | блок (-4,0,0) piston[extended=true,facing=east]                | блок (-4,0,0) piston[extended=true,facing=east]  ← у нас раньше на 1 |
| +3 | блок (-2,0,0) piston[extended=false,facing=north] [пачкой]     | блок (-2,0,0) piston[extended=false,facing=north] [пачкой]  ← у нас раньше на 1 |
| +3 | блок (-3,0,0) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (-3,0,0) piston_head[facing=east,short=false,type=normal] [пачкой]  ← у нас раньше на 1 |
| +3 | звук (-1.5,0.5,0.5) block.piston.extend 0.50 ~0.62             | звук (-1.5,0.5,0.5) block.piston.extend 0.50 ~0.70  ← у нас раньше на 1 |
| +3 | действие (-2,0,0) piston 0/2                                   | действие (-2,0,0) piston 0/2  ← у нас раньше на 1              |
| +4 | блок (-2,0,0) piston[extended=true,facing=north]               | блок (-2,0,0) piston[extended=true,facing=north]  ← у нас раньше на 1 |
| +6 | блок (-2,0,-1) piston_head[facing=north,short=false,type=normal] | блок (-2,0,-1) piston_head[facing=north,short=false,type=normal]  ← у нас раньше на 1 |
| | **▶ set -5,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -5,0,0 minecraft:air** |
| +0 | блок (-5,0,0) air                                              | блок (-5,0,0) air                                              |
| +0 | звук (-3.5,0.5,0.5) block.piston.contract 0.50 ~0.60           | звук (-3.5,0.5,0.5) block.piston.contract 0.50 ~0.81           |
| +0 | действие (-4,0,0) piston 1/5                                   | действие (-4,0,0) piston 1/5                                   |
| +1 | блок (-3,0,0) air                                              | блок (-3,0,0) air                                              |
| +3 | блок (-4,0,0) piston[extended=false,facing=east]               | блок (-4,0,0) piston[extended=false,facing=east]               |
| | **▶ set -2,0,1 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,1 minecraft:air** |
| +0 | блок (-2,0,1) air                                              | блок (-2,0,1) air                                              |
| +0 | звук (-1.5,0.5,0.5) block.piston.contract 0.50 ~0.74           | звук (-1.5,0.5,0.5) block.piston.contract 0.50 ~0.84           |
| +0 | действие (-2,0,0) piston 1/2                                   | действие (-2,0,0) piston 1/2                                   |
| +1 | блок (-2,0,-1) air                                             | блок (-2,0,-1) air                                             |
| +3 | блок (-2,0,0) piston[extended=false,facing=north]              | блок (-2,0,0) piston[extended=false,facing=north]              |

Совпало: 13. Сдвинуто по тактам: 7. Нет у нас: 0. Лишнее у нас: 0.
