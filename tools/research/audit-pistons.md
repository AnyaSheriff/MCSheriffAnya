# Аудит поршней: наш код против minecraft.wiki

Сверка `src/redstone.rs` (вся логика поршней), `src/world/mod.rs` (планировщик тактов)
и `src/tick.rs` со страницами вики `Piston`, `Sticky Piston`, `Piston/Table`,
`Sticky Piston/Table`, `Piston/Technical components`, `Tutorial:Quasi-connectivity`,
`Slime Block`, `Honey Block`, `Block update`, `Tick`.

Код не менялся. Предложений по правкам здесь нет — только факты.

Попутно сверена наша прошлая выписка `tools/research/redstone-pistons.md` (см. раздел
«Ошибки и неточности в прошлой выписке»).

---

## 1. Хода поршня нет: всё происходит за один такт, `moving_piston` отсутствует

**Как в игре**

> «When powered, the piston's wooden surface (the "head") tries to start extending after
> a start delay. The extension takes 2 game ticks (0.1 seconds) to finish.»
> — https://minecraft.wiki/w/Piston (§ Usage → Redstone component)

> «The moving piston (JE) or moving block (BE), also known as block 36 … is an
> unobtainable technical block that holds a block entity … which contains the block the
> piston is currently moving. … At the end of the piston stroke, the moving piston blocks
> are replaced with either the carried block, the piston head (during extensions), or the
> piston itself (during retractions).»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Moving piston)

**Как у нас**

`extend_piston` (`src/redstone.rs:1182-1217`) и `retract_piston` (`src/redstone.rs:1249-1279`)
выполняют весь ход мгновенно, внутри одного вызова `ticked`: `move_group` переставляет
блоки (`src/redstone.rs:1224-1246`), сразу ставится `piston_head`, сразу ставится
`extended=true`. Блок `moving_piston` в коде встречается только один раз — в списке
`IMMOVABLE` (`src/redstone.rs:293`). Блок-сущности с полями `blockState`/`extending`/
`facing`/`progress`/`source` нигде не заводятся; `progress` в проекте не существует.

**Чем это заметно в игре**

Клиент не покажет анимацию — блок телепортируется. Любая схема, рассчитанная на то,
что клетка занята `moving_piston` два такта, работает иначе: летающие машины на
слизи, TNT-дюперы, «нулевые» такты, любые цепочки поршней. По вики цепочка поршней,
где каждый толкает блок редстоуна к следующему, срабатывает с шагом 3 такта; у нас
шаг будет другим.

**Как проверить**

Поставить в ряд четыре поршня, смотрящих вверх, перед каждым блок редстоуна,
активирующий следующий; замерить по логу такты между срабатываниями. Ожидается 3,
у нас будет меньше.

**Важность: высокая** (это фундамент всей тайминг-модели).

---

## 2. Задержка старта всегда 1 такт, фазы такта не разведены

**Как в игре**

> «0 or 1 game tick | 0 ticks if powered during the scheduled tick, random tick or block
> event phase; 1 tick if powered during the entity or block entity phase, or during player
> input handling.»
> — https://minecraft.wiki/w/Piston (§ Start delay, таблица)

> «If the piston is powered and updated in the scheduled tick phase, random tick phase or
> block event phase, the piston activates in this game tick's block event phase, which
> means the start delay in this case is 0.»
> — там же

**Как у нас**

`const PISTON_DELAY: u64 = 1;` (`src/redstone.rs:47`), и комментарий в коде это признаёт:
«В игре это 0 или 1 такт — смотря в какой части такта пришёл сигнал; у нас всегда 1,
потому что фаз внутри такта мы не разводим». Планирование — `src/redstone.rs:1033`.
Фазы «block events» в `src/tick.rs` нет вовсе: есть только очередь запланированных
тактов (`World::advance`, `src/world/mod.rs:655-680`), блочная и жидкостная.

**Чем это заметно в игре**

Поршень, запитанный от повторителя (то есть из фазы запланированных тактов), в игре
трогается в тот же такт, у нас — на такт позже. Все схемы, где поршень и повторитель
считают такты друг от друга, поедут.

**Как проверить**

Рычаг → повторитель на задержке 1 → поршень. Сравнить номер такта, на котором
`extended` становится `true`, с номером такта срабатывания повторителя. В игре тот же
такт, у нас следующий.

**Важность: высокая**

---

## 3. Поршень занимает общую очередь запланированных тактов, а «одно место — один такт» его глушит

**Как в игре**

Поршень в Java с 1.3.1 планирует ход, но именно как **block event**, а не как
scheduled tick:

> «Pistons now schedule extensions and retractions rather than executing them as soon as
> they realize they are (un)powered.» — https://minecraft.wiki/w/Piston (история, 12w26a)

> «the piston activates in this game tick's block event phase»
> — https://minecraft.wiki/w/Piston (§ Start delay)

