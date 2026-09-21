// Примитивы пакетного уровня: чтение и запись пакетов, разбор VarInt и строк
// из тела пакета.
//
// Общие для всех состояний протокола (Configuration, Play).
//
// Формат любого пакета: VarInt длина тела, затем само тело, в начале которого
// идёт VarInt идентификатор пакета.

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::varint::{encode_varint, read_varint};

/// Разобранные сведения о прочитанном пакете.
pub struct PacketInfo {
    /// Идентификатор пакета.
    pub id: i32,
    /// Длина тела пакета в байтах.
    pub length: i32,
    /// Тело пакета целиком, включая идентификатор.
    pub body: Vec<u8>,
}

impl PacketInfo {
    /// Полезная нагрузка пакета — тело без идентификатора.
    ///
    /// Все поля пакета начинаются именно отсюда. Разбирать тело с начала
    /// (`body`) нельзя: тогда первым полем окажется сам идентификатор, и
    /// прочитанные числа будут мусором.
    pub fn payload(&self) -> &[u8] {
        match decode_varint(&self.body) {
            Ok((_, read)) => &self.body[read.min(self.body.len())..],
            Err(_) => &[],
        }
    }
}

/// Читает один пакет целиком: VarInt длина + тело длиной `length`.
///
/// Тело читается в буфер, чтобы границы потока не сбивались и можно было
/// безопасно пропускать пакеты, которые нам не нужны.
pub async fn read_packet(stream: &mut TcpStream) -> io::Result<PacketInfo> {
    let length = read_varint(stream).await?;
    if length < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Отрицательная длина пакета: {}", length),
        ));
    }

    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body).await?;

    let (packet_id, _) = decode_varint(&body)?;

    Ok(PacketInfo {
        id: packet_id,
        length,
        body,
    })
}

/// Записывает тело пакета в поток, добавив перед ним VarInt длину,
/// и сбрасывает буфер.
pub async fn write_body(stream: &mut TcpStream, body: Vec<u8>) -> io::Result<()> {
    let mut packet = encode_varint(body.len() as i32);
    packet.extend_from_slice(&body);

    stream.write_all(&packet).await?;
    stream.flush().await
}

/// Разбирает VarInt из среза байт.
/// Возвращает значение и количество прочитанных байт.
pub fn decode_varint(bytes: &[u8]) -> io::Result<(i32, usize)> {
    let mut result: i32 = 0;

    for (index, byte) in bytes.iter().enumerate().take(5) {
        result |= ((byte & 0b0111_1111) as i32) << (7 * index);

        if byte & 0b1000_0000 == 0 {
            return Ok((result, index + 1));
        }
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Некорректный VarInt в теле пакета",
    ))
}

/// Разбирает строку протокола из среза байт: VarInt длина + UTF-8 байты.
/// Сдвигает `offset` за прочитанную строку.
pub fn decode_string(bytes: &[u8], offset: &mut usize) -> io::Result<String> {
    let (length, read) = decode_varint(&bytes[*offset..])?;
    *offset += read;

    if length < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Отрицательная длина строки: {}", length),
        ));
    }

    let end = *offset + length as usize;
    if end > bytes.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Строка длиной {} не помещается в тело пакета (осталось {} байт)",
                length,
                bytes.len() - *offset
            ),
        ));
    }

    let value = String::from_utf8_lossy(&bytes[*offset..end]).into_owned();
    *offset = end;

    Ok(value)
}

/// Ошибка означает, что клиент закрыл соединение со своей стороны.
pub fn is_client_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof
    )
}
