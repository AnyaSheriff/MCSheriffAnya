# Хранилища по сети: окна, блок-сущности, крышка сундука (Java 775 и Bedrock 944)

Справочник для сундуков, бочек, воронок, раздатчиков, выбрасывателей и крафтера:
как открыть окно, показать и поменять содержимое, разобрать щелчки игрока,
показать блок-сущность в чанке и анимацию крышки. Только сетевая сторона.

Версии: Java Edition 26.1.2 (протокол **775**) и Bedrock 26.10–26.13 (протокол **944**).
Ничего из Java 26.2+ здесь нет.

## Источники

Только разрешённое: данные minecraft-data (JSON, не их JS), minecraft.wiki,
документация Mojang к Bedrock, записи нашего чёрного ящика (официальный сервер,
наблюдаемый снаружи по сети).

- **MD-J** — `minecraft-data/data/pc/26.1/protocol.json`. Какая папка отвечает
  за 26.1, видно в `data/dataPaths.json` (`pc["26.1"].protocol = "pc/26.1"`),
  а `pc/26.1/version.json` даёт `"version": 775`. Локальная копия:
  `~/node_modules/minecraft-data/minecraft-data/data/pc/26.1/protocol.json`.
  Пути внутри: `play.toClient.types.packet` → `mappings` (номера пакетов),
  `play.toClient.types.packet_<имя>` (поля), `types.<Тип>` (общие типы).
- **MD-B** — `minecraft-data/data/bedrock/1.26.10/protocol.json`,
  `bedrock/1.26.10/version.json` → `"version": 944` (ровно наш протокол).
  Пути: `types.mcpe_packet` → `mappings`, `types.packet_<имя>`, `types.<Тип>`.
- **WIKI-P** — minecraft.wiki, `Java Edition protocol/Packets`, ревизия
  **3647471** (25.06.2026, последняя перед правками под 26.2; с 21.04.2026 на
  странице номера под 26.1.2). Копия: `~/.cache/mcsheriffanya/wiki/JEP_Packets_3647471.raw`.
- **WIKI-I** — `Java Edition protocol/Inventory`, ревизия 3201379 (14.10.2025;
  после неё страницу не правили, то есть она действует и для 26.1.2).
  Копия: `~/.cache/mcsheriffanya/wiki/JEP_Inventory.raw`.
- **WIKI-S** — `Java Edition protocol/Slot data`, ревизия 3560807 (04.05.2026), § Hashed Format.
- **WIKI-BA** — `Java Edition protocol/Block actions`, ревизия 3127115, § Chest, § Shulker Box.
- **WIKI-BEF** — `Block entity format` (Java), ревизия 3630225; `Chest`, ревизия 3608904
  (§ Data values / Block data).
- **WIKI-BBEF** — `Bedrock Edition level format/Block entity format` (ревизия 3778756),
  `Bedrock Edition level format/Item format` (ревизия 3793794); строковые id блок-сущностей
  Bedrock — из таблиц «ID» на страницах блоков (`Chest`, `Barrel`, `Hopper`, …, шаблон
  `ID table|displayname=Block entity`).
- **MOJ** — документация Mojang к r/26_u1 (протокол 944):
  `~/.cache/mcsheriffanya/mojang-docs/r26u1/json/<Пакет>.json`,
  `…/dot/<Тип>.dot`, `…/html/enums.html`,
  `…/additional_docs/ServerAuthInventory/ItemStackPackets_Disabled.md`,
  `…/additional_docs/SubChunk Request System v1.18.10.md`.
- **BB** — записи чёрного ящика с официального сервера 26.1.2:
  `tools/blackbox/reports/vanilla-chest-open.json`, `vanilla-trapped-chest.json`,
  `vanilla-hopper-chain.json`, `vanilla-note-instruments.json` и др.

---

## Что уже есть у нас (для привязки)

- `src/network/play.rs`: номера пакетов — константами в начале файла.
  `SET_CONTAINER_CONTENT = 0x12`, `SET_CONTAINER_SLOT = 0x14`, `CLICK_CONTAINER = 0x12`
  (от клиента), `CLOSE_CONTAINER = 0x13` (от клиента), `BLOCK_ACTION = 0x07`,
  `CHUNK_DATA = 0x2D`, `PLAYER_WINDOW = 0`. Все номера сходятся с MD-J.
- `send_container_content` / `send_container_slot`: окно 0, перед каждой отправкой
  `player.state_id += 1`, слот записывает `push_slot` (без компонентов).
- `read_click`: читает окно, номер состояния, слот (`u16` → `i16`), кнопку (`u8` → `i8`)
  и режим. Остальное (изменённые слоты с хэшами, курсор) пропускает: сервер всё равно
  отвечает своим содержимым. Если окно не 0, щелчок сейчас отбрасывается.
- `send_chunk`: список блок-сущностей в Chunk Data — пока всегда `varint 0`.
- `src/inventory.rs`: раскладка окна 0 — 0 итог, 1–4 сетка 2×2, 5–8 броня,
  9–35 рюкзак, 36–44 панель, 45 вторая рука (совпадает с WIKI-I § Player Inventory).
- `src/bedrock/inventory.rs`: `Bag`, разбор Item Stack Request (действия 0/1/7/8 —
  перемещение, 2 — обмен, 3 — выброс, 4/5, 6, 11, 14, 19), виды контейнеров
  12/28/29/6/34/59/60, ответ повторяет имена контейнеров так, как их назвал клиент
  (`write_containers`), динамический номер читается как `u32` big-endian.
- `src/bedrock/world.rs`: `open_inventory` шлёт Container Open для своего инвентаря
  (окно 0, вид −1); на Container Close от клиента отвечает тем же с `server = false`;
  `encode_chunk` после биомов пишет `u8(0)` («блоков на краю нет») — блок-акторов нет.
- `src/network/nbt.rs`: сетевой NBT Java (корень без имени), типы byte/int/float/double/
  string/list/compound. Для предметов в блок-сущностях понадобятся ещё `short`
  и, возможно, `long`.

