# Сущности игроков: пакеты протокола 775 (Minecraft 26.1.2)

Справочник для кодирования пакетов. Только факты и форматы.

Версия клиента: **26.1.2**, протокол **775**. Источники — только открытая документация протокола
(minecraft.wiki, бывший wiki.vg) и числовые таблицы из набора данных PrismarineJS/minecraft-data
(ветка `data/pc/26.1`). Код игры и серверных ядер не использовался.

**Важно про нумерацию.** Страницы вики для протокола 775 не существует: вики документирует
1.21.10 (773) и 26.2 (776), перескочив 775. Номера ниже взяты из minecraft-data (`data/pc/26.1`,
`version.json` → protocol 775, `protocol.json` → карта ID) и сверены: список пакетов 776 на вики
совпадает с 775 из minecraft-data до одного пакета, а номера в вики для 1.21.10 меньше на 1–2
(например, `login` там `0x30` вместо `0x31`, `player_info_update` — `0x44` вместо `0x46`).
Если открыть вики без указания ревизии — там 776, номера совпадают с этим файлом; если попалась
ревизия с шапкой «1.21.10, protocol 773» — номера там сдвинуты и брать их нельзя.

Сводка номеров (775, состояние Play, направление Clientbound):

| Пакет (официальное имя / имя на вики) | ID hex | ID dec |
|---|---|---|
| Add Entity / Spawn Entity | 0x01 | 1 |
| Teleport Entity / Teleport Entity | 0x23 | 35 |
| Update Entity Position / Update Entity Position | 0x35 | 53 |
| Update Entity Position and Rotation / Update Entity Position and Rotation | 0x36 | 54 |
| Update Entity Rotation / Update Entity Rotation | 0x38 | 56 |
| Remove Entities / Remove Entities | 0x4D | 77 |
| Set Head Rotation / Set Head Rotation | 0x53 | 83 |
| Player Info Remove / Player Info Remove | 0x45 | 69 |
| Player Info Update / Player Info Update | 0x46 | 70 |
| (не обязательны) Set Entity Data / Set Entity Data | 0x63 | 99 |
| (не обязательны) Set Entity Velocity / Set Entity Velocity | 0x65 | 101 |
| (не обязательны) Entity Event / Entity Event | 0x22 | 34 |
| (не обязательны) Bundle Delimiter / Bundle Delimiter | 0x00 | 0 |

---

## 1. Add Entity — ID 0x01 (dec 1)

Официальное имя `add_entity`, на вики раздел «Spawn Entity», в minecraft-data `spawn_entity`.
Создаёт сущность на клиенте.

Поля строго по порядку:

| № | Поле | Тип | Размер | Что писать для игрока |
|---|---|---|---|---|
| 1 | Entity ID | VarInt | 1–5 байт | номер сущности |
| 2 | Entity UUID | UUID | 16 байт | UUID игрока |
| 3 | Type | VarInt | 1–5 байт | ID из реестра `minecraft:entity_type`, для игрока **155** (см. раздел 6) |
| 4 | X | Double | 8 байт | абсолютная координата |
| 5 | Y | Double | 8 байт | абсолютная координата |
| 6 | Z | Double | 8 байт | абсолютная координата |
| 7 | Velocity | LpVec3 | 1 и более байт | нулевая скорость = **один байт 0x00** (см. раздел 8) |
| 8 | Pitch | Angle | 1 байт | угол взгляда вниз/вверх |
| 9 | Yaw | Angle | 1 байт | угол поворота тела |
| 10 | Head Yaw | Angle | 1 байт | у игрока практически всегда равен Yaw |
| 11 | Data | VarInt | 1–5 байт | для игрока **0** (см. ниже) |

Пояснения по полям:

- **Entity ID.** Если сущность с таким ID уже есть на клиенте, она удаляется и заменяется новой.
  У ванильного сервера ID уникальны глобально по всем измерениям и не переиспользуются до
  перезапуска сервера.
- **Entity UUID.** Клиент допускает несколько сущностей с одним UUID, но пишет предупреждение в лог.
- **Type** — только **число** из реестра. Строка `minecraft:player` в этом пакете **не передаётся
  нигде и никогда**; имя типа существует только в реестре, который зашит в клиент.
