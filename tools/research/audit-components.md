# План

Режим планирования не даёт записать `tools/research/audit-components.md`.
Аудит проведён полностью; ниже — готовое содержимое файла, его нужно записать
как есть в `/home/lenox/Загрузки/minecraft_rust_server/tools/research/audit-components.md`.
Кода менять не требуется — отчёт только фиксирует факты.

---

# Аудит компонентов редстоуна

Сверка нашего `src/redstone.rs` со страницами minecraft.wiki (Java Edition).
Кода Minecraft и серверных ядер не использовалось: только сохранённые страницы
вики и запросы к minecraft.wiki.

Все «такты» ниже — игровые (game ticks), 20 в секунду. Редстоун-такт — два
игровых.

---

## Расхождения

### 1. Лампа гаснет мгновенно, а должна через 4 такта

**Как в игре.** «A redstone lamp activates instantly, but takes 4 ticks (0.2
seconds) to turn off {{in|java}}».
Источник: https://minecraft.wiki/w/Redstone_Lamp (раздел Usage).

**Как у нас.** `src/redstone.rs:987-995` — в `neighbour_changed` ветка
`Kind::Lamp` считает `any_power` и сразу же ставит `lit` в новое значение, без
планирования такта. Включение и выключение одинаково мгновенные.

**Чем это заметно в игре.** Короткий импульс (кнопка рядом с лампой, или лампа
от повторителя на задержке 1) у нас даёт лампе точно такую же длину свечения,
как длина импульса. В игре лампа «дотягивает» ещё 4 такта. И наоборот: цепочка
из ламп, работающая как задержка (лампа гасит факел на себе), у нас вообще не
задерживает.

**Как проверить.** Лампа, к ней вплотную рычаг. Щёлкнуть рычаг и тут же
щёлкнуть обратно (или собрать: кнопка → повторитель на 1 → лампа, а рядом
второй повторитель от той же кнопки, чтобы видеть тайминг). Снять лог
состояний лампы: у нас `lit=false` приходит в тот же такт, что и пропажа
сигнала, в игре — на 4 такта позже. Ещё нагляднее: 2-тактовый импульс
(наблюдатель) в игре зажигает лампу на 6 тактов, у нас — на 2.

**Важность:** средняя (видно сразу, но схемы обычно не ломает).

---

### 2. Нажимные плиты не срабатывают вовсе

**Как в игре.** «Pressure plates are activated while certain entities are on
top of them. Wood, light weighted, and heavy weighted pressure plates are
activated by all entities, including players, mobs, items, shot arrows, and
thrown tridents. Stone pressure plates can only be activated by players and
mobs.»
Источник: https://minecraft.wiki/w/Redstone_components (раздел Pressure plate).

**Как у нас.** `Kind::Plate` заведён (`src/redstone.rs:247`), плита умеет
питать (`:416` — 15 во все стороны, `:474` — питает блок под собой, `:897` —
сообщает соседям блока под собой) и умеет отваливаться без опоры (`:1012`,
`:1313`). Но нигде во всём дереве `src/` нет кода, который ставил бы плите
`powered=true`: поиск по `Kind::Plate` и `pressure_plate` даёт только
перечисленные места. Сущности на плиту не смотрят. Итог: плита — мёртвый блок.

**Чем это заметно в игре.** Встать на любую плиту — ничего не происходит: ни
дверь, ни лампа, ни провод рядом не включаются.

**Как проверить.** Положить `stone_pressure_plate`, рядом провод и лампу.
Пройти по плите. Ожидаемо: лампа горит, пока игрок на плите, и ещё минимум 20
тактов после. У нас: не горит никогда.

**Важность:** высокая.

---

### 3. Весовые плиты (золотая и железная) не дают переменной силы и всегда выключены

**Как в игре.**
Золотая: «emit a redstone signal strength that is equal to the amount of
entities on top of them, up to a maximum power level of 15».
Источник: https://minecraft.wiki/w/Light_Weighted_Pressure_Plate
Железная: «emit a redstone signal strength that is equal to 1⁄10 of the amount
of entities on top of them (rounded up to the nearest integer), up to a maximum
power level of 15».
Источник: https://minecraft.wiki/w/Heavy_Weighted_Pressure_Plate
И там же на сводной странице: «wood and stone pressure plates emit redstone
signals of strength 15 … light and heavy weighted pressure plates emit a
redstone signal whose strength depends on the number of entities on the
pressure plate».
Источник: https://minecraft.wiki/w/Redstone_components

