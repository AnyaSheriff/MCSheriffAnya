# Плавный ход поршня: `minecraft:moving_piston` и его блок-сущность

Справочник по вики (minecraft.wiki) для протокола 775 / Java Edition 26.1.2.
Всё, что нужно, чтобы сделать ход поршня видимым на клиенте, а не мгновенной
перестановкой блоков.

Источники (все цитаты — дословно оттуда):

- https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston)
- https://minecraft.wiki/w/Piston
- https://minecraft.wiki/w/Block_entity_format
- https://minecraft.wiki/w/Block_update (§ Special blocks)
- https://minecraft.wiki/w/Chunk_format
- https://minecraft.wiki/w/Java_Edition_protocol/Packets
- https://minecraft.wiki/w/Custom_world_generation/block_state

Ранее собранное по механике хода лежит в `tools/research/redstone-pistons.md`
(§ 1.3) и `tools/research/audit-pistons.md` (§ 1, § 11) — здесь не дублируется,
здесь только то, что нужно для **показа** движения клиенту.

---

## 1. Блок `minecraft:moving_piston`

### 1.1 Что это

> «The **moving piston** (JE) or **moving block** (BE), also known as **block 36**
> due to its pre-flattening block ID, is an unobtainable technical block that holds
> a block entity (unless it was placed with a command) which contains the block the
> piston is currently moving. Since moving blocks vary in how much of each grid cell
> they occupy, they can't be stored as normal blocks and are instead stored as block
> entity data. Multiple *moving piston* blocks may be used during the
> extension/retraction process, depending on how many blocks the piston is moving.
> At the end of the piston stroke, the *moving piston* blocks are replaced with
> either the carried block, the piston head (during extensions), or the piston
> itself (during retractions); but if it is placed with the use of commands it
> remains indefinitely.»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston)

> «When placed by commands, the game does not assign a block entity to the *moving
> piston* block, therefore its properties are different than usual: it's invisible,
> has no collisions, and cannot be broken without the use of TNT, commands, or a
> structure generating over it (such as the end platform). Although it is non-solid,
> fluids cannot pass through it. It also prevents players from building at its
> location. Mobs can see through it, but cannot walk through it. The game treats the
> block as a stone block when it comes to the player's footstep sounds.»
> — там же

Главное для нас: **блок сам по себе невидим и пуст.** Всё, что рисует клиент, —
это блок-сущность при нём. Без блок-сущности игрок увидит дырку.

### 1.2 Свойства блока (Java Edition)

> «{{bst|facing|north|down,east,north,south,up,west|The direction the block is being
> pushed by the piston.}}
> {{bst|type|normal|normal,sticky|What piston base this has.}}»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston → Block states)

| Свойство | Тип | По умолчанию | Значения | Смысл (дословно) |
|---|---|---|---|---|
| `facing` | enum (направление) | `north` | `down`, `east`, `north`, `south`, `up`, `west` | «The direction the block is being pushed by the piston.» |
| `type` | enum | `normal` | `normal`, `sticky` | «What piston base this has.» |

Итого **12 состояний** блока `minecraft:moving_piston` (6 × 2). Порядок перебора
свойств для номера состояния у нас уже задан общей таблицей блоков
(`src/blocks_table.rs`), сверяться нужно с ней, а не выводить вручную.

Обрати внимание: `facing` у `moving_piston` — это направление, **в котором блок
едет**, а не куда смотрит поршень. При выдвигании они совпадают; при втягивании
`facing` у `moving_piston` — это направление **к поршню** (то есть обратное
`facing` самого поршня). На вики это прямым текстом не сказано, см. § 8.

### 1.3 Соседние блоки, которые понадобятся

`minecraft:piston_head` (то, во что превращается «головная» запись в конце
выдвигания):

> «{{bst|facing|north|down,east,north,south,up,west|The direction the piston head is
> pointing.}}
> {{bst|short|false|false,true|If true, the piston arm is shorter than usual, by 4 pixels.}}
> {{bst|type|normal|normal,sticky|The type of piston head.}}»
> — https://minecraft.wiki/w/Piston/Technical_components

| Свойство | Значения | Смысл |
|---|---|---|
| `facing` | down/east/north/south/up/west | куда смотрит голова |
| `short` | false/true | «the piston arm is shorter than usual, by 4 pixels» |
| `type` | normal/sticky | обычная или липкая голова |

