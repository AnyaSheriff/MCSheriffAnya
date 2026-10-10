# daylight-shade: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ cmd time set 6000** | **▶ cmd time set 6000** |
| | **▶ cmd time set 18000** (первый отклик через 19 / 19 такт.) | **▶ cmd time set 18000** |
| +0 | блок (0,0,0) daylight_detector[inverted=false,power=0] [пачкой] |   ← нет у нас                                                  |
| +0 | блок (0,0,4) daylight_detector[inverted=true,power=15] [пачкой] | блок (0,0,4) daylight_detector[inverted=true,power=15] [пачкой] |
| +0 |                                                                | блок (0,0,0) daylight_detector[inverted=false,power=4] [пачкой]  ← лишнее |
| | **▶ cmd time set 13000** (первый отклик через 19 / 19 такт.) | **▶ cmd time set 13000** |
| +0 | блок (0,0,4) daylight_detector[inverted=true,power=12]         | блок (0,0,4) daylight_detector[inverted=true,power=12] [пачкой] |
| +0 |                                                                | блок (0,0,0) daylight_detector[inverted=false,power=0] [пачкой]  ← лишнее |

Совпало: 2. Сдвинуто по тактам: 0. Нет у нас: 1. Лишнее у нас: 2.
