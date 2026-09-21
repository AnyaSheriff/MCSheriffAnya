# Содержимое записей реестров (Java Edition 26.1.2, протокол 775)

Источник: только minecraft.wiki. Страницы протокола (`Java Edition protocol/Registries`)
и страницы форматов данных (`Dimension type`, `Biome definition (Java Edition)`,
`Mob variant definitions`, `Painting variant definition`, `Banner pattern definition`,
`Instrument definition`, `Jukebox song definition`, `Armor trim definition`,
`World clock`, `Environment attribute`, `Music Disc`).

Важно: вики описывает текущую версию игры (на момент выписки — снапшоты 26.3).
Поэтому для каждой страницы я отдельно смотрел ревизию вики от ~9 апреля 2026 года
(дата выхода 26.1.2) и ниже помечаю все места, где текущая вики отличается от 26.1.2.

---

## 0. Общие правила (из Java Edition protocol/Registries)

Пакет **Registry Data** (фаза Configuration), по одному пакету на реестр:

| Поле | Тип | Смысл |
|---|---|---|
| Registry ID | Identifier | имя реестра, например `minecraft:dimension_type` |
| Entries | Prefixed Array из (Identifier, Prefixed Optional NBT) | записи реестра |

Ключевые факты со страницы:

- Порядок записей в массиве задаёт числовые id, начиная с 0. Сервер и клиент обязаны
  совпадать, потому что многие пакеты ссылаются на записи по числовому id.
  **«The client will disconnect upon receiving a reference to a non-existing entry.»**
- NBT записи имеет ту же структуру, что и JSON-файл этой записи в data pack,
  только записанный в NBT (см. `NBT format#Conversion from JSON`). То есть:
  строка JSON → NBT String, число с точкой → Float/Double, целое → Int,
  `true`/`false` → Byte 1/0 (NBT-представление boolean), объект → Compound, массив → List.
- Если NBT у записи отсутствует (Prefixed Optional = false), клиент берёт содержимое
  из согласованных known packs (`minecraft:core`). Именно это у нас сейчас и происходит,
  и именно это ломается на клиентах другой версии.
- Если запись вообще не упомянута в Registry Data — её не будет в сессии,
  даже если она есть в known pack.
- Ссылки на другие реестры и теги разрешаются клиентом **в момент Finish Configuration**:
  «detecting unbound references to entries and tags». Несуществующая ссылка на запись
  реестра или тег = отключение клиента на этом шаге.

### Как работают ссылки внутри NBT

Различаем три случая — это важно, потому что от него зависит, опасна ссылка или нет:

1. **Ссылка на запись синхронизируемого реестра** (например биом в `spawn_conditions`,
   world clock в `default_clock`, timeline в `timelines`). Разрешается клиентом,
   несуществующее имя → ошибка загрузки реестров и дисконнект.
   Совет: не ссылаться ни на что лишнее (пустые списки).
2. **Ссылка на запись встроенного реестра `minecraft:sound_event`** (строковая форма
   поля `sound_event` / `*_sound`). Тоже разрешается, несуществующее имя → ошибка.
   **Безопасная альтернатива:** вместо строки послать compound (inline-звук):
   `{sound_id: "minecraft:...", range: 16.0f}`. Такой звук ни в каком реестре не ищется;
   если файла звука нет, клиент просто ничего не проиграет.
3. **Ссылка на ресурс клиента (текстура, спрайт, ключ перевода)** — поля `asset_id`,
   `baby_asset_id`, `asset_name`, `translation_key`, тексты `description`/`title`.
   Реестрами не проверяются, при загрузке реестров не разрешаются.
   Отсутствующая текстура = чёрно-фиолетовая заглушка, отсутствующий ключ перевода =
   показывается сам ключ. Клиент не падает.
4. **Теги блоков** (`infiniburn` у dimension_type) — записываются строкой с `#`.
   Теги приходят отдельным пакетом Update Tags, а не в Registry Data.

---

## 1. `minecraft:cat_variant` (запись `tabby`)

Формат (Mob variant definitions → Cat):

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | resource location текстуры взрослого кота |
| `baby_asset_id` | String | да | resource location текстуры котёнка (поле появилось в 26.1 snap2) |
| `spawn_conditions` | List<Compound> | да (может быть пустым) | условия выбора варианта при спавне |

Пример `minecraft:tabby`:

```
{
  asset_id: "minecraft:entity/cat/tabby",
  baby_asset_id: "minecraft:entity/cat/tabby_baby",
  spawn_conditions: []
}
```

