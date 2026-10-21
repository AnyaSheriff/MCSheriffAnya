// Выполнение команд.
//
// Команды приходят с двух сторон: от игрока в игре и от человека за консолью
// сервера. Разбор у них общий, а последствия разные: игроку надо сменить режим
// игры и ответить ему в чат, консоли — напечатать ответ и, бывает, остановить
// сервер. Поэтому разбор не делает это сам, а описывает, что должен сделать
// тот, кто команду вызвал.

use crate::blocks;
use crate::ops;
use crate::dimension::Dimension;
use crate::players::Order;
use crate::shared::Shared;

/// Режимы игры — как их нумерует игра.
pub const GAME_MODE_SURVIVAL: i32 = 0;
pub const GAME_MODE_CREATIVE: i32 = 1;
pub const GAME_MODE_ADVENTURE: i32 = 2;
pub const GAME_MODE_SPECTATOR: i32 = 3;

/// Кто ввёл команду.
pub enum Source {
    /// Игрок в игре.
    Player { uuid: [u8; 16], name: String },

    /// Человек за консолью сервера.
    Console,

    /// Командный блок (command_block.rs): место, измерение и имя («@» или
    /// своё). Уровень прав — 2, как у оригинала.
    Block { x: i32, y: i32, z: i32, dimension: Dimension, name: String },
}

/// Команды, которых командному блоку нельзя: уровень прав 3 и 4 (вики,
/// Permission level: у командного блока уровень 2).
const ABOVE_BLOCK_LEVEL: &[&str] = &[
    "ban", "ban-ip", "banlist", "debug", "deop", "jfr", "kick", "op", "pardon", "pardon-ip", "perf", "save-all", "save-off", "save-on",
    "setidletimeout", "stop", "transfer", "whitelist",
];

thread_local! {
    /// Итог последней команды этого потока (SuccessCount командного блока),
    /// если команда его назвала (`note_result`).
    static RESULT: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

/// Команда называет свой итог: 0 — не удалась, иначе — сколько сделано.
pub(crate) fn note_result(result: i32) {
    RESULT.with(|cell| cell.set(Some(result)));
}

/// Забирает итог, названный последней командой.
pub(crate) fn take_result() -> Option<i32> {
    RESULT.with(|cell| cell.take())
}

/// Похож ли ответ на отказ — для команд, что итог не называют: такие ответы
/// у нас начинаются с отрицания или с подсказки, как писать команду.
pub(crate) fn looks_failed(reply: &str) -> bool {
    const FAILED: &[&str] = &[
        "Не ", "Нет ", "Неизвестн", "Нужно", "Нельзя", "На это", "Такого", "Укажи", "Кому", "Кого", "Что ", "Как ", "Сколько", "Это ", "Слишком",
        "Пока понимаю", "Ожидалось", "Там уже", "Остановить сервер можно",
    ];

    FAILED.iter().any(|start| reply.starts_with(start))
}

/// Что тому, кто вызвал команду, остаётся сделать самому.
///
/// Разбор команды живёт отдельно от подключения игрока и от консоли, поэтому
/// всё, что касается их лично, он не делает, а называет.
pub enum Effect {
    /// Ничего, кроме ответа.
    None,

    /// Сменить режим игры игроку.
    GameMode { uuid: [u8; 16], mode: i32 },

    /// Остановить сервер.
    Stop,
}

/// Ответ на команду.
pub struct Answer {
    /// Что сказать. Пустая строка — сказать нечего.
    pub reply: String,

    /// Что сделать тому, кто вызвал команду.
    pub effect: Effect,

    /// Ответ текстом игры (ключ перевода с подстановками, сетевой NBT), как
    /// у оригинала: клиенту Java уходит он, а `reply` — его английская
    /// запись для консоли, журнала и Bedrock (commands_world.rs).
    pub component: Option<Vec<u8>>,

    /// Готовые тексты для клиентов Java (переводы оригинала), по сообщению
    /// на элемент; пусто — уходит `reply` простым текстом.
    pub rich: Vec<Vec<u8>>,

    /// Итог команды для `execute store` и функций: Some(число) — успех,
    /// None — отказ (вики, Commands: «success count» и «result»).
    pub result: Option<i32>,
}

impl Answer {
    /// Ответ, после которого ничего делать не надо.
    pub(crate) fn saying(reply: String) -> Self {
        Self {
            reply,
            effect: Effect::None,
            component: None,
            rich: Vec::new(),
            result: Some(1),
        }
    }
}

/// Разбирает и выполняет команду, введённую без ведущей косой черты.
/// Какой уровень прав нужен команде.
///
/// Ноль — можно всем. Всё, что меняет мир или задевает других игроков,
/// требует прав; их выдают командой `op`, а хранятся они в `ops.json`.
fn level_needed(command: &str) -> i32 {
    // Команды commands_admin.rs: уровни по вики.
    if let Some(level) = crate::commands_admin::level(command) {
        return level;
    }

    match command {
        "help" | "list" | "say" => 0,
        // Вики, Commands: trigger и teammsg доступны всем, прочие команды
        // табло и функций — с уровня 2.
        "trigger" | "teammsg" | "tm" => 0,
        "scoreboard" | "team" | "tag" | "bossbar" | "function" | "schedule" | "return" | "execute" | "datapack" => 2,
        // Права на особый динамит — узлом (permissions.rs), не уровнем.
        "tnt" => 0,
        _ => ops::HIGHEST_LEVEL,
    }
}

/// Хватает ли прав у того, кто вызвал команду. У консоли права полные.
fn allowed(command: &str, source: &Source, shared: &Shared) -> bool {
    let needed = level_needed(command);

    if needed == 0 {
        return true;
    }

    match source {
        Source::Console => true,
        Source::Block { .. } => !ABOVE_BLOCK_LEVEL.contains(&command),
        Source::Player { uuid, .. } => {
            let level = shared
                .ops
                .lock()
                .expect("права захвачены другим потоком")
                .level_of(uuid);

            level >= needed
        }
    }
}

/// Пробелы внутри компонентов предмета и SNBT не разделяют аргументы.
/// Кавычки и экранирование сохраняем: текст книги должен дойти без изменений.
fn command_arguments(command: &str) -> impl Iterator<Item = &str> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    command.split(move |c: char| {
        if let Some(q) = quote {
            if escaped { escaped = false; }
            else if c == '\\' { escaped = true; }
            else if c == q { quote = None; }
            return false;
        }
        match c {
            '"' | '\'' if depth > 0 => quote = Some(c),
            '[' | '{' => depth += 1,
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        depth == 0 && c.is_whitespace()
    }).filter(|part| !part.is_empty())
}

/// Стопка по записи предмета команды (`diamond_sword[damage=5]`): None —
/// воздух.
pub(crate) fn stack_from_spec(spec: &str, count: i32) -> Result<Option<crate::inventory::Stack>, String> {
    stack_from_spec_for_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2), spec, count)
}

/// Каталог задаёт сервер; название из команды не может включить новый профиль.
pub(crate) fn stack_from_spec_for_catalog(catalog: crate::gameplay_catalog::Catalog, spec: &str, count: i32) -> Result<Option<crate::inventory::Stack>, String> {
    let (name, data) = crate::stackdata::parse_item_spec(spec).map_err(|why| format!("Не разобрать предмет {spec}: {why}"))?;

    if name == "air" {
        return Ok(None);
    }

    let number = catalog.item_named(&name).ok().flatten().ok_or_else(|| format!("Нет такого предмета: {}", spec.split('[').next().unwrap_or(spec)))?;

    Ok(Some(crate::inventory::Stack::new(number, count.min(catalog.stack_size(number).map_err(|_| format!("Нет такого предмета: {name}"))?)).with_extras(data)))
}

