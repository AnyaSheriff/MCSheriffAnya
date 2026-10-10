# tripwire-hook-break: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 1,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:air** |
| +0 | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] |
| +0 | блок (1,0,0) air [пачкой]                                      | блок (1,0,0) air [пачкой]                                      |
| +0 | блок (5,0,0) tripwire_hook[attached=false,facing=west,powered=false] [пачкой] | блок (5,0,0) tripwire_hook[attached=false,facing=west,powered=false] [пачкой] |
| | **▶ set 1,0,0 minecraft:tripwire_hook[attached=false,facing=east,powered=false]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:tripwire_hook[attached=false,facing=east,powered=false]** |
| +0 | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (1,0,0) tripwire_hook[attached=false,facing=east,powered=false] [пачкой] | блок (1,0,0) tripwire_hook[attached=false,facing=east,powered=false] [пачкой] |

Совпало: 5. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
