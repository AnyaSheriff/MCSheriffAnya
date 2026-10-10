# piston-push-shape: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -2,0,-3 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,-3 minecraft:redstone_block** |
| +0 | блок (-2,0,-3) redstone_block                                  | блок (-2,0,-3) redstone_block                                  |
| +0 | звук (-0.5,0.5,-2.5) block.piston.extend 0.50 ~0.65            | звук (-0.5,0.5,-2.5) block.piston.extend 0.50 ~0.82            |
| +0 | действие (-1,0,-3) piston 0/5                                  | действие (-1,0,-3) piston 0/5                                  |
| +1 | блок (-1,0,-3) piston[extended=true,facing=east]               | блок (-1,0,-3) piston[extended=true,facing=east]               |
| +3 | блок (0,0,-3) piston_head[facing=east,short=false,type=normal] [пачкой] | блок (0,0,-3) piston_head[facing=east,short=false,type=normal] [пачкой] |
| +3 | блок (1,0,-3) stone_slab[type=bottom,waterlogged=false] [пачкой] | блок (1,0,-3) stone_slab[type=bottom,waterlogged=false] [пачкой] |
| +10 | предмет сдвиг 0,0,0                                            | предмет сдвиг 0,0,0  ← случайное, в допуске: сдвиг предмета    |
| | **▶ set -2,0,-3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,-3 minecraft:air** |
| +0 | блок (-2,0,-3) air                                             | блок (-2,0,-3) air                                             |
| +0 | звук (-0.5,0.5,-2.5) block.piston.contract 0.50 ~0.75          | звук (-0.5,0.5,-2.5) block.piston.contract 0.50 ~0.77          |
| +0 | действие (-1,0,-3) piston 1/5                                  | действие (-1,0,-3) piston 1/5                                  |
| +1 | блок (0,0,-3) air                                              | блок (0,0,-3) air                                              |
| +3 | блок (-1,0,-3) piston[extended=false,facing=east]              | блок (-1,0,-3) piston[extended=false,facing=east]              |

Случайное, в допуске: 1 (входит в «Совпало»): сдвиг предмета — 1.
Совпало: 12. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
