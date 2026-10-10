# observer-clock: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ set 1,0,0 minecraft:observer[facing=west,powered=false]** (первый отклик через 0 / 0 такт.) | **▶ set 1,0,0 minecraft:observer[facing=west,powered=false]** |
| +0 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false]               |
| +1 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |
| +1 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       |
| +1 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +3 | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      |
| +3 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +5 | блок (0,0,0) observer[facing=east,powered=true]                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +7 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← у нас раньше на 2 |
| +7 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +9 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← у нас раньше на 2 |
| +11 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +11 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← у нас раньше на 2 |
| +13 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       |
| +13 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← у нас раньше на 2 |
| +15 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      |
| +17 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +17 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +19 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← у нас раньше на 10 |
| +19 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +21 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← у нас раньше на 10 |
| +23 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +23 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← у нас раньше на 10 |
| +25 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       |
| +25 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← у нас раньше на 10 |
| +27 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      |
| +29 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +29 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +31 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← у нас раньше на 14 |
| +31 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +33 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← у нас раньше на 14 |
| +35 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +35 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← у нас раньше на 14 |
| +37 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       |
| +37 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← у нас раньше на 14 |
| +37 |                                                                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← лишнее |
| +39 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      |
| +39 |                                                                | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← лишнее |
| +41 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +41 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +41 |                                                                | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← лишнее |
| +43 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← у нас раньше на 22 |
| +43 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +43 |                                                                | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← лишнее |
| +45 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← у нас раньше на 22 |
| +45 |                                                                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← лишнее |
| +45 |                                                                | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← лишнее |
| +47 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +47 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← у нас раньше на 22 |
| +47 |                                                                | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← лишнее |
| +47 |                                                                | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← лишнее |
| +49 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       |
| +49 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← у нас раньше на 22 |
| +49 |                                                                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← лишнее |
| +51 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      |
| +51 |                                                                | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← лишнее |
| +53 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +53 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +53 |                                                                | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← лишнее |
| +55 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← у нас раньше на 26 |
| +55 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +55 |                                                                | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← лишнее |
| +57 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← у нас раньше на 26 |
| +57 |                                                                | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← лишнее |
| +57 |                                                                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← лишнее |
| +59 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +59 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← у нас раньше на 26 |
| +59 |                                                                | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← лишнее |
| +59 |                                                                | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← лишнее |
| +61 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       |
| +61 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← у нас раньше на 26 |
| +61 |                                                                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← лишнее |
| +63 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]      |
| +63 |                                                                | блок (0,0,0) observer[facing=east,powered=false] [пачкой]  ← лишнее |
| +65 | блок (-1,0,0) redstone_lamp[lit=true] [пачкой]                 |   ← нет у нас                                                  |
| +65 | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       | блок (0,0,0) observer[facing=east,powered=true] [пачкой]       |
| +65 |                                                                | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← лишнее |
| +67 | блок (1,0,0) observer[facing=west,powered=true] [пачкой]       | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← у нас раньше на 34 |
| +67 | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      | блок (0,0,0) observer[facing=east,powered=false] [пачкой]      |
| +67 |                                                                | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← лишнее |
| +69 | блок (1,0,0) observer[facing=west,powered=false]               | блок (1,0,0) observer[facing=west,powered=false] [пачкой]  ← у нас раньше на 34 |
| +69 |                                                                | блок (0,0,0) observer[facing=east,powered=true] [пачкой]  ← лишнее |
| +69 |                                                                | блок (1,0,0) observer[facing=west,powered=true] [пачкой]  ← лишнее |

Совпало: 28. Сдвинуто по тактам: 22. Нет у нас: 10. Лишнее у нас: 22.
