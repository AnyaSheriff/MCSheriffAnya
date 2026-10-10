# piston-push-wall: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 1,1,3 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 1,1,3 minecraft:redstone_block** |
| +0 | блок (1,1,3) redstone_block                                    | блок (1,1,3) redstone_block                                    |
| +0 | звук (2.5,1.5,3.5) block.piston.extend 0.50 ~0.67              | звук (2.5,1.5,3.5) block.piston.extend 0.50 ~0.70              |
| +0 | действие (2,1,3) piston 0/5                                    | действие (2,1,3) piston 0/5                                    |
| +1 | блок (2,1,3) piston[extended=true,facing=east]                 | блок (2,1,3) piston[extended=true,facing=east]                 |
| +1 | робот: Metadata 6=3                                            |   ← случайное, в допуске: поза «плавание» при сдавливании (у нас не было) |
| +3 | блок (3,1,3) piston_head[facing=east,short=false,type=normal]  | блок (3,1,3) piston_head[facing=east,short=false,type=normal]  |
| | **▶ set 1,1,3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 1,1,3 minecraft:air** |
| +0 | блок (1,1,3) air                                               | блок (1,1,3) air                                               |
| +0 | звук (2.5,1.5,3.5) block.piston.contract 0.50 ~0.75            | звук (2.5,1.5,3.5) block.piston.contract 0.50 ~0.80            |
| +0 | действие (2,1,3) piston 1/5                                    | действие (2,1,3) piston 1/5                                    |
| +1 | блок (3,1,3) air                                               | блок (3,1,3) air                                               |
| +2 | робот: Metadata 6=0                                            |   ← случайное, в допуске: поза «плавание» при сдавливании (у нас не было) |
| +3 | блок (2,1,3) piston[extended=false,facing=east]                | блок (2,1,3) piston[extended=false,facing=east]                |

Случайное, в допуске: 2 (входит в «Совпало»): поза «плавание» при сдавливании — 2.
Совпало: 12. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
