# Линии времени (`minecraft:timeline`), свойства среды и небо

Разбор по minecraft.wiki: страницы [Timeline](https://minecraft.wiki/w/Timeline),
[Environment attribute](https://minecraft.wiki/w/Environment_attribute),
[World clock](https://minecraft.wiki/w/World_clock),
[Dimension type](https://minecraft.wiki/w/Dimension_type),
[Timeline tag (Java Edition)](https://minecraft.wiki/w/Timeline_tag_(Java_Edition)),
[Overworld](https://minecraft.wiki/w/Overworld),
[Daylight cycle](https://minecraft.wiki/w/Daylight_cycle),
[Sun](https://minecraft.wiki/w/Sun), [Moon](https://minecraft.wiki/w/Moon),
[Sky](https://minecraft.wiki/w/Sky),
[Java Edition protocol/Registries](https://minecraft.wiki/w/Java_Edition_protocol/Registries),
[Java Edition 25w45a](https://minecraft.wiki/w/Java_Edition_25w45a),
[Java Edition 26.1](https://minecraft.wiki/w/Java_Edition_26.1),
[Java Edition 26.1 Snapshot 3](https://minecraft.wiki/w/Java_Edition_26.1_Snapshot_3).

Кода игры и серверных ядер здесь нет — только описания форматов с вики.
Цитаты приведены по-английски дословно, всё остальное — пересказ и наши расчёты
(наши места помечены словом «реконструкция»).

Главное в двух строках: небо у нас чёрное потому, что **`visual/sky_color` по
умолчанию `#000000`**, а солнце, звёзды и смена суток целиком живут в реестре
`minecraft:timeline`, которого мы не шлём. Чтобы небо стало синим, достаточно
одного поля `attributes` в типе измерения; чтобы появились солнце, луна, звёзды
и переход день/ночь — нужны реестры `minecraft:world_clock` и
`minecraft:timeline`.

---

## 1. Реестр `minecraft:timeline`

Путь в датапаке: `data/<namespace>/timeline/<id>.json`, по сети — обычная запись
реестра в NBT (структура один в один как JSON).

> «**Timelines** control game behavior and visuals based on the absolute day time
> through environment attributes.»
> — [Timeline](https://minecraft.wiki/w/Timeline)

### 1.1. Корень записи

| Поле | Тип NBT | Обязательность | Смысл |
|---|---|---|---|
| `period_ticks` | TAG_Int | необязательное | Длина периода в тактах, после которого линия повторяется. Нет поля — линия не повторяется |
| `clock` | TAG_String | **обязательное** (с 26.1 snap 3) | Id мировых часов (`minecraft:world_clock`), к которым привязана линия |
| `tracks` | TAG_Compound | необязательное | Карта: id свойства среды → дорожка |
| `time_markers` | TAG_Compound | необязательное (с 26.1 snap 3) | Карта: id метки времени → такт или объект |

Дословно с вики (дерево формата):

> «* {{nbt|compound}}: The root element of the timeline.
> ** `period_ticks`: Optional. Defines the duration in ticks over which the
> timeline will repeat. If omitted, the timeline will not repeat.
> ** `clock`: world clock to use for this timeline.
> ** `tracks`: A map between namespaced environment attribute IDs and a
> corresponding attribute track object.
> ** `time_markers`: Map defining time markers.»

Про обязательность `clock` — из журнала изменений 26.1 snapshot 3:

> «Added a `clock` field: a world clock ID, specifies which world clock the
> timeline is tied to. **This field is required**, but to match previous behavior
> the `minecraft:overworld` clock can be used.»

`tracks` на странице Timeline без пометки «Optional», но в заметках 25w45a:
«`tracks`: optional map between Environment Attribute IDs and a corresponding
Environment Attribute Track object». Считаем необязательным (это противоречие
между страницей формата и журналом версии — см. §9).

### 1.2. Дорожка (значение внутри `tracks`)

Ключ карты — **id свойства среды** (`minecraft:visual/sky_color` и т. п.),
значение — компаунд:

| Поле | Тип NBT | Обязательность | Смысл |
|---|---|---|---|
| `ease` | TAG_String **или** TAG_Compound | необязательное, по умолчанию `linear` | Способ сглаживания между кадрами |
| `ease.cubic_bezier` | TAG_List из 4 TAG_Double | если `ease` — компаунд | Координаты двух опорных точек в порядке `x1, y1, x2, y2` |
| `modifier` | TAG_String | необязательное, по умолчанию `override` | Id модификатора свойства среды |
| `keyframes` | TAG_List из TAG_Compound | по смыслу обязательное | Ключевые кадры, **упорядоченные по `ticks`** |

> «`ease`: The easing type used to ease the interpolation between keyframes.
> Either a string name of the interpolation type (see below) or an object with
> the field listed below. Default is `linear`. **Note that easing only has an
> effect for attributes which support interpolation.**»

> «`cubic_bezier`: A list of 4 doubles providing the coordinates of the 2 control
> points in the order x1, y1, x2, y2. Each of these coordinates must be between 0
> and 1 (inclusive).»

(В заметках 25w45a иначе: «`x1`: float between 0 and 1 … `y1`: float …» — то есть
ограничение 0..1 там только на x. Ещё одно противоречие, см. §9.)

> «`modifier`: The ID of an environment attribute modifier. The default is
> `override`.»

### 1.3. Ключевой кадр

| Поле | Тип NBT | Обязательность | Смысл |
|---|---|---|---|
| `ticks` | TAG_Int | обязательное | Такт внутри периода, в который значение кадра активно. Единица — игровой такт (1/20 с) конкретных мировых часов |
| `value` | зависит от свойства/модификатора | обязательное | Аргумент модификатора (при `override` — само значение свойства) |

> «`ticks`: A value between 0 and `period_ticks` (if specified) which defines the
> tick within the period at which the keyframe's value will be active.»
> «`value`: The value for the attribute modifier.»

Из заметок 25w45a (подробнее):

> «`keyframes` – list of keyframe objects, **must be ordered by the ticks
> field** … `value` – the modifier argument (format dependent on the chosen
> modifier). If no modifier is specified (or override is used), the type of this
> field is the same as the Environment Attribute itself. … **Note: at most two
> keyframes can be placed on the same tick, creating an immediate transition.**»

Важное про интерполяцию:

> «Unlike biomes, if a Timeline uses a modifier instead of an override,
> interpolation is applied to the modifier arguments rather than the final
> modified values.»

И как работает зацикливание (пример с вики):

> «Between time = 0 and time = 1000, the sky color will be red. Between 1000 and
> 6000, it will shift from red to magenta. Then, from 6000 all the way until the
> timeline repeats and reaches time = 0 again, the color will slowly shift back
> to red.»

То есть последний кадр интерполируется к первому через границу периода.

### 1.4. Метки времени (`time_markers`)

| Поле | Тип NBT | Обязательность | Смысл |
|---|---|---|---|
| ключ карты | — | — | id метки (`day`, `noon`, `night`, `midnight`, `minecraft:wake_up_from_sleep`, …) |
| значение | TAG_Int **или** TAG_Compound | — | такт метки, либо объект |
| `ticks` | TAG_Int | обязательное в объекте | Такт метки, 0..`period_ticks` |
| `show_in_commands` | TAG_Byte (0/1) | необязательное, по умолчанию `false` | Показывать ли метку в подсказках `/time` |

> «Either an int integer representing the tick of the marker or an object with
> the following fields: `ticks`: The tick of this marker. `show_in_commands`:
> Whether this marker should be suggested for the `/time` command. Defaults to
> `false`.»

Встроенные метки, на которые смотрит сама игра:

> «`wake_up_from_sleep`: When the player wakes up, the tick that the game advances
> the time to. **The time will not advance if this time marker does not exist.**»
> «`roll_village_siege`: The time that the game starts to determine whether a
> zombie siege has happened. If this time marker does not exist, the game will
> not determine if a zombie siege has happened.»
> — [World clock](https://minecraft.wiki/w/World_clock)

Метки живут в контексте часов, а не линии:

> «Time markers will exist within the context of a specific world clock. This
> means that even though the different time markers are defined by different
> Timelines, only one Time Marker can exist with a particular id for a particular
> world clock.»

Для часов `minecraft:overworld` в ваниле доступны метки `day`, `noon`, `night`,
`midnight` (журнал 26.1, раздел `/time`). Их такты вики прямо не даёт, но по
странице `/time` и истории команды: `day` = 1000, `night` = 13000, `noon` = 6000,
`midnight` = 18000.

### 1.5. Способы сглаживания (`ease`)

Два не интерполирующих:

| Значение | Поведение (цитата) |
|---|---|
| `constant` | «Always selects the value from the previous keyframe.» |
| `linear` | «Linearly interpolates between the previous and next keyframes» |

Остальные — семейства кривых в трёх формах `in_`, `out_`, `in_out_`:

> «`in_` applies the easing *from* the value of a keyframe after it's been passed,
> `out_` applies the easing *into* the value of a keyframe before it's been
> passed, and `in_out_` combines the two.»

Семейства: `back`, `bounce`, `circ`, `elastic`, `sine`, `quad`, `cubic`, `quart`,
`quint`, `expo`. Итого имена: `in_back`, `out_back`, `in_out_back`, … `in_expo`,
`out_expo`, `in_out_expo` (30 штук) плюс `constant` и `linear`.

Формулы (ease-out, вики даёт именно их):

| Семейство | Формула ease-out |
|---|---|
| `back` | y = 1 + 2.70158 (x−1)³ + 1.70158 (x−1)² |
| `circ` | y = √(1 − (x−1)²) |
| `elastic` | y = 2^(−10x)·sin((2π/3)(10x − 0.75)) + 1 |
| `sine` | y = sin(πx/2) |
| `quad` | y = 1 − (1−x)² |
| `cubic` | y = 1 − (1−x)³ |
| `quart` | y = 1 − (1−x)⁴ |
| `quint` | y = 1 − (1−x)⁵ |
| `expo` | y = 1 − 2^(−10x) |
| `bounce` | кусочная, 4 куска с коэффициентом 7.5625 (см. страницу Timeline) |

---

## 2. Свойства среды (Environment attribute)

> «**Environment attributes** control various visual and gameplay features
> depending on the dimension, biome, time, and weather.»

Порядок наложения источников (от низшего приоритета к высшему):

> «* Dimension types * Biome definitions * Timelines * Weather (not data-driven)»

В типе измерения и в биоме значение пишется так (это **не** формат линии времени):

```
attributes: {
  "<id свойства>": <значение>            // прямое значение = override
  "<id свойства>": { modifier: "...", argument: <...> }
}
```

### 2.1. Как пишется каждый тип значения в NBT

| Тип | Как записать |
|---|---|
| boolean | TAG_Byte 0/1 (в JSON `true`/`false`) |
| float | TAG_Float (в JSON просто число) |
| угол | тот же TAG_Float, единица — градусы |
| строка | TAG_String |
| RGB-цвет | «RGB colors can be represented as: string using `#RRGGBB`; packed int converted from hex to decimal; a list of 3 floats between 0 and 1.» |
| ARGB-цвет | «ARGB colors can be represented as: string using `#AARRGGBB`; packed int converted from hex to decimal, **as a signed 64 bit integer**; a list of 4 floats between 0 and 1.» |
| компаунд/список | как описано у конкретного свойства |

Про ARGB формулировка вики внутренне противоречива (см. §9): «packed int … as a
signed 64 bit integer». Мы для наших записей берём **строку** `#AARRGGBB` —
это однозначно и без вопросов про знак и разрядность.

### 2.2. Список встроенных свойств

Колонки: id → тип значения → допустимые модификаторы → значение по умолчанию →
интерполируется ли.

**Звук**

| Id | Тип | Модификаторы | По умолчанию | Интерп. |
|---|---|---|---|---|
| `audio/ambient_sounds` | компаунд (`mood`/`additions`/`loop`) | только `override` | `{}` | нет |
| `audio/background_music` | компаунд (`default`/`creative`/`underwater`) | только `override` | `{}` | нет |
| `audio/firefly_bush_sounds` | boolean | логические | `false` | нет |
| `audio/music_volume` | float | числовые | `1.0` | нет |

**Игровая механика**

| Id | Тип | Модификаторы | По умолчанию | Интерп. |
|---|---|---|---|---|
| `gameplay/baby_villager_activity` | строка (активность) | только `override` | `idle` | нет |
| `gameplay/bed_rule` | компаунд (`can_sleep`, `can_set_spawn`, `explodes`, `error_message`) | только `override` | `{can_sleep: "when_dark", can_set_spawn: "always", error_message: {translate: "block.minecraft.bed.no_sleep"}}` | нет |
| `gameplay/bees_stay_in_hive` | boolean | логические | `false` | нет |
| `gameplay/can_pillager_patrol_spawn` | boolean | логические | `true` | нет |
| `gameplay/can_start_raid` | boolean | логические | `true` | нет |
| `gameplay/cat_waking_up_gift_chance` | float 0..1 | числовые | `0.0` | **да** |
| `gameplay/creaking_active` | boolean | логические | `false` | нет |
| `gameplay/eyeblossom_open` | boolean или строка `"default"` | только `override` | `"default"` | нет |
| `gameplay/fast_lava` | boolean | логические | `false` | нет |
| `gameplay/increased_fire_burnout` | boolean | логические | `false` | нет |
| `gameplay/monsters_burn` | boolean | логические | `false` | нет |
| `gameplay/nether_portal_spawns_piglin` | boolean | логические | `false` | нет |
| `gameplay/piglins_zombify` | boolean | логические | `true` | нет |
| `gameplay/respawn_anchor_works` | boolean | логические | `false` | нет |
| `gameplay/sky_light_level` | float | числовые | `15.0` | **да** |
| `gameplay/snow_golem_melts` | boolean | логические | `false` | нет |
| `gameplay/surface_slime_spawn_chance` | float 0..1 | числовые | `0.0` | **да** |
| `gameplay/turtle_egg_hatch_chance` | float 0..1 | числовые | `0.002` | **да** |
| `gameplay/villager_activity` | строка (активность) | только `override` | `idle` | нет |
| `gameplay/water_evaporates` | boolean | логические | `false` | нет |

Про `sky_light_level` дословно:

> «The internal sky light level (inversely known as the "darkening" sky light
> level), **not** the actual sky light level. This affects mechanics such as
> monster spawning, daylight detector power, and ice melting. The visual effect is
> controlled by `visual/sky_light_factor`.»

**Видимое (то, что нас интересует для неба)**

| Id | Тип | Модификаторы | По умолчанию | Интерп. |
|---|---|---|---|---|
| `visual/ambient_light_color` | RGB | RGB | `#000000` | да |
| `visual/ambient_particles` | список | только `override` | `[]` | нет |
| `visual/block_light_tint` | RGB | RGB | `#FFD88C` | да |
| `visual/cloud_color` | ARGB | ARGB | `#00000000` (полностью прозрачный) | да |
| `visual/cloud_fog_end_distance` | float | числовые | `2048.0` | да |
| `visual/cloud_height` | float | числовые | `192.33` | да |
| `visual/default_dripstone_particle` | компаунд | только `override` | `{type: "minecraft:dripping_dripstone_water"}` | нет |
| `visual/fog_color` | RGB | RGB | `#000000` | да |
| `visual/fog_end_distance` | float | числовые | `1024.0` | да |
| `visual/fog_start_distance` | float | числовые | `0.0` | да |
| `visual/moon_angle` | float, градусы | числовые | `0.0` | да |
| `visual/moon_phase` | строка | только `override` | `full_moon` | нет |
| `visual/night_vision_color` | RGB | RGB | `#999999` | да |
| **`visual/sky_color`** | RGB | RGB | **`#000000`** | да |
| `visual/sky_fog_end_distance` | float | числовые | `512.0` | да |
| `visual/sky_light_color` | RGB | RGB | `#FFFFFF` | да |
| `visual/sky_light_factor` | float | числовые | `1.0` | да |
| `visual/star_angle` | float, градусы | числовые | `0.0` | да |
| `visual/star_brightness` | float | числовые | `0.0` | да |
| `visual/sun_angle` | float, градусы | числовые | `0.0` | да |
| `visual/sunrise_sunset_color` | ARGB | ARGB | `#00000000` | да |
| `visual/water_fog_color` | RGB | RGB | `#050533` | да |
| `visual/water_fog_end_distance` | float | числовые | `96.0` | да |
| `visual/water_fog_start_distance` | float | числовые | `-8.0` | да |

Ключевые цитаты:

> «`visual/sky_color`: Color of the sky (also affected by time of day and
> weather). RGB color. RGB modifier. Default `#000000`.»

> «`visual/sun_angle`: Position of the sun in degrees from east to west.
> **0 is directly up. Only used with `overworld` skybox type.**»

> «`visual/moon_angle`: Position of the moon in degrees from east to west. 0 is
> directly up. Only used with `overworld` skybox type.»

> «`visual/star_angle`: Position of the stars in degrees from east to west. Only
> used with `overworld` skybox type.»

> «`visual/star_brightness`: Brightness of the stars. **`0.5` is the default night
> start brightness.** Only used with `overworld` skybox type.» (у них опечатка,
> имеется в виду «night star brightness»)

> «`visual/moon_phase`: Phase of the moon. Only used with `overworld` skybox
> type. `full_moon`, `waning_gibbous`, `third_quarter`, `waning_crescent`,
> `new_moon`, `waxing_crescent`, `first_quarter`, or `waxing_gibbous`.»

> «`visual/sunrise_sunset_color`: Color and intensity of sunrise and sunset. Only
> used with `overworld` skybox type. ARGB color.»

То есть **для солнца/луны/звёзд обязателен `skybox: "overworld"` в типе
измерения** (поле `skybox`, по умолчанию как раз `overworld`).

### 2.3. Модификаторы

| Группа | `modifier` | Аргумент |
|---|---|---|
| любой тип | `override` | новое значение |
| boolean | `and`, `nand`, `or`, `nor`, `xor`, `xnor` | TAG_Byte |
| float | `add`, `subtract`, `multiply`, `minimum`, `maximum` | TAG_Float |
| float | `alpha_blend` | компаунд `{value: float, alpha: float 0..1 (необяз.)}` |
| RGB/ARGB | `add`, `subtract`, `multiply` | цвет (int/строка/список). «The ARGB `multiply` modifier also allows a ARGB argument» |
| RGB/ARGB | `alpha_blend` | ARGB-цвет |
| RGB/ARGB | `blend_to_gray` | компаунд `{brightness: float 0..1, factor: float 0..1}` |

> «`blend_to_gray`: Applies component-wise blending of the RGB components towards
> a grayscale color. `brightness`: Fraction of the brightness of the original
> color to blend towards (0.0 - 1.0). `factor`: Blend factor towards the grayscale
> color (0.0 - 1.0). Behaves like the alpha component.»

Формула из заметок 25w45a:

> «`gray = brightness * (0.3 * red + 0.59 * green + 0.11 * blue)`,
> `result = lerp(factor, subject, [gray, gray, gray])`»

Это и есть готовый способ гасить небо к ночи, не теряя цвет биома.

---

## 3. Тип измерения: `timelines` и `default_clock`

Дословно с [Dimension type](https://minecraft.wiki/w/Dimension_type):

> «`default_clock`: world clock to use as the default for this dimension. This
> clock will be used as default for the `/time` command, and the
> `minecraft:wake_up_from_sleep` and `minecraft:roll_village_siege` time markers
> of this clock will be used. **If not specified, the dimension doesn't have a
> default clock.**»

> «`timelines`: timeline(s) that are active in this dimension.»
> (шаблон `{{nbt|string}}{{nbt|list}}` + `json ref|timeline|tag=1`, то есть
> **строка или список строк**, где строка — либо id записи, либо тег с `#`)

Заметки 25w45a однозначнее:

> «Added a new optional `timelines` field that specifies which Timelines are
> active in this dimension. **Format: a Timeline ID, a list of Timeline IDs, or a
> Timeline Tag.**»

В NBT это либо TAG_String (`"minecraft:day"` или `"#minecraft:in_overworld"`),
либо TAG_List из TAG_String.

Связь с `default_clock`: **прямой нет**. У каждой линии свои часы (`clock`),
и линия тикает по ним. `default_clock` измерения нужен только для `/time` без
`of <clock>`, для пробуждения в кровати и для осады деревни. На практике это
значит: если в измерении активна линия с `clock: "minecraft:overworld"`, то и
`default_clock` разумно поставить `minecraft:overworld`, иначе `/time set day`
и сон будут работать не по той шкале.

> «In a world, every dimension will get their own world clock and the default
> value of a certain world clock shown by the `/time` command according to their
> dimension type, and confirm which time markers are used according to the
> timeline that uses this world clock.» — World clock

Значения по умолчанию в ваниле (таблица Defaults на странице Dimension type):
`default_clock` = `overworld` у Обычного мира и у Overworld Caves,
`the_end` у Края, **нет** у Нижнего мира.

### Что будет при ссылке на несуществующую запись

Реестры разрешаются на клиенте в конце фазы Configuration:

> «Client accumulates registry and tag data for later resolution … Client resolves
> registry and tag data, loading data from packs when requested, and **detecting
> unbound references to entries and tags**.»

> «**If an entry is not mentioned in a Registry Data packet, it will not exist in
> the game session, even if it exists in one of the known packs. All entries that
> will be used in the session must be listed in Registry Data packets; only the
> NBT part can be omitted.**»

> «The client will disconnect upon receiving a reference to a non-existing entry.»
> — [Java Edition protocol/Registries](https://minecraft.wiki/w/Java_Edition_protocol/Registries)

Вывод для нас: если в типе измерения написать `timelines` или `default_clock`
со ссылкой на то, чего мы не прислали, клиент отвалится (или сразу на разборе
реестров, или на Finish Configuration). Поэтому одно из двух:

1. **Не писать** `timelines`/`default_clock` вовсе — тогда небо чёрное, но
   клиент живой (это наше текущее состояние);
2. слать реестр `minecraft:world_clock` (хотя бы `minecraft:overworld`),
   реестр `minecraft:timeline` (наша линия) и ссылаться на **id записи**, а не
   на тег. Теги — отдельный пакет Update Tags, и «Tag data is never sourced from
   known packs, and must always be sent by the server in full», так что ссылку
   `#minecraft:in_overworld` пришлось бы ещё и тегом подкреплять. Проще
   написать `"timelines": "rustcraft:day"` строкой.

---

## 4. Ванильная линия времени Обычного мира

**Точных значений ванильной линии на вики нет.** Вики даёт формат, но не
содержимое файлов `minecraft:day` / `minecraft:moon` / `minecraft:early_game`.
Полнотекстовый поиск по вики по `period_ticks`, `visual/sun_angle`,
`star_brightness`, `sunrise_sunset_color` находит только страницы формата и
журналы версий — ни одного дампа ванильной линии.

Что вики всё-таки даёт.

### 4.1. Какие линии есть в ваниле

Со страницы [Timeline tag](https://minecraft.wiki/w/Timeline_tag_(Java_Edition)):

- тег `#minecraft:in_overworld` содержит: `#universal`, **`day`**, **`moon`**,
  **`early_game`**;
- тег `#minecraft:universal` содержит: **`villager_schedule`**;
- теги `#minecraft:in_nether` и `#minecraft:in_end` содержат только `#universal`.

Содержимое самих этих линий на вики не приводится (страницы помечены
`{{Info needed}}`).

### 4.2. Ванильный тип измерения Обычного мира (это на вики есть целиком)

Со страницы [Overworld](https://minecraft.wiki/w/Overworld), раздел
«Dimension type definition» — нужные нам куски:

```json
{
  "ambient_light": 0.0,
  "attributes": {
    "minecraft:visual/ambient_light_color": "#0a0a0a",
    "minecraft:visual/cloud_color": "#ccffffff",
    "minecraft:visual/cloud_height": 192.33,
    "minecraft:visual/fog_color": "#c0d8ff",
    "minecraft:visual/sky_color": "#78a7ff"
  },
  "default_clock": "minecraft:overworld",
  "timelines": "#minecraft:in_overworld"
}
```

Это ровно то, чего нам не хватает в первую очередь: **дневное небо Обычного мира
— `#78a7ff`**, туман — `#c0d8ff`, облака — `#ccffffff`, фоновый свет — `#0a0a0a`.
(Страница Sky тоже называет цвет неба `#78A7FF`.) Уже одно это поле `attributes`
в нашем типе измерения убирает чёрное небо днём — без всяких линий времени.

### 4.3. Числа суточного цикла (страница Daylight cycle)

| Такт | Событие (цитаты/пересказ) |
|---|---|
| 0 | «Beginning of the Minecraft day», рассвет закончился |
| 0 … 12000 | день: «Start: 0 ticks … Mid: 6000 ticks … End: 12000 ticks» |
| 12000 | «Sunset is the period between daytime and nighttime, and always lasts 50 seconds (1000 ticks). Start: 12000 ticks» |
| 12040 | «In sunny weather, the internal sky-light level begins to decrease» |
| 12542 | «In clear weather, beds can be used at this point … undead mobs no longer burn» |
| 13000 | конец заката, «Nighttime … Start: 13000 ticks» |
| 13670 | «The internal sky-light level reaches 4, the minimum at night» |
| 18000 | полночь, «Mid: 18000 ticks» |
| 22331 | «The internal sky-light level begins to increase» |
| 23000 | «Sunrise … always lasts 50 seconds (1000 ticks). Start: 23000 ticks» |
| 23460 | «In clear weather, beds can no longer be used … undead mobs begin to burn» |
| 23961 | «In sunny weather, the internal sky-light level reaches 15, the maximum» |
| 24000 | «End: 24000 ticks» — период суток |

Небо ночью: «During the night, the moon rises to its peak in **a pure black sky**
dotted with small white stars … Clouds are black and more transparent, and the
fog is colored dark blue.» Звёзды: «The stars appear to move with the moon and
**can be first seen toward the end of the sunset**.»

Солнце и луна: «The sun and moon rise in the east and set in the west.»
Фазы луны: «the moon … goes through eight lunar phases, and **changes phase at
the end of every sunrise**», «`phase = (day mod 8) + 1`», полный лунный цикл —
8 суток = 192000 тактов.

### 4.4. Угол неба — формула есть, точная

> «The sky, including sun, moon, and stars, move faster during noon and midnight
> and slower during sunrises and sunsets. Given the current time in ticks since
> dawn as *t*, the current angle of the sky with **0° being noon** can be
> calculated as:»
>
> α = (1 − cos(π · mod₁((t − 6000)/24000)) + mod₄((t − 6000)/6000)) · 60°

Посчитанная по этой формуле таблица (наш расчёт, углы развёрнуты в непрерывную
возрастающую последовательность, чтобы линейная интерполяция между кадрами не
пошла «назад»):

| такт | `sun_angle`, ° | | такт | `sun_angle`, ° |
|---|---|---|---|---|
| 0 | −77.57 | | 13000 | 93.47 |
| 1000 | −62.40 | | 14000 | 110.00 |
| 2000 | −48.04 | | 15000 | 127.04 |
| 3000 | −34.57 | | 16000 | 144.47 |
| 4000 | −22.04 | | 17000 | 162.17 |
| 5000 | −10.51 | | 18000 | 180.00 |
| 6000 | 0.00 | | 19000 | 197.83 |
| 7000 | 10.51 | | 20000 | 215.53 |
| 8000 | 22.04 | | 21000 | 232.96 |
| 9000 | 34.57 | | 22000 | 250.00 |
| 10000 | 48.04 | | 23000 | 266.53 |
| 11000 | 62.40 | | 24000 | 282.43 (= −77.57) |
| 12000 | 77.57 | | | |

Луна и звёзды противоположны солнцу: `moon_angle = sun_angle + 180`,
`star_angle` — туда же (звёзды «appear to move with the moon»). Это наша
реконструкция: вики нигде не пишет, что `moon_angle` ровно на 180° больше.

---

## 5. Готовый пример: наша линия времени Обычного мира

Реконструкция. Формат — точно по вики, числа — из §4 (такты вики) плюс наши
значения там, где вики молчит (помечено `// наше`).

Запись реестра `minecraft:timeline`, ключ `rustcraft:day`:

```
TAG_Compound (корень записи реестра)
├── "clock"        TAG_String  "minecraft:overworld"
├── "period_ticks" TAG_Int     24000
├── "time_markers" TAG_Compound
│   ├── "day"       TAG_Compound { "ticks" TAG_Int 1000,  "show_in_commands" TAG_Byte 1 }
│   ├── "noon"      TAG_Compound { "ticks" TAG_Int 6000,  "show_in_commands" TAG_Byte 1 }
│   ├── "night"     TAG_Compound { "ticks" TAG_Int 13000, "show_in_commands" TAG_Byte 1 }
│   ├── "midnight"  TAG_Compound { "ticks" TAG_Int 18000, "show_in_commands" TAG_Byte 1 }
│   ├── "minecraft:wake_up_from_sleep" TAG_Int 0
│   └── "minecraft:roll_village_siege" TAG_Int 18000            // наше
└── "tracks" TAG_Compound
    ├── "minecraft:visual/sun_angle" TAG_Compound
    │   ├── "ease"      TAG_String "linear"
    │   ├── "modifier"  TAG_String "override"
    │   └── "keyframes" TAG_List of TAG_Compound
    │       ├── { "ticks" TAG_Int     0, "value" TAG_Float  -77.57 }
    │       ├── { "ticks" TAG_Int  3000, "value" TAG_Float  -34.57 }
    │       ├── { "ticks" TAG_Int  6000, "value" TAG_Float    0.00 }
    │       ├── { "ticks" TAG_Int  9000, "value" TAG_Float   34.57 }
    │       ├── { "ticks" TAG_Int 12000, "value" TAG_Float   77.57 }
    │       ├── { "ticks" TAG_Int 15000, "value" TAG_Float  127.04 }
    │       ├── { "ticks" TAG_Int 18000, "value" TAG_Float  180.00 }
    │       ├── { "ticks" TAG_Int 21000, "value" TAG_Float  232.96 }
    │       └── { "ticks" TAG_Int 23999, "value" TAG_Float  282.42 }
    │       // шаг можно взять и 1000 тактов по таблице §4.4 — точнее
    ├── "minecraft:visual/moon_angle" TAG_Compound
    │   └── то же самое, все значения +180                        // наше
    ├── "minecraft:visual/star_angle" TAG_Compound
    │   └── то же самое, что у луны                               // наше
    ├── "minecraft:visual/star_brightness" TAG_Compound
    │   ├── "ease"      TAG_String "linear"
    │   ├── "modifier"  TAG_String "override"
    │   └── "keyframes" TAG_List
    │       ├── { "ticks" TAG_Int 12000, "value" TAG_Float 0.0 }  // наше
    │       ├── { "ticks" TAG_Int 13000, "value" TAG_Float 0.5 }  // 0.5 — с вики
    │       ├── { "ticks" TAG_Int 22500, "value" TAG_Float 0.5 }  // наше
    │       └── { "ticks" TAG_Int 23500, "value" TAG_Float 0.0 }  // наше
    ├── "minecraft:visual/sky_color" TAG_Compound
    │   ├── "ease"      TAG_String "linear"
    │   ├── "modifier"  TAG_String "multiply"   // гасим цвет биома к ночи
    │   └── "keyframes" TAG_List
    │       ├── { "ticks" TAG_Int 12000, "value" TAG_String "#ffffff" }  // наше
    │       ├── { "ticks" TAG_Int 13500, "value" TAG_String "#000000" }  // наше
    │       ├── { "ticks" TAG_Int 22500, "value" TAG_String "#000000" }  // наше
    │       └── { "ticks" TAG_Int 23800, "value" TAG_String "#ffffff" }  // наше
    ├── "minecraft:visual/fog_color" TAG_Compound
    │   └── так же, как sky_color, но к ночи не в чёрный, а в тёмно-синий:
    │       modifier "multiply", ночное значение "#2a2a4d"        // наше
    ├── "minecraft:visual/sunrise_sunset_color" TAG_Compound       // всё наше
    │   ├── "ease"      TAG_String "linear"
    │   ├── "modifier"  TAG_String "override"
    │   └── "keyframes" TAG_List
    │       ├── { "ticks" TAG_Int 11500, "value" TAG_String "#00ff6b1a" }
    │       ├── { "ticks" TAG_Int 12500, "value" TAG_String "#ccff6b1a" }
    │       ├── { "ticks" TAG_Int 13200, "value" TAG_String "#00ff6b1a" }
    │       ├── { "ticks" TAG_Int 22800, "value" TAG_String "#00ff6b1a" }
    │       ├── { "ticks" TAG_Int 23500, "value" TAG_String "#ccff6b1a" }
    │       └── { "ticks" TAG_Int 23950, "value" TAG_String "#00ff6b1a" }
    └── "minecraft:gameplay/sky_light_level" TAG_Compound
        ├── "ease"      TAG_String "linear"
        ├── "modifier"  TAG_String "override"
        └── "keyframes" TAG_List                    // такты — с вики, §4.3
            ├── { "ticks" TAG_Int 12040, "value" TAG_Float 15.0 }
            ├── { "ticks" TAG_Int 13670, "value" TAG_Float  4.0 }
            ├── { "ticks" TAG_Int 22331, "value" TAG_Float  4.0 }
            └── { "ticks" TAG_Int 23961, "value" TAG_Float 15.0 }
```

Отдельная линия для фаз луны (период 8 суток; у `moon_phase` интерполяции нет,
поэтому `ease` обязан быть `constant`) — тоже реконструкция:

```
ключ "rustcraft:moon"
TAG_Compound
├── "clock"        TAG_String "minecraft:overworld"
├── "period_ticks" TAG_Int    192000
└── "tracks" TAG_Compound
    └── "minecraft:visual/moon_phase" TAG_Compound
        ├── "ease"      TAG_String "constant"
        ├── "modifier"  TAG_String "override"
        └── "keyframes" TAG_List
            ├── { "ticks" TAG_Int      0, "value" TAG_String "full_moon" }
            ├── { "ticks" TAG_Int  24000, "value" TAG_String "waning_gibbous" }
            ├── { "ticks" TAG_Int  48000, "value" TAG_String "third_quarter" }
            ├── { "ticks" TAG_Int  72000, "value" TAG_String "waning_crescent" }
            ├── { "ticks" TAG_Int  96000, "value" TAG_String "new_moon" }
            ├── { "ticks" TAG_Int 120000, "value" TAG_String "waxing_crescent" }
            ├── { "ticks" TAG_Int 144000, "value" TAG_String "first_quarter" }
            └── { "ticks" TAG_Int 168000, "value" TAG_String "waxing_gibbous" }
```

Что дописать в нашу запись `minecraft:dimension_type` / `minecraft:overworld`:

```
"skybox"        TAG_String "overworld"          // иначе солнца/луны/звёзд не будет вообще
"default_clock" TAG_String "minecraft:overworld"
"timelines"     TAG_String "rustcraft:day"      // или TAG_List ["rustcraft:day","rustcraft:moon"]
"attributes"    TAG_Compound {
    "minecraft:visual/sky_color"           TAG_String "#78a7ff"
    "minecraft:visual/fog_color"           TAG_String "#c0d8ff"
    "minecraft:visual/cloud_color"         TAG_String "#ccffffff"
    "minecraft:visual/cloud_height"        TAG_Float  192.33
    "minecraft:visual/ambient_light_color" TAG_String "#0a0a0a"
}
```

И запись реестра `minecraft:world_clock` с ключом `minecraft:overworld`:
пустой `TAG_Compound {}` — «Format: object with no fields».

Порядок, в котором это должно уехать клиенту:
`minecraft:world_clock` → `minecraft:timeline` → `minecraft:dimension_type`
(порядок пакетов Registry Data сам по себе не важен, важно, что все три реестра
присланы до Finish Configuration).

---

## 6. Совместимость с 26.1.2

- Реестр `minecraft:timeline` **есть** и **синхронизируемый**. Со страницы
  [Java Edition protocol/Registries](https://minecraft.wiki/w/Java_Edition_protocol/Registries),
  таблица «List of synchronized registries»:

  > «`minecraft:timeline` | NBT format | Optional | Timelines, which can manipulate
  > environment attributes on the client based on the time values of world clocks.»

  Рядом там же:

  > «`minecraft:world_clock` | NBT format | Optional | World clocks.»

  «Optional» в колонке Requirements значит «нет обязательного минимума записей»,
  а не «можно не слать при ссылках на него».

- Добавлен в 25w45a (1.21.11): «Added timelines to data packs.» Поле `clock` и
  `time_markers` — в 26.1 snapshot 3. Клиент 26.1.2 = ветка 26.1, значит
  **`clock` обязателен**, `time_markers` доступны.

- Формат NBT: «The NBT data of registry entries has the same structure as their
  definitions in data packs, but represented in NBT instead of JSON.» Всё как с
  остальными нашими реестрами (см. `registry-entries.md`).

- Отдельная грабля про часы: «The world clock is loaded once only when the client
  is started. Using `/reload` cannot reload the world clock; **The client must be
  restarted.**» Для нас это значит, что игрок, зашедший на сервер без перезапуска
  клиента после того, как мы поменяли список часов, может увидеть старое
  состояние — при отладке клиент перезапускать.

- Нужно ли слать: если мы хотим небо и солнце — да, `world_clock` + `timeline` +
  ссылка из `dimension_type`. Если пока хотим только синее небо днём без смены
  суток — достаточно одного `attributes` в `dimension_type`, реестры
  `timeline`/`world_clock` можно не слать и `timelines`/`default_clock` не писать.

---

## 7. Чего на вики нет

1. **Содержимого ванильных линий `minecraft:day`, `minecraft:moon`,
   `minecraft:early_game`, `minecraft:villager_schedule`** — ни одного кадра.
   Страницы тегов помечены `{{Info needed}}`.
2. Точных значений `visual/sunrise_sunset_color` в ваниле (цвет и прозрачность
   заката).
3. Точных тактов ванильных меток `day`/`noon`/`night`/`midnight` в новом формате
   (берём старые значения `/time`: 1000/6000/13000/18000) и такта
   `minecraft:wake_up_from_sleep` / `minecraft:roll_village_siege`.
4. Чем именно ваниль гасит `sky_color` ночью — `multiply`, `blend_to_gray` или
   набором `override`-кадров.
5. Как именно считается `moon_angle` и `star_angle` относительно `sun_angle`
   (что это ровно +180° — наш вывод из фразы «The moon is always opposite of the
   sun»).
6. Что происходит с линией, у которой нет `period_ticks`: что берётся за
   значение после последнего кадра и до первого.
7. Точная семантика «at most two keyframes can be placed on the same tick» при
   не-`constant` сглаживании.
8. Как ведёт себя клиент при `ease` на не интерполируемом свойстве, кроме общего
   «easing only has an effect for attributes which support interpolation».
9. Графиков кривых сглаживания (на самой вики стоит `{{wip}}`: «Adding graphs for
   each of the easing types would be helpful»).

## 8. Где вики противоречит сама себе

1. **`tracks`**: на странице Timeline поле идёт без пометки Optional, в заметках
   25w45a — «`tracks`: optional map».
2. **`cubic_bezier`**: страница Timeline — «Each of these coordinates must be
   between 0 and 1 (inclusive)» (то есть и y тоже); заметки 25w45a — ограничение
   0..1 только на `x1`/`x2`, а «`y1`: float», «`y2`: float» без границ.
3. **Упаковка ARGB**: «packed int converted from hex to decimal, as a signed 64
   bit integer» — «int» и «64 bit» в одной фразе. В NBT это либо TAG_Int (32 бита,
   и тогда `#AARRGGBB` с A ≥ 0x80 — отрицательное число), либо TAG_Long. Мы
   обходим вопрос строкой `#AARRGGBB`.
4. **`bed_rule`**: на странице Environment attribute поле называется `explodes`,
   а в истории той же страницы — «26.3 snap3: field `explodes` has been renamed to
   `destroy_on_use`». Для 26.1.2 актуально `explodes`; в ванильном JSON Обычного
   мира на странице Overworld у `straw_bed_rule` уже `destroy_on_leave` — то есть
   дамп на странице Overworld снят с более новой версии, чем 26.1.2, и слепо
   копировать оттуда всё подряд нельзя (цвета при этом с 26.1 не менялись).
5. Мелочь: «`0.5` is the default night **start** brightness» вместо «star».

---

Оригинальным кодом Minecraft (в том числе декомпилированным), client.jar,
server.jar, ванильными датапаками, а также кодом Paper/Spigot/Fabric/Forge/
Bukkit/ViaVersion и любых других ядер и модов при подготовке этого файла
не пользовался: только страницы minecraft.wiki, перечисленные в шапке.