Точных строк текстур вики не приводит — годится любой корректный resource location;
неверный путь даёт заглушку текстуры, но не отключение.

Ссылки наружу: `asset_id`/`baby_asset_id` — только текстуры (безопасно);
`spawn_conditions` может ссылаться на биомы/структуры (опасно, см. §21).

---

## 2. `minecraft:cat_sound_variant` (запись `classic`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `adult_sounds` | Compound | да | звуки взрослых котов |
| `baby_sounds` | Compound | да | звуки котят (те же поля) |

Внутри `adult_sounds` / `baby_sounds` — 9 звуковых полей, каждое либо String (id звука
из реестра `minecraft:sound_event`), либо Compound `{sound_id: String, range: Float(необяз.)}`:

`ambient_sound`, `beg_for_food_sound`, `death_sound`, `eat_sound`, `hiss_sound`,
`hurt_sound`, `purr_sound`, `purreow_sound`, `stray_ambient_sound`.

Пример `minecraft:classic` (безопасная inline-форма):

```
{
  adult_sounds: {
    ambient_sound:       {sound_id: "minecraft:entity.cat.ambient"},
    beg_for_food_sound:  {sound_id: "minecraft:entity.cat.beg_for_food"},
    death_sound:         {sound_id: "minecraft:entity.cat.death"},
    eat_sound:           {sound_id: "minecraft:entity.cat.eat"},
    hiss_sound:          {sound_id: "minecraft:entity.cat.hiss"},
    hurt_sound:          {sound_id: "minecraft:entity.cat.hurt"},
    purr_sound:          {sound_id: "minecraft:entity.cat.purr"},
    purreow_sound:       {sound_id: "minecraft:entity.cat.purreow"},
    stray_ambient_sound: {sound_id: "minecraft:entity.cat.stray_ambient"}
  },
  baby_sounds: { ... те же 9 полей ... }
}
```

Замечание по версиям: кошачьи/свиные/коровьи/куриные sound variants добавлены
в 26.1 snap7 — то есть в 26.1.2 они есть, а в более старых клиентах (1.21.x) этих
реестров нет вообще.

---

## 3. `minecraft:chicken_variant` (записи `cold`, `temperate`, `warm`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | текстура взрослой курицы |
| `baby_asset_id` | String | да | текстура цыплёнка (с 26.1 snap2) |
| `model` | String | да | модель: только `normal` или `cold` |
| `spawn_conditions` | List | да (может быть пустым) | условия спавна |

Пример:

```
temperate: { asset_id: "minecraft:entity/chicken/temperate_chicken",
             baby_asset_id: "minecraft:entity/chicken/temperate_chicken_baby",
             model: "normal", spawn_conditions: [] }
cold:      { ... model: "cold" ... }
warm:      { ... model: "normal" ... }
```

Важно (из раздела «Chicken variant requirements»): именно три записи
`minecraft:cold`, `minecraft:temperate`, `minecraft:warm` обязаны существовать,
иначе клиент не примет Finish Configuration (они — значения по умолчанию
у компонентов предметов).

Значение `model` строго ограничено списком; произвольная строка = ошибка разбора NBT.

---

## 4. `minecraft:chicken_sound_variant` (запись `classic`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `adult_sounds` | Compound | да | звуки взрослых |
| `baby_sounds` | Compound | да | звуки цыплят |

Внутри каждого: `ambient_sound`, `death_sound`, `hurt_sound`, `step_sound`
(String или Compound `{sound_id, range}`).

```
{
  adult_sounds: { ambient_sound: {sound_id:"minecraft:entity.chicken.ambient"},
                  death_sound:   {sound_id:"minecraft:entity.chicken.death"},
                  hurt_sound:    {sound_id:"minecraft:entity.chicken.hurt"},
                  step_sound:    {sound_id:"minecraft:entity.chicken.step"} },
  baby_sounds:  { ... те же 4 ... }
}
```

---

## 5. `minecraft:cow_variant` (запись `temperate`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | текстура коровы |
| `baby_asset_id` | String | да | текстура телёнка (с 26.1 snap2) |
| `model` | String | да | `normal`, `cold` или `warm` |
| `spawn_conditions` | List | да (может быть пустым) | условия спавна |

```
{ asset_id: "minecraft:entity/cow/temperate_cow",
  baby_asset_id: "minecraft:entity/cow/temperate_cow_baby",
  model: "normal",
  spawn_conditions: [] }
```

---

## 6. `minecraft:cow_sound_variant` (запись `classic`)

