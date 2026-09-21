# Пакеты хода поршня — протокол 775 (26.1.x)

Разбор только про **сами пакеты**: номера, поля, порядок байт, порядок отправки.
Механика и такты — в `piston-timing.md`, обязанности сервера против клиента —
в `piston-protocol.md`; здесь они упоминаются лишь там, где от них зависит, что
и когда уходит в сокет.

Версия: Minecraft 26.1.x, протокол **775**. Живая страница вики сейчас описывает
**777 (26.3)**, поэтому номера пакетов сверены с данными
`PrismarineJS/minecraft-data`, `data/pc/26.1` (там `version.json` →
`{"version": 775, "minecraftVersion": "26.1"}`), а раскладка полей — с вики.
Во всех разобранных пакетах раскладка между 775 и 777 одинаковая.

---

## 1. Block Action (`block_event`)

Номер в 775: **0x07** (clientbound, Play). Проверено по
`data/pc/26.1/protocol.json`: `0x07 → block_action`.

Дословно с вики (`Java Edition protocol/Packets`, раздел *Block Action*):

> This packet is used for a number of actions and animations performed by
> blocks, usually non-persistent. The client ignores the provided block type and
> instead uses the block state in their world.

> **Warning:** This packet uses a block ID from the `minecraft:block` registry,
> not a block state.

### Поля

| Поле | Тип | Байт | Что внутри |
|---|---|---|---|
| Location | Position | 8 | «Block coordinates.» — упакованные x/z/y, см. ниже |
| Action ID (Byte 1) | Unsigned Byte | 1 | «Varies depending on block» — у поршня 0/1/2 |
| Action Parameter (Byte 2) | Unsigned Byte | 1 | «Varies depending on block» — у поршня сторона |
| Block Type | VarInt | 1–5 | «ID in the `minecraft:block` registry. This value is unused by the vanilla client, as it will infer the type of block based on the given position.» |

**Последнее число — номер БЛОКА, не состояния.** Ванильный клиент его вообще не
читает (тип блока он берёт из своего мира по координатам), но слать корректный
номер всё равно правильно.

### Position

Дословно (`Java Edition protocol/Data types`, раздел *Position*):

> 64-bit value split into three **signed** integer parts:
> * x: 26 MSBs
> * z: 26 middle bits
> * y: 12 LSBs
>
> Encoded as follows:
>
>     ((x & 0x3FFFFFF) << 38) | ((z & 0x3FFFFFF) << 12) | (y & 0xFFF)

Порядок именно x, **z**, y — z в середине, y в младших 12 битах.

### Значения для поршня

Дословно (`Java Edition protocol/Block actions`, раздел *Piston*):

> Controls the state of the piston and affected blocks. Always used on the
> piston base (either sticky or regular), never on the head.

**Action IDs:**

> **0** to extend the piston, **1** to retract it, and **2** to cancel an
> ongoing extension.
>
> In Notchian implementations, action ID **2** is defined by when a piston is
> retracted (e.g.: due to loss of power) mid-extension. The semantics for this
> action also change based on the type of piston:
> * if the piston is a normal one, the action has the same result as a normal
>   retraction;
> * if the piston is a sticky one, it will be retracted without pulling any
>   blocks.

> **Warning:** Sending an extension action to an already-extended piston has no
> effect, but sending retraction actions to an already-retracted piston may
> cause undesired results, such as the block directly in front of the piston
> head dissappearing on the client.

**Action Parameters:**

> The direction the piston is facing.
>
> Any value can be sent for piston extension, since the direction is infered
> from the piston's block state, but it must match the actual direction for
> piston retractions (action ID **1** and **2**).

> **Warning:** Sending a mismatched direction for piston extension has no
> effect, but sending a mismatched direction to piston retractions will cause
> the piston block to assume the new sent direction on the client.

| Direction ID | Direction |
|---|---|
| 0 | Down |
| 1 | Up |
| 2 | North |
| 3 | South |
| 4 | West |
| 5 | East |

Это в точности нумерация нашего `Dir::number()`.

### Пример байтов: обычный поршень в (10, 64, −20) выдвигается на восток

Position:

    ((10 & 0x3FFFFFF) << 38) | ((-20 & 0x3FFFFFF) << 12) | (64 & 0xFFF)
      = 0x2BFFFFEC040
    восемь байт (big-endian): 00 00 02 BF FF FE C0 40

