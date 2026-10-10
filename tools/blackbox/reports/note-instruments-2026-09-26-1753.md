# note-instruments: оригинал против нас

| такт | оригинал                                                       | у нас                                                          |
|------|----------------------------------------------------------------|----------------------------------------------------------------|
| | **** (первый отклик через 0 / 0 такт.) | **** |
| +0 | ▶ set 0,0,0 minecraft:stone                                    | ▶ set 0,0,0 minecraft:stone                                    |
| +4 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +4 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +4 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +4 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +4 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +4 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +4 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +4 | звук (0.5,1.5,0.5) block.note_block.basedrum 3.00 ~0.50        |   ← нет у нас                                                  |
| +4 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +8 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +8 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +8 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +8 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +8 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +8 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +8 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой]  ← у нас позже на 232 |
| +12 | ▶ set 0,0,0 minecraft:oak_planks                               | ▶ set 0,0,0 minecraft:oak_planks                               |
| +12 | блок (0,0,0) oak_planks [пачкой]                               | блок (0,0,0) oak_planks [пачкой]                               |
| +12 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] |
| +15 | звук (2.875,0,-5.625) entity.llama.step 0.15 ~1.00             |   ← нет у нас                                                  |
| +16 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +16 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +16 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +16 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +16 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +16 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +16 | блок (0,1,0) note_block[instrument=bass,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +16 | звук (0.5,1.5,0.5) block.note_block.bass 3.00 ~0.50            |   ← нет у нас                                                  |
| +16 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +20 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +20 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +20 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +20 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +20 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +20 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +20 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой]  ← у нас позже на 232 |
| +24 | ▶ set 0,0,0 minecraft:sand                                     | ▶ set 0,0,0 minecraft:sand                                     |
| +24 | блок (0,0,0) sand [пачкой]                                     | блок (0,0,0) sand [пачкой]                                     |
| +24 | блок (0,1,0) note_block[instrument=snare,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=snare,note=0,powered=false] [пачкой] |
| +28 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +28 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +28 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +28 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +28 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +28 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +28 | блок (0,1,0) note_block[instrument=snare,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +28 | звук (0.5,1.5,0.5) block.note_block.snare 3.00 ~0.50           |   ← нет у нас                                                  |
| +28 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +32 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +32 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +32 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +32 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +32 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +32 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +32 | блок (0,1,0) note_block[instrument=snare,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=snare,note=0,powered=false] [пачкой]  ← у нас позже на 256 |
| +36 | ▶ set 0,0,0 minecraft:glass                                    | ▶ set 0,0,0 minecraft:glass                                    |
| +36 | блок (0,0,0) glass [пачкой]                                    | блок (0,0,0) glass [пачкой]                                    |
| +36 | блок (0,1,0) note_block[instrument=hat,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=hat,note=0,powered=false] [пачкой] |
| +40 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +40 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +40 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +40 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +40 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +40 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +40 | блок (0,1,0) note_block[instrument=hat,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +40 | звук (0.5,1.5,0.5) block.note_block.hat 3.00 ~0.50             |   ← нет у нас                                                  |
| +40 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +44 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +44 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +44 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +44 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +44 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +44 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +44 | блок (0,1,0) note_block[instrument=hat,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=hat,note=0,powered=false] [пачкой]  ← у нас позже на 256 |
| +48 | ▶ set 0,0,0 minecraft:gold_block                               | ▶ set 0,0,0 minecraft:gold_block                               |
| +48 | блок (0,0,0) gold_block [пачкой]                               | блок (0,0,0) gold_block [пачкой]                               |
| +48 | блок (0,1,0) note_block[instrument=bell,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=bell,note=0,powered=false] [пачкой] |
| +52 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +52 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +52 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +52 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +52 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +52 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +52 | блок (0,1,0) note_block[instrument=bell,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +52 | звук (0.5,1.5,0.5) block.note_block.bell 3.00 ~0.50            |   ← нет у нас                                                  |
| +52 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +56 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +56 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +56 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +56 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +56 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +56 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +56 | блок (0,1,0) note_block[instrument=bell,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +60 | ▶ set 0,0,0 minecraft:clay                                     | ▶ set 0,0,0 minecraft:clay                                     |
| +60 | блок (0,0,0) clay [пачкой]                                     | блок (0,0,0) clay [пачкой]                                     |
| +60 | блок (0,1,0) note_block[instrument=flute,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=flute,note=0,powered=false] [пачкой] |
| +64 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +64 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +64 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +64 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +64 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +64 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +64 | блок (0,1,0) note_block[instrument=flute,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +64 | звук (0.5,1.5,0.5) block.note_block.flute 3.00 ~0.50           |   ← нет у нас                                                  |
| +64 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +66 | звук (1.125,0,-5.625) entity.llama.step 0.15 ~1.00             |   ← нет у нас                                                  |
| +68 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +68 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +68 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +68 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +68 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +68 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +68 | блок (0,1,0) note_block[instrument=flute,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +72 | ▶ set 0,0,0 minecraft:packed_ice                               | ▶ set 0,0,0 minecraft:packed_ice                               |
| +72 | блок (0,0,0) packed_ice [пачкой]                               | блок (0,0,0) packed_ice [пачкой]                               |
| +72 | блок (0,1,0) note_block[instrument=chime,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=chime,note=0,powered=false] [пачкой] |
| +76 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +76 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +76 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +76 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +76 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +76 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +76 | блок (0,1,0) note_block[instrument=chime,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +76 | звук (0.5,1.5,0.5) block.note_block.chime 3.00 ~0.50           |   ← нет у нас                                                  |
| +76 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +80 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +80 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +80 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +80 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +80 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +80 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +80 | блок (0,1,0) note_block[instrument=chime,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +84 | ▶ set 0,0,0 minecraft:white_wool                               | ▶ set 0,0,0 minecraft:white_wool                               |
| +84 | блок (0,0,0) white_wool [пачкой]                               | блок (0,0,0) white_wool [пачкой]                               |
| +84 | блок (0,1,0) note_block[instrument=guitar,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=guitar,note=0,powered=false] [пачкой] |
| +88 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +88 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +88 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +88 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +88 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +88 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +88 | блок (0,1,0) note_block[instrument=guitar,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +88 | звук (0.5,1.5,0.5) block.note_block.guitar 3.00 ~0.50          |   ← нет у нас                                                  |
| +88 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +92 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +92 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +92 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +92 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +92 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +92 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +92 | блок (0,1,0) note_block[instrument=guitar,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +96 | ▶ set 0,0,0 minecraft:bone_block                               | ▶ set 0,0,0 minecraft:bone_block                               |
| +96 | блок (0,0,0) bone_block[axis=y] [пачкой]                       | блок (0,0,0) bone_block[axis=y] [пачкой]                       |
| +96 | блок (0,1,0) note_block[instrument=xylophone,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=xylophone,note=0,powered=false] [пачкой] |
| +100 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +100 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +100 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +100 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +100 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +100 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +100 | блок (0,1,0) note_block[instrument=xylophone,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +100 | звук (0.5,1.5,0.5) block.note_block.xylophone 3.00 ~0.50       |   ← нет у нас                                                  |
| +100 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +104 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +104 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +104 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +104 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +104 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +104 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +104 | блок (0,1,0) note_block[instrument=xylophone,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +108 | ▶ set 0,0,0 minecraft:iron_block                               | ▶ set 0,0,0 minecraft:iron_block                               |
| +108 | блок (0,0,0) iron_block [пачкой]                               | блок (0,0,0) iron_block [пачкой]                               |
| +108 | блок (0,1,0) note_block[instrument=iron_xylophone,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=iron_xylophone,note=0,powered=false] [пачкой] |
| +112 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +112 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +112 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +112 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +112 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +112 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +112 | блок (0,1,0) note_block[instrument=iron_xylophone,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +112 | звук (0.5,1.5,0.5) block.note_block.iron_xylophone 3.00 ~0.50  |   ← нет у нас                                                  |
| +112 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +116 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +116 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +116 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +116 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +116 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +116 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +116 | блок (0,1,0) note_block[instrument=iron_xylophone,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +116 | звук (-0.5,0,-5.625) entity.llama.step 0.15 ~1.00              |   ← нет у нас                                                  |
| +120 | ▶ set 0,0,0 minecraft:soul_sand                                | ▶ set 0,0,0 minecraft:soul_sand                                |
| +120 | блок (0,0,0) soul_sand [пачкой]                                | блок (0,0,0) soul_sand [пачкой]                                |
| +120 | блок (0,1,0) note_block[instrument=cow_bell,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=cow_bell,note=0,powered=false] [пачкой] |
| +124 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +124 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +124 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +124 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +124 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +124 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +124 | блок (0,1,0) note_block[instrument=cow_bell,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +124 | звук (0.5,1.5,0.5) block.note_block.cow_bell 3.00 ~0.50        |   ← нет у нас                                                  |
| +124 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +128 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +128 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +128 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +128 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +128 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +128 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +128 | блок (0,1,0) note_block[instrument=cow_bell,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +132 | ▶ set 0,0,0 minecraft:pumpkin                                  | ▶ set 0,0,0 minecraft:pumpkin                                  |
| +132 | блок (0,0,0) pumpkin [пачкой]                                  | блок (0,0,0) pumpkin [пачкой]                                  |
| +132 | блок (0,1,0) note_block[instrument=didgeridoo,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=didgeridoo,note=0,powered=false] [пачкой] |
| +136 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +136 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +136 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +136 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +136 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +136 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +136 | блок (0,1,0) note_block[instrument=didgeridoo,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +136 | звук (0.5,1.5,0.5) block.note_block.didgeridoo 3.00 ~0.50      |   ← нет у нас                                                  |
| +136 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +140 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +140 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +140 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +140 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +140 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +140 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +140 | блок (0,1,0) note_block[instrument=didgeridoo,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +144 | ▶ set 0,0,0 minecraft:emerald_block                            | ▶ set 0,0,0 minecraft:emerald_block                            |
| +144 | блок (0,0,0) emerald_block [пачкой]                            | блок (0,0,0) emerald_block [пачкой]                            |
| +144 | блок (0,1,0) note_block[instrument=bit,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=bit,note=0,powered=false] [пачкой] |
| +148 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +148 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +148 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +148 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +148 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +148 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +148 | блок (0,1,0) note_block[instrument=bit,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +148 | звук (0.5,1.5,0.5) block.note_block.bit 3.00 ~0.50             |   ← нет у нас                                                  |
| +148 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +152 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +152 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +152 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +152 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +152 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +152 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +152 | блок (0,1,0) note_block[instrument=bit,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +156 | ▶ set 0,0,0 minecraft:hay_block                                | ▶ set 0,0,0 minecraft:hay_block                                |
| +156 | блок (0,0,0) hay_block[axis=y] [пачкой]                        | блок (0,0,0) hay_block[axis=y] [пачкой]                        |
| +156 | блок (0,1,0) note_block[instrument=banjo,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=banjo,note=0,powered=false] [пачкой] |
| +158 | звук (-1.875,0,-5.625) entity.llama.ambient 0.80 ~1.02         |   ← нет у нас                                                  |
| +160 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +160 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +160 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +160 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +160 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +160 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +160 | блок (0,1,0) note_block[instrument=banjo,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +160 | звук (0.5,1.5,0.5) block.note_block.banjo 3.00 ~0.50           |   ← нет у нас                                                  |
| +160 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +164 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +164 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +164 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +164 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +164 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +164 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +164 | блок (0,1,0) note_block[instrument=banjo,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +167 | звук (-2.125,0,-5.625) entity.llama.step 0.15 ~1.00            |   ← нет у нас                                                  |
| +168 | ▶ set 0,0,0 minecraft:glowstone                                | ▶ set 0,0,0 minecraft:glowstone                                |
| +168 | блок (0,0,0) glowstone [пачкой]                                | блок (0,0,0) glowstone [пачкой]                                |
| +168 | блок (0,1,0) note_block[instrument=pling,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=pling,note=0,powered=false] [пачкой] |
| +172 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +172 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +172 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +172 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +172 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +172 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +172 | блок (0,1,0) note_block[instrument=pling,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +172 | звук (0.5,1.5,0.5) block.note_block.pling 3.00 ~0.50           |   ← нет у нас                                                  |
| +172 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +176 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +176 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +176 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +176 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +176 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +176 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +176 | блок (0,1,0) note_block[instrument=pling,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +180 | ▶ set 0,0,0 minecraft:copper_block                             | ▶ set 0,0,0 minecraft:copper_block                             |
| +180 | блок (0,0,0) copper_block [пачкой]                             | блок (0,0,0) copper_block [пачкой]                             |
| +180 | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=false] [пачкой] |
| +184 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +184 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +184 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +184 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +184 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +184 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +184 | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +184 | звук (0.5,1.5,0.5) block.note_block.trumpet 3.00 ~0.50         |   ← нет у нас                                                  |
| +184 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +188 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +188 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +188 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +188 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +188 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +188 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +188 | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=false] [пачкой]  ← у нас позже на 160 |
| +192 | ▶ set 0,0,0 minecraft:exposed_copper                           | ▶ set 0,0,0 minecraft:exposed_copper                           |
| +192 | блок (0,0,0) exposed_copper [пачкой]                           | блок (0,0,0) exposed_copper [пачкой]                           |
| +192 | блок (0,1,0) note_block[instrument=trumpet_exposed,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=trumpet_exposed,note=0,powered=false] [пачкой] |
| +196 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +196 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +196 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +196 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +196 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +196 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +196 | блок (0,1,0) note_block[instrument=trumpet_exposed,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +196 | звук (0.5,1.5,0.5) block.note_block.trumpet_exposed 3.00 ~0.50 |   ← нет у нас                                                  |
| +196 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +200 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +200 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +200 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +200 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +200 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +200 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +200 | блок (0,1,0) note_block[instrument=trumpet_exposed,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +204 | ▶ set 0,0,0 minecraft:weathered_copper                         | ▶ set 0,0,0 minecraft:weathered_copper                         |
| +204 | блок (0,0,0) weathered_copper [пачкой]                         | блок (0,0,0) weathered_copper [пачкой]                         |
| +204 | блок (0,1,0) note_block[instrument=trumpet_weathered,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=trumpet_weathered,note=0,powered=false] [пачкой] |
| +208 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +208 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +208 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +208 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +208 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +208 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +208 | блок (0,1,0) note_block[instrument=trumpet_weathered,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +208 | звук (0.5,1.5,0.5) block.note_block.trumpet_weathered 3.00 ~0.50 |   ← нет у нас                                                  |
| +208 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +212 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +212 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +212 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +212 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +212 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +212 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +212 | блок (0,1,0) note_block[instrument=trumpet_weathered,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +216 | ▶ set 0,0,0 minecraft:oxidized_copper                          | ▶ set 0,0,0 minecraft:oxidized_copper                          |
| +216 | блок (0,0,0) oxidized_copper [пачкой]                          | блок (0,0,0) oxidized_copper [пачкой]                          |
| +216 | блок (0,1,0) note_block[instrument=trumpet_oxidized,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=trumpet_oxidized,note=0,powered=false] [пачкой] |
| +220 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +220 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +220 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +220 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +220 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +220 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +220 | блок (0,1,0) note_block[instrument=trumpet_oxidized,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +220 | звук (0.5,1.5,0.5) block.note_block.trumpet_oxidized 3.00 ~0.50 |   ← нет у нас                                                  |
| +220 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +224 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +224 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +224 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +224 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +224 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +224 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +224 | блок (0,1,0) note_block[instrument=trumpet_oxidized,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +228 | ▶ set 0,0,0 minecraft:dirt                                     | ▶ set 0,0,0 minecraft:dirt                                     |
| +228 | блок (0,0,0) dirt [пачкой]                                     | блок (0,0,0) dirt [пачкой]                                     |
| +228 | блок (0,1,0) note_block[instrument=harp,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=harp,note=0,powered=false] [пачкой] |
| +231 | звук (-2.75,0,-6.125) entity.llama.step 0.15 ~1.00             |   ← нет у нас                                                  |
| +232 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +232 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +232 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +232 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +232 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +232 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +232 | блок (0,1,0) note_block[instrument=harp,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +232 | звук (0.5,1.5,0.5) block.note_block.harp 3.00 ~0.50            |   ← нет у нас                                                  |
| +232 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +236 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +236 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +236 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +236 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +236 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +236 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +236 | блок (0,1,0) note_block[instrument=harp,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +240 | ▶ set 0,0,0 minecraft:obsidian                                 | ▶ set 0,0,0 minecraft:obsidian                                 |
| +240 | блок (0,0,0) obsidian [пачкой]                                 | блок (0,0,0) obsidian [пачкой]                                 |
| +240 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой]  ← у нас позже на 24 |
| +244 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +244 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +244 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +244 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +244 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +244 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +244 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +244 | звук (0.5,1.5,0.5) block.note_block.basedrum 3.00 ~0.50        |   ← нет у нас                                                  |
| +244 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +248 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +248 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +248 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +248 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +248 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +248 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +248 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой]  ← у нас позже на 64 |
| +252 | ▶ set 0,0,0 minecraft:oak_log                                  | ▶ set 0,0,0 minecraft:oak_log                                  |
| +252 | блок (0,0,0) oak_log[axis=y] [пачкой]                          | блок (0,0,0) oak_log[axis=y] [пачкой]                          |
| +252 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой]  ← у нас позже на 72 |
| +256 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +256 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +256 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +256 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +256 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +256 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +256 | блок (0,1,0) note_block[instrument=bass,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +256 | звук (0.5,1.5,0.5) block.note_block.bass 3.00 ~0.50            |   ← нет у нас                                                  |
| +256 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +260 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +260 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +260 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +260 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +260 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +260 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +260 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +264 | ▶ set 0,0,0 minecraft:stone_bricks                             | ▶ set 0,0,0 minecraft:stone_bricks                             |
| +264 | блок (0,0,0) stone_bricks [пачкой]                             | блок (0,0,0) stone_bricks [пачкой]                             |
| +264 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +268 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +268 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +268 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +268 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +268 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +268 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +268 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +268 | звук (0.5,1.5,0.5) block.note_block.basedrum 3.00 ~0.50        |   ← нет у нас                                                  |
| +268 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +272 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +272 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +272 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +272 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +272 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +272 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +272 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +276 | ▶ set 0,0,0 minecraft:deepslate_tile_slab                      | ▶ set 0,0,0 minecraft:deepslate_tile_slab                      |
| +276 | блок (0,0,0) deepslate_tile_slab[type=bottom,waterlogged=false] | блок (0,0,0) deepslate_tile_slab[type=bottom,waterlogged=false] |
| +280 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +280 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +280 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +280 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +280 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +280 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +280 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +280 | звук (0.5,1.5,0.5) block.note_block.basedrum 3.00 ~0.50        |   ← нет у нас                                                  |
| +280 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +281 | звук (-1.25,0,-6.5) entity.llama.step 0.15 ~1.00               |   ← нет у нас                                                  |
| +284 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +284 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +284 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +284 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +284 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +284 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +284 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +288 | ▶ set 0,0,0 minecraft:cyan_concrete_powder                     | ▶ set 0,0,0 minecraft:cyan_concrete_powder                     |
| +288 | блок (0,0,0) cyan_concrete_powder [пачкой]                     | блок (0,0,0) cyan_concrete_powder [пачкой]                     |
| +288 | блок (0,1,0) note_block[instrument=snare,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +292 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +292 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +292 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +292 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +292 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +292 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +292 | блок (0,1,0) note_block[instrument=snare,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +292 | звук (0.5,1.5,0.5) block.note_block.snare 3.00 ~0.50           |   ← нет у нас                                                  |
| +292 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +296 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +296 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +296 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +296 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +296 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +296 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +296 | блок (0,1,0) note_block[instrument=snare,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +300 | ▶ set 0,0,0 minecraft:blue_stained_glass                       | ▶ set 0,0,0 minecraft:blue_stained_glass                       |
| +300 | блок (0,0,0) blue_stained_glass [пачкой]                       | блок (0,0,0) blue_stained_glass [пачкой]                       |
| +300 | блок (0,1,0) note_block[instrument=hat,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +304 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +304 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +304 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +304 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +304 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +304 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +304 | блок (0,1,0) note_block[instrument=hat,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +304 | звук (0.5,1.5,0.5) block.note_block.hat 3.00 ~0.50             |   ← нет у нас                                                  |
| +304 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +308 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +308 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +308 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +308 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +308 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +308 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +308 | блок (0,1,0) note_block[instrument=hat,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +312 | ▶ set 0,0,0 minecraft:iron_ore                                 | ▶ set 0,0,0 minecraft:iron_ore                                 |
| +312 | блок (0,0,0) iron_ore [пачкой]                                 | блок (0,0,0) iron_ore [пачкой]                                 |
| +312 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +316 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +316 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +316 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +316 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +316 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +316 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +316 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +316 | звук (0.5,1.5,0.5) block.note_block.basedrum 3.00 ~0.50        |   ← нет у нас                                                  |
| +316 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +320 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +320 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +320 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +320 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +320 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +320 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +320 | блок (0,1,0) note_block[instrument=basedrum,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +324 | ▶ set 0,0,0 minecraft:bookshelf                                | ▶ set 0,0,0 minecraft:bookshelf                                |
| +324 | блок (0,0,0) bookshelf [пачкой]                                | блок (0,0,0) bookshelf [пачкой]                                |
| +324 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +328 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +328 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +328 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +328 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +328 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +328 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +328 | блок (0,1,0) note_block[instrument=bass,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +328 | звук (0.5,1.5,0.5) block.note_block.bass 3.00 ~0.50            |   ← нет у нас                                                  |
| +328 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +332 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +332 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +332 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +332 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +332 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +332 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +332 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +332 | звук (0.375,0,-6.75) entity.llama.step 0.15 ~1.00              |   ← нет у нас                                                  |
| +336 | ▶ set 0,0,0 minecraft:cherry_hanging_sign                      | ▶ set 0,0,0 minecraft:cherry_hanging_sign                      |
| +336 | блок (0,0,0) cherry_hanging_sign[attached=false,rotation=8,waterlogged=false] | блок (0,0,0) cherry_hanging_sign[attached=false,rotation=8,waterlogged=false] |
| +336 | данные блока (0,0,0) 8 {"type":"compound","value":{"back_text":{"type":"compound","value":{"has_glowing_text":{"type":"byte","value":0},"color":{"type":"string","value":"black"},"messages":{"type":"list","value":{"type":"st |   ← нет у нас                                                  |
| +340 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +340 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +340 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +340 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +340 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +340 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +340 | блок (0,1,0) note_block[instrument=bass,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +340 | звук (0.5,1.5,0.5) block.note_block.bass 3.00 ~0.50            |   ← нет у нас                                                  |
| +340 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +344 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +344 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +344 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +344 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +344 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +344 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +344 | блок (0,1,0) note_block[instrument=bass,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +348 | ▶ set 0,0,0 minecraft:waxed_cut_copper_stairs                  | ▶ set 0,0,0 minecraft:waxed_cut_copper_stairs                  |
| +348 | блок (0,0,0) waxed_cut_copper_stairs[facing=north,half=bottom,shape=straight,waterlogged=false] [пачкой] | блок (0,0,0) waxed_cut_copper_stairs[facing=north,half=bottom,shape=straight,waterlogged=false] [пачкой] |
| +348 | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |
| +352 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +352 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.60                |   ← нет у нас                                                  |
| +352 | блок (2,1,0) lever[face=floor,facing=north,powered=true]       |   ← нет у нас                                                  |
| +352 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +352 | блок (1,1,0) redstone_wire[east=side,north=none,power=15,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +352 | блок (2,1,0) lever[face=floor,facing=north,powered=true] [пачкой] |   ← нет у нас                                                  |
| +352 | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=true] [пачкой] |   ← нет у нас                                                  |
| +352 | звук (0.5,1.5,0.5) block.note_block.trumpet 3.00 ~0.50         |   ← нет у нас                                                  |
| +352 | действие (0,1,0) note_block 0/0                                |   ← нет у нас                                                  |
| +356 | ▶ click 2,1,0                                                  | ▶ click 2,1,0                                                  |
| +356 | звук (2.5,1.5,0.5) block.lever.click 0.30 ~0.50                |   ← нет у нас                                                  |
| +356 | блок (2,1,0) lever[face=floor,facing=north,powered=false]      | блок (2,1,0) lever[face=floor,facing=north,powered=false]      |
| +356 | блок (2,2,0) air                                               |   ← нет у нас                                                  |
| +356 | блок (1,1,0) redstone_wire[east=side,north=none,power=0,south=none,west=side] [пачкой] |   ← нет у нас                                                  |
| +356 | блок (2,1,0) lever[face=floor,facing=north,powered=false] [пачкой] | блок (2,1,0) lever[face=floor,facing=north,powered=false]  ← у нас раньше на 4 |
| +356 | блок (0,1,0) note_block[instrument=trumpet,note=0,powered=false] [пачкой] |   ← нет у нас                                                  |

Совпало: 168. Сдвинуто по тактам: 38. Нет у нас: 369. Лишнее у нас: 0.