---

# Часть 1. Java Edition 26.1.2 (протокол 775)

## 1.1. Номера пакетов

Все номера — из MD-J (`play.toClient/toServer.types.packet`, `mappings`),
имена в скобках — как в minecraft-data. Сверено с WIKI-P по составу полей.

| Направление | Номер | Пакет (вики) | Имя в minecraft-data |
|---|---|---|---|
| клиенту | **0x06** | Block Entity Data | `tile_entity_data` |
| клиенту | **0x07** | Block Action | `block_action` |
| клиенту | **0x11** | Close Container | `close_window` |
| клиенту | **0x12** | Set Container Content | `window_items` |
| клиенту | **0x13** | Set Container Property | `craft_progress_bar` |
| клиенту | **0x14** | Set Container Slot | `set_slot` |
| клиенту | **0x29** | Open Horse Screen | `open_horse_window` |
| клиенту | **0x2D** | Chunk Data and Update Light | `map_chunk` |
| клиенту | **0x3B** | Open Screen | `open_window` |
| клиенту | 0x60 | Set Cursor Item | `set_cursor_item` |
| клиенту | 0x6C | Set Player Inventory Slot | `set_player_inventory` |
| серверу | **0x11** | Click Container Button | `enchant_item` |
| серверу | **0x12** | Click Container | `window_click` |
| серверу | **0x13** | Close Container | `close_window` |
| серверу | 0x14 | Change Container Slot State (крафтер) | `set_slot_state` |
| серверу | 0x42 | Use Item On (щелчок по блоку) | `block_place` |

Внимание: клиентский Close Container — **0x11**, а серверный — **0x13**; у нас пока
есть только серверный.

## 1.2. Open Screen (0x3B, клиенту)

MD-J `play.toClient.types.packet_open_window`; WIKI-P § Open Screen.

| Поле | Тип | Смысл |
|---|---|---|
| Window ID | VarInt | номер окна. У оригинала — счётчик с 1, по кругу после 100 (WIKI-P). 0 — не использовать: это инвентарь игрока |
| Window Type | VarInt | номер в реестре `minecraft:menu` |
| Window Title | Text Component (сетевой NBT, в MD-J — `anonymousNbt`) | заголовок |

Своего инвентаря сервер не открывает: окно 0 «открыто» всегда, когда нет другого
(WIKI-P). Повторная отправка с тем же номером меняет заголовок и вид без закрытия.

### Номера `minecraft:menu` (WIKI-I § Types)

Реестр встроенный, по сети не приходит. Страница не менялась с октября 2025 года,
в 26.1 меню не добавлялись.

| Номер | Имя | Кто открывает |
|---|---|---|
| 0 | `generic_9x1` | — |
| 1 | `generic_9x2` | — |
| **2** | **`generic_9x3`** | сундук, сундук-ловушка, бочка, эндер-сундук, вагонетка с сундуком |
| 3 | `generic_9x4` | — |
| 4 | `generic_9x5` | — |
| **5** | **`generic_9x6`** | большой (двойной) сундук |
| **6** | **`generic_3x3`** | раздатчик, выбрасыватель |
| **7** | **`crafter_3x3`** | крафтер |
| 8 | `anvil` | |
| 9 | `beacon` | |
| 10 | `blast_furnace` | |
| 11 | `brewing_stand` | |
| 12 | `crafting` | верстак |
| 13 | `enchantment` | |
| 14 | `furnace` | |
| 15 | `grindstone` | |
| **16** | **`hopper`** | воронка, вагонетка с воронкой |
| 17 | `lectern` | |
| 18 | `loom` | |
| 19 | `merchant` | |
| 20 | `shulker_box` | |
| 21 | `smithing` | |
| 22 | `smoker` | |
| 23 | `cartography_table` | |
| 24 | `stonecutter` | |

Подтверждено чёрным ящиком (BB, `vanilla-chest-open.json`): двойной сундук открылся
окном вида **5**, бочка — вида **2**; в `vanilla-hopper-chain.json`,
`vanilla-trapped-chest.json`, `vanilla-dropper-pulse.json` сундуки — вида **2**.

Заголовок оригинал шлёт как `{"translate": "<ключ>"}`. Ключи (minecraft-data
`pc/26.1/language.json`): `container.chest` («Chest»), `container.chestDouble`
(«Large Chest»), `container.barrel`, `container.hopper` («Item Hopper»),
`container.dispenser`, `container.dropper`, `container.crafter`,
`container.enderchest`, `container.shulkerBox`. Если у блока есть `CustomName`,
заголовком идёт он.

## 1.3. Set Container Content (0x12, клиенту)

MD-J `packet_window_items`; WIKI-P § Set Container Content.

| Поле | Тип | Смысл |
|---|---|---|
| Window ID | VarInt (`ContainerID` = varint) | окно; 0 — инвентарь игрока |
| State ID | VarInt | номер состояния, см. § 1.9 |
| Slot Data | Prefixed Array (длина VarInt) of Slot | все слоты окна по порядку, **включая** часть инвентаря игрока |
| Carried Item | Slot | что игрок держит курсором |

Клиент игнорирует пакеты для окна, которое у него сейчас не открыто (кроме окна 0).

Формат Slot (MD-J `types.Slot`; WIKI-S § Format): `VarInt count`; если count > 0 —
`VarInt item_id`, `VarInt число добавленных компонентов`, `VarInt число убранных`,
затем сами компоненты. У нас это `push_slot`.

## 1.4. Set Container Slot (0x14, клиенту)

MD-J `packet_set_slot`; WIKI-P § Set Container Slot.

| Поле | Тип |
|---|---|
| Window ID | VarInt |
| State ID | VarInt |
| Slot | **Short** (i16) |
| Slot Data | Slot |