Внимание: у коровы звуки лежат **прямо в корне**, без `adult_sounds`/`baby_sounds`
(в отличие от кота, свиньи, курицы, волка).

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `ambient_sound` | String или Compound | да | обычный звук |
| `death_sound` | String или Compound | да | смерть |
| `hurt_sound` | String или Compound | да | урон |
| `step_sound` | String или Compound | да | шаг |

```
{ ambient_sound: {sound_id:"minecraft:entity.cow.ambient"},
  death_sound:   {sound_id:"minecraft:entity.cow.death"},
  hurt_sound:    {sound_id:"minecraft:entity.cow.hurt"},
  step_sound:    {sound_id:"minecraft:entity.cow.step"} }
```

---

## 7. `minecraft:frog_variant` (запись `temperate`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | текстура лягушки |
| `spawn_conditions` | List | да (может быть пустым) | условия спавна |

У лягушки **нет** `baby_asset_id` и нет `model`.

```
{ asset_id: "minecraft:entity/frog/temperate_frog", spawn_conditions: [] }
```

---

## 8. `minecraft:painting_variant` (запись `kebab`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | спрайт картины, должен быть в атласе `painting` |
| `width` | Int | да | ширина в блоках, 1..16 |
| `height` | Int | да | высота в блоках, 1..16 |
| `title` | String / List / Compound | нет | текстовый компонент — название (с 1.21.2) |
| `author` | String / List / Compound | нет | текстовый компонент — автор (с 1.21.2) |

`kebab` — картина 1×1, оригинальное название «Kebab med tre pepperoni»
(автор Kristoffer Zetterstrand).

```
{ asset_id: "minecraft:kebab", width: 1, height: 1,
  title:  {translate: "painting.minecraft.kebab.title"},
  author: {translate: "painting.minecraft.kebab.author"} }
```

Точных строк ключей перевода вики не даёт — годится любой текстовый компонент,
например просто `{text: "Kebab"}`; можно вообще опустить `title`/`author`.

С 1.21.6 варианты картин больше нельзя задавать inline в NBT сущности —
только через реестр.

---

## 9. `minecraft:pig_variant` (запись `temperate`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | текстура свиньи |
| `baby_asset_id` | String | да | текстура поросёнка (с 26.1 snap2) |
| `model` | String | нет, по умолчанию `normal` | `normal` или `cold` |
| `spawn_conditions` | List | да (может быть пустым) | условия спавна |

```
{ asset_id: "minecraft:entity/pig/temperate_pig",
  baby_asset_id: "minecraft:entity/pig/temperate_pig_baby",
  model: "normal", spawn_conditions: [] }
```

---

## 10. `minecraft:pig_sound_variant` (запись `classic`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `adult_sounds` | Compound | да | звуки взрослых |
| `baby_sounds` | Compound | да | звуки поросят |

Внутри: `ambient_sound`, `death_sound`, `eat_sound`, `hurt_sound`, `step_sound`.

```
{ adult_sounds: { ambient_sound:{sound_id:"minecraft:entity.pig.ambient"},
                  death_sound:  {sound_id:"minecraft:entity.pig.death"},
                  eat_sound:    {sound_id:"minecraft:entity.pig.eat"},
                  hurt_sound:   {sound_id:"minecraft:entity.pig.hurt"},
                  step_sound:   {sound_id:"minecraft:entity.pig.step"} },
  baby_sounds:  { ... те же 5 ... } }
```

---

## 11. `minecraft:wolf_variant` (запись `pale`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `assets` | Compound | да | текстуры взрослого волка |
| `assets.angry` | String | да | текстура злого волка |
| `assets.wild` | String | да | текстура дикого волка |
| `assets.tame` | String | да | текстура прирученного волка |
| `baby_assets` | Compound | да | те же три поля для щенка (с 26.1 snap2) |
| `spawn_conditions` | List | да (может быть пустым) | условия спавна |

```
{ assets:      { angry:"minecraft:entity/wolf/wolf_angry",
                 wild: "minecraft:entity/wolf/wolf",
                 tame: "minecraft:entity/wolf/wolf_tame" },
  baby_assets: { angry:"minecraft:entity/wolf/wolf_baby_angry",
                 wild: "minecraft:entity/wolf/wolf_baby",
                 tame: "minecraft:entity/wolf/wolf_baby_tame" },
  spawn_conditions: [] }
```

История формата (важно для совместимости с более старыми клиентами через ViaVersion):
до 25w04a (1.21.5) поля назывались `angry_texture`, `wild_texture`, `tame_texture`
и лежали в корне, а вместо `spawn_conditions` был список `biomes`.