Тело пакета (без сжатия — сначала VarInt длины тела, потом само тело):

| байты | что это |
|---|---|
| `0D` | длина тела: 13 байт |
| `07` | VarInt: номер пакета Block Action |
| `00 00 02 BF FF FE C0 40` | Position (10, 64, −20) |
| `00` | Action ID: выдвигается |
| `05` | Action Parameter: East |
| `8A 01` | VarInt 138 — номер блока `minecraft:piston` |

Целиком:

    0D 07 00 00 02 BF FF FE C0 40 00 05 8A 01

Для липкого поршня последние два байта — `80 01` (VarInt 128).
При сжатом соединении перед этим идёт ещё VarInt длины несжатых данных.

---

## 2. Остальные пакеты хода

### Block Update (`block_update`) — 0x08

Проверено: `protocol.json` 26.1 → `0x08 block_change`.

> Fired whenever a block is changed within the render distance.

| Поле | Тип | Заметка |
|---|---|---|
| Location | Position | «Block Coordinates.» |
| Block ID | VarInt | «The new block state ID for the block as given in the global block state palette.» |

Здесь, в отличие от Block Action, число — **номер состояния** (у нас — то, что
лежит в мире).

### Update Section Blocks (`section_blocks_update`) — 0x54

Проверено: `protocol.json` 26.1 → `0x54 multi_block_change`.

> Fired whenever 2 or more blocks are changed within the same chunk on the same
> tick.

| Поле | Тип | Заметка |
|---|---|---|
| Chunk section position | Long | «Chunk section coordinate (encoded chunk x and z with each 22 bits, and section y with 20 bits, from left to right).» |
| Blocks | Prefixed Array of VarLong | «Each entry is composed of the block state id, shifted left by 12, and the relative block position in the chunk section (4 bits for x, z, and y, from left to right).» |

Кодирование дословно:

```java
((sectionX & 0x3FFFFF) << 42) | (sectionY & 0xFFFFF) | ((sectionZ & 0x3FFFFF) << 20);
```

```java
blockStateId << 12 | (blockLocalX << 8 | blockLocalZ << 4 | blockLocalY)
```

**Когда вместо Block Update.** Условие в самой вики — «2 or more blocks … within
the same chunk on the same tick»: то есть выбор чисто по объёму, смысл у пакетов
один. Никакого требования слать именно Update Section Blocks для поршня нет:
страница про действия блоков всюду пишет «through Block Update or Update Section
Blocks». Для хода поршня это существенно, потому что за один ход меняется до
13 блоков (12 толкаемых + сам поршень) и почти всегда в одной-двух секциях.

Важно: элементы массива — **VarLong**, а не VarInt. В `minecraft-data`
`packet_multi_block_change` описывает `records` как массив `varint` — это
упрощение их описания (у них нет отдельного VarLong в этом месте), на длинных
значениях оно разойдётся с реальностью. Верить надо вики.

### Sound Effect (`sound`) — 0x75

Проверено: `protocol.json` 26.1 → `0x75 sound_effect`.

> Plays a sound effect at the given location, either by hardcoded ID or
> Identifier.

| Поле | Тип | Заметка |
|---|---|---|
| Sound Event | ID or Sound Event | «ID in the `minecraft:sound_event` registry, or an inline definition.» |
| Sound Category | VarInt Enum | «The category that this sound will be played from» |
| Effect Position X | Int | «Effect X multiplied by 8 (fixed-point number with only 3 bits dedicated to the fractional part).» |
| Effect Position Y | Int | то же по Y |
| Effect Position Z | Int | то же по Z |
| Volume | Float | «1.0 is 100%, capped between 0.0 and 1.0 by vanilla clients.» |
| Pitch | Float | «Float between 0.5 and 2.0 by vanilla clients.» |
| Seed | Long | «Seed used to pick sound variant.» |

Тип `ID or X` (Data types):

> | ID | VarInt | 0 if value of type X is given inline; otherwise registry ID + 1. |
> | Value | Optional X | Only present if ID is 0. |

Инлайновый `Sound Event`:

> | Sound Name | Identifier | |
> | Has Fixed Range | Boolean | Whether this sound has a fixed range, as opposed to a variable volume based on distance. |
> | Fixed Range | Optional Float | The maximum range of the sound. Only present if Has Fixed Range is true. |

Категории звука (`soundSource` из `protocol.json` 26.1):
`0 master, 1 music, 2 record, 3 weather, 4 block, 5 hostile, 6 neutral,
7 player, 8 ambient, 9 voice, 10 ui`. Поршню — **4 (block)**.

