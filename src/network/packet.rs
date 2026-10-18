// Примитивы пакетного уровня: чтение и запись пакетов, разбор VarInt и строк
// из тела пакета.
//
// Общие для всех состояний протокола (Configuration, Play).
//
// Формат любого пакета: VarInt длина тела, затем само тело, в начале которого
// идёт VarInt идентификатор пакета.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Duration, Instant, timeout_at};
use super::connection::Connection;
use super::connection::CryptoStream;
use mcsa_protocol::{Direction, State, JavaVersion, wire};

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
pub async fn write_body(stream: &mut Connection, body: Vec<u8>) -> io::Result<()> {
    write_configuration(stream, &body).await
}

fn translate_error(error: wire::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("Перевод Java-пакета: {error:?}"))
}

/// Единая граница Configuration, в том числе для диалогов AuthMe.
/// Содержимое переводится до сжатия, поток шифрует уже готовые кадры.
pub async fn write_configuration<S: AsyncWrite + Unpin>(stream: &mut CryptoStream<S>, body: &[u8]) -> io::Result<()> {
    let body = wire::translate(State::Configuration, Direction::Clientbound,
        wire::INTERNAL, stream.client_version(), body).map_err(translate_error)?;
    let mut packet = Vec::with_capacity(body.len() + 8);

    frame(&mut packet, &body, compression_threshold());

    write_bytes(stream, &packet).await
}

/// Поток уже расшифрован CryptoStream, кадр уже распакован read_packet.
/// Эта функция применяется только в Play: одинаковый ID в другой фазе иной.
pub async fn read_play_packet<S: AsyncRead + Unpin>(stream: &mut S, version: JavaVersion) -> io::Result<PacketInfo> {
    let packet = read_packet(stream).await?;
    if version == wire::INTERNAL { return Ok(packet); }
    let body = wire::translate(State::Play, Direction::Serverbound,
        version, wire::INTERNAL, &packet.body).map_err(translate_error)?.into_owned();
    let (id, _) = decode_varint(&body)?;
    Ok(PacketInfo { id, length: body.len() as i32, body })
}

/// Предел одной записи вместе со сбросом. Это защита транспорта, а не
/// новый срок ответа Keep Alive: его более ранняя граница учитывается ниже.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn write_bytes<W: AsyncWrite + Unpin>(stream: &mut W, bytes: &[u8]) -> io::Result<()> {
    write_until(stream, bytes, Instant::now() + WRITE_TIMEOUT).await
}

async fn write_until<W: AsyncWrite + Unpin>(stream: &mut W, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    timeout_at(deadline, async {
        stream.write_all(bytes).await?;
        stream.flush().await
    }).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "срок записи в сокет истёк"))?
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
    stream: Transport<'a>,
    buffer: Vec<u8>,
    control: Option<crate::sessions::Control>,
    deadline: Option<Instant>,
    failed: bool,
    version: JavaVersion,
}