### 1.4 Сколько `moving_piston` ставится за ход и куда

Вики даёт только общее правило:

> «Multiple *moving piston* blocks may be used during the extension/retraction
> process, depending on how many blocks the piston is moving.»

и чем они заменяются в конце:

> «At the end of the piston stroke, the *moving piston* blocks are replaced with
> either the carried block, the piston head (during extensions), or the piston
> itself (during retractions)»

Отсюда **выводится** (точных формулировок «в какую клетку» на вики нет, см. § 8):

**Выдвигание (extension):**

| Клетка | Что стоит во время хода | Блок-сущность | Чем становится в конце |
|---|---|---|---|
| основание поршня | `minecraft:piston[facing=D, extended=true]` (обычный блок) | нет | остаётся |
| клетка перед основанием (там, где будет голова) | `moving_piston[facing=D, type=T]` | `source = 1`, `blockState = piston_head[...]` | `minecraft:piston_head` |
| каждая из N толкаемых клеток — **целевая** позиция каждого блока (сдвинутая на 1 по D) | `moving_piston[facing=D, type=T]` | `source = 0`, `blockState` = сам перевозимый блок | перевозимый блок |
| исходные клетки перевозимых блоков, которые **не** перекрыты новым `moving_piston` (то есть только самая дальняя от поршня освободившаяся клетка не бывает такой; на практике это клетка перед основанием — её занимает головная запись) | воздух | — | воздух |

То есть за один ход выдвигания ставится **1 + N** блоков `moving_piston`, где N —
число перевозимых блоков (0…12). При N = 0 ставится ровно один — головной.

**Втягивание (retraction):**

| Клетка | Что стоит во время хода | Блок-сущность | Чем становится в конце |
|---|---|---|---|
| основание поршня | `moving_piston` | `blockState` = `piston[facing=D, extended=false]`, `source = 1`, `extending = 0` | `minecraft:piston` / `sticky_piston` с `extended=false` |
| клетка, где была голова | `moving_piston` | `blockState` = притягиваемый блок (у липкого) или воздух, `source = 0`, `extending = 0` | притянутый блок / воздух |

Это следует из фразы «or the piston itself (during retractions)»: раз в конце хода
`moving_piston` становится **самим поршнем**, значит во время хода на месте
основания стоит `moving_piston`, а не поршень.

**Порядок постановки.** Вики про порядок говорит только через блок-апдейты
(см. § 6): сперва ставятся все `moving_piston` (каждый со своими PP-апдейтами), и
**только после того, как все поставлены**, рассылается пачка NC-апдейтов. Значит
постановка — это одна атомарная операция в фазе блочных событий, а не по одному
блоку с полными апдейтами.

---

## 2. Блок-сущность поршня

### 2.1 Идентификатор

Строковый id блок-сущности — **`minecraft:piston`** (не `moving_piston`!).
На вики он выписан в таблице ID:

> «|displayname=Block entity
> |spritename=moving-piston
> |nameid=piston»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Data values → ID)

### 2.2 Поля, общие для всех блок-сущностей

> «* Compound: A block entity
> ** String **id**: Block entity ID
> ** Boolean **keepPacked**: 1 or 0 (true/false) - If `true`, this is an invalid block
>    entity, and this block is not immediately placed when a loaded chunk is loaded.
>    If `false`, this is a normal block entity that can be immediately placed.
> ** Int **x**: X coordinate of the block entity.
> ** Int **y**: Y coordinate of the block entity.
> ** Int **z**: Z coordinate of the block entity.
> ** Compound **components**: Optional map of data components that are not represented
>    by additional fields.
> ** Additional fields depending on the block entity type (id), see Block entity format § Types»
> — https://minecraft.wiki/w/Block_entity_format (§ NBT format)

| Поле | Тип NBT | Обяз. | Смысл |
|---|---|---|---|
| `id` | String | да (на диске) | id типа блок-сущности, у нас `"minecraft:piston"` |
| `x`, `y`, `z` | Int | на диске да | координаты блока |
| `keepPacked` | Byte (bool) | нет | «invalid block entity» — нам не нужно, не пишем |
| `components` | Compound | нет | компоненты; поршню не нужны |

