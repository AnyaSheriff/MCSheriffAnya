# Ход поршня по протоколу: что обязан делать сервер

Справочник по вики (minecraft.wiki) для протокола 775 / Java Edition 26.1.2.
Здесь — только сетевая сторона: что сервер шлёт клиенту, когда и чего слать
нельзя. Устройство самого блока `moving_piston` и его блок-сущности разобрано
отдельно в `tools/research/moving-piston.md`, механика хода — в
`tools/research/redstone-pistons.md` и `tools/research/audit-pistons.md`.

Источники:

- https://minecraft.wiki/w/Java_Edition_protocol/Block_actions (§ Piston) — главный источник, почти всё ниже отсюда
- https://minecraft.wiki/w/Java_Edition_protocol/Packets (§ Block Action, § Block Update)
- https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston → Block data)
- https://minecraft.wiki/w/Tick (§ Piston tick)
- https://minecraft.wiki/w/Piston (§ Start delay)

---

## 1. Пакет Block Action

### 1.1 Что это за пакет

> «This packet is used for a number of actions and animations performed by
> blocks, usually non-persistent. The client ignores the provided block type and
> instead uses the block state in their world.»
> — https://minecraft.wiki/w/Java_Edition_protocol/Packets (§ Block Action)

> «{{warning|This packet uses a block ID from the `minecraft:block` registry, not
> a block state.}}»
> — там же

Поля (там же):

| Поле | Тип | Смысл (дословно) |
|---|---|---|
| Location | Position | «Block coordinates.» |
| Action ID (Byte 1) | Unsigned Byte | «Varies depending on block» |
| Action Parameter (Byte 2) | Unsigned Byte | «Varies depending on block» |
| Block Type | VarInt | «ID in the `minecraft:block` registry. This value is unused by the vanilla client, as it will infer the type of block based on the given position.» |

### 1.2 Для поршня

> «Controls the state of the piston and affected blocks. Always used on the
> piston base (either sticky or regular), never on the head.»
> — https://minecraft.wiki/w/Java_Edition_protocol/Block_actions (§ Piston)

**Action ID:**

> «**0** to extend the piston, **1** to retract it, and **2** to cancel an
> ongoing extension.
>
> In Notchian implementations, action ID **2** is defined by when a piston is
> retracted (e.g.: due to loss of power) mid-extension. The semantics for this
> action also change based on the type of piston:
> * if the piston is a normal one, the action has the same result as a normal retraction;
> * if the piston is a sticky one, it will be retracted without pulling any blocks.»
> — там же

**Action Parameter:**

> «The direction the piston is facing.
>
> Any value can be sent for piston extension, since the direction is infered from
> the piston's block state, but it must match the actual direction for piston
> retractions (action ID **1** and **2**).»
> — там же

| Direction ID | Direction |
|---|---|
| 0 | Down |
| 1 | Up |
| 2 | North |
| 3 | South |
| 4 | West |
| 5 | East |

### 1.3 Прямые предупреждения (дословно)

> «{{Warning|Sending an extension action to an already-extended piston has no
> effect, but sending retraction actions to an already-retracted piston may cause
> undesired results, such as the block directly in front of the piston head
> dissappearing on the client.}}»
> — там же (орфография оригинала сохранена)

> «{{Warning|Sending a mismatched direction for piston extension has no effect,
> but sending a mismatched direction to piston retractions will cause the piston
> block to assume the new sent direction on the client.}}»
> — там же

### 1.4 Когда слать относительно смены состояния основания

**Прямого ответа на вики нет** (см. § 7). Но он выводится из первого
предупреждения, и вывод получается жёсткий:

- Клиент решает, что делать, **по своему состоянию мира** («The client ignores
  the provided block type and instead uses the block state in their world»).
- Значит, в момент, когда Block Action доходит до клиента, у клиента поршень
  должен быть **ещё в исходном состоянии**: при выдвигании — `extended=false`,
  при задвигании — `extended=true` с головой перед ним.
