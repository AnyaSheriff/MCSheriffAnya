//! Пример модуля (modules/): использует `privileged` — недоступное
//! обычным плагинам. Команда `/modinfo` называет загруженные плагины,
//! `/modrun <команда>` выполняет команду от имени консоли.

use mcsa_sdk::{CommandResult, CommandSpec, Plugin, Sender, export_plugin};
use mcsa_sdk::bindings::mcsa::api::privileged;

struct ModInfo;

impl Plugin for ModInfo {
    fn enable() {
        for (name, description, usage, level) in [("modinfo", "Загруженные плагины", "/modinfo", 4), ("modrun", "Выполнить команду от консоли", "/modrun <команда>", 4)] {
            let _ = mcsa_sdk::command::register(&CommandSpec { name: name.into(), description: description.into(), usage: usage.into(), level, permission: None });
        }
    }

    fn on_command(_from: Sender, name: String, args: Vec<String>) -> CommandResult {
        let message = match name.as_str() {
            "modinfo" => format!("Загружено: {}", privileged::loaded_plugins().join(", ")),
            _ => privileged::run_console_command(&args.join(" ")),
        };

        CommandResult { success: true, message: Some(message) }
    }
}

export_plugin!(ModInfo);
