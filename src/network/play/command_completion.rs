//! Один граф используется для объявления синтаксиса и дополнения. Словари
//! реестров и NBT-пояснения получены обычными сетевыми запросами к оригиналу.
use super::{Shared, Suggestions, text_length};
use super::command_tree::{Node, Tree};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Deserialize)]
struct Entry {
    text: String,
    tooltip: Option<String>,
}

#[derive(Clone)]
struct Match {
    start: usize,
    text: String,
    tooltip: Option<Vec<u8>>,
}

type Resources = BTreeMap<String, Vec<Entry>>;
fn resources(protocol: i32) -> &'static Resources {
    static NATIVE: std::sync::OnceLock<Resources> = std::sync::OnceLock::new();
    static PREVIEW: std::sync::OnceLock<Resources> = std::sync::OnceLock::new();
    if protocol >= 777 {
        PREVIEW.get_or_init(|| serde_json::from_str(include_str!("command_resources_777.json")).expect("словарь подсказок777"))
    } else {
        NATIVE.get_or_init(|| serde_json::from_str(include_str!("command_resources_775.json")).expect("словарь подсказок775"))
    }
}

struct Context<'a> {
    shared: &'a Shared,
    who: &'a [u8; 16],
    level: i32,
    resources: &'static Resources,
}

pub(super) fn complete(tree: &Tree, id: i32, text: &str, shared: &Shared, who: &[u8; 16]) -> Suggestions {
    let level = shared.ops.lock().expect("права").level_of(who);
    let context = Context { shared, who, level, resources: resources(tree.protocol) };
    let start = usize::from(text.starts_with('/'));
    let line = &text[start..];
    let mut found = if !line.contains(' ') {
        super::command_tree::IMPLEMENTED.iter().filter(|name| name.starts_with(&line.to_lowercase()))
            .map(|name| Match { start, text: (*name).into(), tooltip: None }).collect()
    } else {
        walk(tree, tree.root, start, text, &context, "", 0).1
    };
    // Оригинал предлагает reset даже non-op, но не публикует эту ветку
    // в его Commands и не разрешает разбирать её аргументы.
    if level == 0 && let Some(rest) = line.strip_prefix("random ")
        && !rest.contains(' ') && "reset".starts_with(rest) {
        found.push(Match { start: start + "random ".len(), text: "reset".into(), tooltip: None });
    }
    if found.is_empty() { return Suggestions { id, start: 0, length: 0, matches: Vec::new(), tooltips: Vec::new() }; }
    let start = found.iter().map(|hint| hint.start).min().unwrap();
    for hint in &mut found {
        if hint.start > start { hint.text = format!("{}{}", &text[start..hint.start], hint.text); }
    }
    found.sort_by(|a, b| a.text.to_lowercase().cmp(&b.text.to_lowercase()).then_with(|| a.text.cmp(&b.text)));
    found.dedup_by(|a, b| a.text == b.text);
    found.truncate(1000);
    Suggestions { id, start: text_length(&text[..start]), length: text_length(&text[start..]),
        matches: found.iter().map(|hint| hint.text.clone()).collect(),
        tooltips: found.into_iter().map(|hint| hint.tooltip).collect() }
}

fn walk(tree: &Tree, index: usize, offset: usize, text: &str, context: &Context<'_>, command: &str, depth: usize) -> (usize, Vec<Match>) {
    if depth > 128 { return (offset, Vec::new()); }
    let index = tree.nodes[index].redirect.unwrap_or(index);
    let parent = &tree.nodes[index];
    let rest = &text[offset..];
    let exact_literal = parent.children.iter().copied().find(|&child| {
        let node = &tree.nodes[child];
        node.flags & 3 == 1 && rest.strip_prefix(&node.name).is_some_and(|tail| tail.is_empty() || tail.starts_with(' '))
    });
    let children = exact_literal.map_or_else(|| parent.children.clone(), |child| vec![child]);
    let mut local = Vec::new();
    let mut best: Option<(usize, Vec<Match>)> = None;
    for child in children {
        let node = &tree.nodes[child];
        let command = if index == tree.root { node.name.as_str() } else { command };
        let consumed = if node.flags & 3 == 1 {
            rest.strip_prefix(&node.name).filter(|tail| tail.is_empty() || tail.starts_with(' ')).map(|_| node.name.len())
        } else { read_argument(node, rest, context) };
        if let Some(size) = consumed && rest.get(size..).is_some_and(|tail| tail.starts_with(' ')) {
            let candidate = walk(tree, child, offset + size + 1, text, context, command, depth + 1);
            // При равной глубине остаётся первая успешная ветка: teleport
            // сначала разбирает destination, а затем необязательные targets.
            if best.as_ref().is_none_or(|previous| candidate.0 > previous.0) { best = Some(candidate); }
        } else if node.flags & 3 == 1 {
            if node.name.to_lowercase().starts_with(&rest.to_lowercase()) {
                local.push(Match { start: offset, text: node.name.clone(), tooltip: None });
            }
        } else {
            suggest(node, offset, rest, context, command, &mut local);
        }
    }
    best.unwrap_or((offset, local))
}