- Иначе срабатывает предупреждение: выдвигание уже выдвинутому «has no effect»,
  задвигание уже задвинутому — «the block directly in front of the piston head
  dissappearing on the client».

**Правило: Block Action должен уйти на провод РАНЬШЕ, чем Block Update,
меняющий `extended` у основания.** Оба меняются в один такт, поэтому важен
порядок именно байтов в потоке, а не порядок вызовов внутри сервера.

---

## 2. Что клиент делает сам

> «Upon receiving this action, the Notchian client is responsible for:
> * Calculating if the action is possible, and which blocks will be affected;
> * Changing pushed blocks into `MOVING_PISTON` blocks and their respective moving
>   piston block entities (client-side only);
> * Changing the pushed blocks' new block states once the action is finished
>   (action takes 3 ticks, but blocks are set after an extra 5 ticks;
>   client-side only).»
> — https://minecraft.wiki/w/Java_Edition_protocol/Block_actions (§ Piston)

Разбор по вопросам:

- **Какие блоки он создаёт у себя.** `MOVING_PISTON` и блок-сущности при них —
  сам, по своему расчёту связки. Сервер их не присылает (§ 4).
- **Сколько тактов рисует.** Само движение — 3 такта («action takes 3 ticks»).
- **Когда сам ставит настоящие блоки.** Через **3 + 5 = 8 тактов** после
  получения пакета («blocks are set after an extra 5 ticks»).
- **Страховка есть.** Эти 8 тактов и есть страховка: даже если сервер не
  пришлёт ни одного Block Update, клиент сам поставит блоки на новые места по
  своему расчёту. Поэтому опоздание сервера на такт-другой не видно, а вот
  **расхождение расчёта** видно сразу.

Вики прямо отмечает, что зачем это сделано — неизвестно:

> «{{Missing information|The tick logic for moving piston block entities is the
> roughly the same for both client and server, except that the client will wait an
> extra 5 ticks before setting the pushed blocks to their new state. Why does this
> happen? Why does the client even set the new blocks on its own in the first
> place, if their final state is going to be received by the server anyway? Is it a
> lag-compensation mechanism?}}»
> — там же

---

## 3. Что обязан прислать сервер и когда

> «On the other hand, the server Notchian server is responsible for:
> * Calculating if the action is possible, and which blocks will be affected;
> * Initiating the actual action through the Block Action packet;
> * Playing the piston extend/retract sound;
> * Changing pushed blocks into `MOVING_PISTON` blocks and their respective moving
>   piston block entities (server-side only);
> * Immediately sending updates for blocks that would be destroyed by the piston
>   action (through Block Update or Update Section Blocks);
> * Calculate entity collision mid-push/mid-pull and update their position accordingly;
> * Changing the pushed blocks' new block states once the action is finished
>   (after 3 ticks, through Block Update or Update Section Blocks).»
> — https://minecraft.wiki/w/Java_Edition_protocol/Block_actions (§ Piston)

### 3.1 Раскладка «такт → сервер → клиент»

Такт 0 — тот, в котором поршень трогается (фаза блочных событий).

| Такт | Что делает сервер | Что уходит на провод | Что делает клиент |
|---|---|---|---|
| 0 | считает связку; ставит у себя `MOVING_PISTON` + блок-сущности (не рассылая); ломает то, во что упёрлись | **1.** Block Action (0/1/2, направление) — первым; **2.** Block Update: основание `extended=true` (только при выдвигании); **3.** Block Update на **сломанные** блоки — «Immediately»; **4.** звук хода | принимает Block Action, считает связку сам, ставит свои `MOVING_PISTON` + блок-сущности, начинает рисовать |
| 0…2 | считает столкновения сущностей с едущими блоками и двигает их | Entity-пакеты движения | рисует движение (`progress` 0.0 → 0.5 → 1.0) |
| 2 (конец хода) | убирает служебные блоки; ставит голову / сам поршень / перевозимые блоки на новые места; при задвигании — основание `extended=false` | Block Update / Update Section Blocks на **все** новые места: клетка головы, клетки перевозимых блоков, освободившиеся клетки, основание | принимает и ставит; движение у него как раз доигрывается |
| 3 | — | — | у клиента «action takes 3 ticks» — движение кончилось |
| 8 | — | — | **страховка:** если сервер ничего не прислал, клиент ставит блоки сам, по своему расчёту |

