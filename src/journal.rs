// Общий журнал изменений.
//
// Сервер рассказывает подключениям об изменениях не напрямую (они живут в
// разных задачах), а через журнал: изменения складываются в общий список,
// а каждое подключение помнит, сколько записей оно уже разослало своему
// клиенту, и досылает остальные.
//
// Сам по себе такой список рос бы вечно: вода за час наменяет десятки тысяч
// блоков, и все они остались бы в памяти навсегда. Поэтому журнал помнит, до
// какого места дочитал каждый, и выбрасывает записи, которые прочитали все.
//
// Читатель обязан отписаться, когда уходит: иначе журнал будет вечно ждать
// того, кого уже нет, и не выбросит ни одной записи.

use std::collections::{HashMap, VecDeque};

/// Предел на случай застрявшего читателя.
///
/// Обычно записи выбрасываются, как только их прочитали все. Но если кто-то
/// перестал читать и при этом не отписался, журнал не должен расти без
/// конца: сверх этого числа старые записи выбрасываются всё равно, а
/// отставший получит только то, что осталось.
const LIMIT: usize = 1 << 16;

/// Кто читает журнал. Номер выдаёт сервер, он же и общий для всех журналов.
pub type Reader = u64;

/// Журнал изменений.
pub struct Journal<T> {
    /// Записи, которые ещё кому-то нужны.
    entries: VecDeque<T>,

    /// Сколько записей уже выброшено из начала: по нему считаются номера,
    /// чтобы они не сбивались после выбрасывания.
    dropped: usize,

    /// До какого места дочитал каждый читатель.
    readers: HashMap<Reader, usize>,
}

impl<T: Clone> Journal<T> {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            dropped: 0,
            readers: HashMap::new(),
        }
    }

    /// Добавляет запись.
    pub fn push(&mut self, entry: T) {
        self.entries.push_back(entry);
        self.forget_read();
    }

    /// Сколько всего записей было за всё время.
    ///
    /// Именно это число подключение запоминает как «я дочитал досюда».
    pub fn count(&self) -> usize {
        self.dropped + self.entries.len()
    }

    /// Записывает читателя и говорит, сколько записей уже было.
    ///
    /// С этого мгновения журнал держит для него всё новое. Звать надо при
    /// появлении читателя: незаписанному журнал ничего не хранит.
    pub fn watch(&mut self, reader: Reader) -> usize {
        let count = self.count();

        self.readers.insert(reader, count);
        count
    }

    /// Записи, начиная с номера `from`, — то, что читателю осталось получить.
    ///
    /// Заодно журнал запоминает, что этот читатель дочитал до конца: после
    /// этого прочитанное всеми можно выбросить.
    pub fn since(&mut self, from: usize, reader: Reader) -> Vec<T> {
        let start = from.max(self.dropped) - self.dropped;

        let entries: Vec<T> = self
            .entries
            .iter()
            .skip(start.min(self.entries.len()))
            .cloned()
            .collect();

        self.readers.insert(reader, self.count());
        self.forget_read();

        entries
    }

    /// Отписывает читателя: он ушёл, и ждать его больше не надо.
    pub fn forget(&mut self, reader: Reader) {
        self.readers.remove(&reader);
        self.forget_read();
    }

    /// Выбрасывает записи, которые прочитали все.
    fn forget_read(&mut self) {
        let read_by_all = self
            .readers
            .values()
            .copied()
            .min()
            .unwrap_or_else(|| self.count());

        let mut drop = read_by_all.saturating_sub(self.dropped);

        // Застрявший читатель не должен держать журнал вечно.
        drop = drop.max(self.entries.len().saturating_sub(LIMIT));
        drop = drop.min(self.entries.len());

        self.entries.drain(..drop);
        self.dropped += drop;
    }

    /// Сколько записей журнал держит в памяти. Нужно для проверок.
    #[cfg(test)]
    pub fn kept(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Читатель получает то, чего ещё не видел, и ничего сверх того.
    #[test]
    fn a_reader_gets_what_it_has_not_seen() {
        let mut journal = Journal::new();

        let reader = 1;
        let from = journal.watch(reader);

        journal.push("первое");
        journal.push("второе");

        let read = journal.since(from, reader);
        assert_eq!(read, vec!["первое", "второе"]);

        // Второй раз то же самое не придёт.
        assert!(journal.since(journal.count(), reader).is_empty());
    }

    /// Прочитанное всеми выбрасывается, а нужное кому-то одному — остаётся.
    #[test]
    fn what_everyone_read_is_thrown_away() {
        let mut journal = Journal::new();

        let (first, second) = (1, 2);

        // Оба читателя записываются, пока журнал пуст.
        journal.watch(first);
        journal.watch(second);

        journal.push("первое");
        journal.push("второе");

        // Первый дочитал — но второй ещё нет, записи нужны.
        journal.since(0, first);
        assert_eq!(journal.kept(), 2);

        // Дочитал и второй — держать больше нечего.
        journal.since(0, second);
        assert_eq!(journal.kept(), 0);

        // Номера при этом не сбились.
        assert_eq!(journal.count(), 2);
    }

    /// Ушедший читатель больше не держит журнал.
    #[test]
    fn a_gone_reader_stops_holding_the_journal() {
        let mut journal = Journal::new();

        journal.watch(1);
        journal.watch(2);

        journal.push("запись");
        journal.since(journal.count(), 1);

        // Второй читатель ещё держит запись.
        assert_eq!(journal.kept(), 1);

        journal.forget(2);
        assert_eq!(journal.kept(), 0);
    }

    /// Без читателей журнал не копит ничего.
    #[test]
    fn without_readers_nothing_is_kept() {
        let mut journal = Journal::new();

        for _ in 0..1000 {
            journal.push("запись");
        }

        assert_eq!(journal.kept(), 0);
        assert_eq!(journal.count(), 1000);
    }

    /// Застрявший читатель не может раздуть журнал без предела.
    #[test]
    fn a_stuck_reader_does_not_grow_the_journal_forever() {
        let mut journal = Journal::new();

        journal.watch(1);

        for _ in 0..(LIMIT + 500) {
            journal.push("запись");
        }

        assert_eq!(journal.kept(), LIMIT);
    }
}