Фаза block events идёт отдельно от фазы запланированных тактов блоков
(https://minecraft.wiki/w/Tick, § Game process).

**Как у нас**

`world.schedule_once(...)` (`src/redstone.rs:1033`) кладёт поршень в ту же очередь
`TickKind::Block`, что и повторители с факелами. А `schedule_once`
(`src/world/mod.rs:605-612`) молча отбрасывает просьбу, если это место уже чего-то ждёт:

```rust
if self.waiting.contains_key(&((x, y, z), TickKind::Block)) {
    return false;
}
```

**Чем это заметно в игре**

Поршень, которому дважды за короткое время меняют питание, может пропустить второе
изменение. Кроме того, поршень исполняется вперемешку с повторителями по приоритетам
диодов, тогда как в игре все диоды отрабатывают строго раньше всех поршней.

**Как проверить**

Быстрое переключение рычага (вкл/выкл на соседних тактах) рядом с поршнем: посмотреть,
не «залип» ли он в одном состоянии.

**Важность: средняя**

---

## 4. Нет прерывания выдвигания и нет «block dropping» у липкого поршня

**Как в игре**

> «In Java Edition, a piston that is de-powered during the extension process aborts its
> extension and starts to retract (assuming it can detect the signal change). Any block
> which was being pushed by that piston continues to move as if the piston was still
> pushing it.» — https://minecraft.wiki/w/Piston (§ Redstone component)

> «A sticky piston finishes extending early and starts retracting if it loses power before
> the extension process is over (assuming it can detect the signal change). If the sticky
> piston was pushing one or more blocks during the extension, the first block ends up in
> its final position immediately and all the other blocks continue moving as if they were
> still being pushed.»
> — https://minecraft.wiki/w/Sticky_Piston (§ Block dropping)

> «Block dropping only works for blocks that were being pushed by the sticky piston before
> its extension was canceled—if it wasn't pushing any, the sticky piston attempts to pull
> blocks as usual once it starts retracting.» — там же

**Как у нас**

Прерывать нечего: хода не существует (см. §1). В `ticked` (`src/redstone.rs:1409-1422`)
поршень за один вызов либо полностью выдвигается, либо полностью задвигается. Слова
«dropping», «spitting», «abort» в коде отсутствуют.

**Чем это заметно в игре**

Импульс короче 2 тактов (наблюдатель, «нулевой» импульс) у нас даст обычное
выдвигание-задвигание с возвратом блока; в игре липкий поршень «выплюнет» блок и не
заберёт его обратно. Ломаются все схемы на block dropping.

**Как проверить**

Липкий поршень с блоком перед головой, вход — импульс в 1 такт. В игре блок остаётся
на новом месте, поршень пуст. У нас блок вернётся.

**Важность: высокая** (для липких поршней)

---

## 5. Безголовый поршень: мы уничтожаем основание, в игре оно остаётся

**Как в игре**

> «Headless pistons do not break upon receiving a block update anymore.»
> — https://minecraft.wiki/w/Piston (история, Java Edition Beta 1.7_01)

То есть с беты 1.7_01 основание со снятой головой в Java остаётся стоять в состоянии
`extended=true` — это и есть механика «безголовых поршней»
(https://minecraft.wiki/w/Tutorial:Headless_pistons, ссылка со страницы `Piston`,
§ See also). Убирается при этом только сама голова:

> «Piston heads that do not have a valid support block behind them will be removed when
> receiving a shape update from behind in JE»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Piston head)

**Как у нас**

`src/redstone.rs:1025-1028`:

```rust
if block.is("extended", "true") && !head_in_front(world, pos, &block) {
    set(world, pos, AIR);
    return;
}
```

Основание стирается (и, судя по `set`, без выпадения предмета — `destroy` тут не
вызывается, значит `note_destroyed` не срабатывает и поршень пропадает бесследно).
Комментарий над этим кодом («в игре он в этом случае ломается весь») противоречит вики.

**Чем это заметно в игре**

Выдвинуть поршень, сломать киркой голову — в игре остаётся «безголовый поршень»,
пригодный для BUD-схем и известный приём; у нас поршень исчезает целиком и без дропа.

**Как проверить**

Рычаг → поршень выдвинулся → сломать `piston_head`. Смотреть, что осталось в клетке
основания и выпал ли предмет.

**Важность: высокая** (и потеря блока без дропа — отдельно неприятно)

---

## 6. Голова снимается по любому апдейту, а не только по shape-апдейту сзади

**Как в игре**

> «Piston heads that do not have a valid support block behind them will be removed when
> receiving a shape update from behind in JE, or when receiving a block update from any
> direction in BE.»
> — https://minecraft.wiki/w/Piston/Technical_components (§ Piston head)

**Как у нас**

`src/redstone.rs:1039-1043`: в `neighbour_changed` голова проверяет `piston_behind` и
стирается. А `neighbour_changed` вызывается для всех шести соседей изменившегося блока
(`block_changed`, `src/redstone.rs:836-842`) — направление апдейта не хранится вовсе,
PP- и NC-апдейты не различаются.

**Чем это заметно в игре**

Голова, поставленная командой или оставшаяся от безголового поршня, у нас пропадёт от
любого шевеления рядом — в игре нужен именно shape-апдейт со стороны основания.

**Как проверить**

Поставить `piston_head` командой без основания, затем поставить/сломать блок сбоку от
неё. В игре голова остаётся, у нас исчезнет.

**Важность: средняя**

---

## 7. Квазисвязность: «мгновенная» и «по обновлению» — что у нас сходится, а что нет

Это место просили разобрать особенно тщательно, поэтому здесь и совпадения, и
расхождения.

### 7.1 Само правило QC — совпадает

**Как в игре**

> «Quasi-connectivity is a property of dispensers, droppers, and pistons that allows them
> to be activated by anything that would activate the space above them, no matter what is
> actually in that space.»
> — https://minecraft.wiki/w/Tutorial:Quasi-connectivity (вступление)

> «pistons can also be activated if one of the normal methods would activate a mechanism
> component in the space above the piston, even if none are present, or even if the block
> above the piston is non-conductive (including air).» — там же (§ Activation by QC)

> «With quasi-connectivity, a piston facing up can also be powered from the head's
> direction, which is otherwise impossible.»
> — https://minecraft.wiki/w/Piston (§ Quasi-Connectivity)

**Как у нас**

`piston_powered` (`src/redstone.rs:1054-1067`) проверяет ровно две позиции: саму клетку
поршня, пропуская направление головы, и клетку `pos+up` без пропуска. Это буквально
совпадает с вики, включая тонкость «у клетки над поршнем запрета со стороны головы нет».