Важная оговорка WIKI-P: пока открыто чужое окно, **оригинал не шлёт обновлений окну 0** —
все виды окон содержат слоты инвентаря игрока, и клиент сам переносит изменения этих
слотов в основной инвентарь. Броня и вторая рука есть только в окне 0, поэтому их
изменения, случившиеся при открытом окне, сервер должен **отложить до закрытия**.
Исключение для окна 0 (слоты 36–45 можно обновлять и при открытом окне) у клиента есть,
но оригинал им не пользуется.

## 1.5. Set Container Property (0x13, клиенту)

MD-J `packet_craft_progress_bar`; WIKI-P § Set Container Property.

| Поле | Тип |
|---|---|
| Window ID | VarInt |
| Property | Short |
| Value | Short |

Для сундука, бочки, воронки, раздатчика не нужен. Для крафтера: свойства **0–8** —
«слот выключен» (1) / «включён» (0), свойство **9** в таблице вики тоже отнесено
к крафтеру («0-9: Slot Disabled»), вероятно, это признак «есть итог» — не проверено.

## 1.6. Close Container (0x11 клиенту, 0x13 серверу)

MD-J `play.toClient.types.packet_close_window` и `play.toServer.types.packet_close_window`;
WIKI-P § Close Container (оба).

Оба пакета — одно поле: `Window ID: VarInt`.

- Клиенту: принудительное закрытие (например, сундук сломали, пока он открыт).
  Клиент **номер окна не смотрит** и закрывает любое открытое.
- Серверу: игрок закрыл окно. Свой инвентарь клиент тоже «закрывает» — пакетом
  с окном 0, хотя Open Screen для него никогда не было.

## 1.7. Click Container (0x12, серверу)

MD-J `play.toServer.types.packet_window_click`; WIKI-P § Click Container;
Hashed Slot — MD-J `types.HashedSlot`, WIKI-S § Hashed Format.

| # | Поле | Тип | Смысл |
|---|---|---|---|
| 1 | Window ID | VarInt | окно, по которому щёлкнули |
| 2 | State ID | VarInt | последний номер состояния, полученный клиентом (Set Container Slot / Content) |
| 3 | Slot | **Short** | номер слота в нумерации окна; −999 — мимо окна |
| 4 | Button | **Byte** (i8) | кнопка |
| 5 | Mode | VarInt | вид щелчка 0–6 |
| 6 | Changed Slots | Prefixed Array (длина VarInt, не больше 128) of { `Slot: Short`, `Item: Hashed Slot` } | как клиент **думает**, изменились слоты |
| 7 | Carried Item | Hashed Slot | что, по мнению клиента, теперь в курсоре |

**Hashed Slot** (WIKI-S): у предметов в этом пакете компоненты передаются не
значениями, а контрольными суммами CRC32C.

| Поле | Тип | Когда есть |
|---|---|---|
| Has Item | Boolean | всегда |
| Item ID | VarInt | если Has Item |
| Item Count | VarInt | если Has Item |
| Components to add | Prefixed Array of { `Type: VarInt`, `Hash: Int` (4 байта, big-endian) } | если Has Item |
| Components to remove | Prefixed Array of `Type: VarInt` | если Has Item |

В MD-J это же записано как `["option", "HashedSlot"]`, где `option` — булев префикс,
а внутри `itemId, itemCount, components[{type, hash:i32}], removeComponents[{type}]`.
Порядок полей внутри: **сначала id, потом count** (в обычном Slot — наоборот!).

Как считается хэш, вики не описывает («currently undocumented»). Нам он не нужен:
для предметов без своих компонентов (а у нас пока только такие) оба массива пусты.
Сервер вправе не сверять хэши, а после щелчка просто прислать правильное содержимое.

Как поступает оригинал (WIKI-P): выполняет щелчок у себя, сравнивает итог с
присланными изменёнными слотами (наложенными на своё прежнее состояние) и **на каждый
несовпавший слот шлёт Set Container Slot**. Если State ID не совпал с последним
отправленным — вместо этого шлёт Set Container Content целиком.

Нам проще: как и сейчас для окна 0, после каждого щелчка присылать всё окно заново
(Set Container Content). Это надёжно, но тяжелее; для двойного сундука это 90 слотов.

Режимы и кнопки (WIKI-P § Click Container, совпадают с тем, что уже разбирает
`Inventory::click`):

| Mode | Button | Slot | Что |
|---|---|---|---|
| 0 | 0 / 1 | слот | левая / правая кнопка |
| 0 | 0 / 1 | −999 | выбросить всю стопку / одну штуку из курсора |
| 1 | 0 / 1 | слот | Shift + щелчок (поведение одно) |
| 2 | 0–8 | слот | клавиша 1–9 (обмен со слотом панели) |
| 2 | 40 | слот | F — обмен со второй рукой |
| 3 | 2 | слот | средняя кнопка (только творческий, только не своё окно) |
| 4 | 0 / 1 | слот | Q / Ctrl+Q |
| 5 | 0,4,8 / 1,5,9 / 2,6,10 | −999 / слот / −999 | «раскраска» перетаскиванием: начало, добавить слот, конец (левая/правая/средняя) |
| 6 | 0 | слот | двойной щелчок (собрать одинаковые) |

Shift+щелчок в окне сундука перекладывает между частью хранилища и частью игрока —
это новая ветка для `quick_move`: у окна 0 она ходит между рюкзаком и панелью.

## 1.8. Click Container Button (0x11, серверу)

MD-J `packet_enchant_item`; WIKI-P § Click Container Button.

| Поле | Тип |
|---|---|
| Window ID | VarInt |
| Button ID | VarInt |

Для сундуков, воронок, раздатчиков, крафтера не приходит (у крафтера включение слотов —
через Click Container по пустому слоту без предмета в курсоре, WIKI-I § Crafter,
и отдельный пакет Change Container Slot State 0x14: `slot_id: VarInt`,
`window_id: VarInt`, `state: Boolean`, MD-J `packet_set_slot_state`).
Используется кафедрой (1 — назад, 2 — вперёд, 3 — взять книгу, 100+стр.),
столом зачарования, камнерезом, ткацким станком.

