# Правила установки и соединения блоков — Minecraft Java 1.21.x

Источники: только вики (minecraft.wiki, ru.minecraft.wiki). Исходный код Minecraft и код
серверных ядер (Paper/Spigot/Bukkit/Fabric/Forge/декомпиляции) не использовался.

Оговорка о версиях: вики сейчас описывает версии новее 1.21.x — в выдаче встречаются страницы
«Java Edition 26.3», «Bedrock Edition 26.50» и т.п. Там, где правило могло измениться после
1.21, это отмечено отдельно (важнейший случай — 24w21a/1.21 про двойные двери).

Обозначения: «цитата» — дословный текст вики; **неясно** — вики молчит или говорит невнятно.

---

## 1. ДВЕРЬ (door)

### 1.1. `facing` — куда смотрит дверь

Вывод: `facing` = **горизонтальное направление, куда смотрел игрок** в момент установки
(то есть дверь смотрит ТУДА ЖЕ, куда игрок, а не наоборот). Полотно двери при этом занимает
часть блока, ближайшую к игроку.

Цитаты (Block states → facing):
- «The direction the door's "inside" is facing.»
- «The direction the player faces while placing the door.»
- «a door facing east occupies the west part of its block when closed»

То есть «inside» двери — это та её плоскость, которая обращена к игроку; `facing` описывает
нормаль этой стороны. Если игрок смотрит на восток — `facing=east`, а сама створка стоит в
западной части блока (ближе к игроку).

Цитаты (Placement):
- «a door occupies the side of the block closest to the player»
- «or behind the player if placed in the player's own space»

Источник: https://minecraft.wiki/w/Door (разделы Block states, Placement)

### 1.2. `hinge` (left/right) — как выбирается

