// Обработка состояния Login: клиент отправляет Login Start,
// сервер отвечает Login Success.
//
// Формат Login Start (serverbound, без сжатия):
//   VarInt   packet_length
//   VarInt   packet_id = 0x00
//   String   username
//   UUID     16 байт
//
// Формат Login Success (clientbound, без сжатия):
//   VarInt   packet_length
//   VarInt   packet_id = 0x02
//   UUID     16 байт
//   String   username
//   VarInt   properties count

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::varint::{encode_string, encode_varint, read_string, read_varint};

/// Разобранные данные пакета Login Start.
#[derive(Debug)]
pub struct LoginStart {
    pub username: String,
    pub uuid: [u8; 16],
}

/// Годится ли ник: как у оригинала, от 1 до 16 знаков из латинских букв,
/// цифр и подчёркивания. Иной ник ломает других клиентов: в списке игроков
/// у них ник не длиннее 16 знаков, и длинный их отключает.
pub fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Опознаватель игрока без проверки Mojang — как у оригинала в режиме
/// `online-mode=false`: UUID версии 3 от строки «OfflinePlayer:<ник>».
///
/// Присланному клиентом опознавателю верить нельзя: к нему привязаны права
/// и файл игрока, и любой мог бы назваться опознавателем оператора.
pub fn offline_uuid(name: &str) -> [u8; 16] {
    use md5::{Digest, Md5};

    let mut uuid: [u8; 16] = Md5::digest(format!("OfflinePlayer:{}", name)).into();
    uuid[6] = (uuid[6] & 0x0f) | 0x30; // версия 3
    uuid[8] = (uuid[8] & 0x3f) | 0x80; // вариант IETF
    uuid
}

/// Форматирует 16 байт UUID в привычный вид 8-4-4-4-12 (для логов).
pub fn format_uuid(uuid: &[u8; 16]) -> String {
    let hex: String = uuid.iter().map(|b| format!("{:02x}", b)).collect();

    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Читает и разбирает пакет Login Start из TCP-потока.
pub async fn read_login_start(stream: &mut TcpStream) -> io::Result<LoginStart> {
    // Длина пакета нужна только для контроля границ потока.
    let _packet_length = read_varint(stream).await?;

    let packet_id = read_varint(stream).await?;
    if packet_id != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Ожидался пакет Login Start (packet_id=0x00), получен 0x{:02X}",
                packet_id
            ),
        ));
    }

    // Ник — String(16); проверка состава — у вызывающего.
    let username = read_string(stream, 16 * 4).await?;

    // UUID передаётся 16 «сырыми» байтами, а не строкой.
    let mut uuid = [0u8; 16];
    stream.read_exact(&mut uuid).await?;

    Ok(LoginStart { username, uuid })
}

/// Отправляет пакет Login Disconnect: отказывает во входе и объясняет причину.
///
/// Это единственный пакет, в котором текст сообщения передаётся не так, как
/// везде: в фазе входа клиент ещё не получил реестры, поэтому текст здесь
/// записывается строкой JSON, а не сжатым видом, как в игре.
pub async fn send_login_disconnect(stream: &mut TcpStream, reason: &str) -> io::Result<()> {
    // Разметку JSON собираем сами: сообщение состоит из одного куска текста.
    let reason = format!("{{\"text\":\"{}\"}}", escape_json(reason));

    let mut body = encode_varint(0x00);
    body.extend_from_slice(&encode_string(&reason));

    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    stream.write_all(&packet).await?;
    stream.flush().await?;

    Ok(())
}

/// То же с готовым компонентом в виде JSON (блокировки: `translate` и аргументы).
pub async fn send_login_disconnect_json(stream: &mut TcpStream, json: &str) -> io::Result<()> {
    let mut body = encode_varint(0x00);
    body.extend_from_slice(&encode_string(json));

    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    stream.write_all(&packet).await?;
    stream.flush().await?;

    Ok(())
}

/// Готовит текст для строки JSON: кавычки, обратные косые черты и переводы
/// строк в нём пришлось бы принять за разметку.
fn escape_json(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for symbol in text.chars() {
        match symbol {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }

    escaped
}

/// Отправляет пакет Login Success: подтверждает вход игрока под указанным
/// именем и UUID. Список properties пуст (свойства профиля не поддерживаются).
pub async fn send_login_success(
    stream: &mut TcpStream,
    username: &str,
    uuid: &[u8; 16],
) -> io::Result<()> {
    // Тело пакета: VarInt packet_id + UUID + String username + VarInt properties.
    let mut body = encode_varint(0x02);
    body.extend_from_slice(uuid);
    body.extend_from_slice(&encode_string(username));
    body.extend_from_slice(&encode_varint(0)); // properties count = 0

    // Полный пакет: VarInt длина тела + само тело.
    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    stream.write_all(&packet).await?;
    stream.flush().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Опознаватель считается так же, как у оригинала: для ника «Notch»
    /// без проверки Mojang это общеизвестное b50ad385-829d-3141-a216-7e7d7539ba7f.
    #[test]
    fn offline_uuid_matches_the_known_one() {
        assert_eq!(
            format_uuid(&offline_uuid("Notch")),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn names_are_checked_like_the_original() {
        assert!(valid_name("Anya_2008"));
        assert!(!valid_name(""));
        assert!(!valid_name("seventeen_letters"));
        assert!(!valid_name("Аня"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("../../etc"));
    }
}
