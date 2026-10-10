# piston-moves-observer: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -4,0,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set -4,0,0 minecraft:redstone_block** |
| +0 | блок (-4,0,0) redstone_block                                   | блок (-4,0,0) redstone_block                                   |
| +0 | звук (-2.5,0.5,0.5) block.piston.extend 0.50 ~0.83             | звук (-2.5,0.5,0.5) block.piston.extend 0.50 ~0.76             |
| +0 | действие (-3,0,0) sticky_piston 0/5                            | действие (-3,0,0) sticky_piston 0/5                            |
| +1 | блок (-3,0,0) sticky_piston[extended=true,facing=east]         | блок (-3,0,0) sticky_piston[extended=true,facing=east]         |
| +3 | блок (-1,0,0) observer[facing=north,powered=false] [пачкой]    | блок (-1,0,0) observer[facing=north,powered=false] [пачкой]    |
| +3 | блок (-2,0,0) piston_head[facing=east,short=false,type=sticky] [пачкой] | блок (-2,0,0) piston_head[facing=east,short=false,type=sticky] [пачкой] |
| +4 | блок (-1,0,0) observer[facing=north,powered=true] [пачкой]     | блок (-1,0,0) observer[facing=north,powered=true] [пачкой]     |
| +4 | блок (-1,0,1) redstone_lamp[lit=true] [пачкой]                 | блок (-1,0,1) redstone_lamp[lit=true] [пачкой]                 |
| +4 | блок (-2,0,1) redstone_lamp[lit=true] [пачкой]                 | блок (-2,0,1) redstone_lamp[lit=true] [пачкой]                 |
| +6 | блок (-1,0,0) observer[facing=north,powered=false]             | блок (-1,0,0) observer[facing=north,powered=false]             |
| +10 | блок (-1,0,1) redstone_lamp[lit=false] [пачкой]                | блок (-1,0,1) redstone_lamp[lit=false] [пачкой]                |
| +10 | блок (-2,0,1) redstone_lamp[lit=false] [пачкой]                | блок (-2,0,1) redstone_lamp[lit=false] [пачкой]                |
| | **▶ set -4,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -4,0,0 minecraft:air** |
| +0 | блок (-4,0,0) air                                              | блок (-4,0,0) air                                              |
| +0 | звук (-2.5,0.5,0.5) block.piston.contract 0.50 ~0.64           | звук (-2.5,0.5,0.5) block.piston.contract 0.50 ~0.83           |
| +0 | действие (-3,0,0) sticky_piston 1/5                            | действие (-3,0,0) sticky_piston 1/5                            |
| +1 | блок (-1,0,0) air                                              | блок (-1,0,0) air                                              |
| +3 | блок (-2,0,0) observer[facing=north,powered=false] [пачкой]    | блок (-2,0,0) observer[facing=north,powered=false] [пачкой]    |
| +3 | блок (-3,0,0) sticky_piston[extended=false,facing=east] [пачкой] | блок (-3,0,0) sticky_piston[extended=false,facing=east] [пачкой] |
| +4 | блок (-2,0,0) observer[facing=north,powered=true] [пачкой]     | блок (-2,0,0) observer[facing=north,powered=true] [пачкой]     |
| +4 | блок (-1,0,1) redstone_lamp[lit=true] [пачкой]                 | блок (-1,0,1) redstone_lamp[lit=true] [пачкой]                 |
| +4 | блок (-2,0,1) redstone_lamp[lit=true] [пачкой]                 | блок (-2,0,1) redstone_lamp[lit=true] [пачкой]                 |
| +6 | блок (-2,0,0) observer[facing=north,powered=false]             | блок (-2,0,0) observer[facing=north,powered=false]             |
| +10 | блок (-1,0,1) redstone_lamp[lit=false] [пачкой]                | блок (-1,0,1) redstone_lamp[lit=false] [пачкой]                |
| +10 | блок (-2,0,1) redstone_lamp[lit=false] [пачкой]                | блок (-2,0,1) redstone_lamp[lit=false] [пачкой]                |

Совпало: 24. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
