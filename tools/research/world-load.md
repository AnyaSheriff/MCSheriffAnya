# Сохранение и загрузка мира: почему в игре редстоун не «залипает»

Разбор по документации (minecraft.wiki, Mojira, разборы техсообщества) для
Java Edition 26.1.2 / протокол 775. Всё, что в кавычках — дословные цитаты
с указанных страниц; перевод рядом — мой.

Источники перечислены в конце. **Исходный код игры и серверных ядер не
использовался** — только вики, баг-трекер и текстовые разборы.

Смежные файлы: `tools/research/moving-piston.md` (как показать ход поршня),
`tools/research/redstone-pistons.md`, `tools/research/audit-pistons.md`,
`tools/research/redstone-core.md`.

---

## 1. Что сохраняется вместе с чанком

### 1.1 Корень чанка (NBT) — то, что касается редстоуна

Страница [Chunk format](https://minecraft.wiki/w/Chunk_format), раздел с корневыми тегами:

| Тег | Тип | Дословно с вики | Отношение к редстоуну |
|---|---|---|---|
| `block_entities` | list of compound | "Each Compound in this list defines a block entity in the chunk." | сюда попадает `minecraft:piston` (moving piston) и компаратор |
| `block_ticks` | list of compound | "Each Compound in this list is an \"active\" block in this chunk waiting to be updated. **These are used to save the state of redstone machines or falling sand, and other activity.**" | **ключевое**: запланированные такты редстоуна |
| `fluid_ticks` | list of compound | "Each Compound in this list is an \"active\" liquid in this chunk waiting to be updated." | жидкости |
| `PostProcessing` | list of 24 lists | "A List of 24 Lists that store the positions of blocks that need to receive an update when a *proto-chunk* turns into a full chunk, packed in Shorts." | **только для прото-чанков** (генерация), не для обычной загрузки |
| `Status` | string | "Defines the world generation status of this chunk." | `minecraft:full` — чанк полностью сгенерирован |
| `InhabitedTime` | long | — | не относится |

Переименования (раздел History той же страницы), полезно для понимания истории формата:

> "`Level.TileEntities` has moved to `block_entities`."
> "`Level.TileTicks` and `Level.ToBeTicked` have moved to `block_ticks`."
> "`Level.LiquidTicks` and `Level.LiquidsToBeTicked` have moved to `fluid_ticks`."

### 1.2 Формат запланированного такта (Tile tick format)

Вступление раздела [Chunk format § Tile tick format](https://minecraft.wiki/w/Chunk_format#Tile_tick_format) — это,
по сути, прямой ответ на наш вопрос:

> "Tile Ticks represent block updates that need to happen because they could not happen before the chunk was saved. Examples reasons for tile ticks include **redstone circuits needing to continue updating**, water and lava that should continue flowing, recently placed sand or gravel that should fall, etc. Tile ticks are not used for purposes such as leaf decay, where the decay information is stored in the leaf block data values and handled by Minecraft when the chunk loads. For map makers, tile ticks can be used to update blocks after a period of time has passed with the chunk loaded into memory."

Перевод смысла: tile tick — это **отложенное обновление, которое не успели
выполнить до сохранения чанка**. Оно сохраняется, чтобы схема «доиграла»
после загрузки. А то, что можно вычислить из состояния блока (как распад
листвы), в tile tick не пишут — оно выводится при загрузке.

Поля одного такта:

| Поле | Тип | Дословно с вики | Что это для нас |
|---|---|---|---|
| `i` | string | "The ID of the block; used to activate the correct block update procedure." | id блока (`minecraft:repeater` и т.п.); при загрузке проверяется, что блок на месте не сменился |
| `p` | int | "If multiple tile ticks are scheduled for the same tick, tile ticks with lower p are processed first. If they also have the same p, the order is unknown." | приоритет; те самые −3/−2/−1/0 |
| `t` | int | "The number of ticks until processing should occur. **May be negative when processing is overdue.**" | **относительное** время, не абсолютное |
| `x` | int | "X position" | абсолютная координата блока |
| `y` | int | "Y position" | |
| `z` | int | "Z position" | |

Два вывода, важные для нашей реализации:

1. **`t` относительный.** В памяти игра хранит абсолютный игровой такт, при
   записи вычитает текущее время мира. Поэтому чанк, пролежавший на диске
   неделю, после загрузки доигрывает свои такты «как будто прошло 0 тактов».
   Отрицательный `t` = такт просрочен (чанк был выгружен, когда срок уже
   наступил) — его нужно выполнить немедленно, на первом же такте.
2. **`p` сохраняется.** То есть игра считает порядок выполнения частью
   состояния, а не пересчитывает его при загрузке. Значения приоритетов
   (страница [Tick § Scheduled tick](https://minecraft.wiki/w/Tick#Scheduled_tick)):

   > "Block ticks are executed first based on priority, and then based on scheduling order. A lower value for priority results in earlier execution during the scheduled tick phase. If a redstone repeater is facing the back or side of another diode, its block tick has a priority of -3. If a redstone repeater is depowering, it has a priority of -2. Otherwise, the repeater has a priority of -1. If a redstone comparator is facing the back or side of another diode, it has a priority of -1. All other block ticks have a priority of 0. Then, each block with a scheduled fluid tick get a tick. Fluid ticks do not use priorities and are ordered based on scheduling order."

   > "In Java Edition, the maximum number of scheduled ticks per game tick is 65,536."

### 1.3 Блочные события (block events) — НЕ сохраняются

В формате чанка **нет** поля для блочных событий. Прямой поиск по странице
Chunk format по словам *event* / *action* не даёт ни одного совпадения: там
есть `block_entities`, `block_ticks`, `fluid_ticks`, `PostProcessing`,
`Heightmaps`, `structures`, `blending_data`, `Lights`, `CarvingMasks` — и всё.

Это отрицательный факт (отсутствие поля), поэтому подкрепляю его вторым
источником — разбором техсообщества (Technical Minecraft Wiki,
*Piston Mechanics*), который прямо называет это багом:

> "If the piston is unloaded before the blockEvents get processed, it will no longer process the update (this is a bug)."

То есть в игре **блочное событие поршня живёт только в оперативной памяти
одного игрового такта**. Если чанк выгрузится между постановкой события и
его обработкой — событие теряется навсегда. Это и есть механизм появления
«безголовых» поршней и вечных `moving_piston` в ванилле (см. § 3.2).

Фаза блочных событий существует в самом такте — страница
[Piston § Start delay](https://minecraft.wiki/w/Piston):

> "0 ticks if powered during the scheduled tick, random tick or block event phase; 1 tick if powered during the entity or block entity phase, or during player input handling."

и страница [Tick § Piston tick](https://minecraft.wiki/w/Tick):

> "The piston tick starts in the entity phase of the game tick and ends in the block event phase, therefore a piston always takes 3 piston ticks to extend or retract, without any start delay."

### 1.4 Блок-сущность движущегося поршня

Страницы [Block entity format](https://minecraft.wiki/w/Block_entity_format) и
[Piston/Technical components § Moving piston](https://minecraft.wiki/w/Piston/Technical_components).
Идентификатор блок-сущности — `minecraft:piston` (в списке id на странице
Block entity format строка `piston` → *Moving Piston*).

| Поле | Тип | Дословно с вики |
|---|---|---|
| `blockState` | compound | "The moving block represented by this block entity." |
| `extending` | byte | "1 or 0 (true/false) – true if the piston is extending instead of withdrawing." |
| `facing` | int | "Direction that the piston pushes (0=down, 1=up, 2=north, 3=south, 4=west, 5=east)." |
| `progress` | float | "How far the block has been moved. Starts at 0.0, and increments by 0.5 each tick, including the tick on which moving the piston is created. If the value is 1.0 or higher at the *start* of a tick of the block entity (before incrementing), then the block transforms into the stored blockState. Negative values can be used to increase the time until transformation." |
| `source` | byte | "1 or 0 (true/false) – true if the block represents the piston head itself, false if it represents a block being pushed." |

Сам блок `minecraft:moving_piston` (состояния блока):

| Состояние | Значения | Дословно |
|---|---|---|
| `facing` | down, east, north, south, up, west | "The direction the block is being pushed by the piston." |
| `type` | normal, sticky | "What piston base this has." |

Описание блока:

> "The **moving piston** (in JE) or **moving block** (in BE), also known as **block 36** due to its pre-flattening block ID, is an unobtainable technical block that holds a block entity (unless it was placed with a command) which contains the block the piston is currently moving. Since moving blocks vary in how much of each grid cell they occupy, they can't be stored as normal blocks and are instead stored as block entity data. Multiple *moving piston* blocks may be used during the extension/retraction process, depending on how many blocks the piston is moving. **At the end of the piston stroke, the *moving piston* blocks are replaced with either the carried block, the piston head (during extensions), or the piston itself (during retractions); but if it is placed with the use of commands it remains indefinitely.**"

И про «пустой» moving_piston без блок-сущности (именно в такой мы рискуем
превратить свои служебные блоки):

> "When placed by commands, the game does not assign a block entity to the *moving piston* block, therefore its properties are different than usual: it's invisible, has no collisions, and cannot be broken without the use of TNT, commands, or a structure generating over it (such as the end platform). Although it is non-solid, fluids cannot pass through it. It also prevents players from building at its location."

### 1.5 Состояние редстоуна в самих блоках

Ключевой момент: **весь «логический» редстоун игра держит в состоянии блока**
(block state), которое сохраняется как часть палитры секции чанка. Отдельного
«редстоун-хранилища» нет.

| Блок | Состояния (с вики) | Дословное описание |
|---|---|---|
| `redstone_wire` | `power` 0..15 | "The redstone dust's current power level." |
| | `north`/`south`/`east`/`west` = none, side, up | "The way redstone dust connects to the north, side can also mean down." |
| `repeater` | `powered` false/true | "If the redstone repeater is lit." |
| | `locked` false/true | "True if the repeater is currently locked." |
| | `delay` 1..4 | "The redstone repeater's delay in redstone ticks (double game ticks)." |
| `comparator` | `powered` false/true | "True if the redstone comparator is being powered." |
| | `mode` compare/subtract | "Specifies the current mode of the redstone comparator." |
| `redstone_torch` | `lit` false/true | "If the torch is lit." |
| `piston` | `extended` false/true | "If true, the piston is extended." |
| | `facing` down..west | "The direction the piston head is pointing." |

Дополнительно компаратор имеет блок-сущность (страница Redstone Comparator,
§ Block data):

> "`OutputSignal`: Represents the strength of the analog signal output of this redstone comparator."

Итого — **всё нужное для редстоуна сохраняется**: уровень сигнала провода, состояние
диодов, выход компаратора, положение поршня, очередь отложенных тактов.
Не сохраняется только очередь блочных событий текущего такта.

---

## 2. Что происходит при загрузке чанка

### 2.1 Пересчёта нет. Игра доверяет сохранённому состоянию

В документации **нигде не описан пересчёт редстоуна при загрузке чанка**.
Наоборот, вики прямо противопоставляет два способа: то, что нужно доиграть,
кладут в `block_ticks`, а то, что выводимо из состояния блока — не кладут:

> "Tile ticks are not used for purposes such as leaf decay, where the decay information is stored in the leaf block data values and handled by Minecraft when the chunk loads."

Механика такая:

1. Блоки восстанавливаются из палитры **как есть**: `redstone_wire[power=9]`
   загрузится со значением 9, даже если рядом уже ничего не светится.
   Никакой проверки «а откуда 9?» не делается.
2. Блок-сущности восстанавливаются из `block_entities` как есть.
3. `block_ticks` заряжаются обратно в очередь планировщика: абсолютный такт =
   текущее время мира + `t`. Отрицательный `t` → срок уже вышел → выполнится
   на ближайшем такте.
4. Блочные события — пусты (их не было в файле).

Никакого «пробуждения» редстоуна, никакого массового `neighborChanged` по
чанку в документации не описано. Единственный документированный механизм
«разбудить блоки при загрузке» — это `PostProcessing`, и он, по вики, про
генерацию, а не про обычную загрузку:

> "A List of 24 Lists that store the positions of blocks that need to receive an update **when a *proto-chunk* turns into a full chunk**, packed in Shorts."

> "When converting *proto-chunks* to full chunks, only coordinates that are stored in PostProcessing appear to receive a tick update, tick updates stored in block_ticks and fluid_ticks are ignored." *(на вики помечено `{{verify}}`)*

Почему это работает: **состояние согласованно по построению**. Провод со
`power=9` был честно запитан в момент сохранения, и источник этой мощности
тоже сохранён — либо как блок (факел `lit=true`, повторитель `powered=true`),
либо как отложенный такт в `block_ticks`. Игре не нужно ничего пересчитывать,
потому что она не теряет ни одной из этих частей.

Косвенное подтверждение того, что пересчёта нет, — багрепорты. Если бы игра
пересчитывала провода при загрузке, не было бы жалоб вида (MC-151049):

> "repeaters staying on forever when not powered"

### 2.2 Заметка: «форматное» ограничение только на загрузке чанка

Страница [Chunk](https://minecraft.wiki/w/Chunk):

> "Unloaded chunks are unprocessed by the game and do not process any of the game aspects."

> "The game always loads the entire chunk when it decides a chunk needs to be processed."

И про то, какие чанки вообще тикают (§ Ticking):

> "Chunk loading caused by a player ticket allows block entities to be ticked and scheduled tick and chunk tick (without mob spawning) to be processed only if the chunk is in the aforementioned square region along with a one-chunk-thick square frame surrounding this region."

Практически: чанк может быть **загружен, но не тикать** («lazy chunk»):

> "All game aspects are available except that entities are not naturally spawned and are not processed (but are still accessible) and chunk ticks aren't processed either. These are sometimes referred to as \"lazy chunks\"."

Для нас это значит: если мы решим «чинить редстоун при загрузке», мы будем
делать это и для чанков, которые в ванилле вообще не должны ничего считать.

### 2.3 Поведение `moving_piston` после загрузки

Прямой цитаты «после загрузки блок-сущность поршня доигрывает ход» в вики нет.
Но из описания поля `progress` следует, что доигрывает — при условии, что
чанк тикает:

> "How far the block has been moved. Starts at 0.0, and increments by 0.5 each tick, including the tick on which moving the piston is created. If the value is 1.0 or higher at the *start* of a tick of the block entity (before incrementing), then the block transforms into the stored blockState."

Блок-сущность движущегося поршня — тикающая. `progress` — часть её
сохраняемых данных. Значит после загрузки она продолжает считать с того же
`progress`, дойдёт до 1.0 и превратится в `blockState` (или в голову поршня,
если `source=1`). Ход занимает 2 игровых такта (`progress` 0.0 → 0.5 → 1.0);
0.5 такта «потерять» при сохранении невозможно, потому что `progress` — float
и пишется как есть.

**Поэтому в ванилле поршень, застигнутый посреди хода, после загрузки
доигрывает ход сам** — не нужно ни чинить, ни завершать вручную. Ломается
это только тогда, когда потерялось *блочное событие*, которое должно было
поставить `moving_piston` (§ 3.2), а не когда потерялся `progress`.

---

## 3. Согласованность записи

### 3.1 Чанк и его такты — один файл, одна запись

`block_ticks` и `fluid_ticks` — это **теги внутри NBT самого чанка**, не
отдельный файл и не отдельная база. Страница
[Region file format](https://minecraft.wiki/w/Region_file_format):

> "Each file stores a group of 32×32 chunks called a region."

> "The uncompressed data is in NBT format and follows the information detailed on the chunk format article."

Отсюда главный архитектурный вывод для нас:

> **Атомарность получается бесплатно.** Чанк и его отложенные такты
> записываются одним блобом в один сектор региона. Невозможно записать
> блоки без тактов или такты без блоков — они физически одна запись.

Игра дополнительно требует выравнивания:

> "Minecraft always pads the last chunk's data to be a multiple-of-4096B in length"
> "Minecraft does not accept files in which the last chunk is not padded."
> "If the file isn't aligned to this boundary there is a possibility that it is corrupt"

Отдельно оговорка: сущности **вынесены** в свой региональный файл
(`entities/`), а блок-сущности — нет, они в чанке. Chunk format про тег
`entities`:

> "A list of entities in the *proto-chunks*, used when generating. This list is not present for fully generated chunks and entities are moved to a separated region files once the chunk is generated."

То есть сущности могут разъехаться с чанком при краше, а блок-сущности и
такты — нет.

### 3.2 Что бывает при аварийном завершении и при выгрузке

Документированных гарантий «сначала fsync, потом обновить заголовок региона»
на вики нет (см. § 5, «чего в документации нет»). Зато документирован
механизм блокировки уровня — [Java Edition level format § session.lock](https://minecraft.wiki/w/Java_Edition_level_format):

> "Program opens session.lock."
> "Program writes a single character ☃ (☃) to session.lock."
> "Program tries to acquire a lock on a session.lock."
> "If the lock on a session.lock fails, program aborts and gives up its lock on the level."

И резервная копия глобального файла мира:

> "When a world is loaded, the current `level.dat` is backed up to `level.dat_old`."

Для регионов такого бэкапа нет.

**Известные баги «залипшего» редстоуна — все они про потерю тактов или
событий при выгрузке, а не про повреждение файла.** Основные тикеты:

| Тикет | Заголовок | Суть | Статус |
|---|---|---|---|
| [MC-711](https://mojira.dev/MC-711) | "Tile ticks of connected redstone components might be executed in the wrong order when unloading/reloading chunks" | схема на границе чанков: один чанк загружается раньше другого, его tile tick отсчитывается и выполняется, пока второй ещё не загружен | Reopened, не исправлен; в отчётах доходит до 1.20.4 |
| [MC-151049](https://mojira.dev/MC-151049) | "Redstone updates pile up when chunks are not loaded" | обновления копятся, а при загрузке чанка выполняются все разом — получаются нулевые такты, повторители залипают включёнными | Reopened / Unresolved |
| [MC-134979](https://mojira.dev/MC-134979) | "Extended sticky piston at border of unloaded chunk gets stuck permanently extended" | липкий поршень тянет блок через границу чанка, соседний чанк выгружен — поршень остаётся вечно выдвинутым | Cannot Reproduce (в 20w06a) |
| [MC-27056](https://mojira.dev/MC-27056) | баг безголового поршня | база поршня остаётся `extended=true` без головы; используется игроками намеренно | — |

Цитаты из MC-711:

> "when you move away the chunks unload and so do the tile ticks of the repeaters. When you move closer again... the chunk with the repeater that wants to turn on gets loaded first, the tile tick counts down and gets executed while the other repeater was not loaded the entire time."

> "Problem turned out to be that when ticks are scheduled, they are thrown away if the neighboring chunks haven't been loaded yet."

Из MC-151049:

> "the scheduled redstone updates are apparently all queued for that block until I load the chunk"

и жалоба репортёра:

> "repeaters staying on forever when not powered"

**Главный вывод раздела: в игре ЭТО ТОЖЕ БЫВАЕТ.** Утверждение «в игре
такого не бывает» строго говоря неверно — вечно выдвинутые поршни и
залипшие повторители после перезагрузки чанков задокументированы в Mojira и
не исправлены. Разница в том, что в ванилле это **редкий краевой случай**
(граница чанков, выгрузка ровно между событием и его обработкой), а у нас —
воспроизводится при каждом перезапуске, потому что мы теряем то, что игра
сохраняет.

### 3.3 Как их «чинили»

Единого исправления нет. Что видно по тикетам и истории:

- Часть случаев с «двухголовыми» / застрявшими поршнями чинилась в 1.9
  (комментарий к MC-134979: репортёр ссылается на исправление "persistent
  double headed pistons" в 1.9, замечая, что «the underlying problem with
  block events during chunk loading may not have been fully resolved»).
- MC-134979 закрыт как Cannot Reproduce — симптом исчез, причина (потеря
  блочных событий) осталась.
- MC-711 и MC-151049 открыты до сих пор.
- Headless piston (MC-27056) не чинят намеренно-долго, потому что на нём
  построены схемы; вики предупреждает:
  > "Bugs of this nature may be fixed at any time without warning, causing the contraption to stop working."

Игроки чинят залипшие схемы **обновлением блока рядом** — поставить и сломать
блок у залипшего компонента. Это работает именно потому, что состояние
провода/диода хранится в блоке и перечитывается только при block update:
пока обновления нет, неправильное значение живёт вечно. То же следует из
описания headless piston:

> "Powering it will ensure that it stays headless. Once unpowered, it will retract, gain its head back"

— то есть база поршня остаётся в несогласованном виде до следующего
изменения питания.

---

## 4. Рекомендация для нас

### 4.1 Что именно у нас сломано

Сопоставление симптомов с моделью игры:

| Наш симптом | Что это значит |
|---|---|
| провод светится без источника | либо мы **не сохраняем** `power` и восстанавливаем его мусором, либо сохраняем `power`, но **теряем `block_ticks`** источника (факел/повторитель не доиграл), либо теряем состояние источника |
| поршень без головы | потеряно блочное событие / не сохранена блок-сущность `moving_piston` с `source=1`, при этом база сохранена с `extended=true` |
| `moving_piston` лежит в чанке вечно | сохранён блок `moving_piston`, но **не сохранена** его блок-сущность (или её `progress`), поэтому некому досчитать до 1.0 и превратиться в `blockState` |

Все три — это симптомы **неполного сохранения**, а не отсутствия пересчёта.
Игра не чинит — игра не теряет.

### 4.2 Путь (а): хранить состояние согласованно, как игра — рекомендуется

Что нужно сделать, по пунктам:

1. **Сохранять `block_ticks` в чанке**, в формате `{i, p, t, x, y, z}`,
   с `t` = `абсолютный_такт − текущее_время_мира` (может быть отрицательным).
   При загрузке: `абсолютный_такт = текущее_время_мира + t`.
2. **Сохранять `p`**, а не вычислять заново. Приоритет — часть состояния.
3. **Писать такты в тот же блоб, что и блоки чанка.** Один NBT — одна запись
   в регион. Это даёт атомарность без всяких усилий.
4. **Сохранять блок-сущность `minecraft:piston`** со всеми пятью полями,
   включая `progress` как float. После загрузки она сама доиграет ход.
5. **Сохранять `power` провода, `powered`/`locked` диодов, `lit` факела,
   `OutputSignal` компаратора, `extended` поршня** — то есть всю палитру
   состояний блока без «нормализации».
6. **Ничего не пересчитывать при загрузке.** Никакого массового
   `neighborChanged` по чанку — в ванилле его нет, и он ломает схемы
   (нулевые такты, лавины обновлений — ровно то, на что жалуются в
   MC-151049).
7. **Блочные события не сохранять** — как в игре. Но чтобы не наступать на
   ванильные грабли, см. § 4.4.

Чем грозит: строго говоря, ничем нежелательным — это ровно поведение игры.
Цена — дисциплина записи: ни один из перечисленных кусков нельзя потерять.
Если мы сохраним блоки, но не такты, станет **хуже**, чем сейчас: провод
будет светиться, а ничто не будет его гасить.

### 4.3 Путь (б): чинить при загрузке — не рекомендуется как основной

Что было бы: пересчитать `power` всех проводов, снести `moving_piston`,
доиграть ход поршня, привести `extended` в соответствие с наличием головы.

Чем грозит:

- **Ломает схемы игроков.** Пересчёт проводов при загрузке = массовое
  обновление = нулевые такты и лавины, ровно как в MC-151049. Любые часы,
  триггеры, защёлки и T-флип-флопы сбросятся в непредсказуемое состояние.
- **Пересчёт провода невозможно сделать правильно локально.** Мощность
  провода зависит от источников, которые могут лежать в незагруженном
  соседнем чанке. Придётся либо грузить соседей (каскад загрузок), либо
  считать неправильно.
- **Компараторы и повторители не пересчитываются в принципе** — их
  `powered`/`OutputSignal` зависит от прошлого (задержка, режим,
  содержимое сундука в другом чанке). Пересчитать их «из ничего» нельзя.
- **Снос `moving_piston` теряет блок.** Блок, который поршень нёс, лежит
  только в `blockState` блок-сущности. Снесли — потеряли.
- Расходимся с игрой в поведении → схемы из туториалов у нас работают иначе.

Отдельно: путь (б) выглядит привлекательным потому, что он «лечит симптом
не трогая сохранение». Но он лечит именно симптом; при этом каждая схема
сложнее фонарика начинает вести себя не так, как в игре.

### 4.4 Рекомендуемый гибрид: (а) + узкая аварийная уборка

Берём (а) полностью, и добавляем **только одну** страховку — не пересчёт,
а уборку явно невозможных состояний, которых в согласованном мире быть не
может:

- `moving_piston` **без блок-сущности** → это мусор (в ванилле такой блок
  появляется только через команды). Заменить на воздух при загрузке.
  Это не пересчёт редстоуна, это сбор мусора.
- `moving_piston` **с блок-сущностью** → не трогать, дать доиграть.
- `piston[extended=true]` **без головы и без `moving_piston` перед собой** →
  спорный случай: в ванилле это легальный headless piston. **Предлагаю не
  трогать** (иначе сломаем воспроизведение ванильного бага, на котором
  игроки строят схемы), но логировать на уровне debug, чтобы мы видели, если
  такие появляются сами по себе — это был бы признак нашей ошибки.
- Провода/диоды — **не трогать никогда**.

И отдельно, чтобы избежать граблей MC-711 у себя: когда мы ставим
отложенный такт в соседний чанк, который ещё не загружен, **нельзя его
выбрасывать** («they are thrown away if the neighboring chunks haven't been
loaded yet»). Либо загружаем чанк, либо кладём такт в отложенную очередь
уровня. Это как раз то место, где мы можем сделать лучше, чем ванилла, ничего
не ломая.

### 4.5 Порядок работ

1. `block_ticks` в сохранение/загрузку чанка (`i`, `p`, `t`, `x`, `y`, `z`),
   `t` относительный, отрицательный `t` выполняется немедленно.
2. Блок-сущность `minecraft:piston` в `block_entities` со всеми полями.
3. Проверить, что палитра секции сохраняет все состояния редстоуна без потерь
   (`power`, `powered`, `locked`, `lit`, `extended`, `mode`).
4. `OutputSignal` компаратора.
5. Уборка `moving_piston` без блок-сущности.
6. Отложенные такты в незагруженные чанки — не выбрасывать.
7. Убрать любой имеющийся у нас пересчёт редстоуна при загрузке, если он есть.

---

## 5. Чего в документации нет

Честный список того, что я **не нашёл** и не стал додумывать:

1. **Нет описания порядка записи региональных файлов и гарантий при краше.**
   Region file format описывает структуру (сектора 4 КиБ, заголовок, padding),
   но не говорит, пишется ли сначала данные, потом заголовок, делается ли
   fsync, есть ли временный файл. Единственное упоминание — что невыровненный
   файл «possibly corrupt». Про `level.dat_old` сказано, про бэкапы регионов —
   ничего.
2. **Нет явного утверждения «при загрузке чанка редстоун не пересчитывается».**
   Это вывод из отсутствия такого механизма в документации + из формулировки
   про tile ticks + из багрепортов, а не прямая цитата.
3. **Нет явного описания, что блок-сущность `moving_piston` доигрывает ход
   после загрузки чанка.** Вывод сделан из описания `progress` (поле
   сохраняемое, инкремент по тактам блок-сущности).
4. **Нет документации, что блочные события не сохраняются.** Это вывод из
   отсутствия соответствующего тега в формате чанка, подтверждённый
   разбором техсообщества («this is a bug») и симптомами в MC-134979.
5. **Не задокументировано, что происходит с tile tick, если блок по
   координатам сменился.** Поле `i` («used to activate the correct block
   update procedure») намекает на проверку, но как именно — не сказано.
6. **Раздел `PostProcessing` / `ToBeTicked` на вики помечен `{{verify}}`**:
   «Further testing needed for confirmation». Утверждение, что при
   превращении прото-чанка в полный такты из `block_ticks` игнорируются,
   не подтверждено.
7. **Нет описания, как игра ведёт себя с отрицательным `t`** — что именно
   значит «processing is overdue» на практике (все просроченные разом в
   первом же такте? по приоритету?). Порядок внутри одного такта известен
   (`p`, потом порядок постановки), но про «навалом просроченных» ничего.
8. **Нет данных о том, сохраняются ли блочные события в Bedrock** — там
   другой формат, я его не смотрел.
9. **MC-134979 закрыт как Cannot Reproduce** — значит нет подтверждённого
   описания, как именно это было починено и починено ли.

---

## Источники

Вики (minecraft.wiki):

- https://minecraft.wiki/w/Chunk_format — корневые теги чанка, § Tile tick format, § ToBeTicked format, § History
- https://minecraft.wiki/w/Block_entity_format — список id блок-сущностей, `piston`
- https://minecraft.wiki/w/Piston/Technical_components — § Moving piston, § Block states, § Block data
- https://minecraft.wiki/w/Piston — § Block states, § Start delay
- https://minecraft.wiki/w/Tutorial:Headless_pistons
- https://minecraft.wiki/w/Tick — § Scheduled tick, § Piston tick
- https://minecraft.wiki/w/Chunk — § Chunk loading, § Ticking, § Ticket types
- https://minecraft.wiki/w/Region_file_format
- https://minecraft.wiki/w/Java_Edition_level_format — § session.lock format, level.dat_old
- https://minecraft.wiki/w/Redstone_Dust, https://minecraft.wiki/w/Redstone_Repeater,
  https://minecraft.wiki/w/Redstone_Comparator, https://minecraft.wiki/w/Redstone_Torch — § Block states, § Block data

Баг-трекер Mojang (Mojira):

- https://mojira.dev/MC-711 — "Tile ticks of connected redstone components might be executed in the wrong order when unloading/reloading chunks"
- https://mojira.dev/MC-151049 — "Redstone updates pile up when chunks are not loaded"
- https://mojira.dev/MC-134979 — "Extended sticky piston at border of unloaded chunk gets stuck permanently extended"
- https://mojira.dev/MC-263413 — "Redstone bug, Piston magically powered" (оказался дубликатом MC-108, квазиподключение — к нашей теме отношения не имеет, приведён чтобы отсечь)
- MC-27056 — баг безголового поршня (ссылка с вики Tutorial:Headless pistons)

Разборы техсообщества:

- Technical Minecraft Wiki, "Piston Mechanics" — https://technical-minecraft.fandom.com/wiki/Piston_Mechanics
  (цитата про потерю blockEvents при выгрузке)
- Minecraft Discontinued Features Wiki, "Java Edition:Moving Piston (Block)" — https://mcdf.wiki.gg/wiki/Java_Edition:Moving_Piston_(Block)

---

## Подтверждение ограничения

При подготовке этого документа **не использовался** исходный код Minecraft
(в том числе декомпилированный), client.jar / server.jar, ванильные датапаки,
а также исходный код серверных ядер, прокси и библиотек (Paper, Spigot,
Fabric, Forge, Bukkit, ViaVersion, node-minecraft-protocol, PrismarineJS
и любых других реализаций). Использованы только minecraft.wiki, официальный
баг-трекер Mojang (Mojira) и текстовые разборы техсообщества.
