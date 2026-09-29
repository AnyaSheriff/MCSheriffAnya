# Контейнеры Minecraft Java Edition — справочник для реализации

Версия ориентира: Java Edition 26.1.2. Всё, что ниже, взято с minecraft.wiki (страницы и адреса указаны в каждом разделе, общий список — в конце). Код Minecraft, декомпилированный код, код серверных ядер, прокси, протокольных библиотек и модов не открывался и не пересказывался. То, что на вики появилось в 26.2 и позже (например, «sulfur cube», «abandoned camp», «poplar»), сюда **не** включено.

Единицы времени: **игровой такт (gt)** = 1/20 с. «Такт» ниже — всегда игровой такт.

Пометка **[проверить]** — вики молчит, пишет неоднозначно или две страницы противоречат друг другу; такие места надо снимать чёрным ящиком (`tools/blackbox`) с официального сервера.

---

## 1. Сундук (Chest) и большой сундук

Страница: https://minecraft.wiki/w/Chest

**Состояния блока (Java):**
- `facing` = north/east/south/west (по умолчанию north) — «сторона защёлки», **противоположная направлению взгляда игрока** при установке: "The opposite from the direction the player faces when placing a chest."
- `type` = single/left/right (по умолчанию single) — "The direction the chest has a connection with."
- `waterlogged` = false/true.

Блок-сущность `chest`: `Items` (слоты 0–26, "0 starts in the top left corner"), `CustomName`, `lock`, `LootTable`/`LootTableSeed`.

**Объём:** одиночный — 27 слотов, большой — 54: "A small chest has 27 slots of inventory space, and a large chest has twice that amount, at 54 slots."