**Как у нас.** `kind_of` (`src/redstone.rs:247`) ловит по суффиксу
`_pressure_plate`, так что `light_weighted_pressure_plate` и
`heavy_weighted_pressure_plate` тоже становятся `Kind::Plate`. Дальше
`is_on` (`src/redstone.rs:399`) спрашивает у них свойство `powered`, которого у
весовых плит нет вообще (у них свойство `power` 0–15), и `Block::value`
возвращает `None` → плита считается выключенной навсегда. А `source_output`
(`src/redstone.rs:416`) и `strong_power` (`:474`) для любой включённой плиты
возвращают ровно `MAX_POWER`, то есть переменной силы не предусмотрено даже в
замысле.

**Чем это заметно в игре.** Классическая схема «золотая плита → компаратор →
разная длина дорожки по числу предметов» не работает совсем.

**Как проверить.** Положить золотую плиту, к ней провод из 15 блоков. Бросить
на плиту один предмет: в игре загорится один блок провода, у нас — ни одного.

**Важность:** средняя (пока плиты вообще не работают — см. п. 2; но при их
починке это отдельная ошибка, которую легко не заметить).

---

### 4. У плит нет задержки выключения

**Как в игре.** «Light weighted and heavy weighted pressure plates are always
active for a multiple of 10 game ticks (0.5 seconds). For example, if an entity
moves off a light weighted pressure plate after 15 game ticks, the pressure
plate waits 5 ticks before turning off. Light and heavy weighted pressure plates
are always active for at least 10 game ticks.» и «Wood and stone pressure plates
are also always active for a multiple of 10 game ticks, but they first wait 10
game ticks after an entity moves off of them, then deactivate at the next
multiple of 10 game ticks. … This means wood and stone pressure plates are
always active for at least 20 game ticks.»
Источник: https://minecraft.wiki/w/Redstone_components (раздел Pressure plate).

**Как у нас.** Планирования такта для плит нет нигде: `ticked`
(`src/redstone.rs:1323-1426`) не имеет ветки `Kind::Plate`, `neighbour_changed`
(`:1012`) для плиты только проверяет опору. Констант вида 10/20 тактов в файле
нет (сравн. `STONE_BUTTON_TICKS`/`WOODEN_BUTTON_TICKS`, `src/redstone.rs:53-54`).

**Чем это заметно в игре.** Пробежать по каменной плите: в игре сигнал держится
минимум 20 тактов (1 секунда) и гаснет по кратности 10; у нас (когда плиты
заработают) он погаснет в тот же такт, что и уход игрока.

**Как проверить.** Плита → лампа. Пробежать по плите не останавливаясь и
посчитать такты свечения: должно быть ≥20 и кратно 10.

**Важность:** средняя.

---

### 5. Калитка (fence gate) не знает про редстоун — и вообще не открывается

**Как в игре.** «A fence gate activates immediately upon receiving a redstone
signal.» и «When a fence gate receives a redstone signal, the fence gate opens,
or stays open if it was already open. When the redstone signal stops, the fence
gate closes.»
Источник: https://minecraft.wiki/w/Redstone_components (раздел Fence gate).

**Как у нас.** `kind_of` (`src/redstone.rs:235-252`) не знает имени
`*_fence_gate`: калитка попадает в `Kind::Other`. В `neighbour_changed`
(`src/redstone.rs:1045`) `Kind::Other` — пустая ветка. В `use_block`
(`src/redstone.rs:1436-1533`) ветки для калитки тоже нет — значит, калитка не
открывается и рукой: `use_block` вернёт `false`, и сервер попробует поставить
блок из руки.

**Чем это заметно в игре.** Калитка в заборе не открывается ни щелчком, ни
рычагом.

**Как проверить.** Поставить `oak_fence_gate`, щёлкнуть по ней пустой рукой —
должна открыться. Затем положить рядом рычаг и включить — должна открыться и
закрыться по сигналу.

**Важность:** высокая.

---

### 6. `faces_diode` перепутал «зад» и «перед» соседнего диода — приоритеты тактов встают неверно

**Как в игре.** «If a redstone repeater is facing the back or side of another
diode, its block tick has a priority of -3. If a redstone repeater is
depowering, it has a priority of -2. Otherwise, the repeater has a priority
of -1. If a redstone comparator is facing the back or side of another diode, it
has a priority of -1. All other block ticks have a priority of 0.»
Источник: https://minecraft.wiki/w/Tick (раздел Scheduled ticks).

