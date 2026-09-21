# Ход поршня по тактам (Java Edition)

Разбор поведения, а не кода. Источники: minecraft.wiki (страницы Piston,
Sticky Piston, Piston/Technical components, Tick, Tutorial:Zero-ticking,
Tutorial:Quasi-connectivity, Tutorial:Headless pistons), Technical Minecraft
Wiki (techmcdocs.github.io), Mojira. Полный список — в конце.

Дальше «такт» — игровой такт, 1/20 секунды. Нумерация тактов своя: такт 0 —
тот, в чью фазу block events поршень начал ход.

---

## 0. Главное в двух абзацах

Поршень не двигается в тот момент, когда узнал о питании. Он ставит в очередь
«block event» (событие блока) и исполняет его в фазе block events того же или
следующего такта. Исполнение — это и есть **начало** хода: в этот момент все
едущие блоки уже сняты со своих мест, на их местах воздух, а на местах
назначения стоят `moving_piston` с сохранённым внутри блоком.

Само движение занимает **2 такта**: блоки встают на новые места в такте 0+2,
в фазе block entity. Поэтому «2 такта» и «3 такта» в разных источниках — это
разные отсчёты, см. раздел 1.

---

## 1. Сколько тактов занимает ход

### 1.1. Задержка старта (start delay) — 0 или 1 такт

minecraft.wiki, Piston, § Start delay (дословно):

> A piston won't extend or retract immediately when it is activated: this
> phenomenon is known as "start delay".

> * If the piston is powered and updated in the [[Tick#Scheduled tick|scheduled
>   tick]] phase, [[random tick]] phase or block event phase, the piston
>   activates in this game tick's block event phase, which means the start delay
>   in this case is 0.
> * If the piston is powered and updated during the [[entity]] phase or the
>   [[block entity]] phase, or by player actions, the piston activates in the
>   next game tick's block event phase, which means the start delay in this case
>   is 1 tick.

То есть задержка зависит **не от типа поршня и не от вида сигнала, а от фазы
такта, в которой поршень получил обновление**. Порядок фаз в одном такте
(minecraft.wiki, Tick, § Game process, дословно, сокращённо):