**Важно для сети:** в пакете чанка координаты **не пишутся** — «The block entity's
data, without the X, Y, and Z values» (см. § 3.2). В пакете Block Entity Data
позиция тоже отдельным полем; `id` в NBT там избыточен (тип идёт VarInt'ом), но
ванильный сервер его обычно всё равно кладёт.

### 2.3 Поля собственно поршня

> «* Compound Block entity data
> ** *(tags common to all block entities)*
> ** Compound **blockState**: The moving block represented by this block entity.
> *** *(block state)*
> ** Byte **extending**: 1 or 0 (true/false) – true if the piston is extending instead of withdrawing.
> ** Int **facing**: Direction that the piston pushes (0=down, 1=up, 2=north, 3=south, 4=west, 5=east).
> ** Float **progress**: How far the block has been moved. Starts at 0.0, and increments
>    by 0.5 each tick, including the tick on which moving the piston is created. If the
>    value is 1.0 or higher at the *start* of a tick of the block entity (before
>    incrementing), then the block transforms into the stored blockState. Negative values
>    can be used to increase the time until transformation.
> ** Byte **source**: 1 or 0 (true/false) – true if the block represents the piston head
>    itself, false if it represents a block being pushed.»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston → Block data)

| Поле | Тип NBT | Смысл | Значения |
|---|---|---|---|
| `blockState` | Compound | «The moving block represented by this block entity.» — какой блок едет | см. § 2.4 |
| `extending` | Byte (0/1) | «true if the piston is extending instead of withdrawing» | 1 — выдвигается, 0 — втягивается |
| `facing` | **Int** | «Direction that the piston pushes» | 0=down, 1=up, 2=north, 3=south, 4=west, 5=east |
| `progress` | **Float** | «How far the block has been moved» | 0.0 → 0.5 → 1.0; +0.5 за такт |
| `source` | Byte (0/1) | «true if the block represents the piston head itself, false if it represents a block being pushed» | 1 — это голова/сам поршень, 0 — перевозимый блок |

**Единицы `facing`.** Это **не** номер значения в свойстве `facing` блока и **не**
алфавитный порядок. Это порядок перечисления направлений (тот же, что у осей
координат: down, up, north, south, west, east):

| Число | Направление | Смещение (dx, dy, dz) |
|---|---|---|
| 0 | down | (0, −1, 0) |
| 1 | up | (0, +1, 0) |
| 2 | north | (0, 0, −1) |
| 3 | south | (0, 0, +1) |
| 4 | west | (−1, 0, 0) |
| 5 | east | (+1, 0, 0) |

Сравни с порядком значений **свойства блока** `facing` (алфавитный, как на вики
в таблице block states): `down, east, north, south, up, west` — то есть
east = 1, north = 2, … Это **другой** порядок. Смешивать нельзя.

**`progress`.** Дословно: «Starts at 0.0, and increments by 0.5 each tick,
**including the tick on which moving the piston is created**. If the value is 1.0
or higher at the *start* of a tick of the block entity (before incrementing), then
the block transforms into the stored blockState.» То есть:

| Такт | `progress` в начале такта | Что делает сервер |
|---|---|---|
| T0 (такт создания) | 0.0 | 0.0 < 1.0 → не превращает; после такта progress = 0.5 |
| T1 | 0.5 | 0.5 < 1.0 → не превращает; после такта progress = 1.0 |
| T2 | 1.0 | **≥ 1.0 → превращает** в `blockState` |

Значит блок-сущность живёт ровно **2 такта**, что и совпадает с «The extension
takes 2 game ticks (0.1 seconds) to finish» (§ 5).

Клиент рисует блок со смещением, пропорциональным `progress` (плюс интерполяция
внутри такта), в направлении `facing`, если `extending = 1`, и в обратном, если
`extending = 0`. На вики формула смещения не описана — см. § 8.

### 2.4 Как пишется `blockState`

Вики на странице поршня подставляет сюда общий шаблон «block state», который
ведёт на https://minecraft.wiki/w/Custom_world_generation/block_state:

> «** String **id**: The identifier of the block to use.
> ** Compound **properties**: (Optional, can be empty) Block properties. Unspecified
>    properties of the specified block will be set to their default values.
> ** Alternatively, instead of using a compound with String id and Compound properties
>    fields, a plain String string containing a block identifier can be used as a
>    shorthand for that block's default block state.»

**Здесь вики противоречит сама себе, и я не могу это сгладить.** Страница
`Custom world generation/block state` описывает формат из **датапаков (JSON)**, где
поля называются `id` и `properties` в нижнем регистре. Но:

