# sticky-spit-many: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -2,0,0 minecraft:stone** (первый отклик через 2 / 0 такт.) | **▶ set -2,0,0 minecraft:stone**  ← отклик у нас раньше на 2 |
| +0 | блок (-2,0,0) stone                                            | блок (-2,0,0) stone                                            |
| +0 | блок (-1,0,0) observer[facing=west,powered=true]               | блок (-1,0,0) observer[facing=west,powered=true]  ← у нас позже на 1 |
| +0 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.79              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.65  ← у нас позже на 1 |
| +0 | действие (0,0,0) sticky_piston 0/5                             | действие (0,0,0) sticky_piston 0/5  ← у нас позже на 1         |
| +0 | блок (0,0,0) sticky_piston[extended=true,facing=east]          | блок (0,0,0) sticky_piston[extended=true,facing=east]  ← у нас позже на 2 |
| +0 | блок (-1,0,0) observer[facing=west,powered=false]              | блок (-1,0,0) observer[facing=west,powered=false]  ← у нас позже на 3 |
| +0 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.62            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.72  ← у нас позже на 3 |
| +0 | действие (0,0,0) sticky_piston 1/5                             | действие (0,0,0) sticky_piston 1/5  ← у нас позже на 3         |
| +0 | блок (2,0,0) air [пачкой]                                      | блок (2,0,0) air [пачкой]  ← у нас позже на 4                  |
| +0 | блок (1,0,0) moving_piston[facing=east,type=normal] [пачкой]   | блок (1,0,0) moving_piston[facing=east,type=normal] [пачкой]  ← у нас позже на 4 |
| +0 | блок (1,0,0) stone [пачкой]                                    | блок (1,0,0) stone [пачкой]  ← у нас позже на 6                |
| +0 | блок (0,0,0) sticky_piston[extended=false,facing=east] [пачкой] | блок (0,0,0) sticky_piston[extended=false,facing=east] [пачкой]  ← у нас позже на 6 |
| | **▶ set -2,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,0 minecraft:air** |
| +0 | блок (-2,0,0) air                                              | блок (-2,0,0) air                                              |
| +1 | блок (-1,0,0) observer[facing=west,powered=true]               | блок (-1,0,0) observer[facing=west,powered=true]               |
| +1 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.77              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.72              |
| +1 | действие (0,0,0) sticky_piston 0/5                             | действие (0,0,0) sticky_piston 0/5                             |
| +2 | блок (0,0,0) sticky_piston[extended=true,facing=east]          | блок (0,0,0) sticky_piston[extended=true,facing=east]          |
| +3 | блок (-1,0,0) observer[facing=west,powered=false]              | блок (-1,0,0) observer[facing=west,powered=false]              |
| +3 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.67            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.75            |
| +3 | действие (0,0,0) sticky_piston 2/5                             | действие (0,0,0) sticky_piston 2/5                             |
| +4 | блок (2,0,0) stone [пачкой]                                    | блок (2,0,0) stone [пачкой]                                    |
| +4 | блок (1,0,0) air [пачкой]                                      | блок (1,0,0) air [пачкой]                                      |
| +6 | блок (0,0,0) sticky_piston[extended=false,facing=east]         | блок (0,0,0) sticky_piston[extended=false,facing=east]         |
| | **▶ set -2,0,0 minecraft:stone** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,0 minecraft:stone** |
| +0 | блок (-2,0,0) stone                                            | блок (-2,0,0) stone                                            |
| +1 | блок (-1,0,0) observer[facing=west,powered=true]               | блок (-1,0,0) observer[facing=west,powered=true]               |
| +1 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.64              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.67              |
| +1 | действие (0,0,0) sticky_piston 0/5                             | действие (0,0,0) sticky_piston 0/5                             |
| +2 | блок (0,0,0) sticky_piston[extended=true,facing=east]          | блок (0,0,0) sticky_piston[extended=true,facing=east]          |
| +3 | блок (-1,0,0) observer[facing=west,powered=false]              | блок (-1,0,0) observer[facing=west,powered=false]              |
| +3 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.73            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.64            |
| +3 | действие (0,0,0) sticky_piston 1/5                             | действие (0,0,0) sticky_piston 1/5                             |
| +4 | блок (2,0,0) air [пачкой]                                      | блок (2,0,0) air [пачкой]                                      |
| +4 | блок (1,0,0) moving_piston[facing=east,type=normal] [пачкой]   | блок (1,0,0) moving_piston[facing=east,type=normal] [пачкой]   |
| +6 | блок (1,0,0) stone [пачкой]                                    | блок (1,0,0) stone [пачкой]                                    |
| +6 | блок (0,0,0) sticky_piston[extended=false,facing=east] [пачкой] | блок (0,0,0) sticky_piston[extended=false,facing=east] [пачкой] |
| | **▶ set -2,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,0 minecraft:air** |
| +0 | блок (-2,0,0) air                                              | блок (-2,0,0) air                                              |
| +1 | блок (-1,0,0) observer[facing=west,powered=true]               | блок (-1,0,0) observer[facing=west,powered=true]               |
| +1 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.77              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.62              |
| +1 | действие (0,0,0) sticky_piston 0/5                             | действие (0,0,0) sticky_piston 0/5                             |
| +2 | блок (0,0,0) sticky_piston[extended=true,facing=east]          | блок (0,0,0) sticky_piston[extended=true,facing=east]          |
| +3 | блок (-1,0,0) observer[facing=west,powered=false]              | блок (-1,0,0) observer[facing=west,powered=false]              |
| +3 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.67            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.84            |
| +3 | действие (0,0,0) sticky_piston 2/5                             | действие (0,0,0) sticky_piston 2/5                             |
| +4 | блок (2,0,0) stone [пачкой]                                    | блок (2,0,0) stone [пачкой]                                    |
| +4 | блок (1,0,0) air [пачкой]                                      | блок (1,0,0) air [пачкой]                                      |
| +6 | блок (0,0,0) sticky_piston[extended=false,facing=east]         | блок (0,0,0) sticky_piston[extended=false,facing=east]         |
| | **▶ set -2,0,0 minecraft:stone** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,0 minecraft:stone** |
| +0 | блок (-2,0,0) stone                                            | блок (-2,0,0) stone                                            |
| +1 | блок (-1,0,0) observer[facing=west,powered=true]               | блок (-1,0,0) observer[facing=west,powered=true]               |
| +1 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.80              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.83              |
| +1 | действие (0,0,0) sticky_piston 0/5                             | действие (0,0,0) sticky_piston 0/5                             |
| +2 | блок (0,0,0) sticky_piston[extended=true,facing=east]          | блок (0,0,0) sticky_piston[extended=true,facing=east]          |
| +3 | блок (-1,0,0) observer[facing=west,powered=false]              | блок (-1,0,0) observer[facing=west,powered=false]              |
| +3 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.60            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.65            |
| +3 | действие (0,0,0) sticky_piston 1/5                             | действие (0,0,0) sticky_piston 1/5                             |
| +4 | блок (2,0,0) air [пачкой]                                      | блок (2,0,0) air [пачкой]                                      |
| +4 | блок (1,0,0) moving_piston[facing=east,type=normal] [пачкой]   | блок (1,0,0) moving_piston[facing=east,type=normal] [пачкой]   |
| +6 | блок (1,0,0) stone [пачкой]                                    | блок (1,0,0) stone [пачкой]                                    |
| +6 | блок (0,0,0) sticky_piston[extended=false,facing=east] [пачкой] | блок (0,0,0) sticky_piston[extended=false,facing=east] [пачкой] |
| | **▶ set -2,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set -2,0,0 minecraft:air** |
| +0 | блок (-2,0,0) air                                              | блок (-2,0,0) air                                              |
| +1 | блок (-1,0,0) observer[facing=west,powered=true]               | блок (-1,0,0) observer[facing=west,powered=true]               |
| +1 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.71              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.62              |
| +1 | действие (0,0,0) sticky_piston 0/5                             | действие (0,0,0) sticky_piston 0/5                             |
| +2 | блок (0,0,0) sticky_piston[extended=true,facing=east]          | блок (0,0,0) sticky_piston[extended=true,facing=east]          |
| +3 | блок (-1,0,0) observer[facing=west,powered=false]              | блок (-1,0,0) observer[facing=west,powered=false]              |
| +3 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.65            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.71            |
| +3 | действие (0,0,0) sticky_piston 2/5                             | действие (0,0,0) sticky_piston 2/5                             |
| +4 | блок (2,0,0) stone [пачкой]                                    | блок (2,0,0) stone [пачкой]                                    |
| +4 | блок (1,0,0) air [пачкой]                                      | блок (1,0,0) air [пачкой]                                      |
| +6 | блок (0,0,0) sticky_piston[extended=false,facing=east]         | блок (0,0,0) sticky_piston[extended=false,facing=east]         |

Совпало: 58. Сдвинуто по тактам: 12. Нет у нас: 0. Лишнее у нас: 0.