**Обязателен ли.** Да, если звук вообще нужен: по странице действий блоков
воспроизведение звука — обязанность сервера, а не клиента:

> On the other hand, the server Notchian server is responsible for:
> * …
> * Playing the piston extend/retract sound;

То есть Block Action анимацию рисует, но звука не даёт. Ход поршня без Sound
Effect визуально правильный и молчаливый — что у нас сейчас и происходит.

### Block Entity Data (`block_entity_data`) — 0x06

Проверено: `protocol.json` 26.1 → `0x06 tile_entity_data`.

| Поле | Тип | Заметка |
|---|---|---|
| Location | Position | |
| Type | VarInt | «ID in the `minecraft:block_entity_type` registry» |
| NBT Data | NBT | «Data to set.» |

**Поршню не нужен вообще.** Дословно:

> Although the server creates `MOVING_PISTON` blocks and moving piston block
> entities, they're not sent to the client, and are kept on the server merely to
> keep track of the action progression (to handle collision, block changes upon
> completion etc.). The client is responsible for creating such blocks and
> entities on its side upon receiving Block Action packet.

> `MOVING_PISTON` blocks and moving piston block entities can be received in a
> Chunk Data packet. However, the block entity data is ignored by the client and
> `MOVING_PISTON` blocks are set in their default state (empty).

У самих `piston`/`sticky_piston` блок-сущности нет — она есть только у
`moving_piston`, и её слать бессмысленно: клиент её игнорирует.

---

## 3. Порядок пакетов за один ход

Обязанности сервера, дословно:

> * Calculating if the action is possible, and which blocks will be affected;
> * Initiating the actual action through the Block Action packet;
> * Playing the piston extend/retract sound;
> * Changing pushed blocks into `MOVING_PISTON` blocks and their respective
>   moving piston block entities (server-side only);
> * Immediately sending updates for blocks that would be destroyed by the piston
>   action (through Block Update or Update Section Blocks);
> * Calculate entity collision mid-push/mid-pull and update their position
>   accordingly;
> * Changing the pushed blocks' new block states once the action is finished
>   (after 3 ticks, through Block Update or Update Section Blocks).

Отсюда последовательность:

1. **Block Action (0x07)** — первым. Это спусковой крючок: получив его, клиент
   сам считает, какие блоки поедут, сам подменяет их на `moving_piston` и сам
   рисует движение.
2. **Sound Effect (0x75)** — сразу за ним, в том же такте (порядок с шагом 1
   не критичен, но раньше Block Action его слать бессмысленно).
3. **Block Update / Update Section Blocks** для блоков, которые ход
   **уничтожает** (выдавливаемая вода, факелы и т. п.) — «immediately», в том же
   такте.
4. Такты хода — сервер ничего не шлёт.
5. **Block Update / Update Section Blocks** с окончательными состояниями
   сдвинутых блоков — когда ход закончен. Клиент к этому моменту уже поставил
   свои (с задержкой, см. ниже), серверные приходят поверх.

Про блок самого поршня (`extended`) вики прямо ничего не говорит; см. раздел
«чего в документации нет».

**Что будет, если порядок нарушить:**

- **Состояние поршня уходит раньше Block Action.** Клиент, получив уже
  выдвинутый поршень, к моменту действия видит `extended=true`, а для такого
  «sending an extension action to an already-extended piston has no effect» —
  анимации не будет, блоки просто перескочат. Это ровно наша ситуация, см. §5.
- **Block Action раньше, чем клиент вообще знает про чанк.** Пакет привязан к
  координатам; для незагруженного клиентом чанка он бесполезен.
- **Окончательные Block Update раньше конца хода.** Клиент уже держит на этих
  местах свои `moving_piston`; ранняя подмена даёт рывок и остатки
  «застрявших» блоков.
- **Несовпадение стороны при задвигании.** «sending a mismatched direction to
  piston retractions will cause the piston block to assume the new sent
  direction on the client» — поршень у клиента развернётся.
- **Задвигание уже задвинутого.** «may cause undesired results, such as the
  block directly in front of the piston head dissappearing on the client».
- **Расхождение расчёта.** «The calculation for blocks to be pushed/destroyed is
  the same on both server and client, and they rely on that assumption.
  Attempting to change the logic in one side may lead to de-sync issues.» —
  если наш расчёт толкаемых блоков разойдётся с клиентским, картинка разъедется
  независимо от порядка пакетов.

