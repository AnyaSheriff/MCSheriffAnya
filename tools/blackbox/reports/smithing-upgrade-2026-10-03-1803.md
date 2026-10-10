# smithing-upgrade: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 0,0,0** (первый отклик через 0 / 0 такт.) | **▶ click 0,0,0** |
| +0 | окно 21                                                        | окно 21                                                        |
| +0 | содержимое окна 31:netherite_upgrade_smithing_template×2 32:diamond_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} 33:netherite_ingot×2 34:diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} 35:iron_sword×1 | содержимое окна 31:netherite_upgrade_smithing_template×2 32:diamond_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} 33:netherite_ingot×2 34:diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} 35:iron_sword×1 |
| +0 | свойство окна 0=0                                              | свойство окна 0=0                                              |
| +0 | блок (0,0,0) smithing_table                                    | блок (0,0,0) smithing_table                                    |
| +0 | блок (0,1,0) air                                               | блок (0,1,0) air                                               |
| | **▶ window_click {"slot":31,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":31,"mode":1}** |
| +0 | ячейка 0: netherite_upgrade_smithing_template×2                | ячейка 0: netherite_upgrade_smithing_template×2                |
| +0 | ячейка 31: пусто                                               | ячейка 31: пусто                                               |
| | **▶ window_click {"slot":32,"mode":1}** (первый отклик через 0 / 3 такт.) | **▶ window_click {"slot":32,"mode":1}**  ← отклик у нас позже на 3 |
| +0 | ячейка 1: diamond_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} | ячейка 1: diamond_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} |
| +0 | ячейка 32: пусто                                               | ячейка 32: пусто                                               |
| | **▶ window_click {"slot":33,"mode":1}** (первый отклик через 0 / 1 такт.) | **▶ window_click {"slot":33,"mode":1}** |
| +0 | ячейка 2: netherite_ingot×2                                    | ячейка 2: netherite_ingot×2                                    |
| +0 | ячейка 3: netherite_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} | ячейка 3: netherite_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} |
| +0 | ячейка 33: пусто                                               | ячейка 33: пусто                                               |
| | **▶ window_click {"slot":3,"mode":0}** (первый отклик через 0 / 3 такт.) | **▶ window_click {"slot":3,"mode":0}**  ← отклик у нас позже на 3 |
| +0 | событие мира (0,0,0) 1044/0                                    | событие мира (0,0,0) 1044/0                                    |
| +0 | ячейка 0: netherite_upgrade_smithing_template×1                | ячейка 0: netherite_upgrade_smithing_template×1                |
| +0 | ячейка 1: пусто                                                | ячейка 1: пусто                                                |
| +0 | ячейка 2: netherite_ingot×1                                    | ячейка 2: netherite_ingot×1                                    |
| +0 | ячейка 3: пусто                                                | ячейка 3: пусто                                                |
| +0 | курсор: netherite_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} | курсор: netherite_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} |
| | **▶ window_click {"slot":20,"mode":0}** (первый отклик через 0 / 7 такт.) | **▶ window_click {"slot":20,"mode":0}**  ← отклик у нас позже на 7 |
| +0 | ячейка 20: netherite_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} | ячейка 20: netherite_chestplate×1 {damage=50 enchantments={"enchantments":[{"id":28,"level":3}]}} |
| | **▶ window_click {"slot":34,"mode":1}** (первый отклик через 0 / 5 такт.) | **▶ window_click {"slot":34,"mode":1}**  ← отклик у нас позже на 5 |
| +0 | ячейка 1: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} | ячейка 1: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} |
| +0 | ячейка 3: netherite_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} | ячейка 3: netherite_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} |
| +0 | ячейка 34: пусто                                               | ячейка 34: пусто                                               |
| | **▶ window_click {"slot":3,"mode":1}** | **▶ window_click {"slot":3,"mode":1}** |
| +0 | событие мира (0,0,0) 1044/0                                    |   ← нет у нас                                                  |
| +0 | ячейка 0: пусто                                                |   ← нет у нас                                                  |
| +0 | ячейка 1: пусто                                                |   ← нет у нас                                                  |
| +0 | ячейка 2: пусто                                                |   ← нет у нас                                                  |
| +0 | ячейка 3: пусто                                                |   ← нет у нас                                                  |
| +0 | ячейка 39: netherite_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}} |   ← нет у нас                                                  |
| | **▶ window_click {"slot":35,"mode":1}** (первый отклик через 0 / 8 такт.) | **▶ window_click {"slot":35,"mode":1}**  ← отклик у нас позже на 8 |
| +0 | ячейка 4: iron_sword×1                                         | ячейка 4: iron_sword×1  ← у нас позже на 1                     |
| +0 | ячейка 35: пусто                                               | ячейка 35: пусто  ← у нас позже на 1                           |
| +0 |                                                                | событие мира (0,0,0) 1044/0  ← лишнее                          |
| +0 |                                                                | ячейка 0: пусто  ← лишнее                                      |
| +0 |                                                                | ячейка 1: пусто  ← лишнее                                      |
| +0 |                                                                | ячейка 2: пусто  ← лишнее                                      |
| +0 |                                                                | ячейка 3: пусто  ← лишнее                                      |
| +0 |                                                                | ячейка 39: netherite_sword×1 {enchantments={"enchantments":[{"id":33,"level":2}]}}  ← лишнее |
| | **▶ close** | **▶ close** |

Совпало: 22. Сдвинуто по тактам: 7. Нет у нас: 6. Лишнее у нас: 6.
