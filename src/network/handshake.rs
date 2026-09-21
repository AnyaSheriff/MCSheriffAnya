// Разбор пакета Handshake — первого пакета, который отправляет клиент
// при подключении к серверу.
//
// Формат пакета (без сжатия):
//   VarInt   packet_length   — длина пакета без учёта этого поля
//   VarInt   packet_id       — должен быть 0x00
//   VarInt   protocol_version
//   String   server_address
//   UShort   server_port     — 2 байта, big-endian
//   VarInt   next_state      — 1 = Status, 2 = Login

use std::io;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use super::varint::{read_string, read_varint};

/// Разобранные данные пакета Handshake.
#[derive(Debug)]
pub struct HandshakePacket {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: i32,
}

/// Читает и разбирает пакет Handshake из TCP-потока.
pub async fn read_handshake(stream: &mut TcpStream) -> io::Result<HandshakePacket> {
    // Длина всего пакета нам сейчас не нужна для разбора полей,
    // но её обязательно нужно прочитать, чтобы не сбить границы потока.
    let _packet_length = read_varint(stream).await?;

    let packet_id = read_varint(stream).await?;
    if packet_id != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Ожидался пакет Handshake (packet_id=0x00), получен 0x{:02X}",
                packet_id
            ),
        ));
    }

    let protocol_version = read_varint(stream).await?;
    let server_address = read_string(stream).await?;
    // read_u16 читает 2 байта в порядке big-endian, что соответствует
    // формату UShort в протоколе Minecraft.
    let server_port = stream.read_u16().await?;
    let next_state = read_varint(stream).await?;

    Ok(HandshakePacket {
        protocol_version,
        server_address,
        server_port,
        next_state,
    })
}