- **Velocity** — тип LpVec3, а **не** три int16. Три int16 (вектор скорости) в этом пакете были до
  1.21.2; в 775 их нет. Для стоящего на месте игрока пишется ровно один байт `0x00`.
- **Data** — смысл зависит от Type. В таблице Object data (`Java Edition protocol/Object data`)
  перечислены Item Frame, Painting, Falling Block, Fishing Hook, Projectile, Warden; **Player там
  нет**, поле не используется. Писать `VarInt 0`.
- **Свою собственную сущность этим пакетом создавать нельзя и не нужно**: локальная сущность
  игрока создаётся клиентом сама. Дословно: «The local player entity is automatically created by
  the client, and must not be created explicitly using this packet. Doing so on the vanilla client
  will have strange consequences.»

Итого минимальный размер для игрока: 1 (id сущности, обычно) + 16 (UUID) + 2 (тип 155) + 24 (XYZ)
+ 1 (нулевая скорость) + 3 (углы) + 1 (Data) = около 48 байт плюс байты на VarInt-и.

---

## 2. Remove Entities — ID 0x4D (dec 77)

Официальное имя `remove_entities`, в minecraft-data `entity_destroy`.

| Поле | Тип |
|---|---|
| Entity IDs | Prefixed Array: VarInt (количество) + N × VarInt (ID сущности) |

Удаляет перечисленные сущности. Для одного игрока: `VarInt 1` + `VarInt <entity_id>`.
Отдельного пакета «удалить одну сущность» нет — массив из одного элемента.

---

## 3. Перемещение сущностей

Координаты в «позиционных» пакетах — **дельты в виде fixed-point с 12 дробными битами и 4 битами
целой части**: `delta = (new − old) × 4096`, записывается как знаковый **Short (int16, 2 байта,
big-endian)**. Предел по каждой оси: **8 блоков в отрицательную сторону и 7.999755859375 блоков в
положительную**. Дословно: «the maximum movement distance along each axis is 8 blocks in the
negative direction, or 7.999755859375 blocks in the positive direction. If the movement exceeds
these limits, Teleport Entity should be sent instead.» То есть при дельте больше 8 блоков по любой
оси (и в любом случае при телепорте) вместо дельта-пакета шлётся Teleport Entity.

Углы в этих пакетах — **не дельты**, а новые абсолютные углы, тип Angle (1 байт).

### 3а. Update Entity Position — ID 0x35 (dec 53)

`move_entity_pos` (`rel_entity_move`).

| № | Поле | Тип | Размер |
|---|---|---|---|
| 1 | Entity ID | VarInt | 1–5 |
| 2 | Delta X | Short | 2 |
| 3 | Delta Y | Short | 2 |
| 4 | Delta Z | Short | 2 |
| 5 | On Ground | Boolean | 1 |

Шлётся, когда игрок сдвинулся, но не изменил взгляд.

### 3б. Update Entity Position and Rotation — ID 0x36 (dec 54)

`move_entity_pos_rot` (`entity_move_look`).

| № | Поле | Тип | Размер |
|---|---|---|---|
| 1 | Entity ID | VarInt | 1–5 |
| 2 | Delta X | Short | 2 |
| 3 | Delta Y | Short | 2 |
| 4 | Delta Z | Short | 2 |
| 5 | Yaw | Angle | 1 |
| 6 | Pitch | Angle | 1 |
| 7 | On Ground | Boolean | 1 |

Шлётся, когда игрок и сдвинулся, и повернулся.

### 3в. Update Entity Rotation — ID 0x38 (dec 56)

`move_entity_rot` (`entity_look`).

| № | Поле | Тип | Размер |
|---|---|---|---|
| 1 | Entity ID | VarInt | 1–5 |
| 2 | Yaw | Angle | 1 |
| 3 | Pitch | Angle | 1 |
| 4 | On Ground | Boolean | 1 |

Шлётся, когда игрок только повернулся, без смещения.

