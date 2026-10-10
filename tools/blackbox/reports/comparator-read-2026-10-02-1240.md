# comparator-read: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set -1,0,0 minecraft:cake[bites=3]** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:cake[bites=3]** |
| +0 | блок (-1,0,0) cake[bites=3]                                    | блок (-1,0,0) cake[bites=3]                                    |
| | **▶ set 1,0,0 minecraft:water_cauldron[level=3]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:water_cauldron[level=3]** |
| +0 | блок (-1,0,2) redstone_wire[east=none,north=side,power=8,south=side,west=none] [пачкой] | блок (-1,0,2) redstone_wire[east=none,north=side,power=8,south=side,west=none] [пачкой] |
| +0 | блок (1,0,0) water_cauldron[level=3] [пачкой]                  | блок (1,0,0) water_cauldron[level=3] [пачкой]                  |
| | **▶ set 3,0,0 minecraft:composter[level=4]** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,0 minecraft:composter[level=4]** |
| +0 | блок (1,0,2) redstone_wire[east=none,north=side,power=3,south=side,west=none] [пачкой] | блок (1,0,2) redstone_wire[east=none,north=side,power=3,south=side,west=none] [пачкой] |
| +0 | блок (3,0,0) composter[level=4] [пачкой]                       | блок (3,0,0) composter[level=4] [пачкой]                       |
| | **▶ set 5,0,0 minecraft:end_portal_frame[eye=true,facing=north]** (первый отклик через 1 / 0 такт.) | **▶ set 5,0,0 minecraft:end_portal_frame[eye=true,facing=north]** |
| +0 | блок (3,0,1) comparator[facing=north,mode=compare,powered=true] [пачкой] | блок (3,0,1) comparator[facing=north,mode=compare,powered=true] [пачкой] |
| +0 | блок (3,0,2) redstone_wire[east=none,north=side,power=4,south=side,west=none] [пачкой] | блок (3,0,2) redstone_wire[east=none,north=side,power=4,south=side,west=none] [пачкой] |
| +0 | блок (5,0,0) end_portal_frame[eye=true,facing=north] [пачкой]  | блок (5,0,0) end_portal_frame[eye=true,facing=north] [пачкой]  |
| +0 | блок (5,0,1) comparator[facing=north,mode=compare,powered=true] [пачкой] | блок (5,0,1) comparator[facing=north,mode=compare,powered=true] [пачкой]  ← у нас позже на 1 |
| +0 | блок (5,0,2) redstone_wire[east=none,north=side,power=15,south=side,west=none] [пачкой] | блок (5,0,2) redstone_wire[east=none,north=side,power=15,south=side,west=none] [пачкой]  ← у нас позже на 1 |
| | **▶ set -1,0,0 minecraft:cake[bites=6]** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,0 minecraft:cake[bites=6]** |
| +0 | блок (-1,0,0) cake[bites=6]                                    | блок (-1,0,0) cake[bites=6]                                    |
| | **▶ set 1,0,0 minecraft:cauldron** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:cauldron** |
| +0 | блок (-1,0,2) redstone_wire[east=none,north=side,power=2,south=side,west=none] [пачкой] | блок (-1,0,2) redstone_wire[east=none,north=side,power=2,south=side,west=none] [пачкой] |
| +0 | блок (1,0,0) cauldron [пачкой]                                 | блок (1,0,0) cauldron [пачкой]                                 |
| | **▶ set 3,0,0 minecraft:composter[level=8]** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,0 minecraft:composter[level=8]** |
| +0 | блок (1,0,2) redstone_wire[east=none,north=side,power=0,south=side,west=none] [пачкой] | блок (1,0,2) redstone_wire[east=none,north=side,power=0,south=side,west=none] [пачкой] |
| +0 | блок (3,0,0) composter[level=8] [пачкой]                       | блок (3,0,0) composter[level=8] [пачкой]                       |
| +0 | блок (1,0,1) comparator[facing=north,mode=compare,powered=false] [пачкой] | блок (1,0,1) comparator[facing=north,mode=compare,powered=false] [пачкой] |
| | **▶ set 5,0,0 minecraft:end_portal_frame[eye=false,facing=north]** (первый отклик через 0 / 0 такт.) | **▶ set 5,0,0 minecraft:end_portal_frame[eye=false,facing=north]** |
| +0 | блок (3,0,2) redstone_wire[east=none,north=side,power=8,south=side,west=none] [пачкой] | блок (3,0,2) redstone_wire[east=none,north=side,power=8,south=side,west=none] [пачкой] |
| +0 | блок (5,0,0) end_portal_frame[eye=false,facing=north] [пачкой] | блок (5,0,0) end_portal_frame[eye=false,facing=north] [пачкой] |
| +1 | блок (5,0,1) comparator[facing=north,mode=compare,powered=false] [пачкой] | блок (5,0,1) comparator[facing=north,mode=compare,powered=false] [пачкой] |
| +1 | блок (5,0,2) redstone_wire[east=none,north=side,power=0,south=side,west=none] [пачкой] | блок (5,0,2) redstone_wire[east=none,north=side,power=0,south=side,west=none] [пачкой] |
| | **▶ set -1,0,6 minecraft:respawn_anchor[charges=1]** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,6 minecraft:respawn_anchor[charges=1]** |
| +0 | блок (-1,0,6) respawn_anchor[charges=1]                        | блок (-1,0,6) respawn_anchor[charges=1]                        |
| | **▶ set 1,0,6 minecraft:beehive[facing=north,honey_level=3]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,6 minecraft:beehive[facing=north,honey_level=3]** |
| +0 | блок (1,0,6) beehive[facing=north,honey_level=3] [пачкой]      | блок (1,0,6) beehive[facing=north,honey_level=3] [пачкой]      |
| +0 | блок (-1,0,5) comparator[facing=south,mode=compare,powered=true] [пачкой] | блок (-1,0,5) comparator[facing=south,mode=compare,powered=true] [пачкой] |
| +0 | блок (-1,0,4) redstone_wire[east=none,north=side,power=3,south=side,west=none] [пачкой] | блок (-1,0,4) redstone_wire[east=none,north=side,power=3,south=side,west=none] [пачкой] |
| | **▶ set 3,0,6 minecraft:powder_snow_cauldron[level=2]** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,6 minecraft:powder_snow_cauldron[level=2]** |
| +0 | блок (1,0,4) redstone_wire[east=none,north=side,power=3,south=side,west=none] [пачкой] | блок (1,0,4) redstone_wire[east=none,north=side,power=3,south=side,west=none] [пачкой] |
| +0 | блок (3,0,6) powder_snow_cauldron[level=2] [пачкой]            | блок (3,0,6) powder_snow_cauldron[level=2] [пачкой]            |
| +0 | блок (1,0,5) comparator[facing=south,mode=compare,powered=true] [пачкой] | блок (1,0,5) comparator[facing=south,mode=compare,powered=true] [пачкой] |
| +1 | блок (3,0,4) redstone_wire[east=none,north=side,power=2,south=side,west=none] | блок (3,0,4) redstone_wire[east=none,north=side,power=2,south=side,west=none] |
| | **▶ set -1,0,6 minecraft:respawn_anchor[charges=4]** (первый отклик через 0 / 0 такт.) | **▶ set -1,0,6 minecraft:respawn_anchor[charges=4]** |
| +0 | блок (-1,0,6) respawn_anchor[charges=4]                        | блок (-1,0,6) respawn_anchor[charges=4]                        |
| | **▶ set 1,0,6 minecraft:beehive[facing=north,honey_level=5]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,6 minecraft:beehive[facing=north,honey_level=5]** |
| +0 | блок (1,0,6) beehive[facing=north,honey_level=5] [пачкой]      | блок (1,0,6) beehive[facing=north,honey_level=5] [пачкой]      |
| +0 | блок (-1,0,4) redstone_wire[east=none,north=side,power=15,south=side,west=none] [пачкой] | блок (-1,0,4) redstone_wire[east=none,north=side,power=15,south=side,west=none] [пачкой] |
| | **▶ set 3,0,6 minecraft:cauldron** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,6 minecraft:cauldron** |
| +0 | блок (1,0,4) redstone_wire[east=none,north=side,power=5,south=side,west=none] [пачкой] | блок (1,0,4) redstone_wire[east=none,north=side,power=5,south=side,west=none] [пачкой] |
| +0 | блок (3,0,6) cauldron [пачкой]                                 | блок (3,0,6) cauldron [пачкой]                                 |
| +1 | блок (3,0,4) redstone_wire[east=none,north=side,power=0,south=side,west=none] [пачкой] | блок (3,0,4) redstone_wire[east=none,north=side,power=0,south=side,west=none] [пачкой] |
| +1 | блок (3,0,5) comparator[facing=south,mode=compare,powered=false] [пачкой] | блок (3,0,5) comparator[facing=south,mode=compare,powered=false] [пачкой] |

Совпало: 33. Сдвинуто по тактам: 2. Нет у нас: 0. Лишнее у нас: 0.