enum Transport<'a> {
    Tcp(&'a mut Connection),
    #[cfg(test)]
    Duplex(&'a mut tokio::io::DuplexStream),
    #[cfg(test)]
    Capture,
}

impl<'a> Outgoing<'a> {
    pub fn new(stream: &'a mut Connection) -> Self {
        let version = stream.client_version();
        Self { stream: Transport::Tcp(stream), buffer: Vec::with_capacity(OUTGOING_LIMIT), control: None, deadline: None, failed: false, version }
    }

    /// Проверки пакетов без сетевых сокетов и запущенного сервера.
    #[cfg(test)]
    pub fn capture() -> Self { Self { stream: Transport::Capture, buffer: Vec::new(), control: None, deadline: None, failed: false, version: wire::INTERNAL } }

    #[cfg(test)]
    pub(crate) fn duplex(stream: &'a mut tokio::io::DuplexStream) -> Self {
        Self { stream: Transport::Duplex(stream), buffer: Vec::new(), control: None, deadline: None, failed: false, version: wire::INTERNAL }
    }

    /// Кладёт пакет в очередь: VarInt длина, потом тело.
    pub async fn send(&mut self, body: &[u8]) -> io::Result<()> {
        let body = wire::translate(State::Play, Direction::Clientbound,
            wire::INTERNAL, self.version, body).map_err(translate_error)?;
        frame(&mut self.buffer, &body, compression_threshold());

        if self.buffer.len() >= OUTGOING_LIMIT {
            self.flush().await?;
        }

        Ok(())
    }

    pub fn session_control(&self) -> Option<crate::sessions::Control> { self.control.clone() }

    pub fn control(&mut self, control: crate::sessions::Control) { self.control = Some(control); }

    pub fn deadline(&mut self, deadline: Option<Instant>) { self.deadline = deadline; }

    /// Отправляет всё накопленное. После частичной записи продолжать поток
    /// нельзя: повтор с начала буфера испортил бы границы пакетов.
    pub async fn flush(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "запись уже оборвана"));
        }
        if self.buffer.is_empty() { return Ok(()); }
        let deadline = self.deadline.map_or(Instant::now() + WRITE_TIMEOUT,
            |deadline| deadline.min(Instant::now() + WRITE_TIMEOUT));
        let control = self.control.clone();
        let result = tokio::select! {
            biased;
            result = async {
                match &mut self.stream {
                    Transport::Tcp(stream) => write_until(*stream, &self.buffer, deadline).await,
                    #[cfg(test)]
                    Transport::Duplex(stream) => write_until(*stream, &self.buffer, deadline).await,
                    #[cfg(test)]
                    Transport::Capture => Ok(()),
                }
            } => result,
            _ = async {
                match control {
                    Some(control) => control.cancelled().await,
                    None => std::future::pending().await,
                }
            } => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "подключение закрыто командой")),
        };
        if result.is_err() { self.failed = true; }
        #[cfg(test)]
        if matches!(self.stream, Transport::Capture) { return result; }
        self.buffer.clear();
        result
    }

    /// Причина ухода — только в ещё целый поток, с коротким сроком.
    /// Не ждём доставки прощания, чтобы сохранить и убрать игрока.
    pub async fn disconnect(&mut self, body: &[u8]) {
        if self.failed { return; }
        self.buffer.clear();
        let Ok(body) = wire::translate(State::Play, Direction::Clientbound,
            wire::INTERNAL, self.version, body) else { return; };
        frame(&mut self.buffer, &body, compression_threshold());
        self.control = None;
        self.deadline = Some(Instant::now() + Duration::from_millis(250));
        let _ = self.flush().await;
    }

    /// Сам сокет — для чтения после flush.
    pub fn stream(&mut self) -> &mut Connection {
        match &mut self.stream {
            Transport::Tcp(stream) => stream,
            #[cfg(test)]
            _ => panic!("проверка не читает сетевой сокет"),
        }
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

    fn modern_profile() -> mcsa_protocol::Negotiated {
        mcsa_protocol::Negotiated { client: JavaVersion::V26_3, native: wire::INTERNAL }
    }

    #[tokio::test]
    async fn versioned_play_crosses_the_real_encrypted_socket_boundary() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let raw_client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (raw_server, _) = listener.accept().await.unwrap();
        let mut server = CryptoStream::new(raw_server);
        let mut client = CryptoStream::new(raw_client);
        server.protocol = Some(modern_profile());
        server.enable_encryption(&[91; 16]);
        client.enable_encryption(&[91; 16]);
        let mut outgoing = Outgoing::new(&mut server);
        // Старый Move+Look: ground ПОСЛЕ двух углов. Новый — перед delta.
        outgoing.send(&[0x36, 9, 0, 1, 0, 2, 0xff, 0xfe, 127, 254, 1]).await.unwrap();
        let before = outgoing.buffer.clone();
        // Неизвестный формат не портит уже накопленный буфер.
        assert!(outgoing.send(&[0x2d]).await.is_err());
        assert_eq!(outgoing.buffer, before);
        outgoing.flush().await.unwrap();
        assert_eq!(read_packet(&mut client).await.unwrap().body,
            [0x37, 9, 1, 0, 1, 0, 2, 0xff, 0xfe, 127, 254]);
        let mut reason = vec![0x20];
        reason.extend(crate::network::nbt::text_component("Проверка версии"));
        outgoing.disconnect(&reason).await;
        let received = read_packet(&mut client).await.unwrap();
        assert_eq!(received.id, 0x20);
        assert_eq!(received.payload(), &reason[1..]);
    }

    #[tokio::test]
    async fn versioned_incoming_packets_translate_after_decryption() {
        let (server, client) = tokio::io::duplex(1024);
        let mut server = CryptoStream::new(server);
        let mut client = CryptoStream::new(client);
        server.protocol = Some(modern_profile());
        server.enable_encryption(&[13; 16]); client.enable_encryption(&[13; 16]);
        // held_item_slot сместился с 0x35 на 0x36 в JSON777. Поле Short прежнее.
        let original = vec![0x36, 0, 7];
        let mut framed = Vec::new(); frame(&mut framed, &original, compression_threshold());
        write_bytes(&mut client, &framed).await.unwrap();
        let received = read_play_packet(&mut server, JavaVersion::V26_3).await.unwrap();
        assert_eq!(received.id, 0x35);
        assert_eq!(received.payload(), &original[1..]);
        assert_eq!(received.length, received.body.len() as i32);
    }

    #[tokio::test]
    async fn login_success_has_a_fresh_session_and_preserves_signed_textures() {
        use mcsa_protocol::login::Success;
        let skin = crate::skins::Skin { value: "test-textures".into(), signature: "test-signature".into() };
        let mut previous = None;
        for version in [wire::INTERNAL, JavaVersion::V26_3, JavaVersion::V26_3] {
            for threshold in [-1, 0, 256] {
                let (server, client) = tokio::io::duplex(8192);
                let mut server = CryptoStream::new(server);
                let mut client = CryptoStream::new(client);
                server.protocol = Some(mcsa_protocol::Negotiated { client: version, native: wire::INTERNAL });
                server.enable_encryption(&[39; 16]); client.enable_encryption(&[39; 16]);
                super::super::login::send_login_success(&mut server, "McsaProbe", &[42; 16], Some(&skin), threshold).await.unwrap();
                let length = read_varint(&mut client).await.unwrap();
                let mut bytes = vec![0; length as usize]; client.read_exact(&mut bytes).await.unwrap();
                let body = if threshold < 0 { bytes } else { unpack(&bytes, threshold).unwrap() };
                assert_eq!(body[0], 2);
                let decoded = Success::decode(version, &body[1..]).unwrap();
                assert_eq!(decoded.uuid, [42; 16]); assert_eq!(decoded.username, "McsaProbe");
                assert_eq!(decoded.properties[0].value, skin.value);
                assert_eq!(decoded.properties[0].signature.as_deref(), Some(skin.signature.as_str()));
                assert_eq!(decoded.session_id, server.login_session_id);
                if version == JavaVersion::V26_3 {
                    let id = decoded.session_id.unwrap();
                    assert_ne!(Some(id), previous); assert_ne!(id, [42; 16]); assert_ne!(id, [0; 16]);
                    assert_eq!(id[6] >> 4, 4); assert_eq!(id[8] >> 6, 2);
                    previous = Some(id);
                } else { assert!(decoded.session_id.is_none()); }
            }
        }
    }

    #[tokio::test]
    async fn encryption_wraps_compressed_and_uncompressed_frames() {
        use super::super::connection::CryptoStream;
        for threshold in [-1, 0, 256] {
            let (server, client) = tokio::io::duplex(65536);
            let mut server = CryptoStream::new(server);
            let mut client = CryptoStream::new(client);
            server.enable_encryption(&[17; 16]);
            client.enable_encryption(&[17; 16]);
            for size in [1, 127, 256, 16000] {
                let body: Vec<_> = (0..size).map(|i| (i % 251) as u8).collect();
                let mut framed = Vec::new();
                frame(&mut framed, &body, threshold);
                write_bytes(&mut server, &framed).await.unwrap();
                let length = read_varint(&mut client).await.unwrap();
                let mut received = vec![0; length as usize];
                client.read_exact(&mut received).await.unwrap();
                let decoded = if threshold < 0 { received } else { unpack(&received, threshold).unwrap() };
                assert_eq!(decoded, body);
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn unread_peer_times_out_and_partial_frame_is_never_retried() {
        let (mut writer, mut peer) = tokio::io::duplex(1);
        let mut out = Outgoing::duplex(&mut writer);
        out.send(&[1, 2, 3]).await.unwrap();
        let start = Instant::now();
        assert_eq!(out.flush().await.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(start.elapsed(), WRITE_TIMEOUT);
        let mut byte = [0];
        peer.read_exact(&mut byte).await.unwrap();
        assert_eq!(out.flush().await.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        out.disconnect(&[9]).await;
        assert!(tokio::time::timeout(Duration::from_millis(1), peer.read_exact(&mut byte)).await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn keep_alive_deadline_interrupts_an_already_blocked_write() {
        use crate::network::keep_alive::{KeepAlive, KEEP_ALIVE_INTERVAL, KEEP_ALIVE_TIMEOUT};
        let mut alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        alive.send_due().unwrap();
        tokio::time::advance(KEEP_ALIVE_TIMEOUT - Duration::from_secs(2)).await;
        let (mut writer, _peer) = tokio::io::duplex(1);
        let mut out = Outgoing::duplex(&mut writer);
        out.deadline(alive.deadline());
        out.send(&[1, 2, 3]).await.unwrap();
        let start = Instant::now();
        assert_eq!(out.flush().await.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(start.elapsed(), Duration::from_secs(2));
        assert!(alive.expired());
    }

    #[tokio::test(start_paused = true)]
    async fn kick_interrupts_blocked_output_without_waiting_for_timeout() {
        let (mut writer, _peer) = tokio::io::duplex(1);
        let mut out = Outgoing::duplex(&mut writer);
        let control = crate::sessions::Control::default();
        out.control(control.clone());
        out.send(&[1, 2, 3]).await.unwrap();
        let start = Instant::now();
        let (result, ()) = tokio::join!(out.flush(), async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            control.disconnect(crate::sessions::Disconnect::duplicate());
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
        assert_eq!(start.elapsed(), Duration::from_millis(10));
    }

    #[tokio::test(start_paused = true)]
    async fn configuration_write_and_pending_flush_are_bounded() {
        use std::pin::Pin;
        use std::task::{Context, Poll};
        struct PendingFlush;
        impl AsyncWrite for PendingFlush {
            fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
                Poll::Ready(Ok(bytes.len()))
            }
            fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> { Poll::Pending }
            fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> { Poll::Pending }
        }
        let start = Instant::now();
        assert_eq!(write_bytes(&mut PendingFlush, &[1]).await.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(start.elapsed(), WRITE_TIMEOUT);
        let (mut writer, _peer) = tokio::io::duplex(1);
        assert_eq!(write_bytes(&mut writer, &[1, 2]).await.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn healthy_disconnect_contains_the_translation_key() {
        let (mut writer, mut peer) = tokio::io::duplex(1024);
        let mut out = Outgoing::duplex(&mut writer);
        let reason = crate::sessions::Disconnect::duplicate();
        let mut body = vec![0x20];
        body.extend_from_slice(&reason.component);
        out.disconnect(&body).await;
        let packet = read_packet(&mut peer).await.unwrap();
        assert_eq!(packet.payload(), reason.component);
        assert_eq!(crate::network::nbt::string_field(packet.payload(), "translate").as_deref(), Some("multiplayer.disconnect.duplicate_login"));
    }

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
        let (mut client, mut server) = tokio::io::duplex(OUTGOING_LIMIT * 2);
        let mut out = Outgoing::duplex(&mut server);

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
        let (mut client, mut server) = tokio::io::duplex(OUTGOING_LIMIT * 2);
        let mut out = Outgoing::duplex(&mut server);
        let big = vec![5u8; OUTGOING_LIMIT];

        out.send(&big).await.unwrap();
        assert!(out.queued().is_empty());

        let read = read_packet(&mut tokio::io::BufReader::new(&mut client)).await.unwrap();
        assert_eq!(read.body.len(), OUTGOING_LIMIT);
    }
}