Определение состояния:
- `hinge` (default `left`): «Identifies the side the hinge is on (when facing the same direction
  as the door's inside).» — то есть left/right считается ОТ наблюдателя, стоящего со стороны
  «inside» и смотрящего вдоль `facing`.

Алгоритм из подраздела Placement → «Hinge placement» (порядок важен):
1. «If another matching door is adjacent, with its handle side touching the newly placed door,
   a double door forms.»
   → «This causes the new door to have its handle touching the existing door's handle, with
   hinges opposite.»
2. Если подходящего соседа-двери нет: «the hinge is placed on the side with the highest number
   of adjacent solid block faces».
3. Только Java: «if there are no blocks adjacent, or both sides fulfill a single other rule»,
   «the hinge is placed on the side closest to the players aim».

Смысл в переводе на правила: смотрим два блока, примыкающих к месту установки слева и справа
вдоль плоскости двери; у каждой стороны считаем число «adjacent solid block faces»; hinge
попадает на сторону, где их больше; при ничьей или отсутствии соседей — на сторону, ближе к
прицелу игрока.

**неясно**: что именно считает «adjacent solid block faces» — количество соседних блоков с
твёрдой гранью со стороны двери, или количество направлений, или что-то ещё. Точной
формулировки в вики нет. Также **неясно**, что такое «handle side» (сторона ручки) в терминах
состояний — расшифровки нет.

Изменение в 1.21 (24w21a → Java Edition 1.21): «Doors of different materials now default to
being placed oriented together to form a double door instead of both being placed with the hinge
on [the same side].» То есть в 1.21.x двери из РАЗНЫХ материалов по умолчанию ставятся так,
чтобы образовывать двойную дверь (раньше обе получали hinge с одной стороны).

Источник: https://minecraft.wiki/w/Door (Placement → Hinge placement),
https://minecraft.wiki/w/Java_Edition_1.21 , https://minecraft.wiki/w/Java_Edition_24w21a

### 1.3. Две половины по вертикали

Вывод: подтверждается — дверь занимает два блока: `half=lower` и `half=upper`.

Цитаты:
- «A door occupies two block spaces and both halves normally act as a single barrier.»
- «doors are actually two separate blocks»
- `half` (default `lower`): «Identifies which part of the door the block is.»
- «The bottom half of the door is placed where the player is aiming, with the top half extending
  one block above.»
- Про `/setblock`: «The lower half still works, but with graphical bugs, and the upper half does
  not.» (то есть команда создаёт только одну половину — это к вопросу о том, что половины
  реально независимые блоки)

**неясно**: прямого утверждения «facing и hinge у верхней и нижней половины совпадают» в вики
НЕТ. Косвенно так: `hinge` описан как характеристика двери в целом, `half` — как «какая это
часть». В игре на 1.21 это так, но в вики это не написано; проверять на сервере.

Источник: https://minecraft.wiki/w/Door

### 1.4. Опора снизу и случай «нет твёрдой верхней грани» (плита)

Прямое правило:
- «Doors must be supported by a full solid block face beneath them.»
- Если места или опоры нет — «no action is performed», то есть дверь не ставится вообще
  (не ставится «половинкой», не падает предметом — просто ничего не происходит).

**неясно / вывод по косвенным цитатам**: случая «плита» вики прямо не разбирает. Косвенные
цитаты:
- Solid block: «where a solid block's collision box is a full cube that is solid on all sides,
  it is known as a full block. Stone, planks and glass qualify; stairs, fences and chests
  do not.»
- Slab: «Faces described as solid include the top face of top slabs, the bottom of bottom slabs,
  and all faces of double slabs.»

Отсюда вывод (НЕ прямая цитата): у нижней плиты (`type=bottom`) верхняя грань не является
полной твёрдой гранью, поэтому дверь на неё ставить нельзя. Двойная плита — полный куб, на неё
ставить можно. На верхнюю плиту (`type=top`) — её верхняя грань в списке твёрдых, значит можно.
Правило стоит проверить в реальной игре: это моё чтение двух страниц, а не текст вики.

Источники: https://minecraft.wiki/w/Door , https://minecraft.wiki/w/Slab ,
https://minecraft.wiki/w/Solid_block

### 1.5. Щелчок по верхней половине уже существующей двери

**неясно / в вики не нашёл.** Единственное, что есть: «The bottom half of the door is placed
where the player is aiming, with the top half extending one block above.» Как при этом ведёт себя
прицел в верхнюю половину уже стоящей двери — не описано. Варианты (не додумываю, а перечисляю
возможные): (а) нижняя половина новой двери встаёт в целевой блок, верхняя — на блок выше;
(б) установка просто отклоняется, потому что целевой блок занят. Проверить в игре.

Источник: https://minecraft.wiki/w/Door

---

## 2. КНОПКА и РЫЧАГ (button, lever)

### 2.1. `face` (floor / wall / ceiling)

Определение состояния (одинаково у обеих страниц):
- `face` (default `wall`; значения `ceiling, floor, wall`): «The face of the block it's placed
  on.» + «Floor is on top of a block, ceiling is on the bottom, and wall is on one of its sides.»

Правила крепления:
- Кнопка: «Buttons can be placed by using it on a surface.» + «They can be attached to the side,
  bottom and top of any full opaque block.»
- Рычаг: «A lever can be attached to any part of most opaque blocks, or to the top of an
  upside-down slab or upside-down stairs.»

Вывод: `face` — это просто та грань, к которой прикрепились: верх блока → `floor`, низ →
`ceiling`, бок → `wall`. У рычага есть дополнительная возможность — верх перевёрнутой плиты или
перевёрнутых ступеней (тогда `face=floor`).

Пошагового текста «щёлкнул по грани X → получил face=Y» в вики нет, но правило однозначно
следует из описания состояния плюс списка допустимых поверхностей крепления.

Источники: https://minecraft.wiki/w/Button , https://minecraft.wiki/w/Lever ,
https://minecraft.wiki/w/Redstone_components («A button can be attached to any part of most
opaque blocks.» / «A lever can be attached to any part of most opaque blocks, or to the top of an
upside-down slab or upside-down stairs.»)

### 2.2. `facing` на стене — в какую сторону смотрит

Прямая цитата (обе страницы):
- «Opposite to the direction the player is facing if placed on the side of a block.»

Вывод: настенная кнопка/рычаг смотрит ОТ стены, то есть в сторону игрока; `facing` противоположен
направлению взгляда игрока в момент установки.

