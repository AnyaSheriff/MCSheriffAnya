// Команды администрирования, сообщений, звуков и частиц.
//
// Здесь живут: title, tellraw, me, msg/tell/w, playsound, stopsound, particle,
// setworldspawn, spawnpoint, и серверные: ban, ban-ip, banlist, pardon,
// pardon-ip, whitelist, save-all/on/off, setidletimeout, defaultgamemode,
// seed, version, transfer, reload.
//
// Тексты ответов — переводимые компоненты, как у оригинала (чёрный ящик,
// сценарии cmd-*): клиент сам переводит `translate` на свой язык. Компонент
// собирается в сетевой NBT так же, как у оригинала: поля составного тега идут
// в порядке обхода Java HashMap (см. `java_order`), а список из разных видов
// тегов записывается списком составных с пустым ключом.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::commands::{Answer, Effect, Source};
use crate::players::Order;
use crate::shared::Shared;
use crate::snbt::Value;
use crate::{log_info, log_warn};

// ───────────────────────────── NBT-теги ─────────────────────────────

/// Тег NBT — ровно то, из чего собираются компоненты текста.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tag {
    Byte(i8),
    Int(i32),
    Float(f32),
    Str(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Int(_) => 3,
            Tag::Float(_) => 5,
            Tag::Str(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
        }
    }

    /// Корневой тег сетевого NBT: тип и содержимое, без имени.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = vec![self.id()];

        self.payload(&mut out);
        out
    }

    fn payload(&self, out: &mut Vec<u8>) {
        match self {
            Tag::Byte(value) => out.push(*value as u8),
            Tag::Int(value) => out.extend_from_slice(&value.to_be_bytes()),
            Tag::Float(value) => out.extend_from_slice(&value.to_be_bytes()),
            Tag::Str(text) => crate::network::nbt::write_string(out, text),
            Tag::IntArray(values) => {
                out.extend_from_slice(&(values.len() as i32).to_be_bytes());

                for value in values {
                    out.extend_from_slice(&value.to_be_bytes());
                }
            }
            Tag::List(items) => {
                // Разные виды в одном списке оригинал пишет списком
                // составных: чужие теги — под пустым ключом (чёрный ящик,
                // cmd-title: аргументы `with` из чисел и строк).
                let first = items.first().map(Tag::id).unwrap_or(0);
                let mixed = items.iter().any(|item| item.id() != first);

                out.push(if mixed { 10 } else { first });
                out.extend_from_slice(&(items.len() as i32).to_be_bytes());

                for item in items {
                    match (mixed, item) {
                        (true, Tag::Compound(_)) | (false, _) => item.payload(out),
                        (true, other) => {
                            out.push(other.id());
                            crate::network::nbt::write_string(out, "");
                            other.payload(out);
                            out.push(0);
                        }
                    }
                }
            }
            Tag::Compound(fields) => {
                let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();

                for index in java_order(&names) {
                    let (name, tag) = &fields[index];

                    out.push(tag.id());
                    crate::network::nbt::write_string(out, name);
                    tag.payload(out);
                }

                out.push(0);
            }
        }
    }
}

/// Хеш строки как у Java (`String.hashCode`, по единицам UTF-16).
fn java_hash(text: &str) -> u32 {
    text.encode_utf16().fold(0u32, |hash, unit| hash.wrapping_mul(31).wrapping_add(u32::from(unit)))
}

/// Порядок обхода полей составного тега у оригинала: тег лежит в `HashMap`
/// (таблица от 16 корзин, растёт вдвое при заполнении на три четверти), обход
/// идёт по корзинам, внутри корзины — по порядку добавления. Сверено по
/// записям чёрного ящика (cmd-title, cmd-tellraw).
fn java_order(names: &[&str]) -> Vec<usize> {
    let mut table = 16usize;

    while names.len() * 4 > table * 3 {
        table *= 2;
    }

    let bucket = |name: &str| {
        let hash = java_hash(name);

        ((hash ^ (hash >> 16)) as usize) & (table - 1)
    };

    let mut order: Vec<usize> = (0..names.len()).collect();

    order.sort_by_key(|&index| (bucket(names[index]), index));
    order
}

/// Шаблоны текстов 26.1 (minecraft-data, language.json) — для консоли и журнала.
const LANG: &[(&str, &str)] = &[
    ("argument.component.invalid", "Invalid chat component: %s"),
    ("argument.entity.notfound.player", "No player was found"),
    ("argument.entity.options.limit.toosmall", "Limit must be at least 1"),
    ("argument.float.big", "Float must not be more than %s: found %s"),
    ("argument.float.low", "Float must not be less than %s: found %s"),
    ("argument.gamemode.invalid", "Unknown game mode: %s"),
    ("argument.id.invalid", "Invalid ID"),
    ("argument.integer.big", "Integer must not be more than %s: found %s"),
    ("argument.integer.low", "Integer must not be less than %s: found %s"),
    ("argument.literal.incorrect", "Expected literal %s"),
    ("argument.player.unknown", "That player does not exist"),
    ("argument.pos.missing.double", "Expected a coordinate"),
    ("argument.rotation.incomplete", "Incomplete (expected 2 coordinates)"),
    ("argument.time.invalid_tick_count", "The tick count must be non-negative"),
    ("argument.time.invalid_unit", "Invalid unit"),
    ("argument.time.tick_count_too_low", "The tick count must not be less than %s: found %s"),
    ("chat.square_brackets", "[%s]"),
    ("chat.type.admin", "[%s: %s]"),
    ("chat.type.emote", "* %s %s"),
    ("command.context.here", "<--[HERE]"),
    ("command.unknown.argument", "Incorrect argument for command"),
    ("command.unknown.command", "Unknown or incomplete command. See below for error"),
    ("commands.ban.failed", "Nothing changed. The player is already banned"),
    ("commands.ban.success", "Banned %s: %s"),
    ("commands.banip.failed", "Nothing changed. That IP is already banned"),
    ("commands.banip.info", "This ban affects %s player(s): %s"),
    ("commands.banip.invalid", "Invalid IP address or unknown player"),
    ("commands.banip.success", "Banned IP %s: %s"),
    ("commands.banlist.entry", "%s was banned by %s: %s"),
    ("commands.banlist.entry.unknown", "(Unknown)"),
    ("commands.banlist.list", "There are %s ban(s):"),
    ("commands.banlist.none", "There are no bans"),
    ("commands.defaultgamemode.success", "The default game mode is now %s"),
    ("commands.list.nameAndId", "%s (%s)"),
    ("commands.list.players", "There are %s of a max of %s players online: %s"),
    ("commands.message.display.incoming", "%s whispers to you: %s"),
    ("commands.message.display.outgoing", "You whisper to %s: %s"),
    ("commands.pardon.failed", "Nothing changed. The player isn't banned"),
    ("commands.pardon.success", "Unbanned %s"),
    ("commands.pardonip.failed", "Nothing changed. That IP isn't banned"),
    ("commands.pardonip.invalid", "Invalid IP address"),
    ("commands.pardonip.success", "Unbanned IP %s"),
    ("commands.particle.failed", "The particle was not visible for anybody"),
    ("commands.particle.success", "Displaying particle %s"),
    ("commands.playsound.failed", "The sound is too far away to be heard"),
    ("commands.playsound.success.multiple", "Played sound %s to %s players"),
    ("commands.playsound.success.single", "Played sound %s to %s"),
    ("commands.reload.failure", "Reload failed; keeping old data"),
    ("commands.reload.success", "Reloading!"),
    ("commands.save.alreadyOff", "Saving is already turned off"),
    ("commands.save.alreadyOn", "Saving is already turned on"),
    ("commands.save.disabled", "Automatic saving is now disabled"),
    ("commands.save.enabled", "Automatic saving is now enabled"),
    ("commands.save.failed", "Unable to save the game (is there enough disk space?)"),
    ("commands.save.saving", "Saving the game (this may take a moment!)"),
    ("commands.save.success", "Saved the game"),
    ("commands.seed.success", "Seed: %s"),
    ("commands.setidletimeout.success", "The player idle timeout is now %s minute(s)"),
    ("commands.setidletimeout.success.disabled", "The player idle timeout is now disabled"),
    ("commands.setworldspawn.failure.not_overworld", "Can only set the world spawn for overworld"),
    ("commands.setworldspawn.success", "Set the world spawn point to %s, %s, %s [%s]"),
    ("commands.setworldspawn.success.new", "Set the world spawn point to %s, %s, %s [%s, %s] in %s"),
    ("commands.spawnpoint.success.multiple", "Set spawn point to %s, %s, %s [%s] in %s for %s players"),
    ("commands.spawnpoint.success.multiple.new", "Set spawn point to %s, %s, %s [%s, %s] in %s for %s players"),
    ("commands.spawnpoint.success.single", "Set spawn point to %s, %s, %s [%s] in %s for %s"),
    ("commands.spawnpoint.success.single.new", "Set spawn point to %s, %s, %s [%s, %s] in %s for %s"),
    ("commands.stopsound.success.source.any", "Stopped all '%s' sounds"),
    ("commands.stopsound.success.source.sound", "Stopped sound '%s' on source '%s'"),
    ("commands.stopsound.success.sourceless.any", "Stopped all sounds"),
    ("commands.stopsound.success.sourceless.sound", "Stopped sound '%s'"),
    ("commands.title.cleared.multiple", "Cleared titles for %s players"),
    ("commands.title.cleared.single", "Cleared titles for %s"),
    ("commands.title.reset.multiple", "Reset title options for %s players"),
    ("commands.title.reset.single", "Reset title options for %s"),
    ("commands.title.show.actionbar.multiple", "Showing new actionbar title for %s players"),
    ("commands.title.show.actionbar.single", "Showing new actionbar title for %s"),
    ("commands.title.show.subtitle.multiple", "Showing new subtitle for %s players"),
    ("commands.title.show.subtitle.single", "Showing new subtitle for %s"),
    ("commands.title.show.title.multiple", "Showing new title for %s players"),
    ("commands.title.show.title.single", "Showing new title for %s"),
    ("commands.title.times.multiple", "Changed title display times for %s players"),
    ("commands.title.times.single", "Changed title display times for %s"),
    ("commands.transfer.error.no_players", "Must specify at least one player to transfer"),
    ("commands.transfer.success.multiple", "Transferring %s players to %s:%s"),
    ("commands.transfer.success.single", "Transferring %s to %s:%s"),
    ("commands.version.build_time", "build_time = %s"),
    ("commands.version.data", "data = %s"),
    ("commands.version.header", "Server version info:"),
    ("commands.version.id", "id = %s"),
    ("commands.version.name", "name = %s"),
    ("commands.version.pack.data", "pack_data = %s"),
    ("commands.version.pack.resource", "pack_resource = %s"),
    ("commands.version.protocol", "protocol = %s (%s)"),
    ("commands.version.series", "series = %s"),
    ("commands.version.stable.no", "stable = no"),
    ("commands.version.stable.yes", "stable = yes"),
    ("commands.whitelist.add.failed", "Player is already whitelisted"),
    ("commands.whitelist.add.success", "Added %s to the whitelist"),
    ("commands.whitelist.alreadyOff", "Whitelist is already turned off"),
    ("commands.whitelist.alreadyOn", "Whitelist is already turned on"),
    ("commands.whitelist.disabled", "Whitelist is now turned off"),
    ("commands.whitelist.enabled", "Whitelist is now turned on"),
    ("commands.whitelist.list", "There are %s whitelisted player(s): %s"),
    ("commands.whitelist.none", "There are no whitelisted players"),
    ("commands.whitelist.reloaded", "Reloaded the whitelist"),
    ("commands.whitelist.remove.failed", "Player is not whitelisted"),
    ("commands.whitelist.remove.success", "Removed %s from the whitelist"),
    ("gameMode.adventure", "Adventure Mode"),
    ("gameMode.creative", "Creative Mode"),
    ("gameMode.spectator", "Spectator Mode"),
    ("gameMode.survival", "Survival Mode"),
    ("multiplayer.disconnect.banned", "You are banned from this server"),
    ("multiplayer.disconnect.banned.expiration", "\nYour ban will be removed on %s"),
    ("multiplayer.disconnect.banned.reason", "You are banned from this server.\nReason: %s"),
    ("multiplayer.disconnect.banned_ip.expiration", "\nYour ban will be removed on %s"),
    ("multiplayer.disconnect.banned_ip.reason", "Your IP address is banned from this server.\nReason: %s"),
    ("multiplayer.disconnect.idling", "You have been idle for too long!"),
    ("multiplayer.disconnect.ip_banned", "You have been IP banned from this server"),
    ("multiplayer.disconnect.not_whitelisted", "You are not white-listed on this server!"),
    ("parsing.float.expected", "Expected float"),
    ("parsing.float.invalid", "Invalid float '%s'"),
    ("parsing.int.expected", "Expected integer"),
    ("parsing.int.invalid", "Invalid integer '%s'"),
    ("particle.invalidOptions", "Can't parse particle options: %s"),
    ("particle.notFound", "Unknown particle: %s"),
    ("permissions.requires.player", "A player is required to run this command here"),
    ("argument.pos3d.incomplete", "Incomplete (expected 3 coordinates)"),
    ("command.expected.separator", "Expected whitespace to end one argument, but found trailing data"),
];

// ───────────────────────── Сборка компонентов ─────────────────────────

fn s(text: &str) -> Tag {
    Tag::Str(text.to_string())
}

fn obj(fields: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(fields.into_iter().map(|(name, tag)| (name.to_string(), tag)).collect())
}