fn resource_key(node: &Node) -> Option<String> {
    if node.suggestion.as_deref() == Some("minecraft:summonable_entities") { return Some("summon".into()); }
    if node.suggestion.as_deref() == Some("minecraft:available_sounds") { return Some("sound".into()); }
    match node.parser? {
        12 | 13 => Some("block".into()),
        14 | 15 => Some("item".into()),
        27 => Some("particle".into()),
        41 => Some("dimension".into()),
        42 => Some("gamemode".into()),
        36 if node.name == "timemarker" => Some("timemarker".into()),
        37 => Some("function".into()),
        44..=48 => {
            let bytes = super::command_tree::bytes(&node.properties);
            let mut cursor = super::Cursor::new(&bytes);
            let registry = cursor.string()?;
            Some(if registry == "minecraft:recipe" { "recipe".into() } else { registry })
        }
        _ => None,
    }
}

fn read_argument(node: &Node, rest: &str, context: &Context<'_>) -> Option<usize> {
    let size = argument(node, rest)?;
    let value = &rest[..size];
    if matches!(node.parser, Some(6 | 7)) {
        if value.starts_with('@') {
            if context.level < 2 || value.len() < 2 || !"aenprs".contains(value.as_bytes()[1] as char) { return None; }
            let properties = super::command_tree::bytes(&node.properties);
            let flags = properties.first().copied().unwrap_or(0);
            let fields: Vec<_> = value.split(['[', ',', ']']).collect();
            if flags & 1 != 0 && (value.starts_with("@a") || value.starts_with("@e")) && !fields.contains(&"limit=1") { return None; }
            if flags & 2 != 0 && (value.starts_with("@e") || value.starts_with("@n"))
                && !fields.iter().any(|field| matches!(*field, "type=player" | "type=minecraft:player")) { return None; }
        } else if value.chars().any(|c| !c.is_alphanumeric() && !"_-.+".contains(c)) { return None; }
    }
    if let Some(key) = resource_key(node)
        && matches!(node.parser, Some(12..=15 | 41 | 42 | 44..=48))
        && let Some(entries) = context.resources.get(&key) {
        let base = value.split(['[', '{']).next().unwrap_or(value);
        if !entries.iter().any(|entry| entry.text == base || entry.text.strip_prefix("minecraft:") == Some(base)) { return None; }
    }
    if node.parser == Some(43) {
        let amount = value.trim_end_matches(['s', 'd', 't']).parse::<f64>().ok()?;
        let bytes = super::command_tree::bytes(&node.properties);
        if !amount.is_finite() || amount < i32::from_be_bytes(bytes.try_into().ok()?) as f64 { return None; }
    }
    Some(size)
}

fn matches_resource(value: &str, typed: &str) -> bool {
    let typed = typed.to_lowercase();
    if typed.is_empty() { return true; }
    if value.starts_with(['!', '#']) {
        let prefix = value.chars().take_while(|ch| *ch == '!' || *ch == '#').collect::<String>();
        if !typed.starts_with(&prefix) { return value.starts_with(&typed); }
        return matches_resource(&value[prefix.len()..], &typed[prefix.len()..]);
    }
    if typed.starts_with(['!', '#']) { return false; }
    if typed.contains(':') { return value.starts_with(&typed); }
    value.starts_with(&typed) || value.strip_prefix("minecraft:").unwrap_or(value).split('_').any(|part| part.starts_with(&typed))
}

fn entries(context: &Context<'_>, key: &str, offset: usize, typed: &str, out: &mut Vec<Match>) {
    if let Some(values) = context.resources.get(key) {
        for entry in values {
            if matches_resource(&entry.text, typed) {
                out.push(Match { start: offset, text: entry.text.clone(), tooltip: entry.tooltip.as_deref().map(super::command_tree::bytes) });
            }
        }
    }
}

