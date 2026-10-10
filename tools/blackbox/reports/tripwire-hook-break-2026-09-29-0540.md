# tripwire-hook-break: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 1,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:air** |
| +0 | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] |   ← нет у нас                                                  |
| +0 | блок (1,0,0) air [пачкой]                                      | блок (1,0,0) air [пачкой]                                      |
| +0 | блок (5,0,0) tripwire_hook[attached=false,facing=west,powered=false] [пачкой] | блок (5,0,0) tripwire_hook[attached=false,facing=west,powered=false] [пачкой] |
| +0 |                                                                | звук (1.5,0.5,0.5) block.tripwire.detach 0.40 ~1.31  ← лишнее  |
| +0 |                                                                | блок (4,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой]  ← лишнее |
| +0 |                                                                | блок (3,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой]  ← лишнее |
| +0 |                                                                | блок (2,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой]  ← лишнее |
| | **▶ set 1,0,0 minecraft:tripwire_hook[attached=false,facing=east,powered=false]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:tripwire_hook[attached=false,facing=east,powered=false]** |
| +0 | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (1,0,0) tripwire_hook[attached=false,facing=east,powered=false] [пачкой] |   ← нет у нас                                                  |
| +0 |                                                                | звук (1.5,0.5,0.5) block.tripwire.attach 0.40 ~0.70  ← лишнее  |
| +0 |                                                                | звук (5.5,0.5,0.5) block.tripwire.attach 0.40 ~0.70  ← лишнее  |
| +0 |                                                                | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=false] [пачкой]  ← лишнее |
| +0 |                                                                | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=false] [пачкой]  ← лишнее |
| +0 |                                                                | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой]  ← лишнее |
| +0 |                                                                | блок (4,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой]  ← лишнее |

Совпало: 3. Сдвинуто по тактам: 0. Нет у нас: 2. Лишнее у нас: 10.
