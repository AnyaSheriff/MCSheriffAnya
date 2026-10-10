# minecart-hopper: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon hopper_minecart $(0.5,0,0.5)** (первый отклик через 0 / 0 такт.) | **▶ cmd summon hopper_minecart $(0.5,0,0.5)** |
| +0 | сущность (0.5,0,0.5) hopper_minecart 0                         | сущность (0.5,0,0.5) hopper_minecart 0                         |
| +0 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |
| +3 | вагонетка сдвиг 0,256,0                                        | вагонетка сдвиг 0,256,0                                        |
| +11 | блок (1,1,0) comparator[facing=west,mode=compare,powered=false] | блок (1,1,0) comparator[facing=west,mode=compare,powered=false] |
| +15 | блок (2,1,0) redstone_lamp[lit=false]                          | блок (2,1,0) redstone_lamp[lit=false]                          |
| | **▶ set 0,-1,0 minecraft:redstone_block** (первый отклик через 0 / 0 такт.) | **▶ set 0,-1,0 minecraft:redstone_block** |
| +0 | блок (0,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] | блок (0,0,0) activator_rail[powered=true,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (0,-1,0) redstone_block [пачкой]                          | блок (0,-1,0) redstone_block [пачкой]                          |
| | **▶ cmd setblock $(0,1,0) minecraft:chest[facing=north,type=single,waterlogged=false]{Items:[{Slot:0b,id:"minecraft:stone",count:10}]}** (первый отклик через 0 / 1 такт.) | **▶ cmd setblock $(0,1,0) minecraft:chest[facing=north,type=single,waterlogged=false]{Items:[{Slot:0b,id:"minecraft:stone",count:10}]}** |
| +0 | блок (0,1,0) chest[facing=north,type=single,waterlogged=false] |   ← нет у нас                                                  |
| +1 | блок (1,1,0) comparator[facing=west,mode=compare,powered=true] [пачкой] | блок (1,1,0) comparator[facing=west,mode=compare,powered=true] [пачкой]  ← у нас раньше на 1 |
| +1 | блок (2,1,0) redstone_lamp[lit=true] [пачкой]                  | блок (2,1,0) redstone_lamp[lit=true] [пачкой]  ← у нас раньше на 1 |
| | **▶ set 0,-1,0 minecraft:stone** (первый отклик через 0 / 0 такт.) | **▶ set 0,-1,0 minecraft:stone** |
| +0 | блок (0,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] | блок (0,0,0) activator_rail[powered=false,shape=east_west,waterlogged=false] [пачкой] |
| +0 | блок (0,-1,0) stone [пачкой]                                   | блок (0,-1,0) stone [пачкой]                                   |
| +11 | блок (1,1,0) comparator[facing=west,mode=compare,powered=false] | блок (1,1,0) comparator[facing=west,mode=compare,powered=false] |
| +15 | блок (2,1,0) redstone_lamp[lit=false]                          | блок (2,1,0) redstone_lamp[lit=false]                          |
| +20 | вагонетка сдвиг 0,0,0                                          | вагонетка сдвиг 0,0,0                                          |

Совпало: 12. Сдвинуто по тактам: 2. Нет у нас: 1. Лишнее у нас: 0.
