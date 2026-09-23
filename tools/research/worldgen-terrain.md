# Рельеф, шумы и сшивание: сводка исследования

Собрано подагентом-исследователем (модель Haiku) 23.09.2026 по minecraft.wiki
и официальным материалам; код игры и ядер не читался. **Это черновик**:
перед тем как строить на нём код, каждую цифру сверять со ссылкой.

## Пометки при разборе

- Часть ссылок ведёт на `minecraft.fandom.com` — это старое зеркало вики,
  оно отстаёт от `minecraft.wiki`. Всё, что взято оттуда (особенно про
  сшивание в 1.18), перепроверить на `minecraft.wiki/w/Java_Edition_1.18`.
- Про аквиферы исследователь сам себе противоречит («страница не
  опубликована» и тут же ссылка на неё). Перепроверить.
- «Лавовые озёра ниже Y=-10» и «лава ниже Y=-54 всегда» — разные вещи;
  у нас пока реализовано второе.

## Что важно для нас в первую очередь

1. **Сплайны с производными в узлах** (кубические Эрмита), а не ломаная:
   у нас сейчас ломаная — отсюда изломы рельефа на узлах. Узел =
   `location`, `value` (число или вложенный сплайн), `derivative`.
2. **Высота поверхности** на вики выражена через offset и factor:
   `preliminary_surface_level ≈ (offset + 0.5 - 0.2734375 / factor) * 128`
   (страница Tutorial: Custom world generation). Jaggedness — отдельный
   сплайн для пиков.
3. **Сшивание**: в чанке хранится `blending_data` — `heights` (16 чисел,
   высоты по краю чанка), `min_section`, `max_section`. Подмешиваются
   высоты, биомы и плотность. Расстояние затухания и сама функция на вики
   не названы — снимать чёрным ящиком.
4. **Поверхность** — дерево правил (`sequence` / `condition` / `block`,
   условия `biome`, `stone_depth`, `water`, `vertical_gradient`,
   `temperature`).
5. **Стадии чанка**: structure_starts → structure_references → biomes →
   noise → surface → carvers → features → initialize_light → light →
   spawn → full.

---

## Отчёт исследователя (без правок по существу)

### 1. Шумы и плотность рельефа
- Noise router — набор density functions, считающих значение в каждой точке
  (x, y, z); из него берутся рельеф, биомы, аквиферы, жилы руд
  ([Noise router](https://minecraft.wiki/w/Noise_router)).
- Положительная плотность — блок, отрицательная — воздух или жидкость
  (`final_density`) ([Noise settings](https://minecraft.wiki/w/Noise_settings)).
- Continentalness, erosion, peaks & valleys (из weirdness), temperature,
  humidity, depth ([World generation](https://minecraft.wiki/w/World_generation)).
- Density functions вкладываются друг в друга; группы: маркеры (cache,
  interpolated), выборка шума, арифметика, отображения (gradient, lerp,
  squeeze, spline), условия ([Density function](https://minecraft.wiki/w/Density_function)).
- Интерполяция: `interpolated` с `cell_size_xz` / `cell_size_y`; числа для
  верхнего мира в открытых источниках не найдены.

### 2. Сплайны
- Кубический сплайн: `coordinate` + `points` (`location`, `value` — число
  или вложенный сплайн, `derivative`) ([Density function](https://minecraft.wiki/w/Density_function)).
- Offset — сплайн по continentalness, erosion, weirdness, PV; jaggedness —
  сплайн пиков: больше при высокой континентальности, низкой эрозии,
  высоком PV и отрицательной странности ([World generation](https://minecraft.wiki/w/World_generation)).
- `preliminary_surface_level ≈ (offset + 0.5 - 0.2734375/factor) * 128`
  ([Tutorial: Custom world generation](https://minecraft.wiki/w/Tutorial:Custom_world_generation)).
- Узлы и производные стандартных сплайнов в открытых источниках не найдены.

### 3. Аквиферы
- Решают, какая жидкость заполняет каждое пустое место: подземные озёра,
  водопады и лавопады в пещерах, каменные «ободки» между жидкостями
  ([Cave](https://minecraft.wiki/w/Cave)).
- Ячейки 16×12×16 с одним случайным центром; «давление» между центрами
  добавляется к плотности и даёт перемычки; параметры router: `barrier`,
  `fluid_level_floodedness` (пороги −0,3 и 0,8), `fluid_level_spread`
  (уровни около Y = −20, 20, 60, 100), `lava` ([Aquifer](https://minecraft.wiki/w/Aquifer)).
- Под морским дном ячейки почти всегда затоплены, в глубине суши — сухие.
- Ниже Y=−54 — лава с поверхностью на −54 ([Cave](https://minecraft.wiki/w/Cave)).

### 4. Пещеры
- Шумовые: cheese (крупные полости), spaghetti (длинные извилистые ходы),
  noodle (1–5 блоков в ширину) ([Cave](https://minecraft.wiki/w/Cave)).
- Carver-пещеры: главная комната-эллипсоид с 1–4 стволами и ветвями;
  в верхнем мире Y=−56…180 ([Cave](https://minecraft.wiki/w/Cave)).
- Каньоны — отдельный carver ([Canyon](https://minecraft.wiki/w/Canyon)).
- Параметры шумов пещер в открытых источниках не найдены.

### 5. Поверхность
- Дерево правил: `block`, `condition`, `sequence`; условия `biome`,
  `stone_depth`, `water`, `vertical_gradient`, `temperature`
  ([Surface rule](https://minecraft.wiki/w/Surface_rule)).
- Как именно рисуются ступени терракоты и растрёпанные границы — не найдено.

### 6. Сшивание (blending)
- Применяется к чанкам старше 1.18 при открытии в 1.18+; гасит разрывы
  рельефа, биомов и высот на границе ([Java Edition 1.18, Fandom](https://minecraft.fandom.com/wiki/Java_Edition_1.18) —
  перепроверить на minecraft.wiki).
- `blending_data`: `heights` (16 double — высоты по краю чанка),
  `min_section`, `max_section` ([Chunk format](https://minecraft.wiki/w/Chunk_format)).
- Старый слой бедрока Y=0…4 заменяется глубинным сланцем, новый бедрок —
  Y=−64…−60; новый рельеф снизу достраивается только под непустыми столбцами.
- Расстояние затухания и функция перехода — не найдены; предложение
  исследователя: снять чёрным ящиком (старый мир → новая версия, замер
  высот у границы).

### 7. Стадии чанка
EMPTY → STRUCTURE_STARTS → STRUCTURE_REFERENCES → BIOMES → NOISE → SURFACE →
CARVERS → FEATURES → INITIALIZE_LIGHT → LIGHT → SPAWN → FULL
([World generation](https://minecraft.wiki/w/World_generation)).
Неполные чанки — proto-chunks, готовые — level chunks.
