# rail-activator: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -5,0,0 minecraft:lever[face=floor,facing=north,powered=true]** (первый отклик через 2 / 4 такт.) | **▶ set -5,0,0 minecraft:lever[face=floor,facing=north,powered=true]**  ← отклик у нас позже на 2 |
| +0 | блок (8,0,8) air                                               |   ← нет у нас                                                  |
| +1 | блок (-5,0,0) lever[face=floor,facing=north,powered=true]      | блок (-5,0,0) lever[face=floor,facing=north,powered=true]  ← у нас раньше на 1 |
| +1 | блок (-1,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (-1,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой]  ← у нас раньше на 1 |
| +1 | блок (3,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |   ← нет у нас                                                  |
| +1 | блок (-2,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (-2,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой]  ← у нас раньше на 1 |
| +1 | блок (1,-1,3) redstone_block [пачкой]                          |   ← нет у нас                                                  |
| +1 | блок (2,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |   ← нет у нас                                                  |
| +1 | блок (-3,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (-3,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой]  ← у нас раньше на 1 |
| +1 | блок (-4,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (-4,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой]  ← у нас раньше на 1 |
| +1 | блок (0,0,3) activator_rail[powered=false,shape=north_south,waterlogged=false] [пачкой] |   ← нет у нас                                                  |
| +1 | блок (1,0,0) powered_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |   ← нет у нас                                                  |
| +1 | блок (0,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (0,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой]  ← у нас раньше на 1 |
| | **▶ set 1,0,3 minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,3 minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]** |
| +0 | блок (1,0,3) redstone_wire[east=none,north=none,power=15,south=none,west=none] | блок (1,0,3) redstone_wire[east=none,north=none,power=15,south=none,west=none] |
| | **▶ set 1,0,3 minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,3 minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]** |
| +0 | блок (1,0,3) redstone_wire[east=side,north=side,power=0,south=side,west=side] | блок (1,0,3) redstone_wire[east=side,north=side,power=0,south=side,west=side] |
| | **▶ set 1,0,3 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,3 minecraft:air** |
| +0 | блок (1,0,3) air                                               | блок (1,0,3) air                                               |
| | **▶ set -5,0,0 minecraft:lever[face=floor,facing=north,powered=false]** (первый отклик через 0 / 0 такт.) | **▶ set -5,0,0 minecraft:lever[face=floor,facing=north,powered=false]** |
| +0 | блок (-5,0,0) lever[face=floor,facing=north,powered=false]     | блок (-5,0,0) lever[face=floor,facing=north,powered=false]     |
| +0 | блок (-1,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (-1,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (-2,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (-2,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (-3,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (-3,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (-4,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (-4,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (0,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (0,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |

Совпало: 9. Сдвинуто по тактам: 7. Нет у нас: 6. Лишнее у нас: 0.