Источники: https://minecraft.wiki/w/Button , https://minecraft.wiki/w/Lever

### 2.3. Кнопка/рычаг на полу и потолке — какой `facing`

Рычаг (Java, косвенно — через описание ориентации и состояний on/off):
- «When placed on the top or bottom of a block, the lever orients itself in-line with the
  placing player.»
- «On the top or bottom of blocks, off is north or west, on is south or east.»

Вывод для рычага: напольный/потолочный рычаг ориентируется «в линии» с игроком (то есть по
горизонтальному направлению взгляда), а положение вкл/выкл зависит от facing: выкл = north/west,
вкл = south/east.

Кнопка: в Java-разделе вики аналогичного утверждения НЕТ. Есть только таблица для Bedrock:
«0: Button on block bottom facing down», «1: Button on block top facing up» (то есть в Bedrock
это отдельные направления вверх/вниз).

**неясно**: правило `facing` для кнопки на полу/потолке в Java Edition. Наиболее правдоподобно —
как у рычага (по направлению взгляда игрока), но это в вики не написано. Проверить в игре.

Источники: https://minecraft.wiki/w/Lever , https://minecraft.wiki/w/Button

---

## 3. ЗАБОР (fence)

Состояния: `north`, `east`, `south`, `west` (default `false`, «When true, the fence extends from
the center post to the east.» и т.д.), `waterlogged` («Whether or not there's water in the same
place as this fence.»).

### 3.1. Базовое правило соединения

- «A fence occupies the center space of blocks and automatically connects to any solid block that
  is placed next to it.»

Твёрдый блок (Solid block): «any type of block that has a collision box that players, mobs, or
other entities cannot simply move through». То есть критерий — НЕ «полный куб», а наличие
коллизии.

### 3.2. С чем соединяется — что прямо сказано в вики

- Заборы друг с другом: «Wooden fences connect to other wooden fences, but do not connect to
  Nether brick fences.»
- Со стенкой: Wall page — «Although they connect horizontally to bars and glass panes, they do not
  connect horizontally to fences (fence gates can be connected though).» → **стенка с забором НЕ
  соединяется** (и наоборот).
- С калитками: Fence Gate — «Wooden fences, nether brick fences and walls connect to fence gates,
  but glass panes and iron bars do not.»
- Общий список «неполных» блоков, к которым цепляются заборы/стенки/панели/прутья (история,
  17w15a, Blocks): «Now connect to stairs, buttons, signs, all types of rails, banners, string,
  redstone, pressure plates, levers, tripwire hooks, sticky pistons, and structure voids.» Плюс
  там же: «The rear face and underside of stairs are now considered "solid".»
- Лестницы: «Fences now connect to the solid back sides of stairs.» (история, 17w15a)

### 3.3. Список блоков-исключений (полные кубы, к которым забор не присоединяется)

**неясно — в вики такого списка нет.** Единственные явно названные «не-соединения»:
1. деревянный забор ↔ незерский забор;
2. стенка ↔ забор (см. выше);
3. забор/стенка/панель/прутья ↔ (в одну сторону) калитки соединяются, а панели и прутья к
   калиткам — нет.

Косвенные свидетельства о частных случаях (страницы версий, не правило):
- Java Edition 1.9: «Glass panes and iron bars don't connect to solid, graphically transparent
  blocks (slime block, end portal frame, and spawner)» — то есть слизневой блок, рамка портала
  Края и спавнер упоминаются как полные, но «графически прозрачные» блоки, с которыми связи
  может не быть.
- Java Edition 1.12/Development versions: в записи 17w15a есть починка MC-115805
  «Fences/panes/walls/bars/torches connect to a number of non-solid blocks» и MC-10613
  «Fence doesn't connect with stairs».
- Вне вики (трекер Mojang, bugs.mojang.com, MC-117450): «Fences and walls now connect to melons,
  pumpkins, and jack-o-lanterns» (исправлено в 1.12 Pre-Release 3). Это не вики, поэтому
  привожу как вспомогательный источник.

