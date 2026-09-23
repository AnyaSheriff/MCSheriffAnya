# Строения и подземелья: сводка исследования

Собрано подагентом-исследователем (модель Haiku) 23.09.2026 по minecraft.wiki;
код игры и ядер не читался. **Черновик**: перед кодом каждую цифру сверять
со ссылкой.

## Пометки при разборе

- **Комната-данж.** Отчёт путается: «7×9 задокументирован, 5×5, 5×7, 7×7
  отсутствуют». Похоже на неверное прочтение таблицы вариантов на
  [Monster Room](https://minecraft.wiki/w/Monster_Room) — перечитать самому.
- **Аванпост «как деревни»** — сомнительно: у аванпоста свой набор
  размещения со своими числами. Проверить на
  [Structure set](https://minecraft.wiki/w/Structure_set).
- В таблице размещения нет океанских руин, хижины ведьмы, разрушенного
  портала числами — дополнить по той же странице.
- Кольца крепостей (3, 6, 10, 15, 21, 28, 36, 9 = 128; первое кольцо
  1280–2816 блоков) — сходится с тем, что обычно приводит вики.

## Что берём в первую очередь

1. **Размещение по семени** (random spread): spacing, separation (строго
   меньше spacing), salt, вид разброса (linear / triangular); наибольшее
   расстояние 2×spacing − separation. Крепости — concentric rings.
   Зоны исключения (exclusion zone) — «не ближе N чанков к строениям
   другого набора».
2. **Jigsaw**: пул шаблонов с весами, jigsaw-блоки как разъёмы, глубина
   сборки `size` до 20; `rigid` для зданий, `terrain_matching` для дорог.
3. **Подгонка земли**: none, beard_thin (деревни, аванпосты), beard_box
   (древний город), bury (крепость), encapsulate (trial chambers).
4. **Данж — это фича, а не строение**: ставится по своим правилам, не
   через набор строений.
5. **В чанке Anvil**: `structures.References` — чанки, где начинаются
   строения (X в младших 32 битах, Z в старших), `structures.starts` —
   начатые строения с кусками (BB + свои данные); достроенные помечаются
   `INVALID`.

---

## Отчёт исследователя

### Размещение
- Random spread: spacing и separation 0–4096, separation < spacing, salt,
  spread `linear`/`triangular`; макс. расстояние 2×spacing − separation.
- Concentric rings (крепости): distance в единицах 6 чанков, count 1–4095.
- Frequency reduction method: `default`, `legacy_type_1/2/3`;
  frequency 0…1; exclusion zone: chunk_count 1–16, other_set.
([Structure set](https://minecraft.wiki/w/Structure_set),
[JSON format](https://minecraft.wiki/w/Structure_set/JSON_format))

### Jigsaw
Деревни, аванпосты, бастионы, древние города, трейл-руины, trial chambers.
Пул: элементы с весом 1–150 (single, feature, list, empty); projection
rigid / terrain_matching; глубина `size` 0–20.
([Jigsaw structure](https://minecraft.wiki/w/Jigsaw_structure),
[Template pool](https://minecraft.wiki/w/Template_pool))

### Строения верхнего мира

| строение | spacing | separation | размещение |
|---|---|---|---|
| древний город | 24 | 8 | random spread |
| закопанный сундук | 1 | 0 | random spread |
| пустынный храм | 32 | 8 | random spread |
| иглу | 32 | 8 | random spread |
| джунглевый храм | 32 | 8 | random spread |
| заброшенная шахта | 1 | 0 | random spread |
| океанский монумент | 32 | 5 | random spread |
| кораблекрушение | 24 | 4 | random spread |
| крепость | — | — | concentric rings, 128 штук в 8 кольцах |
| трейл-руины | 34 | 8 | random spread |
| trial chambers | 34 | 34 (?) | random spread |
| деревня | 34 | 8 | random spread |
| особняк | 80 | 20 | random spread |

Прочее по отчёту: деревни пяти видов (равнины, пустыня, саванна, тайга,
снег), 2 % заброшенных; пустынный храм — 4 сундука и ловушка с TNT;
джунглевый — 2 сундука, стрелы, рычажная загадка; иглу — в половине случаев
с подвалом; хижина ведьмы — только болото; особняк — тёмный лес; монумент —
глубокий океан, 58×58; шахта — 0,4 % попыток на чанк, в бесплодных землях
на поверхности; разрушенный портал — всюду кроме глубинной тьмы;
древний город — глубинная тьма, Y −51; trial chambers — Y −40…−20;
закопанный сундук — пляж, смещение 9/9 в чанке.

### Данж (комната монстров)
Высота 6, пол из булыжника и замшелого булыжника, 1–2 сундука, спаунер в
центре (зомби 50 %, скелет 25 %, паук 25 %), 1–5 проёмов высотой 2 блока
([Monster Room](https://minecraft.wiki/w/Monster_Room)). Размеры — см. пометку.

### Подгонка и процессоры
Terrain adaptation: none, beard_thin, beard_box, bury, encapsulate.
Процессоры: rule, block_rot, block_age, block_ignore, gravity,
protected_blocks, blackstone_replace, jigsaw_replacement,
lava_submerged_block, capped, nop
([Processor list](https://minecraft.wiki/w/Processor_list)).

### Строения в чанке Anvil
`structures.References` — 64-битные числа, X в младших, Z в старших 32 битах;
`structures.starts` — начатые строения, у кусков BB и свои данные;
достроенное — `INVALID` ([Chunk format](https://minecraft.wiki/w/Chunk_format)).

### Неясно
Точный алгоритм внутри сетки trial chambers; выравнивание jigsaw-разъёмов;
какие процессоры у каких строений (это в data pack, а data pack из jar мы
не извлекаем).
