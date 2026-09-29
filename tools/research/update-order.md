# Порядок обновлений, фазы такта и редстоун 1:1 с Java 26.1.2

Задача разбора — собрать в одном месте всё, что нужно, чтобы обновления
блоков, такты и поршни у нас вели себя ровно как у оригинала Java 26.1.2
в обычной игре (без экспериментов). Для каждого пункта сказано, что у нас
сейчас делается иначе, с именами функций.

Опирается на прежние разборы и не повторяет их целиком:
[redstone-core.md](redstone-core.md) (таблицы NC/PP, приоритеты),
[audit-updates.md](audit-updates.md) (аудит от 20.09, часть уже исправлена),
[piston-timing.md](piston-timing.md) (ход поршня по тактам, прерывание,
block dropping — там подробно).

**Источники** — только описания поведения: minecraft.wiki (Block update, Tick,
Random Tick, Chunk, Chunk format, Piston, Sticky Piston,
Piston/Technical components, Tutorial:Quasi-connectivity,
Tutorial:Zero-ticking, Redstone Experiments, Server.properties, страницы
версий 22w11a/22w12a/1.19/24w33a), Technical Minecraft Wiki
(techmcdocs.github.io), Minecraft Discontinued Features Wiki (mcdf.wiki.gg),
тексты отчётов Mojira (MC-11193, MC-81098, MC-189954, MC-249082, MC-249181,
MC-5726, MC-108), разбор SpaceWalkerRS об экспериментальном редстоуне
(только прозаические места). Кода игры, ядер, модов и протокольных
библиотек не открывалось. Там, где страница показывала фрагмент кода или
имена методов, они пропущены и не использованы.

**Про версию.** Страницы вики — свежие и местами уже описывают 26.2
(на странице Tick упомянут Sulfur Cube, на Random Tick — sulfur spike). Такое
здесь отброшено. Экспериментальный редстоун из 1.21.2 («Redstone
Experiments») в 26.1.2 по-прежнему **только эксперимент**: страница вики
помечена `{{experimental}}`, а отчёт MC-11193 о «случайном» порядке пыли
до сих пор открыт. Значит, для обычного мира 26.1.2 действует **старый**
алгоритм пыли — он и описан ниже; экспериментальный — только для справки
(§ 2.6).

---

## 0. Самое важное коротко

1. Обновления идут **в глубину**; с 1.19 это не рекурсия, а явный стек
   с пределом 1 000 000 записей за цепочку. Порядок при этом тот же, что
   был при рекурсии (очередь из 22w11a откатили в 22w12a).
2. Внутри одной смены блока порядок рассылки фиксирован: сперва особые
   обновления снятого блока (−2), потом поставленного (−1), потом общее NC
   (з, в, н, вх, с, ю), потом обновления сравнителей, потом PP (з, в, с, ю,
   н, вх), потом «поздние» (3). У нас особые −1/−2 идут **после** общего NC.
3. Пыль в 26.1.2 гаснет ступеньками (линия из 15 даёт 64 смены силы) и на
   каждую смену шлёт 42 NC; порядок «опорных точек» зависит от **координат**
   (MC-11193 открыт). Мы обходим в фиксированном порядке сторон.
4. Фазы такта: запланированные блочные → жидкостные → чанки (случайные
   такты) → **блочные события** (поршни, нотные блоки) → сущности →
   **блок-сущности** (конец хода поршня, воронки). У нас конец хода поршня
   в фазе запланированных тактов, события, заведённые во время фазы
   событий, ждут следующего такта, а прерывания хода нет.
5. Запланированный такт привязан к паре «место + вид блока», у пары
   может быть только один ожидающий такт. У нас ключ — только место.

---

## 1. Виды обновлений

### 1.1. NC — «сосед изменился»

> «Most blocks send NC updates to their immediate neighbors when they are
> placed, destroyed, or replaced, in this order: west, east, down, up, north,
> south.» — https://minecraft.wiki/w/Block_update

Это **общее NC**. У части изменений его нет вовсе — длинный список «Some
changes do not produce General NC updates» на той же странице: повторитель
(зажёгся/погас), сравнитель (сигнал или режим), наблюдатель, лампа, двери,
люки, калитки, раздатчик/выбрасыватель/воронка (включились), нажимные плиты
(сигнал), заборы/ступени/панели/стены (форма), котёл, музыкальный ящик,
огонь, растения при росте и т.д., а также **любой блок, поставленный
командой или изменённый отладочной палкой**, и **поршни с кроватями**
(у них своя логика).

Отсюда следствие, которое вики не проговаривает, но которое вытекает из
того, что этот список — перечень исключений: **смена состояния прочих
блоков общее NC шлёт** (рычаг, кнопка, факел, редстоуновая руда при
свечении и т.п.).

Особые NC (полная таблица — redstone-core.md § 7.3): рычаг/кнопка/плита/крюк —
свои соседи, затем соседи опоры; повторитель/сравнитель/наблюдатель — блок
перед выходом, затем его соседи без самого диода; факел — соседи каждого
из шести соседей, соседи факела перебираются **н, вх, с, ю, з, в**; пыль —
см. § 2. Везде внутренний обход соседей — **з, в, н, вх, с, ю**.

**Кто реагирует на NC** (дословный список вики): «Frosted ice … Redstone
dust, Redstone repeater, Redstone comparator, Redstone torch, Rail, Detector
rail, Activator rail, Powered rail, Command blocks, Dropper, Dispenser, Note
block, Redstone lamp, TNT, Piston, Trapdoors, Fence gates, Doors … Water,
Lava … Sponge». Пыль на NC **пересчитывает только силу**, форму — нет.

### 1.2. PP — «пост-постановка», оно же обновление формы

> «When a block is placed, destroyed, replaced, or its blockstate is changed,
> the game sends PP update to its immediate neighbors, in this order: west,
> east, north, south, down, up.» — https://minecraft.wiki/w/Block_update

PP шлётся и при **смене состояния** — в том числе у лампы, двери, диода,
у которых общего NC нет. Поэтому наблюдатель видит, как зажглась лампа.

