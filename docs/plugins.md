# Плагины MCSheriffAnya: WebAssembly и Java

Плагин — это модуль WebAssembly (компонент), написанный под API `mcsa:api`.
Ядро запускает его встроенным [wasmtime](https://wasmtime.dev): никакой JVM, никакого
JDK на сервере. Плагины грузятся, выгружаются и перезагружаются на ходу, без
перезапуска сервера.

Фаза 1 — это хозяин в ядре, `/plugman`, API (WIT) и примеры на Rust.
Ограниченный Java SDK `mcsa-bukkit` и переводчик `.jar → .wasm` доступны в
[tools/plugin-translator](../tools/plugin-translator/README.md). Он принимает
обычные Bukkit JAR только в поддерживаемом подмножестве; неподдерживаемые API
отклоняются с русским отчётом. Полная совместимость Bukkit не заявляется.

Хозяин включается cargo-возможностью `plugins` (по умолчанию включена).
`cargo build --release --no-default-features` собирает ядро без плагинов:
команды `/plugman` и папок `plugins/`, `modules/` тогда нет.

## Сборка и скорость

Исполнитель — Wasmtime 49.0.2 с Cranelift JIT: при загрузке компонент компилируется
в машинный код с `OptLevel::Speed`. Топливо, часы эпох и ограничения памяти
сохраняются. В release зависимости используют обычный `opt-level = 3`, без
переопределений `"s"`: скорость загрузки и вызовов важнее размера бинарника.
Символы удаляются автоматически (`strip = "symbols"`); для отладки можно
собрать с `CARGO_PROFILE_RELEASE_STRIP=none`.

У Wasmtime выключены возможности по умолчанию; оставлены `runtime`, `cranelift`,
`component-model`, `gc`, `gc-null`, `std`. Возможность `gc` нужна для
исключений Java/TeaVM; WASM GC и function references выключены в Config,
куча Java остаётся линейной под прежними лимитами. У WASI включён только `p2`: Preview 0/1/3 не нужны.
Preview 2 транзитивно включает `wasmtime/async`, хотя хозяин использует
`add_to_linker_sync`. Удалять эту зависимость или отдельные импорты WASI
ради размера нельзя без изменения совместимости компонентов.

Замер без запуска сервера (Linux, нужны собранные примеры):

```sh
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp nice -n 19 cargo test --release -j 2 \
  plugins::tests::benchmark_host -- --exact --ignored --nocapture --test-threads=1
```

Он печатает RSS тестового процесса до создания хозяина, с пустым хозяином и
после загрузки `chat-filter`; время создания хозяина, первой загрузки с JIT
и 100 000 вызовов `Host::chat` после 1000 прогревочных вызовов. Вызовы включают
WIT ABI, логику фильтра, топливо, эпохи и статистику; очередь потока плагинов
и сетевой сервер в этот замер не входят. RSS всего сервера — отдельный замер.
Проверки переноса и размеры бинарника — в [REPORT.md](../REPORT.md).

## Две папки

```
plugins/<имя>/plugin.toml      обычные плагины: песочница, ограниченный API
plugins/<имя>/<имя>.wasm
modules/<имя>/plugin.toml      доверенные модули: тот же API и ещё `privileged`
modules/<имя>/<имя>.wasm
```

Рядом с файлами плагин получает свои `config.toml` и папку `data/`.

| | `plugins/` | `modules/` |
|---|---|---|
| События, команды, игроки, чат, планировщик, мир, хранилище | да | да |
| Интерфейс `privileged` (команды от консоли, список плагинов) | нет | да |
| Топливо на вызов | 240 млн (включая GC рантайма) | 400 млн |
| Срок вызова по часам | ~200 мс | ~200 мс |
| Память | 32 МиБ | 256 МиБ |
| Файлы, сеть, окружение, ввод (WASI) | нет | нет |

Плагин из `plugins/`, которому нужен `privileged`, не загружается — сервер
говорит, что его место в `modules/`. Модули ставит администратор: это
доверенный код. Глубокие зацепки (пакеты, механики, генерация мира) лягут
рядом с `privileged` отдельными интерфейсами — только для модулей.

## plugin.toml

```toml
name = "hello"                  # a-z, 0-9, «-», «_»; совпадает с именем папки
version = "0.1.0"
api = "0.1"                     # версия API: major.minor должны совпасть с сервером
author = "Аня"
description = "Приветствие и команда /hello"
main = "hello.wasm"             # необязательно; по умолчанию <имя>.wasm
dependencies = ["other"]        # необязательно; должны быть загружены раньше
```

## /plugman

Только консоль и операторы высшего уровня (4).

| Команда | Что делает |
|---|---|
| `/plugman list` | что загружено и что лежит на диске |
| `/plugman info <имя>` | версия, автор, описание, зависимости, команды |
| `/plugman load <имя>` | загрузить (сперва ищется в `modules/`, потом в `plugins/`) |
| `/plugman unload <имя>` | выгрузить (если от него зависят — отказ) |
| `/plugman reload <имя>` | выгрузить и загрузить заново вместе с зависящими |
| `/plugman reload all` | то же для всех |

При запуске сервер загружает всё из `modules/`, затем из `plugins/`, в порядке
зависимостей. При остановке всё выгружается до записи мира (`disable`).

Плагин, который упал во время вызова (паника, ловушка wasm, вышло топливо,
вышел срок, кончилась память), выгружается, в лог идёт строка
`Плагин имя упал: причина — выгружен`. Сервер не падает и такт не встаёт.

## Когда что вызывается

- **События** (`on-player-join`, `on-chat`, `on-block-break` …) запрашивает
  поток, который применяет действие игрока; он ждёт ответ отдельного потока
  плагинов без замков ядра. Отмена блока, команды или чата действует до
  применения соответствующего действия.
- **Планировщик** (`run-later`, `run-repeating`) запрашивает поток такта,
  после функций набора данных и до работы миров, без замков мира. Задача с
  задержкой 0 срабатывает на ближайшем такте.
- Все вызовы в плагины идут на одном отдельном потоке: по очереди загрузки,
  один за другим. Из плагина нельзя заново войти в хозяина (команда плагина из
  плагина, `/plugman` из модуля) — такой вызов отвечает отказом.
- Обработчики Java-игроков вызываются для чата, команд, входа, выхода,
  ломания и установки блоков; для Bedrock пока — чат, вход и выход
  (блоки Bedrock — позже).

## Java SDK и WIT

Java SDK сохраняет имена `org.bukkit` в поддерживаемом подмножестве;
`ru.mcsa.bukkit` — внутренний мост переводчика, не API для исходного плагина.
Компонент использует `mcsa:api@0.1.0` из `wit/mcsa-api.wit`.
Обычные плагины остаются в `plugins/`; импорт `privileged` разрешён только
модулям из `modules/`. Переводчик не повышает доверие плагина.

## API 0.1

Игрок опознаётся UUID строкой («8-4-4-4-12»). Имена блоков и предметов — как
в игре, с приставкой или без.

**Жизненный цикл** (`lifecycle`): `enable()` — плагин загружен, здесь
регистрируют команды и задачи; `disable()` — выгружается.

**События** (`events`, что сервер вызывает у плагина):

| Обработчик | Ответ |
|---|---|
| `on-player-join(who)`, `on-player-quit(who)` | — |
| `on-chat(who, message)` | `pass`, `cancel` или `replace(текст)` |
| `on-block-break(who, pos, block)` | `allow` / `deny` |
| `on-block-place(who, pos, block)` | `allow` / `deny` (`pos` — блок за гранью щелчка) |
| `on-player-command(who, command-line)` | `allow` / `deny` (любая команда игрока, без `/`) |
| `on-command(from, name, args)` | `command-result { success, message }` — команда, которую плагин зарегистрировал |
| `on-task(task)` | — задача планировщика |

Если обработчиков несколько, они идут по очереди загрузки; первый отменивший
останавливает цепочку, изменённый текст чата передаётся следующему.

**Что плагин может вызвать:**

| Интерфейс | Функции |
|---|---|
| `log` | `write(level, message)` |
| `server` | `api-version`, `current-tick`, `broadcast(message)`, `online-players`, `find-player(name)` |
| `player` | `send-message(uuid, text)`, `teleport(uuid, location)` (в том числе в другое измерение), `give-item(uuid, stack)`, `kick(uuid, reason)` |
| `world` | `get-block(pos)` (none — чанк не загружен), `set-block(pos, block)`, `spawn-location(dimension)` |
| `scheduler` | `run-later(delay, task)`, `run-repeating(delay, period, task)`, `cancel(handle)` |
| `command` | `register(spec)` — имя, описание, использование, `level` (0 — всем … 4), `permission` (поле зарезервировано API; хозяин пока проверяет только `level`) |
| `storage` | `config(default-text)`, `save-config(text)`, `read/write/delete/transfer` в `data/` (путь относительный, без `..` и ссылок; до 4 МиБ) |
| `https` | GET/POST через ядро, только объявленные И разрешённые администратором хосты; [настройка](../tools/plugin-translator/README.md#разрешения-https-и-настройки-мира) |
| `inventory` / `attributes` | Проверенные предметы/PDC, снимки слотов и именованные атрибуты игроков; Java UI |
| `privileged` (модули) | `run-console-command(command)`, `loaded-plugins()` |

Ограничения 0.1, о которых надо знать:
- `player.send-message` пока доходит только до игроков Java (Bedrock — позже);
  ответ команды плагина (`message`) доходит всем.
- Команды загруженных плагинов включаются в дерево при входе Java-клиента.
  Необязательный экспорт `mcsa:api/completion` даёт подсказки аргументов:
  максимум 100 строк по 512 байт, с проверкой уровня прав. После добавления
  новых команд через hot-load дерево обновляется при следующем входе.
  Содержимое подсказок запрашивается при каждом Tab; auth не передаётся плагинам.
- Имена команд ядра (`op`, `stop`, `give`, `plugman` …) плагину занимать нельзя.
- У каждого плагина не больше 256 задач и 64 команд.

## Пример на Rust

Примеры лежат в `plugins-examples/` (отдельная сборка, в основную не входит):
`hello` (приветствие и `/hello`), `chat-filter` (отмена и правка чата),
`protect-spawn` (нельзя ломать и ставить рядом с точкой появления; радиус в
`config.toml`), `modinfo` (модуль с `privileged`) и испытательный `misbehave`.

```sh
rustup target add wasm32-wasip2
cd plugins-examples
mkdir -p tmp
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp nice -n 19 cargo build --release --target wasm32-wasip2 -j 2
./install.sh ..                  # кладёт в ../plugins/<имя>/ и ../modules/<имя>/
```

Перед `cargo test --release` соберите примеры: тесты хозяина загружают эти
компоненты из `plugins-examples/target/wasm32-wasip2/release` и завершаются
ошибкой, если их нет. Сетевой сервер для этих проверок не запускается.
Cargo-команды выполняйте последовательно, по одной.

Свой плагин — это крейт `cdylib` с зависимостью от `mcsa-sdk` (обёртка над
WIT, `wit-bindgen`):

```rust
use mcsa_sdk::{export_plugin, ChatResult, Player, Plugin};

struct Mine;

impl Plugin for Mine {
    fn on_player_join(who: Player) {
        mcsa_sdk::server::broadcast(&format!("{} с нами", who.name));
    }

    fn on_chat(_who: Player, message: String) -> ChatResult {
        if message.contains("spam") { ChatResult::Cancel } else { ChatResult::Pass }
    }
}

export_plugin!(Mine);
```

Собирать на `wasm32-wasip2`; рядом с `.wasm` положить `plugin.toml`.
Плагин на любом другом языке с поддержкой компонентов (C, Go, JS, Python …)
тоже подходит, если собран под `wit/mcsa-api.wit`.

## Оформление списка игроков и HUD

Интерфейс `mcsa:api/presentation` (Rust: `mcsa_sdk::presentation`) доступен
обычным плагинам. Текст передаётся JSON-компонентом, например
`{"text":"Привет","color":"green"}`, и кодируется хозяином в anonymous NBT.
Максимум 16 КиБ на компонент, JSON проверяется до отправки. Поддерживаются
text, translate/with/fallback, keybind, цвета, декорации, font, insertion,
click_event и hover_event/show_text. Произвольного доступа к пакетам нет.

| Вызов | Назначение |
| --- | --- |
| `get-player-info(uuid)` | Ping (мс), здоровье и максимум, мир, display-name (JSON), еда, уровень, опыт, уровень оператора |
| `get-server-info()` | TPS, MSPT, online, max-players |
| `set-tab-header-footer(uuid, header, footer)` | Заголовок и подвал конкретного Java-клиента; отключает для него подвал ядра |
| `reset-tab-header-footer(uuid)` | Освобождает свой заголовок/подвал, ядро возобновляет TPS на ближайшем обновлении |
| `set-display-name(uuid, text)` | Имя для плагинов, возвращаемое в get-player-info |
| `set-player-list-name(uuid, text)` | Имя игрока в TAB у всех Java-клиентов |
| `set-team` / `remove-team` | Префикс, суффикс, цвет, видимость nametag и участники для конкретного зрителя |
| `set-objective` / `remove-objective` | Цель табло конкретного зрителя |
| `set-display-slot` | Слоты list, sidebar, below_name, sidebar.team.COLOR; none освобождает свой слот |
| `set-score` / `remove-score` | Число, необязательное имя строки и fixed-компонент вместо числа |
| `set-boss-bar` / `remove-boss-bar` | Название, доля 0..1, цвет, деления |
| `set-action-bar` | Сообщение над панелью предметов |
| `send-component` / `broadcast-component` | Форматированное сообщение, в том числе для собственного обработчика чата |
| `reset-player` | Удаляет всё оформление вызывающего плагина для этого игрока |

UI-вызовы возвращают false при неверных данных, отсутствии игрока или
Bedrock-подключении. Bedrock в текущем хозяине не переводит эти Java-пакеты.
Компоненты чата проходят существующий путь доставки чата. World возвращается
как world, world_nether, world_the_end. Снимок состояния игрока обновляется
подключением; ping меняется после подтверждённого keep-alive.

Идентификаторы ресурсов локальны плагину, до 128 байт; хозяин создаёт имена
протокола самостоятельно. На зрителя и плагин: до 256 teams, 32 objectives,
32 bossbars, 2048 записей кэша (включая scores). Каждый вызов set заменяет
предыдущее значение; повтор одинаковых данных не создаёт новые ресурсы.
На выгрузке, ошибке enable или аварии ресурсы удаляются. Disconnect очищает
состояние зрителя. Заголовок, actionbar, имя и display slot используют последнее
установленное значение; выгрузка прежнего владельца не стирает чужое.
Один клиентский слот и membership игрока в team всё равно общие: плагины,
которые одновременно управляют одним слотом/nametag, должны договориться.
Оформление не меняет сохраняемое серверное scoreboard и правила боя.

`tab-tps=true` в `config/mcsa.properties` включает подвал ядра раз в 20 тактов.
TPS ограничен целевой tick rate и считается по 100 последним интервалам
начала тактов; MSPT — по существующим 100 замерам работы такта. TPS: зелёный
от 19.5, жёлтый от 15, иначе красный; MSPT: зелёный до 50 мс, жёлтый до
66.67 мс, иначе красный. Плагин забирает подвал только конкретного игрока.

Пример `plugins-examples/custom-tablist` переносит пользовательский
CustomTabList. Его конфиг — `plugins/custom-tablist/data/config.yml`, при
первом включении копируется исходный config.yml без изменений. Команды
`/tablist reload|placeholders|test|format|worldcolors|toggle` и исходные
`/tablistreload`, `/tablistplaceholders`, `/tablisttest`, `/tablistformat`,
`/tablistworldcolors`, `/tablisttoggle`, `/cprefix`, `/csuffix`,
`/cprefixremove`, `/csuffixremove`, `/cnametag`, `/chat` доступны операторам
уровня 2. Узлов Bukkit permissions в хозяине нет; для обхода фильтров чата
используется уровень оператора. Неудачный reload сохраняет действующую
конфигурацию. Запись через команды сохраняет значения YAML, но не комментарии.
Первое применение после enable/reload/join — ближайший такт. Обновления
делятся на четыре группы игроков и отдельные задачи по видам оформления;
все группы выполняются в одном такте с тем же update-interval. Это сохраняет
лимиты WASM на каждый вызов при стандартных 20 игроках.

Парсер реализован по [публичному описанию MiniMessage](https://docs.advntr.dev/minimessage/format.html),
без исходников Adventure. Поддерживает named/hex colours, gradient с несколькими
цветами и фазой, transition, rainbow, decorations/aliases/negation, reset,
newline, font, insertion, click, hover/show_text, keybind, translate; legacy
`&`/`§`, `&#RRGGBB` и `§x§R§R§G§G§B§B`. Режим auto принимает оба синтаксиса,
legacy — только legacy, minimessage — только теги. Неизвестные теги и
плейсхолдеры сохраняются буквально. Это ограниченный парсер, не полная
реализация Adventure: NBT/selector/score, hover предмета/сущности, sprite,
head, pride, shadow и сложные аргументы translate не реализованы.

Нативные placeholders включают внутренние исходного плагина,
`%customtablist_prefix%`, `%customtablist_suffix%`, `%customtablist_full%`,
health-варианты и новые `%tps%`, `%mspt%`, `%online%`, `%max_players%`.
PlaceholderAPI отсутствует; use-placeholderapi сохраняется для совместимости.
`%time%` и `%date%` используют UTC (WASI не получает timezone сервера).
Как в исходном TabListManager/ActionBarManager, long-names-handling,
max-name-length и display-time не изменяют поведение: имена не обрезаются,
actionbar обновляется и циклически меняется по update-interval. Sidebar
сохраняет цвета и повторяющиеся строки, показывая до 15 непустых строк.
ChatListener исходника использует chat.format; global-format сохраняется,
но отдельного глобального канала в этом порте нет. Сообщения игроков
вставляются литералами, без исполнения тегов оформления.

[проверить] Внешний вид на Java-клиенте и сопоставление с исходным плагином
требуют отдельной проверки; автоматические тесты запускают WASM без сервера.

## Дополнения Bukkit-моста

`player.set-game-mode`, `operator-level` (только чтение) и `set-inventory-slot`
доступны через Rust SDK. Ячейки в WIT — номера окна 9..44, count 1..64,
none очищает ячейку. Java SDK переводит Bukkit 0..8 в 36..44.
`world.time/set-time` работают со временем суток, `weather/set-weather` —
с clear/rain/thunder, срок от 1 до 2400000 тактов. Ошибочные UUID, ячейки,
названия блоков/предметов и сроки отвергаются хозяином.
Rust Plugin.on_tab_complete экспортируется макросом SDK; старые компоненты
без этого экспорта тоже загружаются. `bukkit-plugin` — WIT world переводчика,
`sdk-module` — world Rust SDK, прежние plugin/module сохраняются.