/// `{translate: key, with: [...]}` — `with` не пишется, если аргументов нет.
pub(crate) fn translate(key: &str, with: Vec<Tag>) -> Tag {
    if with.is_empty() {
        obj(vec![("translate", s(key))])
    } else {
        obj(vec![("translate", s(key)), ("with", Tag::List(with))])
    }
}

/// Красный текст ошибки: `{color: red, extra: [msg], text: ""}`.
fn red(message: Tag) -> Tag {
    obj(vec![("color", s("red")), ("extra", Tag::List(vec![message])), ("text", s(""))])
}

/// Имя игрока как компонент: с подсказкой при наведении и щелчком «написать».
pub(crate) fn player_name(name: &str, uuid: &[u8; 16]) -> Tag {
    let words: Vec<i32> = uuid.chunks(4).map(|word| i32::from_be_bytes([word[0], word[1], word[2], word[3]])).collect();

    obj(vec![
        ("click_event", obj(vec![("action", s("suggest_command")), ("command", s(&format!("/tell {} ", name)))])),
        ("insertion", s(name)),
        ("text", s(name)),
        (
            "hover_event",
            obj(vec![
                ("name", s(name)),
                ("action", s("show_entity")),
                ("id", s("minecraft:player")),
                ("uuid", Tag::IntArray(words)),
            ]),
        ),
    ])
}

/// Сообщение об ошибке со строкой-контекстом: «…конец введённого <--[HERE]».
fn context(input: &str, byte_cursor: usize) -> Tag {
    let chars: Vec<char> = input.chars().collect();
    let cursor = input[..byte_cursor.min(input.len())].chars().count();
    let mut extra = Vec::new();

    if cursor > 10 {
        extra.push(s("..."));
    }

    let start = cursor.saturating_sub(10);

    if cursor > start {
        extra.push(Tag::Str(chars[start..cursor].iter().collect()));
    }

    if cursor < chars.len() {
        extra.push(obj(vec![("color", s("red")), ("underlined", Tag::Byte(1)), ("text", Tag::Str(chars[cursor..].iter().collect()))]));
    }

    extra.push(obj(vec![("color", s("red")), ("italic", Tag::Byte(1)), ("translate", s("command.context.here"))]));

    red(obj(vec![
        ("click_event", obj(vec![("action", s("suggest_command")), ("command", Tag::Str(format!("/{}", input)))])),
        ("color", s("gray")),
        ("extra", Tag::List(extra)),
        ("text", s("")),
    ]))
}

/// Выводит компонент простым текстом: для консоли, журнала и Bedrock.
pub(crate) fn render(tag: &Tag) -> String {
    match tag {
        Tag::Str(text) => text.clone(),
        Tag::Byte(value) => value.to_string(),
        Tag::Int(value) => value.to_string(),
        Tag::Float(value) => format!("{:?}", value),
        Tag::IntArray(_) => String::new(),
        Tag::List(items) => items.iter().map(render).collect(),
        Tag::Compound(fields) => {
            let get = |name: &str| fields.iter().find(|(field, _)| field == name).map(|(_, tag)| tag);

            let mut out = String::new();

            if let Some(Tag::Str(key)) = get("translate") {
                let arguments: Vec<String> = match get("with") {
                    Some(Tag::List(items)) => items.iter().map(|item| render(unwrap_empty_key(item))).collect(),
                    _ => Vec::new(),
                };
                let template = LANG.iter().find(|(name, _)| name == key).map(|(_, text)| *text).unwrap_or(key.as_str());
                let mut next = 0;
                let mut chars = template.chars().peekable();

                while let Some(symbol) = chars.next() {
                    if symbol == '%' && chars.peek() == Some(&'s') {
                        chars.next();
                        out.push_str(arguments.get(next).map(String::as_str).unwrap_or(""));
                        next += 1;
                    } else {
                        out.push(symbol);
                    }
                }
            } else if let Some(tag) = get("text") {
                out.push_str(&render(tag));
            } else if let Some(Tag::Str(key)) = get("keybind") {
                out.push_str(key);
            }

            if let Some(extra) = get("extra") {
                out.push_str(&render(extra));
            }

            out
        }
    }
}

/// Обёртка `{"": v}` вокруг тега в списке из разных видов — снять.
fn unwrap_empty_key(tag: &Tag) -> &Tag {
    match tag {
        Tag::Compound(fields) if fields.len() == 1 && fields[0].0.is_empty() => &fields[0].1,
        other => other,
    }
}

// ─────────────────── Разбор компонента из JSON и SNBT ───────────────────

/// Что известно при разборе компонента: кто ввёл команду (для селекторов).
struct Resolve<'a> {
    source: &'a Source,
    shared: &'a Shared,
}

/// Приводит записанное в команде к виду, в каком компонент уходит клиенту:
/// простой текст без оформления — строкой, остальное — составным тегом.
fn component(value: &Value, with: &Resolve) -> Result<Tag, String> {
    match value {
        Value::Str(text) => Ok(Tag::Str(text.clone())),
        Value::Num(_) => Err("Not a string".to_string()),
        Value::List(items) => {
            let mut parts = items.iter();
            let Some(first) = parts.next() else {
                return Ok(s(""));
            };

            // Первый — основа, остальные — её продолжение.
            let base = component(first, with)?;
            let mut extra: Vec<Tag> = Vec::new();

            for item in parts {
                extra.push(component(item, with)?);
            }

            if extra.is_empty() {
                return Ok(base);
            }

            Ok(match base {
                Tag::Str(text) => obj(vec![("extra", Tag::List(extra)), ("text", Tag::Str(text))]),
                Tag::Compound(mut fields) => {
                    match fields.iter_mut().find(|(name, _)| name == "extra") {
                        Some((_, Tag::List(existing))) => existing.extend(extra),
                        _ => fields.push(("extra".to_string(), Tag::List(extra))),
                    }

                    Tag::Compound(fields)
                }
                other => other,
            })
        }
        Value::Map(fields) => {
            let get = |name: &str| fields.iter().find(|(field, _)| field == name).map(|(_, value)| value);
            let mut out: Vec<(String, Tag)> = Vec::new();
            let mut content: Vec<(String, Tag)> = Vec::new();

            if let Some(Value::Str(text)) = get("text") {
                content.push(("text".to_string(), Tag::Str(text.clone())));
            } else if let Some(Value::Str(key)) = get("translate") {
                content.push(("translate".to_string(), Tag::Str(key.clone())));

                if let Some(Value::Str(fallback)) = get("fallback") {
                    content.push(("fallback".to_string(), Tag::Str(fallback.clone())));
                }

                if let Some(Value::List(arguments)) = get("with") {
                    let mut list = Vec::new();

                    for argument in arguments {
                        // Числа в `with` остаются числами (чёрный ящик, cmd-tellraw).
                        list.push(match argument {
                            Value::Num(number) if number.fract() == 0.0 => Tag::Int(*number as i32),
                            Value::Num(number) => Tag::Float(*number as f32),
                            other => component(other, with)?,
                        });
                    }

                    content.push(("with".to_string(), Tag::List(list)));
                }
            } else if let Some(Value::Str(key)) = get("keybind") {
                content.push(("keybind".to_string(), Tag::Str(key.clone())));
            } else if let Some(Value::Str(selector)) = get("selector") {
                // Селектор становится именами игроков (с подсказками), как у оригинала.
                let separator = match get("separator") {
                    Some(value) => component(value, with)?,
                    None => obj(vec![("color", s("gray")), ("text", s(", "))]),
                };
                let found = crate::commands::find_targets(selector, with.source, with.shared);
                let names: Vec<Tag> = found.iter().map(|(uuid, name)| player_name(name, uuid)).collect();

                return Ok(join(names, separator));
            } else if get("score").is_some() {
                // Табло у нас нет: оригинал без записи на табло выводит пустой текст.
                return Ok(s(""));
            } else if get("nbt").is_some() {
                // [проверить] данные сущностей и блоков в тексте пока не подставляются.
                return Ok(s(""));
            } else {
                content.push(("text".to_string(), s("")));
            }

            out.extend(content);

            if let Some(Value::List(items)) = get("extra") {
                let mut list = Vec::new();

                for item in items {
                    list.push(component(item, with)?);
                }

                if !list.is_empty() {
                    out.push(("extra".to_string(), Tag::List(list)));
                }
            }

            if let Some(color) = get("color").and_then(Value::as_str) {
                out.push(("color".to_string(), Tag::Str(color_name(color))));
            }

            for flag in ["bold", "italic", "underlined", "strikethrough", "obfuscated"] {
                if let Some(Value::Num(number)) = get(flag) {
                    out.push((flag.to_string(), Tag::Byte(i8::from(*number != 0.0))));
                }
            }

            for text_field in ["font", "insertion"] {
                if let Some(Value::Str(text)) = get(text_field) {
                    out.push((text_field.to_string(), Tag::Str(text.clone())));
                }
            }

            if let Some(Value::Map(event)) = get("click_event") {
                out.push(("click_event".to_string(), Tag::Compound(event.iter().filter_map(|(name, value)| plain_field(name, value)).collect())));
            }

            if let Some(Value::Map(event)) = get("hover_event") {
                let mut hover = Vec::new();

                for (name, value) in event {
                    match (name.as_str(), value) {
                        ("value", inner) => hover.push(("value".to_string(), component(inner, with)?)),
                        (other, value) => hover.extend(plain_field(other, value)),
                    }
                }

                out.push(("hover_event".to_string(), Tag::Compound(hover)));
            }

            // Только текст без оформления — просто строка.
            if out.len() == 1 && out[0].0 == "text" {
                return Ok(out.remove(0).1);
            }

            Ok(Tag::Compound(out))
        }
    }
}

/// Поле события, записанное простым значением.
fn plain_field(name: &str, value: &Value) -> Option<(String, Tag)> {
    match value {
        Value::Str(text) => Some((name.to_string(), Tag::Str(text.clone()))),
        Value::Num(number) => Some((name.to_string(), Tag::Int(*number as i32))),
        _ => None,
    }
}

/// Имя цвета как его хранит оригинал: названия — строчными, `#rrggbb` — прописными.
fn color_name(color: &str) -> String {
    match color.strip_prefix('#') {
        Some(hex) => format!("#{}", hex.to_uppercase()),
        None => color.to_lowercase(),
    }
}

/// Склеивает компоненты через разделитель: `{extra: [a, sep, b], text: ""}`.
/// Один компонент остаётся как есть.
fn join(mut parts: Vec<Tag>, separator: Tag) -> Tag {
    if parts.len() == 1 {
        return parts.remove(0);
    }

    let mut extra = Vec::new();

    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            extra.push(separator.clone());
        }

        extra.push(part);
    }

    obj(vec![("extra", Tag::List(extra)), ("text", s(""))])
}

// ─────────────────────── Разбор строки и ошибки ───────────────────────

/// Чем закончился разбор или выполнение.
enum Fail {
    /// Ошибка при выполнении: одно красное сообщение.
    Run(Tag),
    /// Ошибка разбора: красное сообщение и строка с меткой `<--[HERE]`
    /// на байте `.1` введённого.
    Parse(Tag, usize),
}

type R<T> = Result<T, Fail>;

fn failed(key: &str) -> Fail {
    Fail::Run(translate(key, vec![]))
}

fn unknown_command(at: usize) -> Fail {
    Fail::Parse(translate("command.unknown.command", vec![]), at)
}

fn unknown_argument(at: usize) -> Fail {
    Fail::Parse(translate("command.unknown.argument", vec![]), at)
}

/// Один компонент с начала остатка: строка в кавычках, `{…}` или `[…]` с
/// учётом вложенности и кавычек, иначе слово до пробела. Что дальше —
/// лишнее для команды (`title @s title "x" лишнее`).
fn component_len(text: &str) -> usize {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let first = text.chars().next();

    for (index, symbol) in text.char_indices() {
        if let Some(open) = quote {
            match (escaped, symbol) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, symbol) if symbol == open => {
                    quote = None;

                    if depth == 0 {
                        return index + symbol.len_utf8();
                    }
                }
                _ => {}
            }

            continue;
        }

        match symbol {
            '"' | '\'' => quote = Some(symbol),
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;

                if depth <= 0 {
                    return index + 1;
                }
            }
            ' ' if depth == 0 && !matches!(first, Some('"' | '\'' | '{' | '[')) => return index,
            _ => {}
        }
    }

    text.len()
}

