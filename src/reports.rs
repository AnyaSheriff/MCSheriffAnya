// Отчёты об ошибках — анонимно и только с согласия владельца сервера.
//
// Включается ключом `send-error-reports=true` в config/mcsa.properties; по
// умолчанию выключено, и тогда здесь ничего не копится и никуда не уходит.
//
// Что считается ошибкой: падение потока (panic) и строка лога уровня ERROR.
// К отчёту прикладываются последние строки лога — чтобы было видно, что
// происходило перед ошибкой. Перед отправкой из всего вычищается то, по чему
// можно узнать людей и сервер: ники, адреса, опознаватели (UUID, XUID),
// строки чата, имя пользователя в путях. Вместо адреса сервера — случайный
// номер установки (config/install-id): по нему видно, что две ошибки с одного
// сервера, но не видно, с какого.
//
// Отчёт сначала ложится файлом в logs/reports/pending — сервер может упасть
// тут же, и тогда он уйдёт при следующем запуске, — потом отправляется в
// фоне на приёмник (tools/logreceiver). Отправленные переезжают в
// logs/reports/sent: владелец видит всё, что ушло.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{log_info, log_warn};

/// Куда отправляются отчёты.
const RECEIVER: &str = "https://log.sheriffanya.ru:7443/v1/report";

/// Папки отчётов: ждущие отправки и отправленные.
const PENDING: &str = "logs/reports/pending";
const SENT: &str = "logs/reports/sent";

/// Файл с номером установки.
const INSTALL_ID: &str = "config/install-id";

/// Сколько последних строк лога прикладывается к отчёту.
const LINES: usize = 50;

/// Одна и та же ошибка — не чаще раза в час; всего — не больше стольких
/// отчётов в сутки.
const SAME_AGAIN: u64 = 60 * 60;
const PER_DAY: usize = 30;

/// Сколько отправленных хранить.
const KEEP_SENT: usize = 100;

/// Как часто заглядывать в ждущие.
const SEND_EVERY: Duration = Duration::from_secs(30);

/// Включена ли отправка.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Что уже известно: последние строки лога, ники игроков, когда какая
/// ошибка отправлялась.
#[derive(Default)]
struct Memory {
    lines: VecDeque<String>,
    names: HashSet<String>,
    reported: HashMap<u64, u64>,
    today: VecDeque<u64>,
}

fn memory() -> &'static Mutex<Memory> {
    static MEMORY: OnceLock<Mutex<Memory>> = OnceLock::new();
    MEMORY.get_or_init(Mutex::default)
}

/// Включает отчёты: ловушка для падений и отправка в фоне. Вызывается
/// один раз при запуске, внутри tokio.
pub fn start(enabled: bool) {
    if !enabled {
        return;
    }

    ENABLED.store(true, Ordering::Relaxed);
    log_info!("Отчёты об ошибках включены: анонимно, на {} (config/mcsa.properties)", RECEIVER);

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map(|at| format!(" ({}:{})", at.file(), at.line())).unwrap_or_default();
        let text = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| text.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "паника без сообщения".to_string());

        record("panic", &format!("{}{}", text, place));
        previous(info);
    }));

    tokio::spawn(async {
        loop {
            send_pending().await;
            tokio::time::sleep(SEND_EVERY).await;
        }
    });
}

/// Строка лога — запоминается для будущих отчётов; строка ERROR сама
/// становится отчётом. Вызывается из `log::print` на каждую строку.
pub fn line(level: &str, text: &str) {
    if !ENABLED.load(Ordering::Relaxed) || level == "DEBUG" {
        return;
    }

    if let Ok(mut memory) = memory().lock() {
        memory.lines.push_back(format!("[{}] {}", level, text));
        while memory.lines.len() > LINES {
            memory.lines.pop_front();
        }
    }

    if level == "ERROR" {
        record("error", text);
    }
}

/// Запоминает ник игрока: его не должно быть в отчётах.
pub fn known_player(name: &str) {
    if ENABLED.load(Ordering::Relaxed)
        && let Ok(mut memory) = memory().lock()
    {
        memory.names.insert(name.to_string());
    }
}

