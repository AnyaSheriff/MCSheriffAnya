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
}

impl Answer {
    /// Ответ, после которого ничего делать не надо.
    fn saying(reply: String) -> Self {
        Self {
            reply,
            effect: Effect::None,
        }
    }
}

/// Разбирает и выполняет команду, введённую без ведущей косой черты.
/// Какой уровень прав нужен команде.
///
/// Ноль — можно всем. Всё, что меняет мир или задевает других игроков,
/// требует прав; их выдают командой `op`, а хранятся они в `ops.json`.
fn level_needed(command: &str) -> i32 {
    match command {
        "help" | "list" | "say" => 0,
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

/// Убирает пробелы внутри квадратных скобок вне кавычек: запись предмета с
/// довесками (`iron_sword[enchantments={sharpness:5, knockback:1}]`) должна
/// остаться одним словом команды.
fn compact_item_specs(command: &str) -> String {
    let mut out = String::with_capacity(command.len());
    let mut depth = 0;
    let mut quote: Option<char> = None;

    for c in command.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') if depth > 0 => quote = Some(c),
            (None, '[') => depth += 1,
            (None, ']') if depth > 0 => depth -= 1,
            (None, c) if depth > 0 && c.is_whitespace() => continue,
            _ => {}
        }

        out.push(c);
    }

    out
}

/// Стопка по записи предмета команды (`diamond_sword[damage=5]`): None —
/// воздух.
fn stack_from_spec(spec: &str, count: i32) -> Result<Option<crate::inventory::Stack>, String> {
    let (name, data) = crate::stackdata::parse_item_spec(spec).map_err(|why| format!("Не разобрать предмет {spec}: {why}"))?;

    if name == "air" {
        return Ok(None);
    }

    let number = blocks::item_named(&name).ok_or_else(|| format!("Нет такого предмета: {}", spec.split('[').next().unwrap_or(spec)))?;

    Ok(Some(crate::inventory::Stack::new(number, count.min(blocks::stack_size(number))).with_extras(data)))
}