## 1.9. State ID — как с ним обращаться

По WIKI-P (§ Set Container Content, § Set Container Slot, § Click Container):

- Номер ведёт сервер. Каждая отправка Set Container Content / Set Container Slot
  несёт номер; клиент запоминает последний полученный.
- В Click Container клиент возвращает этот последний номер.
- Не совпал с последним отправленным → оригинал шлёт полную Set Container Content.
- Номер — на окно (меню). Наш `player.state_id` один на игрока и растёт перед каждой
  отправкой — это тоже годится, если сравнивать с ним. Сейчас мы номер из щелчка
  не читаем вовсе (`_state_id`) и всегда отвечаем полным содержимым — рассинхрон
  от этого не страшен.

## 1.10. Нумерация слотов окна хранилища

WIKI-I: «слоты считаются с 0: сначала собственные слоты окна, потом инвентарь игрока».
Верхний левый слот инвентаря игрока в любом окне — `n`, где `n` — число собственных
слотов. −999 — мимо окна. У окон хранилищ **нет** слотов брони, второй руки и сетки 2×2.

| Окно (вид) | Собственные | Рюкзак игрока | Панель игрока | Всего в Set Container Content |
|---|---|---|---|---|
| `generic_9x3` (2), `shulker_box` (20) | 0–26 | 27–53 | 54–62 | 63 |
| `generic_9x6` (5) | 0–53 | 54–80 | 81–89 | 90 |
| `generic_3x3` (6) | 0–8 (`x + 3·y`) | 9–35 | 36–44 | 45 |
| `hopper` (16) | 0–4 | 5–31 | 32–40 | 41 |
| `crafter_3x3` (7) | 0–8 (`x + 3·y`), **итог — 45** | 9–35 | 36–44 | 46 |

Перевод в нашу раскладку окна 0 (`src/inventory.rs`) при `n` собственных слотах:

- слот окна `n + k`, k = 0…26 → рюкзак `9 + k`;
- слот окна `n + 27 + k`, k = 0…8 → панель `36 + k`.

Чёрный ящик подтверждает: в окне бочки (вид 2) предметы с панели робота пришли в
слотах 54–60, в окне двойного сундука (вид 5) — в слотах 81–87 (BB,
`vanilla-chest-open.json`, события `slots`).

Двойной сундук: какая половина (`type=left` или `type=right`) даёт слоты 0–26,
вики не говорит. Проверить чёрным ящиком: положить предмет командой в одну
половину и посмотреть номер слота в окне.

## 1.11. Block Entity Data (0x06, клиенту)

MD-J `packet_tile_entity_data`; WIKI-P § Block Entity Data.

| Поле | Тип | Смысл |
|---|---|---|
| Location | Position (long: X 26 бит, Z 26, Y 12) | место блока |
| Type | VarInt | номер в реестре `minecraft:block_entity_type` (в MD-J поле названо `action`) |
| NBT Data | NBT (сетевой, корень без имени; в MD-J `anonOptionalNbt` — может быть пустым тегом End) | данные |

### Номера `minecraft:block_entity_type` — чего нет и что известно

Числового порядка этого реестра **нет ни на вики, ни в minecraft-data** (это уже
выяснено в `tools/research/moving-piston.md` § 4 и перепроверено сейчас). Страница
`Block entity format` даёт только алфавитный список строковых id, он не является
порядком реестра.

Достоверно известно одно число — снято чёрным ящиком с оригинала 26.1.2:

| Строковый id | Номер | Откуда |
|---|---|---|
| `minecraft:hanging_sign` | **8** | BB `vanilla-note-instruments.json`: `setblock … cherry_hanging_sign` → Block Entity Data с `type: 8` и NBT с `front_text`/`back_text` |

Строковые id нужных нам типов (WIKI-BEF § Types и таблицы «ID» на страницах блоков):
`chest` (и медные сундуки), `trapped_chest`, `ender_chest`, `barrel`, `hopper`,
`dropper`, `dispenser`, `crafter`, `lectern`, `jukebox`, `sign`, `hanging_sign`,
`shulker_box`.

Как получить остальные номера без чтения кода — **два пути, оба требуют решения
пользователя**:

1. Штатный генератор отчётов официального jar (`--reports`), как уже сделано для
   `tools/blackbox/data/sounds.json` (см. `tools/blackbox/README.md`): в
   `generated/reports/registries.json` есть `protocol_id` у каждой записи
   `minecraft:block_entity_type` и `minecraft:menu`. Запуск генератора в этой
   сессии был отклонён системой разрешений — нужно разрешение пользователя.
2. Чёрный ящик: сценарий, ставящий все нужные блоки, после чего робот выходит
   из дальности и возвращается (или перезаходит) — в пакете Chunk Data у
   каждой блок-сущности есть поле Type. `bot.js` сейчас пишет только
   `tile_entity_data`; нужно добавить разбор `map_chunk.blockEntities`
   (у mineflayer поля `x`, `z`, `y`, `type`, `nbtData`, см. MD-J `types.chunkBlockEntity`).

### Какие данные слать

У оригинала в Block Entity Data и в чанке идут не все поля блок-сущности, а только
то, что нужно клиенту для отрисовки. Для таблички (BB) это `front_text`/`back_text`
и др. Для сундука/бочки/воронки чёрный ящик Block Entity Data **ни разу не
записал** — ни при открытии, ни при изменении содержимого (сценарии `chest-open`,
`hopper-chain`, `dropper-pulse`, `trapped-chest`). Содержимое хранилищ клиенту
через блок-сущность не передаётся: оно приходит только в окне.

Вывод: сундук клиент рисует по одному состоянию блока (WIKI-BEF: блок-сущность
сундука отвечает за его отрисовку, но создаёт её клиент сам). Слать сундуки в
списке блок-сущностей чанка или пакетом 0x06 для отрисовки, по-видимому, не нужно;
`CustomName` (заголовок) идёт в Open Screen. Подтвердить на живом клиенте: сундук,
поставленный у нас без блок-сущности в чанке, должен рисоваться и открываться с
анимацией.

