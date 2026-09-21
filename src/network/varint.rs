// Утилиты для чтения и записи типов, используемых в протоколе Minecraft:
// VarInt (целое число переменной длины) и String (VarInt-длина + UTF-8 байты).

use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Читает VarInt напрямую из TCP-потока.
///
/// Формат VarInt: каждый байт содержит 7 бит данных в младших битах,
/// старший бит (0x80) означает "есть следующий байт".
pub async fn read_varint(stream: &mut TcpStream) -> io::Result<i32> {
    let mut num_read: u32 = 0;
    let mut result: i32 = 0;

    loop {
        let byte = stream.read_u8().await?;
        let value = (byte & 0b0111_1111) as i32;
        result |= value << (7 * num_read);

        num_read += 1;
        if num_read > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "VarInt слишком длинный (больше 5 байт)",
            ));
        }

        if (byte & 0b1000_0000) == 0 {
            break;
        }
    }

    Ok(result)
}

/// Кодирует i32 в формат VarInt и возвращает получившиеся байты.
pub fn encode_varint(value: i32) -> Vec<u8> {
    let mut value = value as u32; // работаем как с беззнаковым для корректного сдвига
    let mut bytes = Vec::new();

    loop {
        let mut temp = (value & 0b0111_1111) as u8;
        value >>= 7;

        if value != 0 {
            temp |= 0b1000_0000;
        }

        bytes.push(temp);

        if value == 0 {
            break;
        }
    }

    bytes
}

/// Кодирует i64 в формат VarLong — то же самое, но для длинных чисел.
pub fn encode_varlong(value: i64) -> Vec<u8> {
    let mut value = value as u64;
    let mut bytes = Vec::new();

    loop {
        let mut temp = (value & 0b0111_1111) as u8;
        value >>= 7;

        if value != 0 {
            temp |= 0b1000_0000;
        }

        bytes.push(temp);

        if value == 0 {
            break;
        }
    }

    bytes
}

/// Читает строку протокола: VarInt длина (в байтах) + сами UTF-8 байты.
pub async fn read_string(stream: &mut TcpStream) -> io::Result<String> {
    let len = read_varint(stream).await? as usize;
    let mut buffer = vec![0u8; len];
    stream.read_exact(&mut buffer).await?;

    String::from_utf8(buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Кодирует строку в формат протокола: VarInt длина + UTF-8 байты.
pub fn encode_string(value: &str) -> Vec<u8> {
    let str_bytes = value.as_bytes();
    let mut result = encode_varint(str_bytes.len() as i32);
    result.extend_from_slice(str_bytes);
    result
}

/// Небольшой помощник — оставлен для возможного прямого использования
/// AsyncWriteExt в других модулях сети (например, для flush после записи).
#[allow(dead_code)]
pub async fn write_raw(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    stream.write_all(bytes).await
}
