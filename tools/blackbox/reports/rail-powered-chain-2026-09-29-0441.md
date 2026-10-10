# rail-powered-chain: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click -7,0,0** | **▶ click -7,0,0** |
| +0 |                                                                | блок (-7,0,0) lever[face=floor,facing=north,powered=false]  ← лишнее |
| | **▶ click -7,0,0** | **▶ click -7,0,0** |
| +0 |                                                                | блок (-7,0,0) lever[face=floor,facing=north,powered=false]  ← лишнее |
| | **▶ click -2,0,3** (первый отклик через 5 / 5 такт.) | **▶ click -2,0,3** |
| +0 | звук (-1.5,0.5,3.5) block.lever.click 0.30 ~0.60               | звук (-1.5,0.5,3.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-2,0,3) lever[face=floor,facing=north,powered=true]      | блок (-2,0,3) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-2,1,3) air                                              | блок (-2,1,3) air                                              |
| +0 | блок (0,1,3) powered_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (0,1,3) powered_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (1,1,3) powered_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (1,1,3) powered_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (-1,0,3) powered_rail[powered=true,shape=ascending_east,waterlogged=false] [пачкой] | блок (-1,0,3) powered_rail[powered=true,shape=ascending_east,waterlogged=false] [пачкой] |
| +0 | блок (-2,0,3) lever[face=floor,facing=north,powered=true] [пачкой] | блок (-2,0,3) lever[face=floor,facing=north,powered=true] [пачкой] |
| | **▶ click -2,0,3** (первый отклик через 0 / 0 такт.) | **▶ click -2,0,3** |
| +0 | звук (-1.5,0.5,3.5) block.lever.click 0.30 ~0.50               | звук (-1.5,0.5,3.5) block.lever.click 0.30 ~0.50               |
| +0 | блок (-2,0,3) lever[face=floor,facing=north,powered=false]     | блок (-2,0,3) lever[face=floor,facing=north,powered=false]     |
| +0 | блок (-2,1,3) air                                              | блок (-2,1,3) air                                              |
| +0 | блок (0,1,3) powered_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (0,1,3) powered_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (1,1,3) powered_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (1,1,3) powered_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (-1,0,3) powered_rail[powered=false,shape=ascending_east,waterlogged=false] [пачкой] | блок (-1,0,3) powered_rail[powered=false,shape=ascending_east,waterlogged=false] [пачкой] |
| +0 | блок (-2,0,3) lever[face=floor,facing=north,powered=false] [пачкой] | блок (-2,0,3) lever[face=floor,facing=north,powered=false] [пачкой] |

Совпало: 14. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 2.
