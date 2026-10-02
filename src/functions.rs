// Функции наборов данных: `function`, `schedule`, `datapack`, `return`.
//
// Функция — файл `.mcfunction` в наборе данных мира:
// `world/datapacks/<набор>/data/<пространство>/function/<путь>.mcfunction`
// (вики, Function (Java Edition)). Каждая строка — команда без косой черты;
// `#` — пояснение, `$` — строка макроса (у нас не поддержана). Метки функций
// — `data/<пр>/tags/function/<имя>.json` (`{"values": [...]}`); `#minecraft:load`
// выполняется при запуске, `#minecraft:tick` — каждый такт.
//
// Наборы понимаются только функциональные: ни своих миров, ни добычи они у
// нас не меняют. Список выключенных наборов хранится в
// `world/data/datapacks.json` (у оригинала — в level.dat) [проверить].

use std::cell::Cell;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::commands::{Answer, Effect, Source};
use crate::scoreboard::{literal, translate};
use crate::shared::Shared;

/// Сколько функций может быть вложено друг в друга (защита от бесконечной
/// рекурсии; у оригинала предел — maxCommandSequenceLength).
const MAX_DEPTH: u32 = 512;

/// Сколько команд самое большее выполняет один вызов (вики, gamerule
/// maxCommandSequenceLength: 65536).
const MAX_COMMANDS: usize = 65_536;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Запланированное `schedule function`.
struct Scheduled {
    due: u64,
    id: String,
    tag: bool,
}

/// Функции мира: расписание и выключенные наборы.
pub struct Functions {
    scheduled: Vec<Scheduled>,
    disabled: Vec<String>,
    world: PathBuf,
    /// Такт сейчас — для расписания.
    now: u64,
}

