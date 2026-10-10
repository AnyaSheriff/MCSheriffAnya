# piston-qc-lever: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 0,1,1** (первый отклик через 0 / 0 такт.) | **▶ click 0,1,1** |
| +0 | звук (0.5,1.5,1.5) block.lever.click 0.30 ~0.60                | звук (0.5,1.5,1.5) block.lever.click 0.30 ~0.60                |
| +0 | блок (0,1,1) lever[face=wall,facing=north,powered=true]        | блок (0,1,1) lever[face=wall,facing=north,powered=true]        |
| +0 | блок (0,2,1) air                                               | блок (0,2,1) air                                               |
| +0 | блок (0,1,1) lever[face=wall,facing=north,powered=true]        | блок (0,1,1) lever[face=wall,facing=north,powered=true]        |
| | **▶ set 0,0,-1 minecraft:stone** (первый отклик через 0 / 0 такт.) | **▶ set 0,0,-1 minecraft:stone** |
| +0 | блок (0,0,-1) stone                                            | блок (0,0,-1) stone                                            |
| +0 | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.77              | звук (0.5,0.5,0.5) block.piston.extend 0.50 ~0.72              |
| +0 | действие (0,0,0) piston 0/5                                    | действие (0,0,0) piston 0/5                                    |
| +1 | блок (0,0,0) piston[extended=true,facing=east]                 | блок (0,0,0) piston[extended=true,facing=east]                 |
| +3 | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  | блок (1,0,0) piston_head[facing=east,short=false,type=normal]  |
| | **▶ click 0,1,1** (первый отклик через 0 / 0 такт.) | **▶ click 0,1,1** |
| +0 | звук (0.5,1.5,1.5) block.lever.click 0.30 ~0.50                | звук (0.5,1.5,1.5) block.lever.click 0.30 ~0.50                |
| +0 | блок (0,1,1) lever[face=wall,facing=north,powered=false]       | блок (0,1,1) lever[face=wall,facing=north,powered=false]       |
| +0 | блок (0,2,1) air                                               | блок (0,2,1) air                                               |
| +0 | блок (0,1,1) lever[face=wall,facing=north,powered=false]       | блок (0,1,1) lever[face=wall,facing=north,powered=false]       |
| | **▶ set 0,0,-1 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 0,0,-1 minecraft:air** |
| +0 | блок (0,0,-1) air                                              | блок (0,0,-1) air                                              |
| +0 | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.69            | звук (0.5,0.5,0.5) block.piston.contract 0.50 ~0.85            |
| +0 | действие (0,0,0) piston 1/5                                    | действие (0,0,0) piston 1/5                                    |
| +1 | блок (1,0,0) air                                               | блок (1,0,0) air                                               |
| +3 | блок (0,0,0) piston[extended=false,facing=east]                | блок (0,0,0) piston[extended=false,facing=east]                |

Совпало: 18. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
