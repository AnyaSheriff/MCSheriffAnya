# mcsa-bukkit и перевод Bukkit JAR

Рабочее ограниченное подмножество Bukkit: JAR остаётся обычным Java-плагином
с `plugin.yml`. Переводчик проверяет байткод, компилирует его TeaVM 0.12.3
(Java → C → WASI 0.2), добавляет WIT-мост и выдаёт компонент для `plugins/`.
На сервере JVM не запускается. Исходники Bukkit/Spigot/Paper не использовались.

Нужны Linux x86-64, Python 3.12+, JDK 17+, Maven и доступ к Maven Central/GitHub.
TeaVM, ASM и SnakeYAML загружаются Maven из Central; WASI SDK 34,
wit-bindgen 0.63.0 и wasm-tools 1.262.0 — из официальных GitHub releases.
Для JAR, собранных новым JDK, нужен javac, умеющий читать их версию классов.
SDK подключается как provided и не включается в JAR плагина.
Зависимости кешируются в `.m2`, инструменты — в `.toolchain`, сборка — в `target`.

```sh
python3 tools/plugin-translator/translate.py --build-sdk
# SDK для javac: tools/plugin-translator/target/mcsa-bukkit.jar
python3 tools/plugin-translator/translate.py MyPlugin.jar --check-only --report missing.json
python3 tools/plugin-translator/translate.py MyPlugin.jar --output plugins
```

Выход: `plugins/<имя-из-plugin.yml>/{plugin.toml,plugin.wasm,data/}`.
Существующая папка не перезаписывается. `--check-only` проверяет ссылки API,
но окончательное решение о поддержке стандартной библиотеки Java принимает
TeaVM при полной компиляции. Ненулевой код выхода означает отказ; пустышка
вместо компонента не создаётся. Имя папки нормализуется в нижний регистр.

## Состояние порта FourLegacy

Текущий проверенный набор и ограничения: [fourlegacy-compatibility.md](../../docs/fourlegacy-compatibility.md).
SDK также передаёт снимки состояния игрока и эффектов, базовые типы зелий,
PotionMeta, свет/возраст/состояние блоков и их восстановление, случайные такты,
предметы в мире, скорости сущностей, звуки и частицы из ограниченного каталога.
Есть отменяемые события смены выбранной ячейки, еды и обмена рук; GUI drag
получает исходный курсор и выбранные raw slots. Урон с прямым игроком-виновником
доставляет EntityDamageByEntityEvent. Обёртки мобов/снарядов, нативные контейнеры
и генераторы пока не завершены; успешный аудит одного JAR не подменяется
добавлением классов без реальной доставки событий.

## Поддерживаемые операции

- `JavaPlugin`: onLoad/onEnable/onDisable, getName/getServer, getCommand,
  getConfig/reloadConfig/saveConfig/saveDefaultConfig. Команды описываются
  в plugin.yml (description/usage), исполняются через onCommand или setExecutor.
- `CommandSender`, `CommandExecutor`, `TabCompleter`, PluginCommand.setTabCompleter,
  JavaPlugin.onTabComplete. Пустой результат по умолчанию дополняется именами
  игроков. WIT completion необязателен: старые компоненты продолжают загружаться.
  Подсказки ограничены 100 строками по 512 байт, проверяются права команды.
- `Bukkit`/`Server`: игроки по имени/UUID, онлайн, broadcastMessage, миры,
  PluginManager.registerEvents, синхронный BukkitScheduler.
- Player: имя, UUID, положение, мир, здоровье (чтение/смена; только Java), эффекты зелий (выдача/снятие), gamemode (чтение/смена),
  teleport, sendMessage, kickPlayer, isOp и консервативная hasPermission:
  узлы разрешены только оператору уровня 4, как в текущем ядре.
- Location, World, Block, ограниченный Material enum, ItemStack,
  Inventory/PlayerInventory.setItem: обычные ячейки 0..35 (hotbar 0..8).
  Миры: world, world_nether, world_the_end; блоки только в загруженных чанках,
  время суток, погода и мгновенная смена центра/размера WorldBorder. Полного чтения инвентаря и addItem пока нет.
- NamespacedKey, Adventure Component/TextComponent: text/empty/append, цвета,
  декорации, legacyAmpersand/legacySection; JSON идёт в проверяемый presentation
  хозяина для чата и action bar. Компоненты неизменяемые; hex legacy и hover/click API пока нет.