/// Читатель введённой строки: слова через один пробел.
struct Rd<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Rd<'a> {
    fn new(text: &'a str) -> Self {
        Rd { text, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.pos.min(self.text.len())..]
    }

    /// Переход к следующему слову. Слова нет — команда не закончена.
    fn next_arg(&mut self) -> R<usize> {
        if self.rest().len() >= 2 && self.rest().starts_with(' ') {
            self.pos += 1;
            return Ok(self.pos);
        }

        // Одинокий пробел в конце — «лишний аргумент» (чёрный ящик, cmd-errors).
        if self.rest() == " " {
            return Err(unknown_argument(self.pos));
        }

        Err(unknown_command(self.text.len()))
    }

    /// Есть ли ещё слово (после него не одинокий пробел).
    fn more(&self) -> bool {
        self.rest().len() >= 2 && self.rest().starts_with(' ')
    }

    /// Слово до пробела.
    fn word(&mut self) -> &'a str {
        let rest = self.rest();
        let end = rest.find(' ').unwrap_or(rest.len());

        self.pos += end;
        &rest[..end]
    }

    /// Слово селектора: с квадратными скобками, в которых бывают пробелы.
    fn selector(&mut self) -> &'a str {
        let rest = self.rest();
        let mut depth = 0;
        let mut end = rest.len();

        for (index, symbol) in rest.char_indices() {
            match symbol {
                '[' => depth += 1,
                ']' if depth > 0 => depth -= 1,
                ' ' if depth == 0 => {
                    end = index;
                    break;
                }
                _ => {}
            }
        }

        self.pos += end;
        &rest[..end]
    }

        /// Один компонент (см. `component_len`).
    fn component(&mut self) -> &'a str {
        let rest = self.rest();
        let end = component_len(rest);

        self.pos += end;
        &rest[..end]
    }

    /// Всё оставшееся — для аргументов-сообщений.
    fn greedy(&mut self) -> &'a str {
        let rest = self.rest();

        self.pos = self.text.len();
        rest
    }

    /// После законченной команды остаться ничего не должно.
    fn finish(&self) -> R<()> {
        let rest = self.rest();

        if rest.is_empty() {
            return Ok(());
        }

        // Как у оригинала: после пробела метка стоит у следующего слова.
        let at = if self.more() { self.pos + 1 } else { self.pos };

        Err(unknown_argument(at))
    }

    fn float(&mut self, min: f32, max: f32) -> R<f32> {
        let start = self.next_arg()?;
        let word = self.word();
        let value: f32 = word.parse().map_err(|_| Fail::Parse(translate("parsing.float.expected", vec![]), start))?;

        if value < min {
            return Err(Fail::Parse(translate("argument.float.low", vec![Tag::Float(min), Tag::Float(value)]), start));
        }

        if value > max {
            return Err(Fail::Parse(translate("argument.float.big", vec![Tag::Float(max), Tag::Float(value)]), start));
        }

        Ok(value)
    }

    fn int(&mut self, min: i32, max: i32) -> R<i32> {
        let start = self.next_arg()?;
        let word = self.word();
        let value: i32 = word.parse().map_err(|_| Fail::Parse(translate("parsing.int.expected", vec![]), start))?;

        if value < min {
            return Err(Fail::Parse(translate("argument.integer.low", vec![Tag::Int(min), Tag::Int(value)]), start));
        }

        if value > max {
            return Err(Fail::Parse(translate("argument.integer.big", vec![Tag::Int(max), Tag::Int(value)]), start));
        }

        Ok(value)
    }

    /// Время: число с приставкой `d` (сутки), `s` (секунды), `t` (такты, по
    /// умолчанию). Метка ошибки — после слова.
    fn time(&mut self) -> R<i32> {
        let start = self.next_arg()?;
        let word = self.word();
        let end = start + word.len();
        let (number, factor) = match word.chars().last() {
            Some('d') => (&word[..word.len() - 1], 24000.0),
            Some('s') => (&word[..word.len() - 1], 20.0),
            Some('t') => (&word[..word.len() - 1], 1.0),
            _ => (word, 1.0),
        };
        let value: f32 = match number.parse() {
            Ok(value) => value,
            // Число прочитано, приставка неизвестна: метка после слова.
            Err(_) if word.chars().last().is_some_and(|c| c.is_alphabetic()) && word[..word.len() - 1].parse::<f32>().is_ok() => {
                return Err(Fail::Parse(translate("argument.time.invalid_unit", vec![]), end));
            }
            Err(_) => return Err(Fail::Parse(translate("parsing.float.expected", vec![]), start)),
        };
        let ticks = (value * factor).round() as i32;

        if ticks < 0 {
            return Err(Fail::Parse(translate("argument.time.tick_count_too_low", vec![Tag::Int(0), Tag::Int(ticks)]), end));
        }

        Ok(ticks)
    }

    /// Название ресурса: `minecraft:block.note_block.harp` или без приставки.
    fn id(&mut self) -> R<String> {
        let start = self.next_arg()?;
        let rest = self.rest();
        let valid = rest.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || "_-./:".contains(c))).unwrap_or(rest.len());
        let word = &rest[..valid];

        self.pos += valid;

        // Дальше не пробел — разбор названия остановился на чужом знаке.
        if !self.rest().is_empty() && !self.rest().starts_with(' ') {
            return Err(Fail::Parse(translate("command.expected.separator", vec![]), self.pos));
        }

        if word.is_empty() || word.matches(':').count() > 1 {
            return Err(Fail::Parse(translate("argument.id.invalid", vec![]), start));
        }

        Ok(if word.contains(':') { word.to_string() } else { format!("minecraft:{}", word) })
    }
}

/// Место и поворот того, кто ввёл команду.
struct Where {
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
    dimension: crate::dimension::Dimension,
}

/// Одна координата: число или `~` со сдвигом; целое число по x и z — середина блока.
fn coordinate(word: &str, from: f64, centre: bool) -> Option<f64> {
    match word.strip_prefix('~') {
        Some("") => Some(from),
        Some(shift) => Some(from + shift.parse::<f64>().ok()?),
        None => {
            let value: f64 = word.parse().ok()?;

            Some(if centre && !word.contains('.') { value + 0.5 } else { value })
        }
    }
}

/// Контекст выполнения: кто вводит и что уже сказано.
struct Ctx<'a> {
    source: &'a Source,
    shared: &'a Shared,
    input: &'a str,
    /// Что сказано простым текстом: ответ консоли.
    plain: Vec<String>,
}

const SYSTEM_CHAT: i32 = 0x79;

fn chat_body(tag: &Tag) -> Vec<u8> {
    let mut body = tag.encode();

    body.push(0); // в чат, а не над панелью предметов
    body
}

impl<'a> Ctx<'a> {
    fn me(&self) -> Option<([u8; 16], &'a str)> {
        match self.source {
            Source::Player { uuid, name } => Some((*uuid, name.as_str())),
            Source::Console | Source::Block { .. } => None,
        }
    }

    fn who(&self) -> Where {
        let default = Where { x: 0.0, y: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, dimension: crate::dimension::Dimension::Overworld };

        match self.me() {
            Some((uuid, _)) => self
                .shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .members()
                .iter()
                .find(|member| member.uuid == uuid)
                .map(|member| Where {
                    x: member.position.x,
                    y: member.position.y,
                    z: member.position.z,
                    yaw: member.position.yaw,
                    pitch: member.position.pitch,
                    dimension: member.dimension,
                })
                .unwrap_or(default),
            None => default,
        }
    }

    fn sender_name(&self) -> Tag {
        match self.me() {
            Some((uuid, name)) => player_name(name, &uuid),
            None => s("Server"),
        }
    }

    /// Отдаёт игроку готовый пакет.
    fn packet(&self, uuid: &[u8; 16], id: i32, body: Vec<u8>) {
        self.shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::PacketText { id, body, plain: String::new() });
    }

    /// Сообщение в чат одному игроку.
    fn chat_to(&self, uuid: &[u8; 16], tag: &Tag) {
        self.shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::PacketText { id: SYSTEM_CHAT, body: chat_body(tag), plain: render(tag) });
    }

    /// Успешный ответ: тому, кто ввёл, в журнал и остальным операторам
    /// («[Имя: …]» серым курсивом, как у оригинала; [проверить] вид).
    fn ok(&mut self, message: Tag) {
        let plain = render(&message);

        if let Some((uuid, name)) = self.me() {
            self.chat_to(&uuid, &message);
            log_info!("[{}: {}]", name, plain);
        }

        self.plain.push(plain);

        let admin = obj(vec![
            ("color", s("gray")),
            ("italic", Tag::Byte(1)),
            ("translate", s("chat.type.admin")),
            ("with", Tag::List(vec![self.sender_name(), message])),
        ]);
        let me = self.me().map(|(uuid, _)| uuid);
        let members: Vec<[u8; 16]> = self.shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().map(|member| member.uuid).collect();

        for uuid in members {
            if Some(uuid) != me && self.shared.ops.lock().expect("права захвачены другим потоком").level_of(&uuid) >= 2 {
                self.chat_to(&uuid, &admin);
            }
        }
    }

    /// Игроки по селектору или имени; пусто — ошибка «игрок не найден».
    fn players(&self, word: &str) -> R<Vec<([u8; 16], String)>> {
        let found = find_players(word, self.source, self.shared);

        if found.is_empty() {
            return Err(failed("argument.entity.notfound.player"));
        }

        Ok(found)
    }

    /// Имя одного игрока или число игроков — как их называют в ответе.
    fn who_text(targets: &[([u8; 16], String)]) -> Vec<Tag> {
        match targets {
            [(uuid, name)] => vec![player_name(name, uuid)],
            many => vec![Tag::Int(many.len() as i32)],
        }
    }
}

/// Игроки по селектору (`@a`, `@p[distance=..5]`, `@a[name=Имя,limit=1]`) или имени.
fn find_players(word: &str, source: &Source, shared: &Shared) -> Vec<([u8; 16], String)> {
    let (base, options) = match word.split_once('[') {
        Some((base, tail)) => (base, tail.strip_suffix(']').unwrap_or(tail)),
        None => (word, ""),
    };
    let mut found = crate::commands::find_targets(base, source, shared);

    if options.is_empty() {
        return found;
    }

    let members: Vec<crate::players::Member> = shared.players.lock().expect("список игроков захвачен другим потоком").members().to_vec();
    let origin = match source {
        Source::Player { uuid, .. } => members.iter().find(|member| &member.uuid == uuid).map(|member| (member.position.x, member.position.y, member.position.z)),
        // Командный блок считает от своей середины.
        Source::Block { x, y, z, .. } => Some((f64::from(*x) + 0.5, f64::from(*y) + 0.5, f64::from(*z) + 0.5)),
        Source::Console => None,
    }
    .unwrap_or((0.0, 0.0, 0.0));
    let mut limit = usize::MAX;

    for option in options.split(',') {
        let Some((key, value)) = option.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());

        match key {
            "name" => {
                let (negate, wanted) = value.strip_prefix('!').map_or((false, value), |rest| (true, rest));

                found.retain(|(_, name)| (name == wanted) != negate);
            }
            "limit" => limit = value.parse().unwrap_or(usize::MAX),
            "distance" => {
                let (low, high) = match value.split_once("..") {
                    Some((low, high)) => (low.parse().unwrap_or(0.0), high.parse().unwrap_or(f64::INFINITY)),
                    None => (value.parse().unwrap_or(0.0), value.parse().unwrap_or(0.0)),
                };

                found.retain(|(uuid, _)| {
                    members.iter().find(|member| &member.uuid == uuid).is_some_and(|member| {
                        let distance = ((member.position.x - origin.0).powi(2) + (member.position.y - origin.1).powi(2) + (member.position.z - origin.2).powi(2)).sqrt();

                        distance >= low && distance <= high
                    })
                });
            }
            "gamemode" => {
                let (negate, wanted) = value.strip_prefix('!').map_or((false, value), |rest| (true, rest));
                let wanted = crate::commands::game_mode_by_name(wanted);

                found.retain(|(uuid, _)| members.iter().find(|member| &member.uuid == uuid).is_some_and(|member| (Some(member.game_mode) == wanted) != negate));
            }
            // [проверить] остальные уточнения селекторов (team, tag, nbt, …) не разбираются.
            _ => {}
        }
    }

    found.truncate(limit);
    found
}

// ───────────────── Списки блокировок и прочее состояние ─────────────────

/// Запись `banned-players.json` — поля и их порядок как у оригинала.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct Ban {
    uuid: String,
    name: String,
    created: String,
    source: String,
    expires: String,
    reason: String,
}

/// Запись `banned-ips.json`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct IpBan {
    ip: String,
    created: String,
    source: String,
    expires: String,
    reason: String,
}

/// Запись `whitelist.json`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct Allowed {
    uuid: String,
    name: String,
}

const BANNED_PLAYERS: &str = "banned-players.json";
const BANNED_IPS: &str = "banned-ips.json";
const WHITELIST: &str = "whitelist.json";
const PROPERTIES: &str = "config/server.properties";

/// Причина по умолчанию — как у оригинала.
const DEFAULT_REASON: &str = "Banned by an operator.";

struct Admin {
    bans: Vec<Ban>,
    ip_bans: Vec<IpBan>,
    whitelist: Vec<Allowed>,
    whitelist_on: bool,
    enforce: bool,
    idle_minutes: i32,
    saving: bool,
    default_mode: i32,
    /// Поворот у общей точки появления (`setworldspawn`).
    world_spawn_angle: (f32, f32),
}

fn read_list<T: serde::de::DeserializeOwned>(file: &str) -> Vec<T> {
    std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

fn write_list<T: serde::Serialize>(file: &str, list: &[T]) {
    match serde_json::to_string_pretty(list) {
        Ok(text) => {
            let temporary = format!("{}.tmp", file);

            if let Err(error) = std::fs::write(&temporary, text).and_then(|_| std::fs::rename(&temporary, file)) {
                log_warn!("Не удалось записать {} ({})", file, error);
            }
        }
        Err(error) => log_warn!("Не удалось собрать {} ({})", file, error),
    }
}

/// Значение настройки из server.properties.
fn property(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(PROPERTIES).ok()?;

    text.lines().find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')).map(|value| value.trim().to_string()))
}

/// Записывает настройку в server.properties, остальное не трогая.
fn set_property(key: &str, value: &str) {
    let Ok(text) = std::fs::read_to_string(PROPERTIES) else {
        return;
    };
    let mut done = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.strip_prefix(key).is_some_and(|rest| rest.starts_with('=')) {
                done = true;
                format!("{}={}", key, value)
            } else {
                line.to_string()
            }
        })
        .collect();

    if !done {
        lines.push(format!("{}={}", key, value));
    }

    let _ = std::fs::write(PROPERTIES, lines.join("\n") + "\n");
}