/// Складывает отчёт в ждущие, если такой не отправлялся недавно.
fn record(kind: &str, message: &str) {
    let Ok(mut memory) = memory().lock() else {
        return;
    };
    let now = unix_now();
    let names: Vec<String> = memory.names.iter().cloned().collect();
    let message = clean(message, &names);
    let shape = fnv(format!("{}:{}", kind, without_digits(message.lines().next().unwrap_or(""))).as_bytes());

    if memory.reported.get(&shape).is_some_and(|at| now.saturating_sub(*at) < SAME_AGAIN) {
        return;
    }
    while memory.today.front().is_some_and(|at| now.saturating_sub(*at) > 24 * 60 * 60) {
        memory.today.pop_front();
    }
    if memory.today.len() >= PER_DAY {
        return;
    }
    memory.reported.insert(shape, now);
    memory.today.push_back(now);

    let lines: Vec<String> = memory.lines.iter().map(|line| clean(line, &names)).collect();
    drop(memory);

    let report = serde_json::json!({
        "format": 1,
        "install": install_id(),
        "version": env!("CARGO_PKG_VERSION"),
        "os": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        "kind": kind,
        "message": message,
        "lines": lines,
    });

    let folder = Path::new(PENDING);
    let path = folder.join(format!("{}-{:016x}.json", now, fnv(report.to_string().as_bytes())));
    if fs::create_dir_all(folder).and_then(|()| fs::write(&path, format!("{:#}\n", report))).is_err() {
        eprintln!("Отчёт об ошибке не записан в {}", path.display());
    }
}

/// Отправляет ждущие отчёты. Принятые и отвергнутые (неверный отчёт —
/// повторять незачем) переезжают в отправленные; при сбое сети — ждут.
async fn send_pending() {
    let Ok(entries) = fs::read_dir(PENDING) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    files.sort();

    let Some(client) = client() else {
        return;
    };

    for path in files {
        let Ok(body) = fs::read(&path) else {
            continue;
        };
        let answer = client
            .post(RECEIVER)
            .header("Content-Type", "application/json")
            .header("User-Agent", concat!("MCSheriffAnya/", env!("CARGO_PKG_VERSION")))
            .body(body)
            .send()
            .await;

        match answer.map(|answer| answer.status()) {
            Ok(status) if status.is_success() || status.is_client_error() && status.as_u16() != 429 => {
                if !status.is_success() {
                    log_warn!("Отчёт об ошибке {} не принят ({})", path.display(), status.as_u16());
                }
                let _ = fs::create_dir_all(SENT);
                let _ = fs::rename(&path, Path::new(SENT).join(path.file_name().unwrap_or_default()));
            }
            // Приёмник занят или недоступен — в другой раз.
            _ => return,
        }
    }

    prune_sent();
}

/// Отправленных — не больше KEEP_SENT, старые прочь.
fn prune_sent() {
    let Ok(entries) = fs::read_dir(SENT) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    files.sort();

    for old in files.iter().take(files.len().saturating_sub(KEEP_SENT)) {
        let _ = fs::remove_file(old);
    }
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();

    CLIENT.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(10)).build().ok()).as_ref()
}

/// Номер установки: 16 случайных шестнадцатеричных знаков, один раз на
/// сервер. Не записался — новый на каждый запуск, это не страшно.
fn install_id() -> String {
    static ID: OnceLock<String> = OnceLock::new();

    ID.get_or_init(|| {
        if let Ok(text) = fs::read_to_string(INSTALL_ID) {
            let id = text.trim().to_ascii_lowercase();
            if id.len() == 16 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return id;
            }
        }

        let mut bytes = [0u8; 8];
        if fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes)).is_err() {
            bytes = (SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_nanos()).unwrap_or(0) as u64).to_le_bytes();
        }
        let id = format!("{:016x}", u64::from_le_bytes(bytes));
        let _ = fs::write(INSTALL_ID, format!("{}\n", id));
        id
    })
    .clone()
}

