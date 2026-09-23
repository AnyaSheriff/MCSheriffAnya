# Фичи, растительность и формат Anvil: сводка исследования

Собрано подагентом-исследователем (модель Haiku) 23.09.2026 по minecraft.wiki;
код игры и ядер не читался. **Черновик**: перед кодом каждую цифру сверять
со ссылкой.

## Пометки при разборе

- **Противоречие про `blending_data`.** Этот отчёт называет поля
  `height_profile` и корневой флаг `old_noise`, соседний (worldgen-terrain.md) —
  `heights`, `min_section`, `max_section`. Скорее всего, в разных версиях поля
  звались по-разному; ссылки здесь ведут на wiki.vg и вики «discontinued
  features», то есть на старое. Решать только по
  [Chunk format](https://minecraft.wiki/w/Chunk_format) для 26.1.2.
- **Руды.** Таблица упрощена: у железа, например, на вики несколько
  размещений с разными высотами и распределениями, здесь сведено в одну
  строку. «Ore vein» (70 % камня, 2 % сырой руды) — это большие жилы меди
  и железа, а не обычные гнёзда руды; не путать.
- Упомянут gist «Telepathic Grunt» — это обзор сообщества; если там код,
  его не использовать.
- Сжатие «4 — LZ4» в region-файле проверить: у нас это определяет, какие
  миры мы сможем прочитать.

## Что берём в первую очередь

1. **Порядок стадий декорации** (одиннадцать): raw_generation → lakes →
   local_modifications → underground_structures → surface_structures →
   strongholds → underground_ores → underground_decoration → fluid_springs →
   vegetal_decoration → top_layer_modification. Внутри стадии сперва куски
   строений, потом фичи; фича трогает не дальше 3×3 чанков от своего.
2. **Руды по вики** — размер жилы, попыток на чанк, высоты, распределение
   (равномерное / треугольное) и доля жил, отменяемых при касании воздуха.
   Наши нынешние доли подобраны на глаз — перейти на эти параметры.
3. **Формат region-файла**: сектора по 4 КиБ; сектор 0 — таблица мест
   (3 байта смещения + 1 байт длины на чанк), сектор 1 — время изменения;
   у чанка длина, тип сжатия (2 — zlib), затем NBT. Слишком большие чанки —
   в отдельных `.mcc`.
4. **NBT чанка**: DataVersion, xPos/zPos/yPos, Status, sections
   (block_states и biomes — палитра + упакованные long), Heightmaps,
   block_entities, block_ticks/fluid_ticks, structures (starts/References),
   PostProcessing, blending_data.

---

## Отчёт исследователя

### Стадии чанка
empty → structure_starts → structure_references → biomes → noise → surface →
carvers → (liquid_carvers — под вопросом) → features → initialize_light →
light → spawn → full ([World generation](https://minecraft.wiki/w/World_generation)).

### Placement modifiers
biome, block_predicate_filter, count, count_on_every_layer, cuboid,
environment_scan, fixed_placement, height_range, heightmap, in_square
(сдвиг 0–15 по X/Z), noise_based_count, noise_threshold_count, offset,
random_chance, randomly_selected, rarity_filter,
surface_relative_threshold_filter, surface_water_depth_filter
([Placed feature](https://minecraft.wiki/w/Placed_feature)).

### Деревья
- Дуб: обычный, большой ветвистый, болотный с лозами; саженец 5–7 блоков
  ([Oak](https://minecraft.wiki/w/Oak)).
- Берёза: обычная 5–7, высокая до 13 в древнем березняке
  ([Birch](https://minecraft.wiki/w/Birch)).
- Ель: обычная 1×1, мега 2×2, сосна; до 31+ блока
  ([Spruce](https://minecraft.wiki/w/Spruce)).
- Ещё: тропическое, акация, тёмный дуб, азалия, мангр, вишня
  ([Tree](https://minecraft.wiki/w/Tree)).

### Озёра, источники, прочее
- Лавовые озёра — редко на поверхности, часто под землёй
  ([Lava Lake](https://minecraft.wiki/w/Lava_Lake)).
- Источники — одиночный блок жидкости в стене пещеры или утёса
  ([Spring](https://minecraft.wiki/w/Spring)).
- Айсберги в замёрзших океанах: плотный лёд 1/16 на чанк, синий 1/200
  ([Iceberg](https://minecraft.wiki/w/Iceberg_(feature))).
- Ледяные шипы: короткие ~15, высокие 50+ блоков
  ([Ice Spikes](https://minecraft.wiki/w/Ice_Spikes)).
- Кораллы в тёплом океане: дерево, клешня, гриб
  ([Coral Reef](https://minecraft.wiki/w/Coral_Reef)).
- Морская трава — во всех незамёрзших океанах; ламинария — 1/18 на чанк,
  кроме замёрзших и тёплого ([Seagrass](https://minecraft.wiki/w/Seagrass),
  [Kelp](https://minecraft.wiki/w/Kelp_(feature))).
- Аметистовые жеоды: Y −58…30, 1/24 на чанк, слои базальт → кальцит →
  аметист, трещина в 95 % ([Amethyst Geode](https://minecraft.wiki/w/Amethyst_Geode)).
- Валуны из замшелого булыжника в древней тайге
  ([Forest rock](https://minecraft.wiki/w/Forest_rock)).
- Огромные грибы: болото, цветочный и тёмный лес, грибные поля
  ([Huge mushroom](https://minecraft.wiki/w/Huge_mushroom)).

### Руды верхнего мира (упрощено, см. пометку)

| что | размер | попыток на чанк | высоты | распределение | отказ у воздуха |
|---|---|---|---|---|---|
| земля | 33 | 7 | 0…160 | равномерное | 0 |
| гравий | 33 | 14 | −64…320 | равномерное | 0 |
| гранит, диорит, андезит | 64 | 2 | 0…60 | равномерное | 0 |
| туф | 64 | 2 | −64…0 | равномерное | 0 |
| уголь | 17 | 20 | 0…192 | треугольное | 0,5 |
| железо | 4–9 | 10 | −64…72 | равномерное | 0 |
| медь | 10 | 16 | −16…112 | треугольное | 0 |
| редстоун | 8 | 4 | −64…15 | равномерное | 0 |
| лазурит | 7 | 2 | −32…32 | треугольное | 0 |
| золото | 9 | 4 | −64…32 | треугольное | 0,5 |
| алмаз | 4–8 | 7 | −64…16 | треугольное | 0,5 |
| изумруд | 3 | 100 | −16…480 | треугольное | 0 |
| заражённый камень | 9 | 14 | −64…63 | равномерное | 0 |

([Ore (feature)](https://minecraft.wiki/w/Ore_(feature)),
[Ore vein](https://minecraft.wiki/w/Ore_vein))

### Region-файл
32×32 чанка; сектора по 4 КиБ; сектор 0 — места (3 байта смещения +
1 байт длины в секторах), сектор 1 — время изменения; чанк: длина (4 байта),
тип сжатия (1 — gzip, 2 — zlib, 3 — без сжатия, 4 — LZ4, 127 — своё), NBT;
больше 1020 КиБ — в `.mcc` ([Region file format](https://minecraft.wiki/w/Region_file_format)).

### NBT чанка
DataVersion, xPos, zPos, yPos, Status (`minecraft:empty` … `minecraft:full`),
LastUpdate, InhabitedTime, sections (Y, block_states{palette, data},
biomes{palette, data}, BlockLight, SkyLight), block_entities, block_ticks,
fluid_ticks, Heightmaps (MOTION_BLOCKING, MOTION_BLOCKING_NO_LEAVES,
OCEAN_FLOOR, WORLD_SURFACE; по 9 бит на значение), structures (starts,
References), CarvingMasks, blending_data, PostProcessing
([Chunk format](https://minecraft.wiki/w/Chunk_format)).

### Нужно снять чёрным ящиком
- связь `size` у руды с числом блоков в жиле;
- где именно по краю чанка лежат 16 высот сшивания.
