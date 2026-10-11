# Плагины MCSheriffAnya (фаза 1: WebAssembly)

Плагин — это модуль WebAssembly (компонент), написанный под API `mcsa:api`.
Ядро запускает его встроенным [wasmtime](https://wasmtime.dev): никакой JVM, никакого
JDK на сервере. Плагины грузятся, выгружаются и перезагружаются на ходу, без
перезапуска сервера.

Фаза 1 — это хозяин в ядре, `/plugman`, API (WIT) и примеры на Rust.
Java API, TeaVM и сборщик `.jar → .wasm` (Jar2WASM) — фаза 2, их здесь нет.

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
`component-model`, `std`. У WASI включён только `p2`: Preview 0/1/3 не нужны.
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
| Топливо на вызов | 20 млн (порядка 20 мс) | 400 млн |
| Срок вызова по часам | ~20 мс | ~200 мс |
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

## Соответствие WIT и Java-пакета

WIT-пакет `mcsa:api@0.1.0` (файл `wit/mcsa-api.wit`) — то же, что Java-пакет
`ru.sheriffanya.mcsa.api`; интерфейсы WIT — подпакеты:

| WIT | Java (фаза 2) |
|---|---|
| `mcsa:api/types` | `ru.sheriffanya.mcsa.api` (Player, BlockPos, ItemStack, Location …) |
| `mcsa:api/log` | `ru.sheriffanya.mcsa.api.log` |
| `mcsa:api/server` | `ru.sheriffanya.mcsa.api.server` |
| `mcsa:api/player` | `ru.sheriffanya.mcsa.api.player` |
| `mcsa:api/world` | `ru.sheriffanya.mcsa.api.world` |
| `mcsa:api/scheduler` | `ru.sheriffanya.mcsa.api.scheduler` |
| `mcsa:api/command` | `ru.sheriffanya.mcsa.api.command` |
| `mcsa:api/storage` | `ru.sheriffanya.mcsa.api.storage` |
| `mcsa:api/privileged` | `ru.sheriffanya.mcsa.api.privileged` (только модули) |
| `mcsa:api/lifecycle` | `ru.sheriffanya.mcsa.api.Plugin` (enable/disable) |
| `mcsa:api/events` | `ru.sheriffanya.mcsa.api.event` (обработчики) |

Миры: `plugin` (обычный плагин) и `module` (всё то же и `privileged`).

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
| `storage` | `config(default-text)`, `save-config(text)`, `read/write/delete(name)` в `data/` (без путей, файл до 4 МиБ) |
| `privileged` (модули) | `run-console-command(command)`, `loaded-plugins()` |

Ограничения 0.1, о которых надо знать:
- `player.send-message` пока доходит только до игроков Java (Bedrock — позже);
  ответ команды плагина (`message`) доходит всем.
- Подсказки (`Tab`) для команд плагинов не показываются: дерево команд клиенту
  пока статическое. Команда при этом работает.
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