- Listener/@EventHandler: join, quit, AsyncPlayerChatEvent, command preprocess,
  break/place, EntityDamageEvent для урона игроку (getDamage/setDamage/отмена),
  PlayerDeathEvent (игрок и getKeepInventory, без изменения дропа). Combat-экспорт
  необязателен: старые компоненты загружаются без него. Приоритеты и ignoreCancelled работают внутри одного плагина;
  между компонентами сохраняется порядок ядра. Чат изменяется или отменяется;
  команды и блоки отменяются. Переписывание команды и замена системного join/quit
  сообщения отклоняются. AsyncPlayerChatEvent доставляется синхронно.
- runTask, runTaskLater, runTaskTimer, scheduleSyncDelayedTask,
  scheduleSyncRepeatingTask, отмена задач и BukkitRunnable. Задача с delay=0
  выполняется на следующем такте; period должен быть положительным.
- FileConfiguration/YamlConfiguration: вложенные секции, типизированные скаляры,
  одинарные/двойные кавычки, block/flow списки и карты, getKeys/getConfigurationSection,
  сохранение и перезагрузка. YAML без тегов, anchors, merge keys и block-строк;
  неизвестный синтаксис отклоняется, комментарии при записи не сохраняются.
  Все 13 YAML-ресурсов FourLegacy сверены с безопасным SnakeYAML на JVM переводчика.
- getDataFolder/getResource/saveResource, ограниченные File/Path/Files:
  только относительные пути внутри `data/`, до 4 МиБ, без ссылок и `..`.
  Чтение/запись/копирование/атомарное перемещение выполняет хозяин.
- ItemStack/ItemMeta: копирование, имена/описания Adventure, износ/максимальная
  прочность, зачарования, PDC (Byte/int/long/double/String, списки String/double).
  PDC сохраняется с предметом; Entity/Player PDC сохраняется по UUID в data/
  компонента, с объединением записей между тактами и flush при выгрузке; JSON метаданных до 16 КиБ проверяет хозяин.
  PlayerInventory читает Java-снимок, меняет слоты, броню и вторую руку,
  добавляет/удаляет стопки; неизвестные довески сохраняются через токены хозяина.
  Токены ограничены 1024 записями на плагин; устаревший токен даёт явную ошибку.
- Attribute/AttributeInstance/AttributeModifier: основа, значение, модификаторы
  игрока; pending-правки видны сразу, выполнение — через очередь подключения.
  Модификаторы только в namespace плагина. Атрибуты пока живут до переподключения;
  scale и некоторые иные атрибуты передаются клиенту, но не полностью учитываются физикой.
- ChatColor: стандартные legacy цвета/форматирование, translate/strip.

Точный список методов — в самостоятельно написанных классах `mcsa-bukkit/src`.
Рефлексия, NMS/CraftBukkit, собственные потоки, native, произвольный файловый/
сетевой API Java, загрузчики классов и прямой доступ к мосту отклоняются.
Методы synchronized раскрываются в однопоточном компоненте; явные мониторы запрещены.
Стандартные equals/hashCode/toString record раскрываются без ObjectMethods/рефлексии.
ConcurrentMap/ConcurrentHashMap и AtomicBoolean/Integer/Long/Reference допускаются
на потоке плагина; также допускаются ThreadLocalRandom, преобразования TimeUnit
и RejectedExecutionException. sleep/timedJoin/timedWait и bulk-операции карты
с порогом параллелизма запрещены.
newSingleThreadExecutor, newFixedThreadPool(1), execute/submit, Future и shutdown/close имеют shim:
все такие executors компонента отправляют работу в общую FIFO-очередь хозяина.
До 256 задач, до 64 задач за callback между тактами; новые задачи не выполняются
в том же callback. awaitTermination после shutdown ограниченно опустошает очередь
при выгрузке, без ожидания потока. Незавершённый Future.get не блокирует хозяина.
Неиспользуемая lambda-фабрика ThreadFactory удаляется; её Thread
никогда не создаётся. Прямой new Thread(...).start(), Timer, другие executors
и реальные параллельные операции по-прежнему дают русский отказ.
softdepend принимается как список отсутствующих необязательных интеграций;
PluginManager видит только текущий Java-плагин. При объявленной softdepend
PlaceholderAPI доступен только тип отсутствующего расширения: создание/
регистрация дают явную ошибку; PluginManager.isPluginEnabled возвращает false.
Реальная PlaceholderAPI и её callbacks не предоставляются.
Проверяются владельцы, имена и **полные дескрипторы** методов/полей, constant pool,
лямбды, аннотации и наследование Bukkit. Все классы JAR проверяются, включая
недостижимые методы. Код плагина не исполняется на JVM переводчика;
META-INF/services и Class-Path не передаются компилятору, annotation processing выключен.
Проверка не заменяет песочницу Wasmtime: WASI не даёт файлов, сети, окружения
или privileged; файловые операции идут через ограниченный storage, HTTPS —
только через явно разрешённые вызовы хозяина; действуют топливо, эпохи и память 32 МиБ.
Имена команд ядра и auth зарезервированы также на стороне хозяина.