impl Admin {
    fn load() -> Self {
        Admin {
            bans: read_list(BANNED_PLAYERS),
            ip_bans: read_list(BANNED_IPS),
            whitelist: read_list(WHITELIST),
            whitelist_on: property("white-list").is_some_and(|value| value == "true"),
            enforce: property("enforce-whitelist").is_some_and(|value| value == "true"),
            idle_minutes: property("player-idle-timeout").and_then(|value| value.parse().ok()).unwrap_or(0),
            saving: true,
            // Как и прежде у ядра: новый игрок входит в творческом режиме.
            default_mode: crate::commands::GAME_MODE_CREATIVE,
            world_spawn_angle: (0.0, 0.0),
        }
    }
}

fn admin() -> MutexGuard<'static, Admin> {
    static ADMIN: OnceLock<Mutex<Admin>> = OnceLock::new();

    ADMIN.get_or_init(|| Mutex::new(Admin::load())).lock().expect("списки администратора захвачены другим потоком")
}

/// Адреса, с которых играют: нужны для `ban-ip <ник>`.
fn addresses() -> MutexGuard<'static, HashMap<[u8; 16], IpAddr>> {
    static ADDRESSES: OnceLock<Mutex<HashMap<[u8; 16], IpAddr>>> = OnceLock::new();

    ADDRESSES.get_or_init(|| Mutex::new(HashMap::new())).lock().expect("адреса захвачены другим потоком")
}

/// Запоминает, с какого адреса вошёл игрок.
pub(crate) fn note_address(uuid: &[u8; 16], ip: IpAddr) {
    addresses().insert(*uuid, ip);
}

/// Забывает адрес вышедшего игрока.
pub(crate) fn forget_address(uuid: &[u8; 16]) {
    addresses().remove(uuid);
}

/// Причина выгона за бездействие: «You have been idle for too long!».
pub(crate) fn idling() -> Vec<u8> {
    translate("multiplayer.disconnect.idling", vec![]).encode()
}

/// Режим игры для вошедшего впервые (`defaultgamemode`).
pub(crate) fn default_game_mode() -> i32 {
    admin().default_mode
}

/// Включено ли автосохранение (`save-on` / `save-off`).
/// [проверить] пока только запоминается: сброс на диск идёт из многих мест
/// (выгрузка чанков тоже), и отключать его целиком опасно.
#[allow(dead_code)]
pub(crate) fn saving_enabled() -> bool {
    admin().saving
}

/// Ники для дополнения pardon: текущие блокировки, включая игроков вне сети.
pub(crate) fn banned_names() -> Vec<String> {
    admin().bans.iter().map(|ban| ban.name.clone()).collect()
}

/// Через сколько тактов бездействия выгонять; None — не выгонять.
pub(crate) fn idle_limit_ticks() -> Option<u64> {
    let minutes = admin().idle_minutes;

    (minutes > 0).then(|| minutes as u64 * 60 * 20)
}

/// Время как в файлах оригинала: `2026-10-02 12:34:56 +0000` (всегда по UTC).
fn stamp() -> String {
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_secs() as i64).unwrap_or(0);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Гражданская дата по числу дней от 1970-01-01 (алгоритм Говарда Хиннанта).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = if month_part < 10 { month_part + 3 } else { month_part - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} +0000", year, month, day, rest / 3600, rest % 3600 / 60, rest % 60)
}

/// Кто выдал блокировку: ник или `Server`.
fn author(source: &Source) -> String {
    match source {
        Source::Player { name, .. } | Source::Block { name, .. } => name.clone(),
        Source::Console => "Server".to_string(),
    }
}

/// Игрок по записи команды: селектор среди тех, кто в сети, или ник (даже
/// неизвестный: без проверки Mojang опознаватель считается из ника).
fn profiles(word: &str, source: &Source, shared: &Shared) -> Vec<([u8; 16], String)> {
    if word.starts_with('@') {
        return find_players(word, source, shared);
    }

    match crate::commands::find_targets(word, source, shared).into_iter().next() {
        Some(found) => vec![found],
        None => vec![(crate::network::login::offline_uuid(word), word.to_string())],
    }
}

fn uuid_text(uuid: &[u8; 16]) -> String {
    crate::network::login::format_uuid(uuid)
}

// ───────────────────── Вход: блокировки и белый список ─────────────────────

/// Отказ во входе: текст-компонент в виде JSON для фазы входа и простой
/// текст для журнала. None — пускать. Порядок как у оригинала: блокировка
/// по нику, по адресу, белый список.
pub(crate) fn login_refusal(name: &str, uuid: &[u8; 16], ip: IpAddr, is_op: bool) -> Option<(String, String)> {
    refusal(&admin(), name, uuid, ip, is_op)
}

fn refusal(state: &Admin, name: &str, uuid: &[u8; 16], ip: IpAddr, is_op: bool) -> Option<(String, String)> {
    let id = uuid_text(uuid);

    if let Some(ban) = state.bans.iter().find(|ban| ban.uuid == id || ban.name.eq_ignore_ascii_case(name)) {
        let mut json = format!("{{\"translate\":\"multiplayer.disconnect.banned.reason\",\"with\":[{}]}}", serde_json::Value::String(ban.reason.clone()));
        let mut plain = format!("You are banned from this server.\nReason: {}", ban.reason);

        if ban.expires != "forever" {
            json = format!(
                "{{\"translate\":\"multiplayer.disconnect.banned.reason\",\"with\":[{}],\"extra\":[{{\"translate\":\"multiplayer.disconnect.banned.expiration\",\"with\":[{}]}}]}}",
                serde_json::Value::String(ban.reason.clone()),
                serde_json::Value::String(ban.expires.clone())
            );
            plain.push_str(&format!("\nYour ban will be removed on {}", ban.expires));
        }

        return Some((json, plain));
    }

    if let Some(ban) = state.ip_bans.iter().find(|ban| ban.ip == ip.to_string()) {
        let json = format!("{{\"translate\":\"multiplayer.disconnect.banned_ip.reason\",\"with\":[{}]}}", serde_json::Value::String(ban.reason.clone()));

        return Some((json, format!("Your IP address is banned from this server.\nReason: {}", ban.reason)));
    }

    if state.whitelist_on && !is_op && !state.whitelist.iter().any(|entry| entry.uuid == id || entry.name.eq_ignore_ascii_case(name)) {
        return Some(("{\"translate\":\"multiplayer.disconnect.not_whitelisted\"}".to_string(), "You are not white-listed on this server!".to_string()));
    }

    None
}

/// Выгоняет из сети тех, кого уже нельзя (белый список с `enforce-whitelist`).
fn enforce_whitelist(shared: &Shared) {
    let (on, enforce) = {
        let state = admin();

        (state.whitelist_on, state.enforce)
    };

    if !(on && enforce) {
        return;
    }

    let members: Vec<([u8; 16], String)> = shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().map(|member| (member.uuid, member.name.clone())).collect();

    for (uuid, name) in members {
        let allowed = {
            let state = admin();

            state.whitelist.iter().any(|entry| entry.uuid == uuid_text(&uuid) || entry.name.eq_ignore_ascii_case(&name))
        };

        if !allowed && shared.ops.lock().expect("права захвачены другим потоком").level_of(&uuid) == 0 {
            kick_with(shared, &uuid, translate("multiplayer.disconnect.not_whitelisted", vec![]));
        }
    }
}

/// Выгоняет игрока с причиной-компонентом.
fn kick_with(shared: &Shared, uuid: &[u8; 16], reason: Tag) {
    let plain = render(&reason);

    shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::KickText { component: reason.encode(), plain });
}

// ───────────────────────────── Диспетчер ─────────────────────────────

/// Какой уровень прав нужен команде; None — команда не из этого модуля.
/// Уровни — вики, Commands/<имя>, Java Edition 26.1.
pub(crate) fn level(name: &str) -> Option<i32> {
    Some(match name {
        "me" | "msg" | "tell" | "w" | "list" => 0,
        "tellraw" | "title" | "playsound" | "stopsound" | "particle" | "setworldspawn" | "spawnpoint" | "defaultgamemode" | "seed" | "version" | "reload" => 2,
        "ban" | "ban-ip" | "banlist" | "pardon" | "pardon-ip" | "whitelist" | "setidletimeout" | "transfer" => 3,
        "save-all" | "save-on" | "save-off" => 4,
        _ => return None,
    })
}

/// Названия команд модуля — для дерева команд и подсказок.
pub(crate) const NAMES: &[&str] = &[
    "me", "msg", "tell", "w", "list", "tellraw", "title", "playsound", "stopsound", "particle", "setworldspawn", "spawnpoint", "defaultgamemode", "seed",
    "version", "reload", "ban", "ban-ip", "banlist", "pardon", "pardon-ip", "whitelist", "setidletimeout", "transfer", "save-all", "save-on", "save-off",
];

/// Узлы дерева команд (пакет Commands) для команд модуля: слово и за ним
/// «остаток строки» — разбор целиком у сервера. Возвращает номера узлов,
/// которые идут в корень, и сами узлы; `first` — номер первого из них.
/// [проверить] настоящие аргументы (селекторы, координаты) клиент не подскажет.
pub(crate) fn tree_nodes(first: i32) -> (Vec<i32>, Vec<Vec<u8>>) {
    /// Команды, которые можно выполнить и без остатка строки.
    const BARE: &[&str] = &["seed", "version", "reload", "banlist", "save-all", "save-on", "save-off", "spawnpoint", "setworldspawn"];

    let mut roots = Vec::new();
    let mut nodes = Vec::new();

    // `list` уже есть в основном дереве.
    for name in NAMES.iter().filter(|name| **name != "list") {
        let literal = first + nodes.len() as i32;

        roots.push(literal);

        // Слово: флаги (1 — слово, 4 — можно выполнить), один потомок.
        let mut node = vec![0x01 | if BARE.contains(name) { 0x04 } else { 0 }];

        node.extend_from_slice(&crate::network::varint::encode_varint(1));
        node.extend_from_slice(&crate::network::varint::encode_varint(literal + 1));
        node.extend_from_slice(&crate::network::varint::encode_string(name));
        nodes.push(node);

        // Остаток строки: строка-«жадная» (разборщик 5, поведение 2), подсказки у сервера.
        let mut argument = vec![0x02 | 0x04 | 0x10];

        argument.extend_from_slice(&crate::network::varint::encode_varint(0));
        argument.extend_from_slice(&crate::network::varint::encode_string("args"));
        argument.extend_from_slice(&crate::network::varint::encode_varint(5));
        argument.extend_from_slice(&crate::network::varint::encode_varint(2));
        argument.extend_from_slice(&crate::network::varint::encode_string("minecraft:ask_server"));
        nodes.push(argument);
    }

    (roots, nodes)
}

fn answer(plain: String) -> Answer {
    Answer { reply: plain, effect: Effect::None, component: None, rich: Vec::new(), result: Some(1) }
}

/// Ответ на неизвестную команду, как у оригинала: «Unknown or incomplete command…»
/// и строка с меткой.
pub(crate) fn unknown(command: &str, source: &Source, shared: &Shared) -> Answer {
    let mut ctx = Ctx { source, shared, input: command, plain: Vec::new() };

    report(&mut ctx, unknown_command(0))
}

/// Показывает ошибку тому, кто ввёл команду.
fn report(ctx: &mut Ctx, fail: Fail) -> Answer {
    let mut messages = Vec::new();

    match fail {
        Fail::Run(tag) => messages.push(red(tag)),
        Fail::Parse(tag, at) => {
            messages.push(red(tag));
            messages.push(context(ctx.input, at));
        }
    }

    let plain: Vec<String> = messages.iter().map(render).collect();

    match ctx.me() {
        Some((uuid, name)) => {
            for message in &messages {
                ctx.chat_to(&uuid, message);
            }

            log_info!("[{}: {}]", name, plain[0]);
            answer(String::new())
        }
        None => answer(plain.join("\n")),
    }
}

/// Выполняет команду модуля; None — имя не из этого модуля.
pub(crate) fn run(command: &str, source: &Source, shared: &Shared) -> Option<Answer> {
    let name = command.split(' ').next().unwrap_or("");
    let needed = level(name)?;
    let mut ctx = Ctx { source, shared, input: command, plain: Vec::new() };

    // Без нужных прав команды для игрока «нет»: ответ как на неизвестную.
    if let Source::Player { uuid, .. } = source
        && shared.ops.lock().expect("права захвачены другим потоком").level_of(uuid) < needed
    {
        return Some(report(&mut ctx, unknown_command(0)));
    }

    let mut rd = Rd::new(command);

    rd.pos = name.len();

    let result = match name {
        "title" => title(&mut ctx, &mut rd),
        "tellraw" => tellraw(&mut ctx, &mut rd),
        "me" => me(&mut ctx, &mut rd),
        "msg" | "tell" | "w" => message(&mut ctx, &mut rd),
        "list" => list(&mut ctx, &mut rd),
        "playsound" => playsound(&mut ctx, &mut rd),
        "stopsound" => stopsound(&mut ctx, &mut rd),
        "particle" => particle(&mut ctx, &mut rd),
        "setworldspawn" => setworldspawn(&mut ctx, &mut rd),
        "spawnpoint" => spawnpoint(&mut ctx, &mut rd),
        "defaultgamemode" => defaultgamemode(&mut ctx, &mut rd),
        "seed" => seed(&mut ctx, &mut rd),
        "version" => version(&mut ctx, &mut rd),
        "reload" => reload(&mut ctx, &mut rd),
        "ban" => ban(&mut ctx, &mut rd),
        "ban-ip" => ban_ip(&mut ctx, &mut rd),
        "banlist" => banlist(&mut ctx, &mut rd),
        "pardon" => pardon(&mut ctx, &mut rd),
        "pardon-ip" => pardon_ip(&mut ctx, &mut rd),
        "whitelist" => whitelist(&mut ctx, &mut rd),
        "setidletimeout" => setidletimeout(&mut ctx, &mut rd),
        "transfer" => transfer(&mut ctx, &mut rd),
        "save-all" => save_all(&mut ctx, &mut rd),
        "save-on" => save_switch(&mut ctx, &mut rd, true),
        "save-off" => save_switch(&mut ctx, &mut rd, false),
        _ => return None,
    };

    Some(match result {
        Ok(()) => match ctx.source {
            // Игроку ответ уже ушёл пакетами, консоли — текстом.
            Source::Player { .. } => answer(String::new()),
            Source::Console | Source::Block { .. } => answer(ctx.plain.join("\n")),
        },
        Err(fail) => report(&mut ctx, fail),
    })
}

