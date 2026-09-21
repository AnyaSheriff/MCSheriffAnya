// Обработка состояния Status (Server List Ping):
// сервер получает Status Request и отвечает JSON-описанием
// (версия, количество игроков, MOTD), после чего отвечает на Ping
// пакетом Pong.
//
// Формат ответа (без сжатия):
//   VarInt   packet_length
//   VarInt   packet_id = 0x00
//   String   json_response
//
// Формат Ping/Pong (без сжатия):
//   VarInt   packet_length
//   VarInt   packet_id = 0x01
//   Long     timestamp      — 8 байт, big-endian

use std::io;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::handshake::HandshakePacket;
use super::varint::{encode_string, encode_varint, read_varint};
use crate::log_debug;

use super::{PROTOCOL_VERSION, VERSION_NAME};

/// Формирует JSON-ответ Server List Ping по спецификации протокола.
pub fn build_status_response(motd: &str, online: i32, max: i32) -> String {
    let response = json!({
        "version": {
            "name": VERSION_NAME,
            "protocol": PROTOCOL_VERSION
        },
        "players": {
            "max": max,
            "online": online
        },
        "description": {
            "text": motd
        }
    });

    response.to_string()
}

/// Обрабатывает состояние Status: читает пакет Status Request,
/// отправляет клиенту сформированный статус сервера, а затем
/// ожидает пакет Ping и отвечает на него пакетом Pong.
///
/// `handshake` принимается для доступа к данным рукопожатия
/// (например, для логирования адреса/версии клиента); сам ответ
/// Server List Ping не зависит от его полей.
pub async fn handle_status(
    stream: &mut TcpStream,
    handshake: &HandshakePacket,
    motd: &str,
    online: i32,
    max: i32,
) -> io::Result<()> {
    // Читаем пакет Status Request: VarInt длина пакета + VarInt packet_id.
    // Тело пакета пустое, поэтому больше ничего читать не нужно.
    let _packet_length = read_varint(stream).await?;
    let packet_id = read_varint(stream).await?;

    if packet_id != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Ожидался пакет Status Request (packet_id=0x00), получен 0x{:02X}",
                packet_id
            ),
        ));
    }

    log_debug!(
        "Status Request: protocol_version={}, address={}:{}",
        handshake.protocol_version, handshake.server_address, handshake.server_port
    );

    // Формируем JSON-ответ.
    let json_response = build_status_response(motd, online, max);

    // Тело пакета: VarInt packet_id (0x00) + String с JSON.
    let mut body = encode_varint(0x00);
    body.extend_from_slice(&encode_string(&json_response));

    // Полный пакет: VarInt длина тела + само тело.
    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    stream.write_all(&packet).await?;
    stream.flush().await?;

    log_debug!("Status Response отправлен клиенту");

    // --- Ping / Pong -------------------------------------------------------
    // Сразу после Status Response клиент отправляет Ping (packet_id = 0x01)
    // с 8-байтовым timestamp и ждёт ответный Pong с тем же значением.
    // Без ответа клиент показывает бесконечный пинг в списке серверов.
    //
    //   VarInt   packet_length
    //   VarInt   packet_id = 0x01
    //   Long     timestamp      — 8 байт, big-endian

    // Длина пакета Ping для разбора полей не нужна, но прочитать её
    // обязательно, чтобы не сбить границы потока.
    let _ping_length = read_varint(stream).await?;

    let ping_id = read_varint(stream).await?;
    if ping_id != 0x01 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Ожидался пакет Ping (packet_id=0x01), получен 0x{:02X}",
                ping_id
            ),
        ));
    }

    // read_i64 читает целое в порядке big-endian, что соответствует
    // типу Long в протоколе Minecraft.
    let timestamp = stream.read_i64().await?;
    log_debug!("Ping получен: timestamp={}", timestamp);

    // Тело Pong: VarInt packet_id (0x01) + Long timestamp (BE).
    let mut pong_body = encode_varint(0x01);
    pong_body.extend_from_slice(&timestamp.to_be_bytes());

    // Полный пакет: VarInt длина тела + само тело.
    let mut pong = encode_varint(pong_body.len() as i32);
    pong.extend_from_slice(&pong_body);

    stream.write_all(&pong).await?;
    stream.flush().await?;

    log_debug!("Pong отправлен: timestamp={}", timestamp);

    Ok(())
}
