# enderman-stare: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon enderman $(3.5,0,6.5) {Silent:1b}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon enderman $(3.5,0,6.5) {Silent:1b}** |
| +0 | сущность (3.5,0,6.5) enderman 0 3.5,0,6.5 yaw=0 pitch=0 head=0 | сущность (3.5,0,6.5) enderman 0 3.5,0,6.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 4=true 9=40                                  | свойства сущности 4=true 9=40                                  |
| +0 | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} |
| +0 | enderman сдвиг 0,0,0                                           | enderman сдвиг 0,0,0                                           |
| +0 | свойства сущности 4=true 9=40                                  | свойства сущности 4=true 9=40                                  |
| +0 | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[]}]} |
| +1 | enderman скорость 0,-0.078,0                                   | enderman скорость 0,-0.078,0                                   |
| +1 | свойства сущности 18=true                                      | свойства сущности 18=true                                      |
| +3 | enderman сверка (3.5,0,6.5) скорость 0,-0.078,0 поворот -10/0 на земле | enderman сверка (3.5,0,6.5) скорость 0,-0.078,0 поворот -10/0 на земле |
| +5 | enderman поворот -15/6 на земле                                | enderman поворот -15/6 на земле  ← случайное, в допуске: злость эндермена |
| +5 | свойства сущности 17=true                                      | свойства сущности 17=true  ← случайное, в допуске: злость эндермена |
| +5 | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[{"uuid":"minecraft:attacking","amount":0.15,"operation":0}]}]} | enderman entity_update_attributes {"properties":[{"key":"generic.scale","value":0.3,"modifiers":[{"uuid":"minecraft:attacking","amount":0.15,"operation":0}]}]}  ← случайное, в допуске: злость эндермена |
| +5 | enderman entity_head_rotation {"headYaw":-8}                   | enderman entity_head_rotation {"headYaw":-8}  ← случайное, в допуске: злость эндермена; порядок в такте другой |
| +6 | enderman entity_head_rotation {"headYaw":-15}                  |   ← нет у нас                                                  |
| +7 |                                                                | enderman entity_head_rotation {"headYaw":-22}  ← лишнее        |
| +9 | enderman entity_head_rotation {"headYaw":-36}                  |   ← нет у нас                                                  |
| +10 |                                                                | enderman entity_head_rotation {"headYaw":-43}  ← лишнее        |
| +12 | enderman entity_head_rotation {"headYaw":-57}                  |   ← нет у нас                                                  |
| +13 |                                                                | enderman entity_head_rotation {"headYaw":-64}  ← лишнее        |
| +15 | enderman entity_head_rotation {"headYaw":-79}                  |   ← нет у нас                                                  |
| +16 |                                                                | enderman entity_head_rotation {"headYaw":-86}  ← лишнее        |
| +18 | enderman entity_head_rotation {"headYaw":-100}                 |   ← нет у нас                                                  |
| +19 |                                                                | enderman entity_head_rotation {"headYaw":-107}  ← лишнее       |
| +21 | enderman entity_head_rotation {"headYaw":-121}                 |   ← нет у нас                                                  |
| +24 | enderman entity_head_rotation {"headYaw":-128}                 | enderman entity_head_rotation {"headYaw":-128}  ← у нас раньше на 2 |
| +60 | enderman сдвиг 0,0,0 на земле                                  | enderman сдвиг 0,0,0 на земле  ← у нас позже на 1              |

Случайное, в допуске: 4 (входит в «Совпало»): злость эндермена — 4.
Совпало: 13. Сдвинуто по тактам: 2. Нет у нас: 6. Лишнее у нас: 5.
