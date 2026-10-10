# tripwire-player: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd tp blackbox $(3,0,0)** (первый отклик через 0 / 1 такт.) | **▶ cmd tp blackbox $(3,0,0)** |
| +0 | звук (5.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60          | звук (5.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60          |
| +0 | звук (1.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60          | звук (1.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60  ← порядок в такте другой |
| +1 | блок (0,0,1) redstone_lamp[lit=true] [пачкой]                  | блок (0,0,1) redstone_lamp[lit=true] [пачкой]                  |
| +1 | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=true,south=false,west=true] [пачкой] | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=true,south=false,west=true] [пачкой] |
| +1 | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=true] [пачкой] | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=true] [пачкой] |
| +1 | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=true] [пачкой] | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=true] [пачкой] |
| +1 | блок (6,0,1) redstone_lamp[lit=true] [пачкой]                  | блок (6,0,1) redstone_lamp[lit=true] [пачкой]                  |
| +1 |                                                                | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=true,south=false,west=true] [пачкой]  ← лишнее |
| | **▶ cmd tp blackbox $(3,3,5)** (первый отклик через 5 / 6 такт.) | **▶ cmd tp blackbox $(3,3,5)** |
| +0 | звук (5.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50         | звук (5.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50         |
| +0 | звук (1.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50         | звук (1.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50  ← порядок в такте другой |
| +0 | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=false] [пачкой] | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=false] [пачкой] |
| +0 | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=false] [пачкой] | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=false] [пачкой] |
| +0 |                                                                | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой]  ← лишнее |
| +4 | блок (0,0,1) redstone_lamp[lit=false] [пачкой]                 | блок (0,0,1) redstone_lamp[lit=false] [пачкой]                 |
| +4 | блок (6,0,1) redstone_lamp[lit=false] [пачкой]                 | блок (6,0,1) redstone_lamp[lit=false] [пачкой]                 |

Совпало: 14. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 2.
