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
// debug = true в config/mcsheriffanya.toml.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Что печатается, если время у системы спросить не удалось.
const UNKNOWN_TIME: &str = "--:--:--";

/// Включён ли подробный лог. Задаётся один раз при запуске из настроек
/// (`config/mcsheriffanya.toml`, ключ `debug`).
static DEBUG: AtomicBool = AtomicBool::new(false);

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
    // Печатаем не прямо в терминал, а через консоль: там может идти набор
    // команды, и строку ввода надо перерисовать под сообщением, а не порвать.
    crate::console::prompt::print_above(&format!(
        "[{}] [Server thread/{}]: {}",
        local_time(),
        level,
        message
    ));
}

/// Текущее местное время в виде ЧЧ:ММ:СС.
fn local_time() -> String {
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