Про листву, тыкву, арбуз, шалкер: **в вики прямых утверждений нет.** По базовому правилу
(«any solid block», то есть есть коллизия) все они твёрдые, значит должны соединяться; шалкер
имеет неполную по высоте коллизию, поэтому именно для него **неясно**. В вики не нашёл.

### 3.4. Забор и блок с твёрдой только верхней гранью (нижняя плита)

**неясно.** Прямого ответа нет. Замечание: соединение забора привязано к свойству «solid block»
(наличие коллизии), а не к конкретной грани, поэтому по букве правила — скорее да, но это не
написано. Для панелей в истории вики есть явное упоминание плит («stone slabs»), для забора —
нет.

### 3.5. Плиты / ступени

- Ступени: прямо подтверждено историей («Fences now connect to the solid back sides of stairs»,
  «The rear face and underside of stairs are now considered "solid"»).
- Плиты: прямого утверждения для забора в вики нет. Для панелей — есть (см. п. 5).
  Помечаю как **неясно для забора**, хотя логика соединения у забора/панели/стенки общая.

Источники: https://minecraft.wiki/w/Fence , https://minecraft.wiki/w/Wall ,
https://minecraft.wiki/w/Fence_Gate , https://minecraft.wiki/w/Solid_block ,
https://minecraft.wiki/w/Java_Edition_1.12/Development_versions ,
https://minecraft.wiki/w/Java_Edition_1.9 , https://ru.minecraft.wiki/Забор

---

## 4. СТЕНКА (cobblestone_wall)

Состояния: `north/east/south/west` — default `none`, значения `none, low, tall`
(«How the wall extends from the center post to the east.»); `up` — default `true`;
`waterlogged`.

### 4.1. Когда `up` = true, а когда false

Определение: `up` — «When true, the wall has a center post.»

Правила (дословно):
- «A wall block always has a center post connecting to the top if covered by a banner, a pressure
  plate, a ground torch, soul torch or redstone torch, a sign, tripwire, another wall block with a
  center post, or any block covering the four pixels at the center on its top — without making the
  wall block raise its top on any two opposite sides.»
- «A wall block has a center post even if not covered by the blocks mentioned above, unless
  connecting to only two opposite sides or all four sides.»
- «Without center posts, walls connect to other walls to create a large, flat wall.»

Вывод:
- `up=true` (столб) — по умолчанию, в том числе когда сверху стоит блок из списка выше
  (баннер, нажимная плита, факел/редстоун-факел, табличка, растяжка, другая стенка со столбом,
  либо любой блок, накрывающий 4 центральных пикселя) и при этом стенка не «поднимает верх» с
  двух противоположных сторон.
- `up=false` — только если соединения ровно с двумя ПРОТИВОПОЛОЖНЫМИ сторонами (прямой прогон)
  или со всеми четырьмя сторонами. В остальных случаях (одно соединение, три, два соседних —
  угол) столб есть.

**неясно**: точная механика условия «without making the wall block raise its top on any two
opposite sides» — в вики не расшифровано, что значит «raise its top».

ru-вики даёт то же состояние короче: `up` (default true) — «Ограда имеет середину высотой в
блок.»

### 4.2. Соединения по бокам

- «A wall block automatically connects to any horizontally adjacent connectable block surface,
  bars or glass pane.» → стенка соединяется со стенками, прутьями и стеклянными панелями
  (а также с «connectable block surface» — соединяемой поверхностью блока).
- «they do not connect horizontally to fences (fence gates can be connected though)» → с забором
  НЕ соединяется, с калитками — да.
- «The top of a connect-to-side part rises slightly to support any block immediately above» —
  если блок сверху накрывает 2-пиксельную полосу от центра к этой стороне.
- ru-вики: «Ограды соединяются друг с другом и с воротами, но не с забором.»
- История: 17w15a — «Булыжные ограды теперь соединяются с полными сторонами блоков ступеней»
  (соединение со ступенями).

### 4.3. Как выглядит стенка без соединений

- «A wall block has a center post even if not covered by the blocks mentioned above...» —
  одиночная стенка выглядит как столб.
