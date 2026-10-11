//! Пример: приветствие при входе и команда `/hello`.

use mcsa_sdk::{CommandResult, CommandSpec, Level, Player, Sender, Plugin, export_plugin};

struct Hello;

impl Plugin for Hello {
    fn enable() {
        let spec = CommandSpec {
            name: "hello".into(),
            description: "Поздороваться".into(),
            usage: "/hello [кому]".into(),
            level: 0,
            permission: Some("hello.use".into()),
        };

        if let Err(error) = mcsa_sdk::command::register(&spec) {
            mcsa_sdk::log::write(Level::Error, &format!("не зарегистрировалась /hello: {error}"));
        }

        // Раз в минуту (1200 тактов) напоминаем о себе в консоль.
        mcsa_sdk::scheduler::run_repeating(1200, 1200, 1);
        mcsa_sdk::info("hello включён");
    }

    fn disable() {
        mcsa_sdk::info("hello выключен");
    }

    fn on_player_join(who: Player) {
        // Личное сообщение вошедшему и объявление всем.
        mcsa_sdk::player::send_message(&who.uuid, &format!("Привет, {}! Попробуй /hello", who.name));
        mcsa_sdk::server::broadcast(&format!("[hello] {} с нами", who.name));
    }

    fn on_command(from: Sender, _name: String, args: Vec<String>) -> CommandResult {
        let target = args.first().cloned().unwrap_or(from.name);

        CommandResult { success: true, message: Some(format!("Привет, {target}!")) }
    }

    fn on_task(_task: u32) {
        let online = mcsa_sdk::server::online_players().len();

        mcsa_sdk::info(&format!("игроков на сервере: {online}"));
    }
}

export_plugin!(Hello);
