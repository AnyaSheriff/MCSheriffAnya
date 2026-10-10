# score-criteria: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd scoreboard objectives add sc_d deathCount** (первый отклик через 0 / 0 такт.) | **▶ cmd scoreboard objectives add sc_d deathCount** |
| +0 | табло: System Chat {"with":[{"with":[{"text":"sc_d","hover_event":{"action":"show_text","value":"sc_d"}}],"translate":"chat.square_brackets"}],"translate":"commands.scoreboard.objectives.add.success"} overlay=false | табло: System Chat {"with":[{"with":[{"text":"sc_d","hover_event":{"action":"show_text","value":"sc_d"}}],"translate":"chat.square_brackets"}],"translate":"commands.scoreboard.objectives.add.success"} overlay=false |
| | **▶ cmd scoreboard objectives add sc_h health** (первый отклик через 0 / 0 такт.) | **▶ cmd scoreboard objectives add sc_h health** |
| +0 | табло: System Chat {"with":[{"with":[{"text":"sc_h","hover_event":{"action":"show_text","value":"sc_h"}}],"translate":"chat.square_brackets"}],"translate":"commands.scoreboard.objectives.add.success"} overlay=false | табло: System Chat {"with":[{"with":[{"text":"sc_h","hover_event":{"action":"show_text","value":"sc_h"}}],"translate":"chat.square_brackets"}],"translate":"commands.scoreboard.objectives.add.success"} overlay=false |
| | **▶ cmd scoreboard objectives setdisplay sidebar sc_d** (первый отклик через 0 / 0 такт.) | **▶ cmd scoreboard objectives setdisplay sidebar sc_d** |
| +0 | табло: Objective sc_d action=0 text="sc_d" type=0 format=undefined | табло: Objective sc_d action=0 text="sc_d" type=0 format=undefined |
| +0 | табло: Display slot=1 name=sc_d                                | табло: Display slot=1 name=sc_d                                |
| +0 | табло: System Chat {"with":["sidebar","sc_d"],"translate":"commands.scoreboard.objectives.display.set"} overlay=false | табло: System Chat {"with":["sidebar","sc_d"],"translate":"commands.scoreboard.objectives.display.set"} overlay=false |
| | **▶ cmd scoreboard objectives setdisplay list sc_h** (первый отклик через 0 / 0 такт.) | **▶ cmd scoreboard objectives setdisplay list sc_h** |
| +0 | табло: Objective sc_h action=0 text="sc_h" type=1 format=undefined | табло: Objective sc_h action=0 text="sc_h" type=1 format=undefined |
| +0 | табло: Display slot=0 name=sc_h                                | табло: Display slot=0 name=sc_h                                |
| +0 | табло: System Chat {"with":["list","sc_h"],"translate":"commands.scoreboard.objectives.display.set"} overlay=false | табло: System Chat {"with":["list","sc_h"],"translate":"commands.scoreboard.objectives.display.set"} overlay=false |
| +0 |                                                                | табло: Score blackbox sc_h=20 display=none format=undefined  ← лишнее |
| | **▶ cmd damage $(bot) 3** (первый отклик через 0 / 0 такт.) | **▶ cmd damage $(bot) 3** |
| +0 | табло: System Chat {"with":[{"":3},{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}}],"translate":"commands.damage.success"} overlay=false |   ← нет у нас                                                  |
| +0 | табло: Score blackbox sc_h=17 display=none format=undefined    | табло: Score blackbox sc_h=17 display=none format=undefined    |
| +0 |                                                                | табло: System Chat "Нанесён урон: blackbox" overlay=false  ← лишнее |
| +9 | табло: Score blackbox sc_h=18 display=none format=undefined    | табло: Score blackbox sc_h=18 display=none format=undefined  ← у нас позже на 1 |
| +18 | табло: Score blackbox sc_h=19 display=none format=undefined    | табло: Score blackbox sc_h=19 display=none format=undefined  ← у нас раньше на 1 |
| +19 | табло: Score blackbox sc_h=20 display=none format=undefined    | табло: Score blackbox sc_h=20 display=none format=undefined    |
| | **▶ cmd kill $(bot)** (первый отклик через 0 / 0 такт.) | **▶ cmd kill $(bot)** |
| +0 | табло: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}}],"translate":"death.attack.genericKill"} overlay=false | табло: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}}],"translate":"death.attack.genericKill"} overlay=false |
| +0 | табло: Score blackbox sc_d=1 display=none format=undefined     | табло: Score blackbox sc_d=1 display=none format=undefined     |
| +0 | табло: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"name":"blackbox","action":"show_entity","id":"minecraft:player","uuid":[651459017,1212431114,-1168536378,1449273976]}}],"translate":"commands.kill.success.single"} overlay=false |   ← нет у нас                                                  |
| +0 | табло: Score blackbox sc_h=0 display=none format=undefined     | табло: Score blackbox sc_h=0 display=none format=undefined     |
| +0 |                                                                | табло: System Chat "Убит: blackbox" overlay=false  ← лишнее    |
| +1 | табло: Score blackbox sc_h=20 display=none format=undefined    | табло: Score blackbox sc_h=20 display=none format=undefined  ← у нас позже на 2 |
| | **▶ respawn** | **▶ respawn** |
| | **▶ cmd scoreboard players get $(bot) sc_d** (первый отклик через 0 / 0 такт.) | **▶ cmd scoreboard players get $(bot) sc_d** |
| +0 | табло: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"action":"show_text","value":"blackbox"}},{"":1},{"with":[{"text":"sc_d","hover_event":{"action":"show_text","value":"sc_d"}}],"translate":"chat.square_brackets"}],"translate":"commands.scoreboard.players.get.success"} overlay=false | табло: System Chat {"with":[{"click_event":{"action":"suggest_command","command":"/tell blackbox "},"insertion":"blackbox","text":"blackbox","hover_event":{"action":"show_text","value":"blackbox"}},{"":1},{"with":[{"text":"sc_d","hover_event":{"action":"show_text","value":"sc_d"}}],"translate":"chat.square_brackets"}],"translate":"commands.scoreboard.players.get.success"} overlay=false |

Совпало: 14. Сдвинуто по тактам: 3. Нет у нас: 2. Лишнее у нас: 3.