- «Without center posts, walls connect to other walls to create a large, flat wall.» —
  без столбов (прямой прогон) стенки сливаются в плоскую стену.
- Высота: коллизия стенки — 1,5 блока (ru-вики: «1,5 блока»).
- Побочная деталь из ru-вики: если убрать верхнюю часть дебаг-палочкой и обрезать соединения,
  блок стенки становится невидимым и неосязаемым (но всё ещё держит воду и рост растений).

Источники: https://minecraft.wiki/w/Wall , https://ru.minecraft.wiki/Стена

---

## 5. ПАНЕЛЬ (glass_pane, iron_bars)

Состояния: `north/east/south/west` (default `false`, «When true, the glass pane extends from the
center post to the east.»), `waterlogged`. У прутьев то же, но формулировка «the bars extend».

Поведение: «Bars can be placed in much the same way as fences or glass panes.» и «If no solid
block is adjacent to the bars, they have appearance of a slim, tall column.» — то есть без
соседей это тонкий столбик.

С чем соединяется (в основном по истории вики, так как текущий текст страниц списка не даёт):
- Beta 1.9 Prerelease 4 (Glass Pane): панели «attach to some non-full blocks» — перечислены
  «stone slabs, stairs, pistons, sticky pistons, farmland, iron bars, doors, signs, and fences.»
  → здесь есть и ПЛИТЫ, и СТУПЕНИ, и ЗАБОРЫ.
- 13w41a: «All glass panes now connect to glass and iron bars.»; «Iron bars now connect to glass
  panes.»
- 17w15a: «Glass panes now connect to the solid back sides of stairs.» / «Iron bars now connect to
  the solid back sides of stairs.» Плюс общий список (см. п. 3.2): stairs, buttons, signs, rails,
  banners, string, redstone, pressure plates, levers, tripwire hooks, sticky pistons, structure
  voids.
- 20w15a: «Walls now connect to the bottom and sides of glass panes.» и «Walls now connect to the
  bottom and sides of iron bars.» — это connection в сторону стенки.
- Калитки: «glass panes and iron bars do not [connect to fence gates]» (Fence Gate).

Ответы на конкретные вопросы:
- Панель ↔ забор: **в текущем тексте вики прямого утверждения нет**, но в истории панелей
  (Beta 1.9) fences в списке соединяемых. Формально помечаю как «есть только в истории, для
  1.21.x прямой цитаты нет».
- Панель ↔ стенка: страница Wall говорит «A wall block automatically connects to any horizontally
  adjacent connectable block surface, bars or glass pane», то есть стенка соединяется с панелью.
  Обратное направление (панель со стенкой) прямо не сформулировано, но по симметрии и по
  формулировке Wall — да. Формально: **в вики прямо не сказано про направление панель→стенка**.
- Панель ↔ железные прутья: да, прямо (13w41a).
- Панель ↔ полные блоки: «A fence occupies the center space...» формулировка для панели в вики
  дана как «If no solid block is adjacent...» — то есть да, с твёрдыми блоками рядом.

Источники: https://minecraft.wiki/w/Glass_Pane , https://minecraft.wiki/w/Iron_Bars ,
https://minecraft.wiki/w/Bars , https://minecraft.wiki/w/Wall ,
https://minecraft.wiki/w/Fence_Gate

---

## 6. ЗАПРЕТ НА УСТАНОВКУ ВНУТРЬ СУЩЕСТВА

**Главный вывод: отдельной страницы/раздела с общим правилом «нельзя поставить блок в место,
занятое игроком/мобом» в вики НЕТ.** Проверены: Block, Opacity, Solid block, Hitbox (по поиску) —
ни на одной нет раздела о размещении с таким правилом. Точную формулировку процитировать неоткуда.

Что удалось найти как подтверждение, что правило существует:

1. Страница «Place Block» (первоапрельская шутка, 2024) перечисляет причины, по которым
   размещение отклоняется. Дословно: «The space on the output side is blocked by an entity.»
   И тут же исключение: «However, non-solid blocks like flowers can be placed.»
   Дословно: «The space on the output side is occupied by another block.» и исключение
   «if the space is occupied by a replaceable block, like short grass or water, another block can
   be placed».
   Страница: https://minecraft.wiki/w/Place_Block

