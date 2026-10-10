# bed-sleep-night: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **▶ click 3,0,3** (первый отклик через 0 / 0 такт.) | **▶ click 3,0,3** |
| +0 | робот: System Chat {"translate":"block.minecraft.set_spawn"} overlay=false | робот: System Chat {"translate":"block.minecraft.set_spawn"} overlay=false |
| +0 | робот: System Chat {"translate":"sleep.skipping_night"} overlay=true | робот: System Chat {"translate":"sleep.skipping_night"} overlay=true |
| +0 | робот: Animate anim=0                                          | робот: Animate anim=0                                          |
| +0 | робот: Animate anim=0                                          | робот: Animate anim=0                                          |
| +0 | блок (3,0,3) red_bed[facing=south,occupied=true,part=foot]     | блок (3,0,3) red_bed[facing=south,occupied=true,part=foot]     |
| +0 | блок (3,1,3) air                                               | блок (3,1,3) air                                               |
| +0 | блок (3,0,4) red_bed[facing=south,occupied=true,part=head] [пачкой] | блок (3,0,4) red_bed[facing=south,occupied=true,part=head] [пачкой] |
| +0 | блок (3,0,3) red_bed[facing=south,occupied=true,part=foot] [пачкой] | блок (3,0,3) red_bed[facing=south,occupied=true,part=foot] [пачкой] |
| +0 | данные блока (3,0,4) 25 {}                                     | данные блока (3,0,4) 25 {}                                     |
| +0 | данные блока (3,0,3) 25 {}                                     | данные блока (3,0,3) 25 {}                                     |
| +0 | робот: Metadata 6=2 14={"x":103,"z":104,"y":-59}               | робот: Metadata 6=2 14={"x":103,"z":104,"y":-59}               |
| +1 | робот: сверка места (3.5,0.688,4.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false} | робот: сверка места (3.5,0.688,4.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false}  ← случайное, в допуске: сверка места спящего |
| +100 | робот: Time clock=0 day=0 rate=1                               | робот: Time clock=0 day=0 rate=1                               |
| +100 | робот: Animate anim=2                                          | робот: Animate anim=2                                          |
| +100 | робот: сверка места (2.5,0,4.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false} | робот: сверка места (2.5,0,4.5) {"_value":0,"x":false,"y":false,"z":false,"yaw":false,"pitch":false,"dx":false,"dy":false,"dz":false,"yawDelta":false} |
| +100 | блок (3,0,4) red_bed[facing=south,occupied=false,part=head] [пачкой] | блок (3,0,4) red_bed[facing=south,occupied=false,part=head] [пачкой] |
| +100 | блок (3,0,3) red_bed[facing=south,occupied=false,part=foot] [пачкой] | блок (3,0,3) red_bed[facing=south,occupied=false,part=foot] [пачкой] |
| +100 | данные блока (3,0,4) 25 {}                                     | данные блока (3,0,4) 25 {}                                     |
| +100 | данные блока (3,0,3) 25 {}                                     | данные блока (3,0,3) 25 {}                                     |
| +100 | робот: Metadata 6=0 14=undefined                               | робот: Metadata 6=0 14=undefined                               |

Случайное, в допуске: 1 (входит в «Совпало»): сверка места спящего — 1.
Совпало: 20. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.
