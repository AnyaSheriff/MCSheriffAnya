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
- FileConfiguration: вложенные YAML-секции, скаляры без кавычек или в одинарных
  кавычках, String/int/long/boolean, contains/set, сохранение и перезагрузка.
  `config.yml` хранится в изолированном `data/`, исходный конфиг копируется из JAR.
  Списки, anchors, multiline и двойные кавычки пока отклоняются; это YAML-like,
  не полный YAML. Комментарии при saveConfig не сохраняются.
- ChatColor: стандартные legacy цвета/форматирование, translate/strip.

Точный список методов — в самостоятельно написанных классах `mcsa-bukkit/src`.
Рефлексия, NMS/CraftBukkit, собственные потоки, native/synchronized,
файловый/сетевой API Java, загрузчики классов и прямой доступ к мосту отклоняются.
ConcurrentMap/ConcurrentHashMap и AtomicBoolean/Integer/Long/Reference допускаются
на потоке плагина; также допускаются ThreadLocalRandom, преобразования TimeUnit
и RejectedExecutionException. sleep/timedJoin/timedWait и bulk-операции карты
с порогом параллелизма запрещены.
Executor/Thread/Timer и прочие concurrent API по-прежнему дают русский отказ.
softdepend принимается как список отсутствующих необязательных интеграций;
PluginManager видит только текущий Java-плагин. Классы внешних интеграций всё
равно проверяются: отсутствующий PlaceholderExpansion не подменяется заглушкой.
Проверяются владельцы, имена и **полные дескрипторы** методов/полей, constant pool,
лямбды, аннотации и наследование Bukkit. Все классы JAR проверяются, включая
недостижимые методы. Код плагина не исполняется на JVM переводчика;
META-INF/services и Class-Path не передаются компилятору, annotation processing выключен.
Проверка не заменяет песочницу Wasmtime: компоненту по-прежнему недоступны
файлы, сеть, окружение и privileged; действуют топливо, эпохи и память 32 МиБ.
Имена команд ядра и auth зарезервированы также на стороне хозяина.

Пока нет зависимостей от других плагинов, aliases и деклараций permissions в
plugin.yml, произвольных ресурсов/data-folder через java.io,
interact/move/inventory событий, entity damage с прямым виновником, спавна сущностей и Java-обёрток bossbar/scoreboard.
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
`CARGO_BUILD_JOBS=6 cargo test --release plugins::tests::translated_bukkit_jars -- --ignored --exact`.

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
