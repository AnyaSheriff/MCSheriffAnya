# effect-absorption: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd effect give $(bot) minecraft:absorption 3 1** (первый отклик через 0 / 0 такт.) | **▶ cmd effect give $(bot) minecraft:absorption 3 1** |
| +0 | робот: Effect id=21 amp=1 dur=60 flags=14                      | робот: Effect id=21 amp=1 dur=60 flags=14                      |
| +0 | робот: Metadata 10=[{"type":"entity_effect","data":-14331227}] 17=8 | робот: Metadata 10=[{"type":"entity_effect","data":-14331227}] 17=8 |
| +0 | робот: Attributes [{"key":"generic.max_absorption","value":0,"modifiers":[{"uuid":"minecraft:effect.absorption","amount":8,"operation":0}]}] | робот: Attributes [{"key":"generic.max_absorption","value":0,"modifiers":[{"uuid":"minecraft:effect.absorption","amount":8,"operation":0}]}] |
| | **▶ cmd effect give $(bot) minecraft:health_boost 3 0** (первый отклик через 0 / 0 такт.) | **▶ cmd effect give $(bot) minecraft:health_boost 3 0** |
| +0 | робот: Effect id=20 amp=0 dur=60 flags=14                      | робот: Effect id=20 amp=0 dur=60 flags=14                      |
| +0 | робот: Metadata 10=[{"type":"entity_effect","data":-14331227},{"type":"entity_effect","data":-492253}] | робот: Metadata 10=[{"type":"entity_effect","data":-14331227},{"type":"entity_effect","data":-492253}] |
| +0 | робот: Attributes [{"key":"generic.max_health","value":20,"modifiers":[{"uuid":"minecraft:effect.health_boost","amount":4,"operation":0}]}] | робот: Attributes [{"key":"generic.max_health","value":20,"modifiers":[{"uuid":"minecraft:effect.health_boost","amount":4,"operation":0}]}] |
| | **▶ cmd damage $(bot) 5** (первый отклик через 0 / 0 такт.) | **▶ cmd damage $(bot) 5** |
| +0 | робот: Damage Event type=18 cause=0 direct=0 pos=none          | робот: Damage Event type=18 cause=0 direct=0 pos=none          |
| +0 | робот: Metadata 17=3                                           | робот: Metadata 17=3                                           |
| +0 | робот: скорость 0,-0.078,0                                     | робот: скорость 0,-0.078,0                                     |
| | **▶ cmd damage $(bot) 5** (первый отклик через 0 / 0 такт.) | **▶ cmd damage $(bot) 5** |
| +0 | робот: Damage Event type=18 cause=0 direct=0 pos=none          | робот: Damage Event type=18 cause=0 direct=0 pos=none          |
| +0 | робот: Metadata 9=18 17=0                                      | робот: Metadata 9=18 17=0                                      |
| +0 | робот: скорость 0,-0.078,0                                     | робот: скорость 0,-0.078,0                                     |
| +0 | робот: Remove Effect id=21                                     | робот: Remove Effect id=21                                     |
| +0 | робот: Set Health health=18 food=20 saturation=5               | робот: Set Health health=18 food=20 saturation=5               |
| +1 | робот: Metadata 10=[{"type":"entity_effect","data":-492253}]   | робот: Metadata 10=[{"type":"entity_effect","data":-492253}]   |
| +1 | робот: Attributes [{"key":"generic.max_absorption","value":0,"modifiers":[]}] | робот: Attributes [{"key":"generic.max_absorption","value":0,"modifiers":[]}] |
| +29 | робот: Remove Effect id=20                                     | робот: Remove Effect id=20                                     |
| +30 | робот: Metadata 10=[]                                          | робот: Metadata 10=[]                                          |
| +30 | робот: Attributes [{"key":"generic.max_health","value":20,"modifiers":[]}] | робот: Attributes [{"key":"generic.max_health","value":20,"modifiers":[]}] |

Совпало: 19. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