Отдельно (это про такты, подробности — в `piston-timing.md`, но на пакеты
влияет): вики отмечает, что клиент ставит новые состояния позже сервера:

> The tick logic for moving piston block entities is the roughly the same for
> both client and server, except that the client will wait an extra 5 ticks
> before setting the pushed blocks to their new state.

и это помечено как `Missing information` — то есть почему так, вики не знает.

---

## 4. Номера

### Блоки

Из нашей таблицы (`tools/data/blocks.json`, набор PrismarineJS/minecraft-data,
`data/pc/26.1`) и сгенерированной `src/blocks_table.rs` → `BLOCK_IDS`:

| Блок | Номер блока (в Block Action) | Номер состояния по умолчанию |
|---|---|---|
| `minecraft:piston` | **138** | 2263 (диапазон 2257–2268) |
| `minecraft:sticky_piston` | **128** | 2241 (диапазон 2235–2246) |
| `minecraft:piston_head` | 139 | 2271 |
| `minecraft:moving_piston` | 156 | 2309 |

Наш `blocks::block_id("piston")` даёт 138, `blocks::block_id("sticky_piston")` —
128. Совпадает. Обратите внимание: липкий поршень **меньше** обычного — это не
опечатка, порядок реестра блоков не алфавитный.

### Звуки

Прямого списка номеров звуков на вики нет — она отсылает к внешнему дампу:

> Sound IDs and names can be found [pokechu22.github.io/Burger] here.

Два независимых источника расходятся:

| Источник | `block.piston.contract` | `block.piston.extend` | Всего звуков |
|---|---|---|---|
| Burger, дамп 26.1 (`protocol: 775`, клиент 26.1) | 1249 | 1250 | 1808 |
| minecraft-data `data/pc/26.1/sounds.json` | 1297 | 1298 | 1902 |

Разбор расхождения: у Burger нумерация с нуля, у minecraft-data — та же
последовательность, но сдвинутая на +1 (`entity.allay.ambient_with_item`: 0 и 1,
`ambient.cave`: 7 и 8, `block.anvil.break`: 46 и 47). Но списки не просто
сдвинуты: у minecraft-data на 94 записи больше, и первое расхождение — на
позиции 285 (`entity.baby_cat.purreow` есть только у него). То есть один из
дампов снят с другой сборки, чем другой (у нас клиент 26.1.**2**, а Burger
разбирал 26.1). Какой из наборов верен для 26.1.2 — по этим данным сказать
нельзя.

Отсюда вывод: **номер звука для 775 гадать не надо.** В протоколе для того и
сделан тип `ID or X`: вместо номера можно послать VarInt `0` и имя звука
строкой. Это не запасной вариант, а штатный путь, он не ломается от смены
сборки. Если всё же понадобится номер, снимать его следует так: в поле пишется
`registry ID + 1`, а сам registry ID берётся из дампа реестра
`minecraft:sound_event` ровно той сборки, к которой подключается клиент.

Проверить можно опытом: послать инлайновое имя (звук обязан заиграть) и
после этого — числовой вариант; если заиграет тот же звук, номер верен.

---

## 5. Что у нас не так

1. **Порядок Block Update и Block Action в главном цикле перевёрнут.**
   `src/network/play.rs:2344-2345`: сперва `send_pending_block_changes`, затем
   `send_pending_block_actions`. А `src/redstone.rs:1860-1874` в том же такте
   сначала вызывает `note_action`, потом `set(...)` с `extended=true`, и
   `World::set_block` (`src/world/mod.rs:553-558`) кладёт это в журнал
   изменений. Оба журнала вычитываются в одном обороте цикла, изменения —
   первыми, значит клиент получает **выдвинутый поршень раньше сообщения о
   ходе**. По вики это ровно тот случай, когда действия просто не будет:
   «Sending an extension action to an already-extended piston has no effect».
   Комментарий в `redstone.rs` («Сперва сообщаем о ходе, и только потом меняем
   состояние») описывает намерение, которое сетевой слой отменяет.
   Второе место, `src/network/play.rs:2470-2485`, сделано правильно —
   действия, потом изменения; то есть два пути отправки противоречат друг другу.

2. **Звука нет вовсе.** Пакет Sound Effect (0x75) в `play.rs` не реализован —
   ни константы, ни функции. По документации проигрывание звука поршня —
   обязанность сервера, клиент сам его не издаёт. Что нужно — в §6.