И про смысл свойства: «facing — The direction from the *output* side to the
*input* side of a repeater.»
Источник: https://minecraft.wiki/w/Redstone_Repeater (Block states).

**Как у нас.** `src/redstone.rs:711-719`:

```rust
let target = Block::at(world, step(pos, output));

matches!(target.kind, Kind::Repeater | Kind::Comparator) && output_dir(&target) != Some(output)
```

Разбор. Пусть наш диод выдаёт в сторону `output`. Сосед стоит в
`step(pos, output)`.
* Если сосед выдаёт в ту же сторону (`output_dir(target) == output`), его
  вход — со стороны нас, то есть мы смотрим ему **в зад**. Вики требует −3, наш
  код даёт `false` → −2 или −1.
* Если сосед выдаёт навстречу (`output_dir(target) == output.opposite()`), мы
  смотрим ему **в перед**. Вики требует обычный приоритет, наш код даёт
  `true` → −3.
* Боковой случай (сосед выдаёт поперёк) у нас получается верно.

То есть из трёх случаев два поменяны местами. Та же функция используется и для
сравнителя (`src/redstone.rs:977-981`), так что ошибка тиражируется.

**Чем это заметно в игре.** Только там, где два диода срабатывают в один такт и
порядок решает: повторитель, смотрящий в зад другого повторителя, у нас
обработается позже, чем должен. Это меняет поведение плотных схем — фиксаторов
(RS-защёлок из двух повторителей «в зад друг другу»), мгновенных повторителей,
таймингов у поршневых дверей.

**Как проверить.** Два повторителя в линию, смотрящие в одну сторону (A → зад
B). Подать на A сигнал и одновременно менять что-то у B в тот же такт. Проще —
собрать защёлку из двух повторителей и подать на оба входа импульс одной
кнопкой через равные задержки: в игре защёлка встаёт в определённое состояние,
у нас — в другое или дребезжит.

**Важность:** средняя (проявляется только на тактово-плотных схемах, но там
ломает всё).

---

### 7. Компаратор не читает содержимое блоков сзади

**Как в игре.** «A redstone comparator treats certain blocks behind it as power
sources and outputs a signal strength proportional to the block's state. The
comparator may be separated from the measured block by a comparator-conducting
block.» и «{{IN|je}}, if the intervening block is powered to signal strength 15,
then the comparator outputs 15 no matter the fullness of the container.»
Источник: https://minecraft.wiki/w/Redstone_Comparator (раздел Read blocks).

**Как у нас.** `comparator_output` (`src/redstone.rs:763-783`) берёт зад
исключительно как силу сигнала: `diode_input` → `power_from`. Чтения
содержимого сундуков, воронок, печей, музыкального ящика, котла, торта,
кафедры, зарядного якоря нет; чтения через проводящий блок тоже нет.

**Чем это заметно в игре.** Компаратор за сундуком ничего не выдаёт.

**Как проверить.** Сундук, за ним компаратор, за компаратором провод. Положить
в сундук один предмет: в игре компаратор даёт 1, у нас — 0.

**Важность:** средняя (у нас пока нет самих блоков-контейнеров в мире, поэтому
чинить нечего; но пункт нужно держать в списке).

---

### 8. Сила на выходе компаратора живёт только в памяти и теряется

**Как в игре.** Вики про хранение данных сервером не пишет — это не описано.
Но с точки зрения игрока компаратор, выдающий 7, продолжает выдавать 7 и после
перезахода в мир.

**Как у нас.** Сила выхода лежит в `Memory::comparator_output`
(`src/redstone.rs:152`) — это `HashMap` в оперативной памяти. Она чистится при
выгрузке чанков (`forget_outside`, `src/redstone.rs:164-170`, вызов —
`src/world/mod.rs:531`) и нигде не сохраняется на диск (поиск по
`comparator_output` даёт только сам redstone.rs и world/mod.rs). Состояние блока
хранит только `powered` (да/нет), а не силу.

**Чем это заметно в игре.** Уйти за предел прогрузки и вернуться (или
перезапустить сервер): дорожка провода от компаратора, светившаяся на 7 блоков,
станет пустой, пока компаратору не придёт обновление.

**Как проверить.** Собрать «компаратор в режиме вычитания, сзади 15, сбоку 8» →
выход 7, посмотреть на длину дорожки. Перезапустить сервер, вернуться: дорожка
пропадёт.

