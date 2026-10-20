# Официальные Login, Join, Respawn и атрибуты26.2

Проведены2026-10-20 обычные сетевые пробы официального Java26.2, протокол776.
Входы получены через официальный launcher manifest. Server JAR:
SHA1`823e2250d24b3ddac457a60c92a6a941943fcd6a`,
SHA256`cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`.
JAR только хешировался и исполнялся; распаковки, mappings, reports и дампов
датапаков не было.

Исходные пакеты, команды, provenance и результаты:
[protocol-26.2-observations.json](testdata/protocol-26.2-observations.json).
Login Success дополнительно сохранён как
[43 байта полезной нагрузки](testdata/login-success-26.2.bin).
Сохраняются данные только собственного синтетического игрока McsaProbe.
Configuration registry payload не выгружается.

Установлено:

- Login Success содержит player UUID, username, свойства профиля и отдельный
  session UUID. В пробе session UUID не нулевой и отличается от player UUID.
- Join имеет ID`0x31`; после SpawnInfo находятся onlineMode и
  enforcesSecureChat. В обоих наблюдённых Join onlineMode=false.
- **SpawnInfo776 сохраняет прежний byte формат игровых режимов:** None
  предыдущего режима кодируется255, creative —1. В777 None кодируется0,
  creative —2. Не использовать для776 всю схему777.
- DimensionType разбирается как VarInt по закреплённой JSON-схеме.
  В этих записях наблюдались только0,2,3: они сами по себе не отличают VarInt
  от одного byte. Многобайтное значение отдельно проверено синтетическим тестом.
- Первый Join: creative1/previous=None. После survival и переподключения:
  survival0/previous=creative1.
- Четыре Respawn с ID`0x52` дали пары текущий/предыдущий режим0/1,1/0,3/1,2/3
  при переходах Nether→End→Overworld→Nether; flags3.
- Обычное дополнение `/attribute McsaProbe ` предложило40 уникальных
  атрибутов. Каждое имя проверено отдельным stick с attribute_modifiers.
  Полезная нагрузка каждого Slot разобрана целиком; получены все ID0..39
  без повторов. В частности movement_speed=26, bounciness=9.
- Ошибочный кандидатный protocol.json содержит31 атрибут; его числовые ID
  использовать нельзя. Это подтверждённое расхождение таблицы с сервером.

Список40 и точные ID лежат в `verified.attribute_ids`; команды и исходные
слоты — в `attributes.cases`. Пример отличается от простого угадывания ID по
порядку attributes.json: числовое значение реально получено по сети.

Все три отдельные серверные пробы завершились exit0. Новый порт26028,
loopback-only, отдельные временные конфигурации/миры, affinity двух логических
CPU. После окончания порт свободен, процессы проб завершены.

Переиспользованные инструменты:

```bash
python3 -B tools/protocol-bridge/observe.py --help
python3 -B tools/protocol-bridge/probe_entities.py --help
python3 -B tools/protocol-bridge/probe_item_slots.py --help
python3 -B tools/protocol-bridge/test_protocol_26_2_probe.py
```

Для776 последние две пробы требуют `--server-metadata` официального26.2 и
`--registry-cache` из generate_registry_26_2. Respawn-проба выполняется с
`--respawns-only`, атрибутная — с `--discover-attributes`. Обе проверки входов
используют helper official_26_2_inputs.py.

`analyze_protocol_26_2.py` сопоставляет оба предполагаемых формата SpawnInfo
с известными режимами команд и требует единственного подходящего варианта;
отклонённая optionalVarInt-гипотеза сохранена в результате. Он не предполагает
число40 заранее и отклоняет неполный набор атрибутов.

Восемь офлайн-тестов проверяют новые записи776, прежние75 записей атрибутов
775/777, старые Join/Success777 и отказ на повреждённых/неполных данных.
В Rust добавлены Login Success776, внутренние обработчики Join/Respawn776
и перевод наблюдённых Attribute ID. Проверяется, что все40 атрибутов776
совпадают с наблюдёнными777; для остальных реестров776 не становится
автоматическим псевдонимом777. Join/Respawn проверены по этим записям и
синтетическим проекциям в775/777; проекции не являются новыми записями
других официальных серверов.