Исключение пыли (дословно):

> «When a redstone wire is placed, destroyed, replaced, or its blockstate is
> changed, for any block that the redstone wire points horizontally to, if it
> is not a redstone wire, then the game also sends PP updates to the block
> above it and the block below it. However, it does not send PP update to an
> observer.»

Именно так пыль узнаёт о пыли по диагонали (на ступеньку выше/ниже) и
перестраивает форму.

**Кто реагирует на PP** (вики): блоки, которым нужна опора (факелы, двери,
ковры, снег…), соединяющиеся (заборы, ступени, сундуки, **пыль**, портал);
**повторитель — проверка замка**; нотный блок — инструмент; листва; бетонная
пыль; вода; **наблюдатель**; падающие блоки; тропинка, грядка, кактус,
кораллы, магма, песок душ. У PP учитывается направление: «a wheat block
accepts only PP updates from beneath»; голова поршня снимается только по PP
**сзади** («removed when receiving a shape update from behind» —
https://minecraft.wiki/w/Piston/Technical_components).

Наблюдатель в Java ловит именно PP: «an observer detects shape updates
coming from the block it's facing» — https://minecraft.wiki/w/Observer.

### 1.3. Порядок внутри одной смены блока

Вики задаёт условные «приоритеты» — это **последовательность** рассылки при
одном изменении, не путать с приоритетами тактов:

| № | Что |
|---|---|
| −3 | обновления до изменения |
| −2 | особые обновления **снятого** блока (слом/замена: пыль, факел, активный рычаг, диод…) |
| −1 | особые обновления **поставленного** блока (постановка пыли, факела, диода; смена состояния факела и диода) |
| 0 | общее NC (з, в, н, вх, с, ю) |
| 1 | общие обновления сравнителей |
| 2 | общее PP (з, в, с, ю, н, вх) |
| 3 | поздние: включение рычага/кнопки/плиты, смена силы пыли, смена силы сравнителя, точка⇄крестик, постановка командой |

Источник — таблица «Priorities» на https://minecraft.wiki/w/Block_update.
Смысл: при постановке пыли её двухблочные обновления (−1) уходят **раньше**
общего NC шести соседям; при переключении рычага его обновления (3) —
**после** общего NC и PP.

### 1.4. Цепной обработчик с 1.19: стек, а не рекурсия; предел 1 000 000

- До 1.19 обновления шли рекурсивными вызовами; на этом держалось
  подавление через переполнение стека
  (https://mcdf.wiki.gg/wiki/Java_Edition:Update_Suppression).
- 22w11a: «Added server property max-chained-neighbor-updates to limit the
  number of consecutive neighbor updates before skipping additional ones.
  Negative values remove the limit.» — https://minecraft.wiki/w/Java_Edition_22w11a.
  По умолчанию `max-chained-neighbor-updates=1000000` —
  https://minecraft.wiki/w/Server.properties.
- В 22w11a порядок **сменился со стека на очередь**: «As of 22w11a, the
  update order has been changed from a stack to a queue» (MC-249082,
  https://mojira.dev/MC-249082). Это перевернуло порядок у рельсов:
  «while rails in 1.18 (or earlier) update in a sequence opposite of its
  sequence of being powered, they now update in the same order»
  (MC-249181, https://mojira.dev/MC-249181).
- **22w12a это исправил** (MC-249181 — Fixed, в списке исправлений
  https://minecraft.wiki/w/Java_Edition_22w12a), порядок вернулся к
  прежнему. Как сейчас (Technical Minecraft Wiki):

  > «the update system was rewritten to not rely on nested update calls, but
  > rather to keep a list in the code of blockupdates yet to be processed,
  > which is then iterated through until the list is exhausted. The iteration
  > happens from most-to-least recently scheduled update, meaning that it
  > still employs a depth-first-search. This list has a max length of 1000000
  > updates» — https://techmcdocs.github.io/pages/BugsAndExploits/UpdateSuppression/

  То есть **LIFO-стек, DFS** — как и пишет Block update: «Block updates
  follow the principle of depth-first search (DFS)».
- Что происходит у предела («update skipping»):

  > «all the blocks scheduled to send updates do still send their updates once
  > the list reaches the cap, meaning that all the blocks affected … will send
  > out the rest of their block updates, but that these will not propagate
  > updates any further since they are processed after the cap was first
  > reached.» — там же

  и: «if the game attempts to schedule more than 1,000,000 block updates, any
  beyond the first 1,000,000 will be skipped … each successfully scheduled
  location will also update any blocks directly beside them»
  (https://mcdf.wiki.gg/wiki/Java_Edition:Update_Skipping).
  Итог для реализации: счётчик записей за **одну цепочку** (от первого
  обновления, пока стек не опустеет); после 1 000 000 новые записи
  **отбрасываются**, уже записанные дорабатывают, но ничего нового не
  порождают.
- Там же подмечено: при сломе блока игроком «one [update process] will be
  created for each side of the block» — у слома не одна цепочка, а по одной
  на каждую сторону. Для обычной игры это ненаблюдаемо; для предела —
  значит, счётчик сбрасывается между сторонами.
- Подавление обновлений в 26.1.2: через переполнение стека — нет (стека
  больше нет); через исключение с идентификаторами сущностей — ещё есть
  (закрыто только в 26.2 Pre-Release 1, https://minecraft.wiki/w/Tutorial:Update_suppression).
  Нам это неважно: воспроизводить не будем, но и мешать нечему.

**Открытый вопрос (для чёрного ящика):** при рекурсии блок, который в своём
отклике ставит **два** блока подряд, успевал разобрать всю цепочку от первого
до постановки второго. При стеке обновления от первого могут выполниться
только после того, как отклик закончится. Вики и отчёты об этом прямо не
говорят; MC-249181 говорит лишь, что рельсы вернулись к старому порядку.
Проверять сценарием, где отклик меняет два блока (дверь — обе половины,
поршень — голова и основание).

### 1.5. «Самообновление» — не обновление

> «When a block is placed, it checks and adapts itself to its surroundings …
> Strictly speaking, this isn't a block update» — https://minecraft.wiki/w/Block_update

Поставленная пыль сразу считает свою силу; поставленный диод/поршень/факел
смотрит на вход — это поведение самой постановки, а не NC самому себе.

### 1.6. Что у нас иначе (п. 1)

- **Один вид обновления.** `block_changed` (`src/redstone.rs`) шлёт всем
  одно и то же `neighbour_changed` → `react`; PP нет. Порядок общего NC
  (`NEIGHBOUR_ORDER` = з, в, н, вх, с, ю) верный; PP-порядка (з, в, с, ю,
  н, вх) нет нигде. Пыль в `react` на **любое** обновление пересчитывает и
  силу, и форму (`wanted_wire`), а в игре форма меняется только по PP.
  Повторитель проверяет замок тоже на любое обновление.
- **Порядок внутри смены.** У нас: `itself_changed` (NC самому себе — в игре
  его нет) → общее NC шести соседям → наблюдатели → отступление с поршнями
  (см. § 5.6) → `reach_of(new)` + `reach_of(old)`. В игре особые обновления
  −2/−1 (слом/постановка пыли, факела, диода, смена состояния факела)
  идут **до** общего NC, а «поздние» (3) — после NC **и** PP.
- **Общее NC при смене состояния** у нас глушится списком `keeps_quiet`
  (повторитель, сравнитель, лампа, дверь, люк, калитка, плита, наблюдатель)
  — это верно; но нет раздатчика, выбрасывателя, воронки, растений и прочих
  из списка вики (их у нас нет как деталей). Поставленное **командой**
  в игре шлёт NC шести соседям не как «общее» (0), а как «позднее» (3),
  то есть после PP; у нас — как любое другое изменение. Шлёт ли при этом
  блок свои особые −1/−2, вики не говорит — проверить.
- **Стек изменений и порядок.** `set_block` не рассылает обновления сразу:
  он кладёт место в `World::changed`, а `settle_from` снимает оттуда
  **последнее** (`pop_changed`, LIFO). Если отклик поставил несколько блоков
  (старт хода поршня `start_move`, конец хода `finish_move`, дверь), то
  первым разбирается **последний** поставленный, а вложенный `settle_from`
  внутри `block_changed` забирает из того же стека и **более старые** записи
  — посреди обхода соседей текущего блока. В игре каждая постановка
  рассылает свои обновления в момент постановки, по порядку постановок.
- **Предел глубины.** `MAX_DEPTH = 256`: остаток откладывается до
  следующего `settle` (иногда — до следующего такта). В игре предел —
  1 000 000 записей за цепочку, с отбрасыванием, а не откладыванием.
  Длинная линия BUD-рельсов или цепочка нестабильных блоков у нас разорвётся
  по времени, в игре — нет.

---

## 2. Пыль в 26.1.2 (обычный, не экспериментальный алгоритм)

### 2.1. Когда пыль считает силу и что получается

Пыль пересчитывает силу при постановке и при получении **NC**. Новая сила —
наибольшее из: сильного питания соседних проводящих блоков, соседних
источников и выходов диодов, силы соседней пыли минус 1 (включая пыль на
ступеньку выше/ниже по правилам разреза — redstone-core.md § 3.2).
Сравнивается с **текущими** значениями соседей, какими бы устаревшими они
ни были.

Отсюда «ступенчатое» гашение. Отчёт MC-81098 (открыт до сих пор):

> «redstone dust also DOES NOT directly de-power, as you'd expect. Instead, it
> continually loses 1 signal strength until it's "satisfied" … a line of 15
> redstone dust can easily amount to around 2,500 block updates in total.» —
> https://mojira.dev/MC-81098

Точные числа (SpaceWalkerRS, разбор к 24w33a, описание **ванильного**
поведения):

> «depowering a line of 15 wires will result in 64(!) signal strength changes
> altogether, where 15 should suffice.»
> «With each signal strength change, a wire updates all blocks within a range
> of 2 blocks of itself, 42 block updates in total … a total of 2688 block
> updates when the entire line depowers.»
> — https://gist.github.com/SpaceWalkerRS/25052d03ff956b988c50a75a08619545

**64 смены силы на линию из 15 и 42 NC на одну смену** — готовые числа
для проверки нашей реализации.

### 2.2. Кого пыль обновляет при смене силы

> «Its signal changes — 3 — Update the immediate neighbors (in order of west,
> east, down, up, north, south) of the datum points. Datum points include the
> wire and the immediate neighbors of the wire, in random order.» —
> https://minecraft.wiki/w/Block_update

То есть 7 опорных точек (сама пыль + 6 соседей) × 6 соседей = **42 NC**,
с повторами: сама пыль получает 6 обновлений (как соседка каждого из своих
соседей), каждый сосед — одно, блоки на расстоянии 2 — по одному или по два.
Совпадение с числом 42 у SpaceWalkerRS говорит о том, что **общего NC при
смене силы пыли нет** — только эти 42 (плюс PP, § 1.2). Проверить чёрным
ящиком.

«Random order» на деле — **зависимость от координат**, а не случайность:

> «The order of block updates around a wire is dependent on that wire's
> position in the world … circuits can behave differently in different
> locations» — SpaceWalkerRS, там же.

> «the powering order of redstone dust blocks in wires is dependent on the
> location of those redstone blocks in the world» — MC-11193 (Open),
> https://mojira.dev/MC-11193

Порядок детерминирован для данного места, но меняется от места к месту.
Правила, по которому он выбирается, вики словами не даёт. **Для 1:1 это
главный пробел:** вывести порядок опорных точек можно только замерами на
официальном сервере (чёрный ящик: ставить одну и ту же схему в разных
координатах и записывать, какая из опорных точек обновилась первой, — см.
§ 8).

### 2.3. Постановка и слом пыли

Разобрано в redstone-core.md § 7.3 (таблица вики). Коротко:
- **слом/замена (−2):** соседи каждого из соседей пыли (соседи пыли в
  порядке н, вх, с, ю, з, в), затем для соседней пыли (с, в, ю, з) — её
  соседи и соседи её соседей, затем то же для пыли над твёрдыми соседями,
  затем для пыли под нетвёрдыми соседями;
- **постановка (−1):** соседи блока **над** пылью, соседи блока **под**
  пылью, затем те же три группы соседней пыли.

Сюда же — переключение точка⇄крестик (3): «Update the immediate neighbors
(without the wire itself …) of the solid block next to the wire (in order of
north, east, south, west)».

### 2.4. Переразворот без смены силы — тишина

> «When redstone wire is reconfigured after placement, it does not update other
> redstone components around it of the change unless that reconfiguration also
> includes a change in power level» — https://minecraft.wiki/w/Redstone_Dust

Форма меняется по PP (§ 1.2), и NC-рассылки при этом нет (есть только PP
соседям — и тоже без наблюдателя по диагонали).

### 2.5. Что наблюдаемо: два поршня от одной линии

Поскольку линия гаснет в глубину, а порядок опорных точек у каждой пыли
зависит от её координат, **какой из двух поршней, запитанных одной линией,
получит обновление первым — зависит от места постройки**. Пример из
MC-11193: поршень запитан сверху (через блок) и по квазисвязности (через
блок по диагонали); если сначала гаснет пыль над соседним блоком, поршень
видит, что его ещё питает диагональ, и **залипает** выдвинутым, потому
что гаснущая диагональ до него обновлением не дотягивается. В другом месте
мира та же схема работает. Это поведение 26.1.2, и его придётся повторить.

### 2.6. Для справки: экспериментальный алгоритм (НЕ 26.1.2 по умолчанию)

Только при включённом «Redstone Experiments» (https://minecraft.wiki/w/Redstone_Experiments):
сила всей линии выставляется до рассылки; линия включается/гаснет от
источника; при равном расстоянии — «wires to the left will cause block
updates first»; вокруг пыли порядок «back, front, left, right, down, up»;
пыль обновляет только то, что может от неё запитаться, и игнорирует
обновления от другой пыли. В обычном мире 26.1.2 ничего этого нет.

### 2.7. Что у нас иначе (п. 2)

- `wire_power` считает силу так же (по текущим соседям) — ступенчатое
  гашение у нас, вероятно, тоже есть, но **число смен** зависит от порядка
  обхода и не сверялось. Нужна проверка «линия из 15 → 64 смены».
- Рассылка при смене силы — `reach_of` для `Kind::Wire`: первый уровень в
  фиксированном порядке н, вх, с, ю, з, в (это порядок **факела**, у пыли
  на вики его нет — там опорные точки по координатам), плюс общее NC из
  `block_changed`, плюс `itself_changed`. Сама пыль при этом пропускается
  (`far != pos`): у нас она получает 1 обновление вместо 6. Итого у нас
  1 + 6 + 30 = 37 откликов вместо 42, в другом порядке.
- Слом/постановка пыли — тот же диамант радиуса 2, без цепочки соседней
  пыли (audit-updates.md § 3), и после общего NC, а не до.
- Форма пересчитывается на любое обновление (`react` → `wanted_wire`), PP-
  исключения «блок над и под тем, куда смотрит пыль, кроме наблюдателя»
  нет.
- Точка⇄крестик (`use_block`) шлёт обычное общее NC вместо «соседи твёрдого
  блока рядом, с, в, ю, з».
- Зависимости от координат нет вовсе: схема ведёт себя одинаково везде, а
  в оригинале — нет.

---

## 3. Фазы игрового такта

### 3.1. Порядок (Java, для каждого измерения)

Дословно по https://minecraft.wiki/w/Tick § Game process:

1. Update world border bounds
2. Advance weather cycle
3. Update daylight cycle time
4. Player sleeping logic
5. Run scheduled commands
6. **Execute Scheduled block ticks**
7. **Execute Scheduled fluid ticks**
8. Raid logic
9. Update chunks load level
10. **For all chunks in random order:** Spawn mobs / Tick ice and snow /
    **Random ticks**
11. Send block changes to players
12. Update points of interest
13. Unloads chunks
14. **Execute block events**
15. Tick dragon fight
16. For all non-passenger entities: despawn check, tick entity, tick passengers
17. **Tick block entities**
18. Handle game events

После всех измерений: «Send queued packets and handle incoming packets for
each player». Технически пакеты игрока выполняются между тактами; наш
чёрный ящик снял, что для игрока это выглядит как «в начале следующего
такта» (память: tick-and-network-order). Technical Minecraft Wiki прямо
говорит, что граница условна: «If you consider that the player input phase
happens at the beginning of a tick, then tiles tick scheduled by a player
input lose 1 gametick of delay» (https://techmcdocs.github.io/pages/GameTick/TileTicks/).

Где что происходит (важное для редстоуна):
- повторители, сравнители, факелы, наблюдатели, кнопки, лампы (гашение),
  падение песка — **запланированные блочные такты** (6);
- вода и лава — **жидкостные** (7);
- гаснущая редстоуновая руда, рост растений, окисление меди — **случайные
  такты** (10);
- старт хода поршня, звук нотного блока, открытие сундука/шалкера/
  эндер-сундука — **блочные события** (14)
  (перечень: https://techmcdocs.github.io/pages/GameTick/);
- нажимные плиты и растяжка (их трогают сущности), приземление падающих
  блоков — **сущности** (16);
- **конец хода поршня** (moving_piston превращается в блок), воронки —
  **блок-сущности** (17). Ход: «progress … Starts at 0.0, and increments by
  0.5 each tick, including the tick on which moving the piston is created. If
  the value is 1.0 or higher at the start of a tick of the block entity …
  the block transforms» — https://minecraft.wiki/w/Piston/Technical_components.

### 3.2. Запланированные такты — точные правила

- **Ключ — место и вид блока.** В записи такта хранится блок: «i: The ID of
  the block; used to activate the correct block update procedure»
  (https://minecraft.wiki/w/Chunk_format § Tile tick format). Такт,
  запланированный повторителем, не достанется факелу, поставленному на то
  же место (вывод из формата; проверить).
- **Один ожидающий такт на пару.** Для повторителя вики говорит прямо:
  «Only one scheduled tick can be scheduled at a time; any attempt to schedule
  a tick while one is already scheduled is ignored completely»
  (https://minecraft.wiki/w/Redstone_Repeater). Что это общее свойство
  очереди, следует из истории: «[1.13, 18w10c] Water can now schedule a tick
  before a previous scheduled tick has finished during the scheduled tick
  stage in which the previous tick is executed» (Tick) — то есть обычно
  второй такт при ожидающем не ставится, а исполняемый такт уже не считается
  ожидающим и может перепланировать себя.
- **Тонкость «такт на этот же такт, ещё не исполненный»** (MC-189954, Won't
  Fix, поведение осталось): у **наблюдателя** такт, который должен сработать
  в текущем такте, но ещё не сработал, не считается ожидающим — обновление
  в этот момент ставит второй такт; у **повторителя и факела** — считается,
  и обновление игнорируется: «Observers react to updates if they already have
  a scheduled tick at the current world time but haven't yet been ticked …
  the repeater and redstone torch for example … are not affected by this bug»
  (https://mojira.dev/MC-189954). От этого зависят 4-тактовые часы на
  наблюдателях.
- **Сортировка:** «Block ticks are executed first based on priority, and then
  based on scheduling order. A lower value for priority results in earlier
  execution» — Tick. Приоритеты: повторитель в зад/бок диода −3,
  выключающийся повторитель −2, прочий повторитель −1, сравнитель в зад/бок
  диода −1, всё остальное 0. Technical Minecraft Wiki уточняет две вещи:
  «'FACING ANOTHER DIODE' REALLY MEANS 'FACING ANOTHER DIODE THAT IS NOT
  FACING IT'» и «UNPOWERED REPEATER IS NOT RECEIVING POWER WHEN TICKED —
  PRIORITY: −2» (https://techmcdocs.github.io/pages/GameTick/TileTicks/).
- **Жидкости — отдельный список после всех блочных**, без приоритетов, по
  порядку планирования.
- **Предел:** «the maximum number of scheduled ticks per game tick is 65,536»
  (Tick); «The tile ticks that weren't executed in the tick they were supposed
  to are delayed to the next tick» (techmcdocs). Отдельный ли предел у
  блочных и жидкостных — вики не уточняет (проверить; по формату очередей
  вероятно — у каждой свой).
- **Где исполняются:** только в чанках, где идёт симуляция: «block entities
  … and scheduled tick and chunk tick (without mob spawning) [are] processed
  only if the chunk is in the aforementioned square region along with a
  one-chunk-thick square frame … side length l = min(2s+3, 65)»
  (https://minecraft.wiki/w/Chunk), где s — `simulation-distance`. В чанке
  с уровнем 33 такт можно запланировать, но он ждёт.
- После каждого запланированного такта его цепочка обновлений разбирается
  до конца (DFS, § 1.4), и только потом — следующий такт.

### 3.3. Блочные события

- Очередь — **в порядке добавления**, повторы отбрасываются. Technical
  Minecraft Wiki: «When the block event phase starts, the game loops through
  the block event list, and executes block events in the order of creation
  (first in, first executed)» (цитата по piston-timing.md § 8) и «Piston
  block events can activate and schedule another block event that is added
  at the end … This allow[s] chaining multiple blockevents in the same tick»
  (https://techmcdocs.github.io/pages/GameTick/). То есть **события,
  добавленные во время фазы, исполняются в этой же фазе**, фаза крутится,
  пока очередь не опустеет. Это прямо следует и из вики: «If the piston is
  powered and updated in the … block event phase, the piston activates in
  this game tick's block event phase» (https://minecraft.wiki/w/Piston § Start delay).
- **Повтор определяется целиком:** место, блок, вид события, параметр.
  «If it can it creates a block event … if the block event doesn't already
  exist» (piston-timing.md § 8). Поэтому у поршня в одной очереди могут
  стоять **и** «выдвинуться», **и** «задвинуться» — это и есть 0-тактовый
  импульс: «The piston gets updated twice in the same tick and schedules 2
  block events» (там же, § 6.2).
- **Перед исполнением событие проверяется:** на месте должен стоять тот же
  блок; поршень заново смотрит, можно ли сделать то, что просили («If it is
  trying to push, it checks that it is powered and not blocked. If it is
  trying to pull, it checks if it is unpowered»).
- Нотный блок играет **блочным событием** (Technical Minecraft Wiki: «Note
  Block: Play the note, display the particles and make the sound»). Это,
  возможно, объясняет наш замер для `settle_once` (нота звучит позже
  повтора рычага): не частичный разбор обновлений, а фаза событий. Стоит
  перепроверить.

### 3.4. Что у нас иначе (п. 3)

`src/tick.rs::run`, по измерению:
пакеты → `advance` (блочные и жидкостные в одной куче) с `settle` после
каждого → `take_block_events` + `block_event` → `step_plates` → `settle` →
выпадение/падение → предметы → приземление.

- **Ключ такта — место + `TickKind`**, а не место + блок
  (`World::schedule_once`, `waiting`). Такт повторителя сработает для
  любого блока, стоящего там потом (`redstone::ticked` смотрит на текущий
  блок).
- **«Ещё не исполненный такт этого же такта».** `World::advance` вынимает
  **все** созревшие такты и сразу удаляет их из `waiting` до исполнения.
  Поэтому у нас повторитель и факел ведут себя как наблюдатель из MC-189954
  (могут получить второй такт), а должны игнорировать.
- **Жидкости:** `World::schedule` при уже ожидающем такте берёт **более
  ранний** срок (перепланирует). В игре второй такт просто не ставится.
  Жидкости у нас в общей куче с `FLUID_PRIORITY = 1` — порядок внутри такта
  совпадает, предел 65 536 общий на оба вида.
- **Приоритеты:** `faces_diode` исправлен («не лицом к нам») и совпадает
  с уточнением techmcdocs. Случай «выключенный повторитель, у которого в
  момент такта входа уже нет, — −2» у нас покрыт перепланированием из
  `ticked` с `PRIORITY_DEPOWERING`; но в `ticked` это делается только если
  повторитель **включился**, а по techmcdocs речь о такте, в котором он
  так и остался выключенным. Сверить на сценарии «импульс короче задержки».
- **Симуляционная дистанция не соблюдается:** такты и события идут во всех
  загруженных чанках (`advance` проверяет только «чанк в памяти»), а у нас
  загружено `view_distance + KEEP_EXTRA`. В игре за квадратом 2s+3 такты
  ждут.
- **Блочные события:** `queue_block_event` отбрасывает повтор **по одному
  месту** (без вида события), а `take_block_events` забирает снимок: всё,
  что заведено во время фазы, ждёт **следующего** такта. В игре — в том же.
  Сам `block_event` выводит действие из питания, а не из вида события;
  события «проверить блок на месте» нет (оно подменено проверкой
  `Kind::Piston`).
- **Конец хода поршня** — запланированный такт `PISTON_MOVE = 2`
  (`schedule_block` в `block_event`, `finish_move` в `ticked`): блоки
  встают **в фазе запланированных тактов**, раньше блочных событий.
  В игре — в фазе блок-сущностей, после событий и сущностей. Следствие:
  поршень, запитанный приехавшим блоком редстоуна, у нас стартует в том же
  такте (задержка 0), в игре — в следующем (задержка 1): «in a chain of
  pistons each pushing a redstone block to activate the next, successive
  activations will happen 3 ticks apart» (Piston). У нас — 2.
- **Плиты** (`step_plates`) после событий — совпадает с фазой сущностей.
  Воронок и прочих блок-сущностей нет.

---

## 4. Случайные такты

- **Сколько:** «Every chunk tick, some blocks are chosen at random from each
  subchunk in the chunk … the number of blocks chosen from each subchunk is
  specified by `gamerule random_tick_speed` (defaults to 3), and one block can
  be chosen multiple times in one chunk tick» (Tick). «Subchunks that do not
  contain at least one block that can react to random ticks are skipped. All
  blocks are a possible target of a random tick, including air.» Подчанк —
  16×16×16, от самого нижнего Y. Среднее время до такта у блока — 4096/3 ≈
  1365 тактов (68,27 с).
- **Где:** «chunks with loading type of entity ticking … are ticked on every
  game tick. This primarily includes chunks in the simulation distance, but
  chunks loaded through other methods are also random-ticked» (Tick); с 1.21.5
  «All fully-loaded chunks now receive random ticks, regardless of if a player
  is within an 8-chunk distance or not» (Tick, History). Страница Chunk даёт
  для чанкового такта квадрат 2s+3, Tick — «entity ticking» (2s+1).
  Расхождение источников — замерить.
- **Когда:** внутри фазы чанков, «For all chunks in random order» — чанки
  перемешиваются каждый такт; до блочных событий, после жидкостей.
- **Что важно для редстоуна:** «Lit redstone ore turns off» (случайный такт
  гасит руду; смена состояния руды — не из списка исключений, значит шлёт
  NC и PP, её ловят BUD и наблюдатели); окисление медных блоков (в т.ч.
  медной лампы, двери, люка — смена блока шлёт обновления); рост растений
  (кактус, тростник, бамбук, ламинария — фермы на наблюдателях); таяние
  льда (BUD). Огонь в Java — не случайный, а запланированный такт (30–40).
  Датчик дневного света — не случайный такт (не относится сюда).
- Там же в чанковом такте — погодные проверки (замерзание воды, снег;
  шанс с 1.20.2 зависит от `random_tick_speed`).

**Что у нас иначе:** случайных тактов нет совсем (поиск по `random_tick`
пуст; `server_properties.rs` знает `simulation_distance`, но такт его не
использует). Руда не гаснет, растения не растут, медь не окисляется.

---

## 5. Квазисвязность, BUD, 0-тактовые импульсы, поршни

### 5.1. Квазисвязность

> «a property of dispensers, droppers, and pistons that allows them to be
> activated by anything that would activate the space above them, no matter
> what is actually in that space» — https://minecraft.wiki/w/Tutorial:Quasi-connectivity

Работает и когда блок над поршнем непроводящий (воздух). Крафтер — нет
(MC-265943, WAI). Со стороны головы поршень не питается, но место над
поршнем питается с любой стороны.

### 5.2. Когда поршень проверяет питание

1. **При постановке** (самоподстройка, § 1.5).
2. **При NC-обновлении** — решает, что делать, и ставит блочное событие
   (с учётом QC).
3. **При исполнении события** — проверяет ещё раз (§ 3.3).

Больше никогда. После хода отдельной проверки нет — поэтому QC-поршень,
у которого источник наискось погас, **остаётся выдвинутым** до постороннего
обновления: «components that are supposed to power pistons with QC may not
give out adequate block updates for the piston to realize that it is powered,
until it receives a block update from elsewhere»
(https://techmcdocs.github.io/pages/GameMechanics/QuasiConnectivity/).
Мгновенная QC бывает только от деталей, чьи обновления дотягиваются на 2
блока: пыль, факел, повторитель, сравнитель; и от рычага/кнопки/плиты/
детекторной рельсы/сундука-ловушки, прикреплённых к блоку под местом над
поршнем (вики, § Immediate QC activation).

### 5.3. Задержка старта 0 или 1

> «If the piston is powered and updated in the scheduled tick phase, random
> tick phase or block event phase, the piston activates in this game tick's
> block event phase … If the piston is powered and updated during the entity
> phase or the block entity phase, or by player actions, the piston activates
> in the next game tick's block event phase» — https://minecraft.wiki/w/Piston

Сам ход — 2 такта; блоки встают в фазе блок-сущностей такта +2.

### 5.4. Короткие импульсы и прерывание

- **Выдвигание можно прервать**: «a sticky piston finishes extending early
  and starts retracting if it loses power before the extension process is
  over (assuming it can detect the signal change). If the sticky piston was
  pushing one or more blocks during the extension, the first block ends up in
  its final position immediately and all the other blocks continue moving»
  (https://minecraft.wiki/w/Sticky_Piston § Block dropping, MC-5726 — WAI).
- Длина импульса: «If a sticky piston is powered by a 2-tick pulse, it starts
  extending; after two ticks, the piston drops its block and starts
  retracting. If … a 1-tick pulse, the same happens after only one tick.
  … a 0-tick pulse … causes sticky pistons to instantly drop their block and
  start retracting. Regular pistons start retracting instantly when powered by
  a 0-tick pulse but do not instantly teleport blocks»
  (https://minecraft.wiki/w/Tutorial:Zero-ticking). То есть липкий поршень
  **бросает** блок на импульсе короче полного хода (0, 1 или 2 такта —
  если снятие питания он «увидел» до конца хода); обычный поршень с 1.13.1
  блоков не бросает («Pistons don't drop blocks anymore», 18w30b).
  «Block dropping only works for blocks that were being pushed … if it wasn't
  pushing any, the sticky piston attempts to pull blocks as usual».
- «Микротакты»: 0-тактовый импульс внутри фазы событий длится столько
  событий, сколько успеет исполниться до его конца; «A 1-microtick pulse
  does not affect pistons at all … A 2-microtick pulse is enough to power
  pistons that were updated by the rising-edge of the pulse» (techmcdocs,
  цитата по piston-timing.md § 6.3). Работает только потому, что события,
  заведённые в фазе событий, исполняются в той же фазе (§ 3.3).
- **Задвигание не прерывается**: при задвигании основание само становится
  moving_piston («It replaces itself with a moving piston» —
  https://techmcdocs.github.io/pages/Blocks/Piston/), обновлять некого.
  Прямой цитаты «нельзя прервать» нет — вывод, как и в piston-timing.md § 6.3.
- 0-тактовые генераторы: на приоритетах тактов («comparators are processed
  in a tick later than repeaters»), на BUD-поршнях, цепочки 0-тактов — на
  том, что такты идут раньше событий (Tutorial:Zero-ticking).

### 5.5. BUD

Поршень — BUD, когда он «должен» сменить состояние, но обновления не
получил: QC-источник наискось или дальше двух блоков; пыль переразвернулась
без смены силы; поршень не смог выдвинуться (упёрся) и ждёт обновления;
поршень толкнул то, что его питало. Всё это выходит само из § 1–5.3, если
не добавлять поршню «лишних» проверок.

### 5.6. Что у нас иначе (п. 5)

- **Отступление с QC ещё в коде.** `block_changed` в цикле «Поршень слушает
  не только себя, но и клетку прямо над собой» будит каждый поршень под
  каждым соседом изменившегося блока. Это сознательное отступление от
  20.09; по решению пользователя от 28.09 («как в оригинале») его надо
  **убрать** — иначе BUD и залипание на QC не работают.
- Раздатчиков и выбрасывателей (вторые носители QC) нет.
- **Прерывания нет:** `block_event` при `redstone.moving.contains_key(&pos)`
  выходит («начатый ход не прерывается»). Нет block dropping, нет
  досрочного задвигания обычного поршня, 0-тактовые импульсы не работают.
- Событие перед исполнением проверяется (`piston_powered == extended`
  → выход) — это совпадает с «checks again»; но вида события нет, и два
  события на одно место не встают (§ 3.4).
- Ход 2 такта — есть; фаза конца хода не та (§ 3.4); `finish_move` в конце
  зовёт `neighbour_changed` самому поршню — в игре такой «само-проверки»
  нет, поршень узнаёт о переменах только от обновлений постановки головы/
  блоков рядом. Сверить: не маскирует ли это BUD «поршень толкнул то, что
  его питало».
- Порядок обновлений при старте хода: в игре — «The game first creates moving
  blocks (B36) in front of each block to be moved … sends state updates at
  the position of each of the new blocks. Next, it deletes all the old blocks
  … Finally, it sends redstone block updates around all removed blocks and
  the moving piston head» (techmcdocs Piston; вики Block update § Special
  blocks: PP при постановке каждого moving_piston и снятии обычных блоков,
  NC — только когда все moving_piston поставлены; липкий при втягивании
  NC вокруг головы не шлёт). У нас `start_move` ставит всё через
  `set_hidden`/`set_block_quietly` → одинаковые записи в `changed` →
  общий разбор LIFO (§ 1.6). Порядок другой, и вокруг головы липкого NC
  у нас есть.

---

## 6. Обновление сравнителя

> «The comparator update … is a subtype of the neighbor update that is only
> sent to comparators … When a block that can be detected by comparators
> changes, it sends updates to surrounding comparators (including comparators
> separated by a solid block), in this order: north, east, south, west.» —
> https://minecraft.wiki/w/Block_update

- **Кто шлёт:** блоки, которые сравнитель умеет читать, когда меняется то,
  что он читает: ёмкости (сундук, бочка, воронка, раздатчик, выбрасыватель,
  шалкер…), компостер, котёл, командный блок, детекторная рельса с
  вагонеткой, торт, улей, резной книжный шкаф, крафтер, кафедра,
  музыкальный ящик, якорь возрождения, рамка портала Края, медная лампа,
  сердце скрипуна, полка; из сущностей — рамки предметов (список
  «что читает сравнитель» — redstone-core.md § 6.5). Techmcdocs добавляет
  «A hopper trying to pull items» как источник
  (https://techmcdocs.github.io/pages/GameMechanics/BlockUpdates/).
- **Кому:** по каждой горизонтали (с, в, ю, з) — соседний сравнитель, а если
  сосед — проводящий блок, то сравнитель за ним (на 2 блока). «Comparator
  updates can be detected only with a comparator, and only if the comparator
  is horizontally adjacent or 1 block horizontally away with a conductive
  block between» (techmcdocs, там же).
- **Когда в рассылке:** общие обновления сравнителей — приоритет 1, между
  общим NC и PP (§ 1.3).
- Сравнитель, получив его, пересчитывает выход (с обычной задержкой 2 такта).

**Что у нас иначе:** механизма нет. `comparator_output` читает только
редстоун-вход (`diode_input`), чтения блоков нет вовсе — даже медной лампы,
которая у нас есть (`Kind::Bulb`). Когда появятся ёмкости — нужен третий
вид рассылки.

---

## 7. Сводка расхождений по важности

| # | Что | Где у нас | Важность |
|---|---|---|---|
| 1 | Нет прерывания хода/block dropping; события фазы событий ждут следующего такта; повтор события — по месту без вида | `block_event`, `World::queue_block_event`, `take_block_events` | высокая (0-такты, все технические схемы) |
| 2 | Конец хода поршня в фазе запланированных тактов вместо блок-сущностей (цепочка поршней 2 такта вместо 3) | `block_event` → `schedule_block(PISTON_MOVE)`, `ticked` → `finish_move` | высокая |
| 3 | QC-отступление (поршень слушает клетку над собой) | цикл в `block_changed` | высокая (решено убрать) |
| 4 | Нет разделения NC/PP; пыль и повторитель реагируют на всё; нет PP-порядка | `block_changed`, `react` | высокая |
| 5 | Порядок внутри смены: особые −1/−2 после общего NC, лишний NC себе | `block_changed`, `itself_changed` | средняя-высокая |
| 6 | Многоблочные изменения разбираются LIFO из общего стека `changed`, вложенный разбор хватает старые записи | `set_block` → `changed`, `settle_from`, `pop_changed` | высокая для поршней и дверей |
| 7 | Пыль: фиксированный порядок вместо координатного; 37 откликов вместо 42; нет цепочки при сломе/постановке | `reach_of(Kind::Wire)` | высокая для 1:1, но нужен замер (§ 8) |
| 8 | Ключ такта — место без вида блока; ещё не исполненные такты этого такта не считаются ожидающими | `World::schedule_once`, `advance` | средняя |
| 9 | Жидкость перепланируется на более ранний срок | `World::schedule` | средняя |
| 10 | Такты и события во всех загруженных чанках, не в 2s+3 | `advance` | средняя |
| 11 | Предел цепочки 256 с откладыванием вместо 1 000 000 с отбрасыванием | `MAX_DEPTH`, `settle_from` | низкая-средняя |
| 12 | Нет случайных тактов | — | средняя (вне редстоуна — высокая) |
| 13 | Нет обновлений сравнителя и чтения блоков | `comparator_output` | низкая сейчас |

---

## 8. Что замерить чёрным ящиком (вики этого не даёт)

1. **Порядок опорных точек пыли** (§ 2.2) как функция координат. Схема:
   одна пыль, к которой на расстоянии 2 по разным направлениям стоят
   BUD-поршни (или лампы через проводящие блоки), питание снимается; по
   порядку Block Action поршней в одном такте виден порядок опорных точек.
   Повторить в десятках координат, включая отрицательные, искать период.
2. **64 смены силы** на гашение линии из 15 — считать изменения блоков пыли
   за один такт у оригинала и у нас.
3. **Общее NC при смене силы пыли** — есть или нет (42 против 48).
4. **Два блока в одном отклике** при стековом обработчике (§ 1.4, открытый
   вопрос): дверь из двух половин рядом с BUD-поршнями.
5. **Цепочка поршень → блок редстоуна → поршень**: период 3 такта (вики)
   против наших 2.
6. **MC-189954**: 4-тактовые часы на наблюдателях и повторитель, получивший
   обновление в такт своего срабатывания.
7. **Случайные такты**: в каком квадрате вокруг игрока гаснет редстоуновая
   руда (2s+1 или 2s+3).
8. **Нотный блок**: звук — в фазе событий (объясняет ли это `settle_once`).
9. **Предел тактов**: общий или отдельный для блочных и жидкостных.

---

## Источники

- https://minecraft.wiki/w/Block_update
- https://minecraft.wiki/w/Tick
- https://minecraft.wiki/w/Random_Tick
- https://minecraft.wiki/w/Chunk
- https://minecraft.wiki/w/Chunk_format
- https://minecraft.wiki/w/Redstone_Dust
- https://minecraft.wiki/w/Redstone_Repeater
- https://minecraft.wiki/w/Observer
- https://minecraft.wiki/w/Piston
- https://minecraft.wiki/w/Sticky_Piston
- https://minecraft.wiki/w/Piston/Technical_components
- https://minecraft.wiki/w/Redstone_Ore
- https://minecraft.wiki/w/Tutorial:Quasi-connectivity
- https://minecraft.wiki/w/Tutorial:Zero-ticking
- https://minecraft.wiki/w/Tutorial:Update_suppression
- https://minecraft.wiki/w/Redstone_Experiments
- https://minecraft.wiki/w/Server.properties
- https://minecraft.wiki/w/Java_Edition_22w11a, https://minecraft.wiki/w/Java_Edition_22w12a, https://minecraft.wiki/w/Java_Edition_1.19, https://minecraft.wiki/w/Java_Edition_24w33a, https://minecraft.wiki/w/Java_Edition_1.21.2
- https://techmcdocs.github.io/pages/BugsAndExploits/UpdateSuppression/
- https://techmcdocs.github.io/pages/GameTick/ и /GameTick/TileTicks/
- https://techmcdocs.github.io/pages/GameMechanics/BlockUpdates/ и /QuasiConnectivity/
- https://techmcdocs.github.io/pages/Blocks/Piston/
- https://mcdf.wiki.gg/wiki/Java_Edition:Update_Suppression, https://mcdf.wiki.gg/wiki/Java_Edition:Update_Skipping
- Mojira (тексты отчётов через mojira.dev): MC-11193, MC-81098, MC-189954,
  MC-249082, MC-249181, MC-5726, MC-108
- https://gist.github.com/SpaceWalkerRS/25052d03ff956b988c50a75a08619545 (только прозаическое описание поведения)

*Кодом Minecraft (в том числе декомпилированным), jar-файлами, кодом
серверных ядер, прокси, протокольных библиотек и модов не пользовался.
Фрагменты кода и имена методов, попадавшиеся на страницах Technical
Minecraft Wiki, вики (таблица «Priorities») и в разделе «Code analysis»
отчёта MC-189954, пропущены и в выводах не использованы; из MC-189954 взято
только описание наблюдаемого поведения.*