3. **Block Action уходит всем подключениям без разбора.**
   `send_pending_block_actions` (`play.rs:1873-1899`) берёт весь журнал и шлёт
   каждому игроку, не проверяя, загружен ли у него этот чанк и близко ли он.
   Для далёких игроков это лишний трафик, а для игрока, которому чанк ещё не
   отправлен, — пакет о блоке, которого он не видит. То же и у
   `send_pending_block_changes`.

4. **Действие 2 (отмена начатого выдвигания) не шлётся никогда.**
   `src/redstone.rs:35-36` знает только `PISTON_EXTENDS = 0` и
   `PISTON_RETRACTS = 1`, а `block_event` (`redstone.rs:1841-1844`) на
   поршень, уже находящийся в движении (`world.redstone.moving`), просто
   выходит. Случай «питание пропало посреди выдвигания» по вики требует
   действия 2, иначе у клиента остаётся доигранная анимация, не совпадающая с
   миром.

5. **Никогда не шлётся Update Section Blocks.**
   `send_pending_block_changes` (`play.rs:1957-1959`) на каждое изменение шлёт
   отдельный Block Update. Работать будет, но ход поршня — это до 13 блоков,
   и в конце хода они уходят 13 отдельными пакетами вместо одного. Не ошибка
   протокола, но заметная трата.

6. **Действие не отправляется, если у блока не нашлось номера.**
   `redstone.rs:1864` — `if let Some(id) = blocks::block_id(block.name)`. Для
   поршней номер есть всегда, но молчаливый пропуск сообщения о ходе при
   промахе таблицы — плохой отказ: клиент останется с несдвинутыми блоками и
   никто об этом не узнает. Здесь номер можно слать хоть нулевой: «This value
   is unused by the vanilla client».

7. **Ход не начинается — и сообщения тоже нет.** `redstone.rs:1853-1858`: если
   `start_move` вернул `None`, функция выходит до `note_action`. Для
   задвигания, которому нечего тянуть, поршень в игре всё равно задвигается, и
   клиенту всё равно нужно действие 1. Это на границе с механикой
   (`piston-timing.md`), но последствие — сетевое: у клиента останется
   выдвинутая голова.

**Что у нас верно:** номер пакета `BLOCK_ACTION = 0x07` (`play.rs:257`);
порядок полей — Position, байт, байт, VarInt (`play.rs:1890-1893`); упаковка
координат `((x)<<38)|((z)<<12)|y` (`play.rs:2249-2252`); в последнем поле
именно номер блока, а не состояния (`world/mod.rs:70-85`, `redstone.rs:1867`);
значения действий 0/1 и нумерация сторон 0..5 (`redstone.rs:35-36`,
`redstone.rs:165-174`) — всё совпадает с документацией.

---

## 6. Что нужно, чтобы прислать звук

Пакет 0x75, сразу после Block Action, тем же игрокам. Тело:

| Поле | Значение для поршня |
|---|---|
| Sound Event | VarInt `0`, затем Identifier `minecraft:block.piston.extend` (или `...contract`), затем Boolean `0` (без фиксированной дальности) |
| Sound Category | VarInt `4` — block |
| Effect Position X | `i32` = round(x_центра × 8) |
| Effect Position Y | `i32` = round(y_центра × 8) |
| Effect Position Z | `i32` = round(z_центра × 8) |
| Volume | `f32` 0.5 |
| Pitch | `f32` из 0.6…0.85 (выдвигание) / 0.6…0.8 (задвигание) |
| Seed | `i64` 0 |

Громкость и высота — из статьи [Piston] на вики (раздел Sounds): у
`block.piston.extend` volume 0.5, pitch 0.6–0.85; у `block.piston.contract`
volume 0.5, pitch 0.6–0.8; категория — Blocks; attenuation distance 8.
Высоту игра выбирает случайно в этом промежутке на каждый ход — постоянное
число даст механическое звучание, но работать будет.

Координаты — центр блока поршня: для (10, 64, −20) это (10.5, 64.5, −19.5), в
пакете `84`, `516`, `-156` (умножено на 8). Отдельного тактового окна у звука
нет: он играется один раз в момент начала хода.

Что для этого нужно добавить (кода не трогал, это план):

1. Константа `SOUND_EFFECT: i32 = 0x75` рядом с `BLOCK_ACTION` в
   `src/network/play.rs`.