### 3.2 Про «3 ticks» и про «2 такта»

Числа на вики расходятся, и это надо знать.

Протокольная страница говорит «3 ticks» (и про клиента, и про сервер).
Описание блок-сущности даёт арифметику, из которой выходит **2 такта**:

> «Float **progress**: How far the block has been moved. Starts at 0.0, and
> increments by 0.5 each tick, including the tick on which moving the piston is
> created. If the value is 1.0 or higher at the *start* of a tick of the block
> entity (before incrementing), then the block transforms into the stored
> blockState.»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston → Block data)

Считаем: такт создания 0.0 → 0.5; следующий такт 0.5 (не ≥ 1.0) → 1.0; ещё
следующий — на его начале 1.0 ≥ 1.0, превращение. То есть превращение
происходит **через 2 такта** после такта начала; «3 ticks» — это то же самое,
посчитанное с включением такта начала.

Есть ещё третий счёт, к игровым тактам отношения не имеющий:

> «The piston tick starts in the entity phase of the game tick and ends in the
> block event phase, therefore a piston always takes 3 piston ticks to extend or
> retract, without any start delay.»
> — https://minecraft.wiki/w/Tick (§ Piston tick)

«Piston tick» — не игровой такт, а придуманное сообществом деление такта на
части («It was created by the Chinese redstone community… it never gained
popularity in the Western community» — там же). Для нас это не мера времени.

**Вывод для сервера:** механика встаёт через **2 игровых такта** после такта
начала, и в этот же момент уходят Block Update о новых блоках. Никакой
добавки к этому сроку в документации нет: «once the action is finished».

---

## 4. Чего сервер слать НЕ должен

> «Although the server creates `MOVING_PISTON` blocks and moving piston block
> entities, they're not sent to the client, and are kept on the server merely to
> keep track of the action progression (to handle collision, block changes upon
> completion etc.). The client is responsible for creating such blocks and
> entities on its side upon receiving Block Action packet.»
> — https://minecraft.wiki/w/Java_Edition_protocol/Block_actions (§ Piston, warning)

То есть:

- **никаких Block Update с `minecraft:moving_piston`** в клетках хода;
- **никаких Block Entity Data** с блок-сущностью `minecraft:piston`
  (`blockState`, `extending`, `facing`, `progress`, `source`);
- клетки, из которых блок уехал, **не** объявляются воздухом в начале хода —
  клиент сам поставит там свой `MOVING_PISTON`.

Единственное исключение — пакет чанка, и там это безвредно:

> «`MOVING_PISTON` blocks and moving piston block entities can be received in a
> Chunk Data packet. However, the block entity data is ignored by the client and
> `MOVING_PISTON` blocks are set in their default state (empty). This could
> potentially be intentional, as the piston animation is short (3 ticks long), and
> the client will subsequently receive updates for the final block states from the
> server;»
> — там же

---

## 5. Известные грабли

**Рассинхронизация из-за разного расчёта.** Главная и названная на вики:

> «The calculation for blocks to be pushed/destroyed is the same on both server
> and client, and they rely on that assumption. Attempting to change the logic in
> one side may lead to de-sync issues.»
> — там же

Совет отсюда прямой: логику «что поедет и что сломается» менять нельзя ни на
йоту — ни лимит в 12 блоков, ни порядок обхода липких соседей, ни список
блоков, которые ломаются/не двигаются. Клиент считает то же самое своим кодом,
и поправить его нечем.

**Пропажа блока перед головой.** Названа дословно: «sending retraction actions
to an already-retracted piston may cause undesired results, such as the block
directly in front of the piston head dissappearing on the client». Лечится
одним: не слать задвигание задвинутому. У нас это проверяется (§ 6).

**Поршень «разворачивается» на клиенте.** «sending a mismatched direction to
piston retractions will cause the piston block to assume the new sent direction
on the client» — при задвигании параметр направления обязан совпадать с
настоящим.

