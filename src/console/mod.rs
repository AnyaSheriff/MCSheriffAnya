// Консоль сервера: показ чата и ввод команд.
//
// Консоль — это ввод в том же терминале, из которого запущен сервер. Сюда
// печатается всё, что происходит в чате, и отсюда же можно вводить команды,
// как в оригинале.
//
// Если сервер запущен без терминала (например, в фоне), ввод заканчивается
// сразу. Это не ошибка: консоль в таком случае просто продолжает показывать
// происходящее, а команды вводить негде.
//
// Набранное консоль держит у себя, а не оставляет терминалу: иначе строка,
// которую набирают, разрывалась бы каждой строкой лога. Как это устроено —
// в prompt.rs и raw.rs.

pub mod prompt;
mod raw;

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::{mpsc, watch};

use crate::commands::{self, Effect, Source};
use crate::shared::Shared;
use crate::{log_info, log_warn};

/// Как часто заглядывать в ленту чата, чтобы напечатать новые строки.
///
/// Отдельного оповещения о новых строках нет: лента общая, и так же её читают
/// подключения игроков. Для консоли достаточно заглядывать часто — на глаз
/// задержка незаметна.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Сколько набранных строк может ждать обработки.
const INPUT_QUEUE: usize = 32;

/// Запускает консоль. Возвращается управление сразу, работа идёт в отдельных
/// задачах до остановки сервера.
pub fn start(shared: Arc<Shared>, shutdown: watch::Sender<bool>) {
    tokio::spawn(async move { run(shared, shutdown).await });
}

async fn run(shared: Arc<Shared>, shutdown: watch::Sender<bool>) {
    let (sender, mut input) = mpsc::channel::<String>(INPUT_QUEUE);

    // Свою строку ввода можно вести только в настоящем терминале. Если сервер
    // запущен иначе — читаем как раньше, целыми строками.
    let own_line = raw::enable();

    if own_line {
        prompt::show();
        tokio::spawn(read_keys(sender));
    } else {
        tokio::spawn(read_lines(sender));
    }

    // Что уже напечатано: старое повторять не нужно, иначе при запуске консоль
    // вывалила бы весь чат заново.
    let mut printed = shared
        .chat
        .lock()
        .expect("чат захвачен другим потоком")
        .count();

    let mut has_input = true;

    loop {
        print_new_chat(&shared, &mut printed);

        if !has_input {
            tokio::time::sleep(POLL_INTERVAL).await;
            continue;
        }

        tokio::select! {
            line = input.recv() => match line {
                Some(line) => handle_line(&line, &shared, &shutdown),
                None => {
                    has_input = false;
                    log_warn!("Команды вводить негде: сервер запущен без консоли");
                }
            },
            // Ctrl+C: терминал уже наш, и сам по себе он сервер не остановит —
            // останавливаем по-хорошему, как по команде stop.
            _ = tokio::signal::ctrl_c() => {
                let _ = shutdown.send(true);
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }
    }
}

/// Возвращает терминалу прежние настройки и убирает строку ввода.
///
/// Зовётся при остановке сервера: оставить чужой терминал без эха нельзя.
pub fn give_terminal_back() {
    prompt::hide();
    raw::restore();
}

/// Читает набираемое по знакам и собирает из него строки.
///
/// Здесь же разбираются Enter и Backspace: раз терминал больше не собирает
/// строку сам, это делаем мы.
async fn read_keys(sender: mpsc::Sender<String>) {
    /// Знак, с которого начинаются стрелки и прочие особые клавиши.
    const ESCAPE: u8 = 0x1B;

    /// Backspace — терминалы шлют то одно, то другое.
    const BACKSPACE: [u8; 2] = [0x08, 0x7F];

    let mut stdin = tokio::io::stdin();
    let mut chunk = [0u8; 64];

    // Знак может прийти не целиком: русская буква занимает два байта.
    let mut pending: Vec<u8> = Vec::new();

    // Идёт ли сейчас особая клавиша — её мы пропускаем целиком.
    let mut skipping = false;

    loop {
        let read = match stdin.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };

        for byte in &chunk[..read] {
            let byte = *byte;

            // Стрелки и прочее: пропускаем до конца записи о клавише.
            if skipping {
                skipping = !(0x40..=0x7E).contains(&byte) || byte == b'[';
                continue;
            }

            if byte == ESCAPE {
                skipping = true;
                pending.clear();
                continue;
            }

            if byte == b'\n' || byte == b'\r' {
                if sender.send(prompt::take()).await.is_err() {
                    return;
                }

                continue;
            }

            if BACKSPACE.contains(&byte) {
                prompt::backspace();
                continue;
            }

            // Остальные управляющие знаки набранным не считаются.
            if byte < 0x20 {
                continue;
            }

            pending.push(byte);

            match std::str::from_utf8(&pending) {
                Ok(text) => {
                    for symbol in text.chars() {
                        prompt::push(symbol);
                    }

                    pending.clear();
                }
                // Знак пришёл не целиком — ждём остальные байты. Если же байты
                // вообще не складываются в знак, выбрасываем их.
                Err(error) if error.error_len().is_some() => pending.clear(),
                Err(_) => {}
            }
        }
    }
}

/// Читает ввод целыми строками — когда терминала нет и собирать строку самим
/// незачем.
async fn read_lines(sender: mpsc::Sender<String>) {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if sender.send(line).await.is_err() {
            break;
        }
    }
}

/// Печатает строки чата, которых консоль ещё не показывала.
fn print_new_chat(shared: &Shared, printed: &mut usize) {
    let (lines, count) = {
        let chat = shared.chat.lock().expect("чат захвачен другим потоком");

        (chat.since(*printed).to_vec(), chat.count())
    };

    for line in &lines {
        log_info!("{}", line);
    }

    *printed = count;
}

/// Выполняет строку, набранную в консоли.
fn handle_line(line: &str, shared: &Shared, shutdown: &watch::Sender<bool>) {
    let command = line.trim();

    // Косую черту в консоли ставят по привычке; команда одинакова с ней и без неё.
    let command = command.strip_prefix('/').unwrap_or(command);

    if command.is_empty() {
        return;
    }

    let answer = commands::run(command, &Source::Console, shared);

    if !answer.reply.is_empty() {
        log_info!("{}", answer.reply);
    }

    if matches!(answer.effect, Effect::Stop) {
        let _ = shutdown.send(true);
    }
}