2. Правка в 17w47a / Java Edition 1.13 — баг MC-2208: «Blocks with special placement can be
   placed inside player/entity» (помечен как исправленный). Смысл: штатно блоки внутри
   игрока/сущности ставить нельзя, а блоки «со специальным размещением» (кровати, лилии и т.п.)
   это позволяли — и это считалось багом. На странице «Bed» тот же MC-2208 указан как
   «resolved as "Fixed"».
   Страницы: https://minecraft.wiki/w/Java_Edition_17w47a , https://minecraft.wiki/w/Bed ,
   https://minecraft.wiki/w/Java_Edition_1.13

3. Страница «Armor Stand» описывает возможность поставить стойку для брони в занятое место как
   исключение из обычных правил: «they can place armor stands inside solid blocks, or if the
   needed space is occupied by other entities».
   Страница: https://minecraft.wiki/w/Armor_Stand

4. «Parity issue list»: «Blocks that require solid blocks for support will break if placed on a
   door or trapdoor that is then opened.» — про смежную тему (блок «внутри» двери).
   Страница: https://minecraft.wiki/w/Parity_issue_list

**Проверяется ли форма блока (полный куб) или что-то ещё — неясно.** Прямого утверждения в вики
нет. Косвенно, из формулировки «non-solid blocks like flowers can be placed» (Place Block):
проверка, похоже, идёт по коллизии/твёрдости блока, а не по «полный куб». Но насколько широко
толкуется «non-solid blocks like flowers» (факелы? рельсы? растяжки?) — в вики не сказано.

**Итог по п.6: в вики не нашёл** нормативной формулировки. Есть только косвенные подтверждения
и одна формулировка на странице-шутке.

---

## 7. ПЛИТА и СТУПЕНИ

### 7.1. Ступени — `shape` при установке

Состояния: `facing` (default `north`), `half` (default `bottom`), `shape` (default `straight`,
значения `inner_left, inner_right, outer_left, outer_right, straight`), `waterlogged`.
Описание `shape`: «"straight" is the default stairs shape», «"inner" is an "inside corner" stair
shape, with two full-block and two stair-shaped side faces», «"outer" is an "outside corner" stair
shape», «"left" and "right" specify in which direction is the higher part of the step».

Соединение соседей:
- «Stairs change their shape to join with adjacent stairs (of any material)»
- inner: «the stairs' full-block side wraps into an "L" shape to join the other stairs»
- outer: «the stairs' full-block side shortens to join the other stairs' full-block side»
- «Right side up stairs do not join with upside-down stairs and vice versa.»

Автообновление формы: история 12w34a — «Stairs now automatically change shape into corner stairs
depending on location.» Плюс в Bedrock-истории: изменение corner-состояния теперь вызывает
block updates «for parity with Java Edition» (для миров с базовой версией 1.26.40+).

**неясно**: вики НЕ утверждает явно, что «при установке shape всегда straight, а inner/outer
появляется позже при обновлении соседей». Сказано только, что `straight` — значение по
умолчанию, и что форма меняется автоматически в зависимости от расположения. Ваш вариант
правдоподобен и согласуется с текстом (default + автосмена), но это ваш вывод, а не цитата вики.

Установка (ориентация): «a stair orients itself with the half-block side closest to the player»;
right-side-up — если целиться в верх блока или в нижнюю половину боковой грани; upside-down —
если целиться в низ блока или в верхнюю половину боковой грани. `facing` — «The direction the
stairs' full-block side faces», при установке «matches the direction the player faces».
`half` — «Top if the stairs are upside-down.»

### 7.2. Плита — что может пригодиться

Состояния: `type` = `bottom` / `top` / `double` — «Where the slab is within its block»,
`waterlogged` — «Whether or not there is water in the same place as this slab».

Правила установки (дословно):
- «Placing a slab on top of a block or on the side of a block in the lower half of the side
  surface creates a bottom slab.»
- «Placing a slab on the underside of a block or on the top half of the side surface creates a top
  slab.»
