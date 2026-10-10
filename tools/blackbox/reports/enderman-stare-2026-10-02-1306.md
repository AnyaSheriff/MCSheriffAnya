# enderman-stare: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon enderman $(3.5,0,6.5) {Silent:1b}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon enderman $(3.5,0,6.5) {Silent:1b}** |
| +0 | сущность (3.5,0,6.5) enderman 0 3.5,0,6.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,6.5) enderman 0 3.5,0,6.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 4=true 9=40                                  | свойства сущности 4=true 9=40                                  |
| +0 | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} |
| +1 | enderman сдвиг 0,0,0                                           | enderman сдвиг 0,0,0  ← у нас раньше на 1                      |
| +1 | свойства сущности 4=true 9=40                                  | свойства сущности 4=true 9=40  ← у нас раньше на 1             |
| +1 | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]}  ← у нас раньше на 1 |
| +2 | enderman скорость 0,-0.078,0                                   | enderman скорость 0,-0.078,0  ← у нас позже на 1               |
| +2 | свойства сущности 18=true                                      | свойства сущности 18=true  ← у нас раньше на 1                 |
| +3 |                                                                | enderman сверка (3.5,0,6.5) скорость 0,-0.078,0 поворот 0/0 на земле  ← лишнее |
| +4 | enderman сверка (3.5,0,6.5) скорость 0,-0.078,0 поворот -10/0 на земле |   ← нет у нас                                                  |
| +6 | enderman поворот -15/6 на земле                                |   ← нет у нас                                                  |
| +6 | свойства сущности 17=true                                      | свойства сущности 17=true  ← у нас раньше на 2                 |
| +6 | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[{"uuid":"minecraft:attacking","amount":0.15,"operation":0}]}]} | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[{"uuid":"minecraft:attacking","amount":0.15,"operation":0}]}]}  ← у нас раньше на 2 |
| +6 | enderman entity_head_rotation {"headYaw":-8}                   |   ← нет у нас                                                  |
| +6 |                                                                | enderman поворот 0/6 на земле  ← лишнее                        |
| +6 |                                                                | enderman entity_head_rotation {"headYaw":-22}  ← лишнее        |
| +7 | enderman entity_head_rotation {"headYaw":-15}                  |   ← нет у нас                                                  |
| +9 | enderman entity_head_rotation {"headYaw":-36}                  |   ← нет у нас                                                  |
| +9 |                                                                | enderman entity_head_rotation {"headYaw":-43}  ← лишнее        |
| +12 |                                                                | enderman entity_head_rotation {"headYaw":-64}  ← лишнее        |
| +13 | enderman entity_head_rotation {"headYaw":-57}                  |   ← нет у нас                                                  |
| +15 |                                                                | enderman entity_head_rotation {"headYaw":-86}  ← лишнее        |
| +16 | enderman entity_head_rotation {"headYaw":-79}                  |   ← нет у нас                                                  |
| +18 |                                                                | enderman entity_head_rotation {"headYaw":-107}  ← лишнее       |
| +19 | enderman entity_head_rotation {"headYaw":-100}                 |   ← нет у нас                                                  |
| +21 | enderman entity_head_rotation {"headYaw":-121}                 |   ← нет у нас                                                  |
| +24 | enderman entity_head_rotation {"headYaw":-128}                 | enderman entity_head_rotation {"headYaw":-128}  ← у нас раньше на 3 |
| +60 | enderman сдвиг 0,0,0 на земле                                  | enderman сдвиг 0,0,0 на земле                                  |

Совпало: 4. Сдвинуто по тактам: 8. Нет у нас: 9. Лишнее у нас: 7.
