// Команда `/plugman`: управление плагинами на ходу.
//
//   /plugman list                  — что загружено и что лежит на диске
//   /plugman info <имя>            — сведения о плагине и его команды
//   /plugman load <имя>            — загрузить
//   /plugman unload <имя>          — выгрузить
//   /plugman stats                 — сколько плагины тратят времени
//   /plugman reload <имя|all>      — выгрузить и загрузить заново
//
// Только консоль и операторы высшего уровня: плагин — это чужой код.

use super::enabled::with_host_always as enabled_with;
use super::{Answer, Source};
use crate::shared::Shared;

/// Может ли вызывающий управлять плагинами.
fn may(source: &Source, shared: &Shared) -> bool {
    match source {
        Source::Console => true,
        Source::Block { .. } => false,
        Source::Player { uuid, .. } => shared.ops.lock().expect("права захвачены другим потоком").level_of(uuid) >= crate::ops::HIGHEST_LEVEL,
    }
}

const USAGE: &str = "Плагины: /plugman list | info <имя> | stats | load <имя> | unload <имя> | reload <имя|all>";

pub fn run(args: &[String], source: &Source, shared: &Shared) -> Answer {
    if !may(source, shared) {
        return Answer::saying("На это у тебя нет прав".to_string());
    }

    let Some(sub) = args.first().map(|word| word.to_ascii_lowercase()) else {
        return Answer::saying(USAGE.to_string());
    };

    let name: Option<String> = args.get(1).cloned();

    let reply = enabled_with(move |host| match (sub.as_str(), name.as_deref()) {
        ("list" | "ls", _) => list(host),
        ("stats", _) => Ok(stats(host)),
        ("info", Some(name)) => info(host, name),
        ("load", Some(name)) => host.load(name).map(|summary| format!("Загружен {summary}")),
        ("unload", Some(name)) => host.unload(name),
        ("reload", Some("all")) => {
            let names: Vec<String> = host.loaded.iter().map(|plugin| plugin.manifest.name.clone()).collect();
            let mut done = 0;
            let mut errors = Vec::new();

            for name in names {
                // Уже перезагруженный вместе с основой пропускаем.
                if !host.is_loaded(&name) {
                    continue;
                }

                match host.reload(&name) {
                    Ok(_) => done += 1,
                    Err(error) => errors.push(error),
                }
            }

            if errors.is_empty() { Ok(format!("Перезагружено плагинов: {done}")) } else { Err(errors.join("; ")) }
        }
        ("reload", Some(name)) => host.reload(name).map(|text| format!("Плагин {text}")),
        ("info" | "load" | "unload" | "reload", None) => Err(USAGE.to_string()),
        _ => Err(USAGE.to_string()),
    });

    match reply {
        None => Answer::saying("Плагины не запущены".to_string()),
        Some(Err(text)) | Some(Ok(Err(text))) => {
            let mut answer = Answer::saying(format!("Не вышло: {text}"));
            answer.result = None;
            answer
        }
        Some(Ok(Ok(text))) => Answer::saying(text),
    }
}

fn list(host: &super::host::Host) -> Result<String, String> {
    let on_disk = host.available();
    let loaded: Vec<String> = host
        .loaded
        .iter()
        .map(|plugin| format!("{} {}{}", plugin.manifest.name, plugin.manifest.version, if plugin.trusted { " (модуль)" } else { "" }))
        .collect();
    let idle: Vec<String> = on_disk.into_iter().filter(|(name, _)| !host.is_loaded(name)).map(|(name, _)| name).collect();

    let mut text = format!("Загружено ({}): {}", loaded.len(), if loaded.is_empty() { "нет".to_string() } else { loaded.join(", ") });

    if !idle.is_empty() {
        text.push_str(&format!("\nНа диске, не загружено: {}", idle.join(", ")));
    }

    Ok(text)
}

fn info(host: &super::host::Host, name: &str) -> Result<String, String> {
    let plugin = host.loaded.iter().find(|plugin| plugin.manifest.name == name).ok_or_else(|| format!("{name} не загружен"))?;
    let manifest = &plugin.manifest;
    let mut text = format!("{} {} ({})", manifest.name, manifest.version, if plugin.trusted { "модуль" } else { "плагин, песочница" });

    if !manifest.author.is_empty() {
        text.push_str(&format!("\nАвтор: {}", manifest.author));
    }

    if !manifest.description.is_empty() {
        text.push_str(&format!("\n{}", manifest.description));
    }

    if !manifest.dependencies.is_empty() {
        text.push_str(&format!("\nЗависит от: {}", manifest.dependencies.join(", ")));
    }

    for command in &plugin.store.data().commands {
        let usage = if command.usage.is_empty() { format!("/{}", command.name) } else { command.usage.clone() };

        text.push_str(&format!("\n{usage} — {}", command.description));
    }

    Ok(text)
}

fn stats(host: &super::host::Host) -> String {
    let mut text = format!(
        "Такт плагинов: {:.1} мкс в среднем, худший {:.0} мкс (тактов: {})",
        host.tick_stats.average_micros(),
        host.tick_stats.worst as f64 / 1000.0,
        host.tick_stats.calls
    );

    for plugin in &host.loaded {
        text.push_str(&format!(
            "\n{}: вызовов {}, в среднем {:.1} мкс, худший {:.0} мкс",
            plugin.manifest.name,
            plugin.stats.calls,
            plugin.stats.average_micros(),
            plugin.stats.worst as f64 / 1000.0
        ));
    }

    text
}