**Важность:** средняя.

---

### 9. Деревянная кнопка не нажимается стрелой, каменная — ветрозарядом

**Как в игре.** «A wooden button can also be activated by a fired arrow, a
thrown wind charge, or a thrown trident if its collision box touched the
button.» и «A wooden button activated by a fired arrow or a thrown trident
remains active until the arrow or trident despawns (after 1 minute) or is picked
up by a player.»
Источник: https://minecraft.wiki/w/Wooden_Button
И общее: «A button … produces a temporary redstone signal and powers its
attached block when pressed by a player or a wind charge.»
Источник: https://minecraft.wiki/w/Button

**Как у нас.** Нажатие бывает только от щелчка игрока: `use_block`,
`src/redstone.rs:1442-1456`. Время нажатия всегда фиксированное —
`STONE_BUTTON_TICKS = 20` или `WOODEN_BUTTON_TICKS = 30`
(`src/redstone.rs:53-54`). Взаимодействия со стрелами/трезубцами/ветрозарядом
нет.

**Чем это заметно в игре.** Выстрелить в деревянную кнопку — ничего.

**Как проверить.** Деревянная кнопка на стене, лампа рядом. Выстрелить в
кнопку из лука: в игре лампа горит, пока стрела торчит в кнопке.

**Важность:** низкая (нужны стрелы как сущности, которых у нас нет).

---

### 10. Порог «остывания» выгоревшего факела на единицу шире, чем на вики

**Как в игре.** «A redstone torch experiences "burn-out" when it is forced to
turn off more than eight times in 60 game ticks (3 seconds). After burning out,
a redstone torch … ignores attempts to change its state until the number of
state changes in the last 60 game ticks drops to fewer than eight.»
Источник: https://minecraft.wiki/w/Redstone_Torch (раздел Behavior).

**Как у нас.** `src/redstone.rs:1327-1356`: выгорание при `toggles.len() > 8`
(константа `BURNOUT_TOGGLES = 8`, `src/redstone.rs:36`) — это «больше восьми»,
совпадает. Но условие возврата к работе у нас — та же проверка наоборот, то есть
факел оживает уже при 8 выключениях за окно, тогда как вики требует «fewer than
eight», то есть 7 и меньше. Разница в один шаг окна.

**Чем это заметно в игре.** На факельном генераторе тактов на грани выгорания
период у нас чуть другой: факел выходит из выгорания на один такт раньше.

**Как проверить.** Собрать двухфакельный генератор на выгорании и снять период
в тактах: сравнить с периодом в игре.

**Важность:** низкая.

---

### 11. Опора у деталей проверяется грубее, чем описано

**Как в игре.** «A repeater can be placed only on top of full blocks (dirt,
stone, etc.), on top of upside-down slabs, upside-down stairs, furnaces, and
glass.»
Источник: https://minecraft.wiki/w/Redstone_Repeater
«A pressure plate can be attached to the top of any opaque block, or to the top
of a fence, nether brick fence, an upside-down slab or upside-down stairs.»
Источник: https://minecraft.wiki/w/Redstone_components
«[Redstone torch] can be attached to the top or sides of any conductive block …
Redstone torches cannot be attached to the bottoms of any blocks.»
Источник: https://minecraft.wiki/w/Redstone_Torch

**Как у нас.** `supported` (`src/redstone.rs:1311-1320`):
провод, повторитель, сравнитель и плита держатся на `blocks::solid_top`, а тот
(`src/blocks.rs:326-337`) считает опорой **любую** плиту (в том числе нижнюю,
хотя вики требует перевёрнутую) и **любой** люк (в том числе открытый);
ступеней в списке нет вовсе. Факел, рычаг и кнопка держатся на `has_box`
(`src/redstone.rs:1316`) — то есть на чём угодно, у чего есть ящик
столкновений, без проверки «проводящий/непрозрачный» и без запрета на низ.

**Чем это заметно в игре.** Повторитель у нас останется лежать на нижней плите
и на открытом люке; факел удержится там, где в игре не удержится.

**Как проверить.** Поставить нижнюю плиту, на неё повторитель. В игре
повторитель поставить нельзя; у нас он держится и работает.

**Важность:** низкая (это правила установки, не работа сигнала).

---

### 12. Про боковой вход компаратора вики говорит двумя разными фразами

