# torch-inverter: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 0,1,0** (первый отклик через 0 / 0 такт.) | **▶ click 0,1,0** |
| +0 | звук (0.5,1.5,0.5) block.lever.click 0.30 ~0.60                | звук (0.5,1.5,0.5) block.lever.click 0.30 ~0.60                |
| +0 | блок (0,1,0) lever[face=floor,facing=north,powered=true]       | блок (0,1,0) lever[face=floor,facing=north,powered=true]       |
| +0 | блок (0,2,0) air                                               | блок (0,2,0) air                                               |
| +0 | блок (0,1,0) lever[face=floor,facing=north,powered=true]       | блок (0,1,0) lever[face=floor,facing=north,powered=true]       |
| +1 | блок (1,0,0) redstone_wall_torch[facing=east,lit=false]        | блок (1,0,0) redstone_wall_torch[facing=east,lit=false]        |
| +5 |                                                                | блок (2,0,0) redstone_lamp[lit=false]  ← лишнее                |
| | **▶ click 0,1,0** (первый отклик через 0 / 0 такт.) | **▶ click 0,1,0** |
| +0 | звук (0.5,1.5,0.5) block.lever.click 0.30 ~0.50                | звук (0.5,1.5,0.5) block.lever.click 0.30 ~0.50                |
| +0 | блок (0,1,0) lever[face=floor,facing=north,powered=false]      | блок (0,1,0) lever[face=floor,facing=north,powered=false]      |
| +0 | блок (0,2,0) air                                               | блок (0,2,0) air                                               |
| +0 | блок (0,1,0) lever[face=floor,facing=north,powered=false]      | блок (0,1,0) lever[face=floor,facing=north,powered=false]      |
| +1 | блок (2,0,0) redstone_lamp[lit=true] [пачкой]                  | блок (2,0,0) redstone_lamp[lit=true] [пачкой]                  |
| +1 | блок (1,0,0) redstone_wall_torch[facing=east,lit=true] [пачкой] | блок (1,0,0) redstone_wall_torch[facing=east,lit=true] [пачкой] |

Совпало: 11. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 1.