Полный NBT сундука на диске (WIKI-BEF, `Chest` § Block data) — для нашего файла мира,
не для сети: `id`, `x`, `y`, `z`, `components`, `CustomName`, `lock`, `LootTable`,
`LootTableSeed`, `Items` — список компаундов `{Slot: byte 0–26, id: string, count: int,
components: compound}`.

## 1.12. Блок-сущности в Chunk Data and Update Light (0x2D)

MD-J `packet_map_chunk` и `types.chunkBlockEntity`; WIKI-P § Chunk Data and Update Light.

Порядок полей пакета: `Chunk X: Int`, `Chunk Z: Int`, `Heightmaps: Prefixed Array`,
`Data: Prefixed Array of Byte`, **`Block Entities: Prefixed Array`**, затем Light Data
(`skyLightMask`, `blockLightMask`, `emptySkyLightMask`, `emptyBlockLightMask`,
`skyLight`, `blockLight`). У нас в `send_chunk` на месте Block Entities пишется `varint 0`.

Один элемент Block Entities:

| Поле | Тип | Смысл |
|---|---|---|
| Packed XZ | Unsigned Byte | `((blockX & 15) << 4) \| (blockZ & 15)` — X в старших 4 битах |
| Y | **Short** | абсолютная высота блока в мире (не в секции) |
| Type | VarInt | номер `minecraft:block_entity_type` |
| Data | NBT (сетевой, корень без имени; может быть пустым — тег End) | «данные без X, Y и Z» |

В MD-J то же: `bitfield [x:4, z:4]`, `y: i16`, `type: varint`, `nbtData: anonOptionalNbt`.

WIKI-P: «не обязательно слать все блок-сущности в этом пакете; можно прислать их
позже пакетом Block Entity Data». Для нас: тело чанка кэшируется в самом чанке
(`chunk_packet`), а список блок-сущностей идёт **после** тела — его можно дописывать
отдельно, не ломая кэш секций.

## 1.13. Block Action (0x07) — крышка сундука

MD-J `packet_block_action`: `location: position`, `byte1: u8`, `byte2: u8`,
`blockId: varint`. WIKI-P § Block Action; WIKI-BA § Chest.

| Поле | Тип | Для сундука |
|---|---|---|
| Location | Position | место сундука |
| Action ID | Unsigned Byte | **1** |
| Action Parameter | Unsigned Byte | число игроков, у которых сундук открыт; 0 — закрыт, ≥1 — открыт |
| Block Type | VarInt | номер в реестре `minecraft:block` (не состояние!) — `chest` или `trapped_chest`; клиент его не смотрит |

Относится к `chest`, `trapped_chest`, `ender_chest` (WIKI-BA, таблица блоков).
Шалкеровый ящик: действие 1 — число смотрящих (WIKI-BA § Shulker Box).
Бочка Block Action **не использует**: у неё состояние блока `open=true/false`
(Block Update).

Что делает оригинал (BB `vanilla-chest-open.json`, двойной сундук `(0,0,0)`
`type=left` + `(1,0,0)` `type=right`):

- при открытии в тот же такт: звук `minecraft:block.chest.open` (разряд `block`,
  громкость 0.5, высота случайная ~0.9–1.0) в **середине двойного сундука**
  (`[1, 0.5, 0.5]` относительно левой половины), Open Screen, Set Container Content,
  затем Block Action `1/1` **для обеих половин** (сначала `(1,0,0)`, потом `(0,0,0)`);
- пока сундук открыт, Block Action `1/1` для обеих половин повторяется примерно
  **каждые 5 тактов** (такты +4, +9, +14 от открытия; WIKI-BA: «every 5 ticks»);
- при закрытии: звук `block.chest.close`, Block Action `1/0` обеим половинам,
  и ещё раз через 4 такта;
- бочка: звук `block.barrel.open` в точке `[0.5, 1, 3.5]` (центр блока + 0.5 по
  направлению `facing=up`), Block Update `open=true` (дважды в одном такте),
  Open Screen вида 2; при закрытии — `block.barrel.close` и `open=false`.

Сундук-ловушка (`vanilla-trapped-chest.json`): то же `1/1` → `1/0` на `trapped_chest`.

---

# Часть 2. Bedrock (протокол 944)

Номера пакетов — MD-B `types.mcpe_packet` → `mappings` и MOJ (`$metaProperties["[cereal:packet]"]`
в `json/<Пакет>.json`); оба источника сходятся.

| Номер | Пакет (Mojang) | Имя в minecraft-data | Направление |
|---|---|---|---|
| 26 (0x1A) | BlockEventPacket | `block_event` | клиенту |
| 46 (0x2E) | ContainerOpenPacket | `container_open` | клиенту |
| 47 (0x2F) | ContainerClosePacket | `container_close` | оба |
| 49 (0x31) | InventoryContentPacket | `inventory_content` | клиенту |
| 50 (0x32) | InventorySlotPacket | `inventory_slot` | клиенту |
| 51 (0x33) | ContainerSetDataPacket | `container_set_data` | клиенту |
| 56 (0x38) | BlockActorDataPacket | `block_entity_data` | оба |
| 58 (0x3A) | LevelChunkPacket | `level_chunk` | клиенту |
| 147 (0x93) | ItemStackRequestPacket | `item_stack_request` | серверу |
| 148 (0x94) | ItemStackResponsePacket | `item_stack_response` | клиенту |

Кроме BlockEvent (26), BlockActorData (56) и ContainerSetData (51), все номера уже
есть константами в `src/bedrock/session.rs`.

## 2.1. ContainerOpen (46)

MOJ `json/ContainerOpenPacket.json`; MD-B `types.packet_container_open`.

