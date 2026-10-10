# repeater-lock-comparator: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 1,0,3** (первый отклик через 1 / 0 такт.) | **▶ click 1,0,3** |
| +0 | звук (1.5,0.5,3.5) block.lever.click 0.30 ~0.60                | звук (1.5,0.5,3.5) block.lever.click 0.30 ~0.60                |
| +0 | блок (1,0,3) lever[face=floor,facing=north,powered=true]       | блок (1,0,3) lever[face=floor,facing=north,powered=true]       |
| +0 | блок (1,1,3) air                                               | блок (1,1,3) air                                               |
| +0 | блок (1,0,3) lever[face=floor,facing=north,powered=true]       | блок (1,0,3) lever[face=floor,facing=north,powered=true]       |
| +1 | блок (1,0,2) comparator[facing=south,mode=compare,powered=true] [пачкой] | блок (1,0,2) comparator[facing=south,mode=compare,powered=true] [пачкой] |
| +1 | блок (1,0,1) repeater[delay=1,facing=west,locked=true,powered=false] [пачкой] | блок (1,0,1) repeater[delay=1,facing=west,locked=true,powered=false] [пачкой] |
| | **▶ click 0,0,1** (первый отклик через 5 / 5 такт.) | **▶ click 0,0,1** |
| +0 | звук (0.5,0.5,1.5) block.lever.click 0.30 ~0.60                | звук (0.5,0.5,1.5) block.lever.click 0.30 ~0.60                |
| +0 | блок (0,0,1) lever[face=floor,facing=north,powered=true]       | блок (0,0,1) lever[face=floor,facing=north,powered=true]       |
| +0 | блок (0,1,1) air                                               | блок (0,1,1) air                                               |
| +0 | блок (0,0,1) lever[face=floor,facing=north,powered=true]       | блок (0,0,1) lever[face=floor,facing=north,powered=true]       |
| | **▶ click 1,0,3** (первый отклик через 5 / 5 такт.) | **▶ click 1,0,3** |
| +0 | звук (1.5,0.5,3.5) block.lever.click 0.30 ~0.50                | звук (1.5,0.5,3.5) block.lever.click 0.30 ~0.50                |
| +0 | блок (1,0,3) lever[face=floor,facing=north,powered=false]      | блок (1,0,3) lever[face=floor,facing=north,powered=false]      |
| +0 | блок (1,1,3) air                                               | блок (1,1,3) air                                               |
| +0 | блок (1,0,3) lever[face=floor,facing=north,powered=false]      | блок (1,0,3) lever[face=floor,facing=north,powered=false]      |
| +1 | блок (1,0,2) comparator[facing=south,mode=compare,powered=false] [пачкой] | блок (1,0,2) comparator[facing=south,mode=compare,powered=false] [пачкой] |
| +1 | блок (1,0,1) repeater[delay=1,facing=west,locked=false,powered=false] [пачкой] | блок (1,0,1) repeater[delay=1,facing=west,locked=false,powered=false] [пачкой] |
| +3 | блок (2,0,1) redstone_lamp[lit=true] [пачкой]                  | блок (2,0,1) redstone_lamp[lit=true] [пачкой]                  |
| +3 | блок (1,0,1) repeater[delay=1,facing=west,locked=false,powered=true] [пачкой] | блок (1,0,1) repeater[delay=1,facing=west,locked=false,powered=true] [пачкой] |
| | **▶ click 0,0,1** (первый отклик через 5 / 5 такт.) | **▶ click 0,0,1** |
| +0 | звук (0.5,0.5,1.5) block.lever.click 0.30 ~0.50                | звук (0.5,0.5,1.5) block.lever.click 0.30 ~0.50                |
| +0 | блок (0,0,1) lever[face=floor,facing=north,powered=false]      | блок (0,0,1) lever[face=floor,facing=north,powered=false]      |
| +0 | блок (0,1,1) air                                               | блок (0,1,1) air                                               |
| +0 | блок (0,0,1) lever[face=floor,facing=north,powered=false]      | блок (0,0,1) lever[face=floor,facing=north,powered=false]      |
| +1 | блок (1,0,1) repeater[delay=1,facing=west,locked=false,powered=false] | блок (1,0,1) repeater[delay=1,facing=west,locked=false,powered=false] |
| +5 | блок (2,0,1) redstone_lamp[lit=false]                          | блок (2,0,1) redstone_lamp[lit=false]                          |

Совпало: 24. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