---

## 12. `minecraft:wolf_sound_variant` (запись `classic`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `adult_sounds` | Compound | да | звуки взрослых волков |
| `baby_sounds` | Compound | да | звуки щенков |

Внутри: `ambient_sound`, `death_sound`, `growl_sound`, `hurt_sound`, `pant_sound`,
`whine_sound`.

```
{ adult_sounds: { ambient_sound:{sound_id:"minecraft:entity.wolf.ambient"},
                  death_sound:  {sound_id:"minecraft:entity.wolf.death"},
                  growl_sound:  {sound_id:"minecraft:entity.wolf.growl"},
                  hurt_sound:   {sound_id:"minecraft:entity.wolf.hurt"},
                  pant_sound:   {sound_id:"minecraft:entity.wolf.pant"},
                  whine_sound:  {sound_id:"minecraft:entity.wolf.whine"} },
  baby_sounds:  { ... те же 6 ... } }
```

Версии: реестр появился в 25w08a (1.21.5); в 26.1 snap2 звуки переехали из корня
в `adult_sounds` и добавилось `baby_sounds`. То есть 26.1.2 — новая раскладка,
а 1.21.5–1.21.11 — старая (звуки прямо в корне).

---

## 13. `minecraft:zombie_nautilus_variant` (запись `temperate`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | текстура |
| `model` | String | нет, по умолчанию `normal` | `normal` или `warm` |
| `spawn_conditions` | List | да (может быть пустым) | условия спавна |

```
{ asset_id: "minecraft:entity/zombie_nautilus/temperate_zombie_nautilus",
  model: "normal", spawn_conditions: [] }
```

Реестр добавлен в 25w45a (1.21.11). В более старых клиентах его нет вовсе.

---

## 14. `minecraft:trim_material` (11 записей)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_name` | String | да | суффикс, подставляемый в имена спрайтов и ключи перевода |
| `override_armor_assets` | Compound | нет | переопределения `asset_name` для конкретных материалов брони |
| `override_armor_assets.<equipment asset id>` | String | — | суффикс для брони с таким `asset_id` в компоненте `equippable` |
| `description` | String / List / Compound | да | текстовый компонент — название материала |

```
amethyst: { asset_name: "amethyst",
            description: {translate: "trim_material.minecraft.amethyst",
                          color: "#9A5CC6"} }
```

Аналогично для `copper`, `diamond`, `emerald`, `gold`, `iron`, `lapis`, `netherite`,
`quartz`, `redstone`, `resin` — `asset_name` равен имени записи.
В ваниле у некоторых материалов есть `override_armor_assets`
(например у железа — «тёмный» вариант для железной брони); точных значений вики
не приводит, поле можно опустить — тогда просто не будет затемнённых вариантов.

Все 11 записей **обязательны** (раздел «Trim material requirements»): они используются
как значения по умолчанию у компонентов предметов, без них клиент не примет
Finish Configuration.

Ссылки наружу: только текстуры/ключи перевода — безопасно.
Ключи `override_armor_assets` — идентификаторы наборов брони, тоже не проверяются жёстко.

Замечание: `trim_pattern` (не наш случай) имеет поля `asset_id` (String),
`decal` (Boolean), `description`.

---

## 15. `minecraft:instrument` (запись `ponder_goat_horn`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `description` | Compound (текстовый компонент) | да | описание в подсказке предмета |
| `sound_event` | String или Compound | да | звук, проигрываемый при использовании рога |
| `use_duration` | Float | да | длительность кулдауна использования, ≥ 0 |
| `range` | Float | да | дальность слышимости, ≥ 0 |
| `durability_damage` | Int | нет | **добавлено только в 26.3 snap1 — в 26.1.2 не слать** |

Вики прямо даёт ванильные значения:
`ponder_goat_horn` → звук `minecraft:item.goat_horn.sound.0`, range `256.0`,
use_duration `7.0`, ключ описания `instrument.minecraft.ponder_goat_horn`.

```
{ description: {translate: "instrument.minecraft.ponder_goat_horn"},
  sound_event: {sound_id: "minecraft:item.goat_horn.sound.0", range: 256.0f},
  use_duration: 7.0f,
  range: 256.0f }
```

Запись `minecraft:ponder_goat_horn` **обязательна** — это значение по умолчанию
компонента `minecraft:instrument` у козьего рога; без неё клиент не примет
Finish Configuration.

---

