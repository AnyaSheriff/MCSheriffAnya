// Лента сообщений в чат.
//
// Чат общий: что написал один игрок, должны увидеть все. Подключения живут
// в разных задачах и своими потоками друг другу писать не могут, поэтому
// устроено так же, как рассылка блоков и список игроков: сервер ведёт общую
// ленту, а каждое подключение помнит, сколько строк оно уже отправило своему
// клиенту, и досылает остальные.
//
// Лента не растёт бесконечно: старые строки отбрасываются. Номера строк при
// этом не сбиваются — сервер помнит, сколько строк уже выброшено, поэтому
// «сколько всего было строк» остаётся честным числом, а подключение, отставшее
// слишком сильно, просто не получит поздно появившиеся старые сообщения.
// Для чата это правильное поведение: перечитывать вчерашнюю болтовню незачем.

/// Сколько последних строк хранится.
const KEPT: usize = 256;

/// Лента сообщений в чат.
/// Самое длинное сообщение в чате, в знаках — как у оригинала.
pub const MAX_MESSAGE: usize = 256;

/// Годится ли текст для чата: как у оригинала, не длиннее 256 знаков, без
/// управляющих знаков и без «§» (им клиент красит текст). Иначе None —
/// такое сообщение не рассылается.
pub fn allowed(text: &str) -> Option<&str> {
    let fine = !text.is_empty()
        && text.chars().count() <= MAX_MESSAGE
        && !text.chars().any(|symbol| symbol == '§' || symbol.is_control());
    fine.then_some(text)
}

pub struct Chat {
    /// Строки в порядке появления; хранятся только последние.
    lines: Vec<String>,

    /// Сколько строк уже выброшено из начала ленты.
    dropped: usize,

    /// Кому строку не слать (тому, кто её уже получил своим путём: игрок
    /// видит своё сообщение о смерти в пакете смерти).
    hidden: Vec<Option<[u8; 16]>>,
}

impl Chat {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            dropped: 0,
            hidden: Vec::new(),
        }
    }

    /// Добавляет строку в ленту.
    pub fn push(&mut self, line: String) {
        self.push_hidden(line, None);
    }

    /// Добавляет строку, которую не надо слать игроку `except`.
    pub fn push_hidden(&mut self, line: String, except: Option<[u8; 16]>) {
        self.lines.push(line);
        self.hidden.push(except);

        if self.lines.len() > KEPT {
            self.lines.remove(0);
            self.hidden.remove(0);
            self.dropped += 1;
        }
    }

    /// Строки с номера `from`, кроме скрытых от игрока `who`.
    pub fn since_for(&self, from: usize, who: &[u8; 16]) -> Vec<String> {
        let start = from.max(self.dropped) - self.dropped;
        let start = start.min(self.lines.len());

        self.lines[start..]
            .iter()
            .zip(&self.hidden[start..])
            .filter(|(_, hidden)| hidden.as_ref() != Some(who))
            .map(|(line, _)| line.clone())
            .collect()
    }

    /// Сколько всего строк было — сколько уже разослано.
    pub fn count(&self) -> usize {
        self.dropped + self.lines.len()
    }

    /// Строки, начиная с номера `from` — то, что осталось разослать.
    ///
    /// Если запрошенный номер указывает на уже выброшенную строку, вернётся
    /// всё, что осталось: догнать ушедшее вперёд подключение нельзя.
    pub fn since(&self, from: usize) -> &[String] {
        let start = from.max(self.dropped) - self.dropped;

        &self.lines[start.min(self.lines.len())..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_lines_skip_only_their_player() {
        let mut chat = Chat::new();
        let (a, b) = ([1u8; 16], [2u8; 16]);

        chat.push("для всех".to_string());
        chat.push_hidden("не для a".to_string(), Some(a));

        assert_eq!(chat.since_for(0, &a), vec!["для всех".to_string()]);
        assert_eq!(chat.since_for(0, &b), vec!["для всех".to_string(), "не для a".to_string()]);
        assert_eq!(chat.since_for(1, &b), vec!["не для a".to_string()]);
    }

    fn push(chat: &mut Chat, line: &str) {
        chat.push(line.to_string());
    }

    #[test]
    fn lines_are_read_from_the_remembered_position() {
        let mut chat = Chat::new();

        push(&mut chat, "первая");
        assert_eq!(chat.count(), 1);
        assert_eq!(chat.since(0), ["первая"]);

        // Всё уже разослано — досылать нечего.
        assert!(chat.since(1).is_empty());

        push(&mut chat, "вторая");
        assert_eq!(chat.since(1), ["вторая"]);
    }

    /// Старые строки отбрасываются, но номера остаются честными: подключение,
    /// пропустившее много строк, получает то, что осталось, а не пустоту
    /// и не мусор.
    #[test]
    fn old_lines_are_dropped_without_breaking_numbering() {
        let mut chat = Chat::new();

        for index in 0..KEPT + 10 {
            push(&mut chat, &format!("строка {}", index));
        }

        assert_eq!(chat.count(), KEPT + 10);
        assert_eq!(chat.since(0).len(), KEPT);

        // Строка 10 ещё в ленте — с неё и начинаем.
        assert_eq!(chat.since(10)[0], "строка 10");

        // А счётчик указывает на конец — досылать нечего.
        assert!(chat.since(chat.count()).is_empty());
    }
}

#[cfg(test)]
mod message_tests {
    use super::*;

    #[test]
    fn only_plain_short_messages_pass() {
        assert_eq!(allowed("привет 😀"), Some("привет 😀"));
        assert_eq!(allowed(""), None);
        assert_eq!(allowed("§4красный"), None);
        assert_eq!(allowed("a\u{0}b"), None);
        assert_eq!(allowed("строка\nвторая"), None);
        assert!(allowed(&"я".repeat(256)).is_some());
        assert!(allowed(&"я".repeat(257)).is_none());
    }
}
