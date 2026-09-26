// Справка о сервере для тех, кто подключился не игрой: `telnet адрес порт`,
// браузер, `curl`.
//
// Как SSH называет себя строкой при подключении — только наоборот по
// порядку: в протоколе Minecraft клиент говорит первым, и строка от сервера
// до его первого пакета сломала бы вход настоящему клиенту. Поэтому сервер
// ждёт: клиент Minecraft сразу присылает Handshake — длину пакета и нулевой
// номер пакета, а telnet молчит или шлёт печатный текст. Молчит дольше
// `WAIT` или шлёт текст — ему отвечают справкой и закрывают соединение.

use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::config::server_properties::ServerProperties;
use crate::shared::Shared;

/// Сколько ждать первого пакета, прежде чем решить, что подключился не клиент
/// Minecraft. Клиент шлёт Handshake сразу после подключения — даже по
/// медленной связи это доли секунды.
const WAIT: Duration = Duration::from_secs(1);

/// Кто подключился.
pub enum Visitor {
    /// Клиент Minecraft: дальше обычный разбор.
    Game,
    /// Молчит — telnet или nc.
    Silent,
    /// Прислал текст; `http` — запрос браузера или curl.
    Text { http: bool },
}

/// Смотрит на начало потока, не забирая байты: игре они ещё нужны.
pub async fn classify(socket: &TcpStream) -> Visitor {
    let mut first = [0u8; 4];

    match tokio::time::timeout(WAIT, socket.peek(&mut first)).await {
        Err(_) => Visitor::Silent,
        Ok(Ok(read)) if read >= 2 && is_text(&first[..read]) => Visitor::Text {
            http: first[..read].starts_with(b"GET") || first[..read].starts_with(b"HEAD"),
        },
        _ => Visitor::Game,
    }
}

/// Текст ли это: у пакета Minecraft второй байт — ноль (номер Handshake) или
/// продолжение длины, первый байт которой не печатный; у текста оба печатные.
fn is_text(bytes: &[u8]) -> bool {
    bytes.iter().take(2).all(|&byte| byte.is_ascii_graphic() || byte == b' ' || byte == b'\r' || byte == b'\n')
}

/// Пишет справку и закрывает соединение.
pub async fn answer(mut socket: TcpStream, visitor: Visitor, properties: &ServerProperties, shared: &Shared) -> std::io::Result<()> {
    let text = describe(properties, shared);

    let body = match visitor {
        Visitor::Text { http: true } => format!(
            "HTTP/1.0 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            text.len(),
            text
        ),
        _ => text,
    };

    socket.write_all(body.as_bytes()).await?;
    socket.shutdown().await
}

/// Сама справка: что за сервер, какие версии и порты, кто играет.
fn describe(properties: &ServerProperties, shared: &Shared) -> String {
    let names: Vec<String> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .map(|member| member.name.clone())
        .collect();

    let mut lines = vec![
        format!("MCSheriffAnya {} — сервер Minecraft", env!("CARGO_PKG_VERSION")),
        properties.motd.clone(),
        String::new(),
        format!("Java Edition {} — этот порт ({}, TCP)", super::VERSION_NAME, properties.server_port),
    ];

    if shared.settings.bedrock_port != 0 {
        lines.push(format!(
            "Bedrock Edition {} — порт {} (UDP)",
            crate::bedrock::VERSIONS,
            shared.settings.bedrock_port
        ));
    }

    lines.push(match names.len() {
        0 => format!("Игроков: 0 из {}", properties.max_players),
        count => format!("Игроков: {} из {} — {}", count, properties.max_players, names.join(", ")),
    });
    lines.push(String::new());
    lines.push("https://github.com/AnyaSheriff/MCSheriffAnya".to_string());
    lines.push(String::new());

    lines.join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_is_not_text() {
        // Длина 24 и номер пакета 0 — начало Handshake.
        assert!(!is_text(&[24, 0, 0x87, 0x06]));
        // Старый пинг списка серверов.
        assert!(!is_text(&[0xfe, 0x01]));
        assert!(is_text(b"GET / HTTP/1.1"));
        assert!(is_text(b"hi\r\n"));
    }
}
