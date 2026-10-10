# tripwire-attach: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 2,0,0 minecraft:tripwire** (первый отклик через 0 / 0 такт.) | **▶ set 2,0,0 minecraft:tripwire** |
| +0 | блок (2,0,0) tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=true] | блок (2,0,0) tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=true] |
| | **▶ set 3,0,0 minecraft:tripwire** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,0 minecraft:tripwire** |
| +0 | блок (3,0,0) tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=true] [пачкой] | блок (3,0,0) tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (2,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (2,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| | **▶ set 4,0,0 minecraft:tripwire** (первый отклик через 0 / 0 такт.) | **▶ set 4,0,0 minecraft:tripwire** |
| +0 | звук (5.5,0.5,0.5) block.tripwire.attach 0.40 ~0.70            | звук (5.5,0.5,0.5) block.tripwire.attach 0.40 ~0.70            |
| +0 | звук (1.5,0.5,0.5) block.tripwire.attach 0.40 ~0.70            | звук (1.5,0.5,0.5) block.tripwire.attach 0.40 ~0.70  ← порядок в такте другой |
| +0 | блок (4,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (4,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (3,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] | блок (2,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=false] [пачкой] | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=false] [пачкой] |
| +0 | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=false] [пачкой] | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=false] [пачкой] |
| | **▶ set 3,0,0 minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ set 3,0,0 minecraft:air** |
| +0 | звук (5.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60          | звук (5.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60          |
| +0 | звук (1.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60          | звук (1.5,0.5,0.5) block.tripwire.click_on 0.40 ~0.60  ← порядок в такте другой |
| +0 | блок (0,0,1) redstone_lamp[lit=true] [пачкой]                  | блок (0,0,1) redstone_lamp[lit=true] [пачкой]                  |
| +0 | блок (4,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] | блок (4,0,0) tripwire[attached=true,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] |
| +0 | блок (3,0,0) air [пачкой]                                      | блок (3,0,0) air [пачкой]                                      |
| +0 | блок (2,0,0) tripwire[attached=true,disarmed=false,east=false,north=false,powered=false,south=false,west=true] [пачкой] | блок (2,0,0) tripwire[attached=true,disarmed=false,east=false,north=false,powered=false,south=false,west=true] [пачкой] |
| +0 | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=true] [пачкой] | блок (1,0,0) tripwire_hook[attached=true,facing=east,powered=true] [пачкой] |
| +0 | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=true] [пачкой] | блок (5,0,0) tripwire_hook[attached=true,facing=west,powered=true] [пачкой] |
| +9 | звук (5.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50         | звук (5.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50         |
| +9 | звук (1.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50         | звук (1.5,0.5,0.5) block.tripwire.click_off 0.40 ~0.50  ← порядок в такте другой |
| +9 | блок (4,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] | блок (4,0,0) tripwire[attached=false,disarmed=false,east=true,north=false,powered=false,south=false,west=false] [пачкой] |
| +9 | блок (2,0,0) tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=true] [пачкой] | блок (2,0,0) tripwire[attached=false,disarmed=false,east=false,north=false,powered=false,south=false,west=true] [пачкой] |
| +9 | блок (1,0,0) tripwire_hook[attached=false,facing=east,powered=false] [пачкой] | блок (1,0,0) tripwire_hook[attached=false,facing=east,powered=false] [пачкой] |
| +9 | блок (5,0,0) tripwire_hook[attached=false,facing=west,powered=false] [пачкой] | блок (5,0,0) tripwire_hook[attached=false,facing=west,powered=false] [пачкой] |
| +13 | блок (0,0,1) redstone_lamp[lit=false]                          | блок (0,0,1) redstone_lamp[lit=false]                          |

Совпало: 25. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
