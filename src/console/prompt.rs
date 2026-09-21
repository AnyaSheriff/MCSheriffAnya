// Строка ввода в консоли сервера.
//
// Беда, ради которой это написано: пока набираешь команду, сервер печатает
// в тот же терминал — и набранное разрывается посередине. Починить это можно
// только одним способом: держать набранное у себя и печатать лог не поверх
// него, а над ним, каждый раз перерисовывая строку ввода заново.
//
// Поэтому здесь лежит то, что игрок набрал, но ещё не отправил, и через это же
// место идёт весь вывод лога. Пока строки ввода нет (сервер запущен без
// терминала, вывод перенаправлен в файл), всё печатается как обычно.
//
// Используются простые команды терминала: «в начало строки» и «стереть
// строку». Их понимает любой терминал, и ничего больше нам не нужно.

use std::io::{self, Write};
use std::sync::Mutex;

/// Приглашение к вводу.
const PROMPT: &str = "> ";

/// В начало строки и стереть её.
const CLEAR_LINE: &str = "\r\x1b[2K";

/// Набранное и то, показывается ли строка ввода вообще.
static LINE: Mutex<Line> = Mutex::new(Line::new());

struct Line {
    /// Показывается ли строка ввода. Без терминала — нет.
    shown: bool,

    /// Что набрано, но ещё не отправлено.
    buffer: String,
}

impl Line {
    const fn new() -> Self {
        Self {
            shown: false,
            buffer: String::new(),
        }
    }
}

/// Захватывает набранное.
///
/// Замок берётся даже если его уронил упавший поток: печать лога важнее
/// целости строки ввода, а хуже, чем перепутанная строка, ничего не случится.
fn line() -> std::sync::MutexGuard<'static, Line> {
    LINE.lock().unwrap_or_else(|held| held.into_inner())
}

/// Включает строку ввода.
pub fn show() {
    let mut line = line();

    line.shown = true;
    redraw(&line);
}

/// Убирает строку ввода — при остановке сервера.
///
/// После неё печатается перевод строки: иначе приглашение оболочки окажется
/// на той же строке, где было наше.
pub fn hide() {
    let mut line = line();

    if line.shown {
        let mut out = io::stdout().lock();

        let _ = writeln!(out, "{}", CLEAR_LINE);
        let _ = out.flush();
    }

    line.shown = false;
    line.buffer.clear();
}

/// Печатает строку лога над строкой ввода.
pub fn print_above(message: &str) {
    let line = line();
    let mut out = io::stdout().lock();

    if line.shown {
        // Стираем строку ввода, печатаем сообщение и рисуем строку ввода
        // заново — уже под сообщением, вместе с набранным.
        let _ = write!(out, "{}{}\n{}{}", CLEAR_LINE, message, PROMPT, line.buffer);
    } else {
        let _ = writeln!(out, "{}", message);
    }

    let _ = out.flush();
}

/// Добавляет набранный знак.
pub fn push(symbol: char) {
    let mut line = line();

    line.buffer.push(symbol);
    redraw(&line);
}

/// Убирает последний знак — нажат Backspace.
pub fn backspace() {
    let mut line = line();

    line.buffer.pop();
    redraw(&line);
}

/// Забирает набранное: нажат Enter.
///
/// Набранное остаётся на экране отдельной строкой — как в любой консоли,
/// чтобы видеть, что именно было введено.
pub fn take() -> String {
    let mut line = line();
    let taken = std::mem::take(&mut line.buffer);

    let mut out = io::stdout().lock();

    let _ = write!(out, "{}{}{}\n{}", CLEAR_LINE, PROMPT, taken, PROMPT);
    let _ = out.flush();

    taken
}

/// Рисует строку ввода заново.
fn redraw(line: &Line) {
    if !line.shown {
        return;
    }

    let mut out = io::stdout().lock();

    let _ = write!(out, "{}{}{}", CLEAR_LINE, PROMPT, line.buffer);
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Набранное накапливается и забирается целиком, а Backspace убирает
    /// последний знак.
    ///
    /// Проверяется именно накопление: печать проверить нечем — она уходит
    /// в терминал, — а вот что набрано, важно.
    #[test]
    fn typed_characters_add_up() {
        // Строка ввода выключена: в проверках терминала нет, и рисовать
        // её некуда.
        push('s');
        push('t');
        push('o');
        push('p');
        push('!');
        backspace();

        assert_eq!(take(), "stop");

        // После Enter набранное пустое.
        assert_eq!(take(), "");
    }
}
