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
| +0 | робот: System Chat {"with":[2150,-59,100],"translate":"commands.setblock.success"} overlay=false |   ← нет у нас                                                  |
| +0 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +0 | блок (2,0,0) repeating_command_block[conditional=false,facing=up] | блок (2,0,0) repeating_command_block[conditional=false,facing=up] |
| +0 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| +0 |                                                                | робот: System Chat "Блок в 2150, -59, 100 изменён" overlay=false  ← лишнее |
| +1 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +1 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| +2 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +2 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| +3 | робот: Disguised Chat "auto" name="@"                          |   ← нет у нас                                                  |
| +3 |                                                                | робот: System Chat "[@] auto" overlay=false  ← лишнее          |
| | **▶ cmd setblock $(2,0,0) minecraft:air** (первый отклик через 0 / 0 такт.) | **▶ cmd setblock $(2,0,0) minecraft:air** |
| +0 | робот: System Chat {"with":[2150,-59,100],"translate":"commands.setblock.success"} overlay=false |   ← нет у нас                                                  |
| +0 | блок (2,0,0) air                                               | блок (2,0,0) air                                               |
| +0 |                                                                | робот: System Chat "Блок в 2150, -59, 100 изменён" overlay=false  ← лишнее |

Совпало: 10. Сдвинуто по тактам: 0. Нет у нас: 20. Лишнее у нас: 20.
