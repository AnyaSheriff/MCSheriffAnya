# dragon-hit-head: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd summon ender_dragon $(0.5,0,0.5) {NoAI:1b}** (первый отклик через 0 / 0 такт.) | **▶ cmd summon ender_dragon $(0.5,0,0.5) {NoAI:1b}** |
| +0 | сущность (0.5,0,0.5) ender_dragon 0 0.5,0,0.5 yaw=0 pitch=0 head=0 | сущность (0.5,0,0.5) ender_dragon 0 0.5,0,0.5 yaw=0 pitch=0 head=0 |
| +0 | свойства сущности 9=200 15=1                                   | свойства сущности 9=200 15=1                                   |
| +0 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0                                       |
| +0 | свойства сущности 9=200 15=1                                   | свойства сущности 9=200 15=1                                   |
| +6 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.ambient 5.00 ~1.10  ← лишнее |
| | **▶ attack** | **▶ attack** |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~1.15  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 3 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.4  ← лишнее                       |
| +0 |                                                                | свойства сущности 9=193  ← лишнее                              |
| +0 |                                                                | ender_dragon скорость 0,0,-0.4  ← лишнее                       |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| | **▶ attack** (первый отклик через 15 / 0 такт.) | **▶ attack**  ← отклик у нас раньше на 15 |
| +0 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0  ← у нас позже на 15                  |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~1.02  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.6  ← лишнее                       |
| +0 |                                                                | свойства сущности 9=190.25  ← лишнее                           |
| +0 |                                                                | ender_dragon скорость 0,0,-0.6  ← лишнее                       |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| | **▶ attack** (первый отклик через 11 / 0 такт.) | **▶ attack**  ← отклик у нас раньше на 11 |
| +0 | звук (0.5,0,0.5) entity.ender_dragon.ambient 5.00 ~0.98        |   ← нет у нас                                                  |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~1.03  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.7  ← лишнее                       |
| +0 |                                                                | свойства сущности 9=187.5  ← лишнее                            |
| +0 |                                                                | ender_dragon скорость 0,0,-0.7  ← лишнее                       |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| | **▶ attack** | **▶ attack** |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~1.03  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.75  ← лишнее                      |
| +0 |                                                                | свойства сущности 9=184.75  ← лишнее                           |
| +0 |                                                                | ender_dragon скорость 0,0,-0.75  ← лишнее                      |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| | **▶ attack** (первый отклик через 0 / 0 такт.) | **▶ attack** |
| +0 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0                                       |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~0.99  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.775  ← лишнее                     |
| +0 |                                                                | свойства сущности 9=182  ← лишнее                              |
| +0 |                                                                | ender_dragon скорость 0,0,-0.775  ← лишнее                     |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| +22 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.ambient 5.00 ~1.08  ← лишнее |
| | **▶ attack** | **▶ attack** |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~0.98  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.788  ← лишнее                     |
| +0 |                                                                | свойства сущности 9=179.25  ← лишнее                           |
| +0 |                                                                | ender_dragon скорость 0,0,-0.788  ← лишнее                     |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| | **▶ attack** (первый отклик через 10 / 0 такт.) | **▶ attack**  ← отклик у нас раньше на 10 |
| +0 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0  ← у нас позже на 10                  |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~0.95  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.794  ← лишнее                     |
| +0 |                                                                | свойства сущности 9=176.5  ← лишнее                            |
| +0 |                                                                | ender_dragon скорость 0,0,-0.794  ← лишнее                     |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| +11 | звук (0.5,0,0.5) entity.ender_dragon.ambient 5.00 ~1.06        |   ← нет у нас                                                  |
| | **▶ attack** (первый отклик через 45 / 0 такт.) | **▶ attack**  ← отклик у нас раньше на 45 |
| +0 | ender_dragon сдвиг 0,0,0                                       | ender_dragon сдвиг 0,0,0  ← у нас позже на 45                  |
| +0 |                                                                | ender_dragon damage_event {"sourceTypeId":34,"sourceCauseId":"entity","sourceDirectId":"entity"}  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.hurt 5.00 ~0.91  ← лишнее |
| +0 |                                                                | звук (0.5,0,0.5) entity.player.attack.sweep 1.00 ~1.00  ← лишнее |
| +0 |                                                                | частица (0.5,0.9,-0.5) sweep_attack 0 шт. скорость 0 (0,0,-1)  ← лишнее |
| +0 |                                                                | частица (0.5,4,0.5) damage_indicator 1 шт. скорость 0.2 (0.1,0,0.1)  ← лишнее |
| +0 |                                                                | ender_dragon скорость 0,0,-0.797  ← лишнее                     |
| +0 |                                                                | свойства сущности 9=173.75  ← лишнее                           |
| +0 |                                                                | ender_dragon скорость 0,0,-0.797  ← лишнее                     |
| +2 |                                                                | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← лишнее |
| +39 |                                                                | звук (0.5,0,0.5) entity.ender_dragon.ambient 5.00 ~1.00  ← лишнее |

Совпало: 5. Сдвинуто по тактам: 7. Нет у нас: 2. Лишнее у нас: 75.