Пока нет зависимостей от других плагинов, aliases и деклараций permissions в
plugin.yml, полного java.io/произвольного YAML,
interact/move и событий нативных контейнеров, entity damage с прямым виновником, спавна сущностей и Java-обёрток bossbar/целей scoreboard.
Существующий WIT presentation и Rust SDK сохраняют bossbar/scoreboard/actionbar.
Причины урона сведены к поддерживаемым типам ядра: точное соответствие Paper
и порядок относительно износа брони ещё требуют проверки. Отмена сейчас не
откатывает износ, уже рассчитанный вызывающим путём ядра.
Чтение здоровья использует текущий Java UI-снимок хозяина; Bedrock UI не поддерживается.
Прямые обращения к отсутствующим Bukkit API дают отказ с русским отчётом.

## Проверки

```sh
python3 tools/plugin-translator/test-inspector.py
python3 tools/plugin-translator/build-fixtures.py
# Изолированное ядро, порт 26210, остановка своего процесса по завершении:
python3 tools/plugin-translator/run-blackbox.py --port 26210
```

Mineflayer берётся из `tools/blackbox/node_modules`, при отсутствии — из
`~/node_modules`. Три fixture собираются обычным javac/jar только против SDK:
команда с Tab, join+отмена чата/команды/ломания/установки, repeating с config.yml.
`blackbox.cjs` проверяет реальные сообщения и неизменившийся отменённый блок.
Для теста хозяина положите три результата в `tmp/java-test/plugins`, затем
`CARGO_BUILD_JOBS=4 nice -n 19 cargo test --release plugins::tests::translated_bukkit_jars -- --ignored --exact`.