fn selector(offset: usize, rest: &str, context: &Context<'_>, out: &mut Vec<Match>) {
    if let Some(bracket) = rest.find('[') {
        if rest.ends_with(']') { return; }
        let after = &rest[bracket + 1..];
        let part = after.rsplit(',').next().unwrap();
        let start = offset + rest.len() - part.len();
        if let Some((key, typed)) = part.split_once('=') {
            let catalog = match key { "type" => "selector_type", "sort" => "selector_sort", "gamemode" => "selector_gamemode", "limit" => "", _ => return };
            let bare = typed.strip_prefix('!').unwrap_or(typed);
            let valid = if key == "limit" { typed.parse::<u32>().is_ok_and(|n| n > 0) }
                else if key == "type" && bare.starts_with('#') { true }
                else { context.resources.get(catalog).is_some_and(|entries| entries.iter().any(|entry| entry.text == typed || entry.text.strip_prefix("minecraft:") == Some(typed) || typed.strip_prefix('!').is_some_and(|value| entry.text.strip_prefix("!minecraft:") == Some(value)))) };
            if !typed.is_empty() && valid {
                for text in [",", "]"] { out.push(Match { start: offset + rest.len(), text: text.into(), tooltip: None }); }
            } else { entries(context, catalog, start + key.len() + 1, typed, out); }
        } else {
            let begin = out.len();
            entries(context, "selector_keys", start, part, out);
            // Уже заданный положительный type повторно не предлагается.
            let used: Vec<_> = after.split(',').take(after.matches(',').count()).filter_map(|part| {
                let (key,value) = part.split_once('=')?;
                (key != "type" || !value.starts_with('!')).then(|| format!("{key}="))
            }).collect();
            let mut tail = out.split_off(begin);
            tail.retain(|hint| !used.contains(&hint.text));
            out.extend(tail);
        }
    } else { entries(context, "selectors", offset, rest, out); }
}