impl Functions {
    pub fn new(world: &Path) -> Self {
        let disabled = std::fs::read_to_string(world.join("data").join("datapacks.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
            .unwrap_or_default();

        Self { scheduled: Vec::new(), disabled, world: world.to_path_buf(), now: 0 }
    }

    fn save(&self) {
        let file = self.world.join("data").join("datapacks.json");
        let _ = std::fs::create_dir_all(self.world.join("data"));
        if let Ok(text) = serde_json::to_string(&self.disabled) {
            let _ = std::fs::write(file, text);
        }
    }

    /// Наборы в папке мира (имя папки — `file/<имя>` у оригинала).
    fn packs(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.world.join("datapacks"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().join("pack.mcmeta").exists() || entry.path().join("data").is_dir())
                    .map(|entry| format!("file/{}", entry.file_name().to_string_lossy()))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn enabled_packs(&self) -> Vec<String> {
        self.packs().into_iter().filter(|pack| !self.disabled.contains(pack)).collect()
    }

    /// Файл функции `ns:path` в первом включённом наборе, где она есть.
    fn function_file(&self, id: &str) -> Option<PathBuf> {
        let (space, path) = id.split_once(':').unwrap_or(("minecraft", id));

        if path.contains("..") || space.contains('/') {
            return None;
        }

        self.enabled_packs().into_iter().rev().find_map(|pack| {
            let folder = self.world.join("datapacks").join(pack.trim_start_matches("file/")).join("data").join(space);
            // С 1.21 папка зовётся `function`; прежнее `functions` тоже читаем.
            ["function", "functions"].iter().map(|kind| folder.join(kind).join(format!("{path}.mcfunction"))).find(|file| file.is_file())
        })
    }

    /// Функции метки `#ns:path` (из всех включённых наборов).
    fn tag_members(&self, id: &str) -> Vec<String> {
        let (space, path) = id.split_once(':').unwrap_or(("minecraft", id));
        let mut out = Vec::new();

        for pack in self.enabled_packs() {
            let folder = self.world.join("datapacks").join(pack.trim_start_matches("file/")).join("data").join(space).join("tags");
            for kind in ["function", "functions"] {
                let Ok(text) = std::fs::read_to_string(folder.join(kind).join(format!("{path}.json"))) else {
                    continue;
                };
                let Ok(json) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if json.get("replace").and_then(Value::as_bool) == Some(true) {
                    out.clear();
                }
                for value in json.get("values").and_then(Value::as_array).into_iter().flatten() {
                    let name = value.as_str().or_else(|| value.get("id").and_then(Value::as_str)).unwrap_or("");
                    if let Some(tag) = name.strip_prefix('#') {
                        out.extend(self.tag_members(tag));
                    } else if !name.is_empty() {
                        out.push(normalize_id(name));
                    }
                }
            }
        }

        out
    }
}

fn normalize_id(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

/// Строки функции: без пустых и пояснений.
fn lines_of(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .map(|text| {
            // `\` в конце строки продолжает её следующей (вики).
            let joined = text.replace("\\\r\n", " ").replace("\\\n", " ");
            joined.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).map(str::to_string).collect()
        })
        .unwrap_or_default()
}

/// Итог функции: сколько команд, что вернула (`return`), упала ли (`return fail`).
pub struct Outcome {
    pub commands: usize,
    pub returned: Option<i32>,
    pub failed: bool,
}

/// Выполнить функцию от имени `source`. None — функции нет.
pub fn run_function(id: &str, source: &Source, shared: &Shared) -> Option<Outcome> {
    let file = shared.functions.lock().expect("функции захвачены другим потоком").function_file(id)?;
    let lines = lines_of(&file);

    if DEPTH.with(Cell::get) >= MAX_DEPTH {
        return Some(Outcome { commands: 0, returned: None, failed: true });
    }

    DEPTH.with(|depth| depth.set(depth.get() + 1));
    let mut outcome = Outcome { commands: 0, returned: None, failed: false };

    for line in lines.iter().take(MAX_COMMANDS) {
        // Строки макросов (`$…`) подставляют доводы — у нас их нет.
        if line.starts_with('$') {
            continue;
        }

        outcome.commands += 1;

        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["return", "fail"] => {
                outcome.failed = true;
                break;
            }
            ["return", "run", rest @ ..] => {
                let answer = crate::commands::run(&rest.join(" "), source, shared);
                match answer.result {
                    Some(value) => outcome.returned = Some(value),
                    None => outcome.failed = true,
                }
                break;
            }
            ["return", value] => {
                outcome.returned = value.parse().ok();
                outcome.failed = outcome.returned.is_none();
                break;
            }
            _ => {
                // Ответы команд функции никому не показываются (у оригинала
                // функции молчат: sendCommandFeedback только у вызова).
                crate::commands::run(line, source, shared);
            }
        }
    }

    DEPTH.with(|depth| depth.set(depth.get() - 1));
    Some(outcome)
}

fn answer(text: Value, result: Option<i32>) -> Answer {
    Answer {
        reply: crate::scoreboard::plain_text(&text),
        effect: Effect::None,
        component: None,
        rich: vec![crate::scoreboard::encode_component(&text)],
        result,
    }
}

fn fail(key: &str, with: Vec<Value>) -> Answer {
    answer(serde_json::json!({"text": "", "color": "red", "extra": [translate(key, with)]}), None)
}

/// `function <имя>` и `function #<метка>`.
pub fn function_command(words: &[&str], source: &Source, shared: &Shared) -> Answer {
    let Some(&target) = words.first() else {
        return fail("command.unknown.argument", vec![literal("function")]);
    };

    if words.len() > 1 {
        // Доводы макроса не поддержаны [проверить].
        return fail("command.unknown.argument", vec![literal(words[1])]);
    }

    let ids = match target.strip_prefix('#') {
        Some(tag) => shared.functions.lock().expect("функции захвачены другим потоком").tag_members(&normalize_id(tag)),
        None => vec![normalize_id(target)],
    };

    let mut total = 0;
    let mut ran = 0;
    let mut returned = None;

    for id in &ids {
        match run_function(id, source, shared) {
            Some(outcome) => {
                ran += 1;
                total += outcome.commands;
                if outcome.failed {
                    return answer(literal(""), None);
                }
                if outcome.returned.is_some() {
                    returned = outcome.returned;
                }
            }
            None => return fail("arguments.function.unknown", vec![literal(&normalize_id(target.trim_start_matches('#')))]),
        }
    }

    match (ids.len(), returned) {
        (1, Some(value)) => answer(translate("commands.function.success.single.result", vec![Value::from(value), literal(&ids[0])]), Some(value)),
        (1, None) => answer(translate("commands.function.success.single", vec![Value::from(total as i64), literal(&ids[0])]), Some(total as i32)),
        (_, Some(value)) => answer(translate("commands.function.success.multiple.result", vec![Value::from(ran)]), Some(value)),
        _ => answer(translate("commands.function.success.multiple", vec![Value::from(total as i64), Value::from(ran)]), Some(total as i32)),
    }
}

/// Длительность `schedule`: `20`, `20t`, `1s`, `1d` → такты.
fn ticks_of(text: &str) -> Option<u64> {
    let (number, scale) = match text.chars().last()? {
        't' => (&text[..text.len() - 1], 1.0),
        's' => (&text[..text.len() - 1], 20.0),
        'd' => (&text[..text.len() - 1], 24_000.0),
        _ => (text, 1.0),
    };
    let value: f64 = number.parse().ok()?;
    (value >= 0.0).then(|| (value * scale).round() as u64)
}

/// `schedule function <имя> <время> [append|replace]`, `schedule clear <имя>`.
pub fn schedule_command(words: &[&str], shared: &Shared) -> Answer {
    let mut functions = shared.functions.lock().expect("функции захвачены другим потоком");

    match words {
        ["function", id, time, mode @ ..] => {
            let Some(delay) = ticks_of(time) else {
                return fail("command.unknown.argument", vec![literal(time)]);
            };
            if delay == 0 {
                return fail("commands.schedule.same_tick", vec![]);
            }
            let (tag, name) = match id.strip_prefix('#') {
                Some(tag) => (true, normalize_id(tag)),
                None => (false, normalize_id(id)),
            };
            let exists = if tag { !functions.tag_members(&name).is_empty() } else { functions.function_file(&name).is_some() };
            if !exists {
                return fail("arguments.function.unknown", vec![literal(&name)]);
            }
            let shown = if tag { format!("#{name}") } else { name.clone() };
            // По умолчанию — replace: прежнее с тем же именем снимается.
            if mode.first() != Some(&"append") {
                functions.scheduled.retain(|item| !(item.id == name && item.tag == tag));
            }
            let due = functions.now + delay;
            functions.scheduled.push(Scheduled { due, id: name, tag });
            answer(translate("commands.schedule.created.function", vec![literal(&shown), Value::from(delay), Value::from(due)]), Some((due % (i32::MAX as u64)) as i32))
        }
        ["clear", id] => {
            let (tag, name) = match id.strip_prefix('#') {
                Some(tag) => (true, normalize_id(tag)),
                None => (false, normalize_id(id)),
            };
            let before = functions.scheduled.len();
            functions.scheduled.retain(|item| !(item.id == name && item.tag == tag));
            let removed = before - functions.scheduled.len();
            if removed == 0 {
                return fail("commands.schedule.cleared.failure", vec![literal(id)]);
            }
            answer(translate("commands.schedule.cleared.success", vec![Value::from(removed as i64), literal(id)]), Some(removed as i32))
        }
        _ => fail("command.unknown.argument", vec![literal(words.first().copied().unwrap_or("schedule"))]),
    }
}

/// `datapack list [available|enabled]`, `datapack enable|disable <набор>`.
pub fn datapack_command(words: &[&str], shared: &Shared) -> Answer {
    let mut functions = shared.functions.lock().expect("функции захвачены другим потоком");
    let pack_text = |name: &str| translate("chat.square_brackets", vec![literal(name)]);

    match words {
        ["list", which @ ..] => {
            let enabled = functions.enabled_packs();
            let available: Vec<String> = functions.packs().into_iter().filter(|pack| !enabled.contains(pack)).collect();
            let mut lines = Vec::new();
            let list = |names: &[String]| serde_json::json!({"text": "", "extra": names.iter().enumerate().flat_map(|(at, name)| {
                let mut parts = Vec::new();
                if at > 0 { parts.push(serde_json::json!({"color": "gray", "text": ", "})); }
                parts.push(pack_text(name));
                parts
            }).collect::<Vec<_>>()});
            if which.is_empty() || which == ["available"] {
                lines.push(if available.is_empty() { translate("commands.datapack.list.available.none", vec![]) } else { translate("commands.datapack.list.available.success", vec![Value::from(available.len()), list(&available)]) });
            }
            if which.is_empty() || which == ["enabled"] {
                lines.push(if enabled.is_empty() { translate("commands.datapack.list.enabled.none", vec![]) } else { translate("commands.datapack.list.enabled.success", vec![Value::from(enabled.len()), list(&enabled)]) });
            }
            let mut out = answer(lines.remove(0), Some((available.len() + enabled.len()) as i32));
            for line in lines {
                out.reply.push('\n');
                out.reply.push_str(&crate::scoreboard::plain_text(&line));
                out.rich.push(crate::scoreboard::encode_component(&line));
            }
            out
        }
        [action @ ("enable" | "disable"), pack, ..] => {
            let name = pack.trim_matches('"').to_string();
            if !functions.packs().contains(&name) {
                return fail("commands.datapack.unknown", vec![literal(&name)]);
            }
            let enable = *action == "enable";
            let is_on = !functions.disabled.contains(&name);
            if is_on == enable {
                return fail(&format!("commands.datapack.{action}.failed"), vec![literal(&name)]);
            }
            if enable {
                functions.disabled.retain(|known| known != &name);
            } else {
                functions.disabled.push(name.clone());
            }
            functions.save();
            let count = functions.enabled_packs().len() as i32;
            answer(translate(&format!("commands.datapack.modify.{action}"), vec![pack_text(&name)]), Some(count))
        }
        _ => fail("command.unknown.argument", vec![literal(words.first().copied().unwrap_or("datapack"))]),
    }
}

/// `return <число>|fail|run …` вне функции: итог команды (вики, return).
pub fn return_command(words: &[&str], source: &Source, shared: &Shared) -> Answer {
    match words {
        ["fail"] => Answer { reply: String::new(), effect: Effect::None, component: None, rich: Vec::new(), result: None },
        ["run", rest @ ..] => crate::commands::run(&rest.join(" "), source, shared),
        [value] => match value.parse::<i32>() {
            Ok(value) => Answer { reply: String::new(), effect: Effect::None, component: None, rich: Vec::new(), result: Some(value) },
            Err(_) => fail("command.unknown.argument", vec![literal(value)]),
        },
        _ => fail("command.unknown.argument", vec![literal("return")]),
    }
}

/// Такт: запланированное, чей срок пришёл, и `#minecraft:tick`. Зовётся из
/// такта в начале, без замков (вики, Function: функции метки tick — в начале
/// такта сервера).
pub fn tick(shared: &Shared, now: u64) {
    let (due, ticking) = {
        let mut functions = shared.functions.lock().expect("функции захвачены другим потоком");
        functions.now = now;
        let mut due = Vec::new();
        functions.scheduled.retain(|item| {
            if item.due <= now {
                due.push((item.id.clone(), item.tag));
                false
            } else {
                true
            }
        });
        // Метки читаются с диска — только если наборы вообще есть.
        let ticking = if functions.world.join("datapacks").is_dir() { functions.tag_members("minecraft:tick") } else { Vec::new() };
        (due, ticking)
    };

    for id in ticking {
        run_function(&id, &Source::Console, shared);
    }

    for (id, tag) in due {
        let ids = if tag { shared.functions.lock().expect("функции захвачены другим потоком").tag_members(&id) } else { vec![id] };
        for id in ids {
            run_function(&id, &Source::Console, shared);
        }
    }
}

/// Запуск: `#minecraft:load`.
pub fn load(shared: &Shared) {
    let ids = shared.functions.lock().expect("функции захвачены другим потоком").tag_members("minecraft:load");
    for id in ids {
        run_function(&id, &Source::Console, shared);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_times_are_ticks() {
        assert_eq!(ticks_of("20"), Some(20));
        assert_eq!(ticks_of("3t"), Some(3));
        assert_eq!(ticks_of("1.5s"), Some(30));
        assert_eq!(ticks_of("1d"), Some(24_000));
        assert_eq!(ticks_of("-1"), None);
    }

    #[test]
    fn functions_and_tags_are_found_in_datapacks() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("mcsa-fn-{}", std::process::id()));
        let data = root.join("datapacks").join("p").join("data");
        std::fs::create_dir_all(data.join("t").join("function").join("sub")).unwrap();
        std::fs::create_dir_all(data.join("minecraft").join("tags").join("function")).unwrap();
        std::fs::write(root.join("datapacks").join("p").join("pack.mcmeta"), "{}").unwrap();
        std::fs::write(data.join("t").join("function").join("sub").join("a.mcfunction"), "# пояснение\nsay a\n\nreturn 4\n").unwrap();
        std::fs::write(data.join("minecraft").join("tags").join("function").join("tick.json"), r#"{"values":["t:sub/a"]}"#).unwrap();

        let functions = Functions::new(&root);
        assert_eq!(functions.packs(), vec!["file/p".to_string()]);
        let file = functions.function_file("t:sub/a").unwrap();
        assert_eq!(lines_of(&file), vec!["say a".to_string(), "return 4".to_string()]);
        assert_eq!(functions.tag_members("minecraft:tick"), vec!["t:sub/a".to_string()]);
        assert!(functions.function_file("t:../x").is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
