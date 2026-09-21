// Выполнение команд.
//
// Команды приходят с двух сторон: от игрока в игре и от человека за консолью
// сервера. Разбор у них общий, а последствия разные: игроку надо сменить режим
// игры и ответить ему в чат, консоли — напечатать ответ и, бывает, остановить
// сервер. Поэтому разбор не делает это сам, а описывает, что должен сделать
// тот, кто команду вызвал.

use crate::blocks;
use crate::ops;
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

pub fn run(command: &str, source: &Source, shared: &Shared) -> Answer {
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

            let Some((uuid, name)) = targets.first().cloned() else {
                return Answer::saying(
                    "Такого игрока нет на сервере. Права выдаются тем, кто сейчас играет"
                        .to_string(),
                );
            };

            let given = shared
                .ops
                .lock()
                .expect("права захвачены другим потоком")
                .give(&uuid, &name, ops::HIGHEST_LEVEL);

            if given {
                Answer::saying(format!("У игрока {} теперь есть права", name))
            } else {
                Answer::saying(format!("У игрока {} права уже были", name))
            }
        }
        "deop" => {
            let Some(named) = parts.next() else {
                return Answer::saying("У кого снять права? deop <игрок>".to_string());
            };

            let taken = shared
                .ops
                .lock()
                .expect("права захвачены другим потоком")
                .take_away(named);

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

            let Some(item) = blocks::item_named(&without_namespace(item_name)) else {
                return Answer::saying(format!("Нет такого предмета: {}", item_name));
            };

            let count: i32 = parts.next().and_then(|value| value.parse().ok()).unwrap_or(1);

            if count <= 0 {
                return Answer::saying("Сколько выдавать? Число должно быть больше нуля".to_string());
            }

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
                    players.order(uuid, Order::Give { item, count });
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
        "setblock" => set_blocks(parts.collect::<Vec<&str>>().as_slice(), shared, false),
        "fill" => set_blocks(parts.collect::<Vec<&str>>().as_slice(), shared, true),
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
        "tp" | "teleport" => teleport(parts.collect::<Vec<&str>>().as_slice(), source, shared),
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

/// Имя предмета без «minecraft:» — писать его каждый раз незачем.
fn without_namespace(name: &str) -> String {
    name.strip_prefix("minecraft:").unwrap_or(name).to_string()
}

/// Ники через запятую — для ответов.
fn names_of(targets: &[([u8; 16], String)]) -> String {
    targets
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<&str>>()
        .join(", ")
}

/// Ставит блок в одно место (`setblock`) или заливает им область (`fill`).
///
/// Блок записывается как в игре: `minecraft:lever[face=floor,powered=true]`.
/// Как и у обычного сервера, соседи узнают о новом блоке, а редстоун
/// пересчитывается; своей логики нажатия у поставленного рычага нет.
/// Область у `fill` ограничена, как в игре, 32 768 блоками.
fn set_blocks(parts: &[&str], shared: &Shared, area: bool) -> Answer {
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

    let Some(state) = blocks::state_from_text(block) else {
        return Answer::saying(format!("Не знаю такого блока: {}", block));
    };

    let [x1, y1, z1, x2, y2, z2] = numbers;
    let (x1, x2) = (x1.min(x2), x1.max(x2));
    let (y1, y2) = (y1.min(y2), y1.max(y2));
    let (z1, z2) = (z1.min(z2), z1.max(z2));

    let volume = (x2 - x1 + 1) as i64 * (y2 - y1 + 1) as i64 * (z2 - z1 + 1) as i64;

    if volume > 32_768 {
        return Answer::saying(format!(
            "Слишком много блоков: {} (можно не больше 32768)",
            volume
        ));
    }

    let mut world = shared.world.lock().expect("мир захвачен другим потоком");
    let mut changed = 0;

    for x in x1..=x2 {
        for y in y1..=y2 {
            for z in z1..=z2 {
                world.ensure(x >> 4, z >> 4);

                if world.get_block(x, y, z) != state && world.set_block(x, y, z, state) {
                    changed += 1;
                }
            }
        }
    }

    for x in x1..=x2 {
        for y in y1..=y2 {
            for z in z1..=z2 {
                crate::placing::refresh_neighbours(&mut world, x, y, z);
            }
        }
    }

    crate::redstone::settle(&mut world);
    world.save_if_needed();

    if area {
        Answer::saying(format!("Заполнено блоков: {}", changed))
    } else if changed == 0 {
        Answer::saying("Там уже такой блок".to_string())
    } else {
        Answer::saying(format!("Блок в {}, {}, {} изменён", x1, y1, z1))
    }
}

/// Который час в мире и как его поменять.
///
/// Время идёт тактами, сутки — 24 000 тактов: рассвет 0, полдень 6000,
/// закат 12 000, полночь 18 000.
fn time(parts: &[&str], shared: &Shared) -> Answer {
    let mut world = shared.world.lock().expect("мир захвачен другим потоком");

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

            let time = world.time_of_day() + added;
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
fn teleport(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let own = match source {
        Source::Player { uuid, name } => vec![(*uuid, name.clone())],
        Source::Console => Vec::new(),
    };

    // Сперва решаем, кого переносим, потом — куда.
    let (targets, place) = match parts {
        // tp <куда-то> или tp <x> <y> <z> — переносится сам.
        [_] | [_, _, _] => (own, parts),

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

    let Some((x, y, z)) = place_of(place, source, shared) else {
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
            players.order(uuid, Order::Teleport { x, y, z });
        }
    }

    let own_move = matches!(
        source,
        Source::Player { uuid: caller, .. }
            if targets.len() == 1 && targets[0].0 == *caller
    );

    let reply = if own_move {
        format!("Переношу в {:.0} {:.0} {:.0}", x, y, z)
    } else {
        format!(
            "{} перенесён в {:.0} {:.0} {:.0}",
            targets
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<&str>>()
                .join(", "),
            x,
            y,
            z
        )
    };

    Answer::saying(reply)
}

/// Место, названное в команде: имя игрока или три числа.
fn place_of(parts: &[&str], source: &Source, shared: &Shared) -> Option<(f64, f64, f64)> {
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
                .map(|member| (member.position.x, member.position.y, member.position.z))
        }

        [x, y, z] => Some((x.parse().ok()?, y.parse().ok()?, z.parse().ok()?)),

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
    use crate::players::{Member, Position};
    use crate::world::World;
    use std::path::PathBuf;

    /// Свой файл прав на каждую проверку: проверки идут разом и общий файл
    /// делили бы друг с другом.
    fn own_ops_file() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};

        static NEXT: AtomicU32 = AtomicU32::new(0);

        let mut path = std::env::temp_dir();
        path.push(format!(
            "rustcraft-test-ops-{}-{}.json",
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
                    held: None,
                    position: Position {
                        x: number as f64 * 10.0,
                        y: 1.0,
                        z: 0.0,
                        yaw: 0.0,
                        pitch: 0.0,
                        on_ground: true,
                    },
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

        // Себя по числам.
        run("tp 5 64 -7", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 1).first(),
            Some(Order::Teleport { x, y, z }) if *x == 5.0 && *y == 64.0 && *z == -7.0
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
            Some(Order::Teleport { x, y, z }) if *x == 1.0 && *y == 2.0 && *z == 3.0
        ));

        // Другого — к третьему.
        run("tp Второй Первый", &first, &shared);

        assert!(matches!(
            orders_for(&shared, 2).first(),
            Some(Order::Teleport { x, .. }) if *x == 10.0
        ));

        // Из консоли без имени переносить некого.
        let answer = run("tp 1 2 3", &Source::Console, &shared);
        assert!(answer.reply.contains("Кого переносить"));

        // Чепуха вместо места — понятный ответ и никаких поручений.
        let answer = run("tp сюда-вот", &first, &shared);
        assert!(answer.reply.contains("Не понял"));
        assert!(orders_for(&shared, 1).is_empty());
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

    /// Без прав команды, меняющие мир, не работают, а безобидные — работают.
    #[test]
    fn commands_ask_for_rights() {
        let shared = server();

        {
            let mut ops = shared.ops.lock().expect("права");
            ops.take_away("Первый");
            ops.take_away("Второй");
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
            matches!(orders.first(), Some(Order::Give { count: 5, .. })),
            "поручение не дошло"
        );

        // Несуществующий предмет не выдаётся.
        let answer = run("give Первый такого_нет", &Source::Console, &shared);
        assert!(answer.reply.contains("Нет такого предмета"), "{}", answer.reply);
    }


    #[test]
    fn setblock_and_fill_change_the_world() {
        let shared = server();

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
            let world = shared.world.lock().expect("мир");
            assert_eq!(world.get_block(3, 5, 7), lever);
        }

        let reply = run("fill 0 1 0 1 1 1 stone", &Source::Console, &shared).reply;
        assert!(reply.contains("4"), "{}", reply);

        let stone = blocks::state_by_name("stone").expect("камень есть");
        let world = shared.world.lock().expect("мир");
        assert_eq!(world.get_block(1, 1, 1), stone);

        assert!(run("setblock 0 0 0 nonsense_block", &Source::Console, &shared)
            .reply
            .contains("Не знаю"));
        assert!(run("fill 0 0 0 100 100 100 stone", &Source::Console, &shared)
            .reply
            .contains("Слишком много"));
    }
}