Полный release workspace:2707passed/80ignored/0failed; Clippy all-targets
с `-D warnings` и release build прошли. Публичный переводчик пакетов
по-прежнему отклоняет776; цельный клиентский маршрут и основной профиль26.2
этим этапом не включены.

Ограничения: onlineMode=true и online-session не проверялись. Таблицы всех
остальных реестров, полный gameplay776, первичный профиль26.2 и публичный
допуск этой пробой не подтверждены.

## Обычные Light, Slot и entity payloads:2026-10-20

Следующее отдельное окно official776 на26028 завершено: Light02 PASS,
Slots01 8/8 PASS, Entities02 PASS. Все Java exit0, после последней пробы
процессов Java нет, bind26028 успешен. Использованы два логических CPU;
Rust/Cargo параллельно не запускались.

- Light: обычные MapChunk после размещения/удаления факела дали уровень14→0.
  LightData использует массивы BE-long для масок, как775. В отдельной секции
  y96..111 нашими fill/setblock размещены300 различных устойчивых состояний:
  обе записи имеют direct width15,4096 непустых блоков и0 жидкостей; все4096
  ID совпали с авторскими командами и закреплёнными JSON. Это не экспорт мира:
  сохраняется только выбранная секция обычного сетевого пакета и LightData.
- Slots: stone3, custom_name6, damage3=7, attribute_modifiers16 с
  attack_damage3, map_color45=1193046, swing_animation40=[2,7],
  wolf/collar85=14 и пустой Slot. Проверены точные item/component IDs,
  значения и полное исчерпание каждой полезной нагрузки. Это не проверка
  всех компонентов или инвентарных транзакций.
- Entity metadata: у авторской коровы baby index16=true; у slime и magma_cube
  размер находится в index18 с VarInt4/3. Три перемещения коровы подтверждают
  обычные три i16 delta /4096 и yaw/pitch перед onGround в MoveLook776.
  Начальная позиция взята из SpawnEntity2.5/80/2.5, итоговые позиции совпали
  со всеми тремя командами. Общее поведение остальных сущностей не проверено.
- Короткий teleport confirmation (только VarInt ID) отправлял наш probe;
  официальный776 принимал сессию. Реального GUI-client ACK ещё нет.

Сохранены оба исходных FAIL: Light01 ошибочно выбрал test_instance_block,
создав block entity; после исключения известных block-entity типов и аудита
выбранных300 состояний Light02 прошёл. Entities01 ошибочно начинал сумму
смещений от целых координат summon; Entities02 использует наблюдённый Spawn.
Ни формат пакетов, ни ожидаемые delta/angles ради повторов не менялись.

Локальные неизменённые исходные result.json находятся в handoff:
`verification-protocol776-ordinary-light-02/light/result.json`,
`verification-protocol776-ordinary-slots-01/slots/result.json`,
`verification-protocol776-ordinary-entities-02/entities/result.json`.
Их SHA256 соответственно:

- `light-02`: `d42b0e8fa613fdb6759057748b0292c0c7772e835a520ce7c54232b0f47c4746`.
- `slots-01`: `f565f09e18a9cb6109d13474b138d8fbcefa48675b956e5d0b1c4a484a5355a1`.
- `entities-02`: `ba43741cda1d2df28bebc4271a27f3cd908b355116e5b8f41cb5afe1e9e6c38b`.

Воспроизводимый runner: `run_ordinary_776.py`; порядок Light→Slots→Entities,
каждый этап имеет отдельный временный мир. `--stage` позволяет повторить
только выбранную пробу в новом каталоге, предыдущие FAIL не перезаписываются.
Python6/6 проверок пробы прошли. Эти наблюдения ещё не означают, что Rust
поддерживает весь776: payload helpers готовятся отдельно, public wire,
клиентский допуск и основной профиль остаются закрыты.