**Действие без последствий.** «Sending an extension action to an
already-extended piston has no effect» — если клиент к моменту прихода пакета
уже видит `extended=true`, движение просто не начнётся, и блоки появятся на
новых местах рывком, когда придут Block Update.

**Задержка сети.** Про неё на вики прямо ничего нет. Есть только косвенное:
клиентская страховка в 5 лишних тактов и вопрос «Is it a lag-compensation
mechanism?» (§ 2). То есть у сервера есть запас: Block Update о новых блоках,
пришедший в любой момент до 8-го такта, клиент примет как норму. Опоздание
сверх этого приведёт к тому, что клиент уже поставит блоки по своему расчёту,
и серверная поправка будет выглядеть как мигание.

**Мигание блоков.** Отдельного раздела на вики нет; см. § 7.

---

## 6. Что у нас не так

Проверено по `src/redstone.rs`, `src/world/mod.rs`, `src/network/play.rs`.

Сперва то, что **сделано правильно**, чтобы не трогать:

- `moving_piston` ставится через `set_hidden` → `World::set_block_quietly`
  (`src/world/mod.rs:594`), в журнал изменений не попадает — клиенту не
  уходит. Совпадает с требованием § 4.
- Блок-сущность поршня клиенту не шлётся вовсе.
- Направление в параметре: `Dir::number()` (`src/redstone.rs:165`) даёт
  0=Down, 1=Up, 2=North, 3=South, 4=West, 5=East — таблица § 1.2 совпадает
  дословно.
- Block Type шлётся как id блока из реестра `minecraft:block`
  (`blocks::block_id`), а не состояние — как требует предупреждение § 1.1.
- Задвигание задвинутому не шлётся: `block_event` выходит при `powered ==
  extended` (`src/redstone.rs:1843-1850`), поэтому главная грабля § 5 нам не
  грозит.
- Ломающиеся блоки ломаются в `start_move` через `destroy` → `set` →
  `World::set_block`, то есть попадают в журнал изменений того же такта и
  уходят вместе с остальным — «Immediately sending updates for blocks that
  would be destroyed».
- Срок хода `PISTON_MOVE = 2` (`src/redstone.rs:43`) — верный: § 3.2.

### 6.1 Block Action уходит ПОСЛЕ Block Update о выдвинутом поршне

> «Sending an extension action to an already-extended piston has no effect»
> — Java Edition protocol/Block actions, § Piston, Action IDs

**Наше место.** `src/redstone.rs:1861-1875`: сперва `note_action`, потом
`set(world, pos, extended=true)` — намерение в комментарии правильное («Сперва
сообщаем о ходе, и только потом меняем состояние поршня»). Но это **два разных
журнала**, а на провод они выливаются в обратном порядке:

- `src/network/play.rs:2344-2345` — сперва `send_pending_block_changes`, затем
  `send_pending_block_actions`;
- `src/network/play.rs:2465-2481` — в ветке установки блока то же самое и в том
  же порядке.

То есть клиент получает `piston[extended=true]` **раньше**, чем Block Action.

**Чем грозит.** Ровно тем, о чём предупреждение: клиент видит уже выдвинутый
поршень, действие 0 для него «has no effect» — движение не рисуется. Блоки
появятся на новых местах рывком, когда придут Block Update (у нас — на 4-м
такте, § 6.3). Голова поршня при этом выскочит мгновенно.

Задвигания это не касается: `extended=false` ставится только в `finish_move`
(`src/redstone.rs:2056`), то есть через 2 такта после действия — порядок там
верный.

**Как чинить (не сделано, только вывод):** либо выливать журнал действий
раньше журнала изменений, либо класть и то и другое в один журнал с общим
порядком.

### 6.2 Звука хода нет вовсе

> «Playing the piston extend/retract sound»
> — Java Edition protocol/Block actions, § Piston (обязанности сервера)

