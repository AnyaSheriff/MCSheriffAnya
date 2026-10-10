# sculk-shrieker: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 0,0,4** (первый отклик через 0 / 0 такт.) | **▶ click 0,0,4** |
| +0 | звук (0.5,0.5,4.5) block.lever.click 0.30 ~0.60                | звук (0.5,0.5,4.5) block.lever.click 0.30 ~0.60                |
| +0 | блок (0,0,4) lever[face=floor,facing=north,powered=true]       | блок (0,0,4) lever[face=floor,facing=north,powered=true]       |
| +0 | блок (0,1,4) air                                               | блок (0,1,4) air                                               |
| +0 | блок (0,0,4) lever[face=floor,facing=north,powered=true]       | блок (0,0,4) lever[face=floor,facing=north,powered=true]       |
| +0 | частица (0.5,0.5,4.5) vibration 1 шт. скорость 0 (0,0,0)       | частица (0.5,0.5,4.5) vibration 1 шт. скорость 0 (0,0,0)       |
| +3 | звук (0.5,0.5,0.5) block.sculk_sensor.clicking 1.00 ~0.87      | звук (0.5,0.5,0.5) block.sculk_sensor.clicking 1.00 ~0.84      |
| +4 | блок (0,0,0) sculk_sensor[power=8,sculk_sensor_phase=active,waterlogged=false] | блок (0,0,0) sculk_sensor[power=8,sculk_sensor_phase=active,waterlogged=false] |
| +33 | блок (0,0,0) sculk_sensor[power=0,sculk_sensor_phase=cooldown,waterlogged=false] | блок (0,0,0) sculk_sensor[power=0,sculk_sensor_phase=cooldown,waterlogged=false] |
| +43 | звук (0.5,0.5,0.5) block.sculk_sensor.clicking_stop 1.00 ~0.92 | звук (0.5,0.5,0.5) block.sculk_sensor.clicking_stop 1.00 ~0.99 |
| +43 | блок (0,0,0) sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false] | блок (0,0,0) sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false] |
| | **▶ cmd tp $(bot) $(2.5,0.6,0.5)** (первый отклик через 2 / 2 такт.) | **▶ cmd tp $(bot) $(2.5,0.6,0.5)** |
| +0 | событие мира (2,0,0) 3007/0                                    | событие мира (2,0,0) 3007/0                                    |
| +1 | блок (2,0,0) sculk_shrieker[can_summon=false,shrieking=true,waterlogged=false] | блок (2,0,0) sculk_shrieker[can_summon=false,shrieking=true,waterlogged=false] |
| +2 | блок (4,0,0) redstone_lamp[lit=true] [пачкой]                  | блок (4,0,0) redstone_lamp[lit=true] [пачкой]                  |
| +2 | блок (3,0,0) observer[facing=west,powered=true] [пачкой]       | блок (3,0,0) observer[facing=west,powered=true] [пачкой]       |
| +4 | блок (3,0,0) observer[facing=west,powered=false]               | блок (3,0,0) observer[facing=west,powered=false]               |
| +8 | блок (4,0,0) redstone_lamp[lit=false]                          | блок (4,0,0) redstone_lamp[lit=false]                          |
| +90 | блок (2,0,0) sculk_shrieker[can_summon=false,shrieking=false,waterlogged=false] | блок (2,0,0) sculk_shrieker[can_summon=false,shrieking=false,waterlogged=false] |
| +90 | событие мира (2,0,0) 3007/0                                    | событие мира (2,0,0) 3007/0                                    |
| +91 | блок (2,0,0) sculk_shrieker[can_summon=false,shrieking=true,waterlogged=false] | блок (2,0,0) sculk_shrieker[can_summon=false,shrieking=true,waterlogged=false] |
| +92 | блок (4,0,0) redstone_lamp[lit=true] [пачкой]                  | блок (4,0,0) redstone_lamp[lit=true] [пачкой]                  |
| +92 | блок (3,0,0) observer[facing=west,powered=true] [пачкой]       | блок (3,0,0) observer[facing=west,powered=true] [пачкой]       |
| +95 | блок (3,0,0) observer[facing=west,powered=false]               | блок (3,0,0) observer[facing=west,powered=false]  ← у нас раньше на 1 |
| +98 | блок (4,0,0) redstone_lamp[lit=false]                          | блок (4,0,0) redstone_lamp[lit=false]                          |

Совпало: 22. Сдвинуто по тактам: 1. Нет у нас: 0. Лишнее у нас: 0.