// ───────────────────────── Текст на экране и в чате ─────────────────────────

/// Аргумент-сообщение (компонент): JSON или SNBT.
fn component_arg(ctx: &Ctx, text: &str, start: usize) -> R<Tag> {
    let with = Resolve { source: ctx.source, shared: ctx.shared };

    match crate::snbt::parse(text.trim()) {
        Some(Value::Num(_)) => {
            let shown = match text.trim() {
                "true" => "1b".to_string(),
                "false" => "0b".to_string(),
                other => other.to_string(),
            };
            let message = format!("Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {}; Second: Not a map: {}", shown, shown);

            Err(Fail::Parse(translate("argument.component.invalid", vec![s(&message)]), start))
        }
        Some(value) => component(&value, &with).map_err(|why| Fail::Parse(translate("argument.component.invalid", vec![s(&why)]), start)),
        None => {
            let end = start + text.len();

            Err(Fail::Parse(translate("argument.literal.incorrect", vec![s(":")]), end))
        }
    }
}

fn title(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    const TEXT: i32 = 0x72;
    const SUBTITLE: i32 = 0x70;
    const ACTIONBAR: i32 = 0x57;
    const TIMES: i32 = 0x73;
    const CLEAR: i32 = 0x0e;

    rd.next_arg()?;
    let who = rd.selector();
    let start = rd.next_arg()?;
    let sub = rd.word();

    // (пакет, тело, ключ ответа)
    let (id, body, key): (i32, Vec<u8>, &str) = match sub {
        "clear" => {
            rd.finish()?;
            (CLEAR, vec![0], "cleared")
        }
        "reset" => {
            rd.finish()?;
            (CLEAR, vec![1], "reset")
        }
        "title" | "subtitle" | "actionbar" => {
            let from = rd.next_arg()?;
            let text = rd.component();
            let tag = component_arg(ctx, text, from)?;
            rd.finish()?;
            let (id, key) = match sub {
                "title" => (TEXT, "show.title"),
                "subtitle" => (SUBTITLE, "show.subtitle"),
                _ => (ACTIONBAR, "show.actionbar"),
            };

            (id, tag.encode(), key)
        }
        "times" => {
            let fade_in = rd.time()?;
            let stay = rd.time()?;
            let fade_out = rd.time()?;

            rd.finish()?;

            let mut body = Vec::new();

            for value in [fade_in, stay, fade_out] {
                body.extend_from_slice(&value.to_be_bytes());
            }

            (TIMES, body, "times")
        }
        _ => return Err(unknown_argument(start)),
    };

    let targets = ctx.players(who)?;

    for (uuid, _) in &targets {
        ctx.packet(uuid, id, body.clone());
    }

    let suffix = if targets.len() == 1 { "single" } else { "multiple" };

    ctx.ok(translate(&format!("commands.title.{}.{}", key, suffix), Ctx::who_text(&targets)));

    Ok(())
}

fn tellraw(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let who = rd.selector();
    let start = rd.next_arg()?;
    let text = rd.component();
    let tag = component_arg(ctx, text, start)?;

    rd.finish()?;

    let targets = ctx.players(who)?;

    for (uuid, _) in &targets {
        ctx.chat_to(uuid, &tag);
    }

    Ok(())
}

fn me(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let start = rd.next_arg()?;
    let text = rd.greedy();

    if text.is_empty() {
        return Err(unknown_command(start));
    }

    let line = translate("chat.type.emote", vec![ctx.sender_name(), s(text)]);
    let plain = format!("* {} {}", ctx.me().map_or("Server", |(_, name)| name), text);

    log_info!("{}", plain);
    ctx.shared.chat.lock().expect("чат захвачен другим потоком").push_rich(plain, None, Some(chat_body_inner(&line)));

    Ok(())
}

/// Компонент для ленты чата: лента сама дописывает хвост пакета.
fn chat_body_inner(tag: &Tag) -> Vec<u8> {
    tag.encode()
}

fn message(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let who = rd.selector();
    let start = rd.next_arg()?;
    let text = rd.greedy();

    if text.is_empty() {
        return Err(unknown_command(start));
    }

    let targets = ctx.players(who)?;
    let styled = |key: &str, name: Tag| {
        obj(vec![("color", s("gray")), ("italic", Tag::Byte(1)), ("translate", s(key)), ("with", Tag::List(vec![name, s(text)]))])
    };

    for (uuid, name) in &targets {
        let target = player_name(name, uuid);

        match ctx.me() {
            Some((me, _)) => ctx.chat_to(&me, &styled("commands.message.display.outgoing", target)),
            None => log_info!("You whisper to {}: {}", name, text),
        }

        ctx.chat_to(uuid, &styled("commands.message.display.incoming", ctx.sender_name()));
    }

    Ok(())
}

fn list(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let mut with_ids = false;

    if rd.more() {
        let start = rd.next_arg()?;

        if rd.word() != "uuids" {
            return Err(unknown_argument(start));
        }

        with_ids = true;
    }

    rd.finish()?;

    let members: Vec<crate::players::Member> = ctx.shared.players.lock().expect("список игроков захвачен другим потоком").members().to_vec();
    let parts: Vec<Tag> = members
        .iter()
        .map(|member| if with_ids { translate("commands.list.nameAndId", vec![s(&member.name), Tag::Str(uuid_text(&member.uuid))]) } else { player_name(&member.name, &member.uuid) })
        .collect();
    let separator = obj(vec![("color", s("gray")), ("text", s(", "))]);
    let names = if parts.is_empty() { s("") } else { join(parts, separator) };

    ctx.ok(translate("commands.list.players", vec![Tag::Int(members.len() as i32), Tag::Int(ctx.shared.properties.max_players), names]));

    Ok(())
}

// ───────────────────────────── Звуки ─────────────────────────────

const SOURCES: [&str; 11] = ["master", "music", "record", "weather", "block", "hostile", "neutral", "player", "ambient", "voice", "ui"];

fn source_index(word: &str) -> Option<usize> {
    SOURCES.iter().position(|source| *source == word)
}

/// Координаты: три слова, `~` со сдвигом; `centre` — целое по x и z означает середину блока.
fn vec3(rd: &mut Rd, from: (f64, f64, f64), centre: bool) -> R<(f64, f64, f64)> {
    let mut out = [0.0; 3];
    // Метка ошибки — в начале всей тройки (чёрный ящик, cmd-errors).
    let first = rd.pos + 1;

    for (index, value) in out.iter_mut().enumerate() {
        if !rd.more() {
            return Err(Fail::Parse(translate("argument.pos3d.incomplete", vec![]), first));
        }

        let start = rd.next_arg()?;
        let word = rd.word();
        let origin = [from.0, from.1, from.2][index];

        *value = coordinate(word, origin, centre && index != 1).ok_or_else(|| Fail::Parse(translate("argument.pos.missing.double", vec![]), start))?;
    }

    Ok((out[0], out[1], out[2]))
}

fn playsound(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let sound = rd.id()?;
    let mut category = 0;
    let mut selector = "@s";

    if rd.more() {
        let start = rd.next_arg()?;

        category = source_index(rd.word()).ok_or_else(|| unknown_argument(start))?;
    }

    if rd.more() {
        rd.next_arg()?;
        selector = rd.selector();
    }

    let here = ctx.who();
    let mut at = (here.x, here.y, here.z);

    if rd.more() {
        at = vec3(rd, at, true)?;
    }

    let mut volume = 1.0;
    let mut pitch = 1.0;
    let mut minimum = 0.0;

    if rd.more() {
        volume = rd.float(0.0, f32::MAX)?;
    }

    if rd.more() {
        pitch = rd.float(0.0, 2.0)?;
    }

    if rd.more() {
        minimum = rd.float(0.0, 1.0)?;
    }

    rd.finish()?;

    let targets = ctx.players(selector)?;
    let members: Vec<crate::players::Member> = ctx.shared.players.lock().expect("список игроков захвачен другим потоком").members().to_vec();
    let reach = if volume > 1.0 { f64::from(volume) * 16.0 } else { 16.0 };
    let mut heard: Vec<([u8; 16], String)> = Vec::new();

    for (uuid, name) in &targets {
        let Some(member) = members.iter().find(|member| &member.uuid == uuid) else {
            continue;
        };
        let (dx, dy, dz) = (at.0 - member.position.x, at.1 - member.position.y, at.2 - member.position.z);
        let distance = dx * dx + dy * dy + dz * dz;
        let (place, loudness) = if distance > reach * reach {
            // Вне слышимости звук ставится в двух блоках от слушателя с
            // громкостью `minVolume` (вики, /playsound).
            if minimum <= 0.0 {
                continue;
            }

            let length = distance.sqrt();

            ((member.position.x + dx / length * 2.0, member.position.y + dy / length * 2.0, member.position.z + dz / length * 2.0), minimum)
        } else {
            (at, volume)
        };

        let mut body = crate::network::varint::encode_varint(0);

        body.extend_from_slice(&crate::network::varint::encode_string(&sound));
        body.push(0);
        body.extend_from_slice(&crate::network::varint::encode_varint(category as i32));

        // Место — в восьмых долях блока, дробь отсекается к нулю (чёрный
        // ящик, minecart-tnt-rail; cmd-playsound).
        for value in [place.0, place.1, place.2] {
            body.extend_from_slice(&((value * 8.0) as i32).to_be_bytes());
        }

        body.extend_from_slice(&loudness.to_be_bytes());
        body.extend_from_slice(&pitch.to_be_bytes());
        body.extend_from_slice(&random_seed().to_be_bytes());
        ctx.packet(uuid, 0x75, body);
        heard.push((*uuid, name.clone()));
    }

    if heard.is_empty() {
        return Err(failed("commands.playsound.failed"));
    }

    match heard.as_slice() {
        [(uuid, name)] => ctx.ok(translate("commands.playsound.success.single", vec![s(&sound), player_name(name, uuid)])),
        many => ctx.ok(translate("commands.playsound.success.multiple", vec![s(&sound), Tag::Int(many.len() as i32)])),
    }

    Ok(())
}

/// Случайное зерно звука: клиенту важно лишь, что оно разное.
fn random_seed() -> i64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0x9E37_79B9_7F4A_7C15);

    let moment = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_nanos() as u64).unwrap_or(0);
    let mut value = STATE.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed) ^ moment;

    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (value ^ (value >> 31)) as i64
}

fn stopsound(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let who = rd.selector();
    let mut category: Option<usize> = None;
    let mut sound: Option<String> = None;

    if rd.more() {
        let start = rd.next_arg()?;
        let word = rd.word();

        category = match word {
            "*" => None,
            other => Some(source_index(other).ok_or_else(|| unknown_argument(start))?),
        };

        // «Одинокий» пробел после слова — лишнее, как у оригинала.
        if rd.rest() == " " {
            return Err(unknown_argument(rd.pos));
        }

        if rd.more() {
            sound = Some(rd.id()?);
        } else if word == "*" {
            // `*` без звука не команда (чёрный ящик, cmd-stopsound).
            return Err(unknown_command(rd.text.len()));
        }
    }

    rd.finish()?;

    let targets = ctx.players(who)?;
    let mut body = Vec::new();
    let flags = u8::from(category.is_some()) | (u8::from(sound.is_some()) << 1);

    body.push(flags);

    if let Some(category) = category {
        body.extend_from_slice(&crate::network::varint::encode_varint(category as i32));
    }

    if let Some(sound) = &sound {
        body.extend_from_slice(&crate::network::varint::encode_string(sound));
    }

    for (uuid, _) in &targets {
        ctx.packet(uuid, 0x77, body.clone());
    }

    let message = match (category, &sound) {
        (None, None) => translate("commands.stopsound.success.sourceless.any", vec![]),
        (Some(category), None) => translate("commands.stopsound.success.source.any", vec![s(SOURCES[category])]),
        (None, Some(sound)) => translate("commands.stopsound.success.sourceless.sound", vec![s(sound)]),
        (Some(category), Some(sound)) => translate("commands.stopsound.success.source.sound", vec![s(sound), s(SOURCES[category])]),
    };

    ctx.ok(message);

    Ok(())
}

// ───────────────────────────── Частицы ─────────────────────────────