### 7.2 Запрет питания со стороны головы — совпадает

> «Pistons can be activated from any side, excluding from the head.»
> — https://minecraft.wiki/w/Piston (§ Powering pistons)

> «a piston is not activated by a power component directly in front of it»
> — https://minecraft.wiki/w/Tutorial:Quasi-connectivity (§ Activation by normal methods)

`powered_at(pos, head)` (`src/redstone.rs:1057-1066`) исключает направление `facing`
целиком, для любых источников.

**Оговорка, которую вики не решает однозначно.** Страница `Piston` говорит «from any
side, excluding from the head» (то есть вообще ничто со стороны головы), а страница
`Tutorial:Quasi-connectivity` формулирует исключение уже — только про «power component».
Запитанный непрозрачный блок прямо перед головой под вторую формулировку не подпадает.
**Вики не описывает этот случай явно**, поэтому сказать, правы мы или нет, по вики
нельзя. Наш код следует первой формулировке (не питается ничем со стороны головы).

### 7.3 Мгновенная QC — в целом воспроизводится, но не вся

**Как в игре**

> «Immediate QC activation is the activation of a piston by quasi-connectivity that occurs
> immediately and doesn't require the piston to be separately updated. This only works with
> redstone components that can update other blocks two blocks away from them.»
> — https://minecraft.wiki/w/Tutorial:Quasi-connectivity (§ Immediate QC activation)

Два набора источников, дающих мгновенную QC:

> «The following redstone components can activate mechanism components one block away, but
> update all redstone components up to two blocks away (by taxicab distance): Redstone
> Comparator, Redstone Dust, Redstone Repeater, Redstone Torch.» — там же

> «The following redstone components can activate mechanism components one block away, and
> update redstone components adjacent to the block they are attached to (including above and
> below) as well as redstone components adjacent to themselves: Buttons (attaches in any
> direction), Detector Rail (attaches only downward), Lever (attaches in any direction),
> Pressure Plates (attaches only downward), Trapped Chest (doesn't actually attach, but
> updates as if attached to block beneath), Tripwire Hook (attaches only sideways), Weighted
> Pressure Plates (attaches only downward).» — там же

> «A tripwire hook cannot be attached to a block beneath itself so cannot be used for
> immediate QC activation.» — там же

**Как у нас**

Радиус рассылки задаёт `reach_of` (`src/redstone.rs:870-900`):

- `Kind::Wire | Kind::Torch` → соседи всех шести соседей, то есть полная сфера
  taxicab ≤ 2. **Совпадает** с вики для пыли и факела.
- `Kind::Lever | Kind::Button` → соседи блока крепления; плюс собственные шесть соседей
  добавляет `block_changed` (`src/redstone.rs:841`). Вместе это ровно формулировка вики.
  **Совпадает.**
- `Kind::Plate` → соседи блока под собой, плюс свои соседи. **Совпадает**, и весовые
  плиты сюда попадают: `kind_of` ловит их по суффиксу `_pressure_plate`
  (`src/redstone.rs:247`).
- `Kind::Repeater | Kind::Comparator` → **только** клетка на выходе и её соседи
  (`src/redstone.rs:875-882`). Вики же говорит про все блоки в taxicab ≤ 2 во всех
  направлениях.

**Чем это заметно в игре**

Классические примеры мгновенной QC со страницы вики («Immediate QC Activation by Redstone
Repeater», «by Redstone Comparator») у нас работают: повторитель смотрит в клетку над
поршнем, `reach_of` даёт эту клетку и её соседей, поршень среди них. А вот случаи, где
повторитель или сравнитель обновляет поршень **не выходом**, а боком или тылом в пределах
двух клеток, у нас не отработают.

Отдельно: **детекторная рельса** и **сундук-ловушка** у нас не реализованы вовсе
(`kind_of`, `src/redstone.rs:235-252`), поэтому оба источника мгновенной QC из второго
набора недоступны.

**Как проверить**

Схема «Immediate QC Activation by Redstone Repeater» с вики (рычаг снизу → повторитель →
клетка над поршнем): должна сработать мгновенно, у нас срабатывает. Затем развернуть
повторитель так, чтобы поршень оказался в двух клетках сбоку от него: в игре поршень
получит апдейт, у нас нет.

**Важность: средняя**

### 7.4 QC «по обновлению» — воспроизводится как следствие, но не как правило

**Как в игре**

> «A piston that is currently receiving a redstone signal due to quasi-connectivity but was
> not sent any block update does not activate until it finally receives an update. This
> method of activation is known as update-based QC activation.»
> — https://minecraft.wiki/w/Tutorial:Quasi-connectivity (§ Update-based QC activation)

> «A powered block can activate the space above a piston, from the side or from above,
> without updating the piston, producing an update-based QC activation» — там же

Список того, чем поршень можно «разбудить»:

> «placing or destroying a block next to a piston; moving a block next to a piston;
> changing the state of some blocks next to a piston (for example, changing the delay on a
> repeater); changing some states of some redstone components within two spaces of a piston:
> changing the state of a redstone torch within two spaces of a piston; changing the power
> level (but not the orientation) of redstone dust within two spaces of a piston; changing
> the power level (but not the delay) of a repeater facing a space next to a piston;
> changing the power level or changing the operating mode of a comparator facing a space
> next to a piston.» — там же

**Как у нас**

Явного понятия «QC по обновлению» в коде нет — поведение выходит само собой из того,
что `piston_powered` считается только в `neighbour_changed` (`src/redstone.rs:1018-1035`)
и в `ticked` (`src/redstone.rs:1409`), а `neighbour_changed` до поршня доходит только
через `NEIGHBOUR_ORDER` и `reach_of`. Так что:

- **запитанный непрозрачный блок** над поршнем или сбоку от клетки над ним апдейта
  поршню не шлёт (у `Kind::Other` в `reach_of` пустой список) → поршень стоит, пока его
  не разбудят. **Совпадает с вики.** В коде есть даже тест на это:
  `a_block_of_redstone_moves_a_piston_only_on_an_update` (`src/redstone.rs:2345`), прямо
  ссылающийся на пример «Update-based QC Activation by Block of Redstone» с вики.
- «изменение уровня сигнала пыли/факела в пределах 2 клеток» — работает через
  `reach_of` для `Wire`/`Torch`. **Совпадает.**
- «изменение уровня повторителя, смотрящего в клетку рядом с поршнем» — работает.
  **Совпадает.**
- «изменение режима работы сравнителя» — у нас смена `mode` в `use_block`
  (`src/redstone.rs:1469-1477`) меняет состояние блока, значит апдейт уйдёт.
  **Совпадает.**
- «смена задержки повторителя **не** должна менять уровень сигнала, но **должна** давать
  апдейт соседям поршня» — у нас смена `delay` (`src/redstone.rs:1458-1467`) тоже меняет
  состояние блока и рассылает апдейт, так что совпадает.
- **«moving a block next to a piston»** — блок, двигаемый поршнем, должен будить соседние
  поршни. У нас `move_group` (`src/redstone.rs:1224-1246`) ставит блоки через `set`, то
  есть апдейты рассылает — но мгновенно, одним пакетом, вместо двухтактовой
  последовательности PP→NC из вики (см. §11). Порядок и момент апдейтов будут другими.

**Важность: низкая** для самого факта отложенной QC (он у нас есть), **средняя** для
порядка апдейтов от движущихся блоков.

### 7.5 Чего в проверке QC нет совсем

Вики (`Tutorial:Quasi-connectivity`, вступление) распространяет QC на раздатчики и
выбрасыватели и отдельно исключает крафтер:

> «Despite a functional similarity to dispensers and droppers, crafters are not affected by
> quasi-connectivity.»

У нас раздатчика и выбрасывателя нет как деталей (`kind_of`, `src/redstone.rs:235-252`),
так что вопрос пока не стоит.

---

## 8. Таблица подвижности: десятки блоков ездят вместо того, чтобы ломаться

**Как в игре**

> «Pistons cannot move blocks that require a support block, as they break and drop as an
> item (when applicable).» — https://minecraft.wiki/w/Piston/Table

И далее в таблице, в графе «Breaks when pushed, turning into drops when applicable»
(Java), среди прочих: Amethyst Cluster, Bamboo, Budding Amethyst, Cactus, Carved Pumpkin,
Chorus Flower, Chorus Plant, Copper Golem Statue, Flower Pot, Heads, Jack o'Lantern,
Ladder, Leaves, Lily Pad, Melon, Moss Block, Moss Carpet, Pale Moss Block, Pale Moss
Carpet, Pointed Dripstone, Pumpkin, Sea Pickle, Turtle Egg.
Отдельными строками:

> «Bell, Copper Lantern, Lantern, Soul Lantern | Partial: Breaks when pushed, turning into
> drops when applicable.» (Java)
> «Candles | Partial: Breaks when pushed…» (Java)
> «Campfire, Soul Campfire | Partial: Breaks when pushed…»
> «Anvil | Partial: Can be pushed, but only when in a falling state.»
> «Scaffolding, Suspicious Gravel, Suspicious Sand, Dragon Egg | Partial: Breaks when
> pushed, unless if in a falling state.»
> «Carpets (not moss), Rail, Powered Rail, Activator Rail, Detector Rail | Yes: Can be
> pushed, but breaks if unsupported.»
> — https://minecraft.wiki/w/Piston/Table

**Как у нас**

`push_reaction` (`src/redstone.rs:328-364`) решает по эвристике:

```rust
if !blocks::has_box(state)
    || name.ends_with("_door")
    || name.ends_with("_bed")
    || name.ends_with("shulker_box")
    || name == "cake"
    || name == "decorated_pot"
{
    return Push::Breaks;
}
Push::Moves
```

То есть ломается только то, у чего нет ящика столкновений (`EMPTY_BOX`,
`src/blocks_table.rs:10537`), плюс пять исключений вручную. Проверка по таблице состояний
проекта показывает, что у следующих блоков ящик **есть**, значит у нас они поедут, а в
игре должны сломаться:

`melon`, `pumpkin`, `carved_pumpkin`, `jack_o_lantern`, `moss_block`, `pale_moss_block`,
`moss_carpet`, все `*_leaves`, `cactus`, `ladder`, `bell`, `lantern`, `soul_lantern`,
`candle` и свечи, `amethyst_cluster`, `budding_amethyst`, `turtle_egg`, `flower_pot`,
`player_head` и прочие головы, `chorus_plant`, `chorus_flower`, `sea_pickle`, `campfire`,
`pointed_dripstone`, `lily_pad`, `bamboo`, `copper_golem_statue`, `scaffolding`,
`suspicious_sand`, `suspicious_gravel`, `dragon_egg`, `anvil` (у нас едет всегда, в Java —
только в состоянии падения).

Обратное расхождение: `rail` в нашей таблице лежит в `EMPTY_BOX`, значит у нас он
**ломается**, а по вики рельсы толкаются и ломаются только без опоры.

Состояния «падающего блока» (falling state) в проекте нет вовсе, поэтому строки таблицы
про наковальню, леса, подозрительный песок и яйцо дракона воспроизвести нечем.

**Чем это заметно в игре**

Самое заметное: поршень толкает тыкву, арбуз, листву и блок мха — в игре они разлетаются
в предметы. Ещё заметнее — толкнуть лестницу или фонарь: они поедут по стене.