> ** Execute [[#Scheduled_tick|Scheduled block ticks]]
> ** Execute [[#Scheduled_tick|Scheduled fluid ticks]]
> ...
> *** [[#Random tick|Random ticks]]
> ** Send block changes to players
> ...
> ** Execute [[block]] events
> ** For all [[Entity#Riding|non-passenger]] entities: ... Tick entity
> ** Tick [[Block entity|block entities]]

Фаза block events идёт **после** scheduled ticks и **до** entity/block entity.
Отсюда правило: обновление пришло раньше фазы block events в этом такте →
задержка 0; позже (сущности, блок-сущности, действия игрока) → задержка 1 такт.

Важное следствие оттуда же (дословно):

> Notably, because blocks pushed by pistons arrive during the block entity phase
> (except when [[Sticky Piston#Block dropping|dropped]] by a sticky piston), in
> a chain of pistons each pushing a redstone block to activate the next,
> successive activations will happen 3 ticks apart.

### 1.2. Длительность самого движения — 2 такта

minecraft.wiki, Piston (дословно):

> When powered, the piston's wooden surface (the "head") tries to start
> extending after a [[#Start delay|start delay]]. The extension takes 2 game
> ticks to finish.

> When a piston loses power, its head retracts. Like the extension, the
> retraction starts after a [[#Start delay|start delay]] and takes 2 game ticks
> to finish.

Technical Minecraft Wiki, Piston (дословно):

> A piston initiates its extension in the same tick it gets activated (tick
> no. 0), it replaces itself and the destination blocks of the blocks pushed
> with a moving piston (block 36). 2 ticks later (in tick no. 2), a piston is
> placed back where it was before (except now it is extended) and the pushed
> block arrive at their positions.

Механика этих двух тактов задана полем `progress` у блок-сущности
`moving_piston` (minecraft.wiki, Piston/Technical components, дословно):

> **progress**: How far the block has been moved. Starts at 0.0, and increments
> by 0.5 each tick, including the tick on which moving the piston is created. If
> the value is 1.0 or higher at the *start* of a tick of the block entity
> (before incrementing), then the block transforms into the stored blockState.
> Negative values can be used to increase the time until transformation.

Раскладка из этого правила:

| Такт | Фаза block entity | progress до | progress после |
|---|---|---|---|
| 0 | создана, тикается в этом же такте | 0.0 | 0.5 |
| 1 | тикается | 0.5 | 1.0 |
| 2 | 1.0 ≥ 1.0 → превращение | 1.0 | — |

Итого: блок встаёт на место в фазе block entity такта 0+2.

### 1.3. Откуда берутся «3 такта»

Три разных счёта, которые все встречаются в текстах:

1. **2 такта** — длительность движения от начала хода до постановки блоков.
   Это то, что написано на minecraft.wiki и в Technical Minecraft Wiki.
2. **3 такта** — полный период «сигнал → блок на месте → этот блок успел
   запитать следующий поршень». Ровно то же, что цитата выше про «successive
   activations will happen 3 ticks apart»: блоки приезжают в фазе block entity,
   а это уже **после** фазы block events, поэтому следующий поршень получает
   задержку старта 1 такт. 2 такта движения + 1 такт задержки старта = 3.
3. **3 «поршневых такта»** — отдельная единица счёта китайского редстоун-
   сообщества. minecraft.wiki, Tick, § Piston tick (дословно):

   > The '''piston tick''' is a different way to divide the game tick, based on
   > the ''Immediate Update Theory'' by Sancarn and Selulance. It was created by
   > the Chinese redstone community in order to simplify the calculation of
   > piston timings, but it never gained popularity in the Western community.

   > The piston tick starts in the entity phase of the game tick and ends in the
   > block event phase, therefore a piston always takes 3 piston ticks to extend
   > or retract, without any [[piston#Start_delay|start delay]].

   Это не другое поведение, а другая нарезка того же такта: границу такта
   сдвинули так, что задержка старта «растворилась» в счёте, зато ход стал
   считаться за 3 единицы.

**Вывод:** расхождения между «2» и «3» нет. Движение — 2 такта. Задержка старта
— 0 или 1 такт сверху. Наблюдаемый период цепочки поршень→блок→поршень — 3 такта.

---

## 2. Итоговая таблица: выдвигание (обычный и липкий одинаково)

Отсчёт от момента, когда поршень получил обновление и питание.

| Момент | Что происходит |
|---|---|
| Такт −1 или 0, любая фаза | Поршень получил обновление. Он проверяет, может ли он выдвинуться, и, если да, **создаёт block event** (позиция + действие). Дубликаты не создаются: если такое событие уже есть, второе не добавляется. Никакого движения ещё нет. |
| Такт 0, фаза block events | Событие исполняется в порядке создания (FIFO). Поршень **проверяет питание ещё раз** и проверяет, не заблокирован ли он. Если проверка не прошла — ход не начинается вовсе. |
| Такт 0, фаза block events, начало хода | Строятся два списка: какие блоки двигать и какие ломать. Дальше по порядку: (1) на месте назначения каждого едущего блока ставится `moving_piston`; (2) рассылаются state-обновления в позициях новых `moving_piston`; (3) **старые блоки удаляются** (их больше нет на своих местах); (4) рассылаются редстоун-обновления вокруг всех удалённых блоков и вокруг головы; (5) последним шагом у основания ставится `extended=true`. Блоки, в которые упирается связка и которые ломаются (например, факел, трава), ломаются здесь же, в начале хода. |
| Такт 1 | Ничего внешне не происходит: `moving_piston` тикаются, `progress` 0.5 → 1.0. Едущие блоки физически «нигде»: старых мест нет, новых ещё нет. |
| Такт 2, фаза block entity | `progress` = 1.0 → все `moving_piston` превращаются: тот, что у головы (`source=true`) → `piston_head`, остальные → сохранённые в них блоки. Ход закончен. |

Про порядок в начале хода — Technical Minecraft Wiki, Piston, § Block updates
(дословно):

> The game first creates moving blocks (B36) in front of each block to be moved.
> It then, following the update order explained above, sends state updates at
> the position of the each of the new blocks. Next, it deletes all the old
> blocks by reading from a hashmap, so the order of the state updates here is
> locational. Finally, it sends redstone block updates around all removed blocks
> and the moving piston head.

Про порядок перебора связки (дословно, там же, § Pushed blocks order):

> When the piston starts extending or retracting, it creates a list of blocks to
> move and a list of blocks to break. For this, it first searches all blocks to
> push in the line of blocks from the nearest to the furthest or to pull from
> the furthest to the nearest. If these blocks stick to others blocks, they are
> stored in the -y;+y;-z;+z;-x;+x and it search again the blocks in the line of
> the added blocks. Then the game loop through the blocks list in reverse order,
> creating moving blocks and sending block updates where they were. So blocks
> update from the furthest (in number of lines to get to this one) lines in the
> piston direction, if it's the same distance, it's in the +x;-x;+z;-z;+y;-y
> order (-y;+y;-z;+z;-x;+x reversed) and in a line it's from the furthest to the
> nearest when pushing and from the nearest to the furthest when pulling
> (reversed here again).

Про то, что `extended=true` — последний шаг начала хода, а не первый
(minecraft.wiki, Piston, § History, 1.5 / 13w10a, дословно):

> Pistons changing their `extended` state from `false` to `true` is now the
> *last* step of (the start of) the extension process, rather then the first.
> This change affects some redstone contraptions.

и там же 1.8 / 14w26a (дословно):

> Pistons do not check if they are receiving power in the moment in which their
> `extended` state changes from `false` to `true` anymore.

И про то, что поршень именно планирует, а не делает сразу (1.3.1 / 12w26a,
дословно):

> Pistons now schedule extensions and retractions rather than executing them as
> soon as they realize they are (un)powered.

---

## 3. Что происходит в начале хода — по пунктам вопроса

- **Когда едущие блоки перестают быть на своих местах.** В такте 0, в фазе
  block events, в момент начала хода. Не в конце. Цитата про порядок
  («Next, it deletes all the old blocks») выше. С этого момента и до такта 2
  на старых местах воздух, на новых — `moving_piston`.
- **Когда ставится `moving_piston`.** Тогда же, в такте 0, **до** удаления
  старых блоков: «The game first creates moving blocks (B36) in front of each
  block to be moved». Голова тоже едет как `moving_piston` с `source=true`
  (minecraft.wiki, Piston/Technical components, дословно: «**source**: 1 or 0
  (true/false) – true if the block represents the piston head itself, false if
  it represents a block being pushed»).
- **Когда у основания появляется `extended=true`.** В такте 0, последним шагом
  начала хода (цитата про 13w10a выше). Само основание при выдвигании остаётся
  блоком `piston`, `moving_piston` ставится только на месте будущей головы и на
  местах назначения толкаемых блоков.
- **Когда ломается то, во что упирается связка.** Список «блоков на слом»
  строится вместе со списком «блоков на движение» в начале хода («it creates a
  list of blocks to move and a list of blocks to break»), то есть в такте 0.
  Отдельной цитаты, что слом происходит именно между созданием `moving_piston`
  и удалением старых блоков, найти не удалось — см. раздел «чего не нашлось».

---

## 4. Что происходит в конце хода

minecraft.wiki, Piston/Technical components, § Moving piston (дословно):

> At the end of the piston stroke, the *moving piston* blocks are replaced with
> either the carried block, the piston head (during extensions), or the piston
> itself (during retractions); but if it is placed with the use of commands it
> remains indefinitely.

То есть в такте 2, в фазе block entity:

- `moving_piston` с `source=true` → `piston_head` (при выдвигании) либо сам
  блок поршня `piston`/`sticky_piston` с `extended=false` (при задвигании);
- остальные `moving_piston` → блоки, лежащие у них внутри в `blockState`;
- снятие `moving_piston` и постановка конечного блока — это одно и то же
  действие превращения, отдельного шага «сначала снять, потом поставить» в
  описаниях нет.

Порядок между несколькими `moving_piston` в конце хода нигде явно не описан:
они тикаются как обычные блок-сущности в фазе block entity, а порядок обхода
блок-сущностей в описаниях не зафиксирован. См. «чего не нашлось».

---

## 5. Задвигание

Technical Minecraft Wiki, Piston (дословно):

> When the piston gets deactivated it immediately (tick no. 5) starts retracting
> even if it was already in the process of extension. It replaces itself with a
> moving piston. If it is a sticky piston, it also tries to pull blocks and
> places moving pistons at their destinations. 2 ticks later (in tick no. 2) a
> piston is placed back where it was before and the pulled blocks arrive at
> their positions.

(«tick no. 5» здесь — просто номер такта в их сквозном примере, а «2 ticks
later (in tick no. 2)» — опечатка того же примера: имеется в виду «через
2 такта».)

Ключевое отличие от выдвигания: **при задвигании `moving_piston` ставится на
место самого основания поршня** — «It replaces itself with a moving piston», и
в конце хода там снова появляется поршень («the piston itself (during
retractions)»). То есть все 2 такта задвигания на месте поршня стоит не поршень,
а технический блок.

| Момент | Обычный поршень | Липкий поршень |
|---|---|---|
| Такт −1 или 0 | Поршень обновлён, питания нет → создаётся block event на задвигание. Важно: задвигание **не может провалиться** — в худшем случае оно просто ничего не притянет. | То же |
| Такт 0, фаза block events | Повторная проверка: «If it is trying to pull, it checks if it is unpowered». Если поршень снова запитан — задвигание отменяется. | То же |
| Такт 0, начало хода | `piston_head` со своего места исчезает; на месте основания ставится `moving_piston` (`source=true`, `extending=false`), внутри — блок поршня. Блок перед головой **не трогается**, остаётся на месте. | То же плюс: выбирается притягиваемый блок (он в 2 клетках перед основанием, то есть вплотную к голове), он удаляется со своего места, а на месте бывшей головы ставится `moving_piston` с ним внутри. Рассылаются обновления. |
| Такт 1 | `progress` 0.5 → 1.0 | То же |
| Такт 2, фаза block entity | `moving_piston` на месте основания → `piston`/`sticky_piston` с `extended=false`. | То же плюс: `moving_piston` на месте бывшей головы → притянутый блок. |

Ответы на подвопросы:

- **Когда исчезает голова.** В такте 0, в начале задвигания. Голова не едет
  отдельным блоком `piston_head` — она едет внутри `moving_piston`, который
  стоит на месте основания.
- **Когда тянущийся блок трогается.** В такте 0, в начале хода: он удаляется со
  своего места и превращается в `moving_piston` на месте бывшей головы.
- **Когда встаёт.** В такте 2, в фазе block entity.
- **Если тянуть нечего или нельзя** (minecraft.wiki, Sticky Piston, дословно):

  > Sticky pistons have the same [[piston#Limitations|limitations for pushing]]
  > as normal pistons. These limitations also apply for pulling: when a sticky
  > piston is unpowered but cannot pull a block, it retracts without doing so.

- **Падающий блок не тянется** (minecraft.wiki, Sticky Piston, дословно):
  «A sticky piston cannot pull a [[Falling Block|falling block]].»

---

## 6. Прерывание хода

### 6.1. Питание сняли посреди выдвигания

minecraft.wiki, Piston (дословно):

> In *Java Edition*, a piston that is de-powered during the extension process
> aborts its extension and starts to retract (assuming it can detect the signal
> change). Any block which was being pushed by that piston continues to move as
> if the piston was still pushing it.

Technical Minecraft Wiki, Piston (дословно):

> If the extension of a sticky piston is interrupted (by its retraction), the
> directly pushed block arrives immediately (in the same tick as the retraction
> starts). The retraction does not pull the block and therefore it is dropped.
> This can be done using a 0-tick pulse, 1-tick pulse or 2-tick pulse. The
> indirectly pushed blocks arrive at the same time they would normally.

То есть: прерванное выдвигание не откатывается. Оно **досрочно завершается** —
первый толкаемый блок мгновенно ставится на своё конечное место (не на старое!),
остальные едут как ехали и приезжают в свой обычный такт 0+2.

### 6.2. «Block dropping» у липкого поршня

minecraft.wiki, Sticky Piston, § Block dropping (дословно):

> A sticky piston finishes extending early and starts retracting if it loses
> power before the extension process is over (assuming it can detect the signal
> change). If the sticky piston was pushing one or more blocks during the
> extension, the first block ends up in its final position immediately and all
> the other blocks continue moving as if they were still being pushed.

> This behavior is intended and is referred to as "block dropping" or sometimes
> "block spitting".

> Block dropping only works for blocks that were being pushed by the sticky
> piston before its extension was canceled—if it wasn't pushing any, the sticky
> piston attempts to pull blocks as usual once it starts retracting.

> With block dropping it's possible to push (but not pull) a waterlogged block
> without causing the water to disappear.

Суть: липкий поршень **не притягивает** блок обратно, потому что когда
задвигание начинается, перед головой стоит `moving_piston`, а его тянуть нечего
— он не обычный блок. Из Technical Minecraft Wiki (о 0-тактовом импульсе):

> The piston gets updated twice in the same tick and schedules 2 block events
> for the next time they will get processed. The first one gets processed and
> creates a piston extension block, but when the second one gets processed, the
> piston sees there's nothing powering it, and if it's a sticky piston, it
> attempts to pull in the block in front of it, but sees a block 36 (the
> technical term for moving blocks) and does nothing.

(Эта цитата пришла через поисковую выдачу по статье Technical Minecraft Wikia
«0-tick pulses»; сама страница Fandom отдаёт 402/Cloudflare и напрямую не
читалась — считать её менее надёжной, чем techmcdocs и minecraft.wiki.)

Impulses по длине (minecraft.wiki, Tutorial:Zero-ticking, дословно):

> If a [[sticky piston]] is powered by a 2-[[tick]] pulse, it starts extending;
> after two ticks, the piston drops its block and starts retracting. If a sticky
> piston is powered by a 1-tick pulse, the same happens after only one tick.

> When a piston receives a pulse that turns on and off in the same tick, this is
> known as a 0-tick pulse. This causes sticky pistons to instantly drop their
> block and start retracting. Regular [[piston]]s start retracting instantly
> when powered by a 0-tick pulse but do not instantly teleport blocks. Because a
> 0-tick pulse turns on and off in the same tick, many 0-tick pulses do not
> render. Due to how 0-tick pulses work, 0-tick pulses are unable to
> [[bud-power]] pistons in some circumstances.

Обычные (нелипкие) поршни сейчас блоки не «дропают»: minecraft.wiki, Piston,
§ History — 1.13 pre7 «Pistons now have the same block dropping behavior of
sticky pistons» (MC-133273), затем 1.13.1 / 18w30b «Pistons don't drop blocks
anymore». То есть в 1.21+/26.1 block dropping — свойство **только липкого**
поршня (для нелипкого 0-тактовый импульс даёт досрочное задвигание без
телепортации блока).

### 6.3. Сигнал пришёл во время хода

- **Во время выдвигания** — поршень остаётся обычным блоком `piston` с
  `extended=true`, его можно обновить и он может создать block event на
  задвигание. Отсюда и весь block dropping.
- **Во время задвигания** — на месте поршня стоит `moving_piston`, а не поршень.
  Блок-событие на такую позицию адресовать некому, и задвигание довести до конца
  придётся. Прямой цитаты «задвигание нельзя прервать» в источниках не нашлось;
  это следствие из «It replaces itself with a moving piston». Помечаю как вывод,
  а не как цитату.
- **Микротакты** (Technical Minecraft Wiki, Piston, дословно):

  > A 0-tick pulse can be said to have a certain duration in microticks. A
  > microtick pulse duration determines how many block events can execute before
  > the pulse ends. A 1-microtick pulse does not affect pistons at all, since by
  > the time the block events are executed the pulse has already ended and
  > therefore cannot power the piston. A 2-microtick pulse is enough to power
  > pistons that were updated by the rising-edge of the pulse. A 3-microtick
  > signal can power pistons updated by the execution of the block events created
  > by the pistons updated by the rising-edge of the pulse and so on…

- **Отмена ещё до начала хода** (Technical Minecraft Wiki, дословно):

  > When the piston executes its block event, it checks again if it can still do
  > its action. If it is trying to push, it checks that it is powered and not
  > blocked. If it is trying to pull, it checks if it is unpowered.

  Плюс (дословно):

  > Note that a piston trying to extend can fail (even if later, in the block
  > event phase, it can extend) even though a retraction can't fail (because in
  > the worst case, it retracts without pulling a block).

---

## 7. Как это видно снаружи, если снимать блоки в конце хода, а не в начале

Ниже — разбор последствий. Факты о ванильном поведении подтверждены цитатами
выше; выводы о том, что именно сломается, — мои, они помечены.

Что именно меняется при «снятии в конце»: 2 такта, в течение которых у ванилы
на старом месте **воздух**, а у такой реализации — по-прежнему исходный блок.

Наблюдаемые отличия (выводы):

1. **Задержка реакции всего, что смотрит на исходное место.** Наблюдатель,
   сравнитель, редстоуновая пыль, факел, освещение, вода — у ванилы получают
   обновление «блока не стало» в такте 0. При снятии в конце они получат его в
   такте 2. Все схемы, где «поршень убрал блок → сигнал пошёл дальше», уедут на
   2 такта. Это ломает любую схему с фиксированными задержками повторителей.
2. **Питание/распитывание через двигаемый блок.** Если поршень толкает блок
   редстоуна, то у ванилы этот блок перестаёт питать старое место в такте 0, а
   начинает питать новое — в такте 2. Между ними 2 такта, когда он не питает
   **ничего**. При снятии в конце этого «окна тишины» не будет вовсе, а вместо
   него на один такт появится ситуация «питает оба места сразу». Это прямо
   ломает 0-тактовые генераторы (цитата: «All 0-tick pulses are created by
   powering a redstone line then removing the power source with pistons later in
   the tick») и все «piston feed tape».
3. **Block dropping и любые схемы на прерванном ходе.** Они держатся на том,
   что в момент начала задвигания перед головой стоит `moving_piston`, а не
   обычный блок. Если старый блок ещё лежит на месте, а нового `moving_piston`
   нет, липкий поршень при задвигании увидит обычный блок и **притянет** его —
   то есть block dropping, 0-тактовые толкатели, instant retraction, 0-тактовые
   повторители и 0-тактовые часы перестанут работать вообще.
4. **Невозможность «затолкать» блок в сущность.** Из Tutorial:Zero-ticking
   (дословно): «Zero-ticking a block places it over the space occupied by the
   entity without moving the entity.» Это работает только потому, что блок
   ставится досрочно, минуя обычную фазу. Пропадёт.
5. **Проходимость и коллизии во время хода.** У ванилы 2 такта на старом месте
   можно стоять/лететь, а на новом стоит `moving_piston` с ограничениями. При
   снятии в конце наоборот. Ломает летающие машины и «переброс» сущностей
   поршнями (piston translocation).
6. **Ломаемые блоки.** Факел или трава, в которые упёрлась связка, у ванилы
   исчезают в такте 0 (и в этот момент выпадают предметом). При снятии в конце
   они проживут лишние 2 такта и успеют разослать обновления позже, чем надо.
7. **BUD.** Вся начальная порция обновлений (state-обновления у новых
   `moving_piston`, редстоун-обновления вокруг удалённых блоков и вокруг головы)
   у ванилы приходится на такт 0. При снятии в конце BUD-поршни сработают на
   2 такта позже или не сработают вовсе, если обновление придёт в другой фазе
   (это снова меняет задержку старта с 0 на 1).
8. **Порядок внутри самого начала хода.** Даже если снимать блоки в начале, но
   в другом порядке (сначала удалить старые, потом поставить `moving_piston`),
   получится другой порядок обновлений — а он в ваниле зафиксирован цитатой
   «The game first creates moving blocks (B36)... Next, it deletes all the old
   blocks... Finally, it sends redstone block updates». Схемы, чувствительные к
   порядку обновлений (обновляющие цепочки, update suppression, BUD), увидят
   разницу.
9. **Чего это НЕ меняет.** Видимая анимация клиента идёт от блок-сущности
   `moving_piston` и её `progress`; если сервер всё равно шлёт клиенту
   `moving_piston` с корректным `progress`, картинка останется той же. Разница
   будет только в поведении схем.

---

## 8. Квазисвязность и тайминги вместе: когда поршень проверяет питание

**Дважды.** Technical Minecraft Wiki, Piston, § Activation mechanics (дословно,
целиком, потому что здесь всё важно):

> A piston starts extending or retracting in the block event phase of a tick. A
> piston can be powered by a block directly adjacent to it or by
> quasi-connectivity (by a block that would power the block above the piston).
> When a piston is updated, it checks if it should retract or extend. If it can
> it creates a block event containing its position and what action the piston
> should do, if the block event doesn't already exist (except in 1.15 where
> block events doesn't have a working hashcode, so one block can have multiple
> block events). Note that a piston trying to extend can fail (even if later, in
> the block event phase, it can extend) even though a retraction can't fail
> (because in the worst case, it retracts without pulling a block). When the
> block event phase starts, the game loops through the block event list, and
> executes block events in the order of creation (first in, first executed).
> When the piston executes its block event, it checks again if it can still do
> its action. If it is trying to push, it checks that it is powered and not
> blocked. If it is trying to pull, it checks if it is unpowered.

Итого:

1. **При обновлении блока** — решает, что делать, и ставит block event в очередь.
   На этой проверке уже учитывается квазисвязность.
2. **В фазе block events, перед самым ходом** — проверяет ещё раз. Именно вторая
   проверка даёт block dropping при 0-тактовом импульсе: block event создан,
   а к моменту исполнения питания уже нет.
3. **После хода** отдельной проверки нет; следующее решение поршень примет,
   только когда его снова обновят. Отсюда «залипшие» (budded) поршни.

Ключевое про квазисвязность (minecraft.wiki, Piston, дословно):

> Pistons can be powered from one block above compared to most redstone
> components: this property is called [[quasi-connectivity]] (QC). With
> quasi-connectivity, a piston facing up can also be powered from the head's
> direction, which is otherwise impossible. Due to quasi-connectivity, a piston
> facing up that has a [[block of redstone]] on top of itself can extend but not
> retract: this happens because after the piston extends, it keeps receiving
> power from the redstone block.

> Quasi-connectivity can be used to make a [[Tutorial:Block update detector|BUD
> switch]], taking advantage of the fact that a piston powered through QC doesn't
> always get immediately [[block update|updated]].

И из Tutorial:Quasi-connectivity: компоненты обновляют блоки максимум на 2
клетки по манхэттенскому расстоянию, а квазисвязность создаёт ситуации, когда
поршень должен был бы сработать от компонента в 3 клетках — тогда он не
срабатывает, пока не придёт обновление откуда-нибудь ещё. Это и есть BUD.

Важно для реализации: **квазисвязность влияет только на то, считается ли поршень
запитанным, и никак не влияет на тайминги**. Задержка старта определяется фазой
обновления, а не тем, обычное питание или QC. Но поскольку QC-поршень часто
получает обновление позже (или вообще только от постороннего BUD-события),
наблюдаемая задержка у него бывает другой.

---

## 9. Где источники расходятся

1. **«2 такта» против «3 тактов».** minecraft.wiki и Technical Minecraft Wiki
   говорят «2 такта» про само движение; в форумных постах и обзорных статьях
   часто фигурирует «3 такта». Разобрано в разделе 1.3: это разные отсчёты, а не
   разное поведение. Надёжнее «2 такта движения + 0/1 такт задержки старта»,
   потому что этот счёт прямо выводится из документированного поведения поля
   `progress`, а «3» — производная величина, верная только для конкретного
   сценария (цепочка поршней с блоками редстоуна) или для «поршневых тактов».

2. **Нумерация тактов в Technical Minecraft Wiki.** В абзаце про задвигание
   стоит «it immediately (tick no. 5) starts retracting» и тут же «2 ticks later
   (in tick no. 2)» — числа не сходятся, это явная опечатка сквозного примера.
   Содержательно «через 2 такта» согласуется с minecraft.wiki и с формулой
   `progress`, поэтому беру «2 такта».

3. **Что именно становится `moving_piston` при выдвигании.** Technical Minecraft
   Wiki пишет «it replaces itself and the destination blocks of the blocks
   pushed with a moving piston» — читается так, будто и основание тоже
   становится `moving_piston`. minecraft.wiki же говорит, что `extended=true` —
   последний шаг начала выдвигания (13w10a), то есть основание остаётся блоком
   `piston`, просто с другим состоянием, а `moving_piston` появляется на месте
   будущей головы. Считаю надёжнее вариант minecraft.wiki: иначе block dropping
   был бы невозможен (нечему было бы получать второй block event), а он есть.
   Для задвигания обе вики согласны: `moving_piston` ставится на месте самого
   основания.

4. **Обычный поршень и block dropping.** Много старых текстов утверждает, что
   «дропают блоки только липкие». История на minecraft.wiki показывает, что в
   1.13 pre7 это ненадолго распространили и на обычные (MC-133273), а в 18w30b
   убрали. Для 1.21+/26.1 верно: только липкие.

5. **Fandom-вики (technical-minecraft.fandom.com) против techmcdocs.github.io.**
   Первая отдаёт 402/Cloudflare и цитировалась только через поисковые сниппеты;
   вторая читалась напрямую и написана подробнее. При расхождении опираюсь на
   techmcdocs и minecraft.wiki.

---

## 10. Чего не нашлось нигде

- **Точный момент слома блоков внутри начала хода.** Известно, что список «на
  слом» строится вместе со списком «на движение», но идёт ли слом до создания
  `moving_piston`, между ним и удалением старых блоков, или после — ни один
  источник не пишет. Также не описано, в каком порядке ломаются несколько
  блоков сразу.
- **Порядок превращения нескольких `moving_piston` в конце хода.** Это фаза
  block entity, но порядок обхода блок-сущностей в описаниях не зафиксирован.
  Ни на minecraft.wiki, ни на techmcdocs порядка нет.
- **Явного утверждения «задвигание нельзя прервать».** В разделе 6.3 это вывод
  из того, что основание на 2 такта перестаёт быть поршнем, а не цитата.
- **Что происходит, если во время хода целевую позицию занимает что-то ещё**
  (например, туда поставили блок командой): описания нет.
- **Как ведут себя `moving_piston` при выгрузке/загрузке чанка посреди хода**
  (застывает ли `progress`): описания нет. Известно только, что
  `moving_piston`, поставленный командой, не имеет блок-сущности и остаётся
  навсегда.
- **Численные тайминги для Bedrock** здесь не разбирались специально; там
  задержка старта фиксированная, 2 такта, и активация только на C-tick
  (цитата в разделе 1.1). Нам это не нужно, но полезно помнить: любые
  «правильные» тайминги, взятые из Bedrock-роликов, к Java не относятся.
- **Отдельного описания «instant retraction» как термина** на minecraft.wiki
  нет — это то же самое явление, что block dropping / 0-tick, просто под другим
  названием в роликах сообщества.

---

## 11. Чем пользовался

- minecraft.wiki, [Piston](https://minecraft.wiki/w/Piston) — разделы Redstone
  component, Start delay, Quasi-Connectivity, Sticky blocks, Limitations,
  History, Block states.
- minecraft.wiki, [Sticky Piston](https://minecraft.wiki/w/Sticky_Piston) —
  разделы Block dropping, Limitations, History.
- minecraft.wiki,
  [Piston/Technical components](https://minecraft.wiki/w/Piston/Technical_components)
  — Piston head, Moving piston, Block data (`blockState`, `extending`, `facing`,
  `progress`, `source`).
- minecraft.wiki, [Tick](https://minecraft.wiki/w/Tick) — Game process,
  Scheduled tick, Redstone tick, Half–redstone tick, Piston tick.
- minecraft.wiki,
  [Tutorial:Zero-ticking](https://minecraft.wiki/w/Tutorial:Zero-ticking).
- minecraft.wiki,
  [Tutorial:Quasi-connectivity](https://minecraft.wiki/w/Tutorial:Quasi-connectivity).
- minecraft.wiki,
  [Tutorial:Headless pistons](https://minecraft.wiki/w/Tutorial:Headless_pistons).
- minecraft.wiki,
  [Redstone circuits/Piston](https://minecraft.wiki/w/Redstone_circuits/Piston).
- Technical Minecraft Wiki,
  [Piston](https://techmcdocs.github.io/pages/Blocks/Piston/) — Activation
  mechanics, Movement, Microticks, Pushed blocks order, Block updates. Основной
  источник по порядку действий внутри такта.
- Technical Minecraft Wikia (Fandom), страницы
  [Piston Mechanics](https://technical-minecraft.fandom.com/wiki/Piston_Mechanics)
  и [0-tick pulses](https://technical-minecraft.fandom.com/wiki/0-tick_pulses)
  — только через поисковые сниппеты, сайт отдавал 402/Cloudflare.
- Mojira (через ссылки на вики): MC-5726 (block dropping — WAI), MC-8328
  (0-tick), MC-122711, MC-122911, MC-130183, MC-133273, MC-27056.

**Ограничение соблюдено:** исходный код Minecraft (в том числе декомпилированный),
client.jar/server.jar, ванильные датапаки, а также код серверных ядер и модов
(Paper, Spigot, Fabric, Forge, Bukkit, ViaVersion, node-minecraft-protocol и
любых других реализаций) не использовались. Использованы только вики, описания
поведения, баг-трекер и технические разборы сообщества.