## 16. `minecraft:jukebox_song` (21 запись)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `sound_event` | String или Compound | да | звук, включаемый проигрывателем |
| `description` | String / Compound | да | текстовый компонент для подсказки предмета |
| `length_in_seconds` | Float | да | длина трека в секундах; определяет, сколько времени проигрыватель выдаёт редстоун-сигнал (звук обрезается по этому времени) |
| `comparator_output` | Int | да | 0..15 — сигнал компаратора рядом с проигрывателем |

Поле `range` внутри `sound_event` игнорируется: песня передаётся клиенту
как часть события обновления проигрывателя, а не как обычный звук.

Идентификаторы звуков строятся как `minecraft:music_disc.<имя записи>`
(проверено на странице Music Disc 13: sound event `music_disc.13`).

Значения из вики (длина диска и сила сигнала компаратора):

| Запись | sound_event | length_in_seconds | comparator_output |
|---|---|---|---|
| `13` | `minecraft:music_disc.13` | 178 (2:58) | 1 |
| `cat` | `minecraft:music_disc.cat` | 185 (3:05) | 2 |
| `blocks` | `minecraft:music_disc.blocks` | 345 (5:45) | 3 |
| `chirp` | `minecraft:music_disc.chirp` | 185 (3:05) | 4 |
| `far` | `minecraft:music_disc.far` | 174 (2:54) | 5 |
| `mall` | `minecraft:music_disc.mall` | 197 (3:17) | 6 |
| `mellohi` | `minecraft:music_disc.mellohi` | 96 (1:36) | 7 |
| `stal` | `minecraft:music_disc.stal` | 150 (2:30) | 8 |
| `strad` | `minecraft:music_disc.strad` | 188 (3:08) | 9 |
| `ward` | `minecraft:music_disc.ward` | 251 (4:11) | 10 |
| `11` | `minecraft:music_disc.11` | 71 (1:11) | 11 |
| `wait` | `minecraft:music_disc.wait` | 237 (3:57) | 12 |
| `pigstep` | `minecraft:music_disc.pigstep` | 148 (2:28) | 13 |
| `otherside` | `minecraft:music_disc.otherside` | 195 (3:15) | 14 |
| `5` | `minecraft:music_disc.5` | 178 (2:58) | 15 |
| `relic` | `minecraft:music_disc.relic` | 219 (3:39) | 14 |
| `creator` | `minecraft:music_disc.creator` | 176 (2:56) | 12 |
| `creator_music_box` | `minecraft:music_disc.creator_music_box` | 73 (1:13) | 11 |
| `precipice` | `minecraft:music_disc.precipice` | 299 (4:59) | 13 |
| `tears` | `minecraft:music_disc.tears` | 175 (2:55) | 10 |
| `lava_chicken` | `minecraft:music_disc.lava_chicken` | 135 (2:15) | 9 |

Длины пересчитаны из «минут:секунд» на странице Music Disc; ванильные значения
`length_in_seconds` могут отличаться на доли секунды — вики точных дробных значений
не даёт, любое близкое годится (влияет только на длительность редстоун-сигнала).

Пример записи:

```
13: { sound_event: {sound_id: "minecraft:music_disc.13"},
      description: {translate: "jukebox_song.minecraft.13"},
      length_in_seconds: 178.0f,
      comparator_output: 1 }
```

Все перечисленные записи обязательны (раздел «Jukebox song requirements»).
**Отличие от нашего списка:** в текущей вики к обязательным добавлена запись
`minecraft:bounce` (диск «Bounce», сигнал компаратора 8) — она из версии новее 26.1.2.
Для 26.1.2 её слать не нужно; наш список из 21 записи соответствует 26.1.2.

---

## 17. `minecraft:banner_pattern` (запись `base`)

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `asset_id` | String | да | resource location текстуры узора |
| `translation_key` | String | да | ключ перевода для подсказки на знамени |

```
base: { asset_id: "minecraft:base",
        translation_key: "block.minecraft.banner.base" }
```

Точного значения `translation_key` для `base` вики не приводит — годится любая строка;
при неизвестном ключе клиент покажет сам ключ, но не отключится.
Имя узора `base` подтверждается таблицей Banner/Patterns («Resource name» = `base`).

Отдельно: у реестра banner_pattern особое требование — для принятия Finish Configuration
должны существовать **теги** (пакет Update Tags), а не записи:
`minecraft:pattern_item/bordure_indented`, `.../creeper`, `.../field_masoned`,
`.../flow`, `.../flower`, `.../globe`, `.../guster`, `.../mojang`, `.../piglin`,
`.../skull`. Если тег ссылается на несуществующую запись реестра — клиент отключится,
поэтому теги можно послать пустыми либо добавить соответствующие записи.