**Как проверить**

Поршень, перед ним подряд: тыква, блок мха, дубовая листва, фонарь. Дать питание. В игре
поршень выдвинется, а все четыре блока выпадут предметами.

**Важность: высокая**

### 8.1 Пропуски в списках неподвижных блоков

**Как в игре**, в графе «Cannot be pushed» (Java) есть строки, которых у нас нет:

> «Potent Sulfur» — https://minecraft.wiki/w/Piston/Table
> «Test Block, Test Instance Block | Cannot be pushed.» (только Java) — там же

И в графе «Cannot be pushed, because these blocks hold block entities»:

> «Copper Chests», «Shelf» — там же

**Как у нас**

`IMMOVABLE` (`src/redstone.rs:271-304`) не содержит `potent_sulfur`, `test_block`,
`test_instance_block`. `WITH_CONTENTS` (`src/redstone.rs:307-325`) не содержит медных
сундуков и `shelf`. Зато `IMMOVABLE` избыточно перечисляет `chain_command_block` и
`repeating_command_block` — это правильно, вики их объединяет под «Command Block».

**Чем это заметно в игре**

Поршень сдвинет полку и медный сундук; в игре он не выдвинется вовсе.

**Важность: средняя**

---

## 9. Липкий поршень тянет глазурованную керамику

**Как в игре**

> «Glazed Terracotta | Cannot be pulled.» (и Java, и Bedrock)
> — https://minecraft.wiki/w/Sticky_Piston/Table

> «Sticky pistons can no longer pull glazed terracotta.»
> — https://minecraft.wiki/w/Sticky_Piston (история, 1.13 pre8)

**Как у нас**

`retract_piston` (`src/redstone.rs:1262-1273`) проверяет только
`push_reaction(pulled) == Push::Moves`, а `push_reaction` для керамики даёт `Moves`.
Исключение для керамики прописано **только** в `sticks_to` (`src/redstone.rs:1117-1119`),
то есть только для связок слизи и мёда — там всё верно.

**Чем это заметно в игре**

Липкий поршень выдвигается, толкая белую глазурованную керамику, и при снятии питания
утаскивает её обратно. В игре — не утаскивает.

**Как проверить**

Липкий поршень, перед ним белая глазурованная керамика. Рычаг вкл/выкл. В игре керамика
остаётся на выдвинутой позиции.

**Важность: средняя**

### 9.1 Прочие «нельзя тянуть» из таблицы липкого поршня

**Как в игре**

> «Anvil | Cannot be pulled.» (Java)
> «Scaffolding, Suspicious Gravel, Suspicious Sand, Dragon Egg | Cannot be pulled.»
> «Concrete Powder, Gravel, Red Sand, Sand | Can be pulled, but falls if unsupported.
> Cannot be pulled when in a falling state.»
> — https://minecraft.wiki/w/Sticky_Piston/Table

> «A sticky piston cannot pull a falling block.»
> — https://minecraft.wiki/w/Sticky_Piston (§ Pulling)

**Как у нас**

Отдельной таблицы «что можно тянуть» нет: `retract_piston` пользуется тем же
`push_reaction`, что и толкание (`src/redstone.rs:1267`). Наковальня, леса, яйцо дракона,
подозрительный песок у нас тянутся. Состояния падающего блока нет.

**Важность: низкая** (редкие блоки), кроме наковальни — **средняя**.

---

## 10. Липкий блок должен тянуть только то, что липкий поршень может **потянуть**

**Как в игре**

> «A slime block can move any block a sticky piston can pull except for honey blocks, as
> slime blocks and honey blocks never "stick" to each other.»
> — https://minecraft.wiki/w/Slime_Block (§ Pistons)

> «A honey block can move any block a sticky piston can pull except for slime blocks»
> — https://minecraft.wiki/w/Honey_Block (§ Redstone)

**Как у нас**

`sticks_to` (`src/redstone.rs:1108-1126`) проверяет `push_reaction(...) == Push::Moves` —
то есть критерий «можно **толкнуть**», а не «можно **потянуть**». Разница ровно в том,
что перечислено в §9.1 плюс керамика (керамику мы отдельно исключили, остальное — нет).

**Чем это заметно в игре**

Слизь, толкаемая поршнем, потащит за собой прилипшую наковальню или леса; в игре не
потащит.

**Важность: низкая**

### 10.1 Что в липких блоках у нас совпадает — точно

- «Слизь и мёд друг к другу не липнут»:

  > «Slime and honey blocks do not stick to each other. This allows redstone contraptions to
  > use alternating slime and honey blocks side by side without interfering with each other.»
  > — https://minecraft.wiki/w/Piston (§ Sticky blocks)

  `sticks_to`, `src/redstone.rs:1121-1125` — сравнение `other == sticky`. **Совпадает.**

- «Керамика не липнет, но толкается»:

  > «Glazed terracotta and the heavy core (BE) do not stick to adjacent slime or honey
  > blocks, even though they can be pushed by pistons.» — там же

  `src/redstone.rs:1117-1119`. **Совпадает.** (Тяжёлое ядро — исключение только Bedrock,
  и у нас его правильно нет.)

- «Липкий поршень сам к липким блокам не относится»:

  > «Slime and honey blocks are sticky … Despite their name, sticky pistons do not fall
  > under this definition.» — там же

  `sticky_kind` (`src/redstone.rs:1095-1100`) знает только `slime_block` и `honey_block`.
  **Совпадает**, и комментарий в коде это проговаривает.

- «Односторонность»:

  > «Slime and honey blocks' stickiness is unilateral. They can't be pulled by a non-sticky
  > piston, and they are not moved if an adjacent (non-sticky) block is moved by a piston.»
  > — там же

  В `push_group` (`src/redstone.rs:1167-1175`) соседей добавляет только сам липкий блок;
  обычный блок соседей не зовёт. **Совпадает.**