- формат палитры блоков в NBT (`Chunk format`) — **`Name` и `Properties` с большой
  буквы**:
  > «******* String **Name**: Block resource location
  > ******* Compound **Properties**: List of block state properties, with `name` being
  > the name of the block state property
  > ******** String ***Name***: The block state's name and its value.»
  > — https://minecraft.wiki/w/Chunk_format
- на самой вики пример команды с тем же самым NBT-полем `BlockState` (у сущности
  падающего блока, где вики подставляет **тот же шаблон**) написан через `Name`:
  > «/summon minecraft:falling_block ~4 ~4 ~ {BlockState:{Name:"minecraft:cactus"},Time:1}»
  > — https://minecraft.wiki/w/Cactus (комментарий в исходнике статьи)

Вывод: **шаблон на странице поршня подставлен неаккуратно** — он описывает
датапаковый JSON, а не NBT. Для NBT правильная раскладка:

| Поле внутри `blockState` | Тип NBT | Смысл |
|---|---|---|
| `Name` | String | resource location блока, например `"minecraft:stone"` |
| `Properties` | Compound | необязательный; каждое свойство — **строка**, даже булево и числовое: `{"facing": "east", "short": "true"}` |

Все значения в `Properties` — **строки** (`"true"`, а не `1b`; `"7"`, а не `7`).
Пропущенные свойства берутся по умолчанию.

Это **компаунд, а не номер состояния.** Номер состояния (как в пакете Block
Update) здесь не годится.

---

## 3. Как блок-сущность попадает к клиенту (протокол 775)

### 3.1 Пакет Block Entity Data (clientbound, Play)

Дословно со страницы пакетов:

> «**Block Entity Data** — `block_entity_data`
> Sets the block entity associated with the block at the given location.
>
> | Field Name | Field Type | Notes |
> | Location | Position | |
> | Type | VarInt | ID in the `minecraft:block_entity_type` registry |
> | NBT Data | NBT | Data to set. |»
> — https://minecraft.wiki/w/Java_Edition_protocol/Packets

| Поле | Тип | Заметка |
|---|---|---|
| Location | Position (i64, упаковка x:26 / z:26 / y:12) | позиция блока |
| Type | VarInt | «ID in the `minecraft:block_entity_type` registry» |
| NBT Data | NBT (безымянный корневой компаунд, сетевой формат) | «Data to set.» |

**Номер пакета: `0x06` (6), clientbound, фаза Play.**

Страница вики сейчас документирует протокол **777 (26.3)**, а нам нужен **775
(26.1.2)**. Проверка, что номера не сдвинулись:

