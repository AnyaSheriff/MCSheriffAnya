// Обработка состояния Login: клиент отправляет Login Start,
// сервер проверяет профиль через online.rs (если включён online-mode),
// затем отвечает Login Success.
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

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use super::varint::{encode_string, encode_varint, read_varint};

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
pub async fn read_login_start<R: AsyncRead + Unpin>(stream: &mut super::connection::CryptoStream<R>) -> io::Result<LoginStart> {
    let length = read_varint(stream).await?;
    let legacy = stream.client_version() == mcsa_protocol::JavaVersion::V1_12_2;
    if !(if legacy { 3..=18 } else { 18..=82 }).contains(&length) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "длина Login Start"));
    }
    let mut body = vec![0; length as usize];
    stream.read_exact(&mut body).await?;
    let (id, mut offset) = super::packet::decode_varint(&body)?;
    if id != 0 { return Err(io::Error::new(io::ErrorKind::InvalidData, "ожидался Login Start")); }
    let username = super::packet::decode_string(&body, &mut offset)?;
    let uuid = if legacy {
        if offset != body.len() || !valid_name(&username) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Login Start 1.12.2"));
        }
        // В340 клиент UUID вообще не присылает; это не кандидат для переноса прав.
        offline_uuid(&username)
    } else {
        body.get(offset..).and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "UUID Login Start"))?
    };

    Ok(LoginStart { username, uuid })
}

/// Отправляет пакет Login Disconnect: отказывает во входе и объясняет причину.
///
/// Это единственный пакет, в котором текст сообщения передаётся не так, как
/// везде: в фазе входа клиент ещё не получил реестры, поэтому текст здесь
/// записывается строкой JSON, а не сжатым видом, как в игре.
pub async fn send_login_disconnect<W: AsyncWrite + Unpin>(stream: &mut W, reason: &str) -> io::Result<()> {
    // Разметку JSON собираем сами: сообщение состоит из одного куска текста.
    let reason = format!("{{\"text\":\"{}\"}}", escape_json(reason));

    let mut body = encode_varint(0x00);
    body.extend_from_slice(&encode_string(&reason));

    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    super::packet::write_bytes(stream, &packet).await?;

    Ok(())
}

/// То же с готовым компонентом в виде JSON (блокировки: `translate` и аргументы).
pub async fn send_login_disconnect_json<W: AsyncWrite + Unpin>(stream: &mut W, json: &str) -> io::Result<()> {
    let mut body = encode_varint(0x00);
    body.extend_from_slice(&encode_string(json));

    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    super::packet::write_bytes(stream, &packet).await?;

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

/// Отправляет Set Compression (clientbound, 0x03) с порогом из настроек:
/// с него и всё дальнейшее идёт в сжатом виде (вики, Java Edition protocol,
/// «Set Compression»). Сам он уходит ещё несжатым. Порог −1 — пакета нет.
pub async fn send_set_compression<W: AsyncWrite + Unpin>(stream: &mut W, threshold: i32) -> io::Result<()> {
    if threshold < 0 {
        return Ok(());
    }

    let mut body = encode_varint(0x03);
    body.extend_from_slice(&encode_varint(threshold));

    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    super::packet::write_bytes(stream, &packet).await
}

/// Отправляет пакет Login Success: подтверждает вход игрока под указанным
/// именем и UUID. Подписанное свойство textures приходит от sessionserver.
pub async fn send_login_success<W: AsyncWrite + Unpin>(
    stream: &mut super::connection::CryptoStream<W>,
    username: &str,
    uuid: &[u8; 16],
    skin: Option<&crate::skins::Skin>,
    threshold: i32,
) -> io::Result<()> {
    use mcsa_protocol::{JavaVersion, login::{Property, Success}};
    use rsa::rand_core::{OsRng, RngCore};
    let version = stream.client_version();
    let session_id = if version == JavaVersion::V26_3 {
        let mut id = [0; 16];
        OsRng.try_fill_bytes(&mut id).map_err(io::Error::other)?;
        id[6] = (id[6] & 15) | 0x40;
        id[8] = (id[8] & 63) | 0x80;
        Some(id)
    } else { None };
    let properties = skin.into_iter().map(|skin| Property {
        name: "textures".into(), value: skin.value.clone(),
        signature: (!skin.signature.is_empty()).then(|| skin.signature.clone()),
    }).collect();
    let payload = if version == JavaVersion::V1_12_2 {
        mcsa_protocol::legacy_control::Packet::LoginSuccess { uuid: *uuid, username: username.into() }
            .encode().map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("Login Success340: {error:?}")))?.2
    } else {
        Success { uuid: *uuid, username: username.into(), properties, session_id }
            .encode(version).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("Login Success: {error:?}")))?
    };
    let mut body = encode_varint(0x02);
    body.extend(payload);

    // Полный пакет: длина тела + само тело, в сжатом виде, если оно включено.
    let mut packet = Vec::with_capacity(body.len() + 8);

    super::packet::frame(&mut packet, &body, threshold);

    super::packet::write_bytes(stream, &packet).await?;
    stream.login_session_id = session_id;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn legacy_login_has_no_client_uuid_and_rejects_trailing_identity() {
        use super::super::connection::CryptoStream;
        use mcsa_protocol::{JavaVersion, Negotiated};
        for trailing in [false, true] {
            let mut body = vec![0];
            body.extend(encode_string("McsaProbe"));
            if trailing { body.extend([42; 16]); }
            let mut bytes = encode_varint(body.len() as i32);
            bytes.extend(body);
            let mut stream = CryptoStream::new(bytes.as_slice());
            stream.protocol = Some(Negotiated { client: JavaVersion::V1_12_2, native: JavaVersion::V26_1_2 });
            let result = read_login_start(&mut stream).await;
            if trailing { assert!(result.is_err()); }
            else {
                let login = result.unwrap();
                assert_eq!(login.username, "McsaProbe");
                assert_eq!(login.uuid, offline_uuid("McsaProbe"));
            }
        }
    }

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
