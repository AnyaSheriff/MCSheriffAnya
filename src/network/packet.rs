// Примитивы пакетного уровня: чтение и запись пакетов, разбор VarInt и строк
// из тела пакета.
//
// Общие для всех состояний протокола (Configuration, Play).
//
// Формат любого пакета: VarInt длина тела, затем само тело, в начале которого
// идёт VarInt идентификатор пакета.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::varint::read_varint;

/// Порог сжатия пакетов (`network-compression-threshold`): −1 — сжатия нет.
/// Один на весь сервер; включается для подключения пакетом Set Compression
/// в конце входа, поэтому всё, что читают и пишут эти функции (Configuration
/// и Play), уже идёт в сжатом виде, а статус и сам вход — нет
/// (вики, Java Edition protocol, «Packet format»).
static COMPRESSION: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// Задаёт порог сжатия из server.properties.
pub fn set_compression_threshold(threshold: i32) {
    COMPRESSION.store(threshold.max(-1), std::sync::atomic::Ordering::Relaxed);
}

/// Порог сжатия: пакеты не короче него идут сжатыми; −1 — не сжимаются.
pub fn compression_threshold() -> i32 {
    COMPRESSION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Упаковывает тело пакета в кадр: длина, при сжатии — ещё длина до сжатия
/// (0 — тело не сжато, оно короче порога) и само тело, сжатое zlib.
pub fn frame(out: &mut Vec<u8>, body: &[u8], threshold: i32) {
    if threshold < 0 {
        push_varint(out, body.len() as i32);
        out.extend_from_slice(body);
        return;
    }

    if (body.len() as i32) < threshold {
        // Нулевая длина до сжатия — один байт перед телом.
        push_varint(out, body.len() as i32 + 1);
        out.push(0);
        out.extend_from_slice(body);
        return;
    }

    use std::io::Write;

    let mut packed = flate2::write::ZlibEncoder::new(Vec::with_capacity(body.len() / 2 + 16), flate2::Compression::fast());
    // Запись в память не отказывает.
    let _ = packed.write_all(body);
    let packed = packed.finish().unwrap_or_default();
    let mut size = Vec::with_capacity(5);

    push_varint(&mut size, body.len() as i32);
    push_varint(out, (size.len() + packed.len()) as i32);
    out.extend_from_slice(&size);
    out.extend_from_slice(&packed);
}

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

/// Самый длинный пакет от клиента: длина — VarInt не длиннее трёх байт,
/// как у оригинала. Без предела заголовок «2 ГБ» заставил бы выделить 2 ГБ.
pub const MAX_PACKET: i32 = 2_097_151;

/// Читает один пакет целиком: VarInt длина + тело длиной `length`.
///
/// Тело читается в буфер, чтобы границы потока не сбивались и можно было
/// безопасно пропускать пакеты, которые нам не нужны.
///
/// Поток может быть и буферизованным: тогда длина и тело мелких пакетов
/// берутся из памяти, а не системным вызовом на каждый байт длины.
pub async fn read_packet<R: AsyncRead + Unpin>(stream: &mut R) -> io::Result<PacketInfo> {
    let length = read_varint(stream).await?;
    if !(1..=MAX_PACKET).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Недопустимая длина пакета: {}", length),
        ));
    }

    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body).await?;

    if compression_threshold() >= 0 {
        body = unpack(&body, compression_threshold())?;
    }

    let length = body.len() as i32;
    let (packet_id, _) = decode_varint(&body)?;

    Ok(PacketInfo {
        id: packet_id,
        length,
        body,
    })
}

/// Снимает сжатие с тела кадра: длина до сжатия (0 — не сжато) и данные.
fn unpack(frame: &[u8], threshold: i32) -> io::Result<Vec<u8>> {
    use std::io::Read;

    let (size, read) = decode_varint(frame)?;
    let data = &frame[read..];

    if size == 0 {
        return Ok(data.to_vec());
    }

    // Как у оригинала, длиннее предела распаковывать нельзя: иначе
    // крошечный пакет раздулся бы в гигабайты.
    if !(1..=8 * 1024 * 1024).contains(&size) || size < threshold {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Недопустимая длина сжатого пакета: {size}")));
    }

    let mut body = Vec::with_capacity(size as usize);

    flate2::read::ZlibDecoder::new(data).take(size as u64 + 1).read_to_end(&mut body)?;

    if body.len() != size as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Длина распакованного пакета не сошлась"));
    }

    Ok(body)
}

/// Записывает тело пакета в поток, добавив перед ним VarInt длину,
/// и сбрасывает буфер.
pub async fn write_body(stream: &mut TcpStream, body: Vec<u8>) -> io::Result<()> {
    let mut packet = Vec::with_capacity(body.len() + 8);

    frame(&mut packet, &body, compression_threshold());

    stream.write_all(&packet).await?;
    stream.flush().await
}

/// Сколько байт исходящих пакетов копится, прежде чем уйти в сокет, даже
/// если круг подключения ещё не кончился: так вход в мир или перенос, где
/// шлются сотни чанков, не держит в памяти их все разом.
const OUTGOING_LIMIT: usize = 64 * 1024;