- в истории протокола единственное изменение между 775 и 777 — «Added Session ID
  UUID field to Login Success packet» (776), новых clientbound play-пакетов не
  добавляли
  (https://minecraft.wiki/w/Minecraft_Wiki:Projects/wiki.vg_merge/Protocol_History);
- номера clientbound play-пакетов на странице 777 совпадают с теми, что у нас уже
  рабочие на 775: Login (Play) 0x31, Player Info Update 0x46, Keep Alive 0x2C,
  Set Container Content 0x12, Set Held Item 0x69, Block Update 0x08,
  Chunk Data and Update Light 0x2D.

Значит `0x06` для 775 брать можно. Рядом, на всякий случай: Block Action 0x07,
Block Update 0x08.

### 3.2 Блок-сущности внутри пакета чанка

Дословно:

> «Sent when a chunk comes into the client's view distance, specifying its terrain,
> lighting and block entities. … It is not strictly necessary to send all block
> entities in this packet; it is still legal to send them with Block Entity Data later.»

Поля пакета **Chunk Data and Update Light** (`level_chunk_with_light`, **0x2D**):

| Поле | Тип | Заметка (дословно) |
|---|---|---|
| Chunk X | Int | «Chunk coordinate (block coordinate divided by 16, rounded down)» |
| Chunk Z | Int | «Chunk coordinate (block coordinate divided by 16, rounded down)» |
| Heightmaps | Prefixed Array of Heightmap | |
| Data | Prefixed Array of Byte | секции чанка |
| **Block Entities** | **Prefixed Array** (VarInt длина + записи) | см. ниже |
| Light | Light Data | |

Каждая запись массива Block Entities:

| Поле | Тип | Заметка (дословно) |
|---|---|---|
| Packed XZ | Unsigned Byte | «The packed section coordinates are relative to the chunk they are in. Values 0-15 are valid.» |
| Y | Short | «The height relative to the world» |
| Type | VarInt | «The type of block entity» |
| Data | NBT | «The block entity's data, without the X, Y, and Z values» |

Упаковка координат — дословно из вики:

```
packed_xz = ((blockX & 15) << 4) | (blockZ & 15) // encode
x = packed_xz >> 4, z = packed_xz & 15           // decode
```

То есть старший полубайт — X внутри чанка, младший — Z. `Y` — **абсолютная**
высота в мире (со знаком, у нас от −64), short.

**Практический вывод для нас.** Ход поршня длится 2 такта, чанк за это время
заново не шлётся, поэтому во время хода нужен именно **Block Entity Data (0x06)**,
плюс **Block Update (0x08)** на постановку самого блока `moving_piston`. Порядок:
сперва Block Update (иначе клиенту некуда вешать блок-сущность), сразу за ним
Block Entity Data. Массив в пакете чанка нужен только для игроков, которым чанк
приходит впервые посреди хода, — это редкий случай, но формат выписан выше.

---

## 4. Номер типа блок-сущности `minecraft:piston`

**Точного числа на вики нет, и я его не выдумываю.**

- Страница `Java Edition protocol/Registries` перечисляет встроенные реестры,
  включая `minecraft:block_entity_type` («Types of block entities»), но номеров
  записей не даёт — только ссылку на `Block entity format`.
- Страница `Block entity format` (§ Types) перечисляет типы **по алфавиту**, а не
  в порядке реестра, и без номеров.
- Страница `Block entity` группирует блок-сущности **по назначению** (хранят
  предметы / хранят данные / рисуют / тикают), тоже без номеров.
- В `minecraft-data` (PrismarineJS) файла с типами блок-сущностей нет вообще:
  в `data/pc/<версия>/` лежат `blocks.json`, `items.json`, `entities.json`,
  `biomes.json`, … — `blockEntities.json` отсутствует.

Порядок реестра `minecraft:block_entity_type` задаётся порядком регистрации в коде
игры, а его читать нельзя. Поэтому «полный порядок реестра» по разрешённым
источникам восстановить **невозможно**, и подсунуть алфавитный список вместо него
было бы враньём: реестр не алфавитный (в нём, например, `furnace` и `chest` идут
первыми, а не `banner`).

Что есть достоверного:

- строковый id: **`minecraft:piston`**;
- алфавитный список типов, которые вики вообще знает (для проверки полноты, **не
  порядок реестра**): banner, barrel, beacon, beehive, bell, blast_furnace,
  brewing_stand, brushable_block, calibrated_sculk_sensor, campfire,
  chiseled_bookshelf, chest, comparator, command_block, conduit,
  copper_golem_statue, crafter, creaking_heart, daylight_detector, decorated_pot,
  dispenser, dropper, enchanting_table, ender_chest, end_gateway, end_portal,
  furnace, hanging_sign, hopper, jigsaw, jukebox, lectern, mob_spawner, **piston**,
  potent_sulfur, sculk_catalyst, sculk_sensor, sculk_shrieker, shulker_box, sign,
  skull, smoker, soul_campfire, structure_block, trapped_chest, trial_spawner,
  vault (плюс закомментированные на вики test_block и test_instance_block).
  Список — https://minecraft.wiki/w/Block_entity_format (§ Types), актуален для
  текущей версии вики (26.x).

**Как получить число, не нарушая ограничение.** Реестр встроенный, по сети он не
приходит, так что «спросить у клиента» нельзя. Остаётся эмпирика: поставить в мире
`moving_piston`, послать Block Entity Data с перебором Type и посмотреть, при каком
значении клиент нарисует движение (в логе Lunar Client при неправильном типе
обычно ничего не происходит — пакет молча игнорируется). Перебор конечный,
меньше полусотни значений; один раз прогнать и зафиксировать число константой.

**Замечание о том, нужен ли Type вообще.** По протоколу поле обязательно, и клиент
сверяет его с типом блок-сущности, которую он сам создал в этой позиции по блоку
`moving_piston`. Если типы не совпали, данные не применяются. Так что обойтись
«любым» числом не выйдет.

---

## 5. Тайминги

> «When powered, the piston's wooden surface (the "head") tries to start extending
> after a start delay. **The extension takes 2 game ticks (0.1 seconds) to finish.**
> When it extends, it pushes at most 12 blocks.»
> — https://minecraft.wiki/w/Piston

> «When a piston loses power, its head retracts. Like the extension, the retraction
> starts after a start delay and **takes 2 game ticks (0.1 seconds) to finish.**»
> — там же

Задержка **старта** (это отдельно от самого хода):

> «In Java Edition, the start delay can be 0 (activation in the same tick) or 1 game
> tick (activation in the next tick) depending on the game process in which the piston
> is powered:
> * If the piston is powered and updated in the scheduled tick phase, random tick phase
>   or block event phase, the piston activates in this game tick's block event phase,
>   which means the start delay in this case is 0.
> * If the piston is powered and updated during the entity phase or the block entity
>   phase, or by player actions, the piston activates in the next game tick's block
>   event phase, which means the start delay in this case is 1 tick. Notably, because
>   blocks pushed by pistons arrive during the block entity phase (except when dropped
>   by a sticky piston), in a chain of pistons each pushing a redstone block to activate
>   the next, successive activations will happen 3 ticks apart.»
> — https://minecraft.wiki/w/Piston (§ Start delay)

Сводка:

| Момент | `progress` | Что в мире |
|---|---|---|
| фаза блочных событий такта T | — | ставятся `moving_piston` + блок-сущности, `progress = 0.0` |
| фаза блок-сущностей такта T | 0.0 → 0.5 | клиент рисует блок между клетками |
| фаза блок-сущностей такта T+1 | 0.5 → 1.0 | |
| фаза блок-сущностей такта T+2 | 1.0 → превращение | `moving_piston` заменяются настоящими блоками |

Обрати внимание на «blocks pushed by pistons arrive during the block entity phase»:
превращение обратно в обычный блок происходит **в фазе блок-сущностей**, а не в
фазе блочных событий и не в фазе тактов. Это и даёт разницу в 3 такта в цепочке
поршней. У нас фазы такта пока смешаны (см. `audit-updates.md` § 8, § 10), это
придётся чинить вместе с плавным ходом.

**Что видит клиент, если сервер просто поставит блок в конце без блок-сущности.**
Прямого ответа на вики нет (см. § 8), но следствие однозначное: клиент не знает ни
о каком движении и рисует только те блоки, которые ему прислали. Значит
- блок «прыгает» на клетку за один кадр, без анимации;
- голова поршня появляется сразу целиком, без выезда;
- звук и анимация рассинхронизированы;
- а если сервер на 2 такта ставит `moving_piston` **без** блок-сущности — по вики
  этот блок «invisible, has no collisions», то есть игрок на 2 такта увидит
  **дырку** на месте блока, а потом блок появится из ниоткуда. Это хуже
  мгновенной перестановки.

Вывод: ставить `moving_piston` без блок-сущности нельзя — либо и блок, и
блок-сущность, либо (как сейчас) мгновенная перестановка.

---

## 6. Что происходит в конце хода и какие апдейты рассылаются

Превращение делает **сервер**, в фазе блок-сущностей, по условию
`progress >= 1.0` в начале такта (см. § 2.3). Клиент ничего не решает: он получает
Block Update на новый блок (и Block Entity Data, если у нового блока своя
блок-сущность).

Чем заменяется — дословно:

> «At the end of the piston stroke, the *moving piston* blocks are replaced with
> either the carried block, the piston head (during extensions), or the piston
> itself (during retractions)»
> — https://minecraft.wiki/w/Piston/Technical_components

Блок-апдейты — дословно:

> «**Pistons**: during the start of the extension/retraction process, every time a
> moving piston block gets placed and every time one of the normal blocks[^1] is
> replaced/removed, PP updates are sent to these blocks' neighbors. Once **all**
> moving piston blocks have been placed, the game will send NC updates to the
> replaced/removed normal blocks' neighbors.»
>
> «Sticky pistons, however, do not send NC updates around their head when they start
> to pull blocks: only the NC updates caused by the moving piston block turning back
> into a normal block are sent, once the retraction process is over. If a sticky
> piston fails to retract a slime block or a honey block due to the pull limit, no
> updates will be sent around the head whatsoever.»
>
> [^1]: «"Normal block" is intended as not a "moving piston" block.»
> — https://minecraft.wiki/w/Block_update (§ Special blocks)

Расшифровка для кода:

| Момент | Апдейт | Кому |
|---|---|---|
| ставится очередной `moving_piston` | **PP** (post placement / shape update) | шести соседям этой клетки |
| обычный блок заменён/удалён | **PP** | шести соседям этой клетки |
| **все** `moving_piston` поставлены | **NC** (neighbor changed) — одной пачкой | соседям тех клеток, где обычные блоки были заменены/удалены |
| конец хода: `moving_piston` → обычный блок | обычный набор (PP + NC) | соседям |
| липкий поршень, начало втягивания | **NC вокруг головы не шлётся** | — |
| липкий поршень не смог втянуть слизь/мёд из-за лимита | апдейтов вокруг головы **нет вообще** | — |

Разделения PP / NC у нас в проекте нет (`audit-pistons.md` § 11) — без него точный
порядок апдейтов не воспроизвести, но плавный ход это не блокирует: можно сделать
анимацию сейчас, а разделение апдейтов — отдельной задачей.

---

## 7. Готовое дерево NBT: обычный поршень выдвигается на восток и толкает камень

Расстановка. Поршень в `(x, y, z)`, камень в `(x+1, y, z)`, целевая клетка камня
`(x+2, y, z)`.

`facing` = `east` → в блок-сущности **`facing = 5`** (Int).
Поршень обычный → `type = normal`, `source`-запись даёт `piston_head[type=normal]`.

### 7.1 Что ставится в мире в момент начала хода (фаза блочных событий)

```
(x,   y, z) → minecraft:piston[extended=true, facing=east]      — обычный блок, без блок-сущности
(x+1, y, z) → minecraft:moving_piston[facing=east, type=normal] — + блок-сущность A (голова)
(x+2, y, z) → minecraft:moving_piston[facing=east, type=normal] — + блок-сущность B (камень)
```

Клетка `(x+1, y, z)` не становится воздухом: камень оттуда уехал, но её тут же
занимает головная запись.

### 7.2 Блок-сущность A — голова поршня, клетка `(x+1, y, z)`

```
TAG_Compound("") {
    TAG_String("id")        : "minecraft:piston"
    TAG_Int   ("x")         : x+1                  // только на диске; в пакетах не пишется
    TAG_Int   ("y")         : y
    TAG_Int   ("z")         : z
    TAG_Compound("blockState") {
        TAG_String("Name")  : "minecraft:piston_head"
        TAG_Compound("Properties") {
            TAG_String("facing") : "east"
            TAG_String("type")   : "normal"
            TAG_String("short")  : "true"          // см. оговорку в § 8
        }
    }
    TAG_Byte  ("extending") : 1
    TAG_Int   ("facing")    : 5                    // east
    TAG_Float ("progress")  : 0.0f
    TAG_Byte  ("source")    : 1                    // это голова
}
```

### 7.3 Блок-сущность B — перевозимый камень, клетка `(x+2, y, z)`

```
TAG_Compound("") {
    TAG_String("id")        : "minecraft:piston"
    TAG_Int   ("x")         : x+2
    TAG_Int   ("y")         : y
    TAG_Int   ("z")         : z
    TAG_Compound("blockState") {
        TAG_String("Name")  : "minecraft:stone"
        // Properties можно не писать вовсе: у камня свойств нет,
        // а пропущенные берутся по умолчанию
    }
    TAG_Byte  ("extending") : 1
    TAG_Int   ("facing")    : 5                    // east
    TAG_Float ("progress")  : 0.0f
    TAG_Byte  ("source")    : 0                    // это перевозимый блок
}
```

### 7.4 Что улетает игроку

```
1. Block Update (0x08)       (x,   y, z) → состояние piston[extended=true, facing=east]
2. Block Update (0x08)       (x+1, y, z) → состояние moving_piston[facing=east, type=normal]
3. Block Entity Data (0x06)  Location=(x+1,y,z), Type=<номер minecraft:piston>, NBT = A без x/y/z
4. Block Update (0x08)       (x+2, y, z) → состояние moving_piston[facing=east, type=normal]
5. Block Entity Data (0x06)  Location=(x+2,y,z), Type=<номер minecraft:piston>, NBT = B без x/y/z
```

Через 2 такта (в фазе блок-сущностей третьего такта):

```
6. Block Update (0x08)       (x+1, y, z) → состояние piston_head[facing=east, type=normal, short=false]
7. Block Update (0x08)       (x+2, y, z) → состояние stone
```

Блок-сущности при этом удаляются сами (клиент убирает их вместе со сменой блока).

### 7.5 Тот же ход при втягивании (для симметрии)

Обычный поршень не тянет блоки, поэтому при втягивании обычного поршня получается:

```
(x,   y, z) → moving_piston, блок-сущность: blockState = piston[facing=east, extended=false],
                             extending = 0, facing = 5, progress = 0.0, source = 1
(x+1, y, z) → воздух
```

Через 2 такта `(x, y, z)` становится `minecraft:piston[facing=east, extended=false]`.

У липкого поршня в `(x+1, y, z)` вместо воздуха ставится `moving_piston` с
`source = 0` и `blockState` = притягиваемый блок, `extending = 0`, и `facing` — это
направление **движения**, то есть `west` / 4, а не `east` / 5 (см. оговорку в § 8).

---

## 8. Чего на вики нет

Всё перечисленное ниже вики **не** утверждает; либо это выведено косвенно, либо
проверять придётся опытом.

1. **Числовой номер `minecraft:piston` в реестре `minecraft:block_entity_type`.**
   Ни числа, ни порядка реестра ни на одной странице вики нет; в `minecraft-data`
   таких данных тоже нет. Восстановить порядок по разрешённым источникам нельзя.
   Единственный путь — перебор в тесте (§ 4).

2. **Формат `blockState` в NBT.** Вики подставляет на страницу поршня шаблон
   датапакового (JSON) формата с полями `id` / `properties` в нижнем регистре, что
   противоречит её же странице `Chunk format` (`Name` / `Properties`) и её же
   примеру команды с `BlockState:{Name:"minecraft:cactus"}`. Я выбрал
   `Name` / `Properties` как единственный вариант, подтверждённый примером
   реального NBT, но **это надо проверить в игре** — если клиент не покажет
   движение, первым делом пробовать `id` / `properties`.

3. **В какие именно клетки ставятся `moving_piston`.** Вики говорит только
   «multiple … depending on how many blocks the piston is moving» и чем они
   заменяются. Раскладка «целевая клетка каждого блока + клетка головы» выведена
   из фразы про замену и из того, что блок-сущность хранит целевую клетку.

4. **Порядок постановки блоков внутри хода** (от поршня наружу или наоборот).
   Косвенно из `Block update` следует только, что все `moving_piston` ставятся до
   пачки NC-апдейтов.

5. **`short` у `piston_head` внутри `blockState` головной записи.** Свойство на
   вики описано («the piston arm is shorter than usual, by 4 pixels»), но нигде не
   сказано, каким оно должно быть у *движущейся* головы. По смыслу во время
   выдвигания рука ещё не доехала, поэтому `short = true`, а в конце хода ставится
   `short = false`. Проверять глазами.

6. **Значение `facing` у `moving_piston` и в блок-сущности при втягивании.** Поле
   описано как «Direction that the piston pushes», а при втягивании поршень ничего
   не толкает. Логично, что это направление движения блока (то есть обратное
   направлению поршня), но дословно вики этого не говорит.

7. **Как клиент считает смещение по `progress`.** Формула отрисовки (линейно ли,
   с интерполяцией между тактами, как именно) на вики не описана — это чисто
   клиентская часть, нам её знать и не нужно.

8. **Что именно видит клиент, если блок поставить без блок-сущности.** Прямого
   утверждения нет; вывод в § 5 сделан из описания свойств `moving_piston`,
   поставленного командой («invisible, has no collisions»).

9. **Нужно ли посылать `id` внутри NBT в сетевых пакетах.** Протокол этого не
   требует (тип идёт отдельным VarInt), но и не запрещает. Мы кладём — безопаснее.

10. **Поведение при выходе чанка из зоны видимости посреди хода** и при сохранении
    мира с незавершённым ходом (`keepPacked`) — на вики не описано.

---

## Подтверждение ограничения

При подготовке этого файла **не использовались** ни оригинальный код Minecraft (в
том числе декомпилированный), ни `client.jar` / `server.jar`, ни ванильные
датапаки, ни код неофициальных серверных ядер и модов (Paper, Spigot, Fabric,
Forge, Bukkit, ViaVersion и любых других). Источники — только страницы
minecraft.wiki (перечислены в шапке) и перечень файлов репозитория данных
`PrismarineJS/minecraft-data` (для проверки того, что таблицы типов блок-сущностей
там нет).
