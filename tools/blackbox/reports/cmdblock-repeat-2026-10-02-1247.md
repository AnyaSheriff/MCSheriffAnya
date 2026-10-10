# cmdblock-repeat: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.60               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +0 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      | блок (-1,0,0) lever[face=floor,facing=north,powered=true]      |
| +0 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +0 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| +1 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +1 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +1 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +1 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| +2 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +2 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +2 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +2 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| +3 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +3 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +3 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +3 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| +4 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +4 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +4 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +4 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| +5 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +5 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +5 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +5 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| | **▶ click -1,0,0** (первый отклик через 0 / 0 такт.) | **▶ click -1,0,0** |
| +0 | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               | звук (-0.5,0.5,0.5) block.lever.click 0.30 ~0.50               |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| +0 | блок (-1,1,0) air                                              | блок (-1,1,0) air                                              |
| +0 | робот: Disguised Chat "tick" name="@"                          |   ← нет у нас                                                  |
| +0 | робот: Disguised Chat "chain" name="@"                         |   ← нет у нас                                                  |
| +0 | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     | блок (-1,0,0) lever[face=floor,facing=north,powered=false]     |
| +0 |                                                                | робот: System Chat "[@] tick" overlay=false  ← лишнее          |
| +0 |                                                                | робот: System Chat "[@] chain" overlay=false  ← лишнее         |
| | **▶ cmd setblock $(2,0,0) minecraft:repeating_command_block[facing=up]{auto:1b,Command:"say auto"}** (первый отклик через 0 / 0 такт.) | **▶ cmd setblock $(2,0,0) minecraft:repeating_command_block[facing=up]{auto:1b,Command:"say auto"}** |
| +0 | робот: System Chat {"with":[102,-59,100],"translate":"commands.setblock.success"} overlay=false |   ← нет у нас                                                  |
| +0 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +0 | блок (2,0,0) repeating_command_block[conditional=false,facing=up] | блок (2,0,0) repeating_command_block[conditional=false,facing=up] |
| +0 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| +0 |                                                                | робот: System Chat "Блок в 102, -59, 100 изменён" overlay=false  ← лишнее |
| +1 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +1 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| +2 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +2 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| +3 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +3 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| | **▶ cmd setblock $(2,0,0) minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ cmd setblock $(2,0,0) minecraft:air** |
| +0 | робот: System Chat {"with":[102,-59,100],"translate":"commands.setblock.success"} overlay=false |   ← нет у нас                                                  |
| +0 | блок (2,0,0) air                                               | блок (2,0,0) air                                               |
| +0 |                                                                | робот: System Chat "Блок в 102, -59, 100 изменён" overlay=false  ← лишнее |
| +7 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}},{"with":[290],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +9 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}},{"with":[289],"translate":"commands.fill.success"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +11 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}},{"with":["spawn_mobs","false"],"translate":"commands.gamerule.set"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +13 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}},{"with":["advance_time","true"],"translate":"commands.gamerule.set"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |
| +15 | робот: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox2 "},"insertion":"blackbox2","text":"blackbox2","hover_event":{"name":"blackbox2","action":"show_entity","id":"minecraft:player","uuid":[-1917341979,-280612223,-1278939123,2083834172]}},{"translate":"commands.weather.set.clear"}],"color":"gray","italic":1,"translate":"chat.type.admin"} overlay=false |   ← нет у нас                                                  |

Совпало: 10. Сдвинуто по тактам: 0. Нет у нас: 25. Лишнее у нас: 20.