**Наше место.** В `src/network/play.rs` нет ни пакета Sound Effect, ни любого
другого звукового пакета (поиск по `sound` в файле не даёт ничего). Звук хода
не шлётся.

**Чем грозит.** Ход беззвучный. На картинку не влияет, но «правильно» по
вики — не будет: в списке обязанностей сервера звук стоит наравне с самим
Block Action, потому что клиент сам его не проигрывает.

### 6.3 Лишняя задержка рассылки: `PISTON_TELL_LATER = 2`

> «Changing the pushed blocks' new block states once the action is finished
> (after 3 ticks, through Block Update or Update Section Blocks).»
> — Java Edition protocol/Block actions, § Piston (обязанности сервера)

**Наше место.** `src/redstone.rs:1751` и `src/redstone.rs:2062`: после
`finish_move` все новые места рассылаются не сразу, а через
`tell_later(..., PISTON_TELL_LATER)`, то есть через 2 такта после конца хода —
на 4-м такте от начала.

**Каким должно быть число.** Ноль. Разбор § 3.2: «3 ticks» вики считает с
включением такта начала, а наш `PISTON_MOVE = 2` — без него; это один и тот же
момент. Значит `finish_move` уже происходит ровно тогда, когда ванильный сервер
шлёт Block Update, и добавлять к нему нечего. Даже если считать, что вики имеет
в виду ровно 3 такта задержки, добавка была бы 1, а не 2.

Комментарий у константы («ход у клиента длится три такта, а у нас блоки встают
через два… добавлен ещё один — запас») опирается на буквальное прочтение «3
ticks» и на опасение оборвать движение. Опасение напрасное: Block Update,
пришедший в любой момент, движение не обрывает — клиент рисует по своей
блок-сущности и всё равно ставит блоки сам на 8-м такте (§ 2).

**Чем грозит.** Само по себе — почти ничем: 4 < 8, в клиентскую страховку мы
укладываемся. Но 2 лишних такта мир сервера и картинка клиента заведомо
разные, и всё, что за эти 2 такта прочитает новый блок (наблюдатель,
сравнитель, следующий поршень), отправит клиенту изменения, ссылающиеся на
блоки, которых у него ещё нет. А главное — эта задержка открывает § 6.4.

### 6.4 `tell_later` запоминает состояние в момент вызова, а не в момент отправки

**Наше место.** `src/world/mod.rs:609-614`:

```
pub fn tell_later(&mut self, x: i32, y: i32, z: i32, delay: u64) {
    let state = self.get_block(x, y, z);
    self.late.push((BlockChange { x, y, z, state }, self.tick + delay.max(1)));
}
```

и `src/world/mod.rs:833` — в начале такта отложенное просто перекладывается в
журнал изменений.

**Чем грозит.** Снимок состояния делается за 2 такта до отправки. Если за эти
2 такта блок успел смениться (натекла вода, его толкнул соседний поршень,
игрок его сломал), настоящее изменение уйдёт клиенту **раньше**, а следом
придёт устаревший снимок и затрёт его. У клиента останется блок-призрак — до
перезахода в чанк. Это не теоретическая мелочь: поршни ставят цепочками, и
2 такта — как раз время срабатывания соседнего.

Это расхождение исчезает само, если `PISTON_TELL_LATER` станет нулём (§ 6.3):
рассылать надо в том же такте, что и ставить.

### 6.5 Действие 2 (отмена выдвигания) не шлётся никогда

> «**2** to cancel an ongoing extension. In Notchian implementations, action ID
> **2** is defined by when a piston is retracted (e.g.: due to loss of power)
> mid-extension.»
> — Java Edition protocol/Block actions, § Piston, Action IDs

**Наше место.** `src/redstone.rs:1838-1842`: если ход уже идёт, `block_event`
выходит молча — «начатый ход не прерывается». Констант у нас только две:
`PISTON_EXTENDS = 0`, `PISTON_RETRACTS = 1` (`src/redstone.rs:35-36`); тройки
не существует.