pub fn run(command: &str, source: &Source, shared: &Shared) -> Answer {
    let compact = compact_item_specs(command);
    let command = compact.as_str();
    let mut parts = command.split_whitespace();
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
                (None, Source::Console) => Vec::new(),
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

            Answer { reply, effect }
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

            let stack = match stack_from_spec(item_name, count) {
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

            Answer::saying(format!(
                "Выдано: {} × {} — игрокам: {}",
                item_name,
                count,
                names_of(&targets)
            ))
        }
        "clear" => {
            let named = parts.next();

            let targets = match (named, source) {
                (Some(named), _) => find_targets(named, source, shared),
                (None, Source::Player { uuid, name }) => vec![(*uuid, name.clone())],
                (None, Source::Console) => Vec::new(),
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
        "setblock" => set_blocks(parts.collect::<Vec<&str>>().as_slice(), shared, dimension_of(source, shared), false),
        "fill" => set_blocks(parts.collect::<Vec<&str>>().as_slice(), shared, dimension_of(source, shared), true),
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

            // Сообщение уходит в общую ленту, поэтому в ответе его повторять
            // не надо: сказавший увидит его там же, где все остальные.
            let who = match source {
                Source::Player { name, .. } => name.clone(),
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

            // Сущности, кроме игроков, — только вагонетки: `tp @e[type=minecart] x y z`.
            match rest.as_slice() {
                [who, place @ ..] if cart_selector(who).is_some() => teleport_carts(who, place, source, shared),
                [who, place @ ..] if who.starts_with("@e[") && place.len() == 3 && !who.contains("type=!player") => teleport_mobs(who, place, source, shared),
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
        "experience" | "xp" => experience(parts.collect::<Vec<&str>>().as_slice(), source, shared),
        "gamerule" => gamerule(parts.collect::<Vec<&str>>().as_slice(), shared),

        // Как у оригинала: `execute in <измерение> run tp …` — перенос в
        // другое измерение. Другие виды execute пока не разбираются.
        "execute" => {
            let rest: Vec<&str> = parts.collect();

            match rest.as_slice() {
                ["in", dimension, "run", "tp" | "teleport", tail @ ..] => match Dimension::from_name(dimension) {
                    Some(dimension) => teleport(tail, source, shared, Some(dimension)),
                    None => Answer::saying(format!("Нет такого измерения: {}", dimension)),
                },
                _ => Answer::saying("Пока понимаю только: execute in <измерение> run tp <кого> <x> <y> <z>".to_string()),
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
            },
            Source::Player { .. } => Answer::saying(
                "Остановить сервер можно только из консоли сервера".to_string(),
            ),
        },
        unknown => Answer::saying(format!("Неизвестная команда: {}", unknown)),
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
fn origin_of(source: &Source, shared: &Shared) -> (f64, f64, f64) {
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
    }
}

/// Три координаты места для сущности: числа или `~` со сдвигом от `origin`.
/// Как у оригинала, целое число по x и z означает середину блока (вики,
/// Argument types, vec3: «an integer … is centered»). [проверить]: у
/// оригинала целые по x и z сдвигаются на 0,5.
fn entity_place(parts: &[&str], origin: (f64, f64, f64)) -> Option<(f64, f64, f64)> {
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

    if name.strip_prefix("minecraft:").unwrap_or(name) == "item" {
        return summon_item(rest, source, shared);
    }

    if name.strip_prefix("minecraft:").unwrap_or(name) == "experience_orb" {
        return summon_orb(rest, source, shared);
    }

    if name.strip_prefix("minecraft:").unwrap_or(name) == "lightning_bolt" {
        return summon_lightning(rest, source, shared);
    }

    if let glow @ ("item_frame" | "glow_item_frame") = name.strip_prefix("minecraft:").unwrap_or(name) {
        return summon_frame(glow == "glow_item_frame", rest, source, shared);
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

    let items = match data.map(crate::containers::items_from_text) {
        Some(Some(items)) => items,
        Some(None) => return Answer::saying(format!("Не понял данные сущности: {}", data.unwrap_or(""))),
        None => Vec::new(),
    };

    let mut cart = crate::minecart::Minecart::new(shared.next_entity_id(), kind, place);

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
fn summon_frame(glow: bool, rest: &[&str], source: &Source, shared: &Shared) -> Answer {
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

    frame.item = item_from_text(data).map(|stack| stack.with_count(1));
    frame.rotation = number_after("ItemRotation:").unwrap_or(0) & 7;

    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    world.store_frame(&frame);
    items.add_frame(frame);
    crate::frame::refresh_marks(&mut world, &items);

    Answer::saying(format!("Призвана сущность {}", if glow { "glow_item_frame" } else { "item_frame" }))
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
    if let Some(on) = flag_in(data, "NoAI:") {
        mob.no_ai = on;
    }

    if let Some(on) = flag_in(data, "Silent:") {
        mob.silent = on;
    }

    if let Some(on) = flag_in(data, "Sheared:") {
        mob.sheared = on;
    }

    if let Some(age) = int_after(data, "Age:") {
        mob.age = age;
    }

    if let Some(color) = int_after(data, "Color:") {
        mob.color = color.clamp(0, 15) as u8;
    }

    // Призванный с данными скелет без лука в руке — с луком он только если он
    // назван в данных (`equipment:{mainhand:{id:"minecraft:bow"}}`).
    if mob.kind == crate::mob::Kind::Skeleton && !data.is_empty() {
        mob.h.bow = data.contains("bow");
    }

    if let Some(on) = flag_in(data, "ignited:") {
        mob.h.lit = on;
    }

    // Шлем: `ArmorItems` (четвёртый элемент, голова) или `equipment:{head:…}` (с
    // 1.21.5): любой «…_helmet».
    if let Some(at) = data.find("ArmorItems:").or_else(|| data.find("equipment:")) {
        mob.h.helmet = data[at..].contains("_helmet") || data[at..].contains("_head");
    }

    if let Some(health) = int_after(data, "Health:") {
        mob.vitals.health = (health as f32).clamp(0.5, mob.kind.max_health());
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
}

/// `summon cow|pig|sheep|chicken <x> <y> <z> {NoAI:1b,Age:-24000,…}` — животное
/// ровно в названной точке. У призванного детёныша `Age` отрицательный
/// (вики, Breeding: «Age: negative for baby»).
fn summon_mob(kind: crate::mob::Kind, rest: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: summon <cow|pig|sheep|chicken|zombie|skeleton|creeper|spider|end_crystal|ender_dragon> <x> <y> <z> {NoAI:1b,Age:-24000}".to_string());
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

    apply_mob_data(&mut mob, data);

    if kind == crate::mob::Kind::Chicken {
        mob.egg_timer = 6000 + (mob.entity_id as u32 % 6001) as i32;
    }

    shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком").add_mob(mob);

    Answer::saying(format!("Призвана сущность {}", kind.name()))
}

/// `data merge entity <кто> {NoAI:0b,…}` — правка данных мобов.
fn data_command(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let ["merge", "entity", who, data] = parts else {
        return Answer::saying("Нужно: data merge entity @e[type=cow,limit=1] {NoAI:0b}".to_string());
    };

    let Some(chosen) = Selector::parse(who) else {
        return Answer::saying("Не понял, кого править".to_string());
    };

    let (x, y, z) = origin_of(source, shared);
    let mut left = chosen.limit.unwrap_or(usize::MAX);
    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
    let mut changed = 0;

    for mob in items.mobs_mut() {
        if left > 0 && chosen.takes(mob.kind.name()) && chosen.reaches((x, y, z), (mob.x, mob.y, mob.z)) {
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

/// `tp @e[type=cow,…] <x> <y> <z>` — перенос мобов: скорость обнуляется.
fn teleport_mobs(who: &str, place: &[&str], source: &Source, shared: &Shared) -> Answer {
    let Some(chosen) = Selector::parse(who) else {
        return Answer::saying("Не понял, кого переносить".to_string());
    };

    let origin = origin_of(source, shared);
    let Some((x, y, z)) = entity_place(place, origin) else {
        return Answer::saying("Нужно: tp @e[type=cow] <x> <y> <z>".to_string());
    };

    let mut left = chosen.limit.unwrap_or(usize::MAX);
    let mut items = shared.items_in(dimension_of(source, shared)).lock().expect("предметы захвачены другим потоком");
    let mut moved = 0;

    for mob in items.mobs_mut() {
        if left > 0 && chosen.takes(mob.kind.name()) && chosen.reaches(origin, (mob.x, mob.y, mob.z)) {
            (mob.x, mob.y, mob.z) = (x, y, z);
            (mob.vx, mob.vy, mob.vz) = (0.0, 0.0, 0.0);
            mob.on_ground = false;
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
    text.contains("NoGravity:1b") || text.contains("NoGravity:true") || text.contains("NoGravity:1}") || text.contains("NoGravity:1,")
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

/// Предмет из данных сущности: `{Item:{id:"minecraft:stone",count:5}}`.
/// Прочие поля (Motion, PickupDelay) пропускаются.
fn item_from_text(text: &str) -> Option<crate::inventory::Stack> {
    let inner = &text[text.find("Item:{")? + "Item:{".len()..];
    let inner = &inner[..inner.find('}')?];
    let (mut item, mut count) = (None, 1);

    for field in inner.split(',') {
        let (key, value) = field.split_once(':')?;
        let value = value.trim().trim_matches('"');

        match key.trim() {
            "id" => item = blocks::item_named(value.strip_prefix("minecraft:").unwrap_or(value)),
            "count" | "Count" => count = value.trim_end_matches('b').parse().ok()?,
            _ => {}
        }
    }

    Some(crate::inventory::Stack::new(item?, count))
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
    // `kill <игрок>` / `kill @s` — убить игрока (вид урона generic_kill).
    if let Some(who) = parts.first().filter(|who| !who.starts_with("@e")) {
        let targets = if *who == "@s" {
            match source {
                Source::Player { uuid, name } => vec![(*uuid, name.clone())],
                Source::Console => Vec::new(),
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

    let (x, y, z) = origin_of(source, shared);
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
        if left > 0 && chosen.takes(mob.kind.name()) && near((mob.x, mob.y, mob.z)) && !mob.is_dead() {
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

/// `damage @e[…] <сколько> [вид]` — урон мобам (пока только обычный,
/// `minecraft:generic`); игрокам — как `Hurt`.
fn damage(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let (Some(who), Some(amount)) = (parts.first(), parts.get(1).and_then(|amount| amount.parse::<f32>().ok())) else {
        return Answer::saying("Нужно: damage <кто> <сколько> [minecraft:generic|minecraft:explosion|minecraft:player_attack]".to_string());
    };
    let kind = match parts.get(2).map(|kind| kind.trim_start_matches("minecraft:")) {
        Some("explosion") => crate::survival::Source::Explosion,
        Some("player_attack") => crate::survival::Source::PlayerAttack,
        _ => crate::survival::Source::Generic,
    };

    if !who.starts_with("@e") {
        let targets = find_targets(who, source, shared);
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, _) in &targets {
            players.order(uuid, Order::Hurt { amount, source: kind });
        }

        return Answer::saying(format!("Нанесён урон: {}", names_of(&targets)));
    }

    let Some(chosen) = Selector::parse(who) else {
        return Answer::saying("Не понял, кому наносить урон".to_string());
    };
    let origin = origin_of(source, shared);
    let dimension = dimension_of(source, shared);
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let mut left = chosen.limit.unwrap_or(usize::MAX);
    let mut hurt = 0;

    for mob in items.mobs_mut() {
        // Дракону команда нипочём: урон он берёт только частями — от игрока
        // и взрыва (чёрный ящик, dragon-probe: ни generic, ни player_attack,
        // ни explosion, ни out_of_world его не берут).
        if mob.kind == crate::mob::Kind::EnderDragon {
            continue;
        }

        if left > 0 && chosen.takes(mob.kind.name()) && chosen.reaches(origin, (mob.x, mob.y, mob.z)) && !mob.is_dead() {
            mob.hurt(amount, kind, None);
            left -= 1;
            hurt += 1;
        }
    }

    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));

    Answer::saying(format!("Нанесён урон сущностям: {hurt}"))
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

/// `effect give <игрок> <эффект> [секунды] [уровень] [скрыть]` и
/// `effect clear <игрок>`. Считаются голод, насыщение, сила, слабость, скорость, замедление и сопротивление — остальным вредно
/// не быть [проверить].
fn effect(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: effect give <игрок> <эффект> [секунды] [уровень] | effect clear <игрок>".to_string());

    match parts {
        ["give", who, name, rest @ ..] => {
            let Some(kind) = crate::survival::EffectKind::from_name(name) else {
                return Answer::saying(format!("Такого эффекта пока нет: {}", name));
            };
            let targets = find_targets(who, source, shared);

            if targets.is_empty() {
                return Answer::saying("Такого игрока нет на сервере".to_string());
            }

            let seconds: i32 = rest.first().and_then(|value| value.parse().ok()).unwrap_or(30);
            let amplifier: i32 = rest.get(1).and_then(|value| value.parse().ok()).unwrap_or(0);
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for (uuid, _) in &targets {
                players.order(uuid, Order::Effect { kind, amplifier: amplifier.clamp(0, 255), ticks: seconds.max(1) * 20 });
            }

            Answer::saying(format!("Эффект {} наложен: {}", name, names_of(&targets)))
        }
        ["clear", who, name] => {
            let Some(kind) = crate::survival::EffectKind::from_name(name) else {
                return Answer::saying(format!("Такого эффекта пока нет: {}", name));
            };
            let targets = find_targets(who, source, shared);
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for (uuid, _) in &targets {
                players.order(uuid, Order::ClearEffect(kind));
            }

            Answer::saying(format!("Эффект {} снят: {}", name, names_of(&targets)))
        }
        ["clear", who] => {
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
    match parts {
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
        ["natural_health_regeneration" | "naturalRegeneration", value] => {
            shared.natural_regeneration.store(*value == "true", std::sync::atomic::Ordering::Relaxed);

            Answer::saying(format!("Правило natural_health_regeneration: {}", value))
        }
        [rule, value] => Answer::saying(format!("Правило {}: {}", rule, value)),
        _ => Answer::saying("Нужно: gamerule <правило> <значение>".to_string()),
    }
}

/// Выбор сущностей `@e[…]` для `kill`.
struct Selector {
    /// Вид: `Some((true, "item"))` — только предметы, `Some((false, "player"))`
    /// — все, кроме игроков; `None` — любые.
    kind: Option<(bool, String)>,

    /// Не дальше скольких блоков от того, кто ввёл команду.
    distance: Option<f64>,

    /// Сколько самое большее.
    limit: Option<usize>,
}

impl Selector {
    fn parse(text: &str) -> Option<Selector> {
        let mut chosen = Selector { kind: None, distance: None, limit: text.starts_with("@n").then_some(1) };

        let inner = match text {
            "@e" | "@n" => return Some(chosen),
            _ => text.strip_prefix("@e[").or_else(|| text.strip_prefix("@n["))?.strip_suffix(']')?,
        };

        for pair in inner.split(',') {
            match pair.split_once('=')? {
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
                _ => return None,
            }
        }

        Some(chosen)
    }

    /// Подходит ли сущность такого вида.
    fn takes(&self, kind: &str) -> bool {
        match &self.kind {
            Some((wanted, name)) => (name == kind) == *wanted,
            None => true,
        }
    }

    /// Достаточно ли близко сущность.
    fn reaches(&self, (x, y, z): (f64, f64, f64), (at_x, at_y, at_z): (f64, f64, f64)) -> bool {
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
fn names_of(targets: &[([u8; 16], String)]) -> String {
    targets
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// В каком измерении тот, кто ввёл команду: игрок — в своём, консоль — в
/// обычном мире.
fn dimension_of(source: &Source, shared: &Shared) -> Dimension {
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
    }
}

/// Ставит блок в одно место (`setblock`) или заливает им область (`fill`).
///
/// Блок записывается как в игре: `minecraft:lever[face=floor,powered=true]`.
/// Как и у обычного сервера, соседи узнают о новом блоке, а редстоун
/// пересчитывается; своей логики нажатия у поставленного рычага нет.
/// Область у `fill` ограничена, как в игре, 32 768 блоками.
fn set_blocks(parts: &[&str], shared: &Shared, dimension: Dimension, area: bool) -> Answer {
    let (corners, block) = match (area, parts) {
        (false, [x, y, z, block]) => ([*x, *y, *z, *x, *y, *z], *block),
        (true, [x1, y1, z1, x2, y2, z2, block]) => ([*x1, *y1, *z1, *x2, *y2, *z2], *block),
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

    for (slot, text) in numbers.iter_mut().zip(corners) {
        let Ok(value) = text.parse::<i32>() else {
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
    let items = match data.map(crate::containers::items_from_text) {
        Some(Some(items)) => Some(items),
        Some(None) => return Answer::saying(format!("Не понял данные блока: {}", data.unwrap_or(""))),
        None => None,
    };

    let Some(state) = blocks::state_from_text(block) else {
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

    if area {
        Answer::saying(format!("Заполнено блоков: {}", changed))
    } else if changed == 0 {
        Answer::saying("Там уже такой блок".to_string())
    } else {
        Answer::saying(format!("Блок в {}, {}, {} изменён", x1, y1, z1))
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
        let stack = match stack_from_spec(thing, count) {
            Ok(stack) => stack,
            Err(why) => return Answer::saying(why),
        };
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
        let Some(number) = blocks::item_named(name) else {
            return Answer::saying(format!("Не знаю такого предмета: {}", thing));
        };

        Some(crate::inventory::Stack::new(number, count.min(blocks::stack_size(number))))
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

/// `item replace entity <игрок> hotbar.<N>|inventory.<N>|weapon.mainhand
/// with <предмет> [сколько]`: кладёт предмет в ячейку инвентаря игрока
/// (воздух — опустошает её).
fn item_entity(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let usage = || Answer::saying("Нужно: item replace entity <игрок> hotbar.<N> with <предмет> [сколько]".to_string());

    let ["replace", "entity", named, slot, "with", thing, rest @ ..] = parts else {
        return usage();
    };

    let number = |prefix: &str| slot.strip_prefix(prefix).and_then(|n| n.parse::<i32>().ok());
    let slot = match (number("hotbar."), number("inventory."), *slot) {
        (Some(n), _, _) if (0..9).contains(&n) => crate::inventory::FIRST_HOTBAR + n,
        (_, Some(n), _) if (0..27).contains(&n) => crate::inventory::FIRST_MAIN + n,
        (_, _, "weapon.offhand") => crate::inventory::OFFHAND,
        // Броня: места окна игрока 5–8 (шлем, нагрудник, штаны, ботинки).
        (_, _, "armor.head") => 5,
        (_, _, "armor.chest") => 6,
        (_, _, "armor.legs") => 7,
        (_, _, "armor.feet") => 8,
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

    let stack = match stack_from_spec(thing, count) {
        Ok(stack) => stack,
        Err(why) => return Answer::saying(why),
    };

    let targets = find_targets(named, source, shared);

    if targets.is_empty() {
        return Answer::saying("Такого игрока нет на сервере".to_string());
    }

    {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, _) in &targets {
            players.order(uuid, Order::SetSlot { slot, stack });
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
        Source::Console => Vec::new(),
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
fn find_targets(named: &str, source: &Source, shared: &Shared) -> Vec<([u8; 16], String)> {
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
    };

    match named {
        "@s" => match source {
            Source::Player { uuid, name } => vec![(*uuid, name.clone())],
            // Консоль командой не является: ей выбирать некого.
            Source::Console => Vec::new(),
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

/// Находит одного игрока по имени или значку выбора.
///
/// Если под значок подходят несколько, берётся первый: командам вроде
/// «перенести к» нужен один.
fn find_player_named(named: &str, source: &Source, shared: &Shared) -> Option<([u8; 16], String)> {
    find_targets(named, source, shared).into_iter().next()
}

/// Режим игры по названию, как его вводят в команде.
fn game_mode_by_name(name: &str) -> Option<i32> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_specs_stay_one_word_and_carry_their_components() {
        assert_eq!(compact_item_specs("give a iron_sword[enchantments={sharpness:5, knockback:1}] 1"), "give a iron_sword[enchantments={sharpness:5,knockback:1}] 1");
        assert_eq!(compact_item_specs("say [a b] c"), "say [ab] c");

        let stack = stack_from_spec("minecraft:diamond_sword[damage=3,enchantments={sharpness:2}]", 1).expect("разобралось").expect("не воздух");

        assert_eq!(stack.extras().damage, 3);
        assert_eq!(stack.extras().level("sharpness"), 2);
        assert!(stack_from_spec("nope", 1).is_err());
        assert_eq!(stack_from_spec("air", 1), Ok(None));
    }
    use crate::players::{Member, Position};
    use crate::world::World;
    use std::path::PathBuf;

    /// Данные призываемого предмета: вид и число.
    #[test]
    fn summoned_item_data_is_read() {
        let stack = item_from_text(r#"{Item:{id:"minecraft:cobblestone",count:3}}"#).expect("предмет");

        assert_eq!(stack.item, crate::blocks::item_named("cobblestone").expect("булыжник"));
        assert_eq!(stack.count, 3);
        assert_eq!(item_from_text(r#"{Item:{id:"dirt"}}"#).map(|stack| stack.count), Some(1));
        assert!(item_from_text("{}").is_none());
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
    fn server() -> Shared {
        let shared = Shared::new(
            World::in_memory(),
            crate::world::World::in_memory(),
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
    fn give_sends_the_items_to_the_player() {
        let shared = server();

        let answer = run("give Первый stone 5", &Source::Console, &shared);
        assert!(answer.reply.contains("Выдано"), "{}", answer.reply);

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
        assert!(reply.contains("изменён"), "{}", reply);

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
        assert!(run("summon horse 1 2 3", &Source::Console, &shared).reply.contains("Пока"));
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
}
