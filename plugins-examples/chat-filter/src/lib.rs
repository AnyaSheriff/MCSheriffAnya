//! Пример: фильтр чата. Сообщения с запрещённым словом не рассылаются, а
//! «капс» (три и больше заглавных подряд) переводится в строчные.

use mcsa_sdk::{ChatResult, Player, Plugin, export_plugin};

const BANNED: &[&str] = &["spam", "спам"];

struct ChatFilter;

impl Plugin for ChatFilter {
    fn enable() {
        mcsa_sdk::info("chat-filter включён");
    }

    fn on_chat(who: Player, message: String) -> ChatResult {
        let lower = message.to_lowercase();

        if BANNED.iter().any(|word| lower.contains(word)) {
            mcsa_sdk::player::send_message(&who.uuid, "Это слово здесь нельзя");
            return ChatResult::Cancel;
        }

        let letters: Vec<char> = message.chars().filter(|c| c.is_alphabetic()).collect();

        if letters.len() >= 3 && letters.iter().all(|c| c.is_uppercase()) {
            return ChatResult::Replace(lower);
        }

        ChatResult::Pass
    }
}

export_plugin!(ChatFilter);
