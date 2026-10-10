# anvil-rename-repair: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 0,0,0** (первый отклик через 0 / 0 такт.) | **▶ click 0,0,0** |
| +0 | окно 8                                                         | окно 8                                                         |
| +0 | содержимое окна 30:diamond_sword×1 31:iron_sword×1 {damage=200} 32:iron_ingot×3 33:diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":4}]}} 34:enchanted_book×1 {stored_enchantments={"enchantments":[{"id":33,"level":4}]}} | содержимое окна 30:diamond_sword×1 31:iron_sword×1 {damage=200} 32:iron_ingot×3 33:diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":4}]}} 34:enchanted_book×1 {stored_enchantments={"enchantments":[{"id":33,"level":4}]}} |
| +0 | свойство окна 0=0                                              | свойство окна 0=0                                              |
| +0 | блок (0,0,0) anvil[facing=north]                               | блок (0,0,0) anvil[facing=north]                               |
| +0 | блок (0,1,0) air                                               | блок (0,1,0) air                                               |
| | **▶ window_click {"slot":30,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":30,"mode":1}** |
| +0 | ячейка 0: diamond_sword×1                                      | ячейка 0: diamond_sword×1                                      |
| +0 | ячейка 30: пусто                                               | ячейка 30: пусто                                               |
| +1 | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[]}] | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[]}]  ← у нас позже на 1 |
| | **▶ rename Меч** (первый отклик через 0 / 0 такт.) | **▶ rename Меч** |
| +0 | ячейка 2: diamond_sword×1 {custom_name={"type":"string","value":"Меч"}} | ячейка 2: diamond_sword×1 {custom_name={"type":"string","value":"Меч"}} |
| +0 | свойство окна 0=1                                              | свойство окна 0=1                                              |
| | **▶ window_click {"slot":2,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":0}** |
| +0 | событие мира (0,0,0) 1030/0                                    | событие мира (0,0,0) 1030/0                                    |
| +0 | ячейка 0: пусто                                                | ячейка 0: пусто                                                |
| +0 | ячейка 2: пусто                                                | ячейка 2: пусто                                                |
| +0 | курсор: diamond_sword×1 {custom_name={"type":"string","value":"Меч"}} | курсор: diamond_sword×1 {custom_name={"type":"string","value":"Меч"}} |
| +0 | свойство окна 0=0                                              | свойство окна 0=0                                              |
| +0 | робот: Set Experience bar=0 level=29 total=0                   |   ← нет у нас                                                  |
| +0 |                                                                | робот: Set Experience bar=0 level=29 total=1288  ← лишнее      |
| +0 |                                                                | робот: Metadata 18=1288  ← лишнее                              |
| | **▶ window_click {"slot":30,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":30,"mode":0}** |
| +0 | ячейка 30: diamond_sword×1 {custom_name={"type":"string","value":"Меч"}} | ячейка 30: diamond_sword×1 {custom_name={"type":"string","value":"Меч"}} |
| +1 | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}] | робот: Attributes [{"key":"generic.attack_speed","value":4,"modifiers":[{"uuid":"minecraft:base_attack_speed","amount":-2.4000000953674316,"operation":0}]}]  ← у нас позже на 1 |
| | **▶ window_click {"slot":31,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":31,"mode":1}** |
| +0 | ячейка 0: iron_sword×1 {damage=200}                            | ячейка 0: iron_sword×1 {damage=200}                            |
| +0 | ячейка 2: iron_sword×1 {custom_name={"type":"string","value":"Меч"} damage=200} | ячейка 2: iron_sword×1 {custom_name={"type":"string","value":"Меч"} damage=200} |
| +0 | ячейка 31: пусто                                               | ячейка 31: пусто                                               |
| +0 | свойство окна 0=1                                              | свойство окна 0=1                                              |
| | **▶ rename ** (первый отклик через 0 / 0 такт.) | **▶ rename ** |
| +0 | ячейка 2: пусто                                                | ячейка 2: пусто                                                |
| +0 | свойство окна 0=0                                              | свойство окна 0=0                                              |
| | **▶ window_click {"slot":32,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":32,"mode":1}** |
| +0 | ячейка 1: iron_ingot×3                                         | ячейка 1: iron_ingot×3                                         |
| +0 | ячейка 2: iron_sword×1 {damage=14 repair_cost=1}               | ячейка 2: iron_sword×1 {damage=14 repair_cost=1}               |
| +0 | ячейка 32: пусто                                               | ячейка 32: пусто                                               |
| +0 | свойство окна 0=3                                              | свойство окна 0=3                                              |
| | **▶ window_click {"slot":2,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":1}** |
| +0 | событие мира (0,0,0) 1030/0                                    | событие мира (0,0,0) 1030/0                                    |
| +0 | ячейка 0: пусто                                                | ячейка 0: пусто                                                |
| +0 | ячейка 1: пусто                                                | ячейка 1: пусто                                                |
| +0 | ячейка 2: пусто                                                | ячейка 2: пусто                                                |
| +0 | ячейка 38: iron_sword×1 {damage=14 repair_cost=1}              | ячейка 38: iron_sword×1 {damage=14 repair_cost=1}              |
| +0 | свойство окна 0=0                                              | свойство окна 0=0                                              |
| +0 | робот: Set Experience bar=0 level=26 total=0                   |   ← нет у нас                                                  |
| +0 |                                                                | робот: Set Experience bar=0 level=26 total=997  ← лишнее       |
| +0 |                                                                | робот: Metadata 18=997  ← лишнее                               |
| | **▶ window_click {"slot":33,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":33,"mode":1}** |
| +0 | ячейка 0: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":4}]}} | ячейка 0: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":4}]}} |
| +0 | ячейка 33: пусто                                               | ячейка 33: пусто                                               |
| | **▶ window_click {"slot":34,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":34,"mode":1}** |
| +0 | ячейка 1: enchanted_book×1 {stored_enchantments={"enchantments":[{"id":33,"level":4}]}} | ячейка 1: enchanted_book×1 {stored_enchantments={"enchantments":[{"id":33,"level":4}]}} |
| +0 | ячейка 2: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":5}]} repair_cost=1} | ячейка 2: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":5}]} repair_cost=1} |
| +0 | ячейка 34: пусто                                               | ячейка 34: пусто                                               |
| +0 | свойство окна 0=5                                              | свойство окна 0=5                                              |
| | **▶ window_click {"slot":2,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":2,"mode":0}** |
| +0 | событие мира (0,0,0) 1030/0                                    | событие мира (0,0,0) 1030/0                                    |
| +0 | ячейка 0: пусто                                                | ячейка 0: пусто                                                |
| +0 | ячейка 1: пусто                                                | ячейка 1: пусто                                                |
| +0 | ячейка 2: пусто                                                | ячейка 2: пусто                                                |
| +0 | курсор: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":5}]} repair_cost=1} | курсор: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":5}]} repair_cost=1} |
| +0 | свойство окна 0=0                                              | свойство окна 0=0                                              |
| +0 | блок (0,0,0) chipped_anvil[facing=north]                       | блок (0,0,0) chipped_anvil[facing=north]  ← порядок в такте другой |
| +0 | робот: Set Experience bar=0 level=21 total=0                   |   ← нет у нас                                                  |
| +0 |                                                                | робот: Set Experience bar=0 level=21 total=612  ← лишнее       |
| +0 |                                                                | робот: Metadata 18=612  ← лишнее                               |
| | **▶ window_click {"slot":33,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":33,"mode":0}** |
| +0 | ячейка 33: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":5}]} repair_cost=1} | ячейка 33: diamond_sword×1 {enchantments={"enchantments":[{"id":33,"level":5}]} repair_cost=1} |
| | **▶ close** | **▶ close** |

Совпало: 45. Сдвинуто по тактам: 2. Нет у нас: 3. Лишнее у нас: 6.