Контракты сверялись только с публичной документацией:
[JavaPlugin](https://hub.spigotmc.org/javadocs/bukkit/org/bukkit/plugin/java/JavaPlugin.html),
[Player](https://hub.spigotmc.org/javadocs/bukkit/org/bukkit/entity/Player.html),
[EventHandler](https://hub.spigotmc.org/javadocs/bukkit/org/bukkit/event/EventHandler.html),
[Scheduler](https://hub.spigotmc.org/javadocs/bukkit/org/bukkit/scheduler/BukkitScheduler.html),
[FileConfiguration](https://hub.spigotmc.org/javadocs/bukkit/org/bukkit/configuration/file/FileConfiguration.html),
[TeaVM C interop](https://teavm.org/docs/c-backend/native.html).

Новые контракты сверены только с публичными Paper Javadoc:
[NamespacedKey](https://jd.papermc.io/paper/1.21.11/org/bukkit/NamespacedKey.html),
[Damageable](https://jd.papermc.io/paper/1.21.11/org/bukkit/entity/Damageable.html),
[PotionEffect](https://jd.papermc.io/paper/1.21.11/org/bukkit/potion/PotionEffect.html),
[EntityDamageEvent](https://jd.papermc.io/paper/1.21.11/org/bukkit/event/entity/EntityDamageEvent.html),
[WorldBorder](https://jd.papermc.io/paper/1.21.11/org/bukkit/WorldBorder.html).

## Разрешения HTTPS и настройки мира

В plugin.yml объявляется `network-hosts: [api.example.org]`; переводчик переносит
список в `network_hosts` plugin.toml. По умолчанию доступа нет. Администратор
добавляет в **config/mcsa.properties** строку
`plugin-network-hosts=fourlegacy:api.example.org|other.example.org` и перезапускает
ядро. Работает только пересечение декларации и разрешения; при загрузке консоль
печатает разрешённые хосты каждого плагина. Маски, IP, порты и URL в декларации
не допускаются. Изменение plugin.yml самим плагином разрешения ядра не меняет.

Java API: `org.mcsa.api.Https.get(url)`, `post(url, byte[])` возвращают handle;
`poll(handle)` возвращает null до завершения, затем Response.status()/body()/text().
Только HTTPS GET/POST, порт 443, без credentials/redirects/cookies/system proxies.
DNS закрепляется за проверенными публичными адресами; внутренние адреса запрещены.
Тело до 32 КиБ, ответ до 64 КиБ, срок вместе с DNS — 10 секунд; один старт в секунду,
до четырёх незабранных ответов на плагин и до 64 активных запросов на всё ядро.
Окружение, auth/password данные и файлы вне data/ не передаются.
Настоящий внешний TLS-запрос пока не является частью автоматических тестов.

Настройки ядра: `monster-spawn-multiplier=1` — множитель лимита естественных
монстров (0..16, без спавнеров/команд); `ore-retention-percent=100` — доля руды
только в новых чанках (0..100). Значения по умолчанию сохраняют прежнее поведение.
Это независимые настройки ядра, пока не зеркалирующие конфиг FourLegacy.

Публичные контракты: [PersistentDataContainer](https://jd.papermc.io/paper/1.21.11/org/bukkit/persistence/PersistentDataContainer.html),
[ItemStack](https://jd.papermc.io/paper/1.21.11/org/bukkit/inventory/ItemStack.html),
[ItemMeta](https://jd.papermc.io/paper/1.21.11/org/bukkit/inventory/meta/ItemMeta.html),
[reqwest ClientBuilder](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html).

[Attribute (Paper)](https://jd.papermc.io/paper/1.21.11/org/bukkit/attribute/Attribute.html), [значения и пределы](https://minecraft.wiki/w/Attribute).

## Пользовательские сундуки и команды scoreboard

`Bukkit.createInventory(holder, size, Component)` создаёт сундук на 9..54 ячейки,
кратно 9. `openInventory`, `getOpenInventory`, `closeInventory`, держатели и
`InventoryView` связаны с настоящим окном Java-клиента. Ядро проверяет предметы
и ItemMeta тем же путём, что и инвентарь игрока. Лимит — 512 живых сундуков на
компонент; после закрытия последним смотрящим мост освобождает дескриптор,
сохраняя Java-объект для повторного открытия. Выгрузка компонента закрывает окна.

`InventoryOpenEvent` можно отменить до открытия. `InventoryClickEvent` приходит
до перемещения вещей; отмена возвращает клиенту авторитетные ячейки и курсор.
Обычный/правый/Shift/цифровой/двойной щелчок, выброс, вторая рука и творческое
копирование выполняются ядром. Перетаскивание собирает ячейки, посылает
`InventoryDragEvent` при завершении и распределяет вещи только после разрешения.
`InventoryCloseEvent` приходит при закрытии, замене окна и выходе. Курсор
возвращается в рюкзак, переполнение выпадает. События пока только для своих GUI,
не для обычных сундуков, крафта и торговли. Изменение окна из обработчика щелчка
следует планировать через `runTask`, как предписывает Javadoc.

Нативный `getOpenInventory` даёт снимок верхних ячеек и инвентарь игрока снизу;
запись в верхние ячейки нативного окна явно отклоняется по-русски. Блоковые
держатели и специализированные `AnvilView`/`MerchantInventory` ещё не реализованы.

`getScoreboardManager().getMainScoreboard()` поддерживает команды с именем,
цветом, Component-префиксом/суффиксом и набором участников. Изменения уходят
всем Java-клиентам, новый игрок получает снимок команд; перенос записи между
командами удаляет её из старой. Команды ограничены пространством компонента и
очищаются при выгрузке. Цели/счёт/личные доски Java пока не поддержаны.
`getPlayerExact`, текущий такт, векторы, расстояния/направление Location и поиск
ближайших игроков по ограничивающему прямоугольнику также доступны.

Сигнатуры и контракт сверены только с публичными Javadoc:
[Bukkit](https://jd.papermc.io/paper/1.21.11/org/bukkit/Bukkit.html),
[InventoryView](https://jd.papermc.io/paper/1.21.11/org/bukkit/inventory/InventoryView.html),
[InventoryClickEvent](https://jd.papermc.io/paper/1.21.11/org/bukkit/event/inventory/InventoryClickEvent.html),
[Team](https://jd.papermc.io/paper/1.21.11/org/bukkit/scoreboard/Team.html),
[Vector](https://jd.papermc.io/paper/1.21.11/org/bukkit/util/Vector.html),
[Location](https://jd.papermc.io/paper/1.21.11/org/bukkit/Location.html).

Числа C/WASI проверяет `python3 tools/plugin-translator/test-decimal-runtime.py` (также входит в `test-inspector.py`). Платформенный порт заменяет разбор float/double и вставку чисел в StringBuilder: десятичная запись сохраняет исходное число, включая subnormal и signed zero. Поддержаны ASCII decimal/hex-float, суффиксы Java, NaN/Infinity; длина входа ограничена 4096 символами. Имена материалов, ключей и полей компонентов нормализуются как ASCII; текст сообщений не меняется. Fixture bot проверяет эти пути в WASM, в том числе исключение NumberFormatException.