| # | Поле (Mojang) | Тип | Смысл |
|---|---|---|---|
| 0 | Container Id | uint8 (MD-B: `WindowID`, i8) | номер окна, выбирает сервер; для хранилищ — из диапазона `CONTAINER_ID_FIRST` = 1 … `CONTAINER_ID_LAST` = 100 (MOJ `enums.html`, ContainerID) |
| 1 | Container Type | uint8 (MD-B: `WindowType`, i8) | вид экрана, перечисление `ContainerType` |
| 2 | Position | BlockPos: X, Y, Z — int32 со сжатием (zigzag varint) | блок хранилища; у нас `Out::block_position` |
| 3 | Target Actor ID | ActorUniqueID: int64 со сжатием (zigzag64) | для своего инвентаря у нас — свой номер; для блока документы значения не называют — вероятно −1 («сущности нет»), проверить живым клиентом |

Описание Mojang: «сервер шлёт, чтобы клиент открыл экран контейнера **и проиграл
анимацию открытия сундука**».

`ContainerType` (MOJ `html/enums.html`; MD-B `types.WindowType` — то же):

| Значение | Имя | Для чего |
|---|---|---|
| −9 | NONE | |
| −1 | INVENTORY | свой инвентарь (у нас уже) |
| **0** | **CONTAINER** | сундук, сундук-ловушка, бочка, эндер-сундук, шалкеровый ящик |
| 1 | WORKBENCH | верстак |
| 2 | FURNACE | |
| **6** | **DISPENSER** | раздатчик |
| **7** | **DROPPER** | выбрасыватель |
| **8** | **HOPPER** | воронка |
| 10 | MINECART_CHEST | |
| 11 | MINECART_HOPPER | |
| 25 | LECTERN | |
| **36** | **CRAFTER** | крафтер |

Двойной сундук отдельного вида не имеет — тот же CONTAINER (0); размер окна клиент
берёт из спаренной блок-сущности (`pairx`/`pairz`, § 2.7). Проверить живым клиентом.

MOJ `ItemStackPackets_Disabled.md` § OpenContainerPacket: клиенты **ждут**
ContainerOpen для любого экрана; сервер может отказать, просто не ответив. Сразу
после ContainerOpen сервер «как обычно» шлёт **InventoryContent основного контейнера
экрана** (содержимое сундука). Панель, рюкзак, броню и вторую руку при открытии
экрана оригинал не шлёт.

## 2.2. ContainerClose (47)

MOJ `json/ContainerClosePacket.json`; MD-B `types.packet_container_close`.

| # | Поле | Тип | Смысл |
|---|---|---|---|
| 0 | Container Id | uint8 | номер окна |
| 1 | Container Type | uint8 (ContainerType одним байтом) | вид окна |
| 2 | Server Initiated Close | bool | **true — закрыл сервер** |

Порядок по MOJ: клиент удаляет у себя менеджер контейнера и шлёт этот пакет;
сервер удаляет свой и **отвечает тем же пакетом**, и только тогда экран у клиента
закрывается. У нас это уже сделано (`server = false`). Закрыть экран по своей
воле (сундук сломан) — ContainerClose с `server = true`.

## 2.3. InventoryContent (49) для окна хранилища

MOJ `dot/InventoryContentPacket.dot`; MD-B `types.packet_inventory_content`.

| # | Поле | Тип |
|---|---|---|
| 0 | Inventory Id | unsigned varint (MD-B: `WindowIDVarint`) — **номер окна из ContainerOpen** |
| 1 | Slots | unsigned varint длина + NetworkItemStackDescriptor (MD-B: `ItemStacks` = массив `Item`) |
| 2 | Full Container Name | FullContainerName (§ 2.5) |
| 3 | Storage Item | NetworkItemStackDescriptor; пусто — `zigzag32 0` |

Слоты — только собственные слоты хранилища: 27 у сундука и бочки, 54 у двойного
сундука (проверить), 5 у воронки, 9 у раздатчика/выбрасывателя/крафтера. Инвентарь
игрока в это окно не входит (в отличие от Java). Предмет пишется нашим
`write_item` (номер стопки — свой счётчик, клиент ссылается на него в запросах).

## 2.4. InventorySlot (50)

MOJ `dot/InventorySlotPacket.dot`; MD-B `types.packet_inventory_slot`.

| # | Поле | Тип |
|---|---|---|
| 0 | Container ID | unsigned varint — номер окна |
| 1 | Slot | unsigned varint |
| 2 | Full Container Name | FullContainerName |
| 3 | Storage Item | NetworkItemStackDescriptor |
| 4 | Item | NetworkItemStackDescriptor |

Порядок тот же, что у нас в `slot_packet`: окно, слот, имя контейнера, storage item,
предмет. Нужен, когда содержимое открытого сундука меняет не этот игрок (воронка,
другой игрок).

## 2.5. FullContainerName и ContainerSlotType (имена контейнеров)

FullContainerName (MOJ `dot/FullContainerName.dot`; MD-B `types.FullContainerName`):
`Container Name: uint8 (ContainerEnumName)`, затем `Dynamic ID` — в MD-B
`option<u32>`: байт-признак, при 1 — `u32` (в ProtoDef `u32` — big-endian; у нас
`read_slot` так и читает).

ContainerEnumName (MOJ `html/enums.html`; MD-B `types.ContainerSlotType`), нужные:

| Значение | Mojang | minecraft-data | Для чего |
|---|---|---|---|
| 6 | ArmorContainer | `armor` | (уже есть) |
| **7** | **LevelEntityContainer** | `container` | блок-хранилище: сундук, сундук-ловушка, двойной сундук, эндер-сундук, **воронка, раздатчик, выбрасыватель** |
| 12 | CombinedHotbarAndInventoryContainer | `hotbar_and_inventory` | (уже есть) |
| 28 | HotbarContainer | `hotbar` | (уже есть) |
| 29 | InventoryContainer | `inventory` | (уже есть) |
| 30 | ShulkerBoxContainer | `shulker` | шалкеровый ящик |
| 34 | OffhandContainer | `offhand` | (уже есть) |
| **58** | **BarrelContainer** | `barrel` | бочка |
| 59 | CursorContainer | `cursor` | (уже есть) |
| 60 | CreatedOutputContainer | `creative_output` | (уже есть) |
| **62** | **CrafterLevelEntityContainer** | `crafter` | крафтер |
| 63 | DynamicContainer | `dynamic` | контейнеры с динамическим номером (мешок) |