При любом изменении взгляда дополнительно шлётся **Set Head Rotation** (раздел 3г): для чужих
сущностей игроков ванильный сервер всегда повторяет в повороте головы тот же yaw.
Дословно: «For player entities (other than the client's local player), the vanilla server always
updates the head rotation to be the same as the yaw set in Set Player Rotation».

### 3г. Set Head Rotation — ID 0x53 (dec 83)

`rotate_head` (`entity_head_rotation`).

| № | Поле | Тип | Размер |
|---|---|---|---|
| 1 | Entity ID | VarInt | 1–5 |
| 2 | Head Yaw | Angle | 1 |

### 3д. Teleport Entity — ID 0x23 (dec 35)

Официальное имя `entity_position_sync`, в minecraft-data `sync_entity_position`.
Шлётся, когда сущность сместилась больше чем на 8 блоков (см. выше), а также при любом
«жёстком» переносе.

| № | Поле | Тип | Размер | Комментарий |
|---|---|---|---|---|
| 1 | Entity ID | VarInt | 1–5 | |
| 2 | X | Double | 8 | абсолютная координата |
| 3 | Y | Double | 8 | абсолютная |
| 4 | Z | Double | 8 | абсолютная |
| 5 | dX | Double | 8 | **скорость**, не дельта |
| 6 | dY | Double | 8 | скорость |
| 7 | dZ | Double | 8 | скорость |
| 8 | Yaw | Float | 4 | **абсолютный угол в градусах** (не Angle-байт) |
| 9 | Pitch | Float | 4 | абсолютный угол в градусах |
| 10 | On Ground | Boolean | 1 | |

Полей «Teleport Flags» в этом пакете **нет**.

**Не путать с пакетом 0x7D.** В 1.21.2 пакет `entity_position_sync` (0x23) отобрали у имени
`teleport_entity`; новое `teleport_entity` — это то, что вики называет «Synchronize Vehicle
Position», и у него есть поле Teleport Flags (u32, биты 0x0001…0x0100: относительные X/Y/Z/yaw/
pitch/скорости и поворот скорости). Вики прямо предупреждает, что использовать его вместо 0x23
«will lead to confusing results». Для игроков нужен 0x23.

---

## 4. Player Info Update — ID 0x46 (dec 70)

Официальное имя `player_info_update`. Отвечает за список игроков (Tab): имена, скины, режим игры,
отображение в списке. **Сущности в мире он не создаёт.**

Структура:

| № | Поле | Тип |
|---|---|---|
| 1 | Actions | EnumSet = 1 байт, битовая маска |
| 2 | Number Of Players | VarInt (количество записей ниже) |
| 3 | Players | N × запись, см. ниже |

Каждая запись:

| № | Поле | Тип | Когда присутствует |
|---|---|---|---|
| 1 | UUID | UUID (16 байт) | всегда |
| 2 | Player Actions | поля включённых действий, **в порядке возрастания маски** | всегда |

Поля действий (идут в порядке от младшего бита к старшему, каждое — только если его бит выставлен):

| Маска | Действие | Поля | Тип |
|---|---|---|---|
| 0x01 | Add Player | Name | String (16) |
| | | Game profile properties | Prefixed Array (16): Name String (64), Value String (32767), Signature Prefixed Optional String (1024) |
| 0x02 | Initialize Chat | Data | Prefixed Optional: Chat session ID UUID, Public key expiry Long, Encoded public key Prefixed Array (512) of Byte, Public key signature Prefixed Array (4096) of Byte |
| 0x04 | Update Game Mode | Game Mode | VarInt |
| 0x08 | Update Listed | Listed | Boolean (1 байт: 0/1) |
| 0x10 | Update Latency | Ping | VarInt (мс) |
| 0x20 | Update Display Name | Display Name | Prefixed Optional Text Component |
| 0x40 | Update List Priority | Priority | VarInt |
| 0x80 | Update Hat | Visible | Boolean |

Пример разбора маски (формулировка вики): при значении 5 (двоичное 00000101) ненулевые маски
0x01 и 0x04, значит в записи будут ровно два блока полей — Add Player и Update Game Mode, в этом
порядке.

**Что включать, чтобы игрок появился в списке:** `0x01 | 0x04 | 0x08` = `0x0D`.
- `0x01` (Add Player) — обязателен, задаёт имя и профиль (скины);
- `0x08` (Update Listed) со значением 1 — без него игрок не попадает в Tab;
- `0x04` (Update Game Mode) — не обязателен для появления в списке, но от режима зависит порядок
  сортировки (наблюдатели сортируются после ненаблюдателей) и отображение; ванильный сервер его
  шлёт.
Итого на одного игрока: маска `0x0D`, `VarInt 1` (число записей), UUID, `String` имя, `VarInt 0`
(ноль свойств профиля — тогда клиент даёт дефолтный скин по UUID), `VarInt <режим игры>`, байт 1
(Listed = true).

Порядок полей внутри записи жёсткий: сначала Name, затем properties, затем Game Mode, затем Listed.
Числа действий в маске не влияют на порядок — он всегда по возрастанию бита.

Поля `0x02` (Initialize Chat) в 775 существуют, но для появления игрока в списке не нужны.

Поля свойств профиля (для скинов): имя свойства обычно одно — `textures`, значение — base64 от
UTF-8 JSON. Пустой массив свойств допустим: «An empty properties array is also acceptable, and will
cause clients to display the player with one of the default skins depending on UUID».

Оговорка по названиям битов 0x40/0x80: вики говорит 0x40 = Update List Priority, 0x80 = Update Hat;
в карте minecraft-data эти два имени переставлены местами. Порядок полей в пакете (Priority, затем
Hat) одинаков в обоих источниках. Нам нужны только 0x01/0x04/0x08.

### Player Info Remove — ID 0x45 (dec 69)

`player_info_remove`.

| Поле | Тип |
|---|---|
| UUIDs | Prefixed Array: VarInt (количество) + N × UUID (16 байт) |

Важно: здесь список **UUID**, а не entity ID.

---

## 5. Порядок отправки

Жёсткое правило, от которого зависит, появится ли игрок в мире (вики, ревизия `oldid=2773028`,
раздел «Spawn Player»; в текущем тексте вики фраза отсутствует, так как пакет слит в Spawn Entity):

> «This packet must be sent after the Player Info Update packet that adds the player data for the
> client to use when spawning a player. If the Player Info for the player spawned by this packet is
> not present when this packet arrives, **Notchian clients will not spawn the player entity**.»

То есть: **Player Info Update (0x46) для данного игрока обязан уйти раньше, чем Add Entity (0x01)
для него же.** Если Add Entity приходит без записи в списке — сущность не появится вообще (не
«появится без скина»), и именно это даёт наблюдаемую картину «в Tab есть, в мире нет».

### При заходе игрока N

Что шлём **новичку N** про остальных:
1. `Player Info Update` (0x46) с записью на каждого уже подключённого игрока (маска 0x0D);
2. `Add Entity` (0x01) на каждого уже подключённого игрока — после чанков и Game Event 13
   (у ванильного сервера порядок «inventory, entities, etc.» в конце последовательности входа).

Что шлём **остальным** про N:
1. `Player Info Update` (0x46) с записью на N;
2. `Add Entity` (0x01) на N.

Свою собственную сущность никому создавать не нужно (раздел 1). Свой профиль клиент получает в
пакете Login (play), поэтому отдельная запись о себе в списке для появления чужих сущностей не
требуется; наша отправка записи о себе (уже работающая) этому не мешает.

Дополнительно: ванильный сервер оборачивает Spawn Entity и пакеты настройки сущности в Bundle
Delimiter (0x00), чтобы они обработались в одном тике. Для игрока из одного Add Entity это не
обязательно.

### При движении

На каждого наблюдателя, у которого игрок в зоне видимости:
- сдвинулся и повернулся → `0x36`; только сдвинулся → `0x35`; только повернулся → `0x38`;
- при повороте дополнительно `0x53` (Set Head Rotation) с тем же yaw;
- если дельта по любой оси больше 8 блоков (или это телепорт) → `0x23` с абсолютными
  координатами и углами во Float.

Дельты считаются от той позиции, которую **этому наблюдателю** отправляли последней; для этого на
каждую пару «наблюдатель — сущность» нужно хранить последнюю отправленную позицию.

### При выходе игрока

1. `Remove Entities` (0x4D) с его entity ID — всем остальным;
2. `Player Info Remove` (0x45) с его UUID — всем остальным.

Порядок этих двух пакетов друг относительно друга на вики не оговорён.

---

## 6. Реестр типов сущностей

Реестр `minecraft:entity_type` — **встроенный (built-in), а не синхронизируемый**: страница
`Java Edition protocol/Registries` делит реестры на «Built-in registries» и «Synchronized
registries»; `minecraft:entity_type` перечислен в первом списке, а синхронизируемые передаются
сервером пакетом `registry_data` (0x07) в фазе Configuration. Значит, **сервер этот реестр
клиенту не отправляет**, он зашит в клиент, и в Add Entity поле Type — просто числовой
ID из него. Промах на единицу → сущность не появится или появится другого типа.

Значение для игрока в 26.1 / 26.1.1 / 26.1.2 (протокол 775):

| Свойство | Значение |
|---|---|
| имя | `minecraft:player` |
| ID в реестре | **155** |
| внутренний ID (minecraft-data `internalId`) | 155 |
| ширина | 0.6 |
| высота | 1.8 |

Подтверждения:
- набор данных PrismarineJS/minecraft-data, `data/pc/26.1/entities.json`: запись
  `{"id": 155, "internalId": 155, "name": "player", ...}`, всего 157 записей, у всех `id`
  совпадает с `internalId`;
- вики, `Java Edition protocol/Entity metadata`, ревизия `oldid=3494102` (шапка: «These entity IDs
  are up to date for 26.1») — строка `155 | Player | 0.6 | 1.8 | minecraft:player`.

**В 26.2 (протокол 776) игрок = 156** — в реестр добавился новый тип, и всё после него сдвинулось.
Для 26.1.2 брать 155.

VarInt(155) в байтах: `0x9B 0x01` (больше 127, поэтому два байта).

Ширина/высота в пакете не передаются — они целиком на стороне клиента.

---

## 7. Что для появления игрока НЕ нужно

- **Set Entity Data (0x63)** — метаданные не обязательны: «It is not required to send all metadata
  fields, or even any metadata fields, so long as the terminating entry is correctly sent». Формат
  на случай необходимости: VarInt entity ID, затем записи `u8 индекс, VarInt тип, значение`, конец
  — байт `0xFF`. Без терминатора пакет невалиден.
- **Entity Event (0x22)** — про анимации и статусы (например, уровень оператора), для появления не
  нужен.
- **Создание своей собственной сущности** — клиент делает это сам (раздел 1).
- **Минимум для «игрок виден в мире»**: Player Info Update (0x46) + Add Entity (0x01), в этом
  порядке. Для «игрок стоит там, где стоит» — пакеты движения из раздела 3.

---

## 8. Кодирование базовых типов

**VarInt** — 7 бит на байт, младшая группа первая; старший бит (0x80) означает «продолжение».
Значение 155 → `0x9B 0x01`.

**String** — VarInt (длина в байтах UTF-8) + сами байты UTF-8.

**UUID** — 16 байт (две big-endian восьмёрки).

**Boolean** — 1 байт: 0 или 1.

**Angle** — 1 байт, шаг 1/256 полного оборота: «A rotation angle in steps of 1/256 of a full turn».
Знаковость не важна, результат одинаков. Перевод:
`байт = round(градусы × 256 / 360) & 0xFF`, обратно `градусы = байт × 360 / 256`.
Yaw не ограничен диапазоном [0;360) — допустимы отрицательные значения и больше 360;
pitch: 0 — прямо, −90 — строго вверх, +90 — строго вниз.

**Fixed-point (дельты движения)** — знаковый Short, 12 дробных битов:
`delta_fixed = (int16)((new − old) × 4096)`, обратно `(new − old) = delta_fixed / 4096`.
Предел ±8 блоков на ось, см. раздел 3.

**LpVec3** (скорость в Add Entity и Set Entity Velocity) — «low precision vector», обычно 6 байт.

Алгоритм записи (проверен на всех четырёх тест-векторах вики):

1. `m = max(|x|, |y|, |z|)`. Если `m < 1/32766` (≈ 3.05·10⁻⁵) или значение не число — пишем один
   байт `0x00`, конец. (Нулевая скорость — ровно этот случай.)
2. Иначе `s = ceil(m)` — множитель. Если `s ≤ 3`, масштаб пишется в младшие 2 бита и флаг
   продолжения не ставится. Если `s > 3`, в младшие 2 бита пишется `s & 3`, а в бит `0x04`
   ставится флаг продолжения; остаток `s >> 2` дописывается VarInt-ом **после** основных байт.
3. Каждую компоненту нормируем: `p = round(((компонента / s) × 0.5 + 0.5) × 32766)`, получается
   15-битное беззнаковое (32766 — максимум шкалы).
4. Собираем 48-битное слово: `packedX` в биты 3..17, `packedY` в биты 18..32, `packedZ` в биты
   33..47, младшие 2 бита — масштаб, бит 2 — флаг продолжения.
5. Пишем: байт `packed & 0xFF`, байт `(packed >> 8) & 0xFF`, затем 4 байта **big-endian** от
   `packed >> 16` (то есть 32 бита), затем VarInt `s >> 2`, если флаг продолжения стоял.

Итого: первые два байта идут «как есть» (младшие биты слова), следующие четыре — big-endian.
Обратное чтение: `s = (байт1 & 3) | (VarInt << 2)`, если выставлен бит `0x04`;
`компонента = ((p & 32767) × 2 / 32766 − 1) × s`, где `p` берётся сдвигом слова на 3, 18 и 33.

Тест-векторы вики (для проверки реализации):

| Вектор | Байты |
|---|---|
| (0.0, 0.0, 0.0) | `0x00` |
| (1.0, 0.0, −1.0) | `0xF1 0xFF 0x00 0x00 0xFF 0xFF` |
| (10.0, 0.2, −5.0) | `0xF6 0xFF 0x40 0x01 0x05 0x1F 0x02` |
| (123457.0, 15.071, 0.0) | `0xF5 0xFF 0x7F 0xFF 0x00 0x07 0x90 0xF1 0x01` |

---

## 9. Чего найти не удалось

- Страницы вики, документирующей протокол 775, не существует (есть 773 и 776). Номера для 775
  взяты из minecraft-data и подтверждены тем, что список 776 совпадает с ней покомпонентно, а
  номера в ревизии 773 меньше на 1–2.
- Явного требования «Player Info Update строго до Add Entity» в **текущем** тексте вики нет; фраза
  взята из ревизии 2023 года (`oldid=2773028`, раздел «Spawn Player») и в текущем тексте
  отсутствует, так как пакет слит в Spawn Entity. Правило считается действующим, но проверено
  только этим источником.
- Порядок «Remove Entities» и «Player Info Remove» друг относительно друга на вики не оговорён.
- Смысл поля Data для типа player на вики не описан (в таблице Object data типа Player нет); вывод
  «писать 0» сделан по отсутствию записи.

---

## 10. Источники

- minecraft.wiki, `Java Edition protocol/Packets` — разделы Spawn Entity, Remove Entities, Update
  Entity Position, Update Entity Position and Rotation, Update Entity Rotation, Set Head Rotation,
  Teleport Entity, Player Info Update, Player Info Remove, Object data.
  https://minecraft.wiki/w/Java_Edition_protocol/Packets
- minecraft.wiki, `Java Edition protocol/Data types` — типы Angle, LpVec3, Fixed-point numbers,
  Prefixed Array, Game Profile. https://minecraft.wiki/w/Java_Edition_protocol/Data_types
- minecraft.wiki, `Java Edition protocol/Registries` — деление на Built-in и Synchronized registries.
  https://minecraft.wiki/w/Java_Edition_protocol/Registries
- minecraft.wiki, `Java Edition protocol/Entity metadata`, ревизия `oldid=3494102` (актуальна для
  26.1) — таблица ID типов сущностей.
  https://minecraft.wiki/w/Java_Edition_protocol/Entity_metadata?oldid=3494102
- minecraft.wiki, ревизия `oldid=2773028` (2023) — правило порядка Player Info Update и Spawn
  Player. https://minecraft.wiki/w/Java_Edition_protocol/Packets?oldid=2773028
- Примечание: ревизия `oldid=3544556` — это 1.21.10 / протокол 773 (в шапке прямо написано), в ней
  `login` = 0x30 и `player_info_update` = 0x44; брать номера оттуда нельзя.
- PrismarineJS/minecraft-data, ветка `data/pc/26.1`:
  - `protocol.json` — карта ID пакетов 775 и определения полей;
  - `entities.json` — ID типов сущностей (`player` = 155);
  - `version.json` — соответствие версии и протокола.
  https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/pc/26.1/entities.json