---

## 18. `minecraft:dimension_type` (запись `overworld`)

Самый важный реестр: без него нельзя послать корректный Login (play).
Формат сильно переделан в 1.21.11 (25w42a/25w45a) и 26.1 — старые примеры
из интернета для 1.21.x **не подойдут**.

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `coordinate_scale` | Double | да | множитель координат при переходе; 0.00001..30000000.0 |
| `has_skylight` | Boolean | да | есть ли небесный свет; если false, погода отключается |
| `has_ceiling` | Boolean | да | логический потолок (как в Нижнем мире): нет погоды, другой расчёт спавна и респавна, карты пишут радиус 32 вместо 64 |
| `has_ender_dragon_fight` | Boolean | да | может ли здесь быть бой с драконом Края (**добавлено в 26.1 snap6**) |
| `ambient_light` | Float | да | фоновое освещение: 0 — полностью по уровню света, 1 — без затемнения |
| `has_fixed_time` | Boolean | нет, по умолчанию false | фиксированное время суток (**заменило поле `fixed_time` в 25w45a**) |
| `monster_spawn_block_light_limit` | Int | да | 0..15, предел блочного света для спавна монстров |
| `monster_spawn_light_level` | Int или Compound (int provider) | да | 0..15, предел суммарного света для спавна монстров |
| `logical_height` | Int | да | предел, до которого телепортируют хорус/порталы; не больше `height` |
| `min_y` | Int | да | нижняя граница блоков; −2032..2031, кратно 16 |
| `height` | Int | да | общая высота; 16..4064, кратно 16; `min_y + height − 1` ≤ 2031 |
| `infiniburn` | String | да | тег блоков с `#`, на которых огонь горит вечно (с 26.2 допускаются также id и список id — в 26.1.2 только тег) |
| `skybox` | String | нет, по умолчанию `overworld` | `none`, `overworld` или `end` (**заменило поле `effects` в 25w45a**) |
| `cardinal_light` | String | нет, по умолчанию `default` | `default` или `nether` (**тоже из 25w45a**) |
| `attributes` | Compound | да (может быть пустым `{}`) | карта environment attributes этого измерения (**добавлено в 25w42a**) |
| `default_clock` | String | нет | ссылка на запись реестра `minecraft:world_clock` (**добавлено в 26.1 snap3**) |
| `timelines` | String или List | нет | ссылки на записи реестра `minecraft:timeline` (**добавлено в 25w45a**) |

Ванильные значения Overworld (таблица «Defaults» на вики):

```
overworld: {
  has_skylight: 1b,
  has_ceiling: 0b,
  has_ender_dragon_fight: 0b,
  coordinate_scale: 1.0d,
  has_fixed_time: 0b,
  ambient_light: 0.0f,
  min_y: -64,
  height: 384,
  logical_height: 384,
  monster_spawn_light_level: {type:"minecraft:uniform", min_inclusive:0, max_inclusive:7},
  monster_spawn_block_light_limit: 0,
  infiniburn: "#minecraft:infiniburn_overworld",
  skybox: "overworld",
  cardinal_light: "default",
  attributes: {},
  default_clock: "minecraft:overworld"
}
```

Упрощения, которые можно смело делать:
- `monster_spawn_light_level` разрешено послать простым Int (например `7`) — это
  допустимая форма вместо int provider, у нас спавна монстров всё равно нет.
- `attributes: {}` — пустая карта полностью корректна: у каждого атрибута есть
  значение по умолчанию, измерение их просто не переопределяет.
- `default_clock` и `timelines` можно **не слать вообще** — тогда у измерения
  не будет часов по умолчанию, и клиент не станет разрешать ссылки на world_clock
  и timeline. Если слать `default_clock: "minecraft:overworld"`, то запись
  `minecraft:overworld` обязана быть в реестре `minecraft:world_clock`, иначе дисконнект.

Что чему ссылается:
- `infiniburn` → тег блоков (пакет Update Tags). Несуществующий тег = ошибка привязки.
  Безопасно послать пустой тег `minecraft:infiniburn_overworld`.
- `default_clock` → реестр `minecraft:world_clock` (см. §20).
- `timelines` → реестр `minecraft:timeline` (мы его не шлём — значит поле опускаем).
- `attributes` → имена environment attributes (`minecraft:visual/fog_color`,
  `minecraft:gameplay/bed_rule` и т. п.). Неизвестное имя = ошибка разбора,
  поэтому лучше пустая карта.