/// Вычищает из строки то, по чему можно узнать людей и сервер.
fn clean(text: &str, names: &[String]) -> String {
    // Строка чата — целиком: что люди пишут, в отчётах не нужно.
    if let Some(at) = text.find("]: <")
        && text[at..].contains("> ")
    {
        return format!("{}]: <чат>", &text[..at]);
    }

    let mut out = text.to_string();

    // Длинные ники раньше коротких: «Аня» внутри «Аня2» не должна остаться.
    let mut names: Vec<&String> = names.iter().filter(|name| name.len() >= 2).collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    for name in names {
        out = out.replace(name.as_str(), "<игрок>");
    }

    let out = hide_tokens(&out);
    hide_home(&out)
}

/// Прячет «слова», похожие на адреса и опознаватели: IPv4 (с портом),
/// UUID (с чёрточками и без), длинные числа (XUID).
fn hide_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();

    let flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            out.push_str(&hide_word(word));
            word.clear();
        }
    };

    for char in text.chars() {
        if char.is_ascii_hexdigit() || char == '.' || char == '-' || char == ':' {
            word.push(char);
        } else {
            flush(&mut word, &mut out);
            out.push(char);
        }
    }
    flush(&mut word, &mut out);

    out
}

/// Одно «слово» из цифр, букв a–f, точек, чёрточек и двоеточий.
fn hide_word(word: &str) -> String {
    let trimmed = word.trim_matches(|char| char == '.' || char == ':' || char == '-');
    let (before, after) = word.split_at(word.find(trimmed).unwrap_or(0));
    let after = &after[trimmed.len()..];

    let hex: String = trimmed.chars().filter(|char| *char != '-').collect();
    let is_uuid = hex.len() == 32 && hex.chars().all(|char| char.is_ascii_hexdigit()) && (trimmed.len() == 32 || trimmed.len() == 36);
    let host = trimmed.split(':').next().unwrap_or("");
    let is_ipv4 = host.split('.').count() == 4 && host.split('.').all(|part| !part.is_empty() && part.len() <= 3 && part.parse::<u8>().is_ok());
    let is_long_number = trimmed.len() >= 12 && trimmed.chars().all(|char| char.is_ascii_digit());

    let replaced = if is_uuid {
        "<uuid>".to_string()
    } else if is_ipv4 {
        format!("<ip>{}", &trimmed[host.len()..])
    } else if is_long_number {
        "<id>".to_string()
    } else {
        trimmed.to_string()
    };

    format!("{}{}{}", before, replaced, after)
}

/// Имя пользователя в путях `/home/имя/`.
fn hide_home(text: &str) -> String {
    let mut out = text.to_string();
    let mut from = 0;

    while let Some(found) = out[from..].find("/home/") {
        let start = from + found + "/home/".len();
        let end = out[start..].find(|char: char| char == '/' || char.is_whitespace()).map_or(out.len(), |end| start + end);

        if end > start {
            out.replace_range(start..end, "<user>");
            from = start + "<user>".len();
        } else {
            from = start;
        }
    }

    out
}

fn without_digits(text: &str) -> String {
    text.chars().filter(|char| !char.is_ascii_digit()).collect()
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_and_places_are_hidden() {
        let names = vec!["SheriffAnya".to_string(), "SheriffAnya6650".to_string()];

        assert_eq!(
            clean("SheriffAnya6650[/66.90.91.234:8294] вошёл с Bedrock", &names),
            "<игрок>[/<ip>:8294] вошёл с Bedrock"
        );
        assert_eq!(
            clean("Опознаватель игрока SheriffAnya — 495b5264-926d-3087-8268-f937d40f5421", &names),
            "Опознаватель игрока <игрок> — <uuid>"
        );
        assert_eq!(clean("XUID «2535425101461547», путь /home/lenox/server", &names), "XUID «<id>», путь /home/<user>/server");
        assert_eq!(clean("[18:10:33] [Server thread/INFO]: <SheriffAnya> привет всем", &names), "[18:10:33] [Server thread/INFO]: <чат>");
        // Обычные числа и координаты остаются: без них ошибку не понять.
        assert_eq!(clean("такт 1300 занял 491 мс в 152.5 78 -110", &names), "такт 1300 занял 491 мс в 152.5 78 -110");
    }
}
