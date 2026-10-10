# rail-activator: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click -5,0,0** | **▶ click -5,0,0** |
| +0 |                                                                | блок (-5,0,0) lever[face=floor,facing=north,powered=false]  ← лишнее |
| | **▶ set 1,0,3 minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,3 minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]** |
| +0 | блок (1,0,3) redstone_wire[east=none,north=none,power=15,south=none,west=none] | блок (1,0,3) redstone_wire[east=none,north=none,power=15,south=none,west=none] |
| | **▶ set 1,0,3 minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,3 minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]** |
| +0 | блок (1,0,3) redstone_wire[east=side,north=side,power=0,south=side,west=side] |   ← нет у нас                                                  |
| +0 |                                                                | блок (1,0,3) redstone_wire[east=side,north=side,power=15,south=side,west=side] [пачкой]  ← лишнее |
| +0 |                                                                | блок (0,0,3) activator_rail[powered=true,shape=north_south,waterlogged=false] [пачкой]  ← лишнее |
| | **▶ set 1,0,3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,3 minecraft:air** |
| +0 | блок (1,0,3) air                                               | блок (1,0,3) air [пачкой]                                      |
| +0 |                                                                | блок (0,0,3) activator_rail[powered=false,shape=north_south,waterlogged=false] [пачкой]  ← лишнее |
| | **▶ click -5,0,0** | **▶ click -5,0,0** |
| +0 |                                                                | блок (-5,0,0) lever[face=floor,facing=north,powered=false]  ← лишнее |

Совпало: 2. Сдвинуто по тактам: 0. Нет у нас: 1. Лишнее у нас: 5.
