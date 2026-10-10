# effect-instant: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd effect give $(bot) minecraft:instant_health 1 0** (первый отклик через 0 / 0 такт.) | **▶ cmd effect give $(bot) minecraft:instant_health 1 0** |
| +0 | робот: Effect id=5 amp=0 dur=1 flags=14                        | робот: Effect id=5 amp=0 dur=1 flags=14                        |
| +0 | робот: Metadata 10=[{"type":"entity_effect","data":-515037}]   | робот: Metadata 10=[{"type":"entity_effect","data":-515037}]   |
| +0 | робот: Remove Effect id=5                                      | робот: Remove Effect id=5                                      |
| +0 | робот: Set Health health=14 food=20 saturation=5               | робот: Set Health health=14 food=20 saturation=5               |
| +1 | робот: Metadata 9=14 10=[]                                     | робот: Metadata 9=14 10=[]                                     |
| | **▶ cmd effect give $(bot) minecraft:instant_damage 1 0** (первый отклик через 0 / 0 такт.) | **▶ cmd effect give $(bot) minecraft:instant_damage 1 0** |
| +0 | робот: Effect id=6 amp=0 dur=1 flags=14                        | робот: Effect id=6 amp=0 dur=1 flags=14                        |
| +0 | робот: Metadata 10=[{"type":"entity_effect","data":-5675670}]  | робот: Metadata 10=[{"type":"entity_effect","data":-5675670}]  |
| +0 | робот: Damage Event type=27 cause=0 direct=0 pos=none          | робот: Damage Event type=27 cause=0 direct=0 pos=none          |
| +0 | робот: Remove Effect id=6                                      | робот: Remove Effect id=6                                      |
| +0 | робот: Set Health health=8 food=20 saturation=5                | робот: Set Health health=8 food=20 saturation=5                |
| +1 | робот: Metadata 9=8 10=[]                                      | робот: Metadata 9=8 10=[]                                      |
| +1 | робот: скорость 0,-0.078,0                                     | робот: скорость 0,-0.078,0                                     |
| | **▶ cmd effect give $(bot) minecraft:instant_health 1 1 true** (первый отклик через 0 / 0 такт.) | **▶ cmd effect give $(bot) minecraft:instant_health 1 1 true** |
| +0 | робот: Effect id=5 amp=1 dur=1 flags=8                         | робот: Effect id=5 amp=1 dur=1 flags=8                         |
| +0 | робот: Metadata 11=true                                        | робот: Metadata 11=true                                        |
| +0 | робот: Remove Effect id=5                                      | робот: Remove Effect id=5                                      |
| +0 | робот: Set Health health=16 food=20 saturation=5               | робот: Set Health health=16 food=20 saturation=5               |
| +1 | робот: Metadata 9=16                                           | робот: Metadata 9=16                                           |

Совпало: 17. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