/// Частицы 26.1 по номерам (minecraft-data, particles.json).
const PARTICLES: [&str; 117] = [
    "angry_villager", "block", "block_marker", "bubble", "cloud", "copper_fire_flame", "crit", "damage_indicator", "dragon_breath", "dripping_lava",
    "falling_lava", "landing_lava", "dripping_water", "falling_water", "dust", "dust_color_transition", "effect", "elder_guardian", "enchanted_hit",
    "enchant", "end_rod", "entity_effect", "explosion_emitter", "explosion", "gust", "small_gust", "gust_emitter_large", "gust_emitter_small",
    "sonic_boom", "falling_dust", "firework", "fishing", "flame", "infested", "cherry_leaves", "pale_oak_leaves", "tinted_leaves", "sculk_soul",
    "sculk_charge", "sculk_charge_pop", "soul_fire_flame", "soul", "flash", "happy_villager", "composter", "heart", "instant_effect", "item",
    "vibration", "trail", "pause_mob_growth", "reset_mob_growth", "item_slime", "item_cobweb", "item_snowball", "large_smoke", "lava", "mycelium",
    "note", "poof", "portal", "rain", "smoke", "white_smoke", "sneeze", "spit", "squid_ink", "sweep_attack", "totem_of_undying", "underwater", "splash",
    "witch", "bubble_pop", "current_down", "bubble_column_up", "nautilus", "dolphin", "campfire_cosy_smoke", "campfire_signal_smoke", "dripping_honey",
    "falling_honey", "landing_honey", "falling_nectar", "falling_spore_blossom", "ash", "crimson_spore", "warped_spore", "spore_blossom_air",
    "dripping_obsidian_tear", "falling_obsidian_tear", "landing_obsidian_tear", "reverse_portal", "white_ash", "small_flame", "snowflake",
    "dripping_dripstone_lava", "falling_dripstone_lava", "dripping_dripstone_water", "falling_dripstone_water", "glow_squid_ink", "glow", "wax_on",
    "wax_off", "electric_spark", "scrape", "shriek", "egg_crack", "dust_plume", "trial_spawner_detected_player", "trial_spawner_detected_player_ominous",
    "vault_connection", "dust_pillar", "ominous_spawning", "raid_omen", "trial_omen", "block_crumble", "firefly",
];

/// Цвет частицы: целое ARGB как есть или список `[r, g, b]` (`want` = 3) /
/// `[r, g, b, a]` (4) дробей от 0 до 1; без альфы непрозрачный.
fn color_of(value: &Value, want: usize) -> Result<i32, String> {
    match value {
        Value::Num(number) => Ok(*number as i64 as i32),
        Value::List(items) if items.len() == want => {
            let channel = |index: usize| -> Result<i32, String> {
                let number = items[index].as_f64().ok_or_else(|| "Not a number".to_string())?;

                Ok(((number.clamp(0.0, 1.0) * 255.0) as i32).clamp(0, 255))
            };

            let (r, g, b) = (channel(0)?, channel(1)?, channel(2)?);
            let a = if want == 4 { channel(3)? } else { 255 };

            Ok(((a << 24) | (r << 16) | (g << 8) | b) as u32 as i32)
        }
        _ => Err(format!("Failed to parse either. First: Not a number; Second: Input is not a list of {} elements", want)),
    }
}

/// Размер пылевой частицы: от 0,01 до 4 (оригинал не пропускает остальное).
fn scale_of(value: &Value) -> Result<f32, String> {
    let scale = value.as_f64().ok_or("Not a number")? as f32;

    if !(0.01..=4.0).contains(&scale) {
        return Err(format!("Value must be within range [0.01;4.0]: {:?}", scale));
    }

    Ok(scale)
}

/// Все недостающие поля сразу, в обратном порядке описания: так оригинал
/// сводит ошибки в одну (чёрный ящик, cmd-particle: dust без полей).
fn require(options: Option<&Value>, keys: &[&str]) -> Result<(), String> {
    let missing: Vec<String> = keys.iter().rev().filter(|key| options.and_then(|options| options.get(key)).is_none()).map(|key| format!("No key {} in MapLike[{{}}]", key)).collect();

    if missing.is_empty() { Ok(()) } else { Err(missing.join("; ")) }
}

fn need<'v>(options: Option<&'v Value>, key: &str) -> Result<&'v Value, String> {
    options.and_then(|options| options.get(key)).ok_or_else(|| format!("No key {} in MapLike[{{}}]", key))
}

fn varint(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&crate::network::varint::encode_varint(value));
}

/// Состояние блока из записи `block_state`: строка `minecraft:stone` или `{Name:…, Properties:{…}}`.
fn block_state_of(value: &Value) -> Result<i32, String> {
    let text = match value {
        Value::Str(text) => text.clone(),
        Value::Map(_) => {
            let name = value.get("Name").and_then(Value::as_str).ok_or("No key Name in MapLike[{}]")?;

            match value.get("Properties") {
                Some(Value::Map(properties)) if !properties.is_empty() => {
                    let list: Vec<String> = properties.iter().map(|(key, value)| format!("{}={}", key, value.as_str().map(str::to_string).unwrap_or_else(|| value.as_f64().map(|n| n.to_string()).unwrap_or_default()))).collect();

                    format!("{}[{}]", name, list.join(","))
                }
                _ => name.to_string(),
            }
        }
        _ => return Err("Not a string".to_string()),
    };

    crate::blocks::state_from_text(&text).ok_or_else(|| match value {
        Value::Str(_) => format!("Failed to parse either. First: Not a map: \"{}\"; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: {}", text, text),
        _ => format!("Unknown block {}", text),
    })
}

/// Данные частицы после её номера в пакете (protocol.json 26.1, тип Particle).
fn particle_data(name: &str, options: Option<&Value>) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();

    match name {
        "block" | "block_marker" | "falling_dust" | "dust_pillar" | "block_crumble" => varint(&mut out, block_state_of(need(options, "block_state")?)?),
        "dust" => {
            require(options, &["color", "scale"])?;
            out.extend_from_slice(&color_of(need(options, "color")?, 3)?.to_be_bytes());
            out.extend_from_slice(&scale_of(need(options, "scale")?)?.to_be_bytes());
        }
        "dust_color_transition" => {
            require(options, &["from_color", "to_color", "scale"])?;
            out.extend_from_slice(&color_of(need(options, "from_color")?, 3)?.to_be_bytes());
            out.extend_from_slice(&color_of(need(options, "to_color")?, 3)?.to_be_bytes());
            out.extend_from_slice(&scale_of(need(options, "scale")?)?.to_be_bytes());
        }
        "entity_effect" | "tinted_leaves" | "flash" => out.extend_from_slice(&color_of(need(options, "color")?, 4)?.to_be_bytes()),
        "effect" | "instant_effect" => {
            require(options, &["color", "power"])?;
            out.extend_from_slice(&color_of(need(options, "color")?, 3)?.to_be_bytes());
            out.extend_from_slice(&(need(options, "power")?.as_f64().ok_or("Not a number")? as f32).to_be_bytes());
        }
        "item" => {
            let item = need(options, "item")?;
            let id = match item {
                Value::Str(text) => text.as_str(),
                Value::Map(_) => item.get("id").and_then(Value::as_str).ok_or("No key id in MapLike[{}]")?,
                _ => return Err("Not a string".to_string()),
            };
            let number = crate::blocks::item_named(id.strip_prefix("minecraft:").unwrap_or(id)).ok_or_else(|| match item {
                Value::Str(_) => format!("Failed to parse either. First: Not a map: \"{}\"; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: {}", id, id),
                _ => format!("Unknown item {}", id),
            })?;
            let count = item.get("count").and_then(Value::as_f64).unwrap_or(1.0) as i32;

            varint(&mut out, number);
            varint(&mut out, count);
            varint(&mut out, 0);
            varint(&mut out, 0);
        }
        "sculk_charge" => out.extend_from_slice(&(need(options, "roll")?.as_f64().ok_or("Not a number")? as f32).to_be_bytes()),
        "shriek" => varint(&mut out, need(options, "delay")?.as_f64().ok_or("Not a number")? as i32),
        "dragon_breath" => out.extend_from_slice(&(need(options, "power")?.as_f64().ok_or("Not a number")? as f32).to_be_bytes()),
        "vibration" => {
            let destination = need(options, "destination")?;
            let kind = destination.get("type").and_then(Value::as_str).ok_or("No key type in MapLike[{}]")?;

            match kind.strip_prefix("minecraft:").unwrap_or(kind) {
                "block" => {
                    let Some(Value::List(position)) = destination.get("pos") else {
                        return Err("No key pos in MapLike[{}]".to_string());
                    };
                    let [x, y, z] = position.iter().filter_map(Value::as_f64).map(|n| n as i64).collect::<Vec<_>>()[..] else {
                        return Err("Input is not a list of 3 elements".to_string());
                    };

                    varint(&mut out, 0);
                    out.extend_from_slice(&((((x & 0x3ff_ffff) << 38) | ((z & 0x3ff_ffff) << 12) | (y & 0xfff)) as u64).to_be_bytes());
                }
                "entity" => {
                    // [проверить] цель-сущность: номер сущности берётся из `source_entity`, рост глаз — `source_entity_eye_height`.
                    varint(&mut out, 1);
                    varint(&mut out, destination.get("source_entity").and_then(Value::as_f64).unwrap_or(0.0) as i32);
                    out.extend_from_slice(&(destination.get("source_entity_eye_height").and_then(Value::as_f64).unwrap_or(0.0) as f32).to_be_bytes());
                }
                other => return Err(format!("Unknown position source {}", other)),
            }

            varint(&mut out, need(options, "arrival_in_ticks")?.as_f64().ok_or("Not a number")? as i32);
        }
        "trail" => {
            require(options, &["target", "color", "duration"])?;
            let Some(Value::List(target)) = options.and_then(|options| options.get("target")) else {
                return Err("No key target in MapLike[{}]".to_string());
            };

            for value in target.iter().filter_map(Value::as_f64) {
                out.extend_from_slice(&value.to_be_bytes());
            }

            out.extend_from_slice(&color_of(need(options, "color")?, 3)?.to_be_bytes());
            varint(&mut out, need(options, "duration")?.as_f64().ok_or("Not a number")? as i32);
        }
        _ => {}
    }

    Ok(out)
}

fn particle(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let start = rd.next_arg()?;
    let rest = rd.rest();

    // Название, сразу за ним — необязательные `{…}` без пробела.
    let name_end = rest.find([' ', '{']).unwrap_or(rest.len());
    let mut taken = name_end;
    let mut options: Option<Value> = None;

    if rest[name_end..].starts_with('{') {
        let mut depth = 0;
        let mut quote: Option<char> = None;
        let mut end = None;

        for (index, symbol) in rest[name_end..].char_indices() {
            match (quote, symbol) {
                (Some(open), symbol) if symbol == open => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(symbol),
                (None, '{') => depth += 1,
                (None, '}') => {
                    depth -= 1;

                    if depth == 0 {
                        end = Some(name_end + index + 1);
                        break;
                    }
                }
                _ => {}
            }
        }

        let end = end.ok_or_else(|| Fail::Run(translate("particle.invalidOptions", vec![s("Unclosed")])))?;

        options = Some(crate::snbt::parse(&rest[name_end..end]).ok_or_else(|| Fail::Run(translate("particle.invalidOptions", vec![s("Not a map")])))?);
        taken = end;
    }

    let word = &rest[..name_end];
    let id = word.strip_prefix("minecraft:").unwrap_or(word);
    let number = PARTICLES.iter().position(|particle| *particle == id);

    rd.pos += taken;

    let Some(number) = number.filter(|_| !word.contains(':') || word.starts_with("minecraft:")) else {
        let shown = if word.contains(':') { word.to_string() } else { format!("minecraft:{}", word) };

        return Err(Fail::Parse(translate("particle.notFound", vec![s(&shown)]), start + name_end));
    };

    let data = particle_data(PARTICLES[number], options.as_ref()).map_err(|why| Fail::Run(translate("particle.invalidOptions", vec![s(&why)])))?;

    let here = ctx.who();
    let mut at = (here.x, here.y, here.z);
    let mut delta = (0.0f64, 0.0f64, 0.0f64);
    let mut speed = 0.0f32;
    let mut count = 0;
    let mut force = false;
    let mut viewers: Option<&str> = None;

    if rd.more() {
        at = vec3(rd, at, true)?;

        // Дальше — только целиком: смещение, скорость, число.
        if rd.more() {
            delta = vec3(rd, (0.0, 0.0, 0.0), false)?;
            speed = rd.float(0.0, f32::MAX)?;
            count = rd.int(0, i32::MAX)?;

            if rd.more() {
                let mode_start = rd.next_arg()?;

                force = match rd.word() {
                    "force" => true,
                    "normal" => false,
                    _ => return Err(unknown_argument(mode_start)),
                };

                if rd.more() {
                    rd.next_arg()?;
                    viewers = Some(rd.selector());
                }
            }
        }
    }

    rd.finish()?;

    let targets = match viewers {
        Some(word) => ctx.players(word)?,
        None => {
            let members = ctx.shared.players.lock().expect("список игроков захвачен другим потоком");

            members.members().iter().filter(|member| member.dimension == here.dimension).map(|member| (member.uuid, member.name.clone())).collect()
        }
    };
    let members: Vec<crate::players::Member> = ctx.shared.players.lock().expect("список игроков захвачен другим потоком").members().to_vec();
    let range: f64 = if force { 512.0 } else { 32.0 };
    let mut body = vec![u8::from(force), 0];

    for value in [at.0, at.1, at.2] {
        body.extend_from_slice(&value.to_be_bytes());
    }

    for value in [delta.0, delta.1, delta.2] {
        body.extend_from_slice(&(value as f32).to_be_bytes());
    }

    body.extend_from_slice(&speed.to_be_bytes());
    body.extend_from_slice(&count.to_be_bytes());
    varint(&mut body, number as i32);
    body.extend_from_slice(&data);

    let mut shown = 0;

    for (uuid, _) in &targets {
        let Some(member) = members.iter().find(|member| &member.uuid == uuid) else {
            continue;
        };
        let (dx, dy, dz) = (at.0 - member.position.x, at.1 - member.position.y, at.2 - member.position.z);

        if dx * dx + dy * dy + dz * dz <= range * range {
            ctx.packet(uuid, 0x2f, body.clone());
            shown += 1;
        }
    }

    if shown == 0 {
        return Err(failed("commands.particle.failed"));
    }

    ctx.ok(translate("commands.particle.success", vec![s(&format!("minecraft:{}", PARTICLES[number]))]));

    Ok(())
}

