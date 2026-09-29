# Остальные редстоун-компоненты Java 26.1.2 — точное поведение

Справочник для реализации в MCSheriffAnya. Продолжает `redstone-components.md`: там уже есть основа, здесь — уточнения, недостающие цифры и исправления.
Ориентир — **Java Edition 26.1.2**. Всё, что появилось в 26.2 и позже, помечено «**не наше (26.2+)**» и в реализацию не идёт.

**Источники** (только разрешённые): страницы minecraft.wiki (сырой текст `?action=raw`, скачан 2026-09-28), страницы обсуждений на той же вики, таблицы minecraft-data (JSON: `tools/data/blocks.json` — наш снимок 26.1, и `data/pc/26.1/entities.json`). Кода Minecraft, ядер, прокси и протокольных библиотек не открывали.
Страница вики `Java Edition protocol/Entity metadata` описывает **26.3**; номера полей ниже взяты из minecraft-data **26.1**, они совпадают по составу.

Единицы: **такт (gt)** = 1/20 с; «редстоун-такт» = 2 gt. «Сильно питает блок X» = X становится сильно запитанным и питает пыль вокруг себя.

---

## 0. Исправления к `redstone-components.md`

| Где | Было | Как на самом деле | Источник |
|---|---|---|---|
| §6.3 проигрыватель, таблица | «8 — stal, **Bounce**» | Пластинки Bounce в 26.1 **нет**, её добавили в 26.2 (snap7). Для 26.1: 8 — только stal. | [Music Disc Bounce](https://minecraft.wiki/w/Music_Disc_Bounce): «26.2 … snap7 … Added music disc "Bounce"» |
| §4 мишень, список снарядов | жемчуг Края есть | Верно: страница `Target` включает Ender Pearl (с 20w10a). Страница `Redstone components` пишет обратное («excluding ender pearls») — она устарела. | [Target](https://minecraft.wiki/w/Target): «20w10a — Targets can now be activated by ender pearls» |
| §10 рельсы | «до 17 рельс» | Верно. Формулировка «9 рельс» на `Redstone components` означает «исходная + 8 в одну сторону». | [Powered Rail](https://minecraft.wiki/w/Powered_Rail): «up to 17 rails (one original in the middle and eight on either side)» |
| §5 датчик света | «точная формула на вики не опубликована» | Формула есть на странице обсуждения и **сходится с таблицей вики** на 23 996 тактах из 24 000 (см. §2 ниже). | [Talk:Daylight Detector](https://minecraft.wiki/w/Talk:Daylight_Detector) |
| §13 вибрации | — | Частота `minecraft:bounce` = 2, «potent sulfur» в `block_activate`, таяние льда/снега — всё это 26.2/26.3, **не наше**. | [Vibration](https://minecraft.wiki/w/Vibration), история: «26.2 snap1 — Added minecraft:bounce» |

---

## 1. Мишень (target)

Страница: [Target](https://minecraft.wiki/w/Target), [Redstone components#Target](https://minecraft.wiki/w/Redstone_components#Target).
minecraft-data: `target` id 910, состояния 21752–21767, свойство `power` 0..15, по умолчанию `power=0`.

**Какие снаряды включают** (Java, дословный список вики): стрела, склянка опыта, огненный шар дракона, яйцо, жемчуг Края, огненный шар гаста, ракета фейерверка (в том числе та, что разгоняет летящего на элитрах игрока), поплавок удочки, задерживающееся и взрывное зелье, плевок ламы, снаряд шалкера, снежок, малый огненный шар, трезубец, заряд ветра, череп иссушителя.

**Длительность:**
> «When struck by most projectiles, the target emits redstone power for 8 gt. Arrows and tridents instead cause the target to emit power for 20 gt»

То есть стрела/трезубец — **20 gt**, остальное — **8 gt**. Реализуется как запланированный такт блока, который сбрасывает `power` в 0.

**Сила сигнала.**
> «The strength of the signal depends on how close the projectile is to the center of the block, from 1 to 15.»

Точной формулы на вики и в обсуждениях нет (страница обсуждения `Talk:Target` про формулу молчит). Наша рабочая гипотеза, **требует замера**:
- берём точку попадания на грани, считаем смещения от центра грани по двум осям этой грани `du, dv ∈ [0, 0.5]`;
- `d = max(du, dv)` (кольца-квадраты — так выглядит мишень и так её описывают игроки);
- `power = clamp(ceil(15 * (0.5 - d) / 0.5), 1, 15)`.

Как замерить: в чёрном ящике `/summon arrow x y z {Motion:[...]}` строго перпендикулярно грани, сетка точек с шагом 1/32 по u и v, читать `power` из состояния блока. Это даст точную таблицу, и гипотезу можно будет заменить.

**Состояние блока меняется** — Java: «the target changes its `power` state when hit, meaning that an observer can detect» (в Bedrock — нет).

**Куда идёт сигнал.** Мишень — источник питания, «redirects adjacent redstone wires to point to itself» (притягивает пыль со всех четырёх сторон). Мишень при срабатывании:
- питает соседнюю пыль, повторители и компараторы, смотрящие от неё, силой `power`;
- активирует соседние механизмы;
- **не** запитывает соседние блоки («does not power any blocks when activated»).

**Проводимость.** Мишень — проводящий (conductive) блок с 20w18a: «Targets now conduct redstone power». Если её запитать снаружи, она «powers adjacent redstone components, except dust and other targets». Компаратор читает через неё контейнер, как через обычный сплошной блок.

**Прочее:** горит (Java, ignite 15 / burn 20 по таблице [Fire](https://minecraft.wiki/w/Fire)); быстрее всего ломается мотыгой.

---

## 2. Датчик дневного света (daylight_detector)

Страницы: [Daylight Detector](https://minecraft.wiki/w/Daylight_Detector), [Light#Internal sky light](https://minecraft.wiki/w/Light#Internal_sky_light), [Daylight cycle#Sky angle](https://minecraft.wiki/w/Daylight_cycle), [Talk:Daylight Detector](https://minecraft.wiki/w/Talk:Daylight_Detector).
minecraft-data: id 474, состояния 11279–11310, `inverted` (true/false), `power` 0..15; по умолчанию `inverted=false, power=0`. Есть блок-сущность без данных — нужна только, чтобы блок тикал.

**Период обновления.**
> «The output signal of daylight detectors in the loaded chunk is updated every 20 gametime (Java) … Block updates have no effect on daylight detectors.»

Значит: пересчёт в тактах, где `gametime % 20 == 0` (время мира, не время суток), и ещё один раз при установке (страница `Redstone components`: «updates its output 20 game ticks … after being placed»). Обновления соседей выход не пересчитывают.

**Переключение:** ПКМ по датчику: `inverted = !inverted` и сразу пересчёт выхода.

**Выход:** питает пыль (и под собой), повторители (силой 15) и компараторы, смотрящие от датчика; активирует механизмы, в том числе сверху и снизу; **не** запитывает блоки.

### 2.1 Обычный режим — формула

Со страницы обсуждения (2014, описание наблюдений):
> «The output of the daylight sensor is round( (SL - D) * TOD ), where values over 15 or below 0 are taken as 15 or 0 respectively.»
> «TOD: … cos( 2π * (t + 0.2 * (t < .5 ? -t : 1 - t) ) ) where t ranges from 0 to 1 over the course of the day (roughly t = (T-6000)/24000 …, although the real function has a bit of a curve to it)»
> «D: The current sky "darkening" level, i.e. 15 minus the current brightness of the sky light.»

«Кривизну» `t` даёт формула угла неба со страницы `Daylight cycle`:
> α = (1 − cos(π·frac((T−6000)/24000)) + 4·frac((T−6000)/24000)) · 60°, «with 0° being noon»

Итог для реализации:
```
f   = frac((T - 6000) / 24000)          // T — время суток 0..23999
t   = (1 - cos(pi*f) + 4*f) / 6         // = α/360, 0 = полдень
t2  = t + 0.2 * (t < 0.5 ? -t : 1 - t)
TOD = cos(2*pi*t2)
Lin = внутренний небесный свет при небесном свете 15 (таблица ниже, зависит от T и погоды)
base = Lin - (15 - SL)                  // SL — небесный свет в клетке датчика
power = clamp(round(base * TOD), 0, 15)
```
**Проверка:** я прогнал эту формулу по всем 24 000 тактам против таблицы вики «Daylight detector power levels» (ясно и дождь, SL=15). Совпало всё, кроме 4 тактов в ясную погоду (1865, 2444, 8825, 11834) и 2 в дождь (394, 11118) — каждый раз граница сдвинута на 1 такт, это погрешность самой таблицы или округления. Скрипт: `~/.cache/mcsheriffanya/work/wiki/ddcheck.py`.

**Внутренний небесный свет `Lin`** (вики `Light`, при SL = 15; для SL<15 вычитается `15 − SL`, может уйти в минус):

| Lin | ясно, утро | ясно, вечер | дождь/снег, утро | дождь/снег, вечер | гроза, утро | гроза, вечер |
|---|---|---|---|---|---|---|
| 4 | 13670–22330 (ночь) | — | 13670–22330 | — | 13670–22330 | — |
| 5 | 22331–22491 | 13509–13669 | 22331–22565 | 13436–13669 | 22331–22671 | 13330–13669 |
| 6 | 22492–22652 | 13348–13508 | 22566–22798 | 13203–13435 | 22672–23010 | 12990–13329 |
| 7 | 22653–22812 | 13188–13347 | 22799–23031 | 12969–13202 | 23011–23352 | 12648–12989 |
| 8 | 22813–22973 | 13027–13187 | 23032–23266 | 12734–12968 | 23353–23700 | 12300–12647 |
| 9 | 22974–23134 | 12867–13026 | 23267–23504 | 12497–12733 | 23701–59 | 11941–12299 |
| 10 | 23135–23296 | 12705–12866 | 23505–23745 | 12256–12496 | 60–11940 (день) | — |
| 11 | 23297–23459 | 12542–12704 | 23746–23991 | 12010–12255 | — | — |
| 12 | 23460–23623 | 12377–12541 | 23992–12009 (день) | — | — | — |
| 13 | 23624–23790 | 12210–12376 | — | — | — | — |
| 14 | 23791–23960 | 12041–12209 | — | — | — | — |
| 15 | 23961–12040 (день) | — | — | — | — | — |

Вики: «noon, clear 15; rain or snowfall 12; thunderstorm 10; midnight, any weather 4».

**В 26.1 это атрибут окружения.** `gameplay/sky_light_level` — «The internal sky light level … This affects mechanics such as monster spawning, daylight detector power, and ice melting» ([Environment attribute](https://minecraft.wiki/w/Environment_attribute)); он задаётся линией времени суток (у нас разобрано в `timelines.md`, §4.3). Погода — «not data-driven», то есть снижение до 12/10 делается кодом поверх линии времени. Для реализации: `Lin` = значение атрибута в такте `T` с учётом погоды, округлённое до целого; таблица выше — эталон для проверки.

**Готовая таблица выхода при SL=15** (вики, ясно): 0 — 13670–22330; 1 — 22331–22781 / 13219–13669; 2 — 22782–23069 / 12931–13218; 3 — 23070–23296 / 12705–12930; 4 — 23297–23529 / 12471–12704; 5 — 23530–23767 / 12233–12470; 6 — 23768–23960 / 12041–12232; 7 — 23961–166 / 11835–12040; 8 — 167–535 / 11465–11834; 9 — 536–933 / 11067–11464; 10 — 934–1371 / 10629–11066; 11 — 1372–1865 / 10136–10628; 12 — 1866–2444 / 9557–10135; 13 — 2445–3175 / 8826–9556; 14 — 3176–4294 / 7706–8825; 15 — 4295–7705. Дождь — максимум 12 (4109–7891), гроза — максимум 10 (3943–8058). Полные столбцы дождя и грозы — в `Daylight Detector` (раздел «Daylight detector»).

**Тень.** По формуле тень (SL<15) уменьшает `base`. Обсуждение 2014 года отмечает странность: при 0<SL<11 ночью `base` и `TOD` оба отрицательны, и выход положительный («A sensor with SL=5 will give a signal with strength 5 at both noon … and midnight»). Это описание старой версии — **замерить** в 26.1.2 (датчик под плитой/водой, SL 5 и 10, полночь).

### 2.2 Инвертированный режим

> «it outputs a signal strength of 15 minus the current internal sky light level, where values over 15 or below 0 are taken as 15 or 0 respectively»
> «The effects of shade are applied before inverting, so shade increases the signal strength»

```
power = clamp(15 - (Lin - (15 - SL)), 0, 15)
```
Ясная полночь под открытым небом → 11; полностью закрытый датчик → 15 всегда.

### 2.3 Измерения
- Незер: обычный — всегда 0, инвертированный — всегда 15 (Java).
- Край: обычный — всегда 15 («the End's internal sky light level never changes»), инвертированный работает как в Верхнем мире (Java).
- Датчик 0.375 блока высотой; вода и лава его обтекают; поршнями в Java **не** двигается (движение поршнем помечено «only bedrock»).

---

## 3. Растяжка (tripwire) и крючок (tripwire_hook)

Страницы: [String](https://minecraft.wiki/w/String) (раздел Redstone component), [Tripwire Hook](https://minecraft.wiki/w/Tripwire_Hook), [Redstone components#Tripwire hook](https://minecraft.wiki/w/Redstone_components#Tripwire_hook).

**Состояния (minecraft-data 26.1):**
- `tripwire` id 402, 9599–9726: `attached`, `disarmed`, `east`, `north`, `powered`, `south`, `west` (все булевы). По умолчанию (9726) всё `false`.
- `tripwire_hook` id 401, 9583–9598: `attached`, `facing` (north/south/west/east), `powered`. По умолчанию (9592) `attached=false, facing=north, powered=false`. «a tripwire hook facing north is attached to a block to its south».

**Цепь.** «a straight line of blocks consisting of a block with a tripwire hook attached to it, a "tripwire line" (1 to 40 blocks of tripwire), and a second tripwire hook attached to another block, with both tripwire hooks facing the tripwire.» Максимум **40** растяжек, крючок ищет пару не дальше 41 блока.

**Хитбокс растяжки:** в исправной цепи «0.09375 or 3/32 blocks high, with the bottom 0.0625 or 1/16 blocks above the block below», вне цепи — «0.5 blocks high, bottom not raised». С 1.21.9 (25w31a) область срабатывания чуть уменьшили: «entities can no longer trigger tripwire by exactly touching its former detection boundary» — то есть касание границы ровно не считается, нужно строгое пересечение.

**Кто срабатывает.** Любая сущность, пересекающая хитбокс растяжки, **кроме**: наблюдателей (spectator), облаков эффектов; в Java ещё брошенных глаз Края, дракона Края, маркеров и предметов, вдавленных в сплошной блок. Вексы срабатывают. Сам крючок сущности не нажимают.

**Алгоритм (Java), дословно по вики `String`:**
1. «When any entity activates an inactive tripwire, the tripwire first updates its immediate neighbors, and then searches to the south and west looking for tripwire hooks to signal to update (no search is done toward the east or north).» Поиск удачен, если крючок ближе 41 блока, смотрит на эту растяжку и связан непрерывной линией.
2. «the activated tripwire schedules a block tick at its own position to be processed 10 game ticks later». На этом такте: есть сущности — остаётся `powered` и планирует ещё через 10 gt; нет — выключается и рассылает те же обновления (соседи + «юг и запад»).
3. Те же обновления — при разрушении растяжки (в том числе водой).
4. Крючок, получивший сигнал, проверяет цепь и включается, «in much the same way a button does».

**Выход крючка (активного):** пыль вокруг (и под ним) — 15; повторители и компараторы, смотрящие от крючка, — 15; **сильно** питает блок, к которому прикреплён; активирует механизмы, в том числе сверху и снизу. Сама растяжка питания не даёт, хотя её состояние называется `powered`.

**Длительность.** Всегда кратна 10 gt: «When an entity moves off the tripwire, the tripwire hook waits until the next multiple of 10 game ticks» (пример: ушёл через 25 gt — ждёт ещё 5).

**Разрушение:**
- рукой/инструментом (не ножницами): крючок даёт импульс «for 5 redstone ticks (0.5 seconds)» = **10 gt**;
- **ножницами**: импульса нет, но у крючка `attached` меняется с `true` на `false` — видит наблюдатель. Механика 26.1: при стрижке растяжка сначала получает `disarmed=true` (без обновлений цепи), затем ломается. История: «1.21.2 (24w33a) — Tripwire again breaks when being sheared»; «disarmed … True if the tripwire is disarmed, that is, broken using shears»;
- разрушение крючка или его опоры импульса **не** даёт;
- вода сносит растяжку с выпадением нити; лава — без выпадения; поршень ломает растяжку (нить выпадает).

**Звуки (Java, категория block, дальность 16):**

| Событие | id | громкость | высота |
|---|---|---|---|
| цепь собрана | `block.tripwire.attach` | 0.4 | 0.7 |
| цепь активирована | `block.tripwire.click_on` | 0.4 | 0.6 |
| цепь выключена | `block.tripwire.click_off` | 0.4 | 0.5 |
| крючок в цепи разрушен | `block.tripwire.detach` | 0.4 | 12/11–4/3 |

Крючок с 1.20.2 — звуки дерева.

**Вибрации:** подключение к крючку — `block_attach` (10), отключение — `block_detach` (9).

---

## 4. Редстоун-руда и глубинносланцевая редстоун-руда

Страница: [Redstone Ore](https://minecraft.wiki/w/Redstone_Ore).
minecraft-data: `redstone_ore` id 271, 6881 (`lit=true`) / 6882 (`lit=false`, по умолчанию); `deepslate_redstone_ore` id 272, 6883/6884.

**Когда загорается** (дословно):
> «Redstone ore enters this state instantly when a player interacts with it by pressing either the break or the use control, or when an entity touches it on the top side. There are some exceptions, including sneaking players, projectiles, shulkers, and the ender dragon.»

Итого загорается от:
- удара игрока (начало ломания, ЛКМ);
- ПКМ игрока (с блоком в руке руда тоже загорается **и** блок пытается поставиться; Java: если поставить не удалось, руда горит, а рука не машет);
- сущности, **стоящей/идущей** на верхней грани.

**Не загорается** от: крадущегося игрока, **снарядов** (стрелы, снежки и т.п. — пункт задания про снаряды вики опровергает), шалкеров, дракона Края.

**Гаснет** на случайном такте (в среднем через 68.27 с при randomTickSpeed=3). Повторные взаимодействия в горящем состоянии дают лишь новую порцию частиц.

**Свет** 9, частицы красной пыли. **Питания не даёт**: «This effect does not actually produce redstone power, but the block changes can be detected with an observer.»

---

## 5. Громоотвод (lightning_rod) и его окисленные варианты

Страницы: [Lightning Rod](https://minecraft.wiki/w/Lightning_Rod), [Redstone components#Lightning rod](https://minecraft.wiki/w/Redstone_components#Lightning_rod).

**В 26.1 громоотвод окисляется** (1.21.9, 25w31a). minecraft-data 26.1 — 8 блоков: `lightning_rod` (1097, 27543–27566), `exposed_` (1098), `weathered_` (1099), `oxidized_` (1100), `waxed_` + те же четыре (1101–1104, до 27734). У всех свойства `facing` (north/east/south/west/up/down), `powered`, `waterlogged`; по умолчанию `facing=up, powered=false, waterlogged=false`. Молния и топор снимают окисление, соты — воск.

**Сигнал:**
> «A lightning rod emits a redstone pulse, and strongly powers the block it's attached to, at level 15 for 8 game ticks (0.4 seconds) when struck by lightning.»

- `powered=true` на 8 gt, затем запланированный такт выключает;
- питает пыль, повторители и компараторы, смотрящие от него, силой 15; активирует механизмы;
- **сильно** питает блок, к которому прикреплён (он напротив `facing`).

**Перехват молний:** громоотвод, над которым в Java нет ни одного блока, кроме воздуха, перехватывает молнию в шаре радиуса **128** (Java); из нескольких выбирается ближайший к исходной точке удара. Трезубец с «Громовержцем» в грозу тоже бьёт в громоотвод. Не перехватывает молнию от трезубца в сущность и от команд.

**Огонь от удара (Java):** молния пытается поджечь объём 3×3×3 вокруг воздуха над громоотводом, с обычными условиями появления огня; сам громоотвод и блок под ним не горят. Урон мобам — объём 6×12×6 с центром на 4 блока выше низа громоотвода.

Вибрация удара молнии — `lightning_strike` (14). Звук при ударе трезубцем — `item.trident.thunder`, громкость 5.0, категория weather.

---

## 6. Колокол (bell)

Страницы: [Bell](https://minecraft.wiki/w/Bell), [Redstone components#Bell](https://minecraft.wiki/w/Redstone_components#Bell), [Java Edition protocol/Block actions](https://minecraft.wiki/w/Java_Edition_protocol/Block_actions).
minecraft-data: id 848, 20805–20836, `attachment` (floor/ceiling/single_wall/double_wall), `facing`, `powered`; по умолчанию `floor, north, powered=false`.

**Редстоун (Java):**
> «bells ring one time upon receiving a redstone signal. Observers can detect when a bell receives a redstone signal, and when it stops receiving a signal (corresponding to the powered block state). Observers cannot detect when a bell is activated by the player or other entities.»
> «A bell activates in the same game tick that it receives a redstone signal … The bell does not ring again until the redstone signal stops, and a new signal is supplied.»

То есть звонит **по фронту** (`powered` false→true), в том же такте; на спаде только меняется `powered`. Колокол **прозрачный, не проводит** питание (Java).

**Чем ещё звонит:** ПКМ по боковой стороне (не под любым углом — «Bells can be rung by players only from certain angles»), любой снаряд. Поршень колокол в Java **ломает** (не двигает).

**Пакеты.** Анимация — Block Action: «There is only one action: 1 … this is the direction from which the bell was rung (down=0, up=1, north=2, south=3, west=4, east=5). If the bell was rung by a player, this is the face of the block they clicked on; otherwise, it is based on the bell's facing state.» Звук — отдельным Sound Effect: `block.bell.use`, категория block. Вики указывает громкость 24.0 (?) и высоту 0.93/0.95/0.97 — громкость **проверить** перехватом пакетов в чёрном ящике.

**Рейдеры:** «After 5 ticks, if there are raiders nearby, the bell resonates and 40 ticks later causes them to glow for 60 ticks» (звук `block.bell.resonate`). Можно отложить.

Вибрация звона — `block_change` (11). Установка/ломание — 13/12.

---

## 7. Музыкальный блок и головы сверху

Страница: [Note Block](https://minecraft.wiki/w/Note_Block) (у нас есть копия `tools/data/note_block.wiki`).
minecraft-data: id 109, 581–1930, `instrument` (27 значений), `note` 0..24, `powered`; по умолчанию `harp, note=0, powered=false`. Порядок значений `instrument` в 26.1: harp, basedrum, snare, hat, bass, flute, bell, guitar, chime, xylophone, iron_xylophone, cow_bell, didgeridoo, bit, banjo, pling, **trumpet, trumpet_exposed, trumpet_oxidized, trumpet_weathered**, zombie, skeleton, creeper, dragon, wither_skeleton, piglin, custom_head.

**Новое в 26.1 (snap7): трубы.** «Note blocks placed on Copper Blocks, Chiseled Copper, Cut Copper, Cut Copper Stairs, and Cut Copper Slabs now play trumpet sounds instead of harp.» Стадия окисления блока под музыкальным блоком (вощёный или нет — неважно) даёт `trumpet` / `trumpet_exposed` / `trumpet_weathered` / `trumpet_oxidized`. Диапазон окисленной трубы F♯2–F♯4. Звуки `block.note_block.trumpet[_exposed|_weathered|_oxidized]`.

**Головы.** «Mob head instruments take priority over block instruments, and play the corresponding mob's ambient sound, except for creeper heads, which instead trigger the primed sound. Mob head sounds are not affected by the note block's pitch.»

| Голова сверху | `instrument` | звук |
|---|---|---|
| череп скелета | `skeleton` | `block.note_block.imitate.skeleton` |
| череп скелета-иссушителя | `wither_skeleton` | `block.note_block.imitate.wither_skeleton` |
| голова зомби | `zombie` | `block.note_block.imitate.zombie` |
| голова крипера | `creeper` | `block.note_block.imitate.creeper` |
| голова пиглина | `piglin` | `block.note_block.imitate.piglin` |
| голова дракона | `dragon` | `block.note_block.imitate.ender_dragon` |
| голова игрока | `custom_head` | из компонента `note_block_sound` головы, по умолчанию тишина |

Звук головы: категория record, громкость 3.0, высота 1.0 (страница `Zombie Head`). Частица ноты для голов **не** появляется, для остальных — появляется (цвет по `note`).

**Когда играет:** удар игрока в выживании, ПКМ (`note+1`, после 24 → 0), **фронт** редстоун-сигнала. Играет, только если сверху воздух или голова. Играние — Block Action (id и параметр игнорируются, всё берётся из состояния). Высота `2^((note−12)/12)`. Игроки слышат на 48 блоках, варден/тихоня — 16, датчик скалка — 8. Вибрация — `note_block_play` (10). Блок проводит питание (сплошной).

---

## 8. Динамит (tnt) и взрыв

Страницы: [TNT](https://minecraft.wiki/w/TNT), [Explosion](https://minecraft.wiki/w/Explosion), [Minecart with TNT](https://minecraft.wiki/w/Minecart_with_TNT), [Game rule](https://minecraft.wiki/w/Game_rule).

### 8.1 Блок
minecraft-data: id 177, 2341 (`unstable=true`) / 2342 (`unstable=false`, по умолчанию). `unstable=true`: «Hitting the TNT block causes it to ignite and then explode».

**Чем поджигается (Java):**
- огниво или огненный заряд (ПКМ). **Крадущийся** игрок вместо этого ставит огонь рядом;
- редстоун-сигнал (TNT — механизм, **непроводящий**; соседние TNT сигналом не поджигает, только взрывом). Включается и при установке рядом с уже работающим источником питания (так ведёт себя любой механизм при установке — **замерить**, в тот же такт или нет);
- горящие снаряды (стрела с огнём, огненные шары, черепа иссушителя) — «TNT can now be ignited by any burning projectile» (20w11a);
- другой взрыв;
- огонь, дошедший до блока: «the block burns for several seconds before activating»; шансы огня у TNT — ignite 15 / burn 100 ([Fire](https://minecraft.wiki/w/Fire)), вместо сгорания поджигается;
- раздатчик, выставляющий TNT, ставит сразу подожжённый.

Правило `tnt_explodes` (26.1, snake_case): «Whether TNT explodes after activation»; при `false` не поджигаются и вагонетки с TNT.

### 8.2 Подожжённый TNT (сущность)
minecraft-data 26.1: сущность `tnt`, **id 132**, 0.98×0.98.

- Появляется в `(x+0.5, y+0.0, z+0.5)` от угла блока.
- Начальная скорость: «0.2 blocks per tick upward, and 0.02 blocks per tick in a random direction» (по горизонтали).
- Запал: **80 gt** при поджоге огнём/редстоуном/командой; **случайно 10–30 gt** при поджоге взрывом.
- «The timer decreases by 1 every game tick, and the Primed TNT explodes when it reaches 0.»
- Подвержен гравитации, сносится водой/лавой, толкается поршнями и взрывами; проходит через порталы.
- Звук поджога: `entity.tnt.primed`, 1.0/1.0, категория — «Friendly Mobs» (neutral).

**Метаданные (minecraft-data 26.1, `metadataKeys`):**

| индекс | имя | тип (вики) | по умолчанию |
|---|---|---|---|
| 0–7 | общие поля Entity (флаги, воздух, имя, видимость имени, тишина, без гравитации, поза, заморозка) | | |
| 8 | `fuse` | VarInt — «Automatically decreases by 1 each client tick» | 80 |
| 9 | `block_state` | Block State — «The block state ID that is exploding» | TNT |

NBT: `fuse` (short), `block_state`, `explosion_power` (float 0..128, по умолчанию 4, «If set to the default value, this field is not saved»), `owner`.

### 8.3 Взрыв
- Центр — «0.06125 blocks above the entity's position». Мощность TNT — **4**.
- **Какие блоки рушатся** (дословно, Java):
  > «"rays" are created from the explosion center to the outer points of a 16 by 16 by 16 grid. Each of those rays gets an intensity of power · (random value from 0.7 to 1.3).»
  1. проверить блок в текущей точке (форма блока игнорируется);
  2. если не воздух — `intensity -= (blast_resistance + 0.3) * 0.3`;
  3. если `intensity > 0` и блок можно взорвать — в список;
  4. сдвинуться на 0.3 блока вдоль луча;
  5. `intensity -= 0.22500001`;
  6. пока `intensity > 0` — повторять.

  Лучи — к точкам на **поверхности** куба 16×16×16 (1352 направления). Затем список перемешивается; огненные взрывы пытаются поставить огонь в 1/3 блоков (только над непрозрачным).
- Жидкость в центре гасит слабые взрывы: «explosions happening inside a non-full block get dampened heavily».
- **Выпадение:** «Blocks destroyed by TNT have a 100% chance of dropping. Other explosions have a 1/power chance.» Управляется правилами `block_explosion_drop_decay`, `mob_explosion_drop_decay`, `tnt_explosion_drop_decay` (последнее по умолчанию `false`).
- **Сущности:** доля открытости `exposure` (лучи к сетке точек в хитбоксе), `impact = (1 − distance/(2·power))·exposure`, урон `7·power·(impact² + impact) + 1`; скорость `(1 − distance/(2·power))·exposure·knockback·(1 − explosion_knockback_resistance)`, направление — к глазам, **у TNT — к ногам**. Радиус действия на сущности — `2·power` = 8.
- Вибрация — `explode` (15). Звук — `entity.generic.explode`, громкость 4.0, высота 0.56–0.84.

**Вагонетка с TNT:** «base power of 4 … adds a random bonus value up to 1.5 times velocity, but no higher than 7.5». От активирующей рельсы — через 80 gt; от урона/огня — случайная задержка 0–1.95 с; мгновенно — при падении больше 3 блоков не на рельсы, от горящего снаряда и т.д.

---

## 9. Датчик скалка и калиброванный датчик скалка

Страницы: [Sculk Sensor](https://minecraft.wiki/w/Sculk_Sensor), [Calibrated Sculk Sensor](https://minecraft.wiki/w/Calibrated_Sculk_Sensor), [Vibration](https://minecraft.wiki/w/Vibration), [Redstone components#Sculk sensor](https://minecraft.wiki/w/Redstone_components#Sculk_sensor), [Redstone Comparator](https://minecraft.wiki/w/Redstone_Comparator).

**Состояния (minecraft-data 26.1):**
- `sculk_sensor` id 1001, 24690–24785: `power` 0..15, `sculk_sensor_phase` (inactive/active/cooldown), `waterlogged`. По умолчанию `power=0, inactive, waterlogged=false`.
- `calibrated_sculk_sensor` id 1002, 24786–25169: плюс `facing` (north/south/west/east).

Свет 1 (и в активном состоянии тоже). Поршнями не двигаются. Под водой — без звуков, но работают.

**Цифры:**

| | обычный | калиброванный |
|---|---|---|
| радиус | 8 (шар) | 16 |
| активен (`active`) | **30 gt** | **10 gt** |
| затем `cooldown` | 10 gt | 10 gt |
| после установки | 10 gt кулдауна | 10 gt |
| фильтр по частоте | нет | сигнал в грань с аметистом |

> «When a vibration is made within the range of a sculk sensor, a signal travels from the vibration source to the sensor at a speed of one block per game tick … The sensor cannot detect any other vibrations while activated or while a signal is traveling to it.»

То есть задержка между событием и срабатыванием ≈ расстояние в блоках (в тактах); в блок-сущности это `listener.event_delay`. Калиброванный: «can detect a vibration once every 1.05 seconds … (Plus 0.05 for every block of distance from the vibration past the first.)».

**Сила выхода (в пыль/повторители):**
```
redstone_strength = max(1, 15 - floor(15 / detection_range * distance))
```
«The distance is measured from the block at which the vibration occurred; moving within a block never results in a different strength.» `detection_range` = 8 или 16.

**Компаратор** читает **частоту** последней вибрации («depending on the type of vibration that is detected»); хранится в `last_vibration_frequency`.

**Куда питает:** во все стороны (включая верх и низ) как сильно запитанный блок; блок **снизу** запитан сильно. Калиброванный — во все стороны, **кроме грани с аметистом**.

**Фильтр калиброванного:** сигнал силы N в грань с аметистом → ловит только частоту N; без сигнала — «detects the highest frequency» из одновременных.

**Кто не слышен:** крадущийся игрок для частот 1–3 (шаги, приземление, взаимодействие с предметами, выстрел — но попадание снаряда слышно); всё, что связано с шерстью и коврами; вибрации от самих скалковых блоков и вардена (Java). **Шерсть** на линии между центрами блоков источника и датчика глушит вибрацию. Соседний блок аметиста повторяет пойманную частоту (`resonate_N`).

**Таблица частот 26.1** (вики `Vibration`, без добавлений 26.2+):

| Частота | События (game event) |
|---|---|
| 1 | `step`, `swim`, `flap`, `resonate_1` |
| 2 | `projectile_land`, `hit_ground`, `splash`, `resonate_2` (`bounce` — **не наше, 26.2**) |
| 3 | `item_interact_finish`, `projectile_shoot`, `instrument_play`, `resonate_3` |
| 4 | `entity_action`, `elytra_glide`, `unequip`, `resonate_4` |
| 5 | `entity_dismount`, `equip`, `resonate_5` |
| 6 | `entity_mount`, `entity_interact`, `shear`, `resonate_6` |
| 7 | `entity_damage`, `resonate_7` |
| 8 | `drink`, `eat`, `resonate_8` |
| 9 | `container_close`, `block_close`, `block_deactivate`, `block_detach`, `resonate_9` |
| 10 | `container_open`, `block_open`, `block_activate`, `block_attach`, `prime_fuse`, `note_block_play`, `resonate_10` |
| 11 | `block_change`, `resonate_11` |
| 12 | `block_destroy`, `fluid_pickup`, `resonate_12` |
| 13 | `block_place`, `fluid_place`, `resonate_13` |
| 14 | `entity_place`, `lightning_strike`, `teleport`, `resonate_14` |
| 15 | `entity_die`, `explode`, `resonate_15` |

**Что какое событие даёт для наших механизмов** (по описаниям в таблице):
- поршень выдвигается / кнопка, рычаг, нажимная плита включаются → `block_activate` (10); поршень втягивается / выключаются → `block_deactivate` (9). Раздатчик, выбрасыватель, крафтер и проигрыватель `block_activate` **не** дают;
- двери, люки, калитки: открыть — `block_open` (10), закрыть — `block_close` (9);
- смена состояния кафедры, компостера, торта, звон колокола, рамки и т.п. → `block_change` (11);
- ломание блока, в том числе поршнем, если блок при сдвиге ломается, и снятие опоры → `block_destroy` (12); **не** при взрыве;
- установка → `block_place` (13); жидкость ведром → `fluid_place` (13);
- поджог TNT или крипера → `prime_fuse` (10);
- растяжка присоединена/отсоединена → 10/9.

**Не дают вибраций** (баги, признанные в вики): блоки, снесённые текущей жидкостью; рельсы, пыль, повторители, компараторы, отвалившиеся без опоры; растекание/высыхание жидкостей; смена режима компаратора, задержки повторителя, формы одиночной пыли; огонь, погашенный дождём; ряд неудач раздатчика; крик скалкового крикуна.

**Звуки:** `block.sculk_sensor.clicking` (0.73, 0.8–1.0) при срабатывании, `block.sculk_sensor.clicking_stop` (0.62, 0.8–1.0) при выключении; аметист — `block.amethyst_block.resonate`.

---

## 10. Кафедра (lectern)

Страницы: [Lectern](https://minecraft.wiki/w/Lectern), [Redstone components#Lectern](https://minecraft.wiki/w/Redstone_components#Lectern).
minecraft-data: id 845, 20784–20799, `facing`, `has_book`, `powered`; по умолчанию `north, has_book=false, powered=false`.

**Импульс:** «Lecterns holding a book emit a full-strength redstone pulse that is 2 game ticks long when a page is turned.» Пока идёт импульс:
- **сильно** питает блок **под собой** («powers the block beneath it»);
- активирует соседние механизмы;
- питает пыль и повторители, смотрящие от кафедры, силой 15.

**Компаратор (Java)** — всегда, не только во время импульса:
```
S = floor(1 + 14 * (P - 1) / (M - 1))     // M >= 2; при M = 1 → 15; без книги → 0
```
Таблица на вики (`M` 1..15) совпадает с формулой; книга из 15 страниц даёт шаг 1.

Книга кладётся ПКМ с книгой-с-пером или подписанной книгой (обычная и зачарованная книга — нет); забирается в интерфейсе (Java). Смена страницы → вибрация `block_change` (11). Горит (ignite 30, burn 20).

---

## 11. Проигрыватель (jukebox)

Страницы: [Jukebox](https://minecraft.wiki/w/Jukebox), [Redstone components#Jukebox](https://minecraft.wiki/w/Redstone_components#Jukebox), [Jukebox song definition](https://minecraft.wiki/w/Jukebox_song_definition).
minecraft-data: id 283, 6963 (`has_record=true`) / 6964 (`has_record=false`, по умолчанию).

**В 26.1 проигрыватель — источник питания:**
> «A redstone signal of strength 15 is emitted while music from a disc is playing, powering any adjacent redstone wire, redstone repeaters, and mechanism components. The signal turns off when the music track ends or if the disc is removed before the track ends. Being a power source, a jukebox redirects adjacent redstone wires to point to itself.»
> «A jukebox does not power any blocks.»

Длительность «игры» = `length_in_seconds` песни из реестра `jukebox_song`: «Determines the length of the redstone output of the jukebox». В блок-сущности — `ticks_since_song_started` (с 24w21a).

**Компаратор** — `comparator_output` песни, пока пластинка внутри, даже после окончания. Для 26.1 (21 пластинка в `items.json`):

| сила | пластинки | сила | пластинки |
|---|---|---|---|
| 0 | нет пластинки | 8 | stal |
| 1 | 13 | 9 | strad, Lava Chicken |
| 2 | cat | 10 | ward, Tears |
| 3 | blocks | 11 | 11, Creator (Music Box) |
| 4 | chirp | 12 | wait, Creator |
| 5 | far | 13 | Pigstep, Precipice |
| 6 | mall | 14 | otherside, Relic |
| 7 | mellohi | 15 | 5 |

Проигрыватель запирает соседнюю воронку, пока играет (он её «питает»); по окончании воронка под ним вытягивает пластинку. Поршнями не двигается. Вставка пластинки не даёт `block_activate`.

**Пакет:** World Event 1010 «Play record», 1011 «Stop record». Вики пишет про 1010 «An ID in the minecraft:item registry», а страница `Jukebox song definition` — что песни передаются «as part of the jukebox update event». Что именно кладётся в данные 1010 в 26.1 — сверить агентом protocol-checker или чёрным ящиком.

---

## 12. Рельсы и вагонетки

Страницы: [Rail](https://minecraft.wiki/w/Rail), [Powered Rail](https://minecraft.wiki/w/Powered_Rail), [Activator Rail](https://minecraft.wiki/w/Activator_Rail), [Detector Rail](https://minecraft.wiki/w/Detector_Rail), [Minecart](https://minecraft.wiki/w/Minecart), [South-east rule](https://minecraft.wiki/w/South-east_rule), [Redstone components](https://minecraft.wiki/w/Redstone_components).

### 12.1 Состояния (minecraft-data 26.1)
- `rail` id 222, 5727–5746: `shape` — north_south, east_west, ascending_east, ascending_west, ascending_north, ascending_south, south_east, south_west, north_west, north_east; `waterlogged`. По умолчанию `north_south`.
- `powered_rail` 126 (2187–2210), `detector_rail` 127 (2211–2234), `activator_rail` 482 (11408–11431): `powered`, `shape` — **только 6 прямых/наклонных** значений, `waterlogged`. По умолчанию `powered=false, north_south`.

### 12.2 Установка и автосоединение
**Куда ставить:** «on top of a block whose top face has a rim around the edge, such as any full block, hopper, top slab, upside-down stairs, or top trapdoor». Можно в источник воды → `waterlogged`. Активирующие рельсы ставятся и на прозрачные блоки.

**Обычная рельса (Java):**
- нет соседей → ориентируется «in the direction the player is facing» (с 1.15, 19w39a);
- продолжение линии — в ту же сторону;
- два соседа (на том же уровне или на 1 выше/ниже) → прямая или поворот, чтобы соединить оба;
- три соседа (T) → поворот, соединяющий две стороны, с предпочтением юга и востока («a rail prefers south over north, and east over west»);
- четыре соседа → всегда поворот юг–восток;
- сосед на блок выше → наклон к нему **без** поворота; при нескольких вариантах порядок: запад, восток, юг, север;
- существующая прямая север–юг поворачивается на восток–запад, когда рядом ставят рельсу с востока или запада.

**Редстоун у обычной рельсы:** поворот в Т-развилке переключается питанием. «A rail's preference inverts when a redstone signal is applied, and then its direction is updated to be the most preferred state» — то есть под питанием предпочтение становится север/запад.

**Рельсы с питанием и детекторные (Java):** два соседа не на противоположных сторонах или три и больше → восток–запад; нет соседей → север–юг; при появлении соседа с востока или запада переходят в восток–запад. Наклон — в порядке запад, восток, юг, север.
**Активирующие (Java):** в тех же случаях — «aligns itself in the direction the player is facing»; вики помечает это «testingame» — **замерить**.

### 12.3 Рельсы с питанием и активирующие — передача
- Включаются: соседний источник питания, соседний запитанный блок (сильно или слабо), повторитель/компаратор, смотрящий в рельсу, пыль, **направленная** в рельсу. Пыль-точка рядом рельсу с питанием **не** включает; активирующую включает «плюсик».
- «An activated powered rail also activates any adjacent powered rails it is connected to, up to eight away from the original activation source. Thus a single activation source can activate up to 17 rails (one original in the middle and eight on either side).» Для активирующих — то же самое, только они передают сигнал **между собой**, а не рельсам с питанием.
- Передаётся только по **соединённым** рельсам того же типа; сила исходного сигнала на дальность не влияет; дальше передаёт только исходная рельса.
- Переданное так питание ничего, кроме рельс того же типа, не включает, но смена `powered` видна наблюдателю.
- Активирующие включаются и выключаются в том же такте, что и источник.

**Рельса с питанием и вагонетка:** активная добавляет `(vx*0.06/speed, vz*0.06/speed)` за такт при `speed > 0.01`; стоящую вагонетку разгоняет прочь от непрозрачного блока, в который упирается; неактивная — тормозит (обычно до остановки в пределах блока). Потолок — 8 блоков/с по оси.

**Активирующая рельса и вагонетка:** высаживает пассажиров (точку выбирает как при обычной высадке); вагонетка с командным блоком выполняет команду раз в 4 gt; отключает вагонетку-воронку (включает её неактивная); поджигает вагонетку с TNT (взрыв через 80 gt).

### 12.4 Детекторная рельса
- Включается, когда «any part of any minecart's bounding box comes into contact with it».
- Выход активной: пыль — 15; повторители от неё; **сильно** питает опорный блок, если он проводящий; активирует механизмы, в том числе сверху и снизу; компараторы от неё — заполненность вагонетки с сундуком/воронкой (иначе 0; при нескольких — «reads only the minecart that was placed first»). Компаратор читает её и через сплошной блок.
- Формула заполненности: `floor(1 + (Σ count/max_stack) / slots * 14)`, если есть хоть один предмет, иначе 0. Слотов: сундук — 27, воронка — 5.

**Тайминг — на вики два разных описания, нужно замерить:**
- страница `Detector Rail`: «Every 20th game tick after the rail first becomes powered, it checks whether any minecart is still present … the duration of the signal is always a multiple of 20 game ticks»;
- страница `Redstone components`: «activates 1 game tick after a minecart's hitbox first enter the block … always active for a multiple of 10 game ticks … waits 10 game ticks after the minecart leaves its block, then deactivates at the nearest multiple of 10 … always active for at least 20 game ticks».

Сценарий для чёрного ящика: вагонетка ставится на детекторную рельсу, снимается через N тактов (N = 1, 5, 15, 25, 35), фиксируем такт включения и выключения.

### 12.5 Вагонетка — только для справки
- minecraft-data 26.1: `minecart` id 85, `chest_minecart` 25, `furnace_minecart` 56, `hopper_minecart` 65, `tnt_minecart` 133; все 0.98×0.7.
- Метаданные: 8 `hurt`, 9 `hurtdir`, 10 `damage`, 11 `custom_display_block` (Optional Block State), 12 `display_offset` (VarInt, 1/16 блока, по умолчанию 6); у `furnace_minecart` ещё 13 `fuel` (Boolean).
- Скорость ≤ 8 блоков/с по оси (по диагонали ≈ 11.3). Правило `max_minecart_speed` — только в эксперименте «Minecart Improvements», в обычном 26.1 его нет.
- Урон: 1 HP → +10 к `damage`, −1 за такт, больше 40 → ломается.
- Подбирает мобов, если расширенный на 0.2 по X/Z хитбокс касается их (не: дракон, варден, иссушитель; в Java ещё летучая мышь и железный голем).
- Высадка: восемь соседних клеток в порядке «right, left, rear right, rear left, front right, front left, rear, front», затем выше, ниже.
- Нажимная плита рядом с **поворотом** рельсы срабатывает от вагонетки по диагонали; рядом с прямой — нет.

---

## 13. Нажимные плиты — кто их нажимает

Страницы: [Pressure Plate](https://minecraft.wiki/w/Pressure_Plate), [Stone Pressure Plate](https://minecraft.wiki/w/Stone_Pressure_Plate), [Polished Blackstone Pressure Plate](https://minecraft.wiki/w/Polished_Blackstone_Pressure_Plate), [Wooden Pressure Plate](https://minecraft.wiki/w/Wooden_Pressure_Plate), [Light Weighted Pressure Plate](https://minecraft.wiki/w/Light_Weighted_Pressure_Plate), [Heavy Weighted Pressure Plate](https://minecraft.wiki/w/Heavy_Weighted_Pressure_Plate).

**Зона срабатывания:** «the entity's collision box intersects with a space occupied by the pressure plate that's as large as the pressure plate itself, but is 0.25 blocks tall instead of 0.0625 blocks» (может сработать и от летящего низко).

| Плита | Кто нажимает | Сила | Проверка |
|---|---|---|---|
| каменная, из полированного чернокамня | «activated only by mobs, players, and armor stands» (живые сущности). **Не** предметы, стрелы, трезубцы, сферы опыта, вагонетки; пассажир вагонетки — да | всегда 15 | каждые 20 gt |
| деревянные (все породы 26.1: oak, spruce, birch, jungle, acacia, cherry, dark_oak, pale_oak, mangrove, bamboo, crimson, warped) | «most entities (including players, mobs, items, arrows, experience orbs, fishing bobs, etc.)». **Не** наблюдатели и маркеры. Рамки (обычные и светящиеся) **не включают, но удерживают** уже нажатую (Java) | всегда 15 | каждые 20 gt |
| лёгкая взвешенная (золото) | как деревянные | = число сущностей, максимум 15 | каждые 10 gt |
| тяжёлая взвешенная (железо) | как деревянные | `ceil(n / 10)`, максимум 15 (141+ сущностей) | каждые 10 gt |

Взвешенные считают **сущности, а не предметы**: стак из 64 предметов = 1 сущность. Тополевая плита (poplar) — 26.3, **не наша**.

**Длительность** (`Redstone components`): взвешенные — кратно 10 gt, минимум 10; каменные/деревянные — «first wait 10 game ticks after an entity moves off of them, then deactivate at the next multiple of 10», минимум 20. Со страницы `Pressure Plate`: «This period is 10 game ticks for weighted pressure plates and 20 game ticks for other pressure plates» — это одно и то же: проверка раз в 20 gt от момента нажатия.

**Выход:** пыль, повторители, компараторы вокруг; **сильно** питает блок под собой; активирует механизмы.

**Звуки (Java, 0.3, дальность 16):**

| Плита | включение | выключение |
|---|---|---|
| каменная / чернокамень | `block.stone_pressure_plate.click_on`, 0.6 | `…click_off`, 0.5 |
| деревянная | `block.wooden_pressure_plate.click_on`, 0.8 | `…click_off`, 0.7 |
| вишнёвая (и бамбук/незерские — у каждой свой набор) | `block.cherry_wood_pressure_plate.click_on`, 0.8 | 0.7 |
| взвешенные | `block.metal_pressure_plate.click_on`, 0.75 | `…click_off`, 0.65 |

Вибрации: включение — `block_activate` (10), выключение — `block_deactivate` (9).

---

## 14. Сводка: кто сильно питает, кто нет

| Компонент | Сила | Длительность | Сильно питает | Притягивает пыль |
|---|---|---|---|---|
| мишень | 1..15 по попаданию | 20 gt (стрела, трезубец) / 8 gt | нет (но проводит) | да, 4 стороны |
| датчик света | по формуле §2 | пересчёт раз в 20 gt | нет | да (источник) |
| крючок растяжки | 15 | кратно 10 gt; при обрыве 10 gt | блок-опору | да |
| громоотвод | 15 | 8 gt | блок-опору | да |
| кафедра | 15 | 2 gt при перелистывании | блок снизу | да |
| проигрыватель | 15 | пока играет песня | нет | да |
| датчик скалка | по расстоянию | 30 gt (калибр. 10) + 10 кулдаун | блок снизу | да |
| детекторная рельса | 15 | кратно 20 (или 10) gt, см. §12.4 | блок-опору | да |
| нажимные плиты | 15 / по числу | см. §13 | блок снизу | да |
| редстоун-руда | — | — | — (питания нет) | нет |
| колокол, TNT, музыкальный блок | — (механизмы) | колокол и нота — по фронту | — | — |

---

## 15. Что замерить чёрным ящиком (официальный 26.1.2)

1. **Мишень:** сетка попаданий `/summon arrow` по грани → таблица `power(u, v)`; заменить гипотезу из §1.
2. **Датчик света:** выход при SL=15 для T кратных 20 (сверить формулу §2.1 с сервером); SL=5 и SL=10 ночью (подтвердить или опровергнуть «ночной» выход у обычного датчика в тени).
3. **Детекторная рельса:** такты включения и выключения (§12.4).
4. **TNT:** поджог при установке рядом с включённым рычагом — в тот же такт или нет; позиция и скорость сущности в первом пакете.
5. **Колокол:** громкость звука `block.bell.use` в пакете (вики пишет 24.0).
6. **Проигрыватель:** содержимое World Event 1010 в 26.1.
7. **Скалк:** задержка от события до `active` на расстояниях 1, 4, 8 (и 16 у калиброванного); сила выхода по формуле.
8. **Активирующая рельса:** ориентация при установке без соседей (по взгляду игрока или север–юг).