Своего имени у воронки, раздатчика и выбрасывателя в перечислении нет — значит,
они идут как LevelEntityContainer (7). Это вывод «от противного»; клиент сам
называет контейнер в запросе, а наш ответ повторяет его имя (`write_containers`),
так что при ошибке в InventoryContent клиент просто покажет пустые слоты —
проверить живым клиентом.

Расхождение источников: после 63 у Mojang (944) — RecipeFoodContainer 64,
RecipeBlocksContainer 65, RecipeFurnaceItemsContainer 66 (MOJ `changelog_944_03_04_26.md`:
«Added RecipeFoodContainer (64)…»), а в MD-B 1.26.10 — `64 registry`, `65 recipe_food`,
`66 recipe_blocks`, `67 recipe_furnace_items`. Для хранилищ не важно; при сомнении
верить Mojang.

## 2.6. ItemStackRequest / ItemStackResponse для хранилища

MD-B `types.ItemStackRequest`, `types.StackRequestSlotInfo`, `types.ItemStackResponses`.
Формат уже реализован в `src/bedrock/inventory.rs`; для хранилищ меняется только
то, **куда** указывает слот.

StackRequestSlotInfo: `FullContainerName` (u8 имя + option<u32>), `slot: u8`,
`stack_id: zigzag32`.

Действия (u8), которые встретятся с сундуком:

| Код | Имя (MD-B) | Поля |
|---|---|---|
| 0 | take | `count: u8`, `source`, `destination` |
| 1 | place | `count: u8`, `source`, `destination` |
| 2 | swap | `source`, `destination` |
| 3 | drop | `count: u8`, `source`, `randomly: bool` |
| 7 | place_in_container | `count: u8`, `source`, `destination` |
| 8 | take_out_container | `count: u8`, `source`, `destination` |

Пример: Shift+щелчок из сундука в инвентарь — `source = {7, None, слот 0–26}`,
`destination = {12 или 28/29, None, слот}`. Бочка — то же с именем 58.
Нужно расширить `place()`/`Place`: `Place::Container(slot)` для имён 7/58/62/30,
действительных, только пока у игрока открыто окно этого хранилища, и с
проверкой диапазона слотов (0–26, 0–53, 0–4, 0–8).

Ответ (MD-B `types.ItemStackResponses`): `status: u8` (0 ok, 1 error),
`request_id: zigzag32`, при ok — `containers: [ {FullContainerName, slots: [ {slot: u8,
hotbar_slot: u8, count: u8, item_stack_id: zigzag32, custom_name: string,
filtered_custom_name: string, durability_correction: zigzag32} ] } ]`.
Совпадает с тем, что пишет `write_containers`.

Изменения, вызванные запросом, другим игрокам, у которых тот же сундук открыт,
надо разослать InventorySlot (Bedrock) и Set Container Slot (Java).

## 2.7. BlockActorData (56) и NBT сундука

MOJ `json/BlockActorDataPacket.json`: «шлёт весь пользовательский компаунд и
положение блока»; MD-B `types.packet_block_entity_data`.

| # | Поле | Тип |
|---|---|---|
| 0 | Block Position | BlockPos: zigzag32 X, Y, Z |
| 1 | Actor Data Tags | CompoundTag в сетевом виде |

Сетевой CompoundTag Bedrock (MOJ `dot/CompoundTag.dot`): байт типа тега, имя —
строка с длиной varint, значения: Byte — байт, Short — 2 байта LE, **Int — varint
со знаком (zigzag)**, Int64 — varint64 со знаком, Float/Double — LE, String —
длина varint + байты, List — байт типа элементов + varint длина (для **пустого**
списка тип элементов пишется 1), Compound — теги до End (0). Корень — тег 10 с
пустым именем (у нас `Out::empty_nbt` так и пишет пустой компаунд).

Строковые id блок-сущностей Bedrock (WIKI-BBEF, таблицы «ID … Block entity» на
страницах блоков):

| Блок | `id` |
|---|---|
| сундук **и** сундук-ловушка | `Chest` |
| бочка | `Barrel` |
| воронка | `Hopper` |
| раздатчик | `Dispenser` |
| выбрасыватель | `Dropper` |
| крафтер | `Crafter` |
| эндер-сундук | `EnderChest` |
| шалкеровый ящик | `ShulkerBox` |
| кафедра | `Lectern` |
| проигрыватель | `Jukebox` |
| табличка / висячая | `Sign` / `HangingSign` |

Общие поля (WIKI-BBEF § Block Entity): `id: string`, `x`, `y`, `z: int`,
`CustomName: string` (может не быть), `BlockEntityVersion: int`. Кроме того, до
Bedrock 26.30 у **всех** блок-сущностей было поле `isMovable: byte` (история
страницы WIKI-BBEF: «Preview 26.30.27 — поле isMovable убрано у всех блок-сущностей»),
значит в нашей 26.10–26.13 оно ещё есть.

Сундук (WIKI-BBEF § Chests; относится к сундуку, сундуку-ловушке, бочке, эндер-сундуку):

| Поле | Тип | Смысл |
|---|---|---|
| `Findable` | byte | неизвестно |
| `forceunpair` | byte | (может не быть) 1 — не спариваться с соседом |
| `Items` | list of compound | содержимое: `Slot: byte`, `Name: string`, `Count: byte`, `Damage: short`, `WasPickedUp: byte`, `Block: compound`?, `tag: compound`? (WIKI-BBEF Item format) |
| `LootTable`, `LootTableSeed` | string, int | (может не быть) |
| **`pairlead`** | byte | (может не быть) «неизвестно» по вики; по смыслу — эта половина ведущая в паре |
| **`pairx`**, **`pairz`** | int | (может не быть) координаты второй половины двойного сундука |

