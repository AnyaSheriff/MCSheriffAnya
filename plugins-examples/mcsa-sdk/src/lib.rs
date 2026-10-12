//! Обёртка над API `mcsa:api` (wit/mcsa-api.wit) для плагинов на Rust.
//!
//! Плагин — тип с `impl Plugin` (нужные обработчики; остальные по умолчанию
//! ничего не делают) и одна строка `mcsa_sdk::export_plugin!(МойПлагин);`.

pub mod bindings {
    wit_bindgen::generate!({
        path: "../../wit",
        world: "module",
        pub_export_macro: true,
        default_bindings_module: "mcsa_sdk::bindings",
    });
}

pub use bindings::mcsa::api::command::{self, CommandSpec};
pub use bindings::mcsa::api::log::{self, Level};
pub use bindings::mcsa::api::player;
pub use bindings::mcsa::api::presentation;
pub use bindings::mcsa::api::scheduler;
pub use bindings::mcsa::api::server;
pub use bindings::mcsa::api::storage;
pub use bindings::mcsa::api::types::{BlockPos, ChatResult, CommandResult, Dimension, GameMode, ItemStack, Location, Player, Sender, SenderKind, Verdict};
pub use bindings::mcsa::api::world;

/// Короткие вызовы журнала.
pub fn info(message: &str) {
    log::write(Level::Info, message);
}

pub fn warn(message: &str) {
    log::write(Level::Warn, message);
}

/// Что умеет плагин. Всё необязательно.
pub trait Plugin {
    /// Плагин загружен: регистрируй команды и задачи.
    fn enable() {}
    /// Плагин выгружается.
    fn disable() {}
    fn on_player_join(_who: Player) {}
    fn on_player_quit(_who: Player) {}
    /// Чат: `Pass`, `Cancel` или `Replace(текст)`.
    fn on_chat(_who: Player, _message: String) -> ChatResult {
        ChatResult::Pass
    }
    fn on_block_break(_who: Player, _pos: BlockPos, _block: String) -> Verdict {
        Verdict::Allow
    }
    fn on_block_place(_who: Player, _pos: BlockPos, _block: String) -> Verdict {
        Verdict::Allow
    }
    fn on_player_command(_who: Player, _command_line: String) -> Verdict {
        Verdict::Allow
    }
    /// Вызвана команда, зарегистрированная этим плагином.
    fn on_command(_from: Sender, _name: String, _args: Vec<String>) -> CommandResult {
        CommandResult { success: false, message: Some("команда не реализована".to_string()) }
    }
    /// Сработала задача планировщика.
    fn on_task(_task: u32) {}
}

/// Подключает тип плагина к экспортам `mcsa:api`.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        const _: () = {
            use $crate::bindings::exports::mcsa::api::{events, lifecycle};
            use $crate::bindings::mcsa::api::types as t;

            struct Glue;

            impl lifecycle::Guest for Glue {
                fn enable() {
                    <$plugin as $crate::Plugin>::enable()
                }
                fn disable() {
                    <$plugin as $crate::Plugin>::disable()
                }
            }

            impl events::Guest for Glue {
                fn on_player_join(who: t::Player) {
                    <$plugin as $crate::Plugin>::on_player_join(who)
                }
                fn on_player_quit(who: t::Player) {
                    <$plugin as $crate::Plugin>::on_player_quit(who)
                }
                fn on_chat(who: t::Player, message: String) -> t::ChatResult {
                    <$plugin as $crate::Plugin>::on_chat(who, message)
                }
                fn on_block_break(who: t::Player, pos: t::BlockPos, block: String) -> t::Verdict {
                    <$plugin as $crate::Plugin>::on_block_break(who, pos, block)
                }
                fn on_block_place(who: t::Player, pos: t::BlockPos, block: String) -> t::Verdict {
                    <$plugin as $crate::Plugin>::on_block_place(who, pos, block)
                }
                fn on_player_command(who: t::Player, command_line: String) -> t::Verdict {
                    <$plugin as $crate::Plugin>::on_player_command(who, command_line)
                }
                fn on_command(from: t::Sender, name: String, args: Vec<String>) -> t::CommandResult {
                    <$plugin as $crate::Plugin>::on_command(from, name, args)
                }
                fn on_task(task: u32) {
                    <$plugin as $crate::Plugin>::on_task(task)
                }
            }

            $crate::bindings::export!(Glue with_types_in $crate::bindings);
        };
    };
}
