# flying-machine: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -2,1,0 minecraft:stone** (первый отклик через 2 / 0 такт.) | **▶ set -2,1,0 minecraft:stone**  ← отклик у нас раньше на 2 |
| +0 | блок (-2,1,0) stone                                            | блок (-2,1,0) stone                                            |
| +0 | блок (-1,1,0) observer[facing=west,powered=true]               | блок (-1,1,0) observer[facing=west,powered=true]  ← у нас позже на 1 |
| +0 | звук (0.5,1.5,0.5) block.piston.extend 0.50 ~0.68              | звук (0.5,1.5,0.5) block.piston.extend 0.50 ~0.68  ← у нас позже на 1 |
| +0 | действие (0,1,0) piston 0/5                                    | действие (0,1,0) piston 0/5  ← у нас позже на 1                |
| +0 | блок (1,2,0) air [пачкой]                                      | блок (1,2,0) air [пачкой]  ← у нас позже на 2                  |
| +0 | блок (0,1,0) piston[extended=true,facing=east] [пачкой]        | блок (0,1,0) piston[extended=true,facing=east] [пачкой]  ← у нас позже на 2 |
| +1 | блок (-1,1,0) observer[facing=west,powered=false]              | блок (-1,1,0) observer[facing=west,powered=false]  ← у нас позже на 2 |
| +1 | звук (0.5,1.5,0.5) block.piston.contract 0.50 ~0.72            | звук (0.5,1.5,0.5) block.piston.contract 0.50 ~0.65  ← у нас позже на 2 |
| +1 | действие (0,1,0) piston 2/5                                    | действие (0,1,0) piston 2/5  ← у нас позже на 2                |
| +2 | блок (1,1,0) air [пачкой]                                      | блок (1,1,0) air [пачкой]  ← у нас позже на 2                  |
| +2 | блок (3,2,0) observer[facing=east,powered=false] [пачкой]      | блок (3,2,0) observer[facing=east,powered=false] [пачкой]  ← у нас позже на 2 |
| +2 | блок (2,1,0) slime_block [пачкой]                              | блок (2,1,0) slime_block [пачкой]  ← у нас позже на 2          |
| +2 | блок (2,2,0) sticky_piston[extended=false,facing=west] [пачкой] | блок (2,2,0) sticky_piston[extended=false,facing=west] [пачкой]  ← у нас позже на 2 |
| +2 | блок (3,1,0) slime_block [пачкой]                              | блок (3,1,0) slime_block [пачкой]  ← у нас позже на 2          |
| +3 | блок (3,2,0) observer[facing=east,powered=true]                | блок (3,2,0) observer[facing=east,powered=true]  ← у нас позже на 2 |
| +3 | звук (2.5,2.5,0.5) block.piston.extend 0.50 ~0.60              | звук (2.5,2.5,0.5) block.piston.extend 0.50 ~0.65  ← у нас позже на 2 |
| +3 | действие (2,2,0) sticky_piston 0/4                             | действие (2,2,0) sticky_piston 0/4  ← у нас позже на 2         |
| +4 | блок (2,2,0) sticky_piston[extended=true,facing=west] [пачкой] | блок (2,2,0) sticky_piston[extended=true,facing=west] [пачкой]  ← у нас позже на 2 |
| +4 | блок (0,1,0) piston[extended=false,facing=east] [пачкой]       | блок (0,1,0) piston[extended=false,facing=east] [пачкой]  ← у нас позже на 2 |
| +5 | блок (3,2,0) observer[facing=east,powered=false]               | блок (3,2,0) observer[facing=east,powered=false]  ← у нас позже на 2 |
| +5 | звук (2.5,2.5,0.5) block.piston.contract 0.50 ~0.70            | звук (2.5,2.5,0.5) block.piston.contract 0.50 ~0.80  ← у нас позже на 2 |
| +5 | действие (2,2,0) sticky_piston 1/4                             | действие (2,2,0) sticky_piston 1/4  ← у нас позже на 2         |
| +6 | блок (-1,1,0) air [пачкой]                                     | блок (-1,1,0) air [пачкой]  ← у нас позже на 2                 |
| +6 | блок (1,2,0) moving_piston[facing=west,type=normal] [пачкой]   | блок (1,2,0) moving_piston[facing=west,type=normal] [пачкой]  ← у нас позже на 2 |
| +6 | блок (-1,2,0) air [пачкой]                                     | блок (-1,2,0) air [пачкой]  ← у нас позже на 2                 |
| +8 | блок (1,1,0) piston[extended=false,facing=east] [пачкой]       | блок (1,1,0) piston[extended=false,facing=east] [пачкой]  ← у нас позже на 2 |
| +8 | блок (2,2,0) sticky_piston[extended=false,facing=west] [пачкой] | блок (2,2,0) sticky_piston[extended=false,facing=west] [пачкой]  ← у нас позже на 2 |
| +8 | блок (1,2,0) slime_block [пачкой]                              | блок (1,2,0) slime_block [пачкой]  ← у нас позже на 2          |
| +8 | блок (0,2,0) slime_block [пачкой]                              | блок (0,2,0) slime_block [пачкой]  ← у нас позже на 2          |
| +8 | блок (0,1,0) observer[facing=west,powered=false] [пачкой]      | блок (0,1,0) observer[facing=west,powered=false] [пачкой]  ← у нас позже на 2 |
| +9 | блок (0,1,0) observer[facing=west,powered=true]                | блок (0,1,0) observer[facing=west,powered=true]  ← у нас позже на 2 |
| +9 | звук (1.5,1.5,0.5) block.piston.extend 0.50 ~0.78              | звук (1.5,1.5,0.5) block.piston.extend 0.50 ~0.73  ← у нас позже на 2 |
| +9 | действие (1,1,0) piston 0/5                                    | действие (1,1,0) piston 0/5  ← у нас позже на 2                |
| +10 | блок (1,1,0) piston[extended=true,facing=east] [пачкой]        | блок (1,1,0) piston[extended=true,facing=east] [пачкой]  ← у нас позже на 2 |
| +10 | блок (2,2,0) air [пачкой]                                      | блок (2,2,0) air [пачкой]  ← у нас позже на 2                  |
| +11 | блок (0,1,0) observer[facing=west,powered=false]               | блок (0,1,0) observer[facing=west,powered=false]  ← у нас позже на 2 |
| +11 | звук (1.5,1.5,0.5) block.piston.contract 0.50 ~0.63            | звук (1.5,1.5,0.5) block.piston.contract 0.50 ~0.69  ← у нас позже на 2 |
| +11 | действие (1,1,0) piston 2/5                                    | действие (1,1,0) piston 2/5  ← у нас позже на 2                |
| +12 | блок (4,2,0) observer[facing=east,powered=false] [пачкой]      | блок (4,2,0) observer[facing=east,powered=false] [пачкой]  ← у нас позже на 2 |
| +12 | блок (3,2,0) sticky_piston[extended=false,facing=west] [пачкой] | блок (3,2,0) sticky_piston[extended=false,facing=west] [пачкой]  ← у нас позже на 2 |
| +12 | блок (2,1,0) air [пачкой]                                      | блок (2,1,0) air [пачкой]  ← у нас позже на 2                  |
| +12 | блок (4,1,0) slime_block [пачкой]                              | блок (4,1,0) slime_block [пачкой]  ← у нас позже на 2          |
| +12 | блок (3,1,0) slime_block [пачкой]                              | блок (3,1,0) slime_block [пачкой]  ← у нас позже на 2          |
| +13 | блок (4,2,0) observer[facing=east,powered=true]                | блок (4,2,0) observer[facing=east,powered=true]  ← у нас позже на 2 |
| +13 | звук (3.5,2.5,0.5) block.piston.extend 0.50 ~0.78              | звук (3.5,2.5,0.5) block.piston.extend 0.50 ~0.65  ← у нас позже на 2 |
| +13 | действие (3,2,0) sticky_piston 0/4                             | действие (3,2,0) sticky_piston 0/4  ← у нас позже на 2         |
| +14 | блок (1,1,0) piston[extended=false,facing=east] [пачкой]       | блок (1,1,0) piston[extended=false,facing=east] [пачкой]  ← у нас позже на 2 |
| +14 | блок (3,2,0) sticky_piston[extended=true,facing=west] [пачкой] | блок (3,2,0) sticky_piston[extended=true,facing=west] [пачкой]  ← у нас позже на 2 |
| +15 | блок (4,2,0) observer[facing=east,powered=false]               | блок (4,2,0) observer[facing=east,powered=false]  ← у нас позже на 2 |
| +15 | звук (3.5,2.5,0.5) block.piston.contract 0.50 ~0.70            | звук (3.5,2.5,0.5) block.piston.contract 0.50 ~0.75  ← у нас позже на 2 |
| +15 | действие (3,2,0) sticky_piston 1/4                             | действие (3,2,0) sticky_piston 1/4  ← у нас позже на 2         |
| +16 | блок (2,2,0) moving_piston[facing=west,type=normal] [пачкой]   | блок (2,2,0) moving_piston[facing=west,type=normal] [пачкой]  ← у нас позже на 2 |
| +16 | блок (0,2,0) air [пачкой]                                      | блок (0,2,0) air [пачкой]  ← у нас позже на 2                  |
| +16 | блок (0,1,0) air [пачкой]                                      | блок (0,1,0) air [пачкой]  ← у нас позже на 2                  |
| +18 | блок (1,1,0) observer[facing=west,powered=false] [пачкой]      | блок (1,1,0) observer[facing=west,powered=false] [пачкой]  ← у нас позже на 2 |
| +18 | блок (3,2,0) sticky_piston[extended=false,facing=west] [пачкой] | блок (3,2,0) sticky_piston[extended=false,facing=west] [пачкой]  ← у нас позже на 2 |
| +18 | блок (2,1,0) piston[extended=false,facing=east] [пачкой]       | блок (2,1,0) piston[extended=false,facing=east] [пачкой]  ← у нас позже на 2 |
| +18 | блок (2,2,0) slime_block [пачкой]                              | блок (2,2,0) slime_block [пачкой]  ← у нас позже на 2          |
| +18 | блок (1,2,0) slime_block [пачкой]                              | блок (1,2,0) slime_block [пачкой]  ← у нас позже на 2          |
| +19 | блок (1,1,0) observer[facing=west,powered=true]                | блок (1,1,0) observer[facing=west,powered=true]  ← у нас позже на 2 |
| +19 | звук (2.5,1.5,0.5) block.piston.extend 0.50 ~0.62              | звук (2.5,1.5,0.5) block.piston.extend 0.50 ~0.65  ← у нас позже на 2 |
| +19 | действие (2,1,0) piston 0/5                                    | действие (2,1,0) piston 0/5  ← у нас позже на 2                |
| +20 | блок (3,2,0) air [пачкой]                                      | блок (3,2,0) air [пачкой]  ← у нас позже на 2                  |
| +20 | блок (2,1,0) piston[extended=true,facing=east] [пачкой]        | блок (2,1,0) piston[extended=true,facing=east] [пачкой]  ← у нас позже на 2 |
| +21 | блок (1,1,0) observer[facing=west,powered=false]               | блок (1,1,0) observer[facing=west,powered=false]  ← у нас позже на 2 |
| +21 | звук (2.5,1.5,0.5) block.piston.contract 0.50 ~0.74            | звук (2.5,1.5,0.5) block.piston.contract 0.50 ~0.78  ← у нас позже на 2 |
| +21 | действие (2,1,0) piston 2/5                                    | действие (2,1,0) piston 2/5  ← у нас позже на 2                |
| +22 | блок (4,2,0) sticky_piston[extended=false,facing=west] [пачкой] | блок (4,2,0) sticky_piston[extended=false,facing=west] [пачкой]  ← у нас позже на 2 |
| +22 | блок (4,1,0) slime_block [пачкой]                              | блок (4,1,0) slime_block [пачкой]  ← у нас позже на 2          |
| +22 | блок (3,1,0) air [пачкой]                                      | блок (3,1,0) air [пачкой]  ← у нас позже на 2                  |
| +22 | блок (5,2,0) observer[facing=east,powered=false] [пачкой]      | блок (5,2,0) observer[facing=east,powered=false] [пачкой]  ← у нас позже на 2 |
| +22 | блок (5,1,0) slime_block [пачкой]                              | блок (5,1,0) slime_block [пачкой]  ← у нас позже на 2          |
| +23 | блок (5,2,0) observer[facing=east,powered=true]                | блок (5,2,0) observer[facing=east,powered=true]  ← у нас позже на 2 |
| +23 | звук (4.5,2.5,0.5) block.piston.extend 0.50 ~0.71              | звук (4.5,2.5,0.5) block.piston.extend 0.50 ~0.72  ← у нас позже на 2 |
| +23 | действие (4,2,0) sticky_piston 0/4                             | действие (4,2,0) sticky_piston 0/4  ← у нас позже на 2         |
| +24 | блок (4,2,0) sticky_piston[extended=true,facing=west] [пачкой] | блок (4,2,0) sticky_piston[extended=true,facing=west] [пачкой]  ← у нас позже на 2 |
| +24 | блок (2,1,0) piston[extended=false,facing=east] [пачкой]       | блок (2,1,0) piston[extended=false,facing=east] [пачкой]  ← у нас позже на 2 |
| +25 | блок (5,2,0) observer[facing=east,powered=false]               | блок (5,2,0) observer[facing=east,powered=false]  ← у нас позже на 2 |
| +25 | звук (4.5,2.5,0.5) block.piston.contract 0.50 ~0.71            | звук (4.5,2.5,0.5) block.piston.contract 0.50 ~0.83  ← у нас позже на 2 |
| +25 | действие (4,2,0) sticky_piston 1/4                             | действие (4,2,0) sticky_piston 1/4  ← у нас позже на 2         |
| +26 | блок (1,1,0) air [пачкой]                                      | блок (1,1,0) air [пачкой]  ← у нас позже на 2                  |
| +26 | блок (3,2,0) moving_piston[facing=west,type=normal] [пачкой]   | блок (3,2,0) moving_piston[facing=west,type=normal] [пачкой]  ← у нас позже на 2 |
| +26 | блок (1,2,0) air [пачкой]                                      | блок (1,2,0) air [пачкой]  ← у нас позже на 2                  |
| +28 | блок (4,2,0) sticky_piston[extended=false,facing=west] [пачкой] | блок (4,2,0) sticky_piston[extended=false,facing=west] [пачкой]  ← у нас позже на 2 |
| +28 | блок (3,2,0) slime_block [пачкой]                              | блок (3,2,0) slime_block [пачкой]  ← у нас позже на 2          |
| +28 | блок (2,1,0) observer[facing=west,powered=false] [пачкой]      | блок (2,1,0) observer[facing=west,powered=false] [пачкой]  ← у нас позже на 2 |
| +28 | блок (2,2,0) slime_block [пачкой]                              | блок (2,2,0) slime_block [пачкой]  ← у нас позже на 2          |
| +28 | блок (3,1,0) piston[extended=false,facing=east] [пачкой]       | блок (3,1,0) piston[extended=false,facing=east] [пачкой]  ← у нас позже на 2 |
| +29 | блок (2,1,0) observer[facing=west,powered=true]                | блок (2,1,0) observer[facing=west,powered=true]  ← у нас позже на 2 |
| +31 | блок (2,1,0) observer[facing=west,powered=false]               | блок (2,1,0) observer[facing=west,powered=false]  ← у нас позже на 2 |

Совпало: 1. Сдвинуто по тактам: 90. Нет у нас: 0. Лишнее у нас: 0.