- «Placing a top and bottom slab of the same type in the same block creates a double slab block.»
- «Slabs cannot be oriented vertically.»

Поведение, которое легко упустить:
- «blocks that require a solid surface for placement can be placed on these faces», где «these
  faces» — верхняя грань верхней плиты, низ нижней плиты и все грани двойной плиты.
- «Water in a single slab's empty half makes it waterlogged».
- Мобы не спавнятся на нижних плитах, но спавнятся на верхних и двойных; падающие блоки на
  нижней плите превращаются в предметы; предметы проваливаются сквозь нижнюю плиту в воронку
  снизу.
- Приседание опускает хитбокс игрока до 1,5 блоков (важно для «пролезания»).

Смешивать два разных типа плит в одном блоке обычно нельзя — только пистоном у границы мира.

Источники: https://minecraft.wiki/w/Stairs , https://minecraft.wiki/w/Slab

---

## Что проверить не удалось (сводный список)

1. Точный смысл «adjacent solid block faces» в правиле выбора `hinge` у двери (и что такое
   «handle side»).
2. Прямое утверждение, что `facing`/`hinge` верхней и нижней половины двери совпадают — в вики
   не написано.
3. Ставится ли дверь на нижнюю плиту — только вывод из двух косвенных цитат (Door + Slab/Solid
   block), прямой цитаты нет.
4. Поведение при щелчке по верхней половине уже стоящей двери — не описано.
5. `facing` кнопки на полу/потолке в Java Edition — в вики нет (есть только Bedrock-таблица).
6. Полный список блоков-исключений, к которым забор не присоединяется, хотя они полные кубы —
   такого списка в вики НЕТ. Явно названы только: деревянный забор ↔ незерский забор,
   стенка ↔ забор.
7. Забор ↔ плиты, забор ↔ листва/тыква/арбуз/шалкер — прямых утверждений в вики нет; для
   тыквы/арбуза есть только запись в трекере Mojang (MC-117450, не вики).
8. Забор ↔ блок с твёрдой только верхней гранью — в вики нет.
9. Панель → стенка и панель → забор в текущей версии: прямых цитат нет, только обратное
   направление (Wall → pane) и история версий.
10. Формальное утверждение «при установке ступени всегда получают shape=straight» — в вики нет,
    есть только «default = straight» и «автоматически меняют форму».
11. Нормативное правило «нельзя ставить блок в место, занятое сущностью» — на страницах о
    размещении блоков в вики отсутствует; есть только косвенные подтверждения (MC-2208,
    страница-шутка Place Block, Armor Stand).
12. Точное условие «without making the wall block raise its top on any two opposite sides» для
    `up` у стенки — не расшифровано.

## Использованные страницы вики

- https://minecraft.wiki/w/Door
- https://minecraft.wiki/w/Button
- https://minecraft.wiki/w/Lever
- https://minecraft.wiki/w/Fence
- https://minecraft.wiki/w/Fence_Gate
- https://minecraft.wiki/w/Wall
- https://minecraft.wiki/w/Glass_Pane
- https://minecraft.wiki/w/Iron_Bars
- https://minecraft.wiki/w/Bars
- https://minecraft.wiki/w/Slab
- https://minecraft.wiki/w/Stairs
- https://minecraft.wiki/w/Solid_block
- https://minecraft.wiki/w/Opacity
- https://minecraft.wiki/w/Block
- https://minecraft.wiki/w/Place_Block
- https://minecraft.wiki/w/Redstone_components
- https://minecraft.wiki/w/Armor_Stand
- https://minecraft.wiki/w/Bed
- https://minecraft.wiki/w/Java_Edition_1.21
- https://minecraft.wiki/w/Java_Edition_24w21a
- https://minecraft.wiki/w/Java_Edition_1.12/Development_versions
- https://minecraft.wiki/w/Java_Edition_1.9
- https://minecraft.wiki/w/Java_Edition_17w47a
- https://minecraft.wiki/w/Parity_issue_list
- https://ru.minecraft.wiki/Забор
- https://ru.minecraft.wiki/Стена
