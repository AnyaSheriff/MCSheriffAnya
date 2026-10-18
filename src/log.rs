// Вывод в консоль сервера.
//
// Строки печатаются в том же виде, что у оригинала: время, поток и уровень,
// затем само сообщение. Так консоль нашего сервера выглядит привычно, и по
// логу сразу видно, что когда произошло.
//
// Время местное — то же, что показывают часы на машине. Своей арифметики
// часового пояса здесь нет: её ведёт система, она же учитывает перевод часов.
//
// Подстановки в сообщение те же, что у обычной печати, поэтому вызовы в коде
// отличаются от прежних только именем:
//
//     log_info!("игрок {} вышел", name);
//     log_error!("не удалось прочитать пакет: {}", error);
//
// По умолчанию консоль немногословна — как у обычного сервера: кто зашёл и
// откуда, кто вышел, чат, команды, скины. Всё техническое (пакеты, движение,
// служебный обмен) идёт через log_debug! и печатается только при
// debug=true в config/mcsa.properties.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::io::IsTerminal;
use crate::colors::ConsoleColors;

/// Что печатается, если время у системы спросить не удалось.
const UNKNOWN_TIME: &str = "--:--:--";

/// Включён ли подробный лог. Задаётся один раз при запуске из настроек
/// (`config/mcsa.properties`, ключ `debug`).
static DEBUG: AtomicBool = AtomicBool::new(false);
static COLOR_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_console_colors(mode: ConsoleColors) {
    COLOR_MODE.store(match mode { ConsoleColors::Auto => 0, ConsoleColors::Always => 1, ConsoleColors::Never => 2 }, Ordering::Relaxed);
}

fn colors_enabled() -> bool {
    let mode = match COLOR_MODE.load(Ordering::Relaxed) { 1 => ConsoleColors::Always, 2 => ConsoleColors::Never, _ => ConsoleColors::Auto };
    static AUTO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let detected = *AUTO.get_or_init(|| ConsoleColors::Auto.enabled(std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
        std::env::var_os("TERM").is_some_and(|value| value == "dumb")));
    mode.enabled(detected, false, false)
}

fn format_line(level: &str, time: &str, text: &str, colors: bool) -> String {
    if !colors { return format!("[{time}] [Server thread/{level}]: {}", crate::colors::plain(text)); }
    let level_color = match level { "ERROR" => 91, "WARN" => 93, "DEBUG" => 90, _ => 92 };
    format!("\x1b[90m[{time}]\x1b[0m [Server thread/\x1b[{level_color}m{level}\x1b[0m]: {}", crate::colors::ansi(text))
}

/// Включает или выключает подробный лог.
pub fn set_debug(enabled: bool) {
    DEBUG.store(enabled, Ordering::Relaxed);
}

/// Печатать ли подробности.
pub fn debug_enabled() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

/// Подробность: пакеты, движение игроков, служебный обмен с клиентом.
/// Печатается только при включённом подробном логе; иначе строка даже
/// не собирается — в обычной игре это сотни сообщений в секунду.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::log::debug_enabled() {
            $crate::log::print("DEBUG", format_args!($($arg)*))
        }
    };
}

/// Обычное сообщение.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::log::print("INFO", format_args!($($arg)*))
    };
}

/// Предупреждение: что-то пошло не так, но работа продолжается.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::log::print("WARN", format_args!($($arg)*))
    };
}

/// Ошибка: что-то не удалось сделать.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::log::print("ERROR", format_args!($($arg)*))
    };
}

/// Печатает одну строку лога.
///
/// Наружу эта функция торчит только потому, что макросам нужно куда-то
/// передать собранную строку; в остальном коде пользуются макросами.
pub fn print(level: &str, message: std::fmt::Arguments) {
    let text = message.to_string();

    // Печатаем не прямо в терминал, а через консоль: там может идти набор
    // команды, и строку ввода надо перерисовать под сообщением, а не порвать.
    crate::console::prompt::print_above(&format_line(level, &local_time(), &text, colors_enabled()));

    // Для отчётов об ошибках, если владелец их включил.
    crate::reports::line(level, &crate::colors::plain(&text));
}

/// Текущее местное время в виде ЧЧ:ММ:СС (Windows: системные часы через WinAPI).
#[cfg(windows)]
pub(crate) fn local_time() -> String {
    let mut now: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };

    // SAFETY: функция только заполняет переданную структуру.
    unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut now) };

    format!("{:02}:{:02}:{:02}", now.wHour, now.wMinute, now.wSecond)
}

/// Текущее местное время в виде ЧЧ:ММ:СС.
#[cfg(unix)]
pub(crate) fn local_time() -> String {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_secs() as libc::time_t,
        Err(_) => return UNKNOWN_TIME.to_string(),
    };

    // Структуру заполняет система целиком, но начинать с нулей всё равно надо:
    // так в ней не остаётся мусора, если заполнение почему-то не случится.
    let mut broken_down: libc::tm = unsafe { std::mem::zeroed() };

    // SAFETY: функция пишет в переданную ей структуру и нигде её не сохраняет;
    // для одновременного вызова из разных потоков она и предназначена.
    let filled = unsafe { libc::localtime_r(&seconds, &mut broken_down) };

    if filled.is_null() {
        return UNKNOWN_TIME.to_string();
    }

    format!(
        "{:02}:{:02}:{:02}",
        broken_down.tm_hour, broken_down.tm_min, broken_down.tm_sec
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_levels_and_messages_reset_before_the_prompt() {
        for (level, color) in [("INFO", 92), ("WARN", 93), ("ERROR", 91), ("DEBUG", 90)] {
            let line = format_line(level, "12:34:56", "&aГотово&r!", true);
            assert!(line.contains(&format!("\x1b[{color}m{level}\x1b[0m")));
            assert!(line.contains("\x1b[38;2;85;255;85mГотово"));
            assert!(line.ends_with("\x1b[0m"));
            assert_eq!(format_line(level, "12:34:56", "&aГотово&r!", false),
                format!("[12:34:56] [Server thread/{level}]: Готово!"));
        }
    }
}
