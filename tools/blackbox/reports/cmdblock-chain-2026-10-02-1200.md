# cmdblock-chain: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click -1,0,0** (первый отклик через 1 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | робот: System Chat {"with":[{"":"@"},{"with":[2148,-59,102],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +0 | робот: System Chat {"with":[{"":"@"},{"with":[2149,-59,102],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +0 | робот: System Chat {"with":[{"":"@"},{"with":[2152,-59,102],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +0 | блок (1,0,2) oak_planks [пачкой]                               | блок (1,0,2) oak_planks [пачкой]                               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] |
| +0 | блок (4,0,0) chain_command_block[conditional=false,facing=east] [пачкой] | блок (4,0,0) chain_command_block[conditional=false,facing=east] [пачкой] |
| +0 | блок (0,0,2) stone [пачкой]                                    | блок (0,0,2) stone [пачкой]                                    |
| +0 | блок (2,0,0) chain_command_block[conditional=false,facing=east] [пачкой] | блок (2,0,0) chain_command_block[conditional=false,facing=east] [пачкой] |
| +0 | блок (4,0,2) dirt [пачкой]                                     | блок (4,0,2) dirt [пачкой]                                     |
| +0 | блок (1,0,0) chain_command_block[conditional=false,facing=east] [пачкой] | блок (1,0,0) chain_command_block[conditional=false,facing=east] [пачкой] |
| +0 | блок (0,0,0) command_block[conditional=false,facing=east] [пачкой] | блок (0,0,0) command_block[conditional=false,facing=east] [пачкой] |
| +0 |                                                                | робот: System Chat {"with":[{"":"@"},{"":"Блок в 2148, -59, 102 изменён"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false  ← лишнее |
| +0 |                                                                | робот: System Chat {"with":[{"":"@"},{"":"Блок в 2149, -59, 102 изменён"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false  ← лишнее |
| +0 |                                                                | робот: System Chat {"with":[{"":"@"},{"":"Блок в 2152, -59, 102 изменён"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false  ← лишнее |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| | **▶ cmd setblock $(2,0,2) minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ cmd setblock $(2,0,2) minecraft:air** |
| +0 | робот: System Chat {"color":"red","extra":[{"translate":"commands.setblock.failed"}],"text":""} overlay=false |   ← нет у нас                                                  |
| +0 |                                                                | робот: System Chat "Там уже такой блок" overlay=false  ← лишнее |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] |
| +0 | блок (4,0,0) chain_command_block[conditional=false,facing=east] [пачкой] | блок (4,0,0) chain_command_block[conditional=false,facing=east] [пачкой] |
| +0 | блок (2,0,0) chain_command_block[conditional=false,facing=east] [пачкой] | блок (2,0,0) chain_command_block[conditional=false,facing=east] [пачкой] |
| +0 | блок (1,0,0) chain_command_block[conditional=false,facing=east] [пачкой] | блок (1,0,0) chain_command_block[conditional=false,facing=east] [пачкой] |
| +0 | блок (0,0,0) command_block[conditional=false,facing=east] [пачкой] | блок (0,0,0) command_block[conditional=false,facing=east] [пачкой] |
| +7 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[5],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +11 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}}],"color":"yellow","translate":"multiplayer.player.left"} overlay=false |   ← нет у нас                                                  |
| +12 |                                                                | робот: System Chat "blackbox4 вышел" overlay=false  ← лишнее   |
| +20 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}},{"with":[289],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +22 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}},{"with":[289],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +24 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}},{"with":[{"":0},{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}}],"translate":"commands.experience.set.levels.success.single"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +24 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}},{"with":["spawn_mobs","false"],"translate":"commands.gamerule.set"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |

Совпало: 23. Сдвинуто по тактам: 0. Нет у нас: 10. Лишнее у нас: 5.