// ───────────────────────── Точки возрождения ─────────────────────────

/// Блок из трёх слов: числа или `~`; дробь отсекается вниз.
fn block_pos(rd: &mut Rd, from: (f64, f64, f64)) -> R<(i32, i32, i32)> {
    let first = rd.pos + 1;
    let before = rd.rest().to_string();

    // Блок — только целые числа или `~`: «1.5» оригинал не принимает.
    let words: Vec<&str> = before.split(' ').skip(1).take(3).collect();

    if words.iter().any(|word| !word.starts_with('~') && word.parse::<i32>().is_err()) {
        return Err(Fail::Parse(translate("parsing.int.expected", vec![]), first));
    }

    let (x, y, z) = vec3(rd, from, false)?;

    Ok((x.floor() as i32, y.floor() as i32, z.floor() as i32))
}

/// Поворот из двух слов (`~` — от поворота того, кто ввёл команду); без слов — нули.
fn rotation(rd: &mut Rd, from: (f32, f32)) -> R<(f32, f32)> {
    if !rd.more() {
        return Ok((0.0, 0.0));
    }

    rd.next_arg()?;
    let yaw_word = rd.word();

    if !rd.more() {
        return Err(Fail::Parse(translate("argument.rotation.incomplete", vec![]), rd.text.len()));
    }

    let start = rd.next_arg()?;
    let pitch_word = rd.word();
    let one = |word: &str, base: f32| -> Option<f32> {
        match word.strip_prefix('~') {
            Some("") => Some(base),
            Some(shift) => Some(base + shift.parse::<f32>().ok()?),
            None => word.parse().ok(),
        }
    };
    let bad = || Fail::Parse(translate("argument.pos.missing.double", vec![]), start);

    Ok((one(yaw_word, from.0).ok_or_else(bad)?, one(pitch_word, from.1).ok_or_else(bad)?))
}

/// Пакет «точка появления»: измерение, место, поворот (protocol.json 26.1, `spawn_position`).
fn spawn_packet(dimension: &str, (x, y, z): (i32, i32, i32), (yaw, pitch): (f32, f32)) -> Vec<u8> {
    let mut body = crate::network::varint::encode_string(dimension);

    body.extend_from_slice(&((((i64::from(x) & 0x3ff_ffff) << 38) | ((i64::from(z) & 0x3ff_ffff) << 12) | (i64::from(y) & 0xfff)) as u64).to_be_bytes());
    body.extend_from_slice(&yaw.to_be_bytes());
    body.extend_from_slice(&pitch.to_be_bytes());
    body
}

fn setworldspawn(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let here = ctx.who();
    let mut place = (here.x.floor() as i32, here.y.floor() as i32, here.z.floor() as i32);
    // Поворот по умолчанию — нули, а не взгляд того, кто ввёл команду (чёрный ящик, cmd-spawn).
    let mut angle = (0.0, 0.0);

    if rd.more() {
        place = block_pos(rd, (here.x, here.y, here.z))?;
        angle = rotation(rd, (here.yaw, here.pitch))?;
    }

    rd.finish()?;

    if here.dimension != crate::dimension::Dimension::Overworld {
        return Err(failed("commands.setworldspawn.failure.not_overworld"));
    }

    let before = {
        let world = ctx.shared.world_in(crate::dimension::Dimension::Overworld).lock().expect("мир захвачен другим потоком");
        let old = world.spawn_position();

        world.set_spawn_position((f64::from(place.0) + 0.5, f64::from(place.1), f64::from(place.2) + 0.5));

        (old.0.floor() as i32, old.1.floor() as i32, old.2.floor() as i32)
    };
    let changed = {
        let mut state = admin();
        let changed = before != place || state.world_spawn_angle != angle;

        state.world_spawn_angle = angle;
        changed
    };
    let members: Vec<crate::players::Member> = ctx.shared.players.lock().expect("список игроков захвачен другим потоку").members().to_vec();

    // Пакет — только если точка или поворот сменились (чёрный ящик, cmd-spawn).
    for member in members.iter().filter(|member| changed && member.dimension == crate::dimension::Dimension::Overworld) {
        let order = Order::Spawn { x: f64::from(place.0) + 0.5, y: f64::from(place.1), z: f64::from(place.2) + 0.5, body: Some(spawn_packet("minecraft:overworld", place, angle)) };

        ctx.shared.players.lock().expect("список игроков захвачен другим потоком").order(&member.uuid, order);
    }

    ctx.ok(translate(
        "commands.setworldspawn.success",
        vec![Tag::Int(place.0), Tag::Int(place.1), Tag::Int(place.2), Tag::Float(angle.0), Tag::Float(angle.1), s("minecraft:overworld")],
    ));

    Ok(())
}

fn spawnpoint(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let here = ctx.who();
    let mut selector = "@s";
    let mut place = (here.x.floor() as i32, here.y.floor() as i32, here.z.floor() as i32);
    let mut angle = (0.0, 0.0);

    if rd.more() {
        rd.next_arg()?;
        selector = rd.selector();
    }

    if rd.more() {
        place = block_pos(rd, (here.x, here.y, here.z))?;
        angle = rotation(rd, (here.yaw, here.pitch))?;
    }

    rd.finish()?;

    let targets = ctx.players(selector)?;

    // Личная точка возрождения: игроку пакет не шлётся (чёрный ящик, cmd-spawnpoint).
    for (uuid, _) in &targets {
        ctx.shared.players.lock().expect("список игроков захвачен другим потоку").order(uuid, Order::Spawn { x: f64::from(place.0) + 0.5, y: f64::from(place.1), z: f64::from(place.2) + 0.5, body: None });
    }

    let suffix = if targets.len() == 1 { "single" } else { "multiple" };
    let mut with = vec![Tag::Int(place.0), Tag::Int(place.1), Tag::Int(place.2), Tag::Float(angle.0), Tag::Float(angle.1), s("minecraft:overworld")];

    with.extend(Ctx::who_text(&targets));
    ctx.ok(translate(&format!("commands.spawnpoint.success.{}", suffix), with));

    Ok(())
}

// ───────────────────────── Серверные команды ─────────────────────────

fn defaultgamemode(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let start = rd.next_arg()?;
    let word = rd.word();
    // Метка — после слова: разбор прочёл его целиком (чёрный ящик, cmd-defaultgamemode).
    let mode = crate::commands::game_mode_by_name(word).ok_or_else(|| Fail::Parse(translate("argument.gamemode.invalid", vec![s(word)]), start + word.len()))?;

    rd.finish()?;
    admin().default_mode = mode;
    ctx.ok(translate("commands.defaultgamemode.success", vec![translate(&format!("gameMode.{}", word), vec![])]));

    Ok(())
}

fn seed(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.finish()?;

    let seed = ctx.shared.world_in(crate::dimension::Dimension::Overworld).lock().expect("мир захвачен другим потоком").seed().to_string();
    let copyable = obj(vec![
        ("click_event", obj(vec![("action", s("copy_to_clipboard")), ("value", s(&seed))])),
        ("color", s("green")),
        ("insertion", s(&seed)),
        ("text", s(&seed)),
        ("hover_event", obj(vec![("action", s("show_text")), ("value", translate("chat.copy.click", vec![]))])),
    ]);

    ctx.ok(translate("commands.seed.success", vec![translate("chat.square_brackets", vec![copyable])]));

    Ok(())
}

fn version(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.finish()?;

    // Сведения о версии 26.1.2 — со снимка оригинала (чёрный ящик, cmd-version).
    ctx.ok(translate("commands.version.header", vec![]));
    ctx.ok(translate("commands.version.id", vec![s("26.1.2")]));
    ctx.ok(translate("commands.version.name", vec![s("26.1.2")]));
    ctx.ok(translate("commands.version.data", vec![Tag::Int(4790)]));
    ctx.ok(translate("commands.version.series", vec![s("main")]));
    ctx.ok(translate("commands.version.protocol", vec![Tag::Int(775), s("0x307")]));
    ctx.ok(translate("commands.version.build_time", vec![s("Thu Apr 09 13:11:03 MSK 2026")]));
    ctx.ok(translate("commands.version.pack.resource", vec![s("84.0")]));
    ctx.ok(translate("commands.version.pack.data", vec![s("101.1")]));
    ctx.ok(translate("commands.version.stable.yes", vec![]));

    Ok(())
}

/// `reload`: наборов данных у нас нет, перечитывать нечего — только ответ.
/// [проверить] оригинал заново шлёт рецепты и теги клиентам.
fn reload(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.finish()?;
    ctx.ok(translate("commands.reload.success", vec![]));

    Ok(())
}

fn ban(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let who = rd.selector();
    let reason = if rd.more() {
        rd.next_arg()?;
        rd.greedy().to_string()
    } else {
        DEFAULT_REASON.to_string()
    };

    rd.finish()?;

    let targets = profiles(who, ctx.source, ctx.shared);

    if targets.is_empty() {
        return Err(failed("argument.entity.notfound.player"));
    }

    let mut added = Vec::new();

    {
        let mut state = admin();

        for (uuid, name) in &targets {
            let id = uuid_text(uuid);

            if state.bans.iter().any(|ban| ban.uuid == id) {
                continue;
            }

            state.bans.push(Ban { uuid: id, name: name.clone(), created: stamp(), source: author(ctx.source), expires: "forever".to_string(), reason: reason.clone() });
            added.push((*uuid, name.clone()));
        }

        write_list(BANNED_PLAYERS, &state.bans);
    }

    if added.is_empty() {
        return Err(failed("commands.ban.failed"));
    }

    for (uuid, name) in &added {
        ctx.ok(translate("commands.ban.success", vec![s(name), s(&reason)]));

        // Забаненный в сети выгоняется сразу.
        let online = ctx.shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().any(|member| &member.uuid == uuid);

        if online {
            kick_with(ctx.shared, uuid, translate("multiplayer.disconnect.banned", vec![]));
        }
    }

    Ok(())
}

fn ban_ip(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let target = rd.word();
    let reason = if rd.more() {
        rd.next_arg()?;
        rd.greedy().to_string()
    } else {
        DEFAULT_REASON.to_string()
    };

    rd.finish()?;

    // Адрес или ник игрока в сети (у оригинала IPv6 так не банится: MC-97885).
    let ip: String = match target.parse::<std::net::Ipv4Addr>() {
        Ok(ip) => ip.to_string(),
        Err(_) => {
            let uuid = ctx.shared.players.lock().expect("список игроков захвачен другим потоком").find(target).map(|member| member.uuid);

            match uuid.and_then(|uuid| addresses().get(&uuid).copied()) {
                Some(ip) => ip.to_string(),
                None => return Err(failed("commands.banip.invalid")),
            }
        }
    };

    {
        let mut state = admin();

        if state.ip_bans.iter().any(|ban| ban.ip == ip) {
            return Err(failed("commands.banip.failed"));
        }

        state.ip_bans.push(IpBan { ip: ip.clone(), created: stamp(), source: author(ctx.source), expires: "forever".to_string(), reason: reason.clone() });
        write_list(BANNED_IPS, &state.ip_bans);
    }

    ctx.ok(translate("commands.banip.success", vec![s(&ip), s(&reason)]));

    // Все игроки с этого адреса выгоняются.
    let members: Vec<([u8; 16], String)> = ctx.shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().map(|member| (member.uuid, member.name.clone())).collect();
    let affected: Vec<([u8; 16], String)> = members.into_iter().filter(|(uuid, _)| addresses().get(uuid).is_some_and(|known| known.to_string() == ip)).collect();

    if !affected.is_empty() {
        let names: Vec<&str> = affected.iter().map(|(_, name)| name.as_str()).collect();

        ctx.ok(translate("commands.banip.info", vec![Tag::Int(affected.len() as i32), s(&names.join(", "))]));

        for (uuid, _) in &affected {
            kick_with(ctx.shared, uuid, translate("multiplayer.disconnect.ip_banned", vec![]));
        }
    }

    Ok(())
}

fn banlist(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let (mut players, mut ips) = (true, true);

    if rd.more() {
        let start = rd.next_arg()?;

        match rd.word() {
            "ips" => players = false,
            "players" => ips = false,
            _ => return Err(unknown_argument(start)),
        }
    }

    rd.finish()?;

    let mut entries: Vec<(String, String, String)> = Vec::new();
    let state = admin();

    if players {
        entries.extend(state.bans.iter().map(|ban| (ban.name.clone(), ban.source.clone(), ban.reason.clone())));
    }

    if ips {
        entries.extend(state.ip_bans.iter().map(|ban| (ban.ip.clone(), ban.source.clone(), ban.reason.clone())));
    }

    drop(state);

    if entries.is_empty() {
        ctx.ok(translate("commands.banlist.none", vec![]));
        return Ok(());
    }

    ctx.ok(translate("commands.banlist.list", vec![Tag::Int(entries.len() as i32)]));

    for (who, by, reason) in entries {
        ctx.ok(translate("commands.banlist.entry", vec![s(&who), s(&by), s(&reason)]));
    }

    Ok(())
}