- «Неподвижный сосед игнорируется, но подвижный, которому мешают, всё блокирует»:

  > «Any block that cannot be moved or can be broken by a piston does not stick to sticky
  > blocks. However, if an adjacent block could be moved but is prevented from being moved
  > by the presence of an immovable block, the sticky block is prevented from moving as
  > well, and the piston does not extend.» — там же

  `sticks_to` возвращает `false` для `Stays` и `Breaks` (сосед не зовётся), а если сосед
  `Moves` и упирается — `push_group` доходит до упора и возвращает `None`
  (`src/redstone.rs:1152`). **Совпадает.**

- «Поршень не двигает сам себя через крюк из липких блоков»:

  > «A piston cannot move itself via a "hook" constructed of sticky blocks, but flying
  > machines can be created with multiple pistons.» — там же

  `push_group` пропускает клетку самого поршня (`src/redstone.rs:1142`). **Совпадает по
  результату**; вики не описывает, каким именно способом игра это обеспечивает.

### 10.2 Чего вики требует, а у нас нет: жидкости — исключение

> «Liquids are an exception: they aren't moved, but neither do they stop a piston from
> pushing or pulling blocks into their space (usually destroying the liquid…)»
> — https://minecraft.wiki/w/Slime_Block (§ Pistons)

У нас вода и лава попадают в `Push::Breaks` (нет ящика столкновений), и `push_group` их
пропускает (`src/redstone.rs:1150`), а `move_group` их сносит `destroy`
(`src/redstone.rs:1231-1237`). По результату — **совпадает** (жидкость уничтожается, ход
не отменяется). Но в дроп вода при этом попадёт как «сломанный блок» через
`note_destroyed`, а это поведение вики не описывает — там сказано лишь «destroying the
liquid». Стоит проверить, не выпадает ли у нас из воды предмет.

---

## 11. Блок-апдейты от поршня: рассылаются, но не так и не тогда

**Как в игре**

> «Pistons: during the start of the extension/retraction process, every time a moving piston
> block gets placed and every time one of the normal blocks is replaced/removed, PP updates
> are sent to these blocks' neighbors. Once all moving piston blocks have been placed, the
> game will send NC updates to the replaced/removed normal blocks' neighbors.»
> — https://minecraft.wiki/w/Block_update (§ Special blocks)

> «Sticky pistons, however, do not send NC updates around their head when they start to pull
> blocks: only the NC updates caused by the moving piston block turning back into a normal
> block are sent, once the retraction process is over. If a sticky piston fails to retract a
> slime block or a honey block due to the pull limit, no updates will be sent around the head
> whatsoever.» — там же

**Как у нас**

Разделения на PP (post placement / shape) и NC (neighbor changed) в проекте нет вообще:
`set` → `World::set_block` → запись в `changed` → `settle` → `neighbour_changed` для шести
соседей и для самого блока (`src/redstone.rs:815-865`). Поэтому:

- при выдвигании (`extend_piston`, `src/redstone.rs:1182-1217`) все апдейты уходят
  одним махом в момент перестановки, без деления на «сначала PP по каждому, потом NC
  пачкой»;
- при втягивании липкого поршня (`retract_piston`, `src/redstone.rs:1258-1260`) голова
  убирается через `set(world, front, AIR)`, и это **шлёт полный набор апдейтов вокруг
  головы** — ровно то, чего по вики быть не должно;
- случай «липкий поршень не смог утянуть слизь из-за лимита в 12 → вообще никаких
  апдейтов вокруг головы» у нас **выходит верно** случайно: `push_group` вернёт `None`,
  `move_group` не вызовется, блоки не изменятся — значит и апдейтов не будет. Но голова
  всё равно уже снята строкой выше, и её снятие апдейты пошлёт.

**Чем это заметно в игре**

Любая схема, использующая «тишину» вокруг головы липкого поршня при втягивании (а на
этом стоят многие BUD- и дюпер-схемы), поведёт себя иначе.

**Как проверить**

Липкий поршень, у головы — блок; сбоку от головы — наблюдатель или пыль. Снять питание.
В игре во время начала втягивания вокруг головы NC-апдейтов нет.

**Важность: средняя** (высокая, как только появится `moving_piston`)

### 11.1 Порядок обхода соседей