Отличия от 1.21.x (что делать при ViaVersion): в 1.21.x у dimension_type были поля
`ultrawarm`, `natural`, `bed_works`, `respawn_anchor_works`, `piglin_safe`,
`has_raids`, `effects`, `fixed_time`, `cloud_height`. В 26.1.2 всех их нет —
они переехали в environment attributes и в `skybox`/`cardinal_light`/`has_fixed_time`.
ViaVersion сам перекладывает их в обе стороны, но собственному серверу
надо слать именно формат 26.1.2.

---

## 19. `minecraft:worldgen/biome` (запись `plains`)

Второй критичный реестр: запись `minecraft:plains` обязана существовать,
клиент использует её как биом по умолчанию для незагруженных чанков,
иначе не примет Login (play).

Формат 26.1.2 (ревизия вики от февраля 2026; текущая вики уже показывает
формат 26.3, где убраны `spawners`, `spawn_costs`, `creature_spawn_probability`):

| Поле | Тип NBT | Обязательное | Смысл |
|---|---|---|---|
| `has_precipitation` | Boolean | да | бывают ли осадки |
| `temperature` | Float | да | температура: цвет травы/листвы, снег или дождь, детали генерации |
| `temperature_modifier` | String | нет, по умолчанию `none` | `none` или `frozen` |
| `downfall` | Float | да | влажность: влияет на цвет травы и листвы |
| `effects` | Compound | да | визуальные эффекты биома |
| `effects.water_color` | Int | да | цвет воды, десятичное представление HEX (обычное значение 4159204) |
| `effects.foliage_color` | Int | нет | цвет листвы; если нет — считается из temperature/downfall |
| `effects.dry_foliage_color` | Int | нет | цвет опавшей листвы (добавлено в 1.21.5) |
| `effects.grass_color` | Int | нет | цвет травы; если нет — считается из temperature/downfall |
| `effects.grass_color_modifier` | String | нет, по умолчанию `none` | `none`, `dark_forest` или `swamp` |
| `attributes` | Compound | нет | карта environment attributes биома (добавлено в 25w42a) |
| `carvers` | String / Compound / List | да (может быть пустым) | ссылки на карверы (генерация пещер) |
| `features` | List<List> | да (может быть пустым) | список из шагов генерации, в каждом — список placed feature |
| `creature_spawn_probability` | Float | нет | 0.0..0.9999999 |
| `spawners` | Compound | да (может быть пустым) | настройки спавна по категориям |
| `spawn_costs` | Compound | да (может быть пустым) | «стоимость» спавна мобов |

Структура `spawners` (для полноты, нам достаточно пустого compound):
ключ — категория (`monster`, `creature`, `ambient`, `water_creature`,
`underground_water_creature`, `water_ambient`, `misc`, `axolotls`),
значение — List<Compound> с полями `type` (String, id сущности), `weight` (Int),
`minCount` (Int > 0), `maxCount` (Int ≥ minCount).

Структура `spawn_costs`: ключ — id сущности, значение — Compound
с `energy_budget` (Double) и `charge` (Double).

Минимальная безопасная запись `minecraft:plains`:

```
plains: {
  has_precipitation: 1b,
  temperature: 0.8f,
  downfall: 0.4f,
  effects: { water_color: 4159204 },
  carvers: [],
  features: [],
  spawners: {},
  spawn_costs: {}
}
```

С 25w44a поля `water_color`, `foliage_color`, `dry_foliage_color`, `grass_color`
принимают также hex-строку или массив из трёх float (red, green, blue),
но упакованное целое по-прежнему работает — его и стоит использовать.

Чего в 26.1.2 в биоме **больше нет** (убрано в 25w42a, 1.21.11):
`effects.fog_color`, `effects.sky_color`, `effects.water_fog_color`,
`effects.particle`, `effects.ambient_sound`, `effects.mood_sound`,
`effects.additions_sound`, `effects.music`, `effects.music_volume`.
Всё это стало environment attributes. Для клиентов 1.21.x через ViaVersion
эти поля приходится восстанавливать — но это работа ViaVersion, не наша.

Ссылки наружу: `carvers` и `features` ссылаются на реестры генерации мира
(configured carver / placed feature). Эти реестры **не синхронизируются**, и пустые
списки полностью безопасны — так и делаем. `attributes` — см. §18.

---