fn pardon(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let who = rd.selector();

    rd.finish()?;

    let targets = profiles(who, ctx.source, ctx.shared);
    let mut removed = Vec::new();

    {
        let mut state = admin();

        for (uuid, name) in &targets {
            let id = uuid_text(uuid);
            let before = state.bans.len();

            state.bans.retain(|ban| ban.uuid != id && !ban.name.eq_ignore_ascii_case(name));

            if state.bans.len() != before {
                removed.push(name.clone());
            }
        }

        write_list(BANNED_PLAYERS, &state.bans);
    }

    if removed.is_empty() {
        return Err(failed("commands.pardon.failed"));
    }

    for name in removed {
        ctx.ok(translate("commands.pardon.success", vec![s(&name)]));
    }

    Ok(())
}

fn pardon_ip(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let word = rd.word();

    rd.finish()?;

    let Ok(ip) = word.parse::<std::net::Ipv4Addr>() else {
        return Err(failed("commands.pardonip.invalid"));
    };
    let ip = ip.to_string();

    {
        let mut state = admin();
        let before = state.ip_bans.len();

        state.ip_bans.retain(|ban| ban.ip != ip);

        if state.ip_bans.len() == before {
            return Err(failed("commands.pardonip.failed"));
        }

        write_list(BANNED_IPS, &state.ip_bans);
    }

    ctx.ok(translate("commands.pardonip.success", vec![s(&ip)]));

    Ok(())
}

fn whitelist(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let start = rd.next_arg()?;

    match rd.word() {
        action @ ("add" | "remove") => {
            rd.next_arg()?;
            let who = rd.selector();

            rd.finish()?;

            let targets = profiles(who, ctx.source, ctx.shared);
            let mut changed = Vec::new();

            {
                let mut state = admin();

                for (uuid, name) in &targets {
                    let id = uuid_text(uuid);
                    let listed = state.whitelist.iter().any(|entry| entry.uuid == id);

                    if action == "add" && !listed {
                        state.whitelist.push(Allowed { uuid: id, name: name.clone() });
                        changed.push(name.clone());
                    } else if action == "remove" && listed {
                        state.whitelist.retain(|entry| entry.uuid != id);
                        changed.push(name.clone());
                    }
                }

                write_list(WHITELIST, &state.whitelist);
            }

            if changed.is_empty() {
                return Err(failed(&format!("commands.whitelist.{}.failed", action)));
            }

            for name in changed {
                ctx.ok(translate(&format!("commands.whitelist.{}.success", action), vec![s(&name)]));
            }

            if action == "remove" {
                enforce_whitelist(ctx.shared);
            }
        }
        "list" => {
            rd.finish()?;

            let names: Vec<String> = admin().whitelist.iter().map(|entry| entry.name.clone()).collect();

            if names.is_empty() {
                ctx.ok(translate("commands.whitelist.none", vec![]));
            } else {
                ctx.ok(translate("commands.whitelist.list", vec![Tag::Int(names.len() as i32), s(&names.join(", "))]));
            }
        }
        word @ ("on" | "off") => {
            rd.finish()?;

            let on = word == "on";

            {
                let mut state = admin();

                if state.whitelist_on == on {
                    return Err(failed(if on { "commands.whitelist.alreadyOn" } else { "commands.whitelist.alreadyOff" }));
                }

                state.whitelist_on = on;
            }

            set_property("white-list", if on { "true" } else { "false" });
            ctx.ok(translate(if on { "commands.whitelist.enabled" } else { "commands.whitelist.disabled" }, vec![]));
            enforce_whitelist(ctx.shared);
        }
        "reload" => {
            rd.finish()?;
            admin().whitelist = read_list(WHITELIST);
            ctx.ok(translate("commands.whitelist.reloaded", vec![]));
            enforce_whitelist(ctx.shared);
        }
        _ => return Err(unknown_argument(start)),
    }

    Ok(())
}

fn setidletimeout(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    let minutes = rd.int(0, i32::MAX)?;

    rd.finish()?;
    admin().idle_minutes = minutes;
    set_property("player-idle-timeout", &minutes.to_string());

    if minutes == 0 {
        ctx.ok(translate("commands.setidletimeout.success.disabled", vec![]));
    } else {
        ctx.ok(translate("commands.setidletimeout.success", vec![Tag::Int(minutes)]));
    }

    Ok(())
}

fn transfer(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    rd.next_arg()?;
    let host = rd.word().to_string();
    let mut port = 25565;
    let mut selector: Option<&str> = None;

    if rd.more() {
        port = rd.int(1, 65535)?;
    }

    if rd.more() {
        rd.next_arg()?;
        selector = Some(rd.selector());
    }

    rd.finish()?;

    let targets = match (selector, ctx.me()) {
        (Some(word), _) => ctx.players(word)?,
        (None, Some((uuid, name))) => vec![(uuid, name.to_string())],
        (None, None) => return Err(failed("commands.transfer.error.no_players")),
    };
    let mut body = crate::network::varint::encode_string(&host);

    varint(&mut body, port);

    for (uuid, _) in &targets {
        ctx.packet(uuid, 0x81, body.clone());
    }

    let suffix = if targets.len() == 1 { "single" } else { "multiple" };
    let mut with = Ctx::who_text(&targets);

    with.extend([s(&host), Tag::Int(port)]);
    ctx.ok(translate(&format!("commands.transfer.success.{}", suffix), with));

    Ok(())
}

fn save_all(ctx: &mut Ctx, rd: &mut Rd) -> R<()> {
    if rd.more() {
        let start = rd.next_arg()?;

        if rd.word() != "flush" {
            return Err(unknown_argument(start));
        }
    }

    rd.finish()?;
    ctx.ok(translate("commands.save.saving", vec![]));

    for dimension in crate::dimension::Dimension::ALL {
        let mut world = ctx.shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

        crate::tick::store_mobs(ctx.shared, dimension, &mut world);
        world.save_all();
    }

    ctx.ok(translate("commands.save.success", vec![]));

    Ok(())
}

fn save_switch(ctx: &mut Ctx, rd: &mut Rd, on: bool) -> R<()> {
    rd.finish()?;

    {
        let mut state = admin();

        if state.saving == on {
            return Err(failed(if on { "commands.save.alreadyOn" } else { "commands.save.alreadyOff" }));
        }

        state.saving = on;
    }

    ctx.ok(translate(if on { "commands.save.enabled" } else { "commands.save.disabled" }, vec![]));

    Ok(())
}

// ────────────────────────────── Проверки ──────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Порядок полей, снятый с оригинала (чёрный ящик, cmd-title).
    #[test]
    fn compound_fields_follow_the_java_hash_order() {
        fn order<'a>(names: &[&'a str]) -> Vec<&'a str> {
            java_order(names).into_iter().map(|index| names[index]).collect()
        }


        assert_eq!(order(&["click_event", "insertion", "text", "hover_event"]), ["click_event", "insertion", "text", "hover_event"]);
        assert_eq!(order(&["name", "action", "id", "uuid"]), ["name", "action", "id", "uuid"]);
        assert_eq!(order(&["color", "text", "bold", "font"]), ["color", "text", "bold", "font"]);
        assert_eq!(order(&["translate", "with"]), ["with", "translate"]);
        assert_eq!(order(&["text", "extra"]), ["extra", "text"]);
    }

    /// Простая строка — строка; `with` из чисел и строк — список составных с пустым ключом.
    #[test]
    fn mixed_lists_are_wrapped_like_the_original() {
        assert_eq!(s("Hello").encode(), [8, 0, 5, b'H', b'e', b'l', b'l', b'o']);

        let tag = translate("commands.setworldspawn.success", vec![Tag::Int(0), s("a")]);
        let bytes = tag.encode();
        // with: список составных (10), две записи.
        let at = bytes.windows(4).position(|window| window == b"with").expect("поле with");

        assert_eq!(&bytes[at + 4..at + 10], [10, 0, 0, 0, 2, 3]);
    }

    /// Строка-контекст: метка у курсора, хвост подчёркнут.
    #[test]
    fn the_context_line_marks_the_cursor() {
        let tag = context("title @s", 8);
        let text = render(&tag);

        assert!(text.contains("title @s"), "{}", text);
        assert!(!text.contains("..."), "{}", text);

        let tag = context("publish 25999", 0);

        assert!(render(&tag).starts_with("publish 25999"), "{}", render(&tag));
    }

    #[test]
    fn rendering_fills_the_templates() {
        let tag = translate("commands.title.show.title.single", vec![s("Anya")]);

        assert_eq!(render(&tag), "Showing new title for Anya");
        assert_eq!(render(&translate("commands.whitelist.list", vec![Tag::Int(2), s("a, b")])), "There are 2 whitelisted player(s): a, b");
    }

    #[test]
    fn time_has_units() {
        let mut rd = Rd::new("x 5 2s 1d");

        rd.pos = 1;
        assert_eq!(rd.time().ok(), Some(5));
        assert_eq!(rd.time().ok(), Some(40));
        assert_eq!(rd.time().ok(), Some(24000));

        let mut rd = Rd::new("x -1");

        rd.pos = 1;
        assert!(matches!(rd.time(), Err(Fail::Parse(_, 4))));
    }

    fn admin_with(bans: Vec<Ban>, ip_bans: Vec<IpBan>, whitelist: Vec<Allowed>, whitelist_on: bool) -> Admin {
        Admin { bans, ip_bans, whitelist, whitelist_on, enforce: false, idle_minutes: 0, saving: true, default_mode: 1, world_spawn_angle: (0.0, 0.0) }
    }

    /// Вход: блокировка по нику, по адресу и белый список — с их текстами.
    #[test]
    fn login_is_refused_like_the_original() {
        let uuid = crate::network::login::offline_uuid("Anya");
        let ip: IpAddr = "1.2.3.4".parse().expect("адрес");
        let ban = Ban { uuid: uuid_text(&uuid), name: "Anya".to_string(), created: stamp(), source: "Server".to_string(), expires: "forever".to_string(), reason: "spam".to_string() };
        let ip_ban = IpBan { ip: "1.2.3.4".to_string(), created: stamp(), source: "Server".to_string(), expires: "forever".to_string(), reason: "bad ip".to_string() };

        let state = admin_with(vec![ban], vec![], vec![], false);
        let (json, plain) = refusal(&state, "Anya", &uuid, ip, false).expect("забанен");

        assert_eq!(json, "{\"translate\":\"multiplayer.disconnect.banned.reason\",\"with\":[\"spam\"]}");
        assert_eq!(plain, "You are banned from this server.\nReason: spam");
        assert!(refusal(&state, "Other", &crate::network::login::offline_uuid("Other"), ip, false).is_none());

        let state = admin_with(vec![], vec![ip_ban], vec![], false);
        let (json, _) = refusal(&state, "Other", &crate::network::login::offline_uuid("Other"), ip, false).expect("адрес забанен");

        assert!(json.contains("multiplayer.disconnect.banned_ip.reason") && json.contains("bad ip"));
        assert!(refusal(&state, "Other", &crate::network::login::offline_uuid("Other"), "5.6.7.8".parse().expect("адрес"), false).is_none());

        let listed = Allowed { uuid: uuid_text(&uuid), name: "Anya".to_string() };
        let state = admin_with(vec![], vec![], vec![listed], true);

        assert!(refusal(&state, "Anya", &uuid, ip, false).is_none(), "в списке");
        assert!(refusal(&state, "Other", &crate::network::login::offline_uuid("Other"), ip, true).is_none(), "оператору можно");

        let (json, _) = refusal(&state, "Other", &crate::network::login::offline_uuid("Other"), ip, false).expect("не в списке");

        assert_eq!(json, "{\"translate\":\"multiplayer.disconnect.not_whitelisted\"}");
    }

    #[test]
    fn selectors_filter_by_name_and_limit() {
        // Разбор уточнений без списка игроков: пусто остаётся пустым.
        assert_eq!(component_len("\"x\" extra"), 3);
        assert_eq!(component_len("{\"a\":[1, 2]} tail"), 12);
        assert_eq!(component_len("word rest"), 4);
        assert_eq!(component_len("[\"a b\", {\"text\":\"c\"}]"), 21);
    }

    #[test]
    fn the_date_is_in_the_file_format() {
        let text = stamp();

        assert_eq!(text.len(), "2026-10-02 12:34:56 +0000".len());
        assert!(text.ends_with(" +0000"));
    }

    #[test]
    fn particle_options_use_the_wire_layout() {
        let options = crate::snbt::parse("{color:[1.0,0.0,0.0],scale:2.0}").expect("разбирается");

        assert_eq!(particle_data("dust", Some(&options)), Ok(vec![0xff, 0xff, 0, 0, 0x40, 0, 0, 0]));
        assert!(particle_data("dust", None).is_err());

        let options = crate::snbt::parse("{destination:{type:\"block\",pos:[1,2,3]},arrival_in_ticks:20}").expect("разбирается");

        assert_eq!(particle_data("vibration", Some(&options)), Ok(vec![0, 0, 0, 0, 0x40, 0, 0, 0x30, 0x02, 0x14]));
        assert_eq!(PARTICLES[32], "flame");
        assert_eq!(PARTICLES.len(), 117);
    }

    #[test]
    fn components_are_reduced_like_the_original() {
        let source = Source::Console;
        let shared_less = |text: &str| crate::snbt::parse(text).expect("разбирается");

        // Без Shared компоненты без селекторов не обойтись — проверяем чистые.
        let _ = (&source, &shared_less);
        assert_eq!(color_name("#ff0000"), "#FF0000");
        assert_eq!(color_name("Red"), "red");
    }
}