2. Функция записи Identifier (VarInt длины + UTF-8) — если её ещё нет,
   посмотреть на то, как пишутся строки в других пакетах.
3. `send_sound_effect(...)` по таблице выше, инлайновым именем звука: числовой
   номер для 775 недостоверен (см. §4), а имя не зависит от сборки.
4. Звук — часть записи `BlockAction` или отдельный журнал: разослать его надо
   тем же игрокам и в том же обороте цикла, что и действие, иначе звук
   разъедется с картинкой.
5. Слать **после** Block Action и только когда ход действительно начался.

---

## Чего в документации нет

- **Когда именно слать Block Update для самого поршня (`extended`).** Вики
  перечисляет обязанности сервера, но про состояние блока-основания молчит:
  ни «пошлите его после действия», ни «не шлите вовсе». Из предупреждения про
  «already-extended piston» следует только то, что раньше действия его слать
  нельзя. Ставит ли клиент `extended=true` сам по Block Action — не написано.
- **Номера звуков для 775.** Таблицы нет ни на одной странице вики, а два
  доступных дампа расходятся (см. §4).
- **Точная громкость/высота, с которыми звук шлёт ванильный сервер.** Данные из
  статьи Piston — диапазоны из ресурсов игры; формулу выбора высоты вики не
  приводит.
- **Порог, с которого сервер предпочитает Update Section Blocks.** Сказано лишь
  «2 or more blocks … within the same chunk on the same tick»; как именно
  ванильный сервер группирует изменения хода поршня (одним пакетом на секцию,
  на чанк, или как получится) — не описано.
- **Почему клиент ждёт лишние 5 тактов** — помечено на вики как
  `Missing information` прямым текстом («Why does this happen? … Is it a
  lag-compensation mechanism?»).
- **Поведение при частичной загрузке чанков** — что делает клиент с Block
  Action для чанка, который у него не загружен, нигде не описано.

---

## Источники

- minecraft.wiki, «Java Edition protocol/Packets» (текущая редакция описывает
  протокол 777): разделы Block Action, Block Update, Update Section Blocks,
  Sound Effect, Block Entity Data.
  https://minecraft.wiki/w/Java_Edition_protocol/Packets
- minecraft.wiki, «Java Edition protocol/Block actions», раздел Piston
  (Action IDs, Action Parameters, обязанности сторон, предупреждения).
  https://minecraft.wiki/w/Java_Edition_protocol/Block_actions
- minecraft.wiki, «Java Edition protocol/Data types»: Position, ID or X,
  Sound Event.
  https://minecraft.wiki/w/Java_Edition_protocol/Data_types
- minecraft.wiki, «Piston», раздел Sounds (громкость, высота, категория).
  https://minecraft.wiki/w/Piston
- История правок страницы протокола: редакция от 2026-04-16 «Change handshake
  notes protocol 774 to 775» и от 2026-07-07 «26.2» — окно, когда страница
  описывала именно 775.
- PrismarineJS/minecraft-data, данные (JSON) `data/pc/26.1`: `version.json`
  (775), `protocol.json` (номера пакетов и раскладка полей, перечисление
  `soundSource`), `blocks.json` (номера блоков), `sounds.json` (номера звуков).
  https://github.com/PrismarineJS/minecraft-data/tree/master/data/pc/26.1
- PrismarineJS/minecraft-data, `schemas/sounds_schema.json` — описание полей
  `sounds.json` (оказалось неинформативным насчёт базы нумерации).
- Burger, дамп клиента 26.1 (`protocol: 775`): список звуков с номерами.
  https://pokechu22.github.io/Burger/26.1.html
- Наши файлы: `tools/data/blocks.json`, `src/blocks_table.rs`,
  `src/network/play.rs`, `src/world/mod.rs`, `src/redstone.rs`.

**Ограничение соблюдено:** исходный код игры (в том числе декомпилированный),
client.jar/server.jar, ванильные датапаки и исходный код серверных ядер, прокси
и библиотек (Paper, Spigot, Fabric, Forge, Bukkit, ViaVersion,
node-minecraft-protocol, код PrismarineJS и любых других реализаций) не
открывались и не использовались. Из репозиториев брались только данные (JSON) и
дампы: `minecraft-data/data/pc/26.1/*.json`, схема формата, и HTML/JSON-дамп
Burger. Остальное — документация вики.