Это не расхождение, а неопределённость, которую стоит зафиксировать.

**Как в игре.** На странице компаратора: «Side inputs are accepted only if the
signal received is strongly powering either side of the redstone comparator.
{{IN|je}}, blocks of redstone are also accepted if placed directly next to the
comparator.» (https://minecraft.wiki/w/Redstone_Comparator)
На сводной странице: «The side inputs must receive a signal from a transmission
component (redstone dust, redstone repeater, redstone comparator), or a block of
redstone. Other power components cannot power the side inputs.»
(https://minecraft.wiki/w/Redstone_components)

Первая фраза разрешает сильно запитанный блок сбоку, вторая перечисляет только
провод, повторитель, сравнитель и блок редстоуна.

**Как у нас.** `comparator_side` (`src/redstone.rs:742-760`) принимает и то, и
другое: провод, смотрящий в бок; повторитель/сравнитель/блок редстоуна; а также
`_ if beside.conductive() => strong_power(...)` — сильно запитанный блок.
Рычаг, кнопка и факел сбоку не принимаются — это согласуется со второй фразой.

**Что делать.** Наш вариант соответствует странице самого компаратора. Другого
описания вики не даёт; проверять надо опытом в клиенте, а не по вики.

**Важность:** низкая (пометка, а не ошибка).

---

## Что совпадает

Ниже — то, что сверено построчно и расхождений не дало.

**Редстоун-факел** (`src/redstone.rs:1293-1307`, `:419-425`, `:460`, `:1327`)
* Задержка переключения 2 игровых такта — `TORCH_DELAY = 2` (`:32`).
  Вики: «A redstone torch takes 2 game ticks (0.1 seconds barring lag) to change
  state.»
* Инверсия: горит, пока блок крепления не запитан; гаснет, когда запитан
  (в том числе слабо — проводом на этом блоке). Считается через
  `weak_power(attached)` (`:1298`).
* Не питает блок, к которому прикреплён (`:419-425`).
* Сильно питает блок над собой, слабо — остальных соседей (`:460`,
  `strong_power` ветка `Kind::Torch if dir == Dir::Down`).
  Вики: «A lit redstone torch strongly powers the block above itself and weakly
  powers its other direct neighbor blocks, excluding the one it is attached to.»
* Прикреплённый к непроводящему блоку (стекло, забор, плита, ступени) не гаснет
  никогда (`:1302-1305`).
  Вики: «Walls, fences, glass, slabs, hoppers, and stairs are non-conductive, so
  redstone torches attached to them cannot be deactivated.»
* Выгорание: больше восьми выключений за 60 игровых тактов (`BURNOUT_TOGGLES`,
  `BURNOUT_WINDOW`, `:36-37`); считаются именно выключения (`:1343`).
* Выгоревший оживает от обновления блока, когда окно очистилось (кроме
  оговорки из п. 10).
* Потеряв опору, выпадает предметом (`:921-925`).

**Повторитель** (`src/redstone.rs:786-792`, `:932-965`, `:1358-1386`)
* Задержки 1/2/3/4 → 2/4/6/8 игровых тактов: `repeater_delay` умножает свойство
  `delay` на 2 (`:786-792`). Вики: «delay in redstone ticks (double game
  ticks)», минимум 2 игровых такта.
* Щелчок перебирает задержку по кругу 1→2→3→4→1 (`:1458-1467`).
  Вики: «Each use increases the repeater's delay by two ticks, to a maximum of
  eight, then reverting back to two ticks.»
* Выход всегда 15 (`:427-433`), только вперёд, только с торца; сильно питает
  блок перед собой (`:455-462`).
* `facing` понимается как сторона входа, выход — напротив
  (`output_dir`, `:392-394`). Совпадает с описанием свойства на вики.
* Вход считается со спины: активный источник, провод, другой повторитель или
  сравнитель, смотрящий в нас, а также запитанный проводящий блок
  (`diode_input` → `power_from` с `receiver_is_wire = false`, `:702-707`,
  `:520-545`). Совпадает со списком из «Signal transmission».
* Блокировка: заперт, если сбоку в него смотрит **включённый** повторитель или
  сравнитель (`repeater_locked`, `:723-735`). Вики: «A redstone repeater can be
  "locked" by another powered redstone repeater facing its side» и «A repeater
  can also be locked by a powered redstone comparator facing its side.»
* Запертый повторитель игнорирует и обновления соседей (`:945-947`), и
  запланированный такт (`:1359-1361`). Вики: «A locked repeater completely
  ignores all block updates and scheduled ticks.»
* «Включились, а сигнал уже пропал — выключимся следом» (`:1372-1385`)
  повторяет: «Immediately before turning on, and subsequently whenever it is
  updated, the repeater will check whether it is still receiving a signal, and
  if not, it will schedule a tick to turn off.»
* Одна заявка на такт за раз: `schedule_once` (`src/world/mod.rs:605-612`)
  отбрасывает новую заявку, пока прежняя не прошла. Вики: «only one tick can be
  scheduled at a time, and any attempt to schedule a tick while one already
  exists is completely ignored».
* Пыль тянется к повторителю только вдоль его оси (`wire_connects_to`, `:615`).
* Потеряв опору, выпадает предметом (`:936`).

**Сравнитель** (`src/redstone.rs:742-783`, `:967-985`, `:1388-1401`)
* Задержка 2 игровых такта — `COMPARATOR_DELAY = 2` (`:40`). Вики: «It takes 2
  game ticks for signals to move through a redstone comparator, either from the
  rear or from the sides. This applies to changing signal strengths as well as
  simply to turning on or off.»
* Режим сравнения: выход равен заду, если ни один бок его не превышает, иначе
  0 (`:777-782`). Формула вики: `output = rear × [left ≤ rear AND right ≤ rear]`.
* Режим вычитания: зад минус наибольший бок, не ниже нуля (`:777`,
  `saturating_sub`). Формула вики: `output = max(rear − max(left, right), 0)`.
* Берётся **наибольший** из двух боков (`:772-776`).
* Боковые входы: провод, смотрящий в бок; повторитель; сравнитель; блок
  редстоуна (`:742-760`). Рычаг, кнопка, факел сбоку не считаются — вики:
  «Other power components cannot power the side inputs.»
* Питает только блок перед собой, больше никого (`source_output` `:435-437`,
  `strong_power` `:457-463`). Вики: «The comparator does not power any other
  adjacent blocks.»
* Пыль тянется к сравнителю со всех четырёх сторон (`wire_connects_to`, `:621`).
* Щелчок переключает режим сравнения/вычитания (`:1469-1478`).
* Если сила выхода изменилась, а `powered` остался прежним, соседям всё равно
  сообщают (`:1396-1400`).
* Потеряв опору, выпадает предметом (`:971`).

**Лампа** (`src/redstone.rs:987-995`, `:534-540`)
* Включается мгновенно от любого сигнала со всех шести сторон (`any_power`,
  `:548-552`), включая сверху и снизу. Вики перечисляет ровно это.
* Проводит сигнал как обычный блок (обрабатывается в `power_from` вместе с
  `Kind::Other`, `:534`). Вики: «it conducts redstone power».
* (Выключение — см. расхождение № 1.)

**Кнопка** (`src/redstone.rs:53-54`, `:1442-1456`, `:1403-1407`, `:1536-1538`)
* Каменная держится 20 тактов, деревянная 30. Вики: «A stone button stays on for
  20 ticks (1 second), while a wooden button stays on for 30 ticks (1.5
  seconds).»
* «Каменных» кнопок ровно две — `stone_button` и `polished_blackstone_button`
  (`is_wooden`, `:1536-1538`), остальные считаются деревянными. Совпадает со
  списком кнопок на вики.
* Даёт 15 во все стороны, включая верх и низ, и сильно питает блок крепления
  (`:416`, `:466-471`). Вики: «powers any adjacent redstone dust to power level
  15, including beneath the button … strongly powers its attachment block to
  power level 15».
* При изменении сообщает и соседям блока крепления (`reach_of`, `:892-895`).
  Вики: «provides a redstone update to all redstone components adjacent to
  itself … and to all redstone components adjacent to its attachment block.»
* Повторный щелчок по нажатой кнопке ничего не делает (`:1443-1445`).
* Потеряв опору, выпадает предметом (`:1012-1016`).

**Рычаг** (`src/redstone.rs:1437`, `:416`, `:466-471`)
* Переключается мгновенно, состояние держится до следующего щелчка. Вики: «A
  lever activates immediately … and stays active until the player right-clicks
  it again.»
* Даёт 15 во все стороны, сильно питает блок крепления.
* Крепится к полу, потолку и стене (`attached_to`, `:373-377`) — по свойству
  `face`, как в блок-состояниях на вики.
* Потеряв опору, выпадает предметом.

**Дверь и люк** (`src/redstone.rs:997-1010`, `:1282-1289`, `:1480-1505`)
* Открываются мгновенно по сигналу и закрываются, когда сигнал пропал. Вики:
  «A door activates immediately upon receiving a redstone signal», «When the
  redstone signal stops, the door closes.» То же для люка.
* Дверь открывается целиком: достаточно запитать любую из половин
  (`door_other_half_powered`, `:1282-1289`). Вики (схема): «Weakly powering any
  of the gold blocks opens the door.»
* Слушают слабое питание тоже (`any_power` с `receiver_is_wire = false`).
* Железные дверь и люк рукой не открываются, деревянные открываются
  (`:1481-1483`). Вики: «An iron door can only be opened with a redstone
  signal», «An iron trapdoor can only be opened with a redstone signal.»
  Медные двери и люки у нас попадают в «деревянные», что верно.

**Общее**
* Приоритеты запланированных тактов — значения −3 / −2 / −1 / 0
  (`src/redstone.rs:61-64`) и то, какие из них у повторителя, а какие у
  сравнителя (`:953-959`, `:977-981`), совпадают с текстом вики дословно.
  Неверно определяется только сам случай «смотрит в зад» — расхождение № 6.
* Порядок оповещения соседей — запад, восток, низ, верх, север, юг
  (`NEIGHBOUR_ORDER`, `:79`).
* Сильное и слабое питание разведены так, как описывает вики: сильно питают
  повторитель, сравнитель, факел снизу, рычаг, кнопка, плита; провод питает
  слабо, и слабо запитанный блок провод не зажигает
  (`strong_power` `:445-482`, `weak_power` `:495-508`, `power_from` `:520-545`).

---

## Чего у нас вообще нет

Список составлен по https://minecraft.wiki/w/Redstone_components — по трём её
разделам. В `kind_of` (`src/redstone.rs:235-252`) ни одного из этих имён нет.

**Наблюдатель (observer)** — вики описывает подробно:
«An observer is a block that emits a quick redstone pulse from its output side
whenever its "face" detects that the block, fluid, or air directly in front of
it has changed» и «When it detects something, the observer emits a redstone
pulse of strong power at signal strength 15 for 2 game ticks.» и «{{IN|java}},
an observer detects shape updates coming from the block it's facing.»
Источник: https://minecraft.wiki/w/Observer
У нас — **не сделано**: имени `observer` в `kind_of` нет, импульсов по 2 такта
никто не выдаёт, отслеживания изменений соседнего блока нет.

**Источники сигнала, которых нет:**
* дневной датчик (daylight detector) — и его переключаемый инвертированный режим;
* мишень (target) — сила по точности попадания;
* крюк с растяжкой (tripwire hook) и сама растяжка;
* сундук-ловушка (trapped chest);
* рельсы-детектор (detector rail);
* кафедра (lectern) — сигнал по номеру страницы;
* музыкальный ящик (jukebox) — сигнал по пластинке;
* сенсор скалка (sculk sensor) и калиброванный сенсор (calibrated sculk sensor);
* весовые плиты как работающие источники (см. расхождение № 3).

**Механизмы, которых нет:**
* калитка (fence gate) — см. расхождение № 5;
* раздатчик (dispenser) и выбрасыватель (dropper);
* воронка (hopper) — в том числе её выключение сигналом;
* нотный блок (note block);
* динамит (TNT) — поджиг сигналом;
* рельсы: обычные, ускоряющие (powered rail), активирующие (activator rail);
* медная лампа (copper bulb) — вики описывает её как переключатель;
* крафтер (crafter);
* колокол (bell);
* большой капельник (big dripleaf);
* головы дракона и пиглина;
* командный блок и блок структуры.

**Способности компонентов, которых нет:**
* чтение содержимого блоков сравнителем (см. расхождение № 7);
* обнаружение сущностей нажимными плитами (см. расхождение № 2);
* нажатие кнопки снарядами (см. расхождение № 9).

Поршни у нас есть — они в этот аудит не входили.

---

*При составлении отчёта код Minecraft (в том числе декомпилированный, client.jar,
server.jar, ванильные data pack'и) и код неофициальных серверных ядер и модов
(Paper, Spigot, Fabric, Forge, Bukkit, ViaVersion и любые другие) не
использовались. Единственный источник сведений о поведении игры —
minecraft.wiki.*
