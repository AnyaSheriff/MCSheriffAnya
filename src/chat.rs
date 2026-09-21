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
pub struct Chat {
    /// Строки в порядке появления; хранятся только последние.
    lines: Vec<String>,

    /// Сколько строк уже выброшено из начала ленты.
    dropped: usize,
}

impl Chat {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            dropped: 0,
        }
    }

    /// Добавляет строку в ленту.
    pub fn push(&mut self, line: String) {
        self.lines.push(line);

        if self.lines.len() > KEPT {
            self.lines.remove(0);
            self.dropped += 1;
        }
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
