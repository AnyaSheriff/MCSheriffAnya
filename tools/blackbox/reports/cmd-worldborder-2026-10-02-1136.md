# cmd-worldborder: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd worldborder get** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder get** |
| +0 | робот: System Chat {"with":["59999968"],"translate":"commands.worldborder.get"} overlay=false | робот: System Chat {"with":["59999968"],"translate":"commands.worldborder.get"} overlay=false |
| | **▶ cmd worldborder set 1000** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder set 1000** |
| +0 | робот: world_border_size {"diameter":1000}                     | робот: world_border_size {"diameter":1000}                     |
| +0 | робот: System Chat {"with":["1000.0"],"translate":"commands.worldborder.set.immediate"} overlay=false | робот: System Chat {"with":["1000.0"],"translate":"commands.worldborder.set.immediate"} overlay=false |
| | **▶ cmd worldborder set 1000** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder set 1000** |
| +0 | робот: System Chat {"color":"red","extra":[{"translate":"commands.worldborder.set.failed.nochange"}],"text":""} overlay=false | робот: System Chat {"color":"red","extra":[{"translate":"commands.worldborder.set.failed.nochange"}],"text":""} overlay=false |
| | **▶ cmd worldborder add -20 5** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder add -20 5** |
| +0 | робот: world_border_lerp_size {"oldDiameter":1000,"newDiameter":980,"speed":5} | робот: world_border_lerp_size {"oldDiameter":1000,"newDiameter":980,"speed":5} |
| +0 | робот: System Chat {"with":["980.0","0.25"],"translate":"commands.worldborder.set.shrink"} overlay=false | робот: System Chat {"with":["980.0","0.25"],"translate":"commands.worldborder.set.shrink"} overlay=false |
| | **▶ cmd worldborder set 1500.25 2s** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder set 1500.25 2s** |
| +0 | робот: world_border_lerp_size {"oldDiameter":980,"newDiameter":1500.25,"speed":40} | робот: world_border_lerp_size {"oldDiameter":980,"newDiameter":1500.25,"speed":40} |
| +0 | робот: System Chat {"with":["1500.3","2.00"],"translate":"commands.worldborder.set.grow"} overlay=false | робот: System Chat {"with":["1500.3","2.00"],"translate":"commands.worldborder.set.grow"} overlay=false |
| | **▶ cmd worldborder center 10.5 -20.25** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder center 10.5 -20.25** |
| +0 | робот: world_border_center {"x":10.5,"z":-20.25}               | робот: world_border_center {"x":10.5,"z":-20.25}               |
| +0 | робот: System Chat {"with":["10.50","-20.25"],"translate":"commands.worldborder.center.success"} overlay=false | робот: System Chat {"with":["10.50","-20.25"],"translate":"commands.worldborder.center.success"} overlay=false |
| | **▶ cmd worldborder damage amount 0.5** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder damage amount 0.5** |
| +0 | робот: System Chat {"with":["0.50"],"translate":"commands.worldborder.damage.amount.success"} overlay=false | робот: System Chat {"with":["0.50"],"translate":"commands.worldborder.damage.amount.success"} overlay=false |
| | **▶ cmd worldborder damage buffer 5** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder damage buffer 5** |
| +0 | робот: System Chat {"color":"red","extra":[{"translate":"commands.worldborder.damage.buffer.failed"}],"text":""} overlay=false | робот: System Chat {"color":"red","extra":[{"translate":"commands.worldborder.damage.buffer.failed"}],"text":""} overlay=false |
| | **▶ cmd worldborder warning distance 7** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder warning distance 7** |
| +0 | робот: world_border_warning_reach {"warningBlocks":7}          | робот: world_border_warning_reach {"warningBlocks":7}          |
| +0 | робот: System Chat {"with":[7],"translate":"commands.worldborder.warning.distance.success"} overlay=false | робот: System Chat {"with":[7],"translate":"commands.worldborder.warning.distance.success"} overlay=false |
| | **▶ cmd worldborder warning time 2s** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder warning time 2s** |
| +0 | робот: world_border_warning_delay {"warningTime":40}           | робот: world_border_warning_delay {"warningTime":40}           |
| +0 | робот: System Chat {"with":["2.00"],"translate":"commands.worldborder.warning.time.success"} overlay=false | робот: System Chat {"with":["2.00"],"translate":"commands.worldborder.warning.time.success"} overlay=false |
| | **▶ cmd worldborder set 0.5** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder set 0.5** |
| +0 | робот: System Chat {"color":"red","extra":[{"translate":"commands.worldborder.set.failed.small"}],"text":""} overlay=false | робот: System Chat {"color":"red","extra":[{"translate":"commands.worldborder.set.failed.small"}],"text":""} overlay=false |
| | **▶ cmd worldborder get** (первый отклик через 0 / 0 такт.) | **▶ cmd worldborder get** |
| +0 | робот: System Chat {"with":["1500"],"translate":"commands.worldborder.get"} overlay=false | робот: System Chat {"with":["1500"],"translate":"commands.worldborder.get"} overlay=false |

Совпало: 18. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