Воронка: `Items`, `TransferCooldown: int`. Раздатчик/выбрасыватель: `Items`,
`LootTable`, `LootTableSeed`. Крафтер: `disabled_slots: short`, `Items` (слоты 0–8).

Для отрисовки и открытия клиенту содержимое (`Items`) не нужно — оно приходит в
InventoryContent. **Для двойного сундука Bedrock спаривание задаётся именно
блок-актором** (`pairx`/`pairz`, у одной из половин `pairlead = 1`) — без него
клиент покажет два одиночных сундука. Минимальный набор, который стоит попробовать
первым: `{id: "Chest", x, y, z, isMovable: 1, pairlead, pairx, pairz}` на обе
половины (у второй `pairlead = 0`). Непроверено — проверить живым клиентом.

Отправлять BlockActorData нужно после UpdateBlock с самим сундуком (клиенту
некуда повесить данные до появления блока — то же говорит MOJ о подчанках, § 2.8).

## 2.8. Блок-акторы внутри LevelChunk (58)

MD-B `types.packet_level_chunk`: `x: zigzag32`, `z: zigzag32`, `dimension: zigzag32`,
`sub_chunk_count: varint` (при −2 далее `lu16`), `cache_enabled: bool`,
`blobs` (если кэш), `payload: ByteArray`. Это у нас в `encode_chunk`.

Содержимое payload (у нас, проверено живым клиентом): подчанки (версия 9) →
биомы (палитра на каждый подчанк) → байт числа «блоков на краю» (0). **После него
идут блок-акторы**: подряд записанные сетевые CompoundTag (§ 2.7), по одному на
блок-сущность, до конца payload; число не пишется.

Источники этого места скупые: MOJ `SubChunk Request System v1.18.10.md` говорит,
что сериализованный подчанк в SubChunkPacket состоит из «данных подчанка» и
«данных блок-акторов», и что «сериализация блок-сущностей **такая же, как в
LevelChunk**»; и что при «скелетном» LevelChunk (запрос подчанков) блок-сущности
в LevelChunk **слать нельзя** — они идут в подчанках. Мы скелетные чанки не шлём,
так что блок-акторы — в конец payload. Точная форма (сетевой NBT, без счётчика)
по документам Mojang явно не описана — проверить живым клиентом на двойном
сундуке: если половины спарились, формат верный.

Альтернатива без изменения чанка: слать BlockActorData (56) отдельно после
LevelChunk. Кэш тела чанка при этом не трогается.

## 2.9. BlockEvent (26) — крышка сундука

MOJ `json/BlockEventPacket.json`: «при событии блока сервер шлёт его клиенту,
с произвольно закодированными b0 и b1»; MD-B `types.packet_block_event`.

| # | Поле | Тип |
|---|---|---|
| 0 | Block Position | BlockPos: zigzag32 X, Y, Z |
| 1 | Event Type | int32 со сжатием (zigzag32); MD-B: 0 = `sound`, **1 = `change_state`** |
| 2 | Event Value | int32 со сжатием (zigzag32) |

Для сундука: тип **1**, значение **1 — открыт, 0 — закрыт** (как параметр Block Action
у Java; для Bedrock значения документами не описаны — проверить, не число ли это
смотрящих). Самому открывшему анимацию, судя по описанию ContainerOpen, клиент
проигрывает сам; остальным игрокам рядом нужен BlockEvent. Для двойного сундука —
вероятно, обеим половинам (как у Java), проверить.

Звук — отдельным LevelSoundEvent; номера в MOJ `html/enums.html`: `ChestOpen` 67,
`ChestClosed` 68, `ShulkerBoxOpen` 69, `EnderChestOpen` 71, `BarrelOpen` 270,
`BarrelClose` 271. Бочке BlockEvent не нужен: у неё свойство блока `open_bit`
(UpdateBlock).

## 2.10. ContainerSetData (51)

MOJ `json/ContainerSetDataPacket.json`: `Container ID: uint8`, `ID: int32 (zigzag)`,
`Value: int32 (zigzag)`. Для печей и зельеварки; для наших хранилищ не нужен.

---

# Часть 3. Сводка «что нужно сделать» и что проверить

Порядок открытия сундука (Java, по BB): Use Item On → в том же такте звук открытия →
Open Screen (номер окна 1…100, вид 2/5/6/7/16, заголовок `translate`) → Set Container
Content (свои слоты + рюкзак + панель + курсор, новый State ID) → Block Action `1/1`
(обеим половинам двойного) → каждые ~5 тактов Block Action `1/N` снова.
Закрытие: Close Container (0x13) от клиента → звук закрытия → Block Action `1/0`.

Порядок открытия (Bedrock, по MOJ): щелчок (InventoryTransaction, «использовать на
блоке») → ContainerOpen (окно 1…100, вид 0/6/7/8/36, позиция, `-1`?) →
InventoryContent (окно, свои слоты, имя 7/58/62) → BlockEvent `1/1` соседям.
Закрытие: ContainerClose от клиента → ответ ContainerClose (`server = false`) →
BlockEvent `1/0`.

Непроверенное — по важности:

1. Номера `minecraft:block_entity_type` (Java) — кроме `hanging_sign = 8`,
   неизвестны; способ получить — § 1.11 (нужно решение пользователя).
2. Нужен ли сундуку Java элемент в списке блок-сущностей чанка вообще (по BB —
   оригинал Block Entity Data для хранилищ не шлёт; вероятно, не нужен).
3. Bedrock: спаривание двойного сундука через `pairx`/`pairz`/`pairlead`, место
   блок-акторов в payload LevelChunk, вид окна двойного сундука.
4. Bedrock: имя контейнера для воронки/раздатчика/выбрасывателя = 7.
5. Bedrock: смысл значения BlockEvent для сундука.
6. Java: какая половина двойного сундука — слоты 0–26 в окне `generic_9x6`.
7. Java: свойство 9 у крафтера.
