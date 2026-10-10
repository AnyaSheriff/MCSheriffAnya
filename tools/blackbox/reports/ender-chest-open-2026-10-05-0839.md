# ender-chest-open: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 0,0,0** (первый отклик через 0 / 0 такт.) | **▶ click 0,0,0** |
| +0 | звук (0.5,0.5,0.5) block.ender_chest.open 0.50 ~0.91           | звук (0.5,0.5,0.5) block.ender_chest.open 0.50 ~0.99           |
| +0 | окно 2                                                         | окно 2                                                         |
| +0 | содержимое окна 54:stone×20 55:dirt×5                          |   ← нет у нас                                                  |
| +0 | блок (0,0,0) ender_chest[facing=north,waterlogged=false]       | блок (0,0,0) ender_chest[facing=north,waterlogged=false]       |
| +0 | блок (0,1,0) air                                               | блок (0,1,0) air                                               |
| +0 | действие (0,0,0) ender_chest 1/1                               | действие (0,0,0) ender_chest 1/1                               |
| +0 |                                                                | содержимое окна 3:dirt×5 54:stone×20 55:dirt×5  ← лишнее       |
| +4 | действие (0,0,0) ender_chest 1/1                               | действие (0,0,0) ender_chest 1/1                               |
| +9 | действие (0,0,0) ender_chest 1/1                               | действие (0,0,0) ender_chest 1/1                               |
| | **▶ window_click {"slot":54,"button":0,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":54,"button":0,"mode":1}** |
| +0 | ячейка 0: stone×20                                             | ячейка 0: stone×20                                             |
| +0 | ячейка 54: пусто                                               | ячейка 54: пусто                                               |
| | **▶ window_click {"slot":55,"button":0,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":55,"button":0,"mode":1}** |
| +0 | ячейка 1: dirt×5                                               |   ← нет у нас                                                  |
| +0 | ячейка 55: пусто                                               | ячейка 55: пусто                                               |
| +0 | действие (0,0,0) ender_chest 1/1                               | действие (0,0,0) ender_chest 1/1  ← порядок в такте другой     |
| +0 |                                                                | ячейка 3: dirt×10  ← лишнее                                    |
| +5 | действие (0,0,0) ender_chest 1/1                               | действие (0,0,0) ender_chest 1/1                               |
| | **▶ close** (первый отклик через 0 / 0 такт.) | **▶ close** |
| +0 | звук (0.5,0.5,0.5) block.ender_chest.close 0.50 ~0.94          | звук (0.5,0.5,0.5) block.ender_chest.close 0.50 ~0.96          |
| +0 | действие (0,0,0) ender_chest 1/0                               | действие (0,0,0) ender_chest 1/0                               |
| +4 | действие (0,0,0) ender_chest 1/0                               | действие (0,0,0) ender_chest 1/0                               |
| | **▶ click 6,0,0** (первый отклик через 5 / 5 такт.) | **▶ click 6,0,0** |
| +0 | блок (6,0,0) ender_chest[facing=north,waterlogged=false]       | блок (6,0,0) ender_chest[facing=north,waterlogged=false]       |
| +0 | блок (6,1,0) stone                                             | блок (6,1,0) stone                                             |
| | **▶ click 3,0,0** (первый отклик через 5 / 5 такт.) | **▶ click 3,0,0** |
| +0 | звук (3.5,0.5,0.5) block.ender_chest.open 0.50 ~0.92           | звук (3.5,0.5,0.5) block.ender_chest.open 0.50 ~0.94           |
| +0 | окно 2                                                         | окно 2                                                         |
| +0 | содержимое окна 0:stone×20 1:dirt×5                            |   ← нет у нас                                                  |
| +0 | блок (3,0,0) ender_chest[facing=north,waterlogged=false]       | блок (3,0,0) ender_chest[facing=north,waterlogged=false]       |
| +0 | блок (3,1,0) air                                               | блок (3,1,0) air                                               |
| +0 | действие (3,0,0) ender_chest 1/1                               | действие (3,0,0) ender_chest 1/1                               |
| +0 |                                                                | содержимое окна 0:stone×20 3:dirt×10  ← лишнее                 |
| +4 | действие (3,0,0) ender_chest 1/1                               | действие (3,0,0) ender_chest 1/1                               |
| | **▶ window_click {"slot":0,"button":0,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":0,"button":0,"mode":0}** |
| +0 | ячейка 0: пусто                                                | ячейка 0: пусто                                                |
| +0 | курсор: stone×20                                               | курсор: stone×20                                               |
| | **▶ window_click {"slot":3,"button":0,"mode":0}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":3,"button":0,"mode":0}** |
| +0 | ячейка 3: stone×20                                             | ячейка 3: stone×20                                             |
| +0 | действие (3,0,0) ender_chest 1/1                               | действие (3,0,0) ender_chest 1/1  ← порядок в такте другой     |
| +0 |                                                                | курсор: dirt×10  ← лишнее                                      |
| | **▶ window_click {"slot":1,"button":0,"mode":1}** (первый отклик через 0 / 0 такт.) | **▶ window_click {"slot":1,"button":0,"mode":1}** |
| +0 | ячейка 1: пусто                                                |   ← нет у нас                                                  |
| +0 | ячейка 62: dirt×5                                              |   ← нет у нас                                                  |
| +0 |                                                                | курсор: dirt×10  ← лишнее                                      |
| +1 | действие (3,0,0) ender_chest 1/1                               | действие (3,0,0) ender_chest 1/1                               |
| | **▶ close** (первый отклик через 0 / 0 такт.) | **▶ close** |
| +0 | звук (3.5,0.5,0.5) block.ender_chest.close 0.50 ~0.98          | звук (3.5,0.5,0.5) block.ender_chest.close 0.50 ~0.96          |
| +0 | действие (3,0,0) ender_chest 1/0                               | действие (3,0,0) ender_chest 1/0                               |

Совпало: 30. Сдвинуто по тактам: 0. Нет у нас: 5. Лишнее у нас: 5.