**Чем грозит.** Если питание пропадает посреди выдвигания, наш сервер доводит
ход до конца, а ванильный — отменяет. Это расхождение прежде всего **в
механике** (нам её всё равно предстоит делать), но и в протоколе: пока
действия 2 нет, клиенту нечем сообщить об отмене, и он доиграет выдвигание и
поставит блоки по-своему на 8-м такте — то есть покажет то, чего на сервере
нет.

Заодно на будущее: у липкого поршня отмена тянет **ничего** («it will be
retracted without pulling any blocks»), у обычного — ведёт себя как обычное
задвигание.

### 6.6 Расчёт связки обязан совпадать с клиентским — и это ничем не проверено

> «The calculation for blocks to be pushed/destroyed is the same on both server
> and client, and they rely on that assumption. Attempting to change the logic in
> one side may lead to de-sync issues.»
> — Java Edition protocol/Block actions, § Piston, warning

**Наше место.** `push_group` (`src/redstone.rs:1691`), `pulled_group`
(`src/redstone.rs:1990`), `PUSH_LIMIT = 12` (`src/redstone.rs:69`).

Лимит верный. Но всё остальное — порядок обхода липких соседей
(`NEIGHBOUR_ORDER`), таблица `push_reaction`, `can_be_pulled` — у нас
собственные, и любое их несовпадение с игрой даёт не «чуть иначе», а
**видимую** рассинхронизацию: клиент нарисует движение одного набора блоков, а
сервер пришлёт изменения для другого. Это не расхождение с документацией
(проверить по ней нельзя), а риск, о котором документация прямо
предупреждает — и потому он тут.

Отдельно: `start_move` при задвигании снимает голову (`set_hidden(front,
AIR)`), считает `pulled_group`, и только потом ставит служебный блок
(`src/redstone.rs:1918-1945`). Клиент считает свою связку по своему миру, где
голова в этот момент ещё на месте. Совпадёт ли результат — из документации не
следует; проверять надо игрой.

---

## 7. Чего в документации нет

- **Порядок Block Action и Block Update на проводе.** Прямо не сказано нигде.
  Правило § 1.4 выведено из предупреждения «already-extended piston has no
  effect» и из «uses the block state in their world», а не процитировано.
- **Надо ли слать Block Update на основание при выдвигании вообще.** В списке
  обязанностей сервера его нет; ни строчки про `extended=true`. Что клиент
  ставит его себе сам по Block Action — вероятно, но на вики этого нет.
- **Точный список клеток, о которых сервер шлёт Block Update в конце хода.**
  Сказано только «the pushed blocks' new block states». Про клетку головы, про
  освободившиеся клетки и про основание при задвигании — ничего.
- **Один пакет или пачка.** «through Block Update or Update Section Blocks» —
  выбор оставлен на усмотрение сервера, порога «сколько блоков — уже секция»
  нет.
- **Какой именно звук и в какой момент.** «Playing the piston extend/retract
  sound» — ни id звука, ни громкости, ни такта, ни того, шлётся ли он как
  Sound Effect с id из реестра.
- **Мигание блоков и поведение при лаге.** Разбора нет. Есть только клиентские
  «extra 5 ticks» и вопрос самой вики «Is it a lag-compensation mechanism?» —
  то есть автор страницы сам не знает.
- **Расхождение «3 ticks» и `progress`.** Вики нигде не сводит эти два числа;
  § 3.2 — наш разбор, не цитата.
- **Что делать с недоигранным ходом при перезапуске сервера.** Ни слова.
- **Как клиент ведёт себя, если Block Action пришёл, а Block Update — никогда.**
  Сказано только, что блоки он поставит сам на 8-м такте; что будет дальше, если
  сервер с ним не согласен, не описано.

---

## Ограничение

Исходный код игры (в том числе декомпилированный), client.jar/server.jar,
ванильные датапаки, а также исходный код серверных ядер, прокси и библиотек
(Paper, Spigot, Fabric, Forge, Bukkit, ViaVersion, node-minecraft-protocol,
PrismarineJS и любых других реализаций) при подготовке этого разбора **не
использовались**. Использованы только страницы minecraft.wiki, перечисленные в
начале, и наш собственный код.
