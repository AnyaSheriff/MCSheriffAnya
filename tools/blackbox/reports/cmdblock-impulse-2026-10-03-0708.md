# cmdblock-impulse: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd setblock $(0,0,0) minecraft:command_block[facing=up]{Command:"setblock ~ ~1 ~ minecraft:stone"}** (первый отклик через 0 / 0 такт.) | **▶ cmd setblock $(0,0,0) minecraft:command_block[facing=up]{Command:"setblock ~ ~1 ~ minecraft:stone"}** |
| +0 | робот: System Chat {"with":[612,-59,100],"translate":"commands.setblock.success"} overlay=false |   ← нет у нас                                                  |
| +0 | блок (0,0,0) command_block[conditional=false,facing=up]        | блок (0,0,0) command_block[conditional=false,facing=up]        |
| +0 |                                                                | робот: System Chat "Блок в 612, -59, 100 изменён" overlay=false  ← лишнее |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | робот: System Chat {"with":[{"":"@"},{"with":[612,-58,100],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] |
| +0 | блок (0,0,0) command_block[conditional=false,facing=up] [пачкой] | блок (0,0,0) command_block[conditional=false,facing=up] [пачкой] |
| +0 | блок (0,1,0) stone [пачкой]                                    | блок (0,1,0) stone [пачкой]                                    |
| +0 |                                                                | робот: System Chat {"with":[{"":"@"},{"":"Блок в 612, -58, 100 изменён"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false  ← лишнее |
| +2 | блок (3,0,0) redstone_lamp[lit=true] [пачкой]                  | блок (3,0,0) redstone_lamp[lit=true] [пачкой]                  |
| +2 | блок (2,0,0) redstone_wire[east=side,north=none,power=1,south=none,west=side] [пачкой] | блок (2,0,0) redstone_wire[east=side,north=none,power=1,south=none,west=side] [пачкой] |
| +2 | блок (1,0,0) comparator[facing=west,mode=compare,powered=true] [пачкой] | блок (1,0,0) comparator[facing=west,mode=compare,powered=true] [пачкой] |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| +1 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[293],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +3 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[289],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +5 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":["spawn_mobs","false"],"translate":"commands.gamerule.set"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +7 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":["advance_time","true"],"translate":"commands.gamerule.set"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +9 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"translate":"commands.weather.set.clear"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] |
| +0 | блок (0,0,0) command_block[conditional=false,facing=up] [пачкой] | блок (0,0,0) command_block[conditional=false,facing=up] [пачкой] |
| +1 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[3],"translate":"commands.kill.success.multiple"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +2 | блок (2,0,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] | блок (2,0,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |
| +2 | блок (1,0,0) comparator[facing=west,mode=compare,powered=false] [пачкой] | блок (1,0,0) comparator[facing=west,mode=compare,powered=false] [пачкой] |
| +6 | блок (3,0,0) redstone_lamp[lit=false]                          | блок (3,0,0) redstone_lamp[lit=false]                          |
| | **▶ click 0,0,0** | **▶ click 0,0,0** |
| | **▶ program 0,0,0 setblock ~ ~1 ~ minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ program 0,0,0 setblock ~ ~1 ~ minecraft:air** |
| +0 | данные блока (0,0,0) 23 {"conditionMet":1,"auto":0,"powered":1,"Command":"setblock ~ ~1 ~ minecraft:stone","LastExecution":[0,3664506],"SuccessCount":0,"LastOutput":{"extra":[{"color":"red","extra":[{"translate":"commands.setblock.failed"}],"text":""}],"text":"[10:08:21] "},"TrackOutput":1,"UpdateLastExecution":1} | данные блока (0,0,0) 23 {"conditionMet":1,"auto":0,"powered":1,"Command":"setblock ~ ~1 ~ minecraft:stone","LastExecution":[0,4935316],"SuccessCount":0,"LastOutput":{"extra":[{"color":"red","extra":[{"text":"Там уже такой блок"}],"text":""}],"text":"[10:08:21] "},"TrackOutput":1,"UpdateLastExecution":1} |
| +0 | блок (0,0,0) command_block[conditional=false,facing=up]        | блок (0,0,0) command_block[conditional=false,facing=up]  ← порядок в такте другой |
| +0 | блок (0,1,0) stone                                             | блок (0,1,0) stone  ← порядок в такте другой                   |
| +0 | робот: System Chat {"with":["setblock ~ ~1 ~ minecraft:air"],"translate":"advMode.setCommand.success"} overlay=false | робот: System Chat {"with":["setblock ~ ~1 ~ minecraft:air"],"translate":"advMode.setCommand.success"} overlay=false |
| +0 | блок (0,0,0) command_block[conditional=false,facing=up]        | блок (0,0,0) command_block[conditional=false,facing=up]  ← порядок в такте другой |
| | **▶ click -1,0,0** | **▶ click -1,0,0** |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | робот: System Chat {"with":[{"":"@"},{"with":[612,-58,100],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] | блок (-1,0,0) lever[face=floor,facing=north,powered=true] [пачкой] |
| +0 | блок (0,0,0) command_block[conditional=false,facing=up] [пачкой] | блок (0,0,0) command_block[conditional=false,facing=up] [пачкой] |
| +0 | блок (0,1,0) air [пачкой]                                      | блок (0,1,0) air [пачкой]                                      |
| +0 |                                                                | робот: System Chat {"with":[{"":"@"},{"":"Блок в 612, -58, 100 изменён"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false  ← лишнее |
| +1 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[1644,-59,108],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +2 | блок (3,0,0) redstone_lamp[lit=true] [пачкой]                  | блок (3,0,0) redstone_lamp[lit=true] [пачкой]                  |
| +2 | блок (2,0,0) redstone_wire[east=side,north=none,power=1,south=none,west=side] [пачкой] | блок (2,0,0) redstone_wire[east=side,north=none,power=1,south=none,west=side] [пачкой] |
| +2 | блок (1,0,0) comparator[facing=west,mode=compare,powered=true] [пачкой] | блок (1,0,0) comparator[facing=west,mode=compare,powered=true] [пачкой] |
| +12 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[1644,-59,108],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +22 |                                                                | робот: System Chat "blackbox вышел" overlay=false  ← лишнее    |
| +23 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox4 "},"insertion":"blackbox4","text":"blackbox4","hover_event":{"name":"blackbox4","action":"show_entity","id":"minecraft:player","uuid":[1046487257,691222693,-1775627197,1307832598]}},{"with":[1644,-59,108],"translate":"commands.setblock.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +25 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}}],"color":"yellow","translate":"multiplayer.player.left"} overlay=false |   ← нет у нас                                                  |

Совпало: 39. Сдвинуто по тактам: 0. Нет у нас: 13. Лишнее у нас: 4.
