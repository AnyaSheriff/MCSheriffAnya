# daylight-shade: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd time set 6000** | **▶ cmd time set 6000** |
| | **▶ cmd time set 18000** (первый отклик через 20 / 6 такт.) | **▶ cmd time set 18000** |
| +0 | блок (0,0,0) daylight_detector[inverted=false,power=0] [пачкой] |   ← нет у нас                                                  |
| +0 | блок (0,0,4) daylight_detector[inverted=true,power=13] [пачкой] |   ← нет у нас                                                  |
| +0 |                                                                | блок (0,0,0) daylight_detector[inverted=false,power=9]  ← лишнее |
| | **▶ cmd time set 13000** (первый отклик через 20 / 6 такт.) | **▶ cmd time set 13000** |
| +0 | блок (0,0,0) daylight_detector[inverted=false,power=2] [пачкой] |   ← нет у нас                                                  |
| +0 | блок (0,0,4) daylight_detector[inverted=true,power=9] [пачкой] |   ← нет у нас                                                  |
| +0 |                                                                | блок (0,0,0) daylight_detector[inverted=false,power=0]  ← лишнее |

Совпало: 0. Сдвинуто по тактам: 0. Нет у нас: 4. Лишнее у нас: 2.