/// Исходящие пакеты подключения.
///
/// Раньше каждый пакет уходил своей записью в сокет: при TCP_NODELAY это
/// системный вызов и отдельный сегмент на каждый блок, звук и шаг сущности.
/// Теперь пакеты складываются в буфер по порядку и уходят одной записью —
/// когда подключение собирается ждать (клиента или такт) или буфер вырос
/// (`OUTGOING_LIMIT`). Порядок пакетов прежний; у оригинала пакеты такта
/// тоже копятся и сбрасываются разом, в конце такта.
pub struct Outgoing<'a> {
    stream: &'a mut TcpStream,
    buffer: Vec<u8>,
}

impl<'a> Outgoing<'a> {
    pub fn new(stream: &'a mut TcpStream) -> Self {
        Self { stream, buffer: Vec::with_capacity(OUTGOING_LIMIT) }
    }

    /// Кладёт пакет в очередь: VarInt длина, потом тело.
    pub async fn send(&mut self, body: &[u8]) -> io::Result<()> {
        frame(&mut self.buffer, body, compression_threshold());

        if self.buffer.len() >= OUTGOING_LIMIT {
            self.flush().await?;
        }

        Ok(())
    }

    /// Отправляет всё накопленное.
    pub async fn flush(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let written = self.stream.write_all(&self.buffer).await;

        self.buffer.clear();
        written?;
        self.stream.flush().await
    }

    /// Сам сокет — для чтения. Перед чтением накопленное надо отправить
    /// (`flush`): клиент может ждать именно его.
    pub fn stream(&mut self) -> &mut TcpStream {
        self.stream
    }

    /// Сколько байт ждёт отправки — для проверок.
    #[cfg(test)]
    pub fn queued(&self) -> &[u8] {
        &self.buffer
    }
}

/// Дописывает VarInt в конец буфера, без промежуточного вектора.
pub fn push_varint(out: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;

    loop {
        let byte = (value & 0b0111_1111) as u8;
        value >>= 7;

        if value == 0 {
            out.push(byte);
            return;
        }

        out.push(byte | 0b1000_0000);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::varint::encode_varint;

    /// VarInt, дописанный в буфер, тот же, что и отдельным вектором.
    #[test]
    fn a_pushed_varint_is_the_encoded_one() {
        for value in [0, 1, 127, 128, 255, 25_565, 2_097_151, i32::MAX, -1] {
            let mut out = vec![7];
            push_varint(&mut out, value);

            assert_eq!(&out[1..], encode_varint(value).as_slice());
        }
    }

    /// Кадр со сжатием читается обратно: короткое тело идёт как есть (длина
    /// до сжатия 0), длинное — zlib.
    #[test]
    fn compressed_frames_round_trip() {
        for size in [1usize, 10, 255, 256, 5000] {
            let body: Vec<u8> = (0..size).map(|i| (i % 7) as u8).collect();
            let mut out = Vec::new();

            frame(&mut out, &body, 256);

            let (length, read) = decode_varint(&out).unwrap();

            assert_eq!(length as usize, out.len() - read);
            assert_eq!(unpack(&out[read..], 256).unwrap(), body);

            if size >= 256 {
                assert!(out.len() < size, "длинное тело сжато");
            } else {
                assert_eq!(out[read], 0, "короткое не сжато");
            }
        }
    }

    /// Пакеты уходят одной записью и в том порядке, в каком положены; мелкие
    /// ждут сброса, а прочитанное с той стороны — ровно пакеты по порядку.
    #[tokio::test]
    async fn packets_leave_together_and_in_order() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(address).await.unwrap();
        let (mut server, _) = listener.accept().await.unwrap();

        let mut out = Outgoing::new(&mut server);

        out.send(&[1, 10]).await.unwrap();
        out.send(&[2, 20, 21]).await.unwrap();
        assert_eq!(out.queued(), &[2, 1, 10, 3, 2, 20, 21]);

        out.flush().await.unwrap();
        assert!(out.queued().is_empty());

        let first = read_packet(&mut client).await.unwrap();
        let second = read_packet(&mut client).await.unwrap();

        assert_eq!((first.id, first.body), (1, vec![1, 10]));
        assert_eq!((second.id, second.body), (2, vec![2, 20, 21]));
    }

    /// Большой пакет не копится: буфер сбрасывается, как только перерос предел.
    #[tokio::test]
    async fn a_big_packet_goes_out_at_once() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(address).await.unwrap();
        let (mut server, _) = listener.accept().await.unwrap();

        let mut out = Outgoing::new(&mut server);
        let big = vec![5u8; OUTGOING_LIMIT];

        out.send(&big).await.unwrap();
        assert!(out.queued().is_empty());

        let read = read_packet(&mut tokio::io::BufReader::new(&mut client)).await.unwrap();
        assert_eq!(read.body.len(), OUTGOING_LIMIT);
    }
}