pub fn run(command: &str, source: &Source, shared: &Shared) -> Answer {
    if let Some(answer) = crate::auth::command(command, source, shared) { return answer; }
    // `/plugman` и команды плагинов (plugins/): раньше всего прочего, чтобы
    // права плагина проверял он сам, а не общая таблица уровней.
    if let Some(answer) = crate::plugins::run_command(command, source, shared) {
        return answer;
    }

    // Команды commands_admin.rs разбирают строку сами и целиком, до сжатия
    // пробелов (в `tellraw` и `particle` в скобках важен каждый знак).
    if let Some(answer) = crate::commands_admin::run(command, source, shared) {
        return answer;
    }

    let mut parts = command_arguments(command);
    let name = parts.next().unwrap_or("");

    if !allowed(name, source, shared) {
        return Answer::saying("На это у тебя нет прав".to_string());
    }

    match name {
        "gamemode" => {
            let Some(mode) = parts.next().and_then(game_mode_by_name) else {
                return Answer::saying(
                    "Укажи режим: creative, survival, adventure или spectator".to_string(),
                );
            };

            // Игрок меняет режим себе, поэтому называть себя ему не нужно.
            // В консоли имя обязательно: иначе непонятно, кому менять.
            let named = parts.next();
            let targets = match (named, source) {
                (Some(named), _) => find_targets(named, source, shared),
                (None, Source::Player { uuid, name }) => vec![(*uuid, name.clone())],
                (None, _) => Vec::new(),
            };

            let Some((uuid, name)) = targets.first().cloned() else {
                return Answer::saying(
                    "Такого игрока нет на сервере. Кто сейчас играет — команда list".to_string(),
                );
            };

            {
                let mut players = shared
                    .players
                    .lock()
                    .expect("список игроков захвачен другим потоком");

                for (uuid, _) in &targets {
                    players.set_game_mode(uuid, mode);
                    players.order(uuid, Order::GameMode { mode });
                }
            }

            let own = matches!(source, Source::Player { uuid: own, .. } if *own == uuid);

            let reply = if targets.len() > 1 {
                format!(
                    "Режим {} установлен игрокам: {}",
                    game_mode_name(mode),
                    targets
                        .iter()
                        .map(|(_, name)| name.as_str())
                        .collect::<Vec<&str>>()
                        .join(", ")
                )
            } else if own {
                format!("Режим игры: {}", game_mode_name(mode))
            } else {
                format!("Игроку {} установлен режим: {}", name, game_mode_name(mode))
            };

            // Своё подключение узнаёт о смене режима отсюда, чужие — из
            // общего списка.
            let effect = match source {
                Source::Player { uuid: own, .. }
                    if targets.iter().any(|(target, _)| target == own) =>
                {
                    Effect::GameMode { uuid: *own, mode }
                }
                _ => Effect::None,
            };

            Answer { reply, effect, component: None, rich: Vec::new(), result: Some(1) }
        }
        "op" => {
            let Some(named) = parts.next() else {
                return Answer::saying("Кому выдать права? op <игрок>".to_string());
            };

            let targets = find_targets(named, source, shared);

            if targets.is_empty() {
                return Answer::saying(
                    "Такого игрока нет на сервере. Права выдаются тем, кто сейчас играет"
                        .to_string(),
                );
            }

            // Всем целям — `op @a` выдаёт права всем, а не первому.
            let mut ops = shared.ops.lock().expect("права захвачены другим потоком");
            let given: Vec<&str> = targets
                .iter()
                .filter(|(uuid, name)| ops.give(uuid, name, ops::HIGHEST_LEVEL))
                .map(|(_, name)| name.as_str())
                .collect();

            if given.is_empty() {
                Answer::saying(format!("Права уже были: {}", names_of(&targets)))
            } else {
                Answer::saying(format!("Теперь есть права: {}", given.join(", ")))
            }
        }
        "deop" => {
            let Some(named) = parts.next() else {
                return Answer::saying("У кого снять права? deop <игрок>".to_string());
            };

            // Сначала — среди тех, кто в сети (по опознавателю: права
            // остаются за ним, даже если ник сменился), потом по нику из файла.
            let targets = find_targets(named, source, shared);
            let mut ops = shared.ops.lock().expect("права захвачены другим потоком");
            let taken = if targets.is_empty() {
                ops.take_away(None, named)
            } else {
                targets.iter().fold(false, |any, (uuid, name)| ops.take_away(Some(uuid), name) | any)
            };

            if taken {
                Answer::saying(format!("У игрока {} больше нет прав", named))
            } else {
                Answer::saying(format!("У игрока {} прав и не было", named))
            }
        }
        "give" => {
            let Some(named) = parts.next() else {
                return Answer::saying("Кому выдать? give <игрок> <предмет> [сколько]".to_string());
            };

            let Some(item_name) = parts.next() else {
                return Answer::saying("Что выдать? give <игрок> <предмет> [сколько]".to_string());
            };

            let count: i32 = parts.next().and_then(|value| value.parse().ok()).unwrap_or(1);

            if count <= 0 {
                return Answer::saying("Сколько выдавать? Число должно быть больше нуля".to_string());
            }

            let stack = match stack_from_spec_for_catalog(shared.catalog(), item_name, count) {
                Ok(Some(stack)) => crate::inventory::Stack { count, ..stack },
                Ok(None) => return Answer::saying(format!("Нет такого предмета: {}", item_name)),
                Err(why) => return Answer::saying(why),
            };

            let targets = find_targets(named, source, shared);

            if targets.is_empty() {
                return Answer::saying("Такого игрока нет на сервере".to_string());
            }

            {
                let mut players = shared
                    .players
                    .lock()
                    .expect("список игроков захвачен другим потоком");

                for (uuid, _) in &targets {
                    players.order(uuid, Order::Give { stack });
                }
            }

            crate::commands_feedback::give(shared.catalog(), stack, &targets)
        }
        "clear" => {
            let named = parts.next();

            let targets = match (named, source) {
                (Some(named), _) => find_targets(named, source, shared),
                (None, Source::Player { uuid, name }) => vec![(*uuid, name.clone())],
                (None, _) => Vec::new(),
            };

            if targets.is_empty() {
                return Answer::saying("У кого чистить? Из консоли имя игрока обязательно".to_string());
            }

            {
                let mut players = shared
                    .players
                    .lock()
                    .expect("список игроков захвачен другим потоком");

                for (uuid, _) in &targets {
                    players.order(uuid, Order::Clear);
                }
            }

            Answer::saying(format!("Инвентарь очищен: {}", names_of(&targets)))
        }
        "time" => time(parts.collect::<Vec<&str>>().as_slice(), shared),
        // Данные блока (`{Command:"say a b"}`) бывают с пробелами — слова
        // делятся с учётом скобок и кавычек.
        "setblock" | "fill" => {
            let words = words_of(command);
            let words: Vec<&str> = words.iter().skip(1).map(String::as_str).collect();

            set_blocks(&words, source, shared, dimension_of(source, shared), name == "fill")
        }
        "item" => {
            let parts: Vec<&str> = parts.collect();

            // Основную руку разбирает `item` (сценарии comparator-click),
            // прочие ячейки игрока — `item_entity` (сценарий shelf).
            match parts.as_slice() {
                ["replace", "entity", _, slot, ..] if *slot != "weapon.mainhand" => item_entity(&parts, source, shared),
                _ => item(&parts, source, shared, dimension_of(source, shared)),
            }
        }
        "list" => {
            let names = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .names();

            // Как у обычного сервера: «There are N of a max of M players
            // online: имена».
            let most = shared.properties.max_players;

            if names.is_empty() {
                return Answer::saying(format!("Игроков на сервере: 0 из {}", most));
            }

            Answer::saying(format!(
                "Игроков на сервере: {} из {}: {}",
                names.len(),
                most,
                names.join(", ")
            ))
        }
        "say" => {
            // Текст берётся из исходной строки целиком: в нём могут быть
            // лишние пробелы, а split_whitespace их бы съел.
            let text = command
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.trim())
                .unwrap_or("");

            if text.is_empty() {
                return Answer::saying("Что сказать? Напиши текст после команды".to_string());
            }

            if let Source::Block { name, .. } = source {
                crate::commands_feedback::block_say(shared, name, text);
                return Answer::saying(String::new());
            }

            // Сообщение уходит в общую ленту, поэтому в ответе его повторять
            // не надо: сказавший увидит его там же, где все остальные.
            let who = match source {
                Source::Player { name, .. } | Source::Block { name, .. } => name.clone(),
                Source::Console => "Server".to_string(),
            };

            shared
                .chat
                .lock()
                .expect("чат захвачен другим потоком")
                .push(format!("[{}] {}", who, text));

            Answer::saying(String::new())
        }
        "kick" => {
            let Some(named) = parts.next() else {
                return Answer::saying("Кого выгнать? kick <игрок> [причина]".to_string());
            };

            let targets = find_targets(named, source, shared);

            if targets.is_empty() {
                return Answer::saying(
                    "Такого игрока нет на сервере. Кто сейчас играет — команда list".to_string(),
                );
            }

            // Причина — весь остаток строки: в ней бывают пробелы.
            let reason = parts.collect::<Vec<&str>>().join(" ");
            let reason = if reason.is_empty() {
                "Выгнан с сервера".to_string()
            } else {
                reason
            };

            {
                let mut players = shared
                    .players
                    .lock()
                    .expect("список игроков захвачен другим потоком");

                for (uuid, _) in &targets {
                    players.order(uuid, Order::Kick { reason: reason.clone() });
                }
            }

            let who = targets
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<&str>>()
                .join(", ");

            Answer::saying(format!("{} выгнан: {}", who, reason))
        }
        "tp" | "teleport" => {
            let rest: Vec<&str> = parts.collect();

            // Вагонетки и мобы обрабатываются отдельно от игроков.
            match rest.as_slice() {
                [who, place @ ..] if cart_selector(who).is_some() => teleport_carts(who, place, source, shared),
                [who, place @ ..] if who.starts_with("@e[") && matches!(place.len(), 3 | 5) && !who.contains("type=!player") => teleport_mobs(who, place, source, shared),
                _ => teleport(&rest, source, shared, None),
            }
        }
        "summon" => summon(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "data" => data_command(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "tnt" => custom_tnt(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "kill" => kill(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "damage" => damage(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "difficulty" => difficulty(parts.collect::<Vec<&str>>().as_slice(), shared),
        "effect" => effect(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "advancement" => crate::commands_progress::advancement(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "experience" | "xp" => experience(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "recipe" => recipe(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "gamerule" => gamerule(parts.collect::<Vec<&str>>().as_slice(), shared),
        "scoreboard" | "team" | "tag" | "bossbar" | "trigger" | "teammsg" | "tm" | "function" | "schedule" | "datapack" | "return" => crate::commands_score::run(command, source, shared),

        // Команды мира и сущностей (commands_world.rs).
        "worldborder" | "tick" | "clone" | "attribute" | "enchant" | "locate" | "forceload" | "spreadplayers" | "random" | "ride"
        | "rotate" | "spectate" | "fillbiome" | "place" | "loot" => {
            crate::commands_world::run(name, parts.collect::<Vec<&str>>().as_slice(), source, shared)
        }
        "weather" => weather(parts.collect::<Vec<&str>>().as_slice(), shared),

        // Как у оригинала: `execute in <измерение> run tp …` — перенос в
        // другое измерение. Другие виды execute пока не разбираются.
        "execute" => {
            let rest: Vec<&str> = parts.collect();

            match rest.as_slice() {
                ["in", dimension, "run", "tp" | "teleport", tail @ ..] => match Dimension::from_name(dimension) {
                    Some(dimension) => teleport(tail, source, shared, Some(dimension)),
                    None => Answer::saying(format!("Нет такого измерения: {}", dimension)),
                },
                // throw3-egg-hatch-repeat.md: после cleanup наши цыплята
                // жили дальше: positioned не разбирался. Меняем только центр
                // селектора kill, сохраняя исполнителя и его измерение.
                ["positioned", x, y, z, "run", "kill", tail @ ..] => {
                    match entity_place(&[*x, *y, *z], origin_of(source, shared)) {
                        Some(origin) => kill_at(tail, source, shared, origin),
                        None => Answer::saying("Неверное место positioned".into()),
                    }
                }
                // Остальное (as, if/unless, store, run) — commands_score.rs.
                _ => crate::commands_score::run(command, source, shared),
            }
        }
        "help" => {
            // Показываем только то, что доступно вызвавшему: перечислять
            // недоступное — только путать.
            let mut names = vec!["help", "list", "say <текст>"];

            if allowed("gamemode", source, shared) {
                names.extend([
                    "gamemode <режим> [игрок]",
                    "tp <куда>",
                    "kick <игрок> [причина]",
                    "give <игрок> <предмет> [сколько]",
                    "clear [игрок]",
                    "time set|add|query",
                    "setblock <x> <y> <z> <блок>",
                    "fill <x1> <y1> <z1> <x2> <y2> <z2> <блок>",
                    "op <игрок>",
                    "deop <игрок>",
                ]);
            }

            if crate::permissions::allowed_for(shared, source, crate::custom_tnt::GIVE_NODE) {
                names.extend(["tnt give <игрок> <вид> [сколько]", "tnt list"]);
            }

            if matches!(source, Source::Console) {
                names.push("stop");
            }

            Answer::saying(format!("Команды: {}", names.join(", ")))
        }
        "stop" => match source {
            Source::Console => Answer {
                reply: "Останавливаю сервер".to_string(),
                effect: Effect::Stop,
                component: None,
                rich: Vec::new(),
                result: Some(1),
            },
            Source::Player { .. } | Source::Block { .. } => Answer::saying(
                "Остановить сервер можно только из консоли сервера".to_string(),
            ),
        },
        _ => crate::commands_admin::unknown(command, source, shared),
    }
}

/// `tnt give <игрок> <вид> [сколько]` и `tnt list` — особый динамит
/// (custom_tnt.rs). Только тем, кому позволено (узел `GIVE_NODE`).
fn custom_tnt(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    use crate::custom_tnt::{self, KINDS};

    if !crate::permissions::allowed_for(shared, source, custom_tnt::GIVE_NODE) {
        return Answer::saying("На это у тебя нет прав".to_string());
    }

    let kinds = custom_tnt::names().collect::<Vec<&str>>().join(", ");

    match parts {
        ["list", ..] => {
            let lines: Vec<String> = KINDS
                .iter()
                .map(|kind| format!("{} — {} (сила {}, запал {})", kind.name, kind.title, kind.power, kind.fuse))
                .collect();

            Answer::saying(format!("Особый динамит: {}", lines.join("; ")))
        }
        ["give"] => Answer::saying("Кому выдать? tnt give <игрок> <вид> [сколько]".to_string()),
        ["give", _] => Answer::saying(format!("Какой? tnt give <игрок> <вид> [сколько]. Виды: {}", kinds)),
        ["give", named, kind, rest @ ..] => {
            let Some(kind) = custom_tnt::by_name(kind) else {
                return Answer::saying(format!("Нет такого вида: {}. Виды: {}", kind, kinds));
            };

            let count = match rest.first() {
                None => 1,
                Some(value) => match value.parse::<i32>() {
                    Ok(count) if count > 0 => count,
                    _ => return Answer::saying("Сколько выдавать? Число должно быть больше нуля".to_string()),
                },
            };

            let Some(stack) = custom_tnt::stack(kind, count) else {
                return Answer::saying("Динамита нет в таблице предметов".to_string());
            };

            let targets = find_targets(named, source, shared);

            if targets.is_empty() {
                return Answer::saying("Такого игрока нет на сервере".to_string());
            }

            {
                let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

                for (uuid, _) in &targets {
                    players.order(uuid, Order::Give { stack });
                }
            }

            Answer::saying(format!("Выдано: {} × {} — игрокам: {}", kind.title, count, names_of(&targets)))
        }
        _ => Answer::saying("Как выдать? tnt give <игрок> <вид> [сколько] или tnt list".to_string()),
    }
}

/// Место, откуда отсчитываются относительные координаты (`~`): у игрока —
/// где он стоит, у консоли — начало мира.
pub(crate) fn origin_of(source: &Source, shared: &Shared) -> (f64, f64, f64) {
    match source {
        Source::Player { uuid, .. } => shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .members()
            .iter()
            .find(|member| &member.uuid == uuid)
            .map(|member| (member.position.x, member.position.y, member.position.z))
            .unwrap_or_default(),
        Source::Console => (0.0, 0.0, 0.0),
        // У командного блока — середина блока (`Vec3.atCenterOf`).
        Source::Block { x, y, z, .. } => (*x as f64 + 0.5, *y as f64 + 0.5, *z as f64 + 0.5),
    }
}

/// Три координаты места для сущности: числа или `~` со сдвигом от `origin`.
/// Как у оригинала, целое число по x и z означает середину блока (вики,
/// Argument types, vec3: «an integer … is centered»). [проверить]: у
/// оригинала целые по x и z сдвигаются на 0,5.
pub(crate) fn entity_place(parts: &[&str], origin: (f64, f64, f64)) -> Option<(f64, f64, f64)> {
    let [x, y, z] = parts else {
        return None;
    };

    let one = |text: &str, from: f64, centre: bool| -> Option<f64> {
        let value = match text.strip_prefix('~') {
            Some("") => from,
            Some(shift) => from + shift.parse::<f64>().ok()?,
            None if centre && !text.contains('.') => text.parse::<i64>().ok()? as f64 + 0.5,
            None => text.parse::<f64>().ok()?,
        };

        (value.is_finite() && value.abs() <= 30_000_000.0).then_some(value)
    };

    Some((one(x, origin.0, true)?, one(y, origin.1, false)?, one(z, origin.2, true)?))
}

/// Выбор вагонеток: `@e[type=minecart]`, `@e[type=chest_minecart,limit=1]`,
/// `@n[type=…]`. Возвращает вид и сколько взять (None — всех).
fn cart_selector(text: &str) -> Option<(crate::minecart::Kind, Option<usize>)> {
    let inner = text.strip_prefix("@e[").or_else(|| text.strip_prefix("@n["))?.strip_suffix(']')?;
    let mut kind = None;
    let mut limit = text.starts_with("@n").then_some(1);

    for pair in inner.split(',') {
        match pair.split_once('=')? {
            ("type", name) => kind = Some(crate::minecart::Kind::from_name(name)?),
            ("limit", count) => limit = Some(count.parse().ok()?),
            _ => return None,
        }
    }

    Some((kind?, limit))
}

/// Номера вагонеток по выбору — в порядке появления.
fn chosen_carts(items: &crate::items::Items, (kind, limit): (crate::minecart::Kind, Option<usize>)) -> Vec<i32> {
    items
        .carts()
        .iter()
        .filter(|cart| cart.kind == kind)
        .take(limit.unwrap_or(usize::MAX))
        .map(|cart| cart.entity_id)
        .collect()
}

/// `summon <вагонетка> <x> <y> <z> [{Items:[…]}]` — у нас из сущностей
/// призываются вагонетки, предметы и молния. Она появляется ровно в названной точке,
/// без скорости; к рельсе её прижмёт её же такт.
fn summon(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: summon <вагонетка> <x> <y> <z>".to_string());

    let (name, rest) = match parts {
        [name, rest @ ..] => (*name, rest),
        [] => return usage(),
    };

    let origin = origin_of(source, shared);

    if name.strip_prefix("minecraft:").unwrap_or(name) == "ominous_item_spawner" {
        let [x,y,z,data @ ..] = rest else { return usage(); };
        let Some(at) = entity_place(&[*x,*y,*z], origin) else { return usage(); };
        let data = data.join(" ");
        let Some(stack) = item_from_text(&data) else { return usage(); };
        let delay = crate::snbt::parse(&data).and_then(|v| v.get("spawn_item_after_ticks").and_then(crate::snbt::Value::as_f64)).unwrap_or(60.0).max(0.0) as u64;
        let now = *shared.ticks.borrow();
        // Список игроков берём до замка снарядов: такт держит их в обратном порядке.
        let (id, dimension) = (shared.next_entity_id(), dimension_of(source, shared));
        shared.thrown.lock().expect("снаряды").ominous.push(crate::ominous::Spawner {
            id,dimension,at,stack,due:now.saturating_add(delay.saturating_sub(1)),
        });
        return Answer::saying("Призвана сущность ominous_item_spawner".into());
    }

    if name.strip_prefix("minecraft:").unwrap_or(name) == "item" {
        return summon_item(rest, source, shared);
    }

    if name.strip_prefix("minecraft:").unwrap_or(name) == "experience_orb" {
        return summon_orb(rest, source, shared);
    }

    if name.strip_prefix("minecraft:").unwrap_or(name) == "dragon_fireball" {
        return summon_fireball(rest, source, shared);
    }

    if let base @ ("wither_skull" | "arrow" | "spectral_arrow" | "trident" | "snowball" | "egg" | "ender_pearl" | "experience_bottle" | "potion" | "splash_potion" | "lingering_potion" | "small_fireball" | "fireball" | "llama_spit") = name.strip_prefix("minecraft:").unwrap_or(name) {
        return summon_projectile(base, rest, source, shared);
    }

    // Заряды ветра: летят по прямой со скоростью из `Motion` (wind.rs;
    // чёрный ящик, breeze-charge-flight).
    if let charge @ ("wind_charge" | "breeze_wind_charge") = name.strip_prefix("minecraft:").unwrap_or(name) {
        return summon_wind_charge(charge == "breeze_wind_charge", rest, source, shared);
    }

    if name.strip_prefix("minecraft:").unwrap_or(name) == "lightning_bolt" {
        return summon_lightning(rest, source, shared);
    }

    if let glow @ ("item_frame" | "glow_item_frame" | "painting") = name.strip_prefix("minecraft:").unwrap_or(name) {
        return summon_frame(glow == "glow_item_frame", glow == "painting", rest, source, shared);
    }

    if let Some(kind) = crate::mob::Kind::from_name(name) {
        return summon_mob(kind, rest, source, shared);
    }

    let Some(kind) = crate::minecart::Kind::from_name(name) else {
        return Answer::saying(format!("Пока призываются только вагонетки, предметы, рамки, молния, животные (cow, pig, sheep, chicken), враждебные (zombie, skeleton, creeper, spider), кристалл и дракон Края, а не {}", name));
    };
    let (place, data) = match rest {
        [] => (Some(origin), None),
        [x, y, z] => (entity_place(&[*x, *y, *z], origin), None),
        [x, y, z, data] => (entity_place(&[*x, *y, *z], origin), Some(*data)),
        _ => return usage(),
    };

    let Some(place) = place else {
        return usage();
    };

    // Данные с одним `Motion` — не содержимое (boat-slide).
    let only_motion = data.is_some_and(|text| text.contains("Motion:") && !text.contains("Items"));
    let items = match data.filter(|_| !only_motion).map(crate::containers::items_from_text) {
        Some(Some(items)) => items,
        Some(None) => return Answer::saying(format!("Не понял данные сущности: {}", data.unwrap_or(""))),
        None => Vec::new(),
    };

    let mut cart = crate::minecart::Minecart::new(shared.next_entity_id(), kind, place);

    // `{Motion:[x,y,z]}`: начальная скорость (чёрный ящик, boat-slide).
    if let Some((vx, vy, vz)) = data.and_then(motion_of) {
        (cart.vx, cart.vy, cart.vz) = (vx, vy, vz);
    }

    for (slot, stack) in items {
        if let Some(cell) = cart.slots.get_mut(slot) {
            *cell = Some(stack);
        }
    }

    shared
        .items_in(dimension_of(source, shared))
        .lock()
        .expect("предметы захвачены другим потоком")
        .add_cart(cart);

    Answer::saying(format!("Призвана сущность {}", kind.name()))
}

/// `summon dragon_fireball <x> <y> <z> {Motion:[x,y,z]}` — огненный шар
/// дракона без хозяина; разгоняется по направлению `Motion` (чёрный ящик,
/// dragon-fireball).
fn summon_fireball(rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = origin_of(source, shared);
    let (place, data) = match rest {
        [] => (Some(origin), ""),
        [x, y, z] => (entity_place(&[*x, *y, *z], origin), ""),
        [x, y, z, data @ ..] => (entity_place(&[*x, *y, *z], origin), *data.first().unwrap_or(&"")),
        _ => (None, ""),
    };
    let Some(place) = place else {
        return Answer::saying("Нужно: summon dragon_fireball <x> <y> <z> [{Motion:[x,y,z]}]".to_string());
    };
    let motion = motion_of(data).unwrap_or((0.0, 0.0, 0.0));
    let dimension = dimension_of(source, shared);
    let fireball = crate::thrown::Flying::fireball(shared.next_entity_id(), dimension, place, motion, motion, 0);

    shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(fireball);

    Answer::saying("Призвана сущность dragon_fireball".to_string())
}

/// `summon wither_skull|arrow <x> <y> <z> {Motion:[x,y,z],dangerous:1b}` —
/// череп иссушителя (разгон вдоль Motion, синий с `dangerous`) или стрела без
/// стрелка (чёрный ящик, wither-skull-hit, wither-half-armor).
fn summon_projectile(kind: &str, rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = origin_of(source, shared);
    let (place, _) = match rest {
        [] => (Some(origin), ""),
        [x, y, z] => (entity_place(&[*x, *y, *z], origin), ""),
        [x, y, z, data @ ..] => (entity_place(&[*x, *y, *z], origin), *data.first().unwrap_or(&"")),
        _ => (None, ""),
    };
    let Some(place) = place else {
        return Answer::saying(format!("Нужно: summon {} <x> <y> <z> [{{Motion:[x,y,z]}}]", kind));
    };
    let data = rest.get(3..).unwrap_or_default().join(" ");
    let data = data.as_str();
    let motion = motion_of(data).unwrap_or((0.0, 0.0, 0.0));
    let dimension = dimension_of(source, shared);

    if !matches!(kind, "arrow" | "spectral_arrow" | "trident" | "wither_skull") {
        use crate::thrown::{Flying, Kind};
        use crate::inventory::Stack;
        let id = shared.next_entity_id();
        let mut flying = match kind {
            "small_fireball" | "fireball" => Flying::mob_fireball(id, dimension, if kind == "fireball" { Kind::GhastFireball } else { Kind::SmallFireball }, place, motion, 0, 1),
            "llama_spit" => Flying::spit(id, dimension, place, motion, 0),
            _ => {
                let (flying_kind, item) = match kind {
                    "snowball" => (Kind::Snowball, "snowball"), "egg" => (Kind::Egg, "egg"),
                    "ender_pearl" => (Kind::EnderPearl, "ender_pearl"), "experience_bottle" => (Kind::ExperienceBottle, "experience_bottle"),
                    "lingering_potion" => (Kind::Lingering, "lingering_potion"), _ => (Kind::Splash, "splash_potion"),
                };
                Flying::potion(id, dimension, flying_kind, item_from_text(data).unwrap_or_else(|| Stack::new(blocks::item_named(item).unwrap(), 1)), place, motion, 0)
            }
        };
        flying.no_gravity = no_gravity(data);
        flying.in_owner = false;
        if flying.is_fireball() {
            flying.vx = motion.0; flying.vy = motion.1; flying.vz = motion.2;
            flying.start = (place, motion); flying.prev = (place, motion, flying.rot);
        }
        shared.thrown.lock().expect("снаряды").flying.push(flying);
        return Answer::saying(format!("Призвана сущность {}", kind));
    }

    if kind == "wither_skull" {
        // Летит в том же такте (wither-skull-hit: взрыв на +8, синий — на +12
        // в трёх записях оригинала из четырёх; в одной — на такт позже).
        let skull = crate::thrown::Flying::wither_skull(shared.next_entity_id(), dimension, place, motion, motion, 0, flag_in(data, "dangerous:").unwrap_or(false));

        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(skull);
    } else {
        let mut arrow = crate::arrow::Arrow::from_mob(shared.next_entity_id(), dimension, place, motion, 0, "");

        // Призванная командой летит в том же такте (wither-half-armor: удар
        // через такт после призыва), стрелка у неё нет.
        arrow.shooter = crate::arrow::Shooter::Player;
        arrow.fresh = false;
        arrow.summoned = true;
        arrow.no_gravity = no_gravity(data);
        if kind == "spectral_arrow" { arrow.ammo = blocks::item_named(kind).map(|id| crate::inventory::Stack::new(id, 1)); }
        if kind == "trident" { arrow.trident = blocks::item_named(kind).map(|id| crate::inventory::Stack::new(id, 1)); }
        // Призванная смотрит туда, куда велит поворот (0/0), а не по Motion,
        // и доворачивается к полёту на пятую часть за такт (wither-half-armor:
        // наклон −1 из 256 к удару, а не −2).
        arrow.rot = Some((0.0, 0.0));
        shared.arrows.lock().expect("стрелы захвачены другим потоком").spawn(arrow);
    }

    Answer::saying(format!("Призвана сущность {}", kind))
}

fn summon_wind_charge(breeze: bool, rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = origin_of(source, shared);
    let (place, data) = match rest {
        [] => (Some(origin), ""),
        [x, y, z] => (entity_place(&[*x, *y, *z], origin), ""),
        [x, y, z, data @ ..] => (entity_place(&[*x, *y, *z], origin), *data.first().unwrap_or(&"")),
        _ => (None, ""),
    };
    let Some(place) = place else {
        return Answer::saying("Нужно: summon wind_charge <x> <y> <z> [{Motion:[x,y,z]}]".to_string());
    };
    let motion = motion_of(data).unwrap_or((0.0, 0.0, 0.0));
    let dimension = dimension_of(source, shared);
    let mut charge = crate::thrown::Flying::wind_charge(shared.next_entity_id(), dimension, place, motion, 0, breeze);

    // Призванный ни в ком не сидит. Первый ход — в такте призыва: у
    // оригинала взрыв то на +13, то на +14 (breeze-charge-flight, три прогона:
    // +13, +14, +13).
    charge.in_owner = false;
    shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(charge);

    Answer::saying(format!("Призвана сущность {}", if breeze { "breeze_wind_charge" } else { "wind_charge" }))
}

/// `Motion:[a,b,c]` из данных сущности (числа с `d` на конце или без).
fn motion_of(data: &str) -> Option<(f64, f64, f64)> {
    let start = data.find("Motion:[")? + "Motion:[".len();
    let end = start + data[start..].find(']')?;
    let numbers: Vec<f64> = data[start..end].split(',').filter_map(|part| part.trim().trim_end_matches(['d', 'D']).parse().ok()).collect();

    match numbers[..] {
        [x, y, z] => Some((x, y, z)),
        _ => None,
    }
}

/// `summon item <x> <y> <z> {Item:{id:"…",count:N}}` — предмет ровно
/// в названной точке, без скорости и без задержки подбора: так у оригинала
/// призванный предмет не разлетается, как выпавший из блока (чёрный ящик,
/// piston-push-items — он лежит там, где призван).
fn summon_item(rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: summon item <x> <y> <z> {Item:{id:\"…\",count:N}}".to_string());

    let [x, y, z, data] = rest else {
        return usage();
    };

    let Some(place) = entity_place(&[*x, *y, *z], origin_of(source, shared)) else {
        return usage();
    };

    let Some(stack) = item_from_text(data) else {
        return Answer::saying(format!("Не понял данные предмета: {}", data));
    };

    shared
        .items_in(dimension_of(source, shared))
        .lock()
        .expect("предметы захвачены другим потоком")
        .summon_item(shared.next_entity_id(), stack, place, no_gravity(data));

    Answer::saying("Призвана сущность item".to_string())
}

/// `summon experience_orb <x> <y> <z> {Value:N}` — шарик опыта без толчка;
/// общий, как у оригинала: его видят и подбирают все рядом (xp.rs).
fn summon_orb(rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let (place, data) = match rest {
        [x, y, z] => (entity_place(&[*x, *y, *z], origin_of(source, shared)), ""),
        [x, y, z, data] => (entity_place(&[*x, *y, *z], origin_of(source, shared)), *data),
        _ => return Answer::saying("Нужно: summon experience_orb <x> <y> <z> {Value:N}".to_string()),
    };
    let Some(at) = place else {
        return Answer::saying("Нужно: summon experience_orb <x> <y> <z> {Value:N}".to_string());
    };
    let value = data
        .split_once("Value:")
        .map(|(_, tail)| tail.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect::<String>())
        .and_then(|digits| digits.parse::<i32>().ok())
        .unwrap_or(1)
        .max(0);

    shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком").add_orb(shared.next_entity_id(), value, at, (0.0, 0.0, 0.0));

    Answer::saying("Призвана сущность experience_orb".to_string())
}

/// `summon item_frame <x> <y> <z> {Facing:4b,Item:{id:"…",count:1},ItemRotation:2b}`
/// — рамка в клетке точки. Стороны: 0 — низ, 1 — верх, 2 — север, 3 — юг,
/// 4 — запад, 5 — восток (вики, Item Frame, «Facing»); по умолчанию юг.
/// Как у оригинала, призванная рамка не сообщает сравнителям о себе:
/// им нужен толчок соседа.
fn summon_frame(glow: bool, painting: bool, rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: summon item_frame <x> <y> <z> {Facing:4b,Item:{id:\"…\",count:1},ItemRotation:2b}".to_string());

    let (place, data) = match rest {
        [x, y, z] => (entity_place(&[*x, *y, *z], origin_of(source, shared)), ""),
        [x, y, z, data] => (entity_place(&[*x, *y, *z], origin_of(source, shared)), *data),
        _ => return usage(),
    };

    let Some((x, y, z)) = place else {
        return usage();
    };

    let number_after = |key: &str| -> Option<u8> {
        let tail = &data[data.find(key)? + key.len()..];
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();

        digits.parse().ok()
    };

    let dimension = dimension_of(source, shared);
    let mut frame = crate::frame::Frame::new(
        shared.next_entity_id(),
        (x.floor() as i32, y.floor() as i32, z.floor() as i32),
        number_after("Facing:").unwrap_or(crate::frame::DEFAULT_FACING),
        glow,
    );

    if painting {
        let value=crate::snbt::parse(data);
        let name=value.as_ref().and_then(|v|v.get("variant")).and_then(crate::snbt::Value::as_str).unwrap_or("kebab");
        let name=name.strip_prefix("minecraft:").unwrap_or(name);
        let Some(id)=crate::painting::VARIANTS.iter().position(|v|v.0==name) else {return Answer::saying(format!("Неизвестная картина {name}"));};
        frame.painting=Some(id as u8);
        if frame.facing<2 {frame.facing=crate::frame::DEFAULT_FACING;}
    } else {frame.item = item_from_text(data).map(|stack| stack.with_count(1));}
    frame.rotation = number_after("ItemRotation:").unwrap_or(0) & 7;

    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    world.store_frame(&frame);
    items.add_frame(frame);
    crate::frame::refresh_marks(&mut world, &items);

    Answer::saying(format!("Призвана сущность {}", if painting {"painting"} else if glow { "glow_item_frame" } else { "item_frame" }))
}

/// Число после ключа в данных призыва: `Age:-24000`, `Color:3b`.
fn int_after(data: &str, key: &str) -> Option<i32> {
    let tail = &data[data.find(key)? + key.len()..];
    let digits: String = tail.chars().enumerate().take_while(|(index, ch)| ch.is_ascii_digit() || (*index == 0 && *ch == '-')).map(|(_, ch)| ch).collect();

    digits.parse().ok()
}

/// Признак в данных: `NoAI:1b`, `NoAI:true`, `NoAI:1`.
fn flag_in(data: &str, key: &str) -> Option<bool> {
    let tail = &data[data.find(key)? + key.len()..];

    Some(tail.starts_with("1b") || tail.starts_with("true") || tail.starts_with('1'))
}

/// Правит моба по данным: `NoAI`, `Silent`, `Age`, `Sheared`, `Color`,
/// `Health`, `Rotation:[yaw,pitch]`.
fn apply_mob_data(mob: &mut crate::mob::Mob, data: &str) {
    crate::mob::armor_stand::apply(mob,data);
    crate::mob::village_golems::apply_data(mob,data);
    crate::mob::land::apply_data(mob, data);
    crate::mob::water::apply_data(mob,data);
    if crate::mob::horse::equine(mob.kind) && let Some(value) = crate::snbt::parse(data) {
        crate::mob::horse::apply_data(mob, &value);
    }
    crate::mob::wildlife::apply_data(mob, data);
    if let Some(on) = flag_in(data, "AgeLocked:") && crate::mob::growth::eligible(mob.kind) {
        mob.age_locked = on;
        if on { mob.persistent_tag = true; }
    }
    if let Some(on) = flag_in(data, "NoAI:") {
        mob.no_ai = on;
    }
    if let Some(on) = flag_in(data, "OnGround:") { mob.on_ground = on; }

    // Метки сущности: `Tags:["a","b"]` (вики, Entity format).
    if let Some(rest) = data.split("Tags:[").nth(1)
        && let Some(list) = rest.split(']').next()
    {
        mob.tags = list.split('"').skip(1).step_by(2).map(str::to_string).collect();
    }

    // Имя (`CustomName:"…"`, простой текст) и «не исчезать» (вики, Entity format).
    if let Some(rest) = data.split("CustomName:\"").nth(1)
        && let Some(name) = rest.split('"').next()
    {
        mob.name = Some(name.to_string());
    }

    if let Some(on) = flag_in(data, "PersistenceRequired:") {
        mob.persistent_tag = on;
    }

    // Волк и кошка: хозяин, сидение, ошейник, вариант (mob/tame.rs).
    if matches!(mob.kind, crate::mob::Kind::Wolf | crate::mob::Kind::Cat | crate::mob::Kind::Parrot) {
        crate::mob::tame::apply_data(mob, data);
    }

    if mob.kind == crate::mob::Kind::Warden
        && let Some(value) = crate::snbt::parse(data)
        && let Some(memory) = value.get("Brain").and_then(|brain| brain.get("memories")).and_then(|memories| memories.get("minecraft:is_emerging"))
    {
        mob.warden.pending_emerge = Some(memory.get("ttl").and_then(|ttl| ttl.as_f64()).map_or(crate::mob::warden::EMERGE, |ttl| ttl as i32));
    }

    if mob.kind == crate::mob::Kind::Warden
        && let Some(value) = crate::snbt::parse(data)
        && let Some(memory) = value.get("Brain").and_then(|brain| brain.get("memories")).and_then(|memories| memories.get("minecraft:dig_cooldown"))
    {
        mob.warden.calm = memory.get("ttl").and_then(|ttl| ttl.as_f64()).map_or(crate::mob::warden::CALM, |ttl| ttl as i32);
    }

    crate::mob::small::apply_data(mob, data);

    // Големы: «построен игроком», тыква снежного (mob/golem.rs).
    if let Some(on) = flag_in(data, "PlayerCreated:") {
        mob.golem.player_made = on;
    }

    if let Some(on) = flag_in(data, "Pumpkin:") {
        mob.golem.pumpkin = on;
    }

    if let Some(on) = flag_in(data, "Silent:") {
        mob.silent = on;
    }

    if let Some(on) = flag_in(data, "Sheared:").or_else(|| flag_in(data, "sheared:")) {
        mob.sheared = on;
    }

    // Магмовый куб: `Size:2` — размер 3 (вики, Magma Cube: размер = Size + 1;
    // чёрный ящик, magma-cube-stand: свойство 16=3, здоровье 9, броня 9).
    if crate::mob::slime::cube(mob.kind)
        && let Some(size) = crate::snbt::parse(data).and_then(|value| value.get("Size").and_then(|size| size.as_f64()).map(|size| size as i32))
    {
        crate::mob::nether::set_size(mob, size.clamp(0, 126) + 1);
    }

    if mob.kind == crate::mob::Kind::Slime
        && let Some(value) = crate::snbt::parse(data)
    {
        if let Some(on) = value.get("PersistenceRequired").and_then(|flag| flag.as_f64()) {
            mob.nether.persistence_required = on != 0.0;
        }
        if value.get("CustomName").is_some() {
            mob.nether.persistence_required = true;
        }
        if let Some(on) = value.get("NoAI").and_then(|flag| flag.as_f64()) {
            mob.no_ai = on != 0.0;
        }
        if let Some(on) = value.get("Silent").and_then(|flag| flag.as_f64()) {
            mob.silent = on != 0.0;
        }
    }

    if let Some(age) = int_after(data, "Lifetime:") { mob.newmobs.lifetime = age.max(0); }
    if let Some(ticks) = int_after(data, "TimeInOverworld:") { mob.newmobs.outside = ticks.max(0); }
    if let Some(stage) = int_after(data, "Oxidation:") { mob.newmobs.oxidation = stage.clamp(0,3) as u8; }
    if let Some(waxed) = flag_in(data, "Waxed:") { mob.newmobs.waxed = waxed; }

    if let Some(value)=data.split("weather_state:").nth(1) {
        let value=value.trim_start().trim_start_matches(['\"','\'']);
        for (i,name) in ["unaffected","exposed","weathered","oxidized"].iter().enumerate() {
            if value.starts_with(name) {mob.newmobs.oxidation=i as u8;}
        }
    }
    if let Some(age)=int_after(data,"next_weather_age:") { mob.newmobs.waxed=age == -2; mob.newmobs.oxidize_at=i64::from(age.max(0)); }

    // Пиглины и хоглины: `IsImmuneToZombification:1b` (чёрный ящик, piglin-stand).
    if let Some(on) = flag_in(data, "IsImmuneToZombification:") {
        mob.nether.immune = on;
    }

    // Детёныш-зомби: `IsBaby:1b`, остаётся детёнышем навсегда.
    if mob.kind.hostile() && flag_in(data, "IsBaby:") == Some(true) {
        mob.age = -24000;
    }

    if let Some(age) = int_after(data, "Age:") {
        mob.age = age;
    }

    if let Some(color) = int_after(data, "Color:") {
        mob.color = color.clamp(0, 15) as u8;

        // Шалкер: цвет панциря, 16 — без цвета (вики, Shulker, «Entity data»).
        if mob.kind == crate::mob::Kind::Shulker {
            mob.h.shell_color = color.clamp(0, 16) as u8;
        }
    }

    // Шалкер: грань крепления и приоткрытость (`AttachFace:2b`, `Peek:30b`).
    if let Some(face) = int_after(data, "AttachFace:") {
        mob.h.face = face.clamp(0, 5) as u8;
    }

    if let Some(peek) = int_after(data, "Peek:") {
        mob.h.peek = peek.clamp(0, 100) as u8;
    }

    // Эндермен: блок в руках, `carriedBlockState:{Name:"minecraft:grass_block",…}`
    // (вики, Enderman, «Entity data») — с указанными свойствами или без них.
    if let Some(at) = data.find("carriedBlockState:") {
        let tail = &data[at..];
        let name = tail.split("Name:").nth(1).map(|rest| rest.trim_start_matches('"').split(['"', ',', '}']).next().unwrap_or("")).unwrap_or("");
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        let props = tail.find("Properties:{").map(|start| {
            let inner = &tail[start + "Properties:{".len()..];
            let inner = &inner[..inner.find('}').unwrap_or(inner.len())];

            inner.split(',').map(|pair| pair.replace(['"', ' '], "").replace(':', "=")).filter(|pair| !pair.is_empty()).collect::<Vec<_>>().join(",")
        });
        let full = match props {
            Some(props) if !props.is_empty() => format!("{name}[{props}]"),
            _ => name.to_string(),
        };

        mob.h.carried = crate::blocks::state_from_text(&full).or_else(|| crate::blocks::state_by_name(name));
    }

    // Призванный с данными скелет без лука в руке — с луком он только если он
    // назван в данных (`equipment:{mainhand:{id:"minecraft:bow"}}`).
    if mob.kind.skeleton() && !data.is_empty() {
        mob.h.bow = data.contains("bow");
    }

    if mob.kind == crate::mob::Kind::Skeleton && let Some(ticks) = int_after(data, "StrayConversionTime:") {
        mob.h.converting = ticks.max(0);
    }

    if let Some(on) = flag_in(data, "ignited:") {
        mob.h.lit = on;
    }

    if mob.kind.skeleton() && let Some(value) = crate::snbt::parse(data)
        && let Some(chance) = value.get("drop_chances").and_then(|value| value.get("mainhand")).and_then(|value| value.as_f64())
    {
        mob.h.hand_drop = chance as f32;
    }

    // Шлем: `ArmorItems` (четвёртый элемент, голова) или `equipment:{head:…}` (с
    // 1.21.5): любой «…_helmet».
    if mob.kind != crate::mob::Kind::ArmorStand && let Some(at) = data.find("ArmorItems:").or_else(|| data.find("equipment:")) {
        mob.h.helmet = data[at..].contains("_helmet") || data[at..].contains("_head");
    }

    // Утопленник: что в основной руке и во второй (`equipment:{mainhand:{id:…},
    // offhand:{id:…}}`) и шанс выпасть основной (`drop_chances:{mainhand:0.0f}`).
    if mob.kind == crate::mob::Kind::Drowned {
        let held = |slot: &str| -> Option<&str> {
            let at = data.find(&format!("{slot}:{{"))?;
            let tail = &data[at..];

            tail.split("id:\"").nth(1).and_then(|rest| rest.split('"').next()).map(|id| id.strip_prefix("minecraft:").unwrap_or(id))
        };

        mob.h.hand = match held("mainhand") {
            Some("trident") => crate::mob::hostile::HAND_TRIDENT,
            Some("nautilus_shell") => crate::mob::hostile::HAND_SHELL,
            Some("fishing_rod") => crate::mob::hostile::HAND_ROD,
            _ => crate::mob::hostile::HAND_NONE,
        };
        mob.h.offhand_shell = held("offhand") == Some("nautilus_shell");

        if let Some(at) = data.find("drop_chances:{") {
            let tail = &data[at..];

            if let Some(chance) = tail.split("mainhand:").nth(1).and_then(|rest| rest.split([',', '}']).next()).and_then(|number| number.trim().trim_end_matches('f').parse::<f32>().ok()) {
                mob.h.hand_drop = chance;
            }
        }
    }

    // Житель: `VillagerData`, `Offers`, `Xp` (src/villager.rs).
    if matches!(mob.kind, crate::mob::Kind::Villager | crate::mob::Kind::ZombieVillager)
        && let Some(value) = crate::snbt::parse(data)
    {
        crate::villager::apply_data(&mut mob.villager, &value);
    }

    if let Some(health) = int_after(data, "Health:") {
        mob.vitals.health = (health as f32).clamp(0.5, mob.vitals.max_health);
    }

    if mob.kind == crate::mob::Kind::Slime {
        // Age/IsBaby не относятся к слизням: маленький — отдельный размер.
        mob.age = 0;
        if let Some(health) = crate::snbt::parse(data).and_then(|value| value.get("Health").and_then(|health| health.as_f64())) {
            mob.vitals.health = (health as f32).clamp(0.0, mob.vitals.max_health);
        }
    }

    if let Some(at) = data.find("Rotation:[") {
        let inner = &data[at + "Rotation:[".len()..];
        let inner = &inner[..inner.find(']').unwrap_or(inner.len())];
        let mut numbers = inner.split(',').map(|field| field.trim().trim_end_matches('f').parse::<f32>().ok());

        if let Some(Some(yaw)) = numbers.next() {
            mob.face(yaw);
        }

        if let Some(Some(pitch)) = numbers.next() {
            mob.pitch = pitch;
        }
    }
    crate::mob::equipment::apply(mob, data);
}

/// `summon cow|pig|sheep|chicken <x> <y> <z> {NoAI:1b,Age:-24000,…}` — животное
/// ровно в названной точке. У призванного детёныша `Age` отрицательный
/// (вики, Breeding: «Age: negative for baby»).
fn summon_mob(kind: crate::mob::Kind, rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: summon <cow|pig|sheep|chicken|zombie|skeleton|creeper|spider|enderman|shulker|witch|villager|end_crystal|ender_dragon> <x> <y> <z> {NoAI:1b,Age:-24000}".to_string());
    let origin = origin_of(source, shared);

    let (place, data) = match rest {
        [] => (Some(origin), ""),
        [x, y, z] => (entity_place(&[*x, *y, *z], origin), ""),
        [x, y, z, data] => (entity_place(&[*x, *y, *z], origin), *data),
        _ => return usage(),
    };

    let Some(place) = place else {
        return usage();
    };

    let mut mob = crate::mob::Mob::new(shared.next_entity_id(), kind, place);

    // Части дракона (голова, шея, тело, три хвоста, два крыла) — восемь
    // номеров следом за ним: по ним клиент бьёт (strike_mob) [проверить].
    if kind == crate::mob::Kind::EnderDragon {
        for _ in 0..crate::mob::DRAGON_PARTS {
            shared.next_entity_id();
        }
    }

    if kind == crate::mob::Kind::Slime && data.is_empty() {
        let mut rng = crate::mob::Rng::new(mob.entity_id as u64);
        let size = crate::mob::slime::roll_size(&mut rng);
        crate::mob::slime::set_size(&mut mob, size);
    }
    if crate::mob::land::animal(kind) {
        if data.is_empty() { crate::mob::land::initialize(&mut mob, &mut crate::mob::Rng::new(shared.next_entity_id() as u64)); }
        else if crate::mob::land::llama(kind) {
            // Чёрный ящик, llama-spit/llama-caravan: /summon с данными не бросает жребий
            // (без finalizeSpawn) — здоровье 53 у всех, вариант и сила из данных.
            mob.land.max_health = 53.0; crate::mob::land::health(&mut mob); mob.vitals.health = 53.0;
        }
    }
    // Лошадиные без данных бросают кости, как при естественном рождении.
    if crate::mob::horse::equine(kind) && data.is_empty() {
        let seed = (mob.entity_id as u64).wrapping_mul(0x9E3779B97F4A7C15);
        let mut rng = crate::mob::Rng::new(seed);
        crate::mob::horse::initialize(&mut mob, &mut rng);
        if kind == crate::mob::Kind::Horse { mob.horse.variant = crate::mob::horse::wild_variant(&mut rng); }
    }
    if crate::mob::wildlife::kind(kind) && data.is_empty() {
        let world = shared.world_in(dimension_of(source, shared)).lock().expect("мир захвачен другим потоком");
        let biome = world.mob_biome(place.0.floor() as i32, place.1.floor() as i32, place.2.floor() as i32);
        let mut rng = crate::mob::Rng::new(u64::from(mob.entity_id as u32));
        crate::mob::wildlife::initialize(&mut mob, biome, &mut rng);
    }
    // /summon без данных не даёт водным видам случайный вид (чёрный ящик water-spawn-all:
    // рыба без Variant, аксолотль без Variant — по умолчанию); случайный вид — у яиц и естественного появления.
    apply_mob_data(&mut mob, data);

    // Свойство 20 у жителя: призванный без данных или с VillagerData — да,
    // с прочими данными — нет (чёрный ящик, villager-shake, villager-claim).
    if matches!(kind, crate::mob::Kind::Villager | crate::mob::Kind::ZombieVillager) {
        mob.villager.finalized = data.is_empty() || data.contains("VillagerData");
    }

    // Призванный зомби-житель с `ConversionTime` лечится: сила — на следующем такте.
    if kind == crate::mob::Kind::ZombieVillager && mob.villager.conversion > 0 {
        mob.villager.fresh_cure = true;
    }

    if kind == crate::mob::Kind::Chicken {
        mob.egg_timer = 6000 + (mob.entity_id as u32 % 6001) as i32;
    }

    shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком").add_mob(mob);

    Answer::saying(format!("Призвана сущность {}", kind.name()))
}

/// `data merge entity <кто> {NoAI:0b,…}` — правка данных мобов.
fn data_command(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    if let ["merge", "block", x, y, z, data @ ..] = parts {
        let Some((x, y, z)) = entity_place(&[x, y, z], origin_of(source, shared)) else {
            return Answer::saying("Не понял место блока".to_string());
        };
        let p = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        let mut world = shared.world_in(dimension_of(source, shared)).lock().expect("мир");
        let changed = world.with_container(p.0, p.1, p.2, |c| crate::beacon::apply_text(c, &data.join(" "))).unwrap_or(false);
        if changed {
            let nbt = crate::beacon::nbt(world.container(p.0, p.1, p.2).unwrap());
            // Оригинал перед данными шлёт и сам блок (чёрный ящик, beacon-effect).
            world.tell_directly(p.0, p.1, p.2);
            world.note_block_entity(p.0, p.1, p.2, crate::containers::Kind::Beacon.block_entity_type(), nbt);
        }
        return Answer::saying(if changed { "Данные маяка обновлены" } else { "Нет маяка или данные неверны" }.to_string());
    }

    let copied;
    let (who,data) = match parts {
        ["merge", "entity", who, data] => (*who,*data),
        // Вики, Commands/data: копирование UUID игрока в Owner приручаемого.
        ["modify", "entity", who, "Owner", "set", "from", "entity", player, "UUID"] => {
            let targets=find_targets(player,source,shared);
            let [(uuid,_)] = targets.as_slice() else { return Answer::saying("Нужен один игрок".to_string()); };
            let words: Vec<_> = uuid.as_chunks::<4>().0.iter().map(|b| i32::from_be_bytes(*b).to_string()).collect();
            copied=format!("{{Owner:[I;{}]}}",words.join(","));
            (*who,copied.as_str())
        }
        _ => return Answer::saying("Нужно: data merge entity @e[type=cow,limit=1] {NoAI:0b}".to_string()),
    };

    let Some(chosen) = Selector::parse(who) else {
        return Answer::saying("Не понял, кого править".to_string());
    };

    let (x, y, z) = origin_of(source, shared);
    let mut left = chosen.limit.unwrap_or(usize::MAX);
    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
    let mut changed = 0;

    let mut order: Vec<usize> = (0..items.mobs().len()).collect();
    order.sort_by(|a, b| {
        let d = |m: &crate::mob::Mob| (m.x - x).powi(2) + (m.y - y).powi(2) + (m.z - z).powi(2);
        d(&items.mobs()[*a]).total_cmp(&d(&items.mobs()[*b]))
    });
    for index in order {
        let mob = &mut items.mobs_mut()[index];
        if left > 0 && chosen.takes_tagged(mob.kind.name(), &mob.tags) && chosen.reaches((x, y, z), (mob.x, mob.y, mob.z)) {
            apply_mob_data(mob, data);
            // Правка данных — как перезагрузка из NBT: голова смотрит по телу,
            // клиентам уходят атрибуты (чёрный ящик, cow-physics).
            mob.head_yaw = mob.yaw;
            mob.attr_changes += 1;
            left -= 1;
            changed += 1;
        }
    }

    Answer::saying(format!("Изменено сущностей: {}", changed))
}

/// Official775: relative от rotation контекста; wrap обоих углов, затем clamp pitch.
/// Своя запись: tools/protocol-bridge/testdata/mob-teleport-observations.json.
/// Source::Block не хранит rotation; как у консоли, его исходные углы — нули.
fn mob_teleport_rotation(yaw: &str, pitch: &str, source: &Source, shared: &Shared) -> Option<(f32, f32)> {
    let origin = match source {
        Source::Player { uuid, .. } => shared.players.lock().expect("список игроков")
            .members().iter().find(|member| &member.uuid == uuid)
            .map(|member| (member.position.yaw, member.position.pitch))?,
        Source::Console | Source::Block { .. } => (0.0, 0.0),
    };
    let angle = |text: &str, from: f32| -> Option<f32> {
        let value = match text.strip_prefix('~') {
            Some("") => from,
            Some(offset) => from + offset.parse::<f32>().ok()?,
            None => text.parse::<f32>().ok()?,
        };
        value.is_finite().then_some(value)
    };
    let yaw = angle(yaw, origin.0)?;
    let pitch = angle(pitch, origin.1)?;
    let wrap = |value: f32| {
        // Сначала остаток: прибавление 180 потеряло бы точность малых углов.
        let value = value % 360.0;
        if value >= 180.0 { value - 360.0 }
        else if value < -180.0 { value + 360.0 }
        else { value }
    };
    Some((wrap(yaw), wrap(pitch).clamp(-90.0, 90.0)))
}

/// `tp @e[type=cow,…] <x> <y> <z> [yaw pitch]` — перенос мобов.
/// Свои записи official340/775: head следует указанному yaw.
fn teleport_mobs(who: &str, place: &[&str], source: &Source, shared: &Shared) -> Answer {
    let Some(chosen) = Selector::parse(who) else {
        return Answer::saying("Не понял, кого переносить".to_string());
    };

    // Всё разбирается до захвата/изменения мобов: неверный последний угол
    // не может оставить частично перенесённую группу сущностей.
    let look = match place {
        [_, _, _] => None,
        [_, _, _, yaw, pitch] => {
            let Some(look) = mob_teleport_rotation(yaw, pitch, source, shared) else {
                return Answer::saying("Не понял поворот: нужны конечные yaw и pitch".to_string());
            };
            Some(look)
        }
        _ => return Answer::saying("Нужно: tp @e[type=cow] <x> <y> <z> [yaw pitch]".to_string()),
    };
    let origin = origin_of(source, shared);
    let Some((x, y, z)) = entity_place(&place[..3], origin) else {
        return Answer::saying("Нужно: tp @e[type=cow] <x> <y> <z> [yaw pitch]".to_string());
    };

    let mut left = chosen.limit.unwrap_or(usize::MAX);
    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
    let mut moved = 0;

    for mob in items.mobs_mut() {
        if left > 0 && chosen.takes_tagged(mob.kind.name(), &mob.tags) && chosen.reaches(origin, (mob.x, mob.y, mob.z)) {
            (mob.x, mob.y, mob.z) = (x, y, z);
            if let Some((yaw, pitch)) = look {
                mob.face(yaw);
                mob.pitch = pitch;
            }
            (mob.vx, mob.vy, mob.vz) = (0.0, 0.0, 0.0);
            // enchant-frost-mobs: NoAI сохраняет OnGround при /tp и уже
            // в следующем такте замораживает воду. Его физика не обновляет
            // этот флаг; воздушного NoAI при этом не ставим на землю.
            if !mob.no_ai { mob.on_ground = false; }
            mob.vitals.fall = 0.0;
            mob.teleports += 1;
            left -= 1;
            moved += 1;
        }
    }

    Answer::saying(format!("Перенесено сущностей: {}", moved))
}

/// `NoGravity:1b` (или `true`) в данных призыва: сущность без тяжести.
fn no_gravity(text: &str) -> bool {
    crate::snbt::parse(text).and_then(|v| v.get("NoGravity").and_then(crate::snbt::Value::as_f64)).is_some_and(|n| n != 0.0)
}

/// `summon lightning_bolt [<x> <y> <z>]` — молния в названной точке (без
/// точки — там, где стоит вызвавший). Бьёт в свой первый такт (lightning.rs).
/// Громоотводы командную молнию к себе не притягивают: «do not divert ...
/// lightning summoned by commands» (вики, Lightning Rod).
fn summon_lightning(rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = origin_of(source, shared);
    let place = match rest {
        [] => Some(origin),
        [x, y, z] => entity_place(&[*x, *y, *z], origin),
        _ => None,
    };

    let Some(place) = place else {
        return Answer::saying("Нужно: summon lightning_bolt <x> <y> <z>".to_string());
    };

    shared
        .items_in(dimension_of(source, shared))
        .lock()
        .expect("предметы захвачены другим потоком")
        .add_bolt(crate::lightning::Bolt::new(shared.next_entity_id(), place));

    Answer::saying("Призвана сущность lightning_bolt".to_string())
}

/// Предмет из данных сущности, включая компоненты: они участвуют в слиянии.
pub(crate) fn item_from_text(text: &str) -> Option<crate::inventory::Stack> {
    let root = crate::snbt::parse(text)?;
    let item = root.get("Item").or_else(|| root.get("item"))?;
    let name = item.get("id")?.as_str()?;
    let count = item.get("count").or_else(|| item.get("Count")).and_then(crate::snbt::Value::as_f64).unwrap_or(1.0) as i32;
    if count <= 0 { return None; }
    let mut spec = name.to_string();
    if let Some(crate::snbt::Value::Map(components)) = item.get("components") {
        spec.push('[');
        for (index, (key, value)) in components.iter().enumerate() {
            if index > 0 { spec.push(','); }
            spec.push_str(key);
            spec.push('=');
            spec.push_str(&component_text(value));
        }
        spec.push(']');
    }
    stack_from_spec(&spec, count).ok().flatten().map(|stack| stack.with_count(count))
}

/// JSON — подмножество SNBT; кавычки сохраняют вложенные строки компонентов.
pub(crate) fn component_text(value: &crate::snbt::Value) -> String {
    use crate::snbt::Value;
    match value {
        Value::Num(n) => n.to_string(),
        Value::Str(s) => serde_json::to_string(s).expect("строка JSON"),
        Value::List(values) => format!("[{}]", values.iter().map(component_text).collect::<Vec<_>>().join(",")),
        Value::Map(fields) => format!("{{{}}}", fields.iter().map(|(key, value)| format!("{}:{}", serde_json::to_string(key).expect("ключ JSON"), component_text(value))).collect::<Vec<_>>().join(",")),
    }
}

/// `kill @e[type=<вид>,distance=..<N>,limit=<N>]` — убрать сущности:
/// вагонетки, предметы, подожжённый динамит, падающие блоки, молнии. `type=!<вид>` —
/// все, кроме этого вида. Игроков команда не трогает.
///
/// Как у оригинала, сама вагонетка предметом не выпадает, а содержимое
/// сундука или воронки высыпается (чёрный ящик: после kill вагонетки с
/// сундуком лежит её земля, предмета вагонетки нет). [проверить]: разброс
/// высыпанного.
fn kill(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    kill_at(parts, source, shared, origin_of(source, shared))
}

fn kill_at(parts: &[&str], source: &Source, shared: &Shared, origin: (f64, f64, f64)) -> Answer {
    // `kill <игрок>` / `kill @s` — убить игрока (вид урона generic_kill).
    if let Some(who) = parts.first().filter(|who| !who.starts_with("@e")) {
        let targets = if *who == "@s" {
            match source {
                Source::Player { uuid, name } => vec![(*uuid, name.clone())],
                Source::Console | Source::Block { .. } => Vec::new(),
            }
        } else {
            find_targets(who, source, shared)
        };

        if !targets.is_empty() {
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for (uuid, _) in &targets {
                players.order(uuid, Order::Hurt { amount: f32::MAX, source: crate::survival::Source::GenericKill });
            }

            return Answer::saying(format!("Убит: {}", names_of(&targets)));
        }
    }

    let Some(chosen) = parts.first().and_then(|who| Selector::parse(who)) else {
        return Answer::saying("Нужно: kill @e[type=<вид>] — вагонетки, item, tnt, falling_block".to_string());
    };

    let (x, y, z) = origin;
    let near = |at: (f64, f64, f64)| chosen.reaches((x, y, z), at);
    let mut left = chosen.limit.unwrap_or(usize::MAX);

    let mut world = shared.world_in(dimension_of(source, shared)).lock().expect("мир захвачен другим потоком");
    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");

    // Рамки убираются молча, без выпадения (вики, Commands/kill: сущность
    // просто исчезает). [проверить]: у оригинала рамка при kill могла бы
    // выронить содержимое.
    let frames = items.remove_frames(|frame| {
        let name = if frame.glow { "glow_item_frame" } else { "item_frame" };
        let picked = left > 0 && chosen.takes(name) && near(frame.place());

        left -= usize::from(picked);
        picked
    });

    for frame in &frames {
        world.forget_frame(frame.pos, frame.facing);
    }

    if !frames.is_empty() {
        crate::frame::refresh_marks(&mut world, &items);
    }

    let carts = items.remove_carts(|cart| {
        let picked = left > 0 && chosen.takes(cart.kind.name()) && near((cart.x, cart.y, cart.z));

        left -= usize::from(picked);
        picked
    });

    let dropped = items.remove_items(|item| {
        let picked = left > 0 && chosen.takes(item.type_name()) && near((item.x, item.y, item.z));

        left -= usize::from(picked);
        picked
    });

    // Стрелы игроков: убираются молча (клиентам — по слежению).
    let shot = {
        let dimension = dimension_of(source, shared);
        let mut arrows = shared.arrows.lock().expect("стрелы захвачены другим потоком");
        let before = arrows.list.len();

        arrows.list.retain(|arrow| {
            let picked = arrow.dimension == dimension && left > 0 && chosen.takes("arrow") && near((arrow.x, arrow.y, arrow.z));

            left -= usize::from(picked);
            !picked
        });

        before - arrows.list.len()
    };

    // Последнюю стрелу клиентам убирает слежение стрел: в этом такте оно должно
    // пройти, хоть стрел уже и нет (wither-half-armor: «arrow убран» в такт
    // команды).
    if shot > 0 {
        world.note_arrow_tracker();
    }

    let mut orbs = 0;

    if chosen.takes("experience_orb") {
        orbs = items.remove_orbs(|orb| {
            let picked = left > 0 && near((orb.x, orb.y, orb.z));

            left -= usize::from(picked);
            picked
        });
    }

    // Животные погибают, как от урона `generic_kill`: падают набок, роняют
    // добычу (вики, Commands/kill: сущность умирает).
    let mut killed_mobs = 0;

    let mut discarded = Vec::new();

    for mob in items.mobs_mut() {
        if left > 0 && chosen.takes_tagged(mob.kind.name(), &mob.tags) && near((mob.x, mob.y, mob.z)) && !mob.is_dead() {
            // Дракона и кристалл команда убирает сразу — без анимации смерти,
            // опыта и взрыва (чёрный ящик, dragon-death: «убран» в тот же такт).
            if matches!(mob.kind, crate::mob::Kind::EnderDragon | crate::mob::Kind::EndCrystal) {
                discarded.push(mob.entity_id);
            } else {
                mob.hurt(f32::MAX, crate::survival::Source::GenericKill, None);
            }

            left -= 1;
            killed_mobs += 1;
        }
    }

    if !discarded.is_empty() {
        items.remove_mobs(|mob| discarded.contains(&mob.entity_id));
    }

    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));

    let bolts = items.remove_bolts(|bolt| {
        let picked = left > 0 && chosen.takes("lightning_bolt") && near((bolt.x, bolt.y, bolt.z));

        left -= usize::from(picked);
        picked
    });

    for cart in &carts {
        for stack in cart.slots.iter().flatten() {
            items.drop_item(shared.next_entity_id(), *stack, (cart.x, cart.y, cart.z), (0.0, 0.0, 0.0), crate::items::MINED_DELAY);
        }
    }

    Answer::saying(format!("Убрано сущностей: {}", carts.len() + dropped.len() + bolts + frames.len() + killed_mobs + orbs + shot))
}

/// `damage <кто> <сколько> [вид]` — урон мобам и игрокам (вики,
/// Commands/damage); игрокам — как `Hurt`. У оригинала команда попадает то до
/// такта игрока, то после: Set Health и свойства приходят на +0 или +1
/// (effect-absorption, два прогона); у нас — всегда до такта (+0).
#[path = "commands_damage.rs"]
mod damage_command;

fn damage(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    damage_command::run(parts, source, shared)
}

/// `difficulty <peaceful|easy|normal|hard>` — сложность на ходу.
fn difficulty(parts: &[&str], shared: &Shared) -> Answer {
    let level = match parts.first().copied() {
        Some("peaceful" | "p" | "0") => 0,
        Some("easy" | "e" | "1") => 1,
        Some("normal" | "n" | "2") => 2,
        Some("hard" | "h" | "3") => 3,
        _ => return Answer::saying("Нужно: difficulty <peaceful|easy|normal|hard>".to_string()),
    };

    shared.difficulty.store(level, std::sync::atomic::Ordering::Relaxed);

    Answer::saying(format!("Сложность: {}", parts[0]))
}

/// `effect clear @e[…] [эффект]`: снять эффекты с мобов, выбранных селектором
/// (вики, Commands/effect: «clear … removes all or a specific effect»).
fn clear_mob_effects(who: &str, kind: Option<crate::survival::EffectKind>, source: &Source, shared: &Shared) -> Option<Answer> {
    if !who.starts_with("@e") {
        return None;
    }
    let chosen = Selector::parse(who)?;
    let (x, y, z) = origin_of(source, shared);
    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
    let mut cleared = 0;

    for mob in items.mobs_mut() {
        if chosen.takes_tagged(mob.kind.name(), &mob.tags) && chosen.reaches((x, y, z), (mob.x, mob.y, mob.z)) {
            cleared += 1;
            mob.vitals.remove_effects(kind);
        }
    }

    Some(Answer::saying(format!("Эффекты сняты: {} сущностей", cleared)))
}

/// `effect give <игрок> <эффект> [секунды] [уровень] [скрыть]` и
/// `effect clear <игрок>`. Считаются голод, насыщение, сила, слабость, скорость, замедление и сопротивление — остальным вредно
/// не быть [проверить].
fn effect(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: effect give <игрок> <эффект> [секунды|infinite] [уровень] [скрыть частицы] | effect clear <игрок> [эффект]".to_string());

    match parts {
        ["give", who, name, rest @ ..] => {
            let Some(kind) = crate::survival::EffectKind::from_name(name) else {
                return Answer::saying(format!("Такого эффекта пока нет: {}", name));
            };
            // Мобам эффект даёт селектор сущностей `@e[type=…]` (зомби-житель под
            // слабостью лечится золотым яблоком).
            if who.starts_with("@e")
                && let Some(chosen) = Selector::parse(who)
            {
                let ticks = match rest.first().copied() {
                    Some("infinite") => crate::survival::INFINITE,
                    other => other.and_then(|value| value.parse::<i32>().ok()).unwrap_or(30).max(1) * 20,
                };
                let amplifier: i32 = rest.get(1).and_then(|value| value.parse().ok()).unwrap_or(0);
                let hidden = rest.get(2).copied() == Some("true");
                let (x, y, z) = origin_of(source, shared);
                let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
                let mut changed = 0;

                for mob in items.mobs_mut() {
                    if chosen.takes_tagged(mob.kind.name(), &mob.tags) && chosen.reaches((x, y, z), (mob.x, mob.y, mob.z))
                        && mob.accepts_effect(kind)
                    {
                        mob.vitals.give_effect_shown(crate::survival::Effect { kind, amplifier: amplifier.clamp(0, 255), ticks_left: ticks, ambient: false, visible: !hidden, show_icon: !hidden });
                        changed += 1;
                    }
                }

                return Answer::saying(format!("Эффект {} наложен: {} сущностей", name, changed));
            }

            let targets = find_targets(who, source, shared);

            if targets.is_empty() {
                return Answer::saying("Такого игрока нет на сервере".to_string());
            }

            // Срок: секунды или «infinite» (вики, Commands/effect); без
            // срока — 30 с. Четвёртый довод «true» прячет частицы и значок.
            let ticks = match rest.first().copied() {
                Some("infinite") => crate::survival::INFINITE,
                other => other.and_then(|value| value.parse::<i32>().ok()).unwrap_or(30).max(1) * 20,
            };
            let amplifier: i32 = rest.get(1).and_then(|value| value.parse().ok()).unwrap_or(0);
            let hidden = rest.get(2).copied() == Some("true");
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for (uuid, _) in &targets {
                players.order(uuid, Order::Effect(crate::survival::Effect { kind, amplifier: amplifier.clamp(0, 255), ticks_left: ticks, ambient: false, visible: !hidden, show_icon: !hidden }));
            }

            Answer::saying(format!("Эффект {} наложен: {}", name, names_of(&targets)))
        }
        ["clear", who, name] => {
            let Some(kind) = crate::survival::EffectKind::from_name(name) else {
                return Answer::saying(format!("Такого эффекта пока нет: {}", name));
            };
            if let Some(answer) = clear_mob_effects(who, Some(kind), source, shared) {
                return answer;
            }
            let targets = find_targets(who, source, shared);
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for (uuid, _) in &targets {
                players.order(uuid, Order::ClearEffect(kind));
            }

            Answer::saying(format!("Эффект {} снят: {}", name, names_of(&targets)))
        }
        ["clear", who] => {
            if let Some(answer) = clear_mob_effects(who, None, source, shared) {
                return answer;
            }
            let targets = find_targets(who, source, shared);
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for (uuid, _) in &targets {
                players.order(uuid, Order::ClearEffects);
            }

            Answer::saying(format!("Эффекты сняты: {}", names_of(&targets)))
        }
        _ => usage(),
    }
}

/// `recipe give|take <игрок> *|<рецепт>`: книга рецептов игрока (вики, Commands/recipe).
fn recipe(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let [action @ ("give" | "take"), who, what] = parts else {
        return Answer::saying("Нужно: recipe give|take <игрок> *|<рецепт>".to_string());
    };
    let targets = find_targets(who, source, shared);
    let names = if *what == "*" { Vec::new() } else { vec![(*what).to_string()] };
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

    for (uuid, _) in &targets {
        players.order(uuid, Order::Recipes { give: *action == "give", names: names.clone() });
    }

    Answer::saying(format!("Рецепты: {action} {what} — {}", names_of(&targets)))
}

/// `experience add|set <игрок> <число> [points|levels]` (и `xp`): опыт игрока.
fn experience(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: experience add|set <игрок> <число> [points|levels]".to_string());

    let [action @ ("add" | "set"), who, amount, rest @ ..] = parts else {
        return usage();
    };
    let Ok(amount) = amount.parse::<i32>() else {
        return usage();
    };
    let levels = match rest.first().copied() {
        None | Some("points") => false,
        Some("levels") => true,
        _ => return usage(),
    };
    let set = *action == "set";

    if set && amount < 0 {
        return Answer::saying("Значение не может быть отрицательным".to_string());
    }

    let targets = find_targets(who, source, shared);

    if targets.is_empty() {
        return Answer::saying("Такого игрока нет на сервере".to_string());
    }

    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

    for (uuid, _) in &targets {
        players.order(uuid, Order::Experience { set, amount, levels });
    }

    Answer::saying(format!("Опыт изменён: {}", names_of(&targets)))
}

/// `gamerule <правило> [значение]`: настоящим считается `keepInventory`,
/// остальные правила принимаются и молча игнорируются.
fn gamerule(parts: &[&str], shared: &Shared) -> Answer {
    // Правила командных блоков (command_block.rs).
    if let [rule, rest @ ..] = parts
        && rest.len() <= 1
        && let Some(answer) = crate::command_block::gamerule(shared, rule, rest.first().copied())
    {
        return match answer {
            Ok(reply) => Answer::saying(reply),
            Err(why) => {
                note_result(0);
                Answer::saying(why)
            }
        };
    }

    match parts {
        ["doLimitedCrafting" | "limited_crafting" | "minecraft:limited_crafting", value @ ("true" | "false")] => {
            shared.limited_crafting.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило limited_crafting: {value}"))
        }
        ["doLimitedCrafting" | "limited_crafting" | "minecraft:limited_crafting"] => Answer::saying(format!("{}", shared.limited_crafting.load(std::sync::atomic::Ordering::Relaxed))),
        ["doFireTick" | "fire_tick" | "minecraft:fire_tick", value @ ("true" | "false")] => {
            for dimension in crate::dimension::Dimension::ALL { shared.world_in(dimension).lock().expect("мир").fire_tick = *value == "true"; }
            Answer::saying(format!("Правило doFireTick: {}", value))
        }
        ["doFireTick" | "fire_tick" | "minecraft:fire_tick"] => Answer::saying(format!("Правило doFireTick: {}", shared.world_in(crate::dimension::Dimension::Overworld).lock().expect("мир").fire_tick)),
        ["keepInventory" | "keep_inventory", value] => {
            let on = *value == "true";

            shared.keep_inventory.store(on, std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило keepInventory: {}", value))
        }
        // 26.1: `spawn_mobs` (прежнее doMobSpawning) — появляются ли животные
        // сами в новых чанках.
        ["spawn_mobs" | "doMobSpawning", value] => {
            shared.spawn_mobs.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило spawn_mobs: {}", value))
        }
        ["spawn_monsters", value] => {
            shared.spawn_monsters.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило spawn_monsters: {}", value))
        }
        ["spawn_wandering_traders" | "doTraderSpawning", value] => {
            shared.spawn_traders.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило spawn_wandering_traders: {}", value))
        }
        ["spawn_wandering_traders" | "doTraderSpawning"] => Answer::saying(format!("{}", shared.spawn_traders.load(std::sync::atomic::Ordering::Relaxed))),
        ["spawn_phantoms" | "doInsomnia", value] => {
            shared.spawn_phantoms.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило spawn_phantoms: {}", value))
        }
        ["mob_griefing" | "mobGriefing", value] => {
            shared.mob_griefing.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило mob_griefing: {}", value))
        }
        // Набеги и патрули (raid.rs); прежнее disableRaids — наоборот.
        ["raids", value] => {
            shared.raids.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило raids: {}", value))
        }
        ["disableRaids", value] => {
            shared.raids.store(*value != "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило raids: {}", *value != "true"))
        }
        ["spawn_patrols" | "doPatrolSpawning", value] => {
            shared.spawn_patrols.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило spawn_patrols: {}", value))
        }
        ["block_drops" | "doTileDrops", value] => {
            shared.block_drops.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило block_drops: {}", value))
        }
        ["announce_advancements" | "announceAdvancements", value] => {
            shared.announce_advancements.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило announce_advancements: {}", value))
        }
        ["freeze_damage" | "freezeDamage", value] if matches!(*value, "true" | "false") => {
            shared.freeze_damage.store(*value == "true", std::sync::atomic::Ordering::Relaxed);
            Answer::saying(format!("Правило freeze_damage: {}", value))
        }
        ["freeze_damage" | "freezeDamage"] => Answer::saying(format!("Правило freeze_damage: {}", shared.freeze_damage.load(std::sync::atomic::Ordering::Relaxed))),
        ["natural_health_regeneration" | "naturalRegeneration", value] => {
            shared.natural_regeneration.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило natural_health_regeneration: {}", value))
        }
        // Скорость случайного такта (farming.rs): мест на секцию за такт.
        ["random_tick_speed" | "randomTickSpeed", value] => match value.parse::<i32>() {
            Ok(speed) => {
                crate::farming::set_random_tick_speed(speed);

                Answer::saying(format!("Правило random_tick_speed: {}", speed))
            }
            Err(_) => {
                note_result(0);
                Answer::saying(format!("Ожидалось целое число, а не «{}»", value))
            }
        },
        ["random_tick_speed" | "randomTickSpeed"] => Answer::saying(format!("Правило random_tick_speed сейчас: {}", crate::farming::random_tick_speed())),
        ["advance_weather" | "doWeatherCycle", value] => {
            shared.advance_weather.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило advance_weather: {}", value))
        }
        // 26.1: `players_sleeping_percentage` (прежнее playersSleepingPercentage):
        // доля спящих для пропуска ночи (вики, Game rule; bed.rs).
        ["players_sleeping_percentage" | "playersSleepingPercentage", value] => match value.parse::<i32>() {
            Ok(percentage) => {
                shared.sleeping_percentage.store(percentage, std::sync::atomic::Ordering::Relaxed);

                Answer::saying(format!("Правило players_sleeping_percentage: {}", percentage))
            }
            Err(_) => Answer::saying(format!("Ожидалось целое число, а не «{}»", value)),
        },
        [rule, value] => Answer::saying(format!("Правило {}: {}", rule, value)),
        _ => Answer::saying("Нужно: gamerule <правило> <значение>".to_string()),
    }
}

/// Выбор сущностей `@e[…]` для `kill`.
pub(crate) struct Selector {
    /// Вид: `Some((true, "item"))` — только предметы, `Some((false, "player"))`
    /// — все, кроме игроков; `None` — любые.
    pub(crate) kind: Option<(bool, String)>,

    /// Метки `tag=x` / `tag=!x` (вики, Target selectors): у мобов они
    /// берутся из `Tags` данных; пустое имя — «без меток» / «с любой».
    tags: Vec<(bool, String)>,

    /// Не дальше скольких блоков от того, кто ввёл команду.
    pub(crate) distance: Option<f64>,

    /// Сколько самое большее.
    pub(crate) limit: Option<usize>,
}

impl Selector {
    pub(crate) fn parse(text: &str) -> Option<Selector> {
        let mut chosen = Selector { kind: None, tags: Vec::new(), distance: None, limit: text.starts_with("@n").then_some(1) };

        let inner = match text {
            "@e" | "@n" => return Some(chosen),
            _ => text.strip_prefix("@e[").or_else(|| text.strip_prefix("@n["))?.strip_suffix(']')?,
        };

        for pair in split_selector_arguments(inner) {
            // Метки, команды и счёт: у мобов их у нас нет (commands_score).
            let (key, value) = pair.split_once('=')?;
            if key == "tag" {
                chosen.tags.push(match value.strip_prefix('!') { Some(tag) => (false, tag.to_string()), None => (true, value.to_string()) });
                continue;
            }
            let mut board = crate::commands_score::BoardFilter::default();
            if board.take(key, value)? {
                if !board.matches_entity() {
                    chosen.limit = Some(0);
                }
                continue;
            }

            match (key, value) {
                ("type", name) => {
                    let (wanted, name) = match name.strip_prefix('!') {
                        Some(name) => (false, name),
                        None => (true, name),
                    };

                    chosen.kind = Some((wanted, name.strip_prefix("minecraft:").unwrap_or(name).to_string()));
                }
                // `..N`, `M..N` или `N` — важна только верхняя граница.
                ("distance", range) => chosen.distance = Some(range.rsplit("..").next()?.parse().ok()?),
                ("limit", count) => chosen.limit = Some(count.parse().ok()?),
                // Порядок (`sort=nearest`, `random`…): правка данных идёт от ближнего.
                ("sort", "nearest" | "arbitrary" | "furthest" | "random") => {}
                _ => return None,
            }
        }

        Some(chosen)
    }

    /// Подходит ли сущность такого вида без меток (предметы, стрелы и прочее).
    pub(crate) fn takes(&self, kind: &str) -> bool {
        self.takes_tagged(kind, &[])
    }

    /// Подходит ли моб: вид и метки.
    pub(crate) fn takes_tagged(&self, kind: &str, tags: &[String]) -> bool {
        let kind_ok = match &self.kind {
            Some((wanted, name)) => (name == kind) == *wanted,
            None => true,
        };

        kind_ok && self.tags.iter().all(|(wanted, tag)| {
            let has = if tag.is_empty() { !tags.is_empty() } else { tags.iter().any(|known| known == tag) };

            if tag.is_empty() { has != *wanted } else { has == *wanted }
        })
    }

    /// Достаточно ли близко сущность.
    pub(crate) fn reaches(&self, (x, y, z): (f64, f64, f64), (at_x, at_y, at_z): (f64, f64, f64)) -> bool {
        self.distance.is_none_or(|distance| (at_x - x).powi(2) + (at_y - y).powi(2) + (at_z - z).powi(2) <= distance * distance)
    }
}

/// `tp @e[type=<вагонетка>] <x> <y> <z>` — перенести вагонетки. Скорость
/// по горизонтали сохраняется, по вертикали обнуляется, и вагонетка
/// считается стоящей на земле: у оригинала перенесённая в воздух вагонетка
/// в первый такт падает со скоростью −0,04·0,5·0,95 = −0,019 — как с земли
/// (чёрный ящик, rail-detector).
fn teleport_carts(who: &str, place: &[&str], source: &Source, shared: &Shared) -> Answer {
    let Some(chosen) = cart_selector(who) else {
        return Answer::saying("Не понял, кого переносить".to_string());
    };

    let Some((x, y, z)) = entity_place(place, origin_of(source, shared)) else {
        return Answer::saying("Куда переносить? tp @e[type=minecart] <x> <y> <z>".to_string());
    };

    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
    let ids = chosen_carts(&items, chosen);

    for cart in items.carts_mut().iter_mut().filter(|cart| ids.contains(&cart.entity_id)) {
        cart.x = x;
        cart.y = y;
        cart.z = z;
        cart.vy = 0.0;
        cart.on_ground = true;
        cart.teleports += 1;
    }

    Answer::saying(format!("Перенесено сущностей: {}", ids.len()))
}

/// Ники через запятую — для ответов.
pub(crate) fn names_of(targets: &[([u8; 16], String)]) -> String {
    targets
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// В каком измерении тот, кто ввёл команду: игрок — в своём, консоль — в
/// обычном мире.
pub(crate) fn dimension_of(source: &Source, shared: &Shared) -> Dimension {
    match source {
        Source::Player { uuid, .. } => shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .members()
            .iter()
            .find(|member| &member.uuid == uuid)
            .map(|member| member.dimension)
            .unwrap_or_default(),
        Source::Console => Dimension::Overworld,
        Source::Block { dimension, .. } => *dimension,
    }
}

/// Ставит блок в одно место (`setblock`) или заливает им область (`fill`).
///
/// Блок записывается как в игре: `minecraft:lever[face=floor,powered=true]`.
/// Как и у обычного сервера, соседи узнают о новом блоке, а редстоун
/// пересчитывается; своей логики нажатия у поставленного рычага нет.
/// Область у `fill` ограничена, как в игре, 32 768 блоками.
/// Слова команды: пробел внутри `{}`, `[]` и кавычек слова не делит.
pub(crate) fn words_of(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for c in command.chars() {
        match (quote, c) {
            (Some(_), _) if escaped => escaped = false,
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') if depth > 0 => quote = Some(c),
            (None, '{' | '[') => depth += 1,
            (None, '}' | ']') => depth -= 1,
            (None, c) if depth <= 0 && c.is_whitespace() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }

                continue;
            }
            _ => {}
        }

        word.push(c);
    }

    if !word.is_empty() {
        words.push(word);
    }

    words
}

/// Координата блока: число или `~` со сдвигом от `from` (у командного блока
/// и игрока — от их места).
fn block_coordinate(text: &str, from: f64) -> Option<i32> {
    match text.strip_prefix('~') {
        Some("") => Some(from.floor() as i32),
        Some(shift) => Some((from + shift.parse::<f64>().ok()?).floor() as i32),
        None => text.parse::<i32>().ok(),
    }
}

fn set_blocks(parts: &[&str], source: &Source, shared: &Shared, dimension: Dimension, area: bool) -> Answer {
    let (corners, block) = match (area, parts) {
        (false, [x, y, z, block, ..]) => ([*x, *y, *z, *x, *y, *z], *block),
        (true, [x1, y1, z1, x2, y2, z2, block, ..]) => ([*x1, *y1, *z1, *x2, *y2, *z2], *block),
        (false, _) => {
            return Answer::saying("Нужно: setblock <x> <y> <z> <блок>".to_string());
        }
        (true, _) => {
            return Answer::saying(
                "Нужно: fill <x1> <y1> <z1> <x2> <y2> <z2> <блок>".to_string(),
            );
        }
    };

    let mut numbers = [0i32; 6];
    let from = if corners.iter().any(|text| text.starts_with('~')) { origin_of(source, shared) } else { (0.0, 0.0, 0.0) };

    for (index, (slot, text)) in numbers.iter_mut().zip(corners).enumerate() {
        let base = [from.0, from.1, from.2][index % 3];
        let Some(value) = block_coordinate(text, base) else {
            note_result(0);
            return Answer::saying(format!("Не понял координату «{}»", text));
        };
        *slot = value;
    }

    // Данные блока в фигурных скобках — как у оригинала: пока понимаются
    // только ячейки хранилища (`{Items:[{Slot:0b,id:"minecraft:stone",count:5}]}`).
    let (block, data) = match block.find('{') {
        Some(at) => (&block[..at], Some(&block[at..])),
        None => (block, None),
    };
    // Командному блоку — его поля (command_block.rs), прочим — ячейки.
    let command_block = blocks::state_from_text(block).and_then(crate::command_block::mode_of).is_some();
    let spawner = blocks::state_from_text(block).and_then(blocks::block_at_state) == Some("spawner");
    // Испытательный спавнер и хранилище — свои поля (trial.rs).
    let trial = matches!(blocks::state_from_text(block).and_then(blocks::block_at_state), Some("trial_spawner" | "vault"));
    let hive = blocks::state_from_text(block).is_some_and(crate::hive::is_hive);
    if let Some(text) = data.filter(|_| hive) && crate::hive::from_snbt(text).is_none() { return Answer::saying("Не понял данные улья".into()); }
    let brushable = blocks::state_from_text(block).is_some_and(crate::archaeology::suspicious);
    let command_block_or_spawner = command_block || spawner || trial || hive || brushable;
    let items = match data.filter(|_| !command_block_or_spawner).map(crate::containers::items_from_text) {
        Some(Some(items)) => Some(items),
        Some(None) => return Answer::saying(format!("Не понял данные блока: {}", data.unwrap_or(""))),
        None => None,
    };

    if let Some(text) = data.filter(|_| command_block)
        && let Err(why) = crate::command_block::Data::default().apply_snbt(text)
    {
        note_result(0);
        return Answer::saying(why);
    }

    if let Some(text) = data.filter(|_| spawner)
        && let Err(why) = crate::spawner::Data::default().apply_snbt(text)
    {
        note_result(0);
        return Answer::saying(why);
    }

    if let Some(text) = data.filter(|_| trial)
        && let Some(name) = blocks::state_from_text(block).and_then(blocks::block_at_state)
        && let Err(why) = trial_apply(&mut crate::trial::Data::for_block(name), text)
    {
        note_result(0);
        return Answer::saying(why);
    }

    let Some(state) = shared.catalog().state_from_text(block).ok().flatten() else {
        return Answer::saying(format!("Не знаю такого блока: {}", block));
    };

    let [x1, y1, z1, x2, y2, z2] = numbers;
    let (x1, x2) = (x1.min(x2), x1.max(x2));
    let (y1, y2) = (y1.min(y2), y1.max(y2));
    let (z1, z2) = (z1.min(z2), z1.max(z2));

    // Считать в i64: разность крайних i32 в i32 переполняется.
    let volume = (x2 as i64 - x1 as i64 + 1) * (y2 as i64 - y1 as i64 + 1) * (z2 as i64 - z1 as i64 + 1);

    if volume > 32_768 {
        return Answer::saying(format!(
            "Слишком много блоков: {} (можно не больше 32768)",
            volume
        ));
    }

    // Вне высоты мира ставить некуда — и складывать ради этого чанки незачем.
    let (lowest, highest) = (crate::world::MIN_Y, crate::world::MIN_Y + crate::world::WORLD_HEIGHT - 1);

    if y2 < lowest || y1 > highest {
        return Answer::saying(format!("Это вне мира: высота бывает от {} до {}", lowest, highest));
    }

    let (y1, y2) = (y1.max(lowest), y2.min(highest));
    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

    // Как у оригинала: только в загруженных чанках — иначе команда
    // складывала бы мир под замком, и сервер стоял бы.
    for chunk_x in x1 >> 4..=x2 >> 4 {
        for chunk_z in z1 >> 4..=z2 >> 4 {
            if !world.has_chunk(chunk_x, chunk_z) {
                return Answer::saying("Это место не загружено".to_string());
            }
        }
    }

    let mut changed = 0;
    let mut placed = Vec::new();

    for x in x1..=x2 {
        for y in y1..=y2 {
            for z in z1..=z2 {
                // Как у оригинала: команда хранилище сперва опустошает, и
                // заменённый сундук ничего не высыпает.
                world.with_container(x, y, z, |container| container.slots.fill(None));

                if world.get_block(x, y, z) != state {
                    world.note_commanded(x, y, z);
                }

                if world.get_block(x, y, z) != state && world.set_block(x, y, z, state) {
                    changed += 1;
                    placed.push((x, y, z));
                }

                if let Some(text)=data.filter(|_|hive) && let Some(mut data)=crate::hive::from_snbt(text) {
                    // Тик блока-сущности у оригинала начинается со следующего такта
                    // после постановки (чёрный ящик, hive-exit: выход на +21 при 20).
                    for bee in &mut data.bees { bee.ticks-=1; }
                    world.with_hive(x,y,z,|h|*h=data);
                    if !placed.contains(&(x,y,z)) { changed+=1; }
                }

                if crate::archaeology::suspicious(state) && let Some(text)=data && let Some(loot)=crate::archaeology::from_snbt(text) {
                    world.with_archaeology((x,y,z),|d|*d=loot);
                }

                // Спавнер с данными: поля из SNBT; клиентам — данные блока, как
                // у оригинала после постановки (чёрный ящик, spawner-egg).
                if let Some(text) = data.filter(|_| spawner)
                    && world.get_block(x, y, z) == state
                {
                    let applied = world.with_spawner(x, y, z, |spawner| spawner.apply_snbt(text));
                    let nbt = world.spawner(x, y, z).map(crate::spawner::Data::nbt);

                    if let (Some(Ok(())), Some(nbt)) = (applied, nbt) {
                        world.note_block_entity(x, y, z, crate::spawner::BLOCK_ENTITY, nbt);

                        if !placed.contains(&(x, y, z)) {
                            changed += 1;
                        }
                    }
                }

                // Испытательный блок: данные из SNBT; клиентам — данные блока
                // сразу после постановки, даже без SNBT (чёрный ящик,
                // trial-spawner-wave: `{}`, vault-open: `{"shared_data":{}}`).
                if trial && world.get_block(x, y, z) == state {
                    if let Some(text) = data {
                        world.with_trial(x, y, z, |entry| {
                            let mut wrapped = Some(entry.clone());
                            let result = trial_apply(&mut wrapped, text);

                            if let Some(new) = wrapped {
                                *entry = new;
                            }

                            result
                        });
                    }

                    if let Some((kind, nbt)) = world.trial(x, y, z).map(|entry| entry.nbt(crate::trial::ominous_state(state))) {
                        world.note_block_entity(x, y, z, kind, nbt);
                    }
                }

                // Командный блок с данными: поля — из записи, как у
                // оригинала, и блок считается изменённым, даже если стоял
                // такой же.
                if let Some(text) = data.filter(|_| command_block)
                    && crate::command_block::set_from_command(&mut world, (x, y, z), text) == Ok(true)
                    && !placed.contains(&(x, y, z))
                {
                    changed += 1;
                    world.announce_change(x, y, z, state);
                }

                if let Some(items) = &items {
                    let filled = world.with_container(x, y, z, |container| {
                        container.slots.fill(None);

                        for &(slot, stack) in items {
                            if let Some(place) = container.slots.get_mut(slot) {
                                *place = Some(stack);
                            }
                        }
                    });

                    if filled.is_some() {
                        changed += 1;

                        // Тот же блок с новыми данными оригинал ставит
                        // заново: клиентам уходит блок, хоть он и прежний
                        // (чёрный ящик, minecart-hopper: сундук с камнем
                        // поверх такого же сундука).
                        if !placed.contains(&(x, y, z)) {
                            world.announce_change(x, y, z, state);
                        }
                    }

                    // Кафедра: сколько страниц в книге и какая открыта.
                    if let Some(text) = data {
                        world.with_container(x, y, z, |container| {
                            if container.kind == crate::containers::Kind::Lectern {
                                container.pages = crate::redstone::lectern::pages_from_text(text);
                                container.page = crate::redstone::lectern::page_from_text(text)
                                    .clamp(0, (container.pages - 1).max(0));
                            }

                            // Крафтер: выключенные ячейки (`disabled_slots`).
                            if container.kind == crate::containers::Kind::Crafter {
                                container.disabled = crate::redstone::crafter::disabled_from_text(text);
                            }

                            // Печь и стойка: жар из данных, чего нет — ноль.
                            crate::furnace::heat_from_text(container, text);
                            crate::beacon::apply_text(container, text);
                        });
                    }
                }
            }
        }
    }

    world.take_spilled();
    crate::containers::after_change(&mut world);

    for x in x1..=x2 {
        for y in y1..=y2 {
            for z in z1..=z2 {
                crate::placing::refresh_neighbours(&mut world, x, y, z);
            }
        }
    }

    crate::redstone::settle(&mut world);

    // Поставленное командой подстраивается под соседей уже после того, как
    // соседи узнали о нём (сценарий observer-clock: сосед-наблюдатель
    // срабатывает раньше поставленного).
    for place in placed {
        crate::redstone::command_placed(&mut world, place);
        crate::redstone::settle(&mut world);
    }

    world.save_if_due();

    note_result(changed.min(i32::MAX as usize) as i32);

    if area {
        Answer::saying(format!("Заполнено блоков: {}", changed))
    } else {
        crate::commands_feedback::setblock(changed != 0, x1, y1, z1)
    }
}

/// `item replace block <x> <y> <z> container.<N> with <предмет> [сколько]`:
/// кладёт предмет в ячейку хранилища (воздух — опустошает её). Как у
/// оригинала, сравнители рядом узнают о смене содержимого.
fn item(parts: &[&str], source: &Source, shared: &Shared, dimension: Dimension) -> Answer {
    let usage = || Answer::saying("Нужно: item replace block <x> <y> <z> container.<N> with <предмет> [сколько]".to_string());

    // Основная рука игрока: `item replace entity <игрок> weapon.mainhand
    // with <предмет> [сколько]`.
    if let ["replace", "entity", named, "weapon.mainhand", "with", thing, rest @ ..] = parts {
        let count = match rest {
            [] => 1,
            [count] => match count.parse::<i32>() {
                Ok(count) if count > 0 => count,
                _ => return usage(),
            },
            _ => return usage(),
        };
        let stack = match stack_from_spec_for_catalog(shared.catalog(), thing, count) {
            Ok(stack) => stack,
            Err(why) => return Answer::saying(why),
        };
        if let Some(answer) = crate::commands_world::replace_equipment(named, 0, stack, source, shared) { return answer; }
        let targets = find_targets(named, source, shared);

        if targets.is_empty() {
            return Answer::saying("Такого игрока нет на сервере".to_string());
        }

        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, _) in &targets {
            players.order(uuid, Order::Hold { stack });
        }

        return Answer::saying(format!("Предмет в руке заменён — игрокам: {}", names_of(&targets)));
    }

    let ["replace", "block", x, y, z, slot, "with", thing, rest @ ..] = parts else {
        return usage();
    };

    let (Ok(x), Ok(y), Ok(z)) = (x.parse::<i32>(), y.parse::<i32>(), z.parse::<i32>()) else {
        return usage();
    };

    let Some(slot) = slot.strip_prefix("container.").and_then(|n| n.parse::<usize>().ok()) else {
        return usage();
    };

    let count = match rest {
        [] => 1,
        [count] => match count.parse::<i32>() {
            Ok(count) if count > 0 => count,
            _ => return usage(),
        },
        _ => return usage(),
    };

    let name = thing.strip_prefix("minecraft:").unwrap_or(thing);
    let stack = if name == "air" {
        None
    } else {
        // С довесками: `potion[potion_contents={potion:"minecraft:water"}]`.
        match stack_from_spec_for_catalog(shared.catalog(), thing, count) {
            Ok(stack) => stack,
            Err(why) => return Answer::saying(why),
        }
    };

    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let done = world.with_container(x, y, z, |container| match container.slots.get_mut(slot) {
        Some(place) => {
            *place = stack;
            true
        }
        None => false,
    });

    match done {
        Some(true) => {
            crate::containers::after_change(&mut world);
            Answer::saying(format!("Ячейка {} изменена", slot))
        }
        Some(false) => Answer::saying(format!("Нет ячейки {}", slot)),
        None => Answer::saying("Там нет хранилища".to_string()),
    }
}

/// `item replace entity <игрок> hotbar.<N>|inventory.<N>|enderchest.<N>|weapon.mainhand
/// with <предмет> [сколько]`: кладёт предмет в ячейку инвентаря игрока
/// (воздух — опустошает её).
fn item_entity(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: item replace entity <игрок> hotbar.<N> with <предмет> [сколько]".to_string());

    let ["replace", "entity", named, slot, "with", thing, rest @ ..] = parts else {
        return usage();
    };

    let number = |prefix: &str| slot.strip_prefix(prefix).and_then(|n| n.parse::<i32>().ok());
    let ender = number("enderchest.").filter(|n| (0..crate::playerdata::ENDER_SLOTS as i32).contains(n));
    let slot = match (number("hotbar."), number("inventory."), *slot) {
        (Some(n), _, _) if (0..9).contains(&n) => crate::inventory::FIRST_HOTBAR + n,
        (_, Some(n), _) if (0..27).contains(&n) => crate::inventory::FIRST_MAIN + n,
        (_, _, "weapon.offhand") => crate::inventory::OFFHAND,
        // Броня: места окна игрока 5–8 (шлем, нагрудник, штаны, ботинки).
        (_, _, "armor.head") => 5,
        (_, _, "armor.chest") => 6,
        (_, _, "armor.legs") => 7,
        (_, _, "armor.feet") => 8,
        // Эндер-сундук — не окно игрока: номер ячейки свой, 0–26.
        _ if ender.is_some() => -1,
        _ => return usage(),
    };
    // Основная рука — выбранная ячейка панели: её номер знает только
    // подключение, поэтому `weapon.mainhand` пока не разбирается. [проверить]

    let count = match rest {
        [] => 1,
        [count] => match count.parse::<i32>() {
            Ok(count) if count > 0 => count,
            _ => return usage(),
        },
        _ => return usage(),
    };

    let stack = match stack_from_spec_for_catalog(shared.catalog(), thing, count) {
        Ok(stack) => stack,
        Err(why) => return Answer::saying(why),
    };

    if ender.is_none() && let Some(wire) = match slot { 5 => Some(5), 6 => Some(4), 7 => Some(3), 8 => Some(2), crate::inventory::OFFHAND => Some(1), _ => None }
        && let Some(answer) = crate::commands_world::replace_equipment(named, wire, stack, source, shared) { return answer; }
    let targets = find_targets(named, source, shared);

    if targets.is_empty() {
        return Answer::saying("Такого игрока нет на сервере".to_string());
    }

    {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, _) in &targets {
            match ender {
                Some(n) => players.order(uuid, Order::SetEnderSlot { slot: n as usize, stack }),
                None => players.order(uuid, Order::SetSlot { slot, stack }),
            };
        }
    }

    Answer::saying(format!("Ячейка изменена: {}", names_of(&targets)))
}

/// Который час в мире и как его поменять.
///
/// Время идёт тактами, сутки — 24 000 тактов: рассвет 0, полдень 6000,
/// закат 12 000, полночь 18 000.
fn time(parts: &[&str], shared: &Shared) -> Answer {
    // Время суток — у обычного мира: в Незере его нет.
    let mut world = shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком");

    match parts {
        ["query"] | [] => Answer::saying(format!("Время: {}", world.time_of_day())),

        ["set", value] => {
            let Some(time) = time_by_name(value) else {
                return Answer::saying(
                    "Не понял время. Можно: day, noon, night, midnight или число".to_string(),
                );
            };

            world.set_time_of_day(time);
            Answer::saying(format!("Время: {}", time))
        }

        ["add", value] => {
            let Ok(added) = value.parse::<i64>() else {
                return Answer::saying("Сколько добавить? Нужно число".to_string());
            };

            let Some(time) = world.time_of_day().checked_add(added).filter(|time| *time >= 0) else {
                return Answer::saying("Слишком большое число".to_string());
            };
            world.set_time_of_day(time);

            Answer::saying(format!("Время: {}", world.time_of_day()))
        }

        _ => Answer::saying("Как пользоваться: time set <когда>, time add <сколько>, time query".to_string()),
    }
}

/// `weather clear|rain|thunder [срок]` (вики, Commands/weather): погода
/// одна на сервер — у Обычного мира, откуда бы ни звали. Без срока он
/// случаен в промежутке своего вида.
fn weather(parts: &[&str], shared: &Shared) -> Answer {
    use crate::weather::{Kind, parse_duration};

    let usage = || Answer::saying("Как пользоваться: weather clear|rain|thunder [срок]".to_string());

    let (kind, duration) = match parts {
        [kind] => (Kind::from_name(kind), None),
        [kind, duration] => match parse_duration(duration) {
            Some(duration) => (Kind::from_name(kind), Some(duration)),
            None => return Answer::saying("Срок — число тактов, можно с d, s или t".to_string()),
        },
        _ => return usage(),
    };
    let Some(kind) = kind else {
        return usage();
    };

    let mut world = shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком");
    let duration = duration.unwrap_or_else(|| {
        let (low, high) = kind.default_range();

        (low + world.random_bits() % (high - low + 1)) as i32
    });

    world.weather.set(kind, duration);

    Answer::saying(
        match kind {
            Kind::Clear => "Погода: ясно",
            Kind::Rain => "Погода: дождь",
            Kind::Thunder => "Погода: гроза",
        }
        .to_string(),
    )
}

/// Время по названию или числом.
fn time_by_name(value: &str) -> Option<i64> {
    Some(match value {
        "day" => 1_000,
        "noon" => 6_000,
        "night" => 13_000,
        "midnight" => 18_000,
        number => number.parse().ok()?,
    })
}

/// Переносит игрока — себя или другого.
///
/// Разбирается четыре вида записи, как на вики (страница Commands/teleport):
/// `tp <куда>`, `tp <кого> <куда>`, где «куда» — это либо имя игрока, либо
/// три числа.
///
/// Относительных координат (`~`) пока нет: они требуют знать, откуда считать,
/// а это отдельная работа.
fn teleport(parts: &[&str], source: &Source, shared: &Shared, into: Option<Dimension>) -> Answer {
    let own = match source {
        Source::Player { uuid, name } => vec![(*uuid, name.clone())],
        Source::Console | Source::Block { .. } => Vec::new(),
    };

    // Сперва решаем, кого переносим, потом — куда.
    let (targets, place) = match parts {
        // tp <куда-то>, tp <x> <y> <z> или с поворотом взгляда (так пишет
        // F3+C) — переносится сам.
        [_] | [_, _, _] | [_, _, _, _, _] => (own, parts),

        // tp <кого> <куда> или tp <кого> <x> <y> <z>.
        [who, rest @ ..] if !rest.is_empty() => (find_targets(who, source, shared), rest),

        _ => {
            return Answer::saying(
                "Куда переносить? tp <игрок> или tp <x> <y> <z>, можно с именем впереди"
                    .to_string(),
            );
        }
    };

    if targets.is_empty() {
        return Answer::saying("Кого переносить? Из консоли имя игрока обязательно".to_string());
    }

    let Some(Place { x, y, z, dimension: theirs, look }) = place_of(place, source, shared) else {
        return Answer::saying(
            "Не понял, куда переносить: нужно имя игрока или три числа".to_string(),
        );
    };

    {
        let mut players = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком");

        for (uuid, _) in &targets {
            // В другое измерение — переход, в своём — обычный перенос. Место
            // у другого игрока — это и его измерение, как у оригинала.
            let here = players.members().iter().find(|member| &member.uuid == uuid).map(|member| member.dimension);

            match into.or(theirs) {
                Some(dimension) if Some(dimension) != here => {
                    players.order(uuid, Order::ChangeDimension { dimension, x, y, z });
                }
                _ => {
                    players.order(uuid, Order::Teleport { x, y, z });

                    // Оригинал переносит сразу: сущности этого же такта уже
                    // видят игрока на новом месте (minecart-push: вагонетку
                    // толкает в такте команды, у нас без этого — тактом
                    // позже, и рассылки её движения сдвигаются).
                    if let Some(mut position) = players.members().iter().find(|member| &member.uuid == uuid).map(|member| member.position) {
                        (position.x, position.y, position.z) = (x, y, z);
                        players.set_position(uuid, position);

                        // Перенос — не шаг: на крикуна (и руду) под точкой
                        // переноса игрок «встаёт» не сразу (`SETTLE_TICKS`).
                        players.unsettle(uuid);
                    }
                }
            }

            if let Some((yaw, pitch)) = look {
                players.order(uuid, Order::Look { yaw, pitch });
            }
        }
    }

    let own_move = matches!(
        source,
        Source::Player { uuid: caller, .. }
            if targets.len() == 1 && targets[0].0 == *caller
    );

    let reply = if own_move {
        format!("Переношу в {} {} {}", coordinate(x), coordinate(y), coordinate(z))
    } else {
        format!(
            "{} перенесён в {} {} {}",
            targets
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<&str>>()
                .join(", "),
            coordinate(x),
            coordinate(y),
            coordinate(z)
        )
    };

    Answer::saying(reply)
}

/// Координата для ответа: как есть, без лишних нулей — 103.5, а не
/// округлённое 104 (сервер переносит на середину блока).
fn coordinate(value: f64) -> String {
    let text = format!("{:.3}", value);

    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Место из команды `tp`.
struct Place {
    x: f64,
    y: f64,
    z: f64,
    /// Измерение, если место — у другого игрока.
    dimension: Option<Dimension>,
    /// Поворот и наклон взгляда, если названы.
    look: Option<(f32, f32)>,
}

/// Место, названное в команде: имя игрока (его место и измерение) или три
/// числа, можно с поворотом и наклоном взгляда (измерение не названо).
fn place_of(parts: &[&str], source: &Source, shared: &Shared) -> Option<Place> {
    match parts {
        [who] => {
            let (uuid, _) = find_player_named(who, source, shared)?;

            shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .members()
                .iter()
                .find(|member| member.uuid == uuid)
                .map(|member| Place {
                    x: member.position.x,
                    y: member.position.y,
                    z: member.position.z,
                    dimension: Some(member.dimension),
                    look: None,
                })
        }

        // Только конечные числа в пределах мира, как у оригинала: NaN или
        // бесконечность записались бы в файл игрока и ломали каждый вход.
        // Целые X и Z оригинал ставит в середину блока (+0.5): `tp 470 -58
        // 470` приводит в 470.5 (чёрный ящик, tripwire-player, minecart-hit).
        [x, y, z] => {
            let number = |text: &str| text.parse::<f64>().ok().filter(|value| value.is_finite() && value.abs() <= 30_000_000.0);
            let centred = |text: &str| Some(number(text)? + if text.contains('.') { 0.0 } else { 0.5 });

            Some(Place { x: centred(x)?, y: number(y)?, z: centred(z)?, dimension: None, look: None })
        }

        // Поворот — любое конечное число (приводится к −180..180), наклон —
        // от −90 до 90, как у оригинала.
        [x, y, z, yaw, pitch] => {
            let mut place = place_of(&[*x, *y, *z], source, shared)?;
            let yaw = yaw.parse::<f32>().ok().filter(|value| value.is_finite())?;
            let pitch = pitch.parse::<f32>().ok().filter(|value| value.is_finite() && value.abs() <= 90.0)?;

            place.look = Some(((yaw + 180.0).rem_euclid(360.0) - 180.0, pitch));
            Some(place)
        }

        _ => None,
    }
}

/// Находит игроков, которых назвали в команде: по имени или значком выбора.
///
/// Значки с вики (страница Target selectors): `@s` — тот, кто ввёл команду,
/// `@p` — ближайший к нему игрок, `@r` — случайный, `@a` — все, `@e` и `@n` —
/// все существа и ближайшее существо, но существ, кроме игроков, команды
/// у нас пока не трогают, поэтому это те же игроки.
///
/// Уточнений в квадратных скобках (`@a[distance=10]`) пока нет.
pub(crate) fn find_targets(named: &str, source: &Source, shared: &Shared) -> Vec<([u8; 16], String)> {
    // Выбор с условиями (`@a[tag=…,scores={…}]`) — отдельно.
    if named.starts_with('@') && named.contains('[') {
        return select_players(named, source, shared);
    }

    let everyone = || {
        shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .members()
            .iter()
            .map(|member| (member.uuid, member.name.clone(), member.position))
            .collect::<Vec<_>>()
    };

    let named_player = |name: &str| {
        shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .find(name)
            .map(|member| (member.uuid, member.name))
    };

    // Откуда считается «ближайший»: от того, кто ввёл команду. У консоли
    // места нет, поэтому для неё ближайший — первый в списке.
    let from = match source {
        Source::Player { uuid, .. } => everyone()
            .into_iter()
            .find(|(player, _, _)| player == uuid)
            .map(|(_, _, position)| (position.x, position.y, position.z)),
        Source::Console => None,
        Source::Block { .. } => Some(origin_of(source, shared)),
    };

    match named {
        "@s" => match source {
            Source::Player { uuid, name } => vec![(*uuid, name.clone())],
            // Консоль и блок — не игроки: им выбирать некого.
            Source::Console | Source::Block { .. } => Vec::new(),
        },

        "@a" | "@e" => everyone()
            .into_iter()
            .map(|(uuid, name, _)| (uuid, name))
            .collect(),

        "@p" | "@n" => {
            let mut players = everyone();

            // Ближайший к тому, кто ввёл команду. Позже всех зашедший идёт
            // первым при равном расстоянии — поэтому список переворачиваем.
            players.reverse();

            match from {
                Some((x, y, z)) => players
                    .into_iter()
                    .min_by(|(_, _, first), (_, _, second)| {
                        let distance = |place: &crate::players::Position| {
                            (place.x - x).powi(2) + (place.y - y).powi(2) + (place.z - z).powi(2)
                        };

                        distance(first).total_cmp(&distance(second))
                    })
                    .map(|(uuid, name, _)| vec![(uuid, name)])
                    .unwrap_or_default(),
                None => players
                    .into_iter()
                    .next_back()
                    .map(|(uuid, name, _)| vec![(uuid, name)])
                    .unwrap_or_default(),
            }
        }

        "@r" => {
            let players = everyone();

            if players.is_empty() {
                return Vec::new();
            }

            // Своего источника случайных чисел у сервера нет, поэтому
            // считаем от времени: для выбора игрока этого достаточно.
            let moment = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|passed| passed.subsec_nanos() as usize)
                .unwrap_or(0);

            let (uuid, name, _) = players[moment % players.len()].clone();

            vec![(uuid, name)]
        }

        name => named_player(name).into_iter().collect(),
    }
}

/// Выбор игроков с условиями: `@a[…]`, `@p[…]`, `@r[…]`, `@s[…]`, `@e[…]`
/// (из сущностей у нас так выбираются только игроки). Понимаются `tag`,
/// `team`, `scores`, `name`, `distance`, `limit`, `gamemode`, `type=player`
/// (вики, Target selectors); незнакомое условие — никого.
fn select_players(text: &str, source: &Source, shared: &Shared) -> Vec<([u8; 16], String)> {
    let Some((base, inner)) = text.split_once('[') else {
        return Vec::new();
    };
    let Some(inner) = inner.strip_suffix(']') else {
        return Vec::new();
    };
    let mut filter = crate::commands_score::BoardFilter::default();
    let mut names: Vec<(bool, String)> = Vec::new();
    let mut distance: Option<(Option<f64>, Option<f64>)> = None;
    let mut limit: Option<usize> = None;
    let mut modes: Vec<(bool, i32)> = Vec::new();
    // x, y, z — своя точка отсчёта вместо места источника (для distance).
    let mut at: [Option<f64>; 3] = [None; 3];

    for pair in split_selector_arguments(inner) {
        let Some((key, value)) = pair.split_once('=') else {
            return Vec::new();
        };
        match filter.take(key, value) {
            Some(true) => continue,
            None => return Vec::new(),
            Some(false) => {}
        }
        let (negated, bare) = value.strip_prefix('!').map_or((false, value), |rest| (true, rest));
        match key {
            "name" => names.push((!negated, bare.trim_matches('"').to_string())),
            "distance" => {
                let (low, high) = bare.split_once("..").unwrap_or((bare, bare));
                let parse = |part: &str| if part.is_empty() { Some(None) } else { part.parse::<f64>().ok().map(Some) };
                let (Some(low), Some(high)) = (parse(low), parse(high)) else {
                    return Vec::new();
                };
                distance = Some((low, high));
            }
            "x" | "y" | "z" => match value.parse::<f64>() {
                Ok(coordinate) => at[usize::from(key.as_bytes()[0] - b'x')] = Some(coordinate),
                Err(_) => return Vec::new(),
            },
            "limit" => match value.parse() {
                Ok(count) => limit = Some(count),
                Err(_) => return Vec::new(),
            },
            "gamemode" => match game_mode_by_name(bare) {
                Some(mode) => modes.push((!negated, mode)),
                None => return Vec::new(),
            },
            "type" if bare.trim_start_matches("minecraft:") == "player" => {
                if negated {
                    return Vec::new();
                }
            }
            "sort" => {}
            _ => return Vec::new(),
        }
    }

    let source_at = origin_of(source, shared);
    let from = (at[0].unwrap_or(source_at.0), at[1].unwrap_or(source_at.1), at[2].unwrap_or(source_at.2));
    let mut found: Vec<([u8; 16], String, f64)> = {
        let players = shared.players.lock().expect("список игроков захвачен другим потоком");
        players
            .members()
            .iter()
            .map(|member| {
                let place = &member.position;
                let gap = ((place.x - from.0).powi(2) + (place.y - from.1).powi(2) + (place.z - from.2).powi(2)).sqrt();
                (member.uuid, member.name.clone(), gap, member.game_mode)
            })
            .filter(|(uuid, name, gap, mode)| {
                let me = matches!(source, Source::Player { uuid: own, .. } if own == uuid);
                (base != "@s" || me)
                    && names.iter().all(|(wanted, named)| (name == named) == *wanted)
                    && modes.iter().all(|(wanted, wanted_mode)| (mode == wanted_mode) == *wanted)
                    && distance.is_none_or(|(low, high)| low.is_none_or(|low| *gap >= low) && high.is_none_or(|high| *gap <= high))
            })
            .map(|(uuid, name, gap, _)| (uuid, name, gap))
            .collect()
    };

    if !filter.is_empty() {
        let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
        found.retain(|(uuid, name, _)| filter.matches(&board, uuid, name));
    }

    if matches!(base, "@p" | "@n") {
        found.sort_by(|a, b| a.2.total_cmp(&b.2));
        limit = Some(limit.unwrap_or(1));
    }

    if base == "@r" {
        limit = Some(limit.unwrap_or(1));
    }

    found.into_iter().take(limit.unwrap_or(usize::MAX)).map(|(uuid, name, _)| (uuid, name)).collect()
}

/// Условия выбора через запятую — запятые внутри `{…}` не делят.
pub(crate) fn split_selector_arguments(inner: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (at, c) in inner.char_indices() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&inner[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }

    if start < inner.len() {
        out.push(&inner[start..]);
    }

    out
}

/// Находит одного игрока по имени или значку выбора.
///
/// Если под значок подходят несколько, берётся первый: командам вроде
/// «перенести к» нужен один.
fn find_player_named(named: &str, source: &Source, shared: &Shared) -> Option<([u8; 16], String)> {
    find_targets(named, source, shared).into_iter().next()
}

/// Режим игры по названию, как его вводят в команде.
pub(crate) fn game_mode_by_name(name: &str) -> Option<i32> {
    match name {
        "survival" => Some(GAME_MODE_SURVIVAL),
        "creative" => Some(GAME_MODE_CREATIVE),
        "adventure" => Some(GAME_MODE_ADVENTURE),
        "spectator" => Some(GAME_MODE_SPECTATOR),
        _ => None,
    }
}

/// Название режима игры для человека.
fn game_mode_name(mode: i32) -> &'static str {
    match mode {
        GAME_MODE_SURVIVAL => "выживание",
        GAME_MODE_CREATIVE => "творческий",
        GAME_MODE_ADVENTURE => "приключение",
        GAME_MODE_SPECTATOR => "наблюдатель",
        _ => "неизвестный",
    }
}

/// Поля испытательного блока из SNBT команды (trial.rs).
fn trial_apply(entry: &mut Option<crate::trial::Data>, text: &str) -> Result<(), String> {
    match entry {
        Some(crate::trial::Data::Spawner(spawner)) => spawner.apply_snbt(text),
        Some(crate::trial::Data::Vault(vault)) => vault.apply_snbt(text),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_gravity_reads_top_level_snbt_only() {
        for data in ["{NoGravity:1b}","{NoGravity: true}","{\"NoGravity\": 1}"] { assert!(no_gravity(data)); }
        for data in ["{NoGravity:0b}","{foo:{NoGravity:1b}}","{CustomName:'NoGravity:1b'}"] { assert!(!no_gravity(data)); }
    }

    #[test]
    fn item_specs_stay_one_word_and_carry_their_components() {
        assert_eq!(command_arguments("give a iron_sword[enchantments={sharpness:5, knockback:1}] 1").collect::<Vec<_>>(), ["give", "a", "iron_sword[enchantments={sharpness:5, knockback:1}]", "1"]);
        assert_eq!(command_arguments("say [a b] c").collect::<Vec<_>>(), ["say", "[a b]", "c"]);

        let stack = stack_from_spec("minecraft:diamond_sword[damage=3,enchantments={sharpness:2}]", 1).expect("разобралось").expect("не воздух");

        assert_eq!(stack.extras().damage, 3);
        assert_eq!(stack.extras().level("sharpness"), 2);
        assert!(stack_from_spec("nope", 1).is_err());
        assert_eq!(stack_from_spec("air", 1), Ok(None));
    }
    use crate::players::{Member, Position};
    use crate::world::World;
    use std::path::PathBuf;

    /// Движение призываемого огненного шара.
    #[test]
    fn a_fireball_motion_is_read_from_entity_data() {
        // dragon-fireball: {Motion:[0.0d,-0.2d,0.0d],acceleration_power:0.1d}
        assert_eq!(motion_of("{Motion:[0.0d,-0.2d,0.0d],acceleration_power:0.1d}"), Some((0.0, -0.2, 0.0)));
        assert_eq!(motion_of("{Motion:[1,2]}"), None);
        assert_eq!(motion_of(""), None);
    }

    #[test]
    fn skeleton_variant_summon_data_accepts_shearing_and_conversion() {
        for kind in [crate::mob::Kind::Stray, crate::mob::Kind::Bogged] {
            let mut mob = crate::mob::Mob::new(1, kind, (0.0, 64.0, 0.0));
            apply_mob_data(&mut mob, r#"{NoAI:1b,sheared:1b,equipment:{mainhand:{id:"minecraft:bow",count:1}},drop_chances:{mainhand:0.0f}}"#);
            assert!(mob.no_ai && mob.sheared && mob.h.bow);
            assert_eq!(mob.h.hand_drop, 0.0);
        }
        let mut mob = crate::mob::Mob::new(1, crate::mob::Kind::Skeleton, (0.0, 64.0, 0.0));
        apply_mob_data(&mut mob, "{StrayConversionTime:123}");
        assert_eq!(mob.h.converting, 123);
        apply_mob_data(&mut mob, "{StrayConversionTime:-1}");
        assert_eq!(mob.h.converting, 0);
    }

    /// Данные призываемого предмета: вид и число.
    #[test]
    fn summoned_item_data_is_read() {
        let stack = item_from_text(r#"{Item:{id:"minecraft:cobblestone",count:3}}"#).expect("предмет");

        assert_eq!(stack.item, crate::blocks::item_named("cobblestone").expect("булыжник"));
        assert_eq!(stack.count, 3);
        assert_eq!(item_from_text(r#"{Item:{id:"dirt"}}"#).map(|stack| stack.count), Some(1));
        assert!(item_from_text("{}").is_none());
        let named = item_from_text(r#"{Item:{id:"minecraft:stone",count:5,components:{"minecraft:custom_name":"A","minecraft:enchantments":{"minecraft:unbreaking":3}}}}"#).unwrap();
        assert_eq!(named.extras().name.as_deref(), Some("A"));
        assert_eq!(named.extras().level("unbreaking"), 3);
        assert!(!named.stacks_with(&stack));

    }

    /// Свой файл прав на каждую проверку: проверки идут разом и общий файл
    /// делили бы друг с другом.
    fn own_ops_file() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};

        static NEXT: AtomicU32 = AtomicU32::new(0);

        let mut path = std::env::temp_dir();
        path.push(format!(
            "mcsheriffanya-test-ops-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_file(&path);
        path
    }

    /// Сервер с двумя игроками для проверок команд.
    pub(super) fn server() -> Shared {
        server_with_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2))
    }

    fn server_with_catalog(catalog: crate::gameplay_catalog::Catalog) -> Shared {
        let shared = Shared::new(
            World::in_memory_with_catalog(catalog),
            crate::world::World::in_memory_with_catalog(catalog),
            PathBuf::new(),
            PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(&own_ops_file()),
            crate::skins::Settings::default(),
        );

        {
            let mut players = shared.players.lock().expect("список игроков");

            for (number, name) in [(1u8, "Первый"), (2, "Второй")] {
                let member = Member {
                    uuid: [number; 16],
                    name: name.to_string(),
                    game_mode: GAME_MODE_CREATIVE,
                    entity_id: number as i32,
                    skin: None,
                    skin_parts: 0x7F,
                    main_hand: 1,
                    held: None,
                    position: Position {
                        x: number as f64 * 10.0,
                        y: 1.0,
                        z: 0.0,
                        yaw: 0.0,
                        pitch: 0.0,
                        on_ground: true,
                    },
                    dimension: Default::default(),
                    sneaking: false,
                    swings: 0,
                    shoved: ((0.0, 0.0, 0.0), 0),
                    unsettled: 0,
                };

                players.join(member, number as i32).expect("ник свободен");
            }
        }

        // В проверках оба игрока с правами: иначе половина команд им
        // недоступна, а проверяем мы не права, а сами команды. Права
        // проверяются отдельно.
        {
            let mut ops = shared.ops.lock().expect("права");

            ops.give(&[1u8; 16], "Первый", crate::ops::HIGHEST_LEVEL);
            ops.give(&[2u8; 16], "Второй", crate::ops::HIGHEST_LEVEL);
        }

        shared
    }

    fn orders_for(shared: &Shared, uuid: u8) -> Vec<Order> {
        shared
            .players
            .lock()
            .expect("список игроков")
            .take_orders(&[uuid; 16])
    }

    /// Выгнать игрока: поручение уходит именно ему, с причиной.
    #[test]
    fn small_mobs_summon_and_parrot_owner_copies_from_player_uuid() {
        let shared=server();
        for name in ["armadillo","sniffer","parrot","bat","allay"] {
            let reply=run(&format!("summon {name} 1 64 2 {{NoAI:1b}}"),&Source::Console,&shared);
            assert!(reply.reply.contains("Призвана"));
        }
        run("data modify entity @e[type=parrot,limit=1] Owner set from entity Первый UUID",&Source::Console,&shared);
        let items=shared.items_in(Dimension::Overworld).lock().unwrap();
        assert_eq!(items.mobs().len(),5);
        assert_eq!(items.mobs().iter().find(|m| m.kind==crate::mob::Kind::Parrot).unwrap().tame.owner,Some([1;16]));
    }

    #[test]
    fn kick_sends_an_order_with_a_reason() {
        let shared = server();

        let answer = run("kick Второй за дело", &Source::Console, &shared);
        assert!(answer.reply.contains("Второй"));

        let orders = orders_for(&shared, 2);

        assert!(
            matches!(orders.first(), Some(Order::Kick { reason }) if reason == "за дело"),
            "поручение не то: {:?}",
            orders
        );

        // Первому ничего не досталось.
        assert!(orders_for(&shared, 1).is_empty());

        // Незнакомое имя — только ответ, без поручений.
        let answer = run("kick Никто", &Source::Console, &shared);
        assert!(answer.reply.contains("нет на сервере"));
    }

    /// Перенос: и по числам, и к другому игроку, и себе, и другому.
    #[test]
    fn teleport_understands_its_four_forms() {
        let shared = server();
        let first = Source::Player { uuid: [1; 16], name: "Первый".to_string() };

        // Себя по числам: целые X и Z — в середину блока, как у оригинала.
        run("tp 5 64 -7", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 1).first(),
            Some(Order::Teleport { x, y, z }) if *x == 5.5 && *y == 64.0 && *z == -6.5
        ));

        // Себя — к другому игроку: тот стоит в 20 по X.
        run("tp Второй", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 1).first(),
            Some(Order::Teleport { x, .. }) if *x == 20.0
        ));

        // Другого — по числам.
        run("tp Второй 1 2 3", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 2).first(),
            Some(Order::Teleport { x, y, z }) if *x == 1.5 && *y == 2.0 && *z == 3.5
        ));

        // Другого — к третьему. Первый уже там, куда его перенесли (к
        // Второму, в 20): перенос виден сразу, как у оригинала.
        run("tp Второй Первый", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 2).first(),
            Some(Order::Teleport { x, .. }) if *x == 20.0
        ));

        // Из консоли без имени переносить некого.
        let answer = run("tp 1 2 3", &Source::Console, &shared);
        assert!(answer.reply.contains("Кого переносить"));

        // Чепуха вместо места — понятный ответ и никаких поручений.
        let answer = run("tp сюда-вот", &first, &shared);
        assert!(answer.reply.contains("Не понял"));
        assert!(orders_for(&shared, 1).is_empty());
    }

    #[test]
    fn teleport_mob_absolute_rotation_reaches_mob_handler_and_turns_head() {
        let shared = server();
        run("summon cow 1.0 64 2.0 {NoAI:1b,Tags:[\"tp_angle\"]}", &Source::Console, &shared);
        for (command, expected) in [
            ("tp @e[type=cow,tag=tp_angle,limit=1] 3.5 64 0.5 90 10", ((3.5,64.0,0.5),(90.0,10.0,90.0),1)),
            ("teleport @e[type=cow,tag=tp_angle,limit=1] -6.5 64 0.5 180 0", ((-6.5,64.0,0.5),(-180.0,0.0,-180.0),2)),
            ("tp @e[type=cow,tag=tp_angle,limit=1] 3 64 4 450 -30", ((3.5,64.0,4.5),(90.0,-30.0,90.0),3)),
            ("tp @e[type=cow,tag=tp_angle,limit=1] 3.5 64 4.5 -450 30", ((3.5,64.0,4.5),(-90.0,30.0,-90.0),4)),
        ] {
            assert!(run(command, &Source::Console, &shared).reply.contains("Перенесено сущностей: 1"));
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let mob = &items.mobs()[0];
            assert_eq!(((mob.x,mob.y,mob.z),(mob.yaw,mob.pitch,mob.head_yaw),mob.teleports), expected);
        }
    }

    #[test]
    fn teleport_mob_pitch_wraps_before_clamping() {
        // ±270 отличают wrap→clamp от простого clamp; ±360 возвращают ноль.
        let shared = server();
        run("summon cow 1.0 64 2.0 {NoAI:1b}", &Source::Console, &shared);
        for (pitch, expected) in [
            (95,90.0),(-95,-90.0),(450,90.0),(-450,-90.0),
            (270,-90.0),(-270,90.0),(360,0.0),(-360,0.0),
        ] {
            let command = format!("tp @e[type=cow,limit=1] 3.5 64 0.5 0 {pitch}");
            assert!(run(&command, &Source::Console, &shared).reply.contains("Перенесено сущностей: 1"));
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let mob = &items.mobs()[0];
            assert_eq!((mob.yaw,mob.pitch,mob.head_yaw),(0.0,expected,0.0),"pitch={pitch}");
        }
    }

    #[test]
    fn teleport_mob_relative_rotation_uses_executor_and_omission_preserves_look() {
        // Official775: player35/-12 и console0/0; omitted сохраняет rotation моба.
        let shared = server();
        run("summon cow 1.0 64 2.0 {NoAI:1b,Rotation:[80f,20f],Tags:[\"tp_angle\"]}", &Source::Console, &shared);
        let source = Source::Player { uuid: [1;16], name: "Первый".into() };
        {
            let mut players = shared.players.lock().unwrap();
            let mut position = players.members().iter().find(|member| member.uuid==[1;16]).unwrap().position;
            position.yaw = 35.0; position.pitch = -12.0;
            players.set_position(&[1;16], position);
        }
        for (who, tail, expected) in [
            (&source, "~10 ~5", (45.0,-7.0,45.0)),
            (&source, "~ ~", (35.0,-12.0,35.0)),
            (&source, "~ ~120", (35.0,90.0,35.0)),
            (&source, "~ ~-120", (35.0,-90.0,35.0)),
            (&Source::Console, "~10 ~5", (10.0,5.0,10.0)),
            (&Source::Console, "~ ~", (0.0,0.0,0.0)),
        ] {
            let command=format!("tp @e[type=cow,tag=tp_angle] 3.5 64 4.5 {tail}");
            assert!(run(&command, who, &shared).reply.contains("Перенесено сущностей: 1"));
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let mob = &items.mobs()[0];
            assert_eq!((mob.yaw,mob.pitch,mob.head_yaw),expected);
        }
        {
            let mut items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let mob = &mut items.mobs_mut()[0];
            (mob.yaw,mob.pitch,mob.head_yaw)=(70.0,-20.0,35.0);
        }
        run("tp @e[type=cow,tag=tp_angle] ~ ~ ~", &source, &shared);
        let items=shared.items_in(Dimension::Overworld).lock().unwrap();let mob=&items.mobs()[0];
        assert_eq!((mob.yaw,mob.pitch,mob.head_yaw),(70.0,-20.0,35.0));
        assert_eq!((mob.x,mob.y,mob.z),(10.0,1.0,0.0));
    }

    #[test]
    fn malformed_mob_teleport_does_not_change_any_selected_entity() {
        let shared=server();
        for x in [1,2] {
            run(&format!("summon cow {x}.0 64 2.0 {{NoAI:1b,Rotation:[80f,20f],Tags:[\"tp_angle\"]}}"),&Source::Console,&shared);
        }
        let snapshot=|| {
            shared.items_in(Dimension::Overworld).lock().unwrap().mobs().iter().map(|mob|
                (mob.entity_id,(mob.x,mob.y,mob.z),(mob.yaw,mob.pitch,mob.head_yaw),
                 (mob.vx,mob.vy,mob.vz),mob.vitals.fall,mob.on_ground,mob.teleports)).collect::<Vec<_>>()
        };
        let before=snapshot();
        for tail in ["3.5 64 4.5 NaN 0", "3.5 64 4.5 90 Infinity", "3.5 64 4.5 ~bad 0",
            "3.5 64 4.5 90 ~NaN", "bad 64 4.5 90 0", "3.5 64 4.5 90", "3.5 64 4.5 90 0 extra"] {
            let answer=run(&format!("tp @e[type=cow,tag=tp_angle] {tail}"),&Source::Console,&shared);
            assert!(!answer.reply.contains("Перенесено сущностей:"),"{tail}: {}",answer.reply);
            assert_eq!(snapshot(),before,"{tail}");
        }
    }

    #[test]
    fn teleport_preserves_no_ai_ground_and_reapplies_frost_at_destination() {
        for (no_ai, grounded) in [(true, true), (true, false), (false, true), (false, false)] {
            let shared = server();
            {
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                world.ensure(0, 0);
                for x in 6..=14 { for z in -4..=4 {
                    world.set_block_silently(x, 63, z, crate::blocks::state_by_name("water").unwrap());
                } }
            }
            run(&format!("summon zombie 0.5 64 0.5 {{NoAI:{}b,OnGround:{}b,Tags:[\"frost_tp_test\"]}}", u8::from(no_ai), u8::from(grounded)), &Source::Console, &shared);
            run("item replace entity @e[tag=frost_tp_test,limit=1] armor.feet with diamond_boots[enchantments={frost_walker:2}]", &Source::Console, &shared);
            // Заполняем прежнее состояние чар до переноса реальной командой.
            {
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                let mut items = shared.items_in(Dimension::Overworld).lock().unwrap();
                crate::mob::movement_enchantments::step(&mut world, &mut items, &[], &[]);
            }
            run("tp @e[tag=frost_tp_test] 10.5 64 0.5", &Source::Console, &shared);
            let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
            let mut items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let mob = items.mobs().iter().find(|mob| mob.tags.iter().any(|tag| tag == "frost_tp_test")).unwrap();
            assert_eq!((mob.x, mob.y, mob.z), (10.5, 64.0, 0.5));
            assert_eq!(mob.no_ai, no_ai);
            assert_eq!(mob.on_ground, no_ai && grounded);
            crate::mob::movement_enchantments::step(&mut world, &mut items, &[], &[]);
            assert_eq!(crate::blocks::block_at_state(world.get_block(10, 63, 0)),
                Some(if no_ai && grounded { "frosted_ice" } else { "water" }));
        }
    }

    /// Перенос — не шаг: на новом месте игрок не стоит, пока от клиента не
    /// придут пакеты о перемещении (иначе крикун под точкой переноса
    /// вскрикивал бы в такте команды — sculk-shrieker).
    #[test]
    fn teleport_leaves_the_player_in_the_air() {
        let shared = server();
        let first = Source::Player { uuid: [1; 16], name: "Первый".to_string() };

        run("tp 5 64 -7", &first, &shared);

        let players = shared.players.lock().expect("список игроков");
        let member = players.members().iter().find(|member| member.uuid == [1; 16]).expect("игрок");
        assert_eq!((member.position.x, member.position.y, member.position.z), (5.5, 64.0, -6.5));
        assert_eq!(member.unsettled, crate::players::AWAITING_CONFIRM);
    }

    /// Строка из F3+C: числа места и взгляда, в `execute in` — измерение.
    #[test]
    fn teleport_takes_the_look_from_f3_c() {
        let shared = server();
        let first = Source::Player { uuid: [1; 16], name: "Первый".to_string() };

        run("execute in minecraft:the_nether run tp @s 149.30 68.38 -112.30 -8.25 30.30", &first, &shared);

        let orders = orders_for(&shared, 1);
        assert!(matches!(orders.first(), Some(Order::ChangeDimension { dimension: Dimension::Nether, x, .. }) if *x == 149.3));
        assert!(matches!(orders.get(1), Some(Order::Look { yaw, pitch }) if *yaw == -8.25 && *pitch == 30.3));

        // Поворот приводится к −180..180; наклон за 90 — ошибка.
        run("tp 0 64 0 270 0", &first, &shared);
        assert!(matches!(orders_for(&shared, 1).get(1), Some(Order::Look { yaw, .. }) if *yaw == -90.0));
        assert!(run("tp 0 64 0 0 95", &first, &shared).reply.contains("Не понял"));
    }

    /// Перенос к игроку в другом измерении — переход в его измерение.
    #[test]
    fn teleport_to_a_player_follows_their_dimension() {
        let shared = server();
        let first = Source::Player { uuid: [1; 16], name: "Первый".to_string() };
        shared.players.lock().expect("список игроков").set_dimension(&[2; 16], Dimension::Nether);

        run("tp Первый Второй", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 1).first(),
            Some(Order::ChangeDimension { dimension: Dimension::Nether, x, .. }) if *x == 20.0
        ));

        // Второго — к первому: из Незера обратно в обычный мир.
        run("tp Второй @s", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 2).first(),
            Some(Order::ChangeDimension { dimension: Dimension::Overworld, x, .. }) if *x == 10.0
        ));
    }

    /// Значки выбора: @s — сам, @a — все, @p — ближайший, @r — кто-то один.
    #[test]
    fn selectors_choose_the_right_players() {
        let shared = server();
        let first = Source::Player { uuid: [1; 16], name: "Первый".to_string() };

        let names = |named: &str, source: &Source| {
            let mut found: Vec<String> = find_targets(named, source, &shared)
                .into_iter()
                .map(|(_, name)| name)
                .collect();

            found.sort();
            found
        };

        assert_eq!(names("@s", &first), vec!["Первый".to_string()]);
        assert_eq!(names("@a", &first), vec!["Второй".to_string(), "Первый".to_string()]);
        assert_eq!(names("@e", &first), names("@a", &first));

        // Ближайший к Первому — он сам: он стоит в 10 по X, Второй в 20.
        assert_eq!(names("@p", &first), vec!["Первый".to_string()]);

        // Случайный — всегда кто-то один из тех, кто на сервере.
        let random = names("@r", &first);
        assert_eq!(random.len(), 1);
        assert!(names("@a", &first).contains(&random[0]));

        // У консоли «себя» нет.
        assert!(names("@s", &Source::Console).is_empty());

        // Имя работает по-прежнему, а незнакомое не находит никого.
        assert_eq!(names("Второй", &first), vec!["Второй".to_string()]);
        assert!(names("Никто", &first).is_empty());
    }

    /// Команды понимают значки выбора: выгнать всех, перенести всех к себе.
    #[test]
    fn commands_accept_selectors() {
        let shared = server();
        let first = Source::Player { uuid: [1; 16], name: "Первый".to_string() };

        run("kick @a разошлись", &first, &shared);

        for number in [1u8, 2] {
            assert!(
                matches!(orders_for(&shared, number).first(), Some(Order::Kick { .. })),
                "игрок {} не выгнан",
                number
            );
        }

        // Перенос всех к тому, кто ввёл команду: он стоит в 10 по X.
        run("tp @a @s", &first, &shared);

        for number in [1u8, 2] {
            assert!(matches!(
                orders_for(&shared, number).first(),
                Some(Order::Teleport { x, .. }) if *x == 10.0
            ));
        }
    }

    /// Особый динамит выдаёт только тот, кому позволено: без прав — отказ
    /// и никаких поручений, с правами — стопка с пометкой вида.
    #[test]
    fn custom_tnt_is_given_only_by_those_allowed() {
        let shared = server();
        let player = Source::Player { uuid: [2; 16], name: "Второй".to_string() };

        shared.ops.lock().expect("права").take_away(None, "Второй");

        assert!(!crate::permissions::allowed(&shared, &[2; 16], crate::custom_tnt::GIVE_NODE));
        assert!(run("tnt give Второй mega 3", &player, &shared).reply.contains("нет прав"));
        assert!(run("tnt list", &player, &shared).reply.contains("нет прав"));
        assert!(orders_for(&shared, 2).is_empty(), "без прав что-то выдано");
        assert!(!run("help", &player, &shared).reply.contains("tnt"));

        // С правами (у первого они есть) — выдаётся.
        let op = Source::Player { uuid: [1; 16], name: "Первый".to_string() };
        let answer = run("tnt give Второй mega 3", &op, &shared);
        assert!(answer.reply.contains("Мега-ТНТ"), "{}", answer.reply);

        let mega = crate::custom_tnt::by_name("mega").expect("mega").id;
        assert!(matches!(
            orders_for(&shared, 2).first(),
            Some(Order::Give { stack }) if stack.count == 3 && stack.tnt == Some(mega)
        ));

        assert!(run("tnt give Второй бомба", &op, &shared).reply.contains("Нет такого вида"));
        assert!(run("tnt give Второй safe 0", &op, &shared).reply.contains("больше нуля"));
        assert!(run("tnt list", &op, &shared).reply.contains("nuke"));
        assert!(run("tnt list", &Source::Console, &shared).reply.contains("cluster"));
    }

    /// Без прав команды, меняющие мир, не работают, а безобидные — работают.
    #[test]
    fn commands_ask_for_rights() {
        let shared = server();

        {
            let mut ops = shared.ops.lock().expect("права");
            ops.take_away(None, "Первый");
            ops.take_away(None, "Второй");
        }

        let player = Source::Player {
            uuid: [1; 16],
            name: "Первый".to_string(),
        };

        assert!(
            run("kick Второй", &player, &shared).reply.contains("нет прав"),
            "без прав удалось выгнать"
        );
        assert!(run("tp 1 2 3", &player, &shared).reply.contains("нет прав"));
        assert!(run("give Первый stone", &player, &shared).reply.contains("нет прав"));

        // А списком игроков и помощью можно пользоваться всем.
        assert!(!run("list", &player, &shared).reply.contains("нет прав"));
        assert!(!run("help", &player, &shared).reply.contains("нет прав"));

        // Консоль может всё и без записи в файле.
        assert!(!run("kick Первый", &Source::Console, &shared).reply.contains("нет прав"));

        // Выдали права — команды заработали.
        shared
            .ops
            .lock()
            .expect("права")
            .give(&[1; 16], "Первый", crate::ops::HIGHEST_LEVEL);

        assert!(!run("tp 1 2 3", &player, &shared).reply.contains("нет прав"));
    }

    /// Время переводится и спрашивается.
    #[test]
    fn the_clock_can_be_set_and_asked() {
        let shared = server();

        assert!(run("time set night", &Source::Console, &shared).reply.contains("13000"));
        assert!(run("time query", &Source::Console, &shared).reply.contains("13000"));

        run("time add 1000", &Source::Console, &shared);
        assert!(run("time query", &Source::Console, &shared).reply.contains("14000"));

        // Сутки замыкаются: после полуночи снова утро.
        run("time set 23500", &Source::Console, &shared);
        run("time add 1000", &Source::Console, &shared);
        assert!(run("time query", &Source::Console, &shared).reply.contains("500"));
    }

    /// Выдача предметов доходит до игрока поручением.
    #[test]
    fn preview_commands_resolve_slab_through_server_catalog() {
        use crate::gameplay_catalog::{Catalog, Profile, SlabHalf, WHITE_SLAB_ITEM};
        let catalog = Catalog::new(Profile::Preview26_3);
        let shared = server_with_catalog(catalog);
        shared.world_in(Dimension::Overworld).lock().unwrap().ensure(0, 0);
        let answer = run(r#"give Первый minecraft:white_concrete_slab[custom_name="Плита"] 70"#, &Source::Console, &shared);
        assert!(answer.reply.contains("white_concrete_slab"), "{}", answer.reply);
        let orders = orders_for(&shared, 1);
        assert!(matches!(orders.as_slice(), [Order::Give { stack }] if stack.item == WHITE_SLAB_ITEM && stack.count == 70 && stack.extras().name.as_deref() == Some("Плита")));
        let component = answer.component.unwrap();
        assert!(component.windows(b"block.minecraft.white_concrete_slab".len()).any(|bytes| bytes == b"block.minecraft.white_concrete_slab"));
        assert!(component.windows(b"minecraft:white_concrete_slab".len()).any(|bytes| bytes == b"minecraft:white_concrete_slab"));
        let answer = run("setblock 2 65 2 minecraft:white_concrete_slab[type=top,waterlogged=true]", &Source::Console, &shared);
        assert!(answer.reply.contains("Changed the block"), "{}", answer.reply);
        let answer = run("fill 3 65 2 4 65 2 white_concrete_slab[type=double]", &Source::Console, &shared);
        assert!(!answer.reply.contains("Не знаю"), "{}", answer.reply);
        let world = shared.world_in(Dimension::Overworld).lock().unwrap();
        assert_eq!(catalog.slab_properties(world.get_block(2, 65, 2)), Ok(Some((SlabHalf::Top, true))));
        for x in 3..=4 {
            assert_eq!(catalog.slab_properties(world.get_block(x, 65, 2)), Ok(Some((SlabHalf::Double, false))));
        }
    }

    #[test]
    fn replace_item_uses_primary_catalog_for_hands_slots_and_containers() {
        use crate::gameplay_catalog::{Catalog, Profile, slabs};
        use crate::inventory::Stack;
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let shared = server_with_catalog(Catalog::new(profile));
            {
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                world.ensure(0, 0);
                world.set_block(2, 65, 2, blocks::state_by_name("chest").unwrap());
            }
            for row in slabs() {
                let spec = format!(r#"minecraft:{}[custom_name="Цветная плита"]"#, row.name);
                for target in ["weapon.mainhand", "weapon.offhand", "hotbar.2", "enderchest.3"] {
                    let answer = run(&format!("item replace entity Первый {target} with {spec} 2"), &Source::Console, &shared);
                    let orders = orders_for(&shared, 1);
                    if profile == Profile::Java26_1_2 {
                        assert!(answer.reply.contains("Нет такого предмета"));
                        assert!(orders.is_empty());
                    } else {
                        assert_eq!(orders.len(), 1, "{}: {}", row.name, answer.reply);
                        let stack = match &orders[0] {
                            Order::Hold { stack } | Order::SetSlot { stack, .. } | Order::SetEnderSlot { stack, .. } => stack.unwrap(),
                            _ => panic!("неверное поручение замены"),
                        };
                        assert_eq!((stack.item, stack.count), (row.item, 2));
                        assert_eq!(stack.extras().name.as_deref(), Some("Цветная плита"));
                    }
                }
                let stone = Stack::new(blocks::item_named("stone").unwrap(), 7);
                shared.world_in(Dimension::Overworld).lock().unwrap().with_container(2, 65, 2, |container| container.slots[0] = Some(stone));
                let answer = run(&format!("item replace block 2 65 2 container.0 with {spec} 2"), &Source::Console, &shared);
                let world = shared.world_in(Dimension::Overworld).lock().unwrap();
                let stored = world.container(2, 65, 2).unwrap().slots[0].unwrap();
                if profile == Profile::Java26_1_2 {
                    assert!(answer.reply.contains("Нет такого предмета"));
                    assert_eq!(stored, stone);
                } else {
                    assert_eq!((stored.item, stored.count), (row.item, 2), "{}", answer.reply);
                    assert_eq!(stored.extras().name.as_deref(), Some("Цветная плита"));
                }
            }
        }
    }

    #[test]
    fn native_commands_reject_preview_slab_without_orders_or_world_changes() {
        let shared = server();
        shared.world_in(Dimension::Overworld).lock().unwrap().ensure(0, 0);
        let before = shared.world_in(Dimension::Overworld).lock().unwrap().get_block(2, 65, 2);
        let answer = run("give Первый white_concrete_slab 3", &Source::Console, &shared);
        assert!(answer.reply.contains("Нет такого предмета"), "{}", answer.reply);
        assert!(orders_for(&shared, 1).is_empty());
        for command in ["setblock 2 65 2 white_concrete_slab", "fill 2 65 2 3 65 2 white_concrete_slab[type=double]"] {
            let answer = run(command, &Source::Console, &shared);
            assert!(answer.reply.contains("Не знаю такого блока"), "{}", answer.reply);
            assert_eq!(shared.world_in(Dimension::Overworld).lock().unwrap().get_block(2, 65, 2), before);
        }
        let preview = server_with_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3));
        let answer = run("give Первый not_a_real_item", &Source::Console, &preview);
        assert!(answer.reply.contains("Нет такого предмета"));
        assert!(orders_for(&preview, 1).is_empty());
    }

    #[test]
    fn give_sends_the_items_to_the_player() {
        let shared = server();

        let answer = run("give Первый stone 5", &Source::Console, &shared);
        assert!(answer.reply.contains("Gave"), "{}", answer.reply);
        assert!(answer.component.as_ref().unwrap().windows(28).any(|part| part == b"commands.give.success.single"));

        let orders = shared
            .players
            .lock()
            .expect("список игроков")
            .take_orders(&[1; 16]);

        assert!(
            matches!(orders.first(), Some(Order::Give { stack }) if stack.count == 5),
            "поручение не дошло"
        );

        // Несуществующий предмет не выдаётся.
        let answer = run("give Первый такого_нет", &Source::Console, &shared);
        assert!(answer.reply.contains("Нет такого предмета"), "{}", answer.reply);
    }


    #[test]
    fn give_books_preserves_spaces_quotes_and_count() {
        let shared = server();
        for (item, component) in [("writable_book", "writable_book_content"), ("written_book", "written_book_content")] {
            let command = format!(r#"give Первый {item}[{component}={{title:"A story",author:"Some author",pages:["First  page", "Вторая \"страница\" ] {{"]}}] 2"#);
            let answer = run(&command, &Source::Console, &shared);
            assert!(answer.reply.contains("Gave"), "{}", answer.reply);
            let orders = shared.players.lock().unwrap().take_orders(&[1; 16]);
            let Some(Order::Give { stack }) = orders.first() else { panic!("нет выдачи") };
            assert_eq!(stack.count, 2);
            let data = stack.extras();
            let book = data.book.as_ref().unwrap();
            assert_eq!(book.pages, ["First  page", "Вторая \"страница\" ] {"]);
            if item == "written_book" {
                assert_eq!(book.title.as_deref(), Some("A story"));
                assert_eq!(book.author, "Some author");
            }
        }
    }

    #[test]
    fn difficulty_and_keep_inventory_are_runtime_settings() {
        let shared = server();

        run("difficulty hard", &Source::Console, &shared);
        assert_eq!(shared.difficulty.load(std::sync::atomic::Ordering::Relaxed), 3);

        run("difficulty peaceful", &Source::Console, &shared);
        assert_eq!(shared.difficulty.load(std::sync::atomic::Ordering::Relaxed), 0);

        run("gamerule keepInventory true", &Source::Console, &shared);
        assert!(shared.keep_inventory.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn effect_names_are_checked() {
        let shared = server();

        assert!(run("effect give nobody minecraft:poison", &Source::Console, &shared).reply.contains("нет"));
        assert!(run("effect give nobody minecraft:hunger 1 255 true", &Source::Console, &shared).reply.contains("нет на сервере"));
    }

    #[test]
    fn kill_removes_the_chosen_entities_within_reach() {
        let shared = server();
        let stack = crate::inventory::Stack::new(1, 1);

        {
            let mut items = shared.items_in(Dimension::Overworld).lock().expect("предметы");

            items.drop_item(101, stack, (1.0, 0.0, 0.0), (0.0, 0.0, 0.0), crate::items::MINED_DELAY);
            items.drop_item(102, stack, (100.0, 0.0, 0.0), (0.0, 0.0, 0.0), crate::items::MINED_DELAY);
            items.prime(103, 0, (2.0, 0.0, 0.0), (0.0, 0.0, 0.0), 80, None);
        }

        // Консоль стоит в 0 0 0: дальний предмет вне досягаемости.
        let reply = run("kill @e[type=!player,distance=..48]", &Source::Console, &shared).reply;
        assert_eq!(reply, "Убрано сущностей: 2");

        let left: Vec<i32> = shared.items_in(Dimension::Overworld).lock().expect("предметы").items().iter().map(|item| item.entity_id).collect();
        assert_eq!(left, vec![102]);

        // Только динамит — предмет остаётся.
        assert_eq!(run("kill @e[type=tnt]", &Source::Console, &shared).reply, "Убрано сущностей: 0");
        assert_eq!(run("kill @e[type=item]", &Source::Console, &shared).reply, "Убрано сущностей: 1");
    }

    #[test]
    fn setblock_and_fill_change_the_world() {
        let shared = server();

        // Команды меняют только загруженные места.
        shared.world_in(crate::dimension::Dimension::Overworld).lock().expect("мир").ensure(0, 0);

        // Рычагу на полу нужна опора, иначе он тут же отвалится.
        run("setblock 3 4 7 stone", &Source::Console, &shared);
        let reply = run(
            "setblock 3 5 7 minecraft:lever[face=floor,facing=north,powered=true]",
            &Source::Console,
            &shared,
        )
        .reply;
        assert!(reply.contains("Changed the block"), "{}", reply);

        let lever = blocks::state_from_text("lever[face=floor,facing=north,powered=true]")
            .expect("рычаг разбирается");

        {
            let world = shared.world_in(crate::dimension::Dimension::Overworld).lock().expect("мир");
            assert_eq!(world.get_block(3, 5, 7), lever);
        }

        let reply = run("fill 0 1 0 1 1 1 stone", &Source::Console, &shared).reply;
        assert!(reply.contains("4"), "{}", reply);

        let stone = blocks::state_by_name("stone").expect("камень есть");
        let world = shared.world_in(crate::dimension::Dimension::Overworld).lock().expect("мир");
        assert_eq!(world.get_block(1, 1, 1), stone);

        assert!(run("setblock 0 0 0 nonsense_block", &Source::Console, &shared)
            .reply
            .contains("Не знаю"));
        assert!(run("fill 0 0 0 100 100 100 stone", &Source::Console, &shared)
            .reply
            .contains("Слишком много"));
    }

    /// cmdblock-chain.md:33–34: отказ остаётся нулём, но теперь он красный
    /// перевод, а не строка «Там уже такой блок».
    #[test]
    fn setblock_feedback_preserves_success_and_failure() {
        use crate::commands_world::{Arg, fail, tr};
        let shared = server();
        shared.world_in(Dimension::Overworld).lock().unwrap().ensure(0, 0);
        let first = run("setblock 3 5 7 stone", &Source::Console, &shared);
        assert_eq!(take_result(), Some(1));
        assert_eq!(first.result, Some(1));
        assert_eq!(first.component, tr("commands.setblock.success", &[Arg::Int(3), Arg::Int(5), Arg::Int(7)]).component);
        let second = run("setblock 3 5 7 stone", &Source::Console, &shared);
        assert_eq!(take_result(), Some(0));
        assert_eq!(second.result, None);
        assert_eq!(second.component, fail("commands.setblock.failed", &[]).component);
    }

    /// cmdblock-repeat.md:9–42: по одному Disguised Chat каждому игроку,
    /// без дублирующего System Chat и без ответа в LastOutput.
    #[test]
    fn block_say_uses_profileless_chat_for_every_player() {
        let shared = server();
        let source = Source::Block { x: 0, y: 5, z: 0, dimension: Dimension::Overworld, name: "@".to_string() };
        let answer = run("say tick", &source, &shared);
        assert!(answer.reply.is_empty());
        assert_eq!(shared.chat.lock().unwrap().count(), 0);
        for uuid in [1, 2] {
            let orders = orders_for(&shared, uuid);
            assert_eq!(orders.len(), 1);
            assert!(matches!(&orders[0], Order::PacketText { id: 0x21, body, plain }
                if body.starts_with(&crate::network::nbt::text_component("tick")) && plain == "[@] tick"));
        }
    }

    /// Пакет команд при задержке подключения: get/remove должны видеть add
    /// сразу (tmp/bb/ours-cmd-attribute.json, ответы на +42).
    #[test]
    fn attribute_commands_see_pending_edits_and_survive_old_publications() {
        use crate::commands_world::{apply_attribute_edit, finish_attribute_edit, publish_attributes};
        let shared = server();
        let source = Source::Player { uuid: [1; 16], name: "Первый".to_string() };
        let mut actual = crate::attributes::Attributes::player();
        let command = |tail: &str| run(&format!("attribute @s movement_speed {tail}"), &source, &shared);
        publish_attributes(&shared, &[1; 16], &actual);
        command("base set 0.2");
        command("modifier add test:x 0.5 add_multiplied_base");
        assert!(command("get").reply.ends_with("0.30000000000000004"));
        publish_attributes(&shared, &[1; 16], &actual);
        assert!(command("get").reply.ends_with("0.30000000000000004"));
        let duplicate = command("modifier add test:x 0.5 add_multiplied_base");
        assert!(String::from_utf8_lossy(&duplicate.component.unwrap()).contains("modifier_already_present"));
        let removed = command("modifier remove test:x");
        assert!(String::from_utf8_lossy(&removed.component.unwrap()).contains("modifier.remove.success"));
        assert!(command("get").reply.ends_with("0.2"));
        command("base reset");
        let pending = orders_for(&shared, 1);
        assert_eq!(pending.len(), 4);
        for order in pending {
            let Order::Attribute(edit) = order else { panic!("ожидали правку атрибута"); };
            apply_attribute_edit(&mut actual, &edit);
            finish_attribute_edit(&shared, &[1; 16], &actual);
            publish_attributes(&shared, &[1; 16], &actual);
            assert!(command("get").reply.ends_with("0.10000000149011612"));
        }
        assert_eq!(actual, crate::attributes::Attributes::player());
        command("base set 0.8");
        crate::commands_world::forget_attributes(&shared, &[1; 16]);
        publish_attributes(&shared, &[1; 16], &actual);
        assert!(command("get").reply.ends_with("0.10000000149011612"));
    }

    /// minecraft.wiki, Command Block: «executes every game tick» — только
    /// пока блок существует. Уборка cmdblock-repeat должна гасить и уже
    /// извлечённую очередь выполнения, и следующий запланированный такт.
    #[test]
    fn command_block_cleanup_stops_queued_and_scheduled_runs() {
        for (cleanup, auto) in [
            ("fill -1 5 0 2 6 0 air", true), ("fill -1 5 0 2 6 0 air", false),
            ("setblock 0 5 0 stone", true), ("setblock 0 5 0 stone", false),
        ] {
            let shared = server();
            {
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                world.ensure(-1, 0);
                world.ensure(0, 0);
            }
            let command = format!(r#"setblock 0 5 0 repeating_command_block[facing=up]{{auto:{}b,Command:"say tick"}}"#, u8::from(auto));
            run(&command, &Source::Console, &shared);
            run(r#"setblock 0 6 0 chain_command_block[facing=up]{auto:1b,Command:"say chain"}"#, &Source::Console, &shared);
            if !auto { run("setblock -1 5 0 redstone_block", &Source::Console, &shared); }
            let advance = || {
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                for (pos, _) in world.advance(100) {
                    crate::redstone::ticked(&mut world, pos);
                }
                world.take_command_runs()
            };
            crate::command_block::run_queued(&shared, Dimension::Overworld, advance());
            for uuid in [1, 2] {
                let orders = orders_for(&shared, uuid);
                assert_eq!(orders.len(), 2, "repeat и chain работали до уборки");
            }
            let queued = advance();
            assert!(!queued.is_empty());
            take_result();
            run(cleanup, &Source::Console, &shared);
            assert!(take_result().unwrap() > 0);
            {
                let world = shared.world_in(Dimension::Overworld).lock().unwrap();
                assert!(world.command_block(0, 5, 0).is_none());
                if cleanup.starts_with("fill") {
                    assert!(world.command_block(0, 6, 0).is_none());
                }
            }
            crate::command_block::run_queued(&shared, Dimension::Overworld, queued);
            for _ in 0..20 {
                let queued = advance();
                assert!(queued.is_empty(), "удалённый блок снова попал в очередь");
                crate::command_block::run_queued(&shared, Dimension::Overworld, queued);
            }
            for uuid in [1, 2] { assert!(orders_for(&shared, uuid).is_empty()); }
        }
    }

    /// cmdblock-chain.md:9–18: неудачный setblock гасит только условное
    /// звено; ответы удачных звеньев сохраняют вложенные переводы.
    #[test]
    fn command_chain_keeps_results_and_translated_feedback() {
        let shared = server();
        shared.world_in(Dimension::Overworld).lock().unwrap().ensure(0, 0);
        for command in [
            r#"setblock 0 5 0 command_block[facing=east]{auto:1b,Command:"setblock ~ ~ ~2 stone"}"#,
            r#"setblock 1 5 0 chain_command_block[facing=east]{auto:1b,Command:"setblock ~-1 ~ ~2 stone"}"#,
            r#"setblock 2 5 0 chain_command_block[facing=east,conditional=true]{auto:1b,Command:"setblock ~ ~ ~2 glass"}"#,
            r#"setblock 3 5 0 chain_command_block[facing=east]{auto:1b,Command:"setblock ~ ~ ~2 dirt"}"#,
        ] {
            run(command, &Source::Console, &shared);
        }
        let starts = {
            let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
            for (pos, _) in world.advance(100) {
                crate::redstone::ticked(&mut world, pos);
            }
            world.take_command_runs()
        };
        crate::command_block::run_queued(&shared, Dimension::Overworld, starts);
        {
            let world = shared.world_in(Dimension::Overworld).lock().unwrap();
            for (x, success) in [(0, 1), (1, 0), (2, 0), (3, 1)] {
                assert_eq!(crate::command_block::comparator_reading(&world, (x, 5, 0)), Some(success));
            }
            assert_eq!(world.get_block(2, 5, 2), blocks::state_by_name("air").unwrap());
            assert_eq!(world.get_block(3, 5, 2), blocks::state_by_name("dirt").unwrap());
        }
        for uuid in [1, 2] {
            let orders = orders_for(&shared, uuid);
            assert_eq!(orders.len(), 2);
            for (order, x) in orders.iter().zip([0, 3]) {
                let answer = crate::commands_feedback::setblock(true, x, 5, 2);
                let expected = crate::command_block::admin_message("@", &answer.reply, answer.component.as_deref());
                assert!(matches!(order, Order::Message { component } if component == &expected));
            }
        }
    }

    /// Враждебные призываются командой: скелет без данных — с луком, с данными —
    /// только если лук назван; шлем и `ignited` разбираются; `kill` берёт их по виду.
    #[test]
    fn hostile_mobs_are_summoned_with_their_data() {
        let shared = server();

        assert!(run("summon zombie 1 64 2 {NoAI:1b,equipment:{head:{id:\"minecraft:iron_helmet\",count:1}}}", &Source::Console, &shared).reply.contains("zombie"));
        assert!(run("summon skeleton 3 64 2", &Source::Console, &shared).reply.contains("skeleton"));
        assert!(run("summon skeleton 5 64 2 {Silent:1b}", &Source::Console, &shared).reply.contains("skeleton"));
        assert!(run("summon skeleton 7 64 2 {equipment:{mainhand:{id:\"minecraft:bow\",count:1}}}", &Source::Console, &shared).reply.contains("skeleton"));
        assert!(run("summon creeper 9 64 2 {ignited:1b,NoAI:1b}", &Source::Console, &shared).reply.contains("creeper"));
        assert!(run("summon spider 11 64 2", &Source::Console, &shared).reply.contains("spider"));

        {
            let items = shared.items_in(Dimension::Overworld).lock().expect("предметы");
            let mobs = items.mobs();

            assert_eq!(mobs.len(), 6);
            assert!(mobs[0].h.helmet && mobs[0].no_ai && !mobs[1].h.helmet);
            assert!(mobs[1].h.bow, "без данных — с луком");
            assert!(!mobs[2].h.bow, "с данными без лука — без лука");
            assert!(mobs[3].h.bow);
            assert!(mobs[4].h.lit);
            assert_eq!(mobs[5].kind, crate::mob::Kind::Spider);
        }

        assert_eq!(run("kill @e[type=skeleton]", &Source::Console, &shared).reply, "Убрано сущностей: 3");
    }

    #[test]
    fn mob_equipment_commands_preserve_enchants_and_can_clear_slots() {
        let shared = server();
        run("summon zombie 1 64 2 {NoAI:1b,OnGround:1b}", &Source::Console, &shared);
        run("item replace entity @e[type=zombie,limit=1] armor.feet with diamond_boots[enchantments={frost_walker:2}]", &Source::Console, &shared);
        run("item replace entity @e[type=zombie,limit=1] weapon.mainhand with diamond_sword", &Source::Console, &shared);
        run("enchant @e[type=zombie,limit=1] sharpness 3", &Source::Console, &shared);
        {
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let mob = &items.mobs()[0];
            assert!(mob.on_ground);
            assert_eq!(crate::mob::equipment::slot(mob, 2).unwrap().extras().level("frost_walker"), 2);
            assert_eq!(crate::mob::equipment::slot(mob, 0).unwrap().extras().level("sharpness"), 3);
        }
        run("item replace entity @e[type=zombie,limit=1] armor.feet with air", &Source::Console, &shared);
        let items = shared.items_in(Dimension::Overworld).lock().unwrap();
        assert_eq!(crate::mob::equipment::slot(&items.mobs()[0], 2), None);
    }

    #[test]
    fn bee_feed_summons_both_ages_with_no_ai() {
        // Вики, Bee: «flowers»; bee-feed.md:6–16 — прежде всего
        // нужны оба spawn. Проверяем команды из неполной записи без сервера.
        let shared = server();
        for command in [
            "summon bee 103.5 -59 103.5 {NoAI:1b,Silent:1b}",
            "summon bee 102.5 -59 103.5 {NoAI:1b,Silent:1b,Age:-24000}",
        ] {
            assert!(run(command, &Source::Console, &shared).reply.contains("bee"));
        }
        let items = shared.items_in(Dimension::Overworld).lock().unwrap();
        assert_eq!(items.mobs().len(), 2);
        for (mob, age) in items.mobs().iter().zip([0, -24000]) {
            assert_eq!(mob.kind, crate::mob::Kind::Bee);
            assert!(mob.no_ai && mob.silent);
            assert_eq!(mob.age, age);
            assert_eq!(mob.vitals.health, 10.0);
        }
    }

    /// Животные призываются по вики: NoAI, Age, Silent, Sheared, Color,
    /// Health, Rotation — и правятся `data merge`; `tp` и `kill` берут их
    /// по виду и досягаемости.
    #[test]
    fn mobs_are_summoned_edited_moved_and_killed() {
        let shared = server();
        let overworld = Dimension::Overworld;

        assert!(run("summon cow 1 64 2 {NoAI:1b,Silent:1b,Age:-500,Health:6}", &Source::Console, &shared).reply.contains("cow"));
        assert!(run("summon sheep 3 64 2 {Sheared:1b,Color:11b,Rotation:[90f,10f]}", &Source::Console, &shared).reply.contains("sheep"));
        assert!(run("summon chicken 5 64 2", &Source::Console, &shared).reply.contains("chicken"));
        assert!(run("summon pig 7 64 2 {NoAI:true}", &Source::Console, &shared).reply.contains("pig"));

        {
            let items = shared.items_in(overworld).lock().expect("предметы");
            let mobs = items.mobs();

            assert_eq!(mobs.len(), 4);
            assert_eq!((mobs[0].kind, mobs[0].no_ai, mobs[0].silent, mobs[0].age), (crate::mob::Kind::Cow, true, true, -500));
            assert_eq!(mobs[0].vitals.health, 6.0);
            assert_eq!((mobs[0].x, mobs[0].y, mobs[0].z), (1.5, 64.0, 2.5), "целые x и z — середина блока");
            assert_eq!((mobs[1].sheared, mobs[1].color, mobs[1].yaw, mobs[1].pitch), (true, 11, 90.0, 10.0));
            assert!(!mobs[1].no_ai);
            assert!(mobs[2].egg_timer >= 6000, "курице назначен срок яйца");
            assert!(mobs[3].no_ai, "NoAI:true тоже разбирается");
        }

        // Правка: NoAI выключен только у одного, счёт правок растёт.
        let reply = run("data merge entity @e[type=cow,limit=1] {NoAI:0b}", &Source::Console, &shared).reply;

        assert_eq!(reply, "Изменено сущностей: 1");

        {
            let items = shared.items_in(overworld).lock().expect("предметы");

            assert!(!items.mobs()[0].no_ai);
            assert_eq!(items.mobs()[0].attr_changes, 1);
            assert!(items.mobs()[3].no_ai);
        }

        // Перенос: скорость и падение сброшены, клиентам нужен перенос целиком.
        assert_eq!(run("tp @e[type=pig] 9 70 9", &Source::Console, &shared).reply, "Перенесено сущностей: 1");

        {
            let items = shared.items_in(overworld).lock().expect("предметы");

            assert_eq!((items.mobs()[3].x, items.mobs()[3].y, items.mobs()[3].z), (9.5, 70.0, 9.5));
            assert_eq!(items.mobs()[3].teleports, 1);
        }

        // Убить: погибают, а не пропадают; добыча падает предметами.
        assert_eq!(run("kill @e[type=sheep]", &Source::Console, &shared).reply, "Убрано сущностей: 1");

        {
            let items = shared.items_in(overworld).lock().expect("предметы");

            assert!(items.mobs()[1].is_dead(), "овца падает набок, а не исчезает");
            assert!(items.items().iter().any(|item| blocks::item_name(item.stack.item) == Some("mutton")), "выпала добыча овцы (остриженная — без шерсти)");
        }

        // Неведомый вид и нехватка чисел — понятный ответ.
        assert!(run("summon cow 1 2", &Source::Console, &shared).reply.contains("Нужно"));
        assert!(run("summon unsupported_mob 1 2 3", &Source::Console, &shared).reply.contains("Пока"));
    }

    /// Правило `spawn_mobs` (26.1) выключает появление в новых чанках.
    #[test]
    fn spawn_mobs_rule_is_a_runtime_setting() {
        let shared = server();

        assert!(shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed));
        run("gamerule spawn_mobs false", &Source::Console, &shared);
        assert!(!shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed));
        run("gamerule spawn_mobs true", &Source::Console, &shared);
        assert!(shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed));
    }

    /// Предмет для `summon item` берётся из поля Item; прочее пропускается.
    #[test]
    fn a_summoned_item_is_read_from_its_data() {
        let stack = item_from_text(r#"{Item:{id:"minecraft:stone",count:64},Motion:[0.0,0.0,0.0]}"#).expect("разобрано");

        assert_eq!(stack.item, blocks::item_named("stone").unwrap());
        assert_eq!(stack.count, 64);
        assert!(item_from_text("{Motion:[0.0,0.0,0.0]}").is_none());
    }
    #[test]
    fn slime_summon_size_is_zero_based_and_health_uses_the_size() {
        for (nbt, size) in [(0, 1), (1, 2), (3, 4), (-1, 1), (999, 127)] {
            let mut mob = crate::mob::Mob::new(1, crate::mob::Kind::Slime, (0.0, 0.0, 0.0));
            apply_mob_data(&mut mob, &format!("{{Size: {nbt}, NoAI: true, Silent: 1b, PersistenceRequired: 1b}}"));
            assert_eq!(mob.nether.size, size);
            assert_eq!(mob.vitals.max_health, (size * size) as f32);
            assert!(mob.no_ai && mob.silent && mob.nether.persistence_required);
        }
        let mut mob = crate::mob::Mob::new(1, crate::mob::Kind::Slime, (0.0, 0.0, 0.0));
        apply_mob_data(&mut mob, "{Size:3,Health:12.5f,Age:-24000,IsBaby:1b,PersistenceRequired:1b}");
        assert_eq!(mob.vitals.health, 12.5);
        assert!(!mob.is_baby());
        apply_mob_data(&mut mob, "{Silent:1b}");
        assert!(mob.nether.persistence_required, "частичная правка не сбрасывает PersistenceRequired");
    }

}