## 20. `minecraft:world_clock` (запись `overworld`)

Страница World clock прямо говорит: «Format: object with no fields» —
у записи **нет ни одного поля**. Игра различает часы только по их id.

```
overworld: {}
```

По умолчанию в игре два world clock: `minecraft:overworld` и `minecraft:the_end`.
Реестр помечен как Optional, но если dimension_type ссылается на часы через
`default_clock`, эта запись обязана существовать.

Особенность: world clock загружается клиентом только при старте клиента,
`/reload` его не перечитывает.

Версии: world clock добавлены в 26.1 snap3 (`the_end` — в snap4). Это совсем новый
реестр: клиенты 1.21.x о нём не знают.

---

## 21. Про `spawn_conditions` (общее для всех mob variant)

Формат (Mob variant definitions → Spawn condition):

```
spawn_conditions: [
  { priority: <Int>, condition: { type: "<тип>", ... } }
]
```

- `priority` (Int, обязательно) — приоритет варианта, если условие совпало.
- `condition` (Compound, необязательно) — если опущено, условие совпадает всегда.

Типы условий:
- `biome` — поле `biomes`: String или List, **ссылка на реестр `minecraft:worldgen/biome`
  или на тег биомов**. Несуществующий биом/тег → клиент отключится при
  Finish Configuration.
- `structure` — поле `structures`: String или List, ссылка на структуры или тег структур.
- `moon_brightness` — поле `range`: Double либо Compound `{min: Double, max: Double}`.

**Рекомендация для нашего сервера:** слать `spawn_conditions: []` (пустой список).
Выбор варианта при спавне делает сервер, а не клиент, так что клиенту эти условия
не нужны, и пустой список убирает все опасные ссылки наружу.

---

## 22. Сводка: что обязательно и почему

Из раздела «Requirements» страницы протокола:

| Реестр | Требование |
|---|---|
| `dimension_type` | минимум одна запись — иначе нельзя послать Login (play) |
| `worldgen/biome` | обязана быть запись `minecraft:plains` — иначе клиент не примет Login (play) |
| `chicken_variant` | обязаны быть `cold`, `temperate`, `warm` |
| `trim_material` | обязаны быть все 11: amethyst, copper, diamond, emerald, gold, iron, lapis, netherite, quartz, redstone, resin |
| `instrument` | обязана быть `minecraft:ponder_goat_horn` |
| `jukebox_song` | обязаны быть все перечисленные в §16 |
| `banner_pattern` | требуются теги `minecraft:pattern_item/*` (Update Tags), а не записи |
| `damage_type` | 26 обязательных записей + теги `bypasses_shield`, `is_explosion`, `is_fire` (в нашем списке этого реестра нет, но клиент его требует) |
| `cat_variant`, `cat_sound_variant`, `chicken_sound_variant`, `cow_variant`, `cow_sound_variant`, `frog_variant`, `painting_variant`, `pig_variant`, `pig_sound_variant`, `wolf_variant`, `wolf_sound_variant`, `zombie_nautilus_variant` | «Non-empty» — достаточно одной записи, но она должна быть |
| `world_clock` | Optional — но обязателен, если на него ссылается `default_clock` |

---

## 23. Чего на вики нет

- Точных ванильных строк `asset_id` / `baby_asset_id` для вариантов кота, курицы,
  коровы, лягушки, свиньи, волка и зомби-наутилуса. Вики описывает только тип поля.
  Любой корректный resource location принимается; при отсутствии текстуры
  клиент покажет заглушку и не отключится.
- Точных ключей перевода (`translation_key`, `description`, `title`, `author`)
  для banner_pattern `base`, картины `kebab` и материалов отделки. Формат ключей
  в описании подтверждён только для инструментов
  (`instrument.minecraft.ponder_goat_horn`).
- Точных значений `override_armor_assets` у ванильных trim_material.
- Дробных значений `length_in_seconds` у jukebox_song — на вики есть только
  длина трека в формате «минуты:секунды».
- Списка ванильных `spawn_conditions` для конкретных вариантов мобов.
- Описания формата записи реестра `minecraft:timeline` я здесь не разбирал —
  он нам не нужен, поле `timelines` мы не шлём.

---

Использована только minecraft.wiki: страницы протокола и страницы форматов данных.
Код Minecraft, декомпилированный код, содержимое client.jar/server.jar,
файлы data pack из поставки игры, а также код серверных ядер (Paper, Spigot, Fabric,
Forge, Bukkit, ViaVersion) и модов не открывались, не скачивались и не распаковывались.