**Как в игре** страница `Block update` описывает порядок рассылки PP-апдейтов как
west, east, north, south, down, up (см. нашу прошлую выписку §1.4 — цитата из
https://minecraft.wiki/w/Block_update).

**Как у нас** `NEIGHBOUR_ORDER` (`src/redstone.rs:79`) = west, east, down, up, north,
south. Это порядок, описанный на странице `Redstone mechanics` для NC-обновлений, и для
редстоуна он верен; но для PP-апдейтов поршня вики называет другой порядок, а мы эти два
вида апдейтов не различаем.

**Важность: низкая** (пока нет `moving_piston`)

---

## 12. Толкание в пустоту и за предел построек: проверки нет, возможна паника

**Как в игре**

> «Pistons cannot push: … Blocks into the void or above the build limit. … In Java Edition,
> blocks beyond the world border.»
> «If the requirements for a block to be pushed are not met, the piston will not extend.»
> — https://minecraft.wiki/w/Piston (§ Limitations)

**Как у нас**

`push_group` (`src/redstone.rs:1135-1179`) шагает вперёд без проверки высоты, а
`World::get_block` возвращает `AIR` только для незагруженного чанка
(`src/world/mod.rs:440-446`); внутри чанка индекс считает `split(y)`
(`src/world/mod.rs:238-246`) без ограничения сверху и снизу. Запись через
`Chunk::set`/`move_group` для `y` выше последней секции даст выход за границы массива.
Границы мира (`world border`) в проекте нет.

**Чем это заметно в игре**

Поршень, стоящий у потолка мира и смотрящий вверх, в игре просто не выдвигается. У нас,
судя по коду, либо выдвинется, либо сервер упадёт.

**Как проверить**

Поставить поршень на максимальной высоте лицом вверх, положить перед ним блок, дать
питание.

**Важность: высокая** (падение сервера), сама же проверка предела — средняя.

---

## 13. Сущности поршень не замечает

**Как в игре**

> «Any entities in the path of an extending piston (or any block it might be moving) are
> pushed along, when possible. If the entities cannot be moved, the block is pushed inside
> them, suffocating mobs when pushed into their eye height (assuming said block is solid).»
> — https://minecraft.wiki/w/Piston (§ Redstone component)

> «When being pushed by a piston, entities (except ender dragons, item frames, paintings,
> and cushions) that are ahead are launched into the direction the block is pushed into, at
> an initial speed of 20 blocks a second. When pulled by a piston, no entities are
> launched.» — https://minecraft.wiki/w/Slime_Block (§ Pistons)

> «When being moved by a piston, entities on a honey block's top surface move with it. They
> are not launched in the direction of the push, as a slime block would do. Honey blocks
> moved by pistons do not move entities that are touching the side or bottom of the block.»
> — https://minecraft.wiki/w/Honey_Block (§ Usage)

**Как у нас**

В `src/entity/mod.rs` слова «piston» нет; `extend_piston`/`move_group` сущностей не
трогают.

**Чем это заметно в игре**

Игрока и выброшенные предметы поршень не сдвигает; блок вдвигается в них; отбрасывания
слизью нет, мёд никого не везёт; удушения нет.

**Важность: низкая** (у проекта физики почти нет, и прошлая выписка сама относила это к
отложенному)

---

## 14. Скульк-вибрации и звук

**Как в игре**

> «Pistons emit vibrations detectable by sculk sensors. A calibrated sculk sensor tuned to
> signal strength 10 detects piston retraction; strength 11 detects extension; strength 12
> detects a block broken by a piston.» — https://minecraft.wiki/w/Piston (§ Sculk detection)

> «The piston makes a sound that can be heard within a 31×31×31 cube centered on the
> activating piston.» — https://minecraft.wiki/w/Piston (§ Redstone component)

**Как у нас** ничего этого нет.

**Важность: низкая**

---

## Что совпадает

1. **Правило квазисвязности целиком** — две проверяемые клетки, отсутствие запрета со
   стороны головы для верхней клетки (`piston_powered`, `src/redstone.rs:1054-1067`).
2. **Запрет питания со стороны головы** для самого поршня.
3. **Отложенная («по апдейту») QC от запитанного непрозрачного блока** — воспроизводится,
   и на это есть тест `a_block_of_redstone_moves_a_piston_only_on_an_update`
   (`src/redstone.rs:2345`), сделанный прямо по примеру с вики.
4. **Мгновенная QC от пыли, факела, рычага, кнопки и нажимной плиты** — радиусы
   рассылки в `reach_of` (`src/redstone.rs:870-900`) совпадают с формулировками вики.
5. **Предел в 12 блоков**, считаемый по всей связке, с отменой всего хода при превышении
   (`PUSH_LIMIT`, `src/redstone.rs:50`; `push_group`, `src/redstone.rs:1158-1160`).
   Есть тест `a_piston_pushes_at_most_twelve_blocks` (`src/redstone.rs:1988`).
6. **Отмена всего хода при упоре в неподвижный блок** — «If the requirements for a block
   to be pushed are not met, the piston will not extend» (`src/redstone.rs:1152`).
   Тест `immovable_blocks_stop_the_piston` (`src/redstone.rs:2019`).
7. **Липкие блоки**: слизь и мёд как единственные липкие; слизь и мёд не липнут друг к
   другу; керамика не липнет; липкий поршень не относится к липким блокам;
   односторонность; неподвижный сосед игнорируется, а заблокированный подвижный отменяет
   ход; поршень не двигает сам себя. Подробно — §10.1.
8. **Липкий поршень тянет один блок у головы плюс его липкую связку**, и «когда тянуть
   нельзя — просто задвигается впустую» («when a sticky piston is unpowered but cannot pull
   a block, it retracts without doing so», https://minecraft.wiki/w/Sticky_Piston):
   `retract_piston`, `src/redstone.rs:1249-1279`.
9. **Обычный поршень при задвигании ничего не тянет** — `if block.name.starts_with("sticky")`
   (`src/redstone.rs:1262`).
10. **Выдвинутый поршень и голова неподвижны** (`push_reaction`, `src/redstone.rs:338-344`;
    `piston_head` в `IMMOVABLE`, `src/redstone.rs:296`).
11. **Блоки с блок-сущностями в Java не двигаются** — список `WITH_CONTENTS`
    (`src/redstone.rs:307-325`) построен по правильному принципу, Bedrock-поведение
    (двигаются, большой сундук делится) правильно не реализовано.
12. **`extended=true` ставится последним шагом выдвигания** («Pistons changing their
    `extended` state from `false` to `true` is now the last step of (the start of) the
    extension process», https://minecraft.wiki/w/Piston, история 13w10a):
    `extend_piston`, `src/redstone.rs:1212-1214`.
13. **Голова получает правильные `facing`, `type` (normal/sticky) и `short=false`**
    (`src/redstone.rs:1200-1206`).
14. **Голова снимается, когда за ней нет выдвинутого поршня** (`piston_behind`,
    `src/redstone.rs:1081-1089`) — правило верное, спорен только повод (см. §6).
15. **Блоки, сломанные поршнем, выпадают предметами** — `destroy` → `note_destroyed`
    (`src/redstone.rs:804-807`), что соответствует «turning into drops when applicable».
16. **Из Bedrock ничего лишнего не взято**: нет soft inversion, нет автозагиба пыли к
    поршню, нет фиксированной задержки в 2 такта, нет движения блок-сущностей, тяжёлое
    ядро не исключено из липких связок.

---

## Чего у нас вообще нет

1. Блок `moving_piston` и его блок-сущность (`blockState`, `extending`, `facing`,
   `progress`, `source`); двухтактовая длительность хода.
2. Фаза block events в такте; задержка старта 0 тактов.
3. Прерывание выдвигания при снятии питания; «block dropping» липкого поршня.
4. Свойство `short=true` у головы во время хода.
5. Механика безголовых поршней (у нас основание уничтожается — §5).
6. Разделение апдейтов на PP (shape) и NC (neighbor changed); направление апдейта.
7. Особый порядок апдейтов поршня и тишина вокруг головы липкого поршня при втягивании.
8. Состояние «падающего блока» (falling block) — а с ним вся строка таблицы про
   наковальню, леса, подозрительный песок и яйцо дракона, и запрет тянуть падающий блок.
9. Отдельная таблица «что можно **тянуть**» (Sticky Piston/Table) — тяга использует
   таблицу толкания.
10. Проверки «не толкать в пустоту, выше предела построек и за границу мира».
11. Взаимодействие с сущностями: толкание, отбрасывание слизью (20 бл/с), перевозка
    мёдом, удушение.
12. Исключения для рельсов и мёртвых кораллов (опора в новой позиции, переориентация
    рельсов, коралл, который одновременно ломается и едет).
13. Звук поршня (куб 31×31×31), частицы разрушения, звук ломаемого блока, скульк-вибрации
    (сила 10/11/12).
14. Раздатчик и выбрасыватель — вторые и третьи носители квазисвязности; крафтер, который
    ей не подвержен.
15. Детекторная рельса и сундук-ловушка как источники мгновенной QC.

---

## Ошибки и неточности в прошлой выписке `tools/research/redstone-pistons.md`

Выписка в целом аккуратная и подтверждается страницами вики. Замечания:

1. **§4.1, строка «potent_sulfur»** — записана без перевода и без пометки; в таблице вики
   это отдельная строка `Potent Sulfur`. В нашем коде её нет (см. §8.1). Не ошибка
   выписки, но пункт потерялся при переносе в код.
2. **§4.1 не содержит `shelf` и медных сундуков**, хотя они есть в графе «блоки с
   блок-сущностями» обеих таблиц вики (`Piston/Table`, `Sticky Piston/Table`).
   `copper chests` в §4.1 упомянуты в скобках, `Shelf` отсутствует совсем.
3. **§4.2 утверждает «candles (свечи) — в Java ломаются при толчке; BE: двигаются»** —
   это верно, но в §4.5 (что нельзя тянуть) свечи не упомянуты, хотя `Sticky Piston/Table`
   даёт для Java «Candles | Cannot be pulled» в одной строке с колоколом и фонарями.
4. **§4.5 говорит «heavy_core — Java: тянуть можно. BE: нельзя»** — верно по
   `Sticky Piston/Table`. Но §6 там же пишет «heavy_core … только в BE (в Java он
   прилипает и тянется)» — формулировки надо читать вместе, по отдельности вторая
   выглядит как противоречие первой. Обе по сути верны.
5. **§1.4, порядок PP-апдейтов «west, east, north, south, down, up»** — этот порядок
   отличается от порядка NC-обновлений («west, east, down, up, north, south»), который
   использован в нашем коде (`NEIGHBOUR_ORDER`, `src/redstone.rs:79`). В выписке эти два
   порядка стоят рядом без предупреждения, что они разные, — легко перепутать (и, судя по
   коду, пока применяется один на оба случая).
6. **§3.3 в перечне источников мгновенной QC даёт крюк растяжки как «не годится»** — это
   верно («A tripwire hook cannot be attached to a block beneath itself so cannot be used
   for immediate QC activation»), но в тот же список попал «рельс-детектор (только вниз)»
   без оговорки, что у нас его нет. Мелочь.
7. **§2.1, фраза «Сообщество также называет 1 игровой такт "полутиком редстоуна"»** — на
   просмотренных страницах вики (`Piston`, `Tick`) этой формулировки я не нашёл.
   **Вики этого не описывает** в том месте, на которое ссылается выписка; либо цитата из
   другой страницы, либо добавлена от себя.
8. **§7, «Практический пример из вики (раздел про обход QC котлом)»** — на странице
   `Piston` и на `Tutorial:Quasi-connectivity` раздела про котёл нет. Источник не сходится;
   утверждение про «импульс короче 3 игровых тактов» проверить по имеющимся страницам не
   удалось. **Считать непроверенным.**
9. **§1.2, «Голова считается блоком со сплошной верхней поверхностью, если смотрит вверх»**
   — на странице `Piston/Technical components` этого нет. Вероятно взято со страницы
   `Opacity`/`Solid block`; источник в выписке не указан. Стоит перепроверить.

Всё остальное в выписке я сверил с сохранёнными страницами и подтвердил дословно.

---

*Всё в этом файле взято только с minecraft.wiki (страницы Piston, Sticky Piston,
Piston/Table, Sticky Piston/Table, Piston/Technical components,
Tutorial:Quasi-connectivity, Slime Block, Honey Block, Block update, Tick). Код Minecraft,
декомпилированный код, client.jar/server.jar, ванильные data pack'и, а также код
неофициальных серверных ядер и модов (Paper, Spigot, Fabric, Forge, Bukkit, ViaVersion и
любых других) не использовался, не открывался, не скачивался и не пересказывался.*