fn suggest(node: &Node, offset: usize, rest: &str, context: &Context<'_>, command: &str, out: &mut Vec<Match>) {
    if matches!(node.parser, Some(6 | 7)) {
        if context.level >= 2 && (rest.is_empty() || rest.starts_with('@')) && !matches!(command, "op" | "deop" | "pardon") {
            selector(offset, rest, context, out);
        }
        if !rest.starts_with('@') {
            let mut names = context.shared.players.lock().expect("игроки").names();
            if matches!(command, "op" | "deop") {
                let ops = context.shared.ops.lock().expect("права");
                let operators = ops.names();
                if command == "op" { names.retain(|name| !operators.iter().any(|op| op.eq_ignore_ascii_case(name))); }
                else { names = operators.into_iter().map(str::to_owned).collect(); }
            } else if command == "pardon" { names = crate::commands_admin::banned_names(); }
            for name in names {
                if name.to_lowercase().starts_with(&rest.to_lowercase()) {
                    out.push(Match { start: offset, text: name, tooltip: None });
                }
            }
        }
        return;
    }
    if matches!(node.parser, Some(14 | 15)) && let Some(bracket) = rest.find('[') {
        let suffix = &rest[bracket + 1..];
        let typed = suffix.rsplit(',').next().unwrap();
        if !typed.contains('=') {
            entries(context, "item_components", offset + rest.len() - typed.len(), typed, out);
        }
        return;
    }
    if let Some(key) = resource_key(node) {
        entries(context, &key, offset, rest, out);
        return;
    }
    match node.parser {
        Some(0) => {
            for value in ["false", "true"] {
                if value.starts_with(rest) { out.push(Match { start: offset, text: value.into(), tooltip: None }); }
            }
        }
        Some(8..=11) => {
            let count = if matches!(node.parser, Some(8 | 10)) { 3 } else { 2 };
            for number in 1..=count {
                let text = vec!["~"; number].join(" ");
                if text.starts_with(rest) { out.push(Match { start: offset, text, tooltip: None }); }
            }
        }
        Some(24) => {
            // [проверить] Динамический список trigger зависит от enabled целей;
            // имена игроков здесь не являются допустимыми objective.
            let _ = context.who;
        }
        _ => {}
    }
}
fn argument(node: &Node, rest: &str) -> Option<usize> {
    if rest.is_empty() { return None; }
    let word = rest.split(' ').next()?;
    match node.parser? {
        0 => matches!(word, "true" | "false").then_some(word.len()),
        1..=4 => {
            let value = word.parse::<f64>().ok()?;
            let bytes = super::command_tree::bytes(&node.properties);
            if !value.is_finite() { return None; }
            if matches!(node.parser, Some(3 | 4)) && word.parse::<i64>().is_err() { return None; }
            if let Some(&flags) = bytes.first() {
                let width = if matches!(node.parser, Some(2 | 4)) { 8 } else { 4 };
                let read = |slice: &[u8]| match node.parser.unwrap() {
                    1 => f32::from_be_bytes(slice.try_into().unwrap()) as f64,
                    2 => f64::from_be_bytes(slice.try_into().unwrap()),
                    3 => i32::from_be_bytes(slice.try_into().unwrap()) as f64,
                    _ => i64::from_be_bytes(slice.try_into().unwrap()) as f64,
                };
                let mut at = 1;
                if flags & 1 != 0 {
                    let min = read(&bytes[at..at + width]);
                    at += width;
                    if value < min { return None; }
                }
                if flags & 2 != 0 && value > read(&bytes[at..at + width]) { return None; }
            }
            Some(word.len())
        }
        5 if node.properties == "02" => Some(rest.len()),
        8 | 10 | 9 | 11 | 29 => {
            let count = if matches!(node.parser, Some(8 | 10)) { 3 } else { 2 };
            let mut end = 0;
            for (i, coordinate) in rest.split(' ').take(count).enumerate() {
                let value = coordinate.trim_start_matches(['~', '^']);
                if coordinate.is_empty() || (!value.is_empty() && value.parse::<f64>().is_err()) { return None; }
                end += coordinate.len() + usize::from(i > 0);
                if i + 1 == count { return Some(end); }
            }
            None
        }
        18..=20 => Some(rest.len()),
        // Строки, NBT и селекторы с пробелами читаются до закрывающей скобки.
        _ => {
            let mut stack = Vec::new();
            let mut quote = None;
            let mut escape = false;
            for (at, ch) in rest.char_indices() {
                if escape { escape = false; continue; }
                if ch == '\\' && quote.is_some() { escape = true; continue; }
                if let Some(delimiter) = quote {
                    if ch == delimiter { quote = None; }
                } else {
                    match ch {
                        '"' | '\'' => quote = Some(ch),
                        '[' | '{' => stack.push(ch),
                        ']' if stack.last() == Some(&'[') => { stack.pop(); }
                        '}' if stack.last() == Some(&'{') => { stack.pop(); }
                        ' ' if stack.is_empty() => return Some(at),
                        _ => {}
                    }
                }
            }
            (stack.is_empty() && quote.is_none()).then_some(rest.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_bounds_and_incomplete_arguments() {
        let integer = Node { flags: 6, children: vec![], redirect: None, name: "count".into(), parser: Some(3), properties: "030000000100000040".into(), suggestion: None };
        assert_eq!(argument(&integer, "1 "), Some(1));
        assert_eq!(argument(&integer, "64"), Some(2));
        assert_eq!(argument(&integer, "0"), None);
        assert_eq!(argument(&integer, "65"), None);
        assert_eq!(argument(&integer, "1.5"), None);
        let selector = Node { parser: Some(6), properties: "00".into(), ..integer };
        assert_eq!(argument(&selector, "@e[type="), None);
        assert_eq!(argument(&selector, "@e[name=\"a b\"] "), Some(14));
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    fn compare(fixture: &str, protocol: i32, role: &str) {
        let ops_path = std::env::temp_dir().join(format!("mcsa-command-fixture-{protocol}-{role}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&ops_path);
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(), crate::world::World::in_memory(),
            std::path::PathBuf::new(), std::path::PathBuf::new(), Default::default(),
            crate::ops::Ops::open(&ops_path), crate::skins::Settings::default(),
        );
        let mut member = super::super::PlayerState::new([37; 16], "McsaProbe".into(), None, None, 0x7f, 1, (0.5, 64.0, 0.5)).member();
        member.uuid = [37; 16];
        shared.players.lock().unwrap().join(member, 1).unwrap();
        let level = if role == "op4" { 4 } else { 0 };
        if level == 4 { shared.ops.lock().unwrap().give(&[37; 16], "McsaProbe", 4); }
        let tree = super::super::command_tree::for_level(level, protocol);
        let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let mut before = 0;
        let mut after = 0;
        let samples = fixture["roles"][role]["suggestions"].as_array().unwrap();
        for sample in samples {
            let input = sample["input"].as_str().unwrap();
            let root_context = !input.contains(' ') || input.ends_with("run ");
            let expected: Vec<_> = sample["matches"].as_array().unwrap().iter().filter(|hint| {
                !root_context || super::super::command_tree::IMPLEMENTED.contains(&hint["text"].as_str().unwrap())
            }).collect();
            let expected_text: Vec<String> = expected.iter().map(|hint| hint["text"].as_str().unwrap().to_owned()).collect();
            let expected_tips: Vec<_> = expected.iter().map(|hint| hint["tooltip"].as_str().map(super::super::command_tree::bytes)).collect();
            let result = complete(&tree, 1, input, &shared, &[37; 16]);
            let expected_start = sample["start"].as_i64().unwrap() as i32;
            let expected_length = sample["length"].as_i64().unwrap() as i32;
            let typed = input.rsplit(' ').next().unwrap();
            let old_matches = super::super::hints_for(input, typed, &shared, &[37; 16]);
            let old_start = text_length(input) - text_length(typed);
            before += usize::from(old_matches != expected_text || old_start != expected_start || text_length(typed) != expected_length || expected_tips.iter().any(Option::is_some));
            if result.matches != expected_text || result.start != expected_start || result.length != expected_length || result.tooltips != expected_tips {
                after += 1;
                let missing: Vec<_> = expected_text.iter().filter(|s| !result.matches.contains(s)).take(5).collect();
                let extra: Vec<_> = result.matches.iter().filter(|s| !expected_text.contains(s)).take(5).collect();
                eprintln!("{protocol}/{role} {input:?}: got {} @{}+{}, expected {} @{expected_start}+{expected_length}; missing={missing:?}, extra={extra:?}, tips={}", result.matches.len(),result.start,result.length,expected_text.len(),result.tooltips==expected_tips);
            }
        }
        let _ = std::fs::remove_file(ops_path);
        eprintln!("{protocol}/{role}: {} suggestion fixtures, mismatches {before} -> {after}", samples.len());
        assert_eq!(after, 0);
    }

    #[test]
    fn player_suggestions_follow_live_permission_changes() {
        let path = std::env::temp_dir().join(format!("mcsa-live-completion-{}.json",std::process::id()));
        let _ = std::fs::remove_file(&path);
        let shared = crate::shared::Shared::new(crate::world::World::in_memory(), crate::world::World::in_memory(),
            std::path::PathBuf::new(), std::path::PathBuf::new(), Default::default(), crate::ops::Ops::open(&path), crate::skins::Settings::default());
        for (id, name) in [(1, "Alice"), (2, "Bob")] {
            let mut member = super::super::PlayerState::new([id;16], name.into(), None, None, 0x7f, 1, (0.5,64.0,0.5)).member();
            member.uuid = [id;16];
            shared.players.lock().unwrap().join(member, id as i32).unwrap();
        }
        shared.ops.lock().unwrap().give(&[1;16], "Alice", 4);
        let tree = super::super::command_tree::op4(777);
        let ask = |input| complete(tree, 1, input, &shared, &[1;16]).matches;
        assert_eq!(ask("/op "), ["Bob"]);
        assert_eq!(ask("/deop "), ["Alice"]);
        assert_eq!(ask("/msg B"), ["Bob"]);
        assert!(ask("/give @e ").is_empty());
        assert_eq!(ask("/give @s ").len(), 1000);
        assert!(ask("/ride @a ").is_empty());
        shared.ops.lock().unwrap().give(&[2;16], "Bob", 4);
        assert!(ask("/op ").is_empty());
        assert_eq!(ask("/deop "), ["Alice", "Bob"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn nonop_replies_diff_against_official_fixture() {
        compare(include_str!("../../../tools/protocol-bridge/testdata/commands-777/official.json"), 777, "nonop");
    }

    #[test]
    fn op4_replies_diff_against_official_fixture() {
        compare(include_str!("../../../tools/protocol-bridge/testdata/commands-777/official.json"), 777, "op4");
    }

    #[test]
    fn native775_op4_replies_diff_against_official_fixture() {
        compare(include_str!("../../../tools/protocol-bridge/testdata/commands-777/native-775/official.json"), 775, "op4");
    }
}