**Слияние в большой сундук.**
- Сундук, поставленный рядом с другим, объединяется: "A chest placed adjacent to another chest joins to create a large chest."
- Чтобы не объединялись: присесть при установке (Java) или поставить второй сундук с другим `facing`: "by sneaking while placing the second chest in java … or placing the second chest facing a different direction from the first chest."
- Обычный и ловушечный сундук **не** объединяются: "trapped chests do not combine with normal chests."
- С 1.13 (17w47a) сундук можно ставить вплотную к большому, «тройных» не бывает; при приседании установка «пытается соединиться с блоком, по которому кликнули»: "Crouching while placing the chest now makes it try to connect to the clicked block." Страница 17w47a: "Shift right-clicking a chest … next to a chest … doesn't make the two chests … combine" (https://minecraft.wiki/w/Java_Edition_17w47a).
- Сливаются только по горизонтали, только одиночные сундуки того же вида и с тем же `facing` (последнее следует из «другое направление — не сливаются»).
- **[проверить]** точный алгоритм: какой сосед (слева/справа относительно `facing`) выбирается первым, если подходят оба; какому из двух достаётся `type=left`, а какому `right`; поведение при приседании (берётся ли `facing` соседа, по которому кликнули). Вики этого не расписывает — снять чёрным ящиком.
- Разрушение половины большого сундука: выпадает содержимое этой половины, вторая становится одиночной: "the top three rows for the left-hand chest or the bottom three for the chest on the right … are dropped and the remaining half continues to function as a small chest."

**Какая половина — верхние три ряда окна.** Страница Chest: "the top three rows for a large chest correspond to the left half of the chest when facing it, and the bottom three rows correspond to the right half." Страница Trapped Chest пишет иначе: "the top three rows in the interface correspond to the western or northern chest block". **[проверить]** — формулировки противоречат друг другу.

**Заголовок окна:** "Chest" / "Large Chest"; если переименована одна половина, берётся её имя; если обе — имя «северной или западной» половины.

**Что мешает открыть.**
- Проводящий блок сверху: "A chest cannot be opened if there is a conductive block above it." Непроводящие полные блоки (стекло, лёд), нижние плиты и ступени не мешают (Java): "the lid of the chest can phase through blocks such as bottom-half slabs, stairs, and non-conductive full cubes such as glass and ice."
- Сидящая на сундуке кошка: "Cats sitting on chests prevent them from opening."
- Два сундука друг на друге открываются (сундук непроводящий).
- **[проверить]** для большого сундука: блокирует ли препятствие над *любой* половиной (ожидается — да).
- Замок `lock`: без предмета, подходящего под предикат, не открывается; звук `block.chest.locked` (громкость 1.0, высота 1.0).

**Звуки (Java):**
| Событие | id | громкость | высота |
|---|---|---|---|
| открытие | `block.chest.open` | 0.5 | 0.9–1.0 |
| закрытие | `block.chest.close` | 0.5 | 0.9–1.0 |
| заперт | `block.chest.locked` | 1.0 | 1.0 |

Громкость 0.5 — после 1.21.9 (25w31a): "Sounds for chests now plays at a lower volume." Номера в реестре звуков 26.1.2 (из `tools/blackbox/data/sounds.json`): `block.chest.close` = 345, `block.chest.locked` = 346, `block.chest.open` = 347. **[проверить]** звук играется при первом открывшем (0→1) и последнем закрывшем (1→0); откуда звучит большой сундук (центр между половинами или каждая половина).

**Block Action (анимация крышки).** Страница: https://minecraft.wiki/w/Java_Edition_protocol/Block_actions
- Для `chest`, `trapped_chest`, `ender_chest` одно действие **id = 1**, параметр = число игроков, у которых сундук открыт: "When the action ID is 1, this is the number of players who have the chest open. If 0, then the chest is closed; if 1 or more the chest is open."
- Когда отправляется: "whenever a player opens or closes the chest, and every 5 ticks (0.25 seconds) while at least one player has the chest open." **[проверить]** периодическую отправку раз в 5 тактов — это текст бывшего wiki.vg, мог устареть.
- Для большого сундука **[проверить]**, шлётся ли действие на обе половины.
- Для медного сундука в списке вики его нет, но в Java у него блок-сущность `chest` (см. раздел 2) — **[проверить]** тот же id 1.

**События для датчиков скалка:** открытие контейнера — `container_open` (частота 10), закрытие — `container_close` (частота 9) (https://minecraft.wiki/w/Vibration).

**Прочее:** не двигается поршнем в Java (двигается только в Bedrock); пиглины злятся при открытии и разрушении.

---

## 2. Медный сундук (Copper Chest) — кратко

Страница: https://minecraft.wiki/w/Copper_Chest

- Добавлен в 1.21.9; 8 блоков: `copper_chest`, `exposed_…`, `weathered_…`, `oxidized_…` и воскованные версии. Состояния — как у сундука (`facing`, `type`, `waterlogged`). Блок-сущность в Java — `chest`.
- Сливаются **независимо от окисления и воска**: "the resulting large chest takes the oxidization level of the least oxidized half, and is unwaxed if one of the two halves is unwaxed."
- Приседание / другой `facing` — не сливаются (как у сундука). В Java поршнем не двигается.
- Открытие блокируется так же, как у сундука (проводящий блок сверху, кошка).
- Звуки (реестр 26.1.2): `block.copper_chest.open`/`close` (404/403), `block.copper_chest_weathered.open`/`close` (406/405), `block.copper_chest_oxidized.open`/`close` (408/407). Вики даёт таблицу только для Bedrock (громкость 1.0, высота 0.9–1.0); **[проверить]** громкость в Java.
- Заголовок окна в Java — "Chest"/"Large Chest".

---

## 3. Ловушечный сундук (Trapped Chest)

Страницы: https://minecraft.wiki/w/Trapped_Chest, https://minecraft.wiki/w/Redstone_components

- Всё как у сундука (27/54 слота, слияние, приседание, блокировка открытия, звуки `block.chest.*` 0.5 / 0.9–1.0, Block Action id 1). С обычным сундуком не сливается.
- Активен, пока его кто-то открыл (игрок или медный голем): "activates when accessed by a player or copper golem." Воронки и выбрасыватели его **не** активируют: "trapped chest is not activated by items moving into or out of it by droppers or hoppers."
- Открытие любой половины большого ловушечного сундука активирует **обе** половины.

**Сигнал, пока активен:**
- сила = число смотрящих, не больше 15: "powers any adjacent redstone dust, including beneath the trapped chest, to a power level equal to the number of players or copper golems accessing the trapped chest at once (maximum 15)";
- повторители, смотрящие от сундука, получают 15 (повторитель и так всегда выдаёт 15);
- **сильно питает блок под собой** той же силой: "strongly powers any full solid opaque block … beneath the trapped chest to a power level equal to the number of players";
- активирует механизмы со всех сторон, включая верх и низ; поэтому воронка под открытым ловушечным сундуком заперта и не тянет из него;
- компаратор, смотрящий от сундука, питанием **не** получает, а читает заполненность: "An active trapped chest does not power any adjacent redstone comparators facing away from it."
- Страница Redstone components: "activates immediately when one or more players accesses it, and deactivates immediately when players stop accessing it."

Итого для движка: слабый сигнал во все 6 сторон = min(viewers, 15), сильный — только вниз. При каждом изменении числа смотрящих — обновление соседей сундука и соседей блока под ним. Пыль тянется к ловушечному сундуку, к обычному — нет.

---

## 4. Бочка (Barrel)

Страница: https://minecraft.wiki/w/Barrel

- Состояния: `facing` = down/east/north/south/up/west (по умолчанию north), `open` = false/true ("Whether the barrel is currently being looked at by a player").
- 27 слотов. **Открытию ничто не мешает:** "Unlike chests, the action of opening a barrel is never prevented."
- Воронки кладут и забирают, выбрасыватели кладут. Поршнем в Java не двигается.
- Анимации через Block Action нет — только смена `open`.
- Звуки: `block.barrel.open` / `block.barrel.close`, громкость 0.5, высота 0.9–1.0 (реестр: 141/140).
- **[проверить]** `facing` при установке (ожидается «к игроку» по 6 направлениям, как у раздатчика).
- Рабочее место рыбака.

---

## 5. Эндер-сундук (Ender Chest) — только заметка

Страница: https://minecraft.wiki/w/Ender_Chest

- 27 слотов, у каждого игрока свои; с соседями не сливается. `facing` — как у сундука.
- Твёрдый блок сверху не даёт открыть: "if a solid block is placed above an ender chest, it cannot be opened."
- Воронки и выбрасыватели с ним **никак** не работают. Компаратор его не читает.
- Block Action id 1 (параметр = число смотрящих), звуки `block.ender_chest.open`/`close`, 0.5 / 0.9–1.0.

## 5а. Шалкеровый ящик — заметка

Страница: https://minecraft.wiki/w/Shulker_Box
- Block Action: id 1 = число смотрящих; id 0 = открытие/закрытие.
- Открыть можно, если перед крышкой свободно полблока: "require a transparent area in the direction of its top surface … exactly half of a full block".
- Компаратор читает его **всегда**, даже если открыть нельзя. Шалкеровый ящик нельзя положить в шалкеровый ящик ни воронкой, ни выбрасывателем.
- Звуки `block.shulker_box.open`/`close`, 0.5 / 0.9–1.0.

---

## 6. Воронка (Hopper)

Страницы: https://minecraft.wiki/w/Hopper, https://minecraft.wiki/w/Tutorial:Hopper, https://minecraft.wiki/w/Tick

**Состояния:** `enabled` = true/false ("When the hopper is being powered by redstone current, this is set to false"), `facing` = down/east/north/south/west (вверх не бывает). Блок-сущность: `Items` (5 слотов), `TransferCooldown` ("naturally between 1 and 8 or 0 if there is no transfer").

**Установка:** выход смотрит на ту грань, по которой кликнули: "aiming at the surface to which its output should face (Hoppers do not orient themselves automatically)". Клик по нижней грани блока → `facing=down`. К контейнеру не «прилипает». В Java поршнем не двигается.

**Три действия** (когда не заперта и не на перезарядке):
- **Push** — один предмет из себя в контейнер, на который смотрит выход;
- **Pull** — один предмет из контейнера сверху;
- **Collect** — подбор предметов-сущностей сверху.

"A hopper first attempts to push any items inside it. Afterward, it checks if the block above it is a type of container. If so, it attempts to pull from it. Otherwise, the hopper attempts to collect item entities."

**Порядок внутри такта:** сначала push, потом pull — в одном такте: "Item pushes and pulls are processed in the same game tick, but pushes are processed before pulls." После push и/или pull — перезарядка 8 тактов: "After pulling and/or pushing items, a hopper enters an 8 game tick … cooldown period (a transfer rate of 2.5 items per second)." После сбора сущностей — тоже 8 тактов.
(Tutorial:Hopper в одном месте пишет, что сначала «ищется предмет на входе», — это противоречит основной странице со схемой; верить основной странице.)

**Перезарядка и получение предметов.**
- Java: если предмет втолкнули в **пустую** воронку, её перезарядка — **7** тактов: "In JE, when an item is pushed into an empty hopper, the cooldown of that hopper lasts 7 game ticks … instead. Pushing an item into a hopper that isn't empty does not trigger its cooldown." Игрок руками — не считается.
- История (16w35a/1.11-pre1): "Blocks that push an item into an empty hopper now apply a cooldown to it even if that hopper was already in cooldown, effectively overriding the previous cooldown."; "Custom hopper cooldowns … that last more than 8 game ticks cannot be overridden"; "if an empty hopper has an item pushed into it by another hopper, the cooldown … will always last 7 game ticks. This change solves some of the hoppers' locationality issues."
- **[проверить]** даёт ли эту перезарядку выбрасыватель/крафтер, толкающий в пустую воронку, или только воронка; 7 ли это всегда или зависит от того, прошёл ли уже такт принимающей воронки.

**Как тикается.** Воронка — блок-сущность; в такте мира блок-сущности тикают после сущностей, до обработки игровых событий (порядок со страницы Tick: запланированные такты блоков → жидкостей → … → block events → сущности → **блок-сущности** → game events). Каждый такт счётчик `TransferCooldown` уменьшается; действия пробуются, когда он дошёл до 0. Запертая воронка счётчик продолжает гонять: "Depowering a locked hopper does not affect its cooldown time. Therefore, unlocking a hopper that was not already in cooldown results in it starting to pull, push, or collect items as soon as it gets ticked."

**Запирание редстоуном.** Любой сигнал останавливает все три действия (`enabled=false`). Квазисвязности у воронки нет (её нет в списках QC). Запертая воронка **всё ещё принимает** предметы от выбрасывателей, крафтеров и других воронок и отдаёт их воронке снизу: "While a locked hopper does not push or pull/collect items, it may still receive items from droppers, crafters and other hoppers, and may have its items pulled out by another hopper beneath it." Выбрасыватель кладёт даже в запертую воронку.

**Слоты — всегда «самый левый подходящий».**
- "A hopper always tries to push or pull items using the leftmost available slot."
- При вытягивании приоритет у **первого слота источника**, а не первого слота воронки: воронка [камень, пусто], сундук [курица, камень] → тянется курица во второй слот воронки; если курицу взять некуда, тянется камень из второго слота сундука.
- При проталкивании приоритет у **первого слота воронки**: воронка [камень, курица], цель [курица, камень] → камень из первого слота воронки уходит во второй слот цели.
- Сундук наполняется слева направо и опустошается слева направо.
- **[проверить]** при вставке, если раньше идёт пустой слот, а позже — неполная стопка того же предмета: кладётся в первый подходящий (пустой) или сперва доливается стопка.

**Откуда тянет (сверху).**
1. Если сверху блок-контейнер — только из него (сущности-контейнеры тогда не смотрятся): "A hopper that has both a container block and one or more container entities above itself pulls items only from the container block."
2. Иначе — из сущностей-контейнеров (вагонетка с сундуком/воронкой, лодка с сундуком). Полный блок сверху этой проверке **не** мешает.
3. Иначе — подбирает предметы-сущности.

**Зона сбора предметов.** Внутренность самой воронки плюс весь блок над ней: "Items are gathered from the entire block space above the hopper, meaning that items sitting on partial blocks such as soul sand directly above a hopper can be collected." Не собирает, если сверху контейнер или **полный** блок без тега `#does_not_block_hoppers` (в теге — улей и пчелиное гнездо); лежащее внутри воронки собирается всегда. Сущность забирается «пачкой» — сколько влезет. Если предметов несколько, первым берётся тот, что раньше попал в чанк. Собранное кладётся в самый левый пустой слот. **[проверить]** точные границы нижней части зоны (внутренность чаши).

**Куда толкает.** В блок-контейнер перед выходом; если его нет — в сущности-контейнеры, хитбокс которых хоть частично в целевом блоке: "if any part of the entity's hitbox is within the hopper's target block-space."

**Особые контейнеры** (сторона = с какой стороны блока стоит воронка):
| Контейнер | Сверху (воронка смотрит вниз) | Сбоку | Снизу (воронка тянет) |
|---|---|---|---|
| Печь / плавильня / коптильня | в слот сырья, **любой** предмет | в слот топлива, только топливо | из слота результата + пустые вёдра из слота топлива |
| Варочная стойка | только годные ингредиенты | бутылки (вкл. пустые) и огненный порошок | из трёх слотов с зельями, **даже если варка не кончилась** |
| Компостер | только компостируемое, с шансом поднять уровень | не взаимодействует | костная мука при уровне 8, сброс в 0 |
| Проигрыватель | пластинка (если пуст) | пластинка | пластинка, когда не играет (играющий проигрыватель запирает соседние воронки своим сигналом) |
| Резная книжная полка | книги в первый пустой слот | то же | первая непустая; попытка вытянуть, даже неудачная, меняет «последний слот» |
| Декорированный горшок | до стопки **одного** вида | то же | забирает (единственный способ без разрушения) |
| Крафтер | в включённые слоты по правилу крафтера (раздел 9) | то же | забирает **ингредиенты**, не результат |
| Полка (Shelf) | в первый пустой / в стопку того же вида | то же | из первого непустого |
| Шалкеровый ящик | всё, кроме шалкеровых ящиков | то же | обычно |
| Эндер-сундук, кафедра | нет | нет | нет |
| Большой сундук | один инвентарь на 54 слота | то же | то же |

Цитаты: печь — "A hopper pointing into the side of a furnace deposits into the fuel slot, and only items that are usable as fuel. A hopper below a furnace pulls everything from the output slot and empty buckets from the fuel slot"; варочная стойка — "A hopper underneath a brewing stand always extracts from the three brew slots, whether brewing is finished or not"; компостер — "Hoppers to the side of a composter do not interact with it"; большой сундук — "Large chests and large trapped chests are treated as a single container". Опыт за переплавку, вынутую воронкой, копится в печи.

**Заблокированный сундук.** С 1.9.1 воронка толкает в сундук и тянет из сундука, который нельзя открыть: "A hopper can now push into and pull items from a blocked chest." (А выбрасыватель — не может, см. раздел 7; компаратор такой сундук читает как 0, раздел 10.)

**Вагонетка с воронкой** (https://minecraft.wiki/w/Minecart_with_Hopper): тянет 1 предмет каждый такт (20/с) из контейнера, нижняя грань которого на 0.5 (не вкл.)–1.5 (вкл.) выше низа вагонетки; толкать не умеет; запирается активирующими рельсами, а не сигналом рядом.

---

## 7. Выбрасыватель (Dropper)

Страница: https://minecraft.wiki/w/Dropper

**Состояния** (общие с раздатчиком): `facing` = 6 направлений, "The opposite from the direction the player faces while placing the block" (т.е. выход к игроку); `triggered` = false/true. 9 слотов. В Java поршнем не двигается.

**Срабатывание.** Как механизм: сигнал соседнего источника (факел не включает блок, на котором висит), запитанный непрозрачный блок рядом, повторитель/компаратор, смотрящий на него, пыль, указывающая на него или лежащая сверху. Плюс квазисвязность (Java): срабатывает, если сигнал *включил бы* механизм в клетке над ним, но только при получении обновления блока.

**Задержка 4 такта, по фронту.** "A dropper has a delay of 4 game ticks … between activation and a response. During this time, additional inputs are ignored." "ejection occurs only on the initial activation (the rising edge of an input signal)." То есть: сигнал появился и `triggered=false` → `triggered=true` + запланировать такт через 4; сигнал пропал → `triggered=false` (без такта).

**Выброс.** Случайный непустой слот, выбираемый в момент выброса, а не в момент включения: "The slot is chosen when an item is ejected, not when the dropper is initially activated." **[проверить]** равномерность выбора.

- **Перед ним контейнер:** переносится **один** предмет. Если вставить нельзя — выбрасыватель «не срабатывает» (предмет остаётся): "If the container it is facing is full, or the item cannot be inserted into the container, the dropper does not activate. Examples … include chests that cannot be opened because of a solid block above them, and droppers that are already in an activated state."
- **Контейнера нет:** предмет выбрасывается, "as if a player had used the drop control". Стрелы и прочее, что раздатчик применяет особо, просто выпадают.
- Сущности-контейнеры (вагонетки с сундуком/воронкой) — если хитбокс заходит в целевую клетку; в вагонетку с воронкой — даже запертую.
- Стороны у особых контейнеров: печь сверху → сырьё, сбоку и **снизу** → топливо; варочная стойка сверху → ингредиент, сбоку/снизу → огненный порошок и **наполненные** бутылки (у воронки вики пишет «включая пустые» — **[проверить]**); компостер только сверху; резная полка — книги с любой стороны; проигрыватель — с любой стороны, не срабатывает, если уже играет; кафедра и эндер-сундук — никак; полка — сперва в стопку того же вида, иначе в первый пустой, не срабатывает, если не влезло.
- Директно запитанный выбрасыватель срабатывает раньше соседних, включённых в тот же такт (дропперный лифт двигает предмет на 2 блока за такт).

**Звук и частицы.** Вики: "A dropper makes a clicking noise … when activated empty or when ejecting items into air. It is silent when it successfully transfers an item into any kind of chest or barrel, or another dropper."
| Событие | звук | громкость / высота | World Event (id, data) |
|---|---|---|---|
| выброс в мир | `block.dispenser.dispense` (514) | 1.0 / 1.0 | 1000; плюс дым 2000 с направлением |
| пустой / неудача | `block.dispenser.fail` (515) | 1.0 / 1.2 | 1001 |

Направление дыма в data события 2000: 0 вниз, 1 вверх, 2 север, 3 юг, 4 запад, 5 восток (https://minecraft.wiki/w/Java_Edition_protocol/Packets, раздел World Event). **[проверить]** есть ли дым при успешной вставке в контейнер и играет ли `fail`, если контейнер полон.

**Скорость и разброс выброшенного предмета** на вики не описаны — **[проверить]** чёрным ящиком (стартовая точка, скорость, разброс).

---

## 8. Раздатчик (Dispenser)

Страница: https://minecraft.wiki/w/Dispenser

Те же состояния, 9 слотов, та же активация (включая квазисвязность), та же задержка 4 такта по фронту, тот же случайный слот в момент выброса. В отличие от выбрасывателя, **в контейнер не кладёт**: "Unlike a dropper, a dispenser can't push items inside another container." Инструменты с прочностью её тратят.

**Звук/частицы:** "it emits a clicking sound (unless the item is a projectile or a spawn egg) and a smoke particle. If it is empty when activated, it emits a slightly higher-pitched click." Снаряды — `block.dispenser.launch` (516), 1.0 / 1.2, World Event 1002. Остальное — как у выбрасывателя (1000/1001/2000).

**Особые действия** (всё, что ниже, — из таблицы "Dispenser behavior"; «перед ним» = клетка, куда смотрит выход):
| Предмет | Действие |
|---|---|
| Броня, элитры | надевает на игрока / моба / стойку в клетке перед ним, если слот пуст (моб должен уметь подбирать предметы) |
| Стойка для брони | ставит сущность |
| Стрела, спектральная, с эффектом | выстрел по направлению |
| Лодка, лодка с сундуком | ставит сущность, если перед ним вода или воздух над водой; иначе выпадает |
| Костная мука | применяет к блоку перед собой; если не на что — не тратится |
| Пузырёк опыта, яйцо, снежок, взрывное и оседающее зелье, заряд ветра | бросок; зелья летят дальше |
| Пустое ведро | черпает источник воды, лаву или порошковый снег; полное ведро — в пустой слот, если некуда — выпадает; если нечего черпать — выпадает пустое ведро |
| Огненный заряд | огненный шар (как у ифрита) |
| Фейерверк | запуск по направлению |
| Огниво | огонь в воздухе, если под ним есть блок; поджигает ТНТ; зажигает потухший костёр; иначе ничего |
| Вёдра воды, лавы, рыбы (треска, лосось, тропическая, иглобрюх), аксолотля, головастика, порошкового снега | выливает, если туда можно вылить ведро (не в котёл); ведро становится пустым; иначе выпадает; вода в Незере испаряется; рыба/аксолотль/головастик появляются |
| Вагонетки всех видов | ставит на рельсы перед собой; иначе выпадает |
| Головы, вырезанная тыква | достраивает железного, медного, снежного голема или иссушителя; череп визер-скелета не ставится на Peaceful и под водой; иначе надевает на голову сущности; иначе ничего |
| Шалкеровый ящик | ставит блок; если под целевой клеткой нет блока — смотрит как раздатчик, иначе вверх (страница Shulker Box) |
| Яйцо призыва | призывает моба перед собой (без щелчка) |
| ТНТ | подожжённый ТНТ в клетке перед собой, с подбросом вверх и небольшой случайной скоростью по горизонтали |
| Ножницы | стрижёт овцу, болотника, снежного голема, грибную корову; собирает соты с полного улья/гнезда; отвязывает поводки |
| Стеклянный пузырёк | наполняет водой из источника или мёдом из полного улья; если некуда положить — выпадает; иначе выпадает пустой пузырёк |
| Конская броня | надевает на приручённую лошадь / лошадь-зомби |
| Броня наутилуса | на приручённого наутилуса / зомби-наутилуса (1.21.11) |
| Седло | свинья, лавомерка, верблюд, верблюд-скелет (camel husk), приручённая лошадь, осёл, мул, лошадь-зомби, лошадь-скелет, наутилус, зомби-наутилус |
| Упряжь (harness) | на счастливого гаста |
| Ковёр | на приручённую ламу |
| Сундук | на приручённую ламу, осла, мула |
| Светокамень | заряжает неполный якорь возрождения |
| Медовые соты | вощат медный блок перед собой |
| Бутылка воды | земля, каменистая земля, земля с корнями → грязь |
| Кисть | чистит броненосца |
| Трезубец | бросок — **только Bedrock** |
| Всё остальное | выпадает, как у выбрасывателя |

Мобы, на которых раздатчик надевает броню: болотник, утопленник, кадавр, parched, разбойник, пиглин, брут, скелет, зимогор, житель (Java), визер-скелет, зомби, зомби-житель (только обращённый из жителя), зомбифицированный пиглин.

**[проверить]** какие неудачи дают звук `fail` (1001): огниво без цели, мука без цели, ножницы без цели, шалкеровый ящик не встал, голова/тыква не надета, светокамень при полном якоре (на странице Sculk Sensor эти случаи перечислены как «раздатчик не смог выполнить действие»). Вики не пишет и о надевании брони волку — **[проверить]**.

---

## 9. Крафтер (Crafter)

Страницы: https://minecraft.wiki/w/Crafter, https://minecraft.wiki/w/Tutorial:Crafter

**Состояния:** `crafting` = false/true ("Whether the crafter's mouth is open and top is glowing"), `orientation` — 12 значений (`down_east`, `down_north`, `down_south`, `down_west`, `east_up`, `north_up`, `south_up`, `up_east`, `up_north`, `up_south`, `up_west`, `west_up`; по умолчанию `north_up`), `triggered` = false/true.

**Блок-сущность:** `Items` (слоты 0–8, 0 — левый верхний), `disabled_slots` (int-массив номеров отключённых слотов), `triggered` (1, пока запитан), `crafting_ticks_remaining` ("Set to 6 when the crafter crafts something" — то есть `crafting=true` держится 6 тактов).

**Слоты.** 9 слотов 3×3. Слот переключается кликом, **только когда пуст**; в отключённый ничего не положить.

**Вставка воронкой/выбрасывателем/другим крафтером** (с любой стороны):
1. ищется пустой **включённый** слот в порядке чтения (по рядам, слева направо);
2. если все включённые заняты — предмет добавляется к **самой маленькой** стопке того же предмета; при равенстве — первая по порядку чтения.
Воронка снизу забирает ингредиенты, не результат.

**Срабатывание.** По фронту сигнала, задержка **4 такта**: "When a crafter receives a redstone signal, it waits for 4 game ticks … before ejecting one crafted item using the ingredients from the nine inventory slots." **Квазисвязности нет:** "crafters aren't affected by quasi-connectivity." Результат уходит в контейнер перед лицом (как у выбрасывателя) или выбрасывается; "If a recipe has byproducts (e.g. empty bottles for honey blocks or empty buckets for cake) those are ejected after the crafted item." Если рецепта нет — ничего не выпадает (звук неудачи).

**Звуки и события:** `block.crafter.craft` (456) 1.0 / 1.0, 0.96, 1.04; `block.crafter.fail` (457) 1.0 / те же. World Event 1049 — успех, 1050 — неудача, 2010 — дым крафтера с направлением.

**Компаратор:** число слотов, которые **заняты или отключены** (0–9); размер стопки не важен: "having 1 stick in a slot vs having 64 sticks in that same slot both output the same comparator signal."

---

## 10. Что читает компаратор

Страницы: https://minecraft.wiki/w/Redstone_Comparator (раздел "Read blocks"), https://minecraft.wiki/w/Block_update (раздел "Comparator update") и страницы самих блоков.

### 10.1 Как компаратор берёт блок сзади
- Читается блок **прямо сзади** или блок за одним **проводящим** блоком: "The comparator may be separated from the measured block by a comparator-conducting block. These blocks include all conductive blocks." Уровень питания промежуточного блока при этом не учитывается, **кроме** 15: "if the intervening block is powered to signal strength 15, then the comparator outputs 15 no matter the fullness of the container."
- У котла через промежуточный блок обновление приходит не сразу: "if there is a block between the cauldron and the comparator, the comparator does not immediately update."

### 10.2 Формула заполненности контейнера
- Пустой → 0.
- Иначе: `сила = floor(1 + (Σ fullness_слота / число_слотов) × 14)`, где `fullness = количество / максимальный_размер_стопки_этого_предмета`.
- Вики: "When a container is empty, the output is off." "signal strength = floor(1 + ((sum of all slots' fullnesses) ÷ (number of slots in container)) × 14)". Частично заполненный мешок считается полной стопкой: "A partially filled bundle counts as a full stack."
- Обратная формула (сколько предметов по 64 нужно): `max(s, ceil(слоты × 64 / 14 × (s − 1)))`.
- Большой сундук читается целиком (54 слота) с любой половины.
- **Java: сундук и ловушечный сундук, которые нельзя открыть (проводящий блок или кошка сверху), всегда дают 0**; шалкеровый ящик читается всегда.

Пороги (минимум предметов, стакающихся по 64):
| Слотов | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 (горшок) | 1 | 5 | 10 | 14 | 19 | 23 | 28 | 32 | 37 | 42 | 46 | 51 | 55 | 60 | 64 |
| 3 (печи) | 1 | 14 | 28 | 42 | 55 | 69 | 83 | 96 | 110 | 124 | 138 | 151 | 165 | 179 | 192 |
| 5 (воронка, вагонетка с воронкой, варочная стойка) | 1 | 23 | 46 | 69 | 92 | 115 | 138 | 160 | 183 | 206 | 229 | 252 | 275 | 298 | 320 |
| 9 (раздатчик, выбрасыватель) | 1 | 42 | 83 | 124 | 165 | 206 | 247 | 288 | 330 | 371 | 412 | 453 | 494 | 535 | 576 |
| 27 (сундук, медный, шалкер, бочка, вагонетка с сундуком) | 1 | 124 | 247 | 371 | 494 | 618 | 741 | 864 | 988 | 1111 | 1235 | 1358 | 1482 | 1605 | 1728 |
| 54 (большой сундук) | 1 | 247 | 494 | 741 | 988 | 1235 | 1482 | 1728 | 1975 | 2222 | 2469 | 2716 | 2963 | 3210 | 3456 |

(Посчитано по обратной формуле; совпадает с таблицей вики "Minimum items for container signal strength".) Предмет со стопкой 16 весит как 4, нестакающийся — как 64.

Контейнеры, которые читаются по этой формуле: бочка, плавильня, варочная стойка, сундук, медный сундук, декорированный горшок, раздатчик, выбрасыватель, печь, воронка, большие сундуки (обычный, медный, ловушечный), вагонетки с сундуком и с воронкой **на детекторной рельсе**, шалкеровые ящики, коптильня, ловушечный сундук.

### 10.3 Прочие блоки и сущности
| Что | Выход |
|---|---|
| Улей, пчелиное гнездо | `honey_level` 0–5 |
| Торт | 2 × оставшиеся куски: целый 14, далее −2 за кусок (`(7 − bites) × 2`) |
| Котёл | вода и порошковый снег: уровень 0–3; лава — всегда 3; пустой — 0 |
| Резная книжная полка | номер последнего слота, с которым было взаимодействие, 1–6 (`last_interacted_slot + 1`; слоты 0–2 верхний ряд слева направо, 3–5 нижний); не было — 0 |
| Компостер | уровень 0–8 (7 — «полон, но мука ещё не готова», держится недолго) |
| Медная лампа | 15, если горит, иначе 0 |
| Статуя медного голема | 1–4 по позе (позы по циклу: стоит, сидит, бежит, «звезда») — **[проверить]** соответствие чисел позам |
| Командный блок | `SuccessCount` последней команды (по коду может быть больше 15) |
| Крафтер | число занятых или отключённых слотов 0–9 |
| Сердце скрипуна (активное, со связанным скрипуном) | `15 − floor(расстояние / 32 × 15)` |
| Рамка портала Края | 15 с оком, иначе 0 |
| Рамка и светящаяся рамка (сущности) | пусто 0; с предметом 1–8 по повороту (1 при установке, +1 за 45°); компаратор — за блоком, к которому прикреплена рамка; если в той же клетке ещё что-то читаемое — берётся большее |
| Проигрыватель | по пластинке: 13→1, cat→2, blocks→3, chirp→4, far→5, mall→6, mellohi→7, stal/Bounce→8, strad/Lava Chicken→9, ward/Tears→10, 11/Creator (Music Box)→11, wait/Creator→12, Pigstep/Precipice→13, otherside/Relic→14, 5→15; без пластинки 0; держится, пока пластинка внутри, даже если отыграла |
| Кафедра | `floor(1 + (страница − 1) / (страниц − 1) × 14)`; книга в 1 страницу → 15 |
| Якорь возрождения | 0, 3, 7, 11, 15 для 0–4 зарядов (`floor(заряды / 4 × 15)`) |
| Датчик скалка и настроенный датчик | частота последней вибрации (1–15) — **[проверить]** держится ли значение, когда датчик неактивен |
| Полка (Shelf) | левый слот 1, средний 2, правый 4, сумма до 7; только если к компаратору обращена задняя сторона полки |
| Детекторная рельса | заполненность вагонетки с сундуком/воронкой на ней (читает ту, что поставлена первой); другие вагонетки — 0 |

Частоты вибраций, полезные для контейнеров (https://minecraft.wiki/w/Vibration): 9 — `container_close`, `block_close`, `block_deactivate`, `block_detach`; 10 — `container_open`, `block_open`, `block_activate`, `block_attach`, `prime_fuse`, `note_block_play`; 11 — `block_change`.

### 10.4 Обновление компараторов («comparator update»)
Страница Block update: "The comparator update … is a subtype of the neighbor update that is only sent to comparators." "When a block that can be detected by comparators changes, it sends updates to surrounding comparators (including comparators separated by a solid block), in this order: north, east, south, west."

Для движка:
- Источник обновления — любое изменение содержимого контейнера (клик игрока в окне, перенос воронкой, выбрасывателем, крафтером, вагонеткой, подбор сущности воронкой) и любое изменение читаемого состояния (кусок торта, уровень котла/компостера, страница кафедры, пластинка, око в рамке, заряд якоря, мёд, лампа, поворот предмета в рамке и т.д.).
- Получатели — 4 горизонтальных соседа по порядку **С, В, Ю, З**: компаратор рядом; если сосед — проводящий блок, то компаратор **за ним** в том же направлении.
- Рамки — сущности, но тоже рассылают такие обновления.
- **[проверить]** рассылает ли большой сундук обновления от обеих половин и шлётся ли обновление, когда сундук становится заблокированным (кошка села, блок поставили сверху).

---

## 11. Общие правила переноса предметов

Страницы: https://minecraft.wiki/w/Inventory, https://minecraft.wiki/w/Hopper, https://minecraft.wiki/w/Dropper

- Размер стопки: 64 у большинства, 16 у многих снарядов и т.п. (снежки, эндер-жемчуг, таблички), 1 у предметов с прочностью, вёдер и т.д.; компонентами можно до 99 в Java: "All items can stack up to 99 (JE) … using components." Стопки сравниваются по предмету **и компонентам** (переименованные не складываются с обычными — на этом стоят сортировщики).
- Воронка и выбрасыватель переносят **ровно 1 предмет** за операцию; воронка забирает предмет-сущность целиком, сколько влезет.
- Контейнер полон / предмет не подходит (нет места, не тот слот у печи/стойки/компостера, чужой предмет в горшке, пластинка при занятом проигрывателе, шалкер в шалкер) → перенос не происходит: воронка ничего не меняет в эту сторону (и не уходит в перезарядку, если не удался ни push, ни pull), выбрасыватель «не срабатывает».
- Предмет в воронке, которую нельзя вытолкнуть, не мешает ей тянуть сверху в свободные слоты; полная воронка не тянет, пустая не толкает (следует из схемы "Push then pull" на странице Hopper).
- Вагонетка с сундуком и лодка с сундуком для воронок и выбрасывателей — обычные контейнеры (27 слотов).
- Комбинации, которые запирают воронки своим сигналом: открытый ловушечный сундук, играющий проигрыватель, перелистывание книги на кафедре (импульс 2 такта), детекторная рельса.

---

## 12. Что проверить чёрным ящиком в первую очередь
1. Слияние сундуков: какой сосед выбирается, `type=left/right`, приседание.
2. Какая половина большого сундука — верхние ряды окна.
3. Перезарядка воронки после получения в пустую: всегда 7 или 7/8 в зависимости от порядка; даёт ли её выбрасыватель.
4. Точная зона сбора предметов воронкой.
5. Скорость/разброс предмета из выбрасывателя; дым и звук при вставке в контейнер и при неудаче.
6. Периодичность Block Action сундука (раз в 5 тактов или только при смене числа смотрящих).
7. Звуки неудачи раздатчика по видам предметов.

---

## Использованные страницы

Все — только minecraft.wiki (разметка страниц через `?action=raw`):
- https://minecraft.wiki/w/Chest
- https://minecraft.wiki/w/Copper_Chest
- https://minecraft.wiki/w/Trapped_Chest
- https://minecraft.wiki/w/Barrel
- https://minecraft.wiki/w/Ender_Chest
- https://minecraft.wiki/w/Shulker_Box, https://minecraft.wiki/w/Shulker_Box/Sounds
- https://minecraft.wiki/w/Hopper
- https://minecraft.wiki/w/Tutorial:Hopper
- https://minecraft.wiki/w/Minecart_with_Hopper
- https://minecraft.wiki/w/Dropper
- https://minecraft.wiki/w/Dispenser
- https://minecraft.wiki/w/Crafter, https://minecraft.wiki/w/Tutorial:Crafter
- https://minecraft.wiki/w/Redstone_Comparator
- https://minecraft.wiki/w/Redstone_components
- https://minecraft.wiki/w/Block_update
- https://minecraft.wiki/w/Tick
- https://minecraft.wiki/w/Java_Edition_protocol/Block_actions
- https://minecraft.wiki/w/Java_Edition_protocol/Packets (раздел World Event)
- https://minecraft.wiki/w/Java_Edition_17w47a
- https://minecraft.wiki/w/Inventory
- https://minecraft.wiki/w/Item_(entity)
- https://minecraft.wiki/w/Jukebox, https://minecraft.wiki/w/Chiseled_Bookshelf, https://minecraft.wiki/w/Composter, https://minecraft.wiki/w/Decorated_Pot, https://minecraft.wiki/w/Furnace, https://minecraft.wiki/w/Brewing_Stand, https://minecraft.wiki/w/Shelf
- https://minecraft.wiki/w/Cauldron, https://minecraft.wiki/w/Cake, https://minecraft.wiki/w/Lectern, https://minecraft.wiki/w/End_Portal_Frame, https://minecraft.wiki/w/Respawn_Anchor, https://minecraft.wiki/w/Beehive, https://minecraft.wiki/w/Command_Block, https://minecraft.wiki/w/Item_Frame, https://minecraft.wiki/w/Copper_Bulb, https://minecraft.wiki/w/Copper_Golem_Statue, https://minecraft.wiki/w/Creaking_Heart, https://minecraft.wiki/w/Detector_Rail
- https://minecraft.wiki/w/Sculk_Sensor, https://minecraft.wiki/w/Calibrated_Sculk_Sensor, https://minecraft.wiki/w/Vibration
- https://minecraft.wiki/w/Sulfur_Cube — только чтобы убедиться, что это 26.2 и исключить

Номера звуков — из `tools/blackbox/data/sounds.json` (реестр 26.1.2, таблица minecraft-data). Исходный код Minecraft, декомпилированный код, код серверных ядер (Paper, Spigot, Fabric, Forge и прочих), прокси, протокольных библиотек и модов не открывался, не цитировался и не пересказывался.
