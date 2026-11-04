// Примитивы пакетного уровня: чтение и запись пакетов, разбор VarInt и строк
// из тела пакета.
//
// Общие для всех состояний протокола (Configuration, Play).
//
// Формат любого пакета: VarInt длина тела, затем само тело, в начале которого
// идёт VarInt идентификатор пакета.

use std::io;
use std::sync::Arc;

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
    /// Дополнительные поля ConfirmTeleportation777; тело приведено к775.
    pub teleport: Option<mcsa_protocol::teleport::Confirmation>,
    /// Новый Punch777 отличаем от прежнего Arm Animation после перевода ID.
    pub punch: bool,
    /// Предсказания хэшей компонентов чужой версии не подтверждают инвентарь.
    pub foreign_inventory: bool,
    /// Старое подтверждение хранится отдельно от современных хэшей.
    pub legacy_inventory: Option<mcsa_protocol::legacy_inventory::Action>,
}

impl PacketInfo {
    pub fn teleport_confirmation(&self) -> Option<mcsa_protocol::teleport::Confirmation> {
        if self.id != 0 { return None; }
        self.teleport.or_else(|| mcsa_protocol::teleport::Confirmation::decode(wire::INTERNAL, self.payload()).ok())
    }

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
        teleport: None, punch: false, foreign_inventory: false, legacy_inventory: None,
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
    let body = configuration_body(stream.probe_776_enabled(), stream.client_version(), body).map_err(translate_error)?;
    let mut packet = Vec::with_capacity(body.len() + 8);

    frame(&mut packet, &body, compression_threshold());

    write_bytes(stream, &packet).await
}

fn configuration_body(probe_776: bool, version: JavaVersion, body: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, wire::Error> {
    if probe_776 {
        if version==JavaVersion::V1_21_4 { return mcsa_protocol::client_769::configuration(body); }
        if version==JavaVersion::V1_21_5 { return mcsa_protocol::client_770::configuration(body); }
        if version==JavaVersion::V1_21_6 { return mcsa_protocol::client_771::configuration(body); }
        if version==JavaVersion::V1_21_8 { return mcsa_protocol::client_772::configuration(body); }
        if version==JavaVersion::V1_21_9 { return mcsa_protocol::client_773::configuration(body); }
        if version == JavaVersion::V1_21_11 { return mcsa_protocol::client_774::configuration(body); }
        if version == JavaVersion::V26_2 { return mcsa_protocol::client_776::configuration(body); }
        return Err(wire::Error::UnsupportedVersion(version));
    }
    wire::translate(State::Configuration, Direction::Clientbound, wire::INTERNAL, version, body)
}

/// Реестр уже собран для целевого клиента. Обычный wire::translate не
/// пропускает произвольный RegistryData между версиями: NBT-схема не следует
/// из неизменного внешнего заголовка пакета. Здесь проверяем метку builder-а.
pub async fn write_registry<S: AsyncWrite + Unpin>(stream: &mut CryptoStream<S>, registry: &mcsa_protocol::registry::Registry) -> io::Result<()> {
    if registry.version != stream.client_version() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Реестр подготовлен для другой версии клиента"));
    }
    let body = registry.encode().map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("RegistryData: {error:?}")))?;
    let mut packet = Vec::with_capacity(body.len() + 8);
    frame(&mut packet, &body, compression_threshold());
    write_bytes(stream, &packet).await
}

/// Поток уже расшифрован CryptoStream, кадр уже распакован read_packet.
/// Эта функция применяется только в Play: одинаковый ID в другой фазе иной.
#[cfg(test)]
pub async fn read_play_packet<S: AsyncRead + Unpin>(stream: &mut S, version: JavaVersion) -> io::Result<PacketInfo> {
    read_play_packet_with_legacy(stream, version, None).await
}

#[cfg(test)]
pub async fn read_play_packet_with_legacy<S: AsyncRead + Unpin>(stream: &mut S, version: JavaVersion, legacy: Option<&super::connection::LegacySession>) -> io::Result<PacketInfo> {
    read_play_packet_routed(stream, version, legacy, false).await
}

#[cfg(test)]
pub(super) async fn read_play_packet_routed<S: AsyncRead + Unpin>(stream: &mut S, version: JavaVersion, legacy: Option<&super::connection::LegacySession>, probe_776: bool) -> io::Result<PacketInfo> {
    read_play_packet_with_content(stream, version, legacy, probe_776, Default::default()).await
}

#[cfg(test)]
static LEGACY_LOCK_PROBE: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(super) fn probe_legacy_lock_wait(sender: std::sync::mpsc::Sender<()>) {
    *LEGACY_LOCK_PROBE.lock().unwrap() = Some(sender);
}

pub(super) async fn read_play_packet_with_content<S: AsyncRead + Unpin>(stream: &mut S, version: JavaVersion, legacy: Option<&super::connection::LegacySession>, probe_776: bool, content: mcsa_protocol::content::ContentCodec) -> io::Result<PacketInfo> {
    if content.is_preview() && !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3) && !(probe_776 && matches!(version,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_2)) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Тестовый каталог требует клиента26.3"));
    }
    if probe_776 && !matches!(version,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9|JavaVersion::V1_21_11|JavaVersion::V26_2) {
        return Err(translate_error(wire::Error::UnsupportedVersion(version)));
    }
    let mut packet = read_packet(stream).await?;
    if content.is_preview() && version == JavaVersion::V26_1_2 && matches!(packet.id, 0x12 | 0x38) { packet.foreign_inventory=true; }
    if version == JavaVersion::V1_12_2 {
        let legacy = legacy.ok_or_else(|| io::Error::other("Нет состояния соединения1.12.2"))?;
        if matches!(packet.id, 0x05 | 0x07 | 0x1b) {
            let (body, action) = mcsa_protocol::legacy_inventory::decode(&packet.body,
                mcsa_protocol::legacy_items::Context { enchantments: legacy_enchantments() }).map_err(translate_error)?;
            // Только типизированный ACK имеет пустое тело и внутренний ID−1.
            // Такой пакет никогда не передаётся обычному обработчику.
            let id = if body.is_empty() { -1 } else { decode_varint(&body)?.0 };
            return Ok(PacketInfo { id, length: body.len() as i32, body, teleport: None,
                punch: false, foreign_inventory: false, legacy_inventory: action });
        }
        let body = loop {
            // Общий IO worker не ждёт std mutex старого encoder. Порядок
            // serverbound сохраняется: следующий кадр читается после этого.
            let ready = {
                match legacy.try_lock() {
                    Ok(mut session) => Some(session.serverbound(&packet.body).map_err(translate_error)),
                    Err(std::sync::TryLockError::Poisoned(_)) => return Err(io::Error::other("Состояние соединения1.12.2 повреждено")),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        #[cfg(test)]
                        if let Some(probe) = LEGACY_LOCK_PROBE.lock().unwrap().take() { let _ = probe.send(()); }
                        None
                    }
                }
            };
            if let Some(result) = ready { break result?; }
            tokio::time::sleep(Duration::from_millis(1)).await;
        };
        let (id, _) = decode_varint(&body)?;
        return Ok(PacketInfo { id, length: body.len() as i32, body, teleport: None, punch: false, foreign_inventory: false, legacy_inventory: None });
    }
    // Confirm имеет ID0 в обеих проверенных версиях. Его новые поля нужны
    // обработчику, поэтому не пропускаем их через обычный перевод в775.
    if packet.id == 0 && (matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3) || probe_776) {
        let confirmation = mcsa_protocol::teleport::Confirmation::decode(version, packet.payload())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("ConfirmTeleportation: {error:?}")))?;
        let mut body = vec![0];
        push_varint(&mut body, confirmation.id);
        return Ok(PacketInfo { id: 0, length: body.len() as i32, body, teleport: Some(confirmation), punch: false, foreign_inventory: false, legacy_inventory: None });
    }
    if probe_776 {
        let adapter = if version==JavaVersion::V1_21_4 {mcsa_protocol::client_769::serverbound} else if version==JavaVersion::V1_21_5 {mcsa_protocol::client_770::serverbound} else if version==JavaVersion::V1_21_6 {mcsa_protocol::client_771::serverbound} else if version==JavaVersion::V1_21_8 {mcsa_protocol::client_772::serverbound} else if version==JavaVersion::V1_21_9 {mcsa_protocol::client_773::serverbound} else if version==JavaVersion::V1_21_11 {mcsa_protocol::client_774::serverbound} else {mcsa_protocol::client_776::serverbound};
        let body = adapter(&packet.body, wire::Context { items: item_context(content), ..Default::default() })
            .map_err(translate_error)?.into_owned();
        let (id, _) = decode_varint(&body)?;
        return Ok(PacketInfo { id, length: body.len() as i32, body, teleport: None, punch: false, foreign_inventory: id == 0x12 || (matches!(version,JavaVersion::V1_21_4 | JavaVersion::V1_21_5 | JavaVersion::V1_21_6|JavaVersion::V1_21_8) && id==0x42) || (content.is_preview() && id == 0x38), legacy_inventory: None });
    }
    if version == wire::INTERNAL {
        if content.is_preview() && packet.id == 0x38 {
            let payload=packet.payload();
            let slot=payload.get(2..).ok_or_else(||io::Error::other("обрезан CreativeSlot775"))?;
            let (_,size)=mcsa_protocol::items::decode(version,slot,true,item_context(content))
                .map_err(|e|io::Error::new(io::ErrorKind::InvalidData,format!("CreativeSlot775: {e:?}")))?;
            if size!=slot.len(){return Err(io::Error::other("лишние данные CreativeSlot775"));}
        }
        return Ok(packet);
    }
    let punch = version == JavaVersion::V26_3 && packet.id == 0x2e;
    let body = wire::translate_with_context(State::Play, Direction::Serverbound,
        version, wire::INTERNAL, &packet.body, wire::Context { items: item_context(content), ..Default::default() }).map_err(translate_error)?.into_owned();
    let (id, _) = decode_varint(&body)?;
    // Родной клиент777 основного263 получает обычные поячеечные ответы;
    // полный снимок нужен при переходе между каталогами предметов.
    let foreign_inventory = id == 0x12 && !(version == JavaVersion::V26_3 && content.is_preview());
    Ok(PacketInfo { id, length: body.len() as i32, body, teleport: None, punch, foreign_inventory, legacy_inventory: None })
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

/// Предел транспорта неизменен; ответ читателя может снять только предел
/// текущего Keep Alive. Частичная запись продолжает ту же future без повтора.
async fn write_until_alive<W: AsyncWrite + Unpin>(stream: &mut W, bytes: &[u8], hard_deadline: Instant,
    alive: Option<&super::keep_alive::KeepAlive>,
) -> io::Result<()> {
    let mut changes = alive.map(super::keep_alive::KeepAlive::subscribe);
    let writing = async {
        stream.write_all(bytes).await?;
        stream.flush().await
    };
    tokio::pin!(writing);
    loop {
        let now = Instant::now();
        if alive.is_some_and(super::keep_alive::KeepAlive::closed) {
            return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "читатель подключения остановлен"));
        }
        if now >= hard_deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "срок записи в сокет истёк"));
        }
        if alive.is_some_and(super::keep_alive::KeepAlive::expired) {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "срок ответа Keep Alive истёк"));
        }
        let limit = alive.and_then(super::keep_alive::KeepAlive::deadline).map_or(hard_deadline, |deadline| deadline.min(hard_deadline));
        tokio::select! {
            biased;
            // После старого таймера снова читаем control: ACK мог уже снять его.
            _ = tokio::time::sleep_until(limit) => {}
            _ = async {
                match &mut changes {
                    Some(changes) => { let _ = changes.changed().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {}
            result = &mut writing => return result,
        }
    }
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
    chunk_biomes: Option<Arc<super::configuration::ChunkBiomes>>,
    content: mcsa_protocol::content::ContentCodec,
    stream: Transport<'a>,
    buffer: Vec<u8>,
    queued_at: Option<Instant>,
    control: Option<crate::sessions::Control>,
    deadline: Option<Instant>,
    keep_alive: Option<super::keep_alive::KeepAlive>,
    failed: bool,
    version: JavaVersion,
    probe_776: bool,
    entities: mcsa_protocol::entities::Tracker,
    legacy: Option<super::connection::LegacySession>,
}

enum Transport<'a> {
    Tcp(&'a mut Connection),
    #[cfg(test)]
    Duplex(&'a mut tokio::io::DuplexStream),
    #[cfg(test)]
    Capture,
}

pub(super) fn legacy_enchantments() -> &'static [&'static str] {
    static NAMES: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(||
        crate::stackdata::ENCHANTMENTS.iter().map(|(name, _)| *name).collect());
    &NAMES
}

fn item_context(content: mcsa_protocol::content::ContentCodec) -> mcsa_protocol::items::Context {
    // Оба профиля Configuration получают этот порядок; тест реестров это проверяет.
    mcsa_protocol::items::Context { shared_enchantment_count: Some(crate::stackdata::ENCHANTMENTS.len() as u32), content }
}

/// Неизменяемая часть кодирования чанка; ни сокет, ни Tracker в работу не уходят.
#[derive(Clone, Copy)]
pub(crate) struct ChunkEncoder {
    version: JavaVersion,
    content: mcsa_protocol::content::ContentCodec,
    threshold: i32,
}
pub(crate) struct PreparedChunkPacket {
    bytes: Vec<u8>,
    version: JavaVersion,
    content: mcsa_protocol::content::ContentCodec,
}
impl PreparedChunkPacket {
    pub fn len(&self) -> usize { self.bytes.len() }
}
impl ChunkEncoder {
    pub fn encode(self, body: &[u8], context: mcsa_protocol::chunk::Context) -> io::Result<PreparedChunkPacket> {
        if body.first() != Some(&0x2d) { return Err(io::Error::other("Фоновый кодировщик принимает только чанк")); }
        let content = if self.version == wire::INTERNAL { self.content.native_view() } else { self.content };
        let body = wire::translate_with_context(State::Play, Direction::Clientbound, wire::INTERNAL, self.version, body,
            wire::Context { chunk: Some(context), items: item_context(content), ..Default::default() }).map_err(translate_error)?;
        let mut bytes = Vec::new();
        frame(&mut bytes, &body, self.threshold);
        Ok(PreparedChunkPacket { bytes, version: self.version, content: self.content })
    }
}

impl<'a> Outgoing<'a> {
    pub fn new(stream: &'a mut Connection) -> Self {
        let version = stream.client_version();
        let legacy = stream.legacy.clone();
        let probe_776 = stream.probe_776_enabled();
        let content = stream.content;
        Self { chunk_biomes: stream.chunk_biomes.clone(), content, stream: Transport::Tcp(stream), buffer: Vec::with_capacity(OUTGOING_LIMIT), queued_at: None, control: None, deadline: None, keep_alive: None, failed: false, version, probe_776, entities: Default::default(), legacy }
    }

    /// Проверки пакетов без сетевых сокетов и запущенного сервера.
    #[cfg(test)]
    pub fn capture() -> Self { Self { chunk_biomes: None, content: Default::default(), stream: Transport::Capture, buffer: Vec::new(), queued_at: None, control: None, deadline: None, keep_alive: None, failed: false, version: wire::INTERNAL, probe_776: false, entities: Default::default(), legacy: None } }

    #[cfg(test)]
    pub(crate) fn capture_content(version: JavaVersion, content: mcsa_protocol::content::ContentCodec) -> Self {
        let mut out=Self::capture();out.version=version;out.content=content;out
    }

    #[cfg(test)]
    pub(crate) fn duplex(stream: &'a mut tokio::io::DuplexStream) -> Self {
        Self { chunk_biomes: None, content: Default::default(), stream: Transport::Duplex(stream), buffer: Vec::new(), queued_at: None, control: None, deadline: None, keep_alive: None, failed: false, version: wire::INTERNAL, probe_776: false, entities: Default::default(), legacy: None }
    }

    /// Кладёт пакет в очередь: VarInt длина, потом тело.
    pub async fn send(&mut self, body: &[u8]) -> io::Result<()> {
        self.send_with_context(body, wire::Context::default()).await
    }

    pub(crate) fn chunk_biomes(&self) -> io::Result<Arc<super::configuration::ChunkBiomes>> {
        if let Some(biomes) = &self.chunk_biomes { return Ok(biomes.clone()); }
        #[cfg(test)]
        return Ok(Arc::new(super::configuration::test_chunk_biomes(self.version, self.content.is_preview())));
        #[cfg(not(test))]
        Err(io::Error::other("Чанк до согласования реестров Configuration"))
    }

    pub(crate) fn chunk_encoder(&self) -> Option<ChunkEncoder> {
        (!self.probe_776 && matches!(self.version, JavaVersion::V26_1_2 | JavaVersion::V26_3))
            .then_some(ChunkEncoder { version: self.version, content: self.content, threshold: compression_threshold() })
    }

    pub(crate) async fn send_prepared_chunk(&mut self, packet: &PreparedChunkPacket) -> io::Result<()> {
        if packet.version != self.version || packet.content != self.content {
            return Err(io::Error::other("Чанк подготовлен для другого протокола/каталога"));
        }
        if self.version == JavaVersion::V26_3 { self.entities.prepared_chunk_sent(); }
        if self.queued_at.is_none() && timing_enabled() { self.queued_at = Some(Instant::now()); }
        self.buffer.extend_from_slice(&packet.bytes);
        if self.buffer.len() >= OUTGOING_LIMIT { self.flush().await?; }
        Ok(())
    }

    pub async fn send_chunk(&mut self, body: &[u8], context: mcsa_protocol::chunk::Context) -> io::Result<()> {
        self.send_with_context(body, wire::Context { chunk: Some(context), ..wire::Context::default() }).await
    }

    pub(super) fn client_version(&self) -> JavaVersion { self.version }

    pub fn legacy_view(&self) -> io::Result<Option<(mcsa_protocol::legacy_chunk::HeightWindow, bool)>> {
        self.legacy.as_ref().map(|legacy| {
            let legacy = legacy.lock().map_err(|_| io::Error::other("Состояние соединения1.12.2 повреждено"))?;
            let dimension = legacy.dimension.ok_or_else(|| io::Error::other("Нет измерения1.12.2"))?;
            Ok((legacy.window, dimension == 0))
        }).transpose()
    }

    pub async fn send_legacy_confirmation(&mut self, ack: mcsa_protocol::legacy_inventory::Confirmation) -> io::Result<()> {
        if self.version != JavaVersion::V1_12_2 { return Err(io::Error::other("Транзакция340 для другого клиента")); }
        frame(&mut self.buffer, &ack.packet(), compression_threshold());
        if self.buffer.len() >= OUTGOING_LIMIT { self.flush().await?; }
        Ok(())
    }

    pub async fn send_legacy_chunk(&mut self, chunk: &mcsa_protocol::legacy_chunk::Chunk) -> io::Result<()> {
        if self.version != JavaVersion::V1_12_2 { return Err(io::Error::other("Чанк340 для другого клиента")); }
        let (_, sky) = self.legacy_view()?.ok_or_else(|| io::Error::other("Нет окна1.12.2"))?;
        let mut body = vec![0x20]; body.extend(chunk.encode(sky).map_err(translate_error)?);
        frame(&mut self.buffer, &body, compression_threshold());
        if self.buffer.len() >= OUTGOING_LIMIT { self.flush().await?; }
        Ok(())
    }

    pub async fn send_login(&mut self, body: &[u8], online_mode: bool) -> io::Result<()> {
        self.send_with_context(body, wire::Context { online_mode: Some(online_mode), ..wire::Context::default() }).await
    }

    pub async fn send_entity_properties(&mut self, body:&[u8], clock:Option<i64>)->io::Result<()> {
        self.send_with_context(body,wire::Context{entity_clock:clock,..Default::default()}).await
    }

    async fn send_with_context(&mut self, body: &[u8], mut context: wire::Context) -> io::Result<()> {
        let measured = timing_enabled().then(Instant::now);
        if self.queued_at.is_none() { self.queued_at = measured; }
        // Configuration777: sulfur_cube_hot занимает42, прежние виды от
        // sweet_berry_bush сдвинуты на1. Внутренние Source остаются775;
        // основной26.1.2 сохраняет прежние пакеты для любого клиента.
        let remapped;
        let body = if self.content.is_preview() && body.first() == Some(&0x19) {
            let (_, entity_len) = decode_varint(&body[1..])?;
            let at = 1 + entity_len;
            let (kind, len) = decode_varint(&body[at..])?;
            if (42..50).contains(&kind) {
                remapped = [&body[..at], super::varint::encode_varint(kind + 1).as_slice(), &body[at + len..]].concat();
                remapped.as_slice()
            } else { body }
        } else { body };
        context.items = item_context(if self.version == JavaVersion::V26_1_2 { self.content.native_view() } else { self.content });
        if self.content.is_preview() && !matches!(self.version, JavaVersion::V26_1_2 | JavaVersion::V26_3) && !self.probe_776 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Тестовый каталог требует клиента26.3"));
        }
        // Оба Configuration-профиля используют REQUIRED_REGISTRIES в том же
        // порядке; имя/число записей ограничивает динамические metadata ID.
        static COUNTS: std::sync::LazyLock<Vec<(&'static str,u32)>> = std::sync::LazyLock::new(||
            super::configuration::REQUIRED_REGISTRIES.iter().map(|(name,entries)| (*name,entries.len() as u32))
                .chain(std::iter::once(("minecraft:damage_type", super::configuration::DAMAGE_TYPES.len() as u32))).collect());
        context.metadata_registry_counts = &COUNTS;
        if self.probe_776 {
            let adapter=if self.version==JavaVersion::V1_21_4 {mcsa_protocol::client_769::clientbound} else if self.version==JavaVersion::V1_21_5 {mcsa_protocol::client_770::clientbound} else if self.version==JavaVersion::V1_21_6 {mcsa_protocol::client_771::clientbound} else if self.version==JavaVersion::V1_21_8 {mcsa_protocol::client_772::clientbound} else if self.version==JavaVersion::V1_21_9 {mcsa_protocol::client_773::clientbound} else if self.version==JavaVersion::V1_21_11 {mcsa_protocol::client_774::clientbound} else {mcsa_protocol::client_776::clientbound};
            let body = adapter(&mut self.entities, body, context).map_err(translate_error)?;
            frame(&mut self.buffer, &body, compression_threshold());
        } else if self.version == JavaVersion::V1_12_2 {
            let legacy = self.legacy.as_ref().ok_or_else(|| io::Error::other("Нет состояния соединения1.12.2"))?;
            let translated = legacy.lock().map_err(|_| io::Error::other("Состояние соединения1.12.2 повреждено"))?
                .clientbound(body, legacy_enchantments()).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("Пакет340 {:02x?}: {error:?}", body.first())))?;
            for body in translated { frame(&mut self.buffer, &body, compression_threshold()); }
        } else if self.version == wire::INTERNAL {
            let body = self.entities.translate(wire::INTERNAL,self.version,body,context).map_err(translate_error)?;
            frame(&mut self.buffer, &body, compression_threshold());
        } else {
            for body in self.entities.translate_packets(wire::INTERNAL, self.version, body, context).map_err(translate_error)? {
                frame(&mut self.buffer, &body, compression_threshold());
            }
        }

        if let Some(start) = measured { eprintln!("NET_TIMING encode id={} us={}", body.first().copied().unwrap_or(0), start.elapsed().as_micros()); }
        if self.buffer.len() >= OUTGOING_LIMIT {
            self.flush().await?;
        }

        Ok(())
    }

    pub fn session_control(&self) -> Option<crate::sessions::Control> { self.control.clone() }

    pub fn control(&mut self, control: crate::sessions::Control) { self.control = Some(control); }

    pub fn deadline(&mut self, deadline: Option<Instant>) { self.deadline = deadline; }

    pub fn keep_alive(&mut self, alive: super::keep_alive::KeepAlive) { self.keep_alive = Some(alive); }

    /// Отправляет всё накопленное. После частичной записи продолжать поток
    /// нельзя: повтор с начала буфера испортил бы границы пакетов.
    pub async fn flush(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "запись уже оборвана"));
        }
        if self.buffer.is_empty() { return Ok(()); }
        let deadline = self.deadline.map_or(Instant::now() + WRITE_TIMEOUT,
            |deadline| deadline.min(Instant::now() + WRITE_TIMEOUT));
        let writing = timing_enabled().then(Instant::now);
        let bytes = self.buffer.len();
        let control = self.control.clone();
        let alive = self.keep_alive.clone();
        let result = tokio::select! {
            biased;
            result = async {
                match &mut self.stream {
                    Transport::Tcp(stream) => write_until_alive(*stream, &self.buffer, deadline, alive.as_ref()).await,
                    #[cfg(test)]
                    Transport::Duplex(stream) => write_until_alive(*stream, &self.buffer, deadline, alive.as_ref()).await,
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
        if let Some(start) = writing {
            eprintln!("NET_TIMING write bytes={bytes} queue_us={} write_us={}", self.queued_at.take().map_or(0, |at| start.duration_since(at).as_micros()), start.elapsed().as_micros());
        }
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
        self.control = None;
        self.keep_alive = None;
        self.deadline(Some(Instant::now() + Duration::from_millis(250)));
        if self.version == JavaVersion::V1_12_2 || self.probe_776 {
            if self.send(body).await.is_ok() { let _ = self.flush().await; }
            return;
        }
        let Ok(body) = wire::translate(State::Play, Direction::Clientbound,
            wire::INTERNAL, self.version, body) else { return; };
        frame(&mut self.buffer, &body, compression_threshold());
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

    #[tokio::test]
    async fn translated_punch_keeps_its_identity_apart_from_legacy_arm_animation() {
        for (version, body, punch) in [
            (JavaVersion::V26_3, vec![0x2e], true),
            (JavaVersion::V26_3, vec![0x3f,0], false),
            (JavaVersion::V26_1_2, vec![0x3f,0], false),
        ] {
            let mut bytes = Vec::new();
            frame(&mut bytes, &body, compression_threshold());
            let packet = read_play_packet_with_content(&mut bytes.as_slice(), version, None, false,
                mcsa_protocol::content::ContentCodec::white_slab_preview()).await.unwrap();
            assert_eq!(packet.id, 0x3f);
            assert_eq!(packet.payload(), [0]);
            assert_eq!(packet.punch, punch);
        }
    }

    #[tokio::test]
    async fn preview_keep_alive_uses_client_ids_and_replies_reach_the_canonical_handler() {
        use mcsa_protocol::{content::ContentCodec, ids::{self, RegistryKind}};
        use crate::network::keep_alive::{KeepAlive, KEEP_ALIVE_INTERVAL};
        for content in [ContentCodec::default(), ContentCodec::white_slab_preview()] {
            for version in [JavaVersion::V26_1_2, JavaVersion::V26_3, JavaVersion::V26_2, JavaVersion::V1_21_4] {
                let foreign = matches!(version, JavaVersion::V26_2 | JavaVersion::V1_21_4);
                let mut out = Outgoing::capture_content(version, content);
                out.probe_776 = foreign;
                let keep_alive = KeepAlive::new();
                // Используем настоящий pending ID, а не только проверку таблицы.
                tokio::time::pause();
                tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
                let id = keep_alive.send_due().unwrap();
                let mut request = vec![0x2c]; request.extend(id.to_be_bytes());
                out.send(&request).await.unwrap();
                let mut bytes = out.queued();
                let sent = read_packet(&mut bytes).await.unwrap();
                assert!(bytes.is_empty());
                let request_id = match version {
                    JavaVersion::V26_2 => 0x2c,
                    JavaVersion::V1_21_4 => 0x27,
                    _ => ids::translate(RegistryKind::PlayClientbound, wire::INTERNAL, version, 0x2c).unwrap(),
                };
                assert_eq!(sent.id as u32, request_id);
                assert_eq!(sent.payload(), id.to_be_bytes());
                // Общий wire-маршрут775→776/769 закрыт;
                // это реальные ID клиентов из закреплённых JSON, как в ботах.
                let response_id = match version {
                    JavaVersion::V26_2 => 0x1c,
                    JavaVersion::V1_21_4 => 0x1a,
                    _ => ids::translate(RegistryKind::PlayServerbound, wire::INTERNAL, version, 0x1c).unwrap() as i32,
                };
                let mut response = encode_varint(response_id);
                response.extend(sent.payload());
                let mut framed = Vec::new(); frame(&mut framed, &response, compression_threshold());
                let received = read_play_packet_with_content(&mut framed.as_slice(), version, None, foreign, content).await.unwrap();
                assert_eq!(received.id, 0x1c);
                assert!(keep_alive.answer(received.payload()));
                assert!(keep_alive.deadline().is_none());
                tokio::time::resume();
            }
        }
    }

    #[tokio::test]
    async fn native_outgoing_keeps_packet_size_guards() {
        let mut outgoing=Outgoing::capture();
        assert_eq!(outgoing.send(&[]).await.unwrap_err().kind(),io::ErrorKind::InvalidData);
        assert_eq!(outgoing.send(&vec![0;8*1024*1024+1]).await.unwrap_err().kind(),io::ErrorKind::InvalidData);
        assert!(outgoing.buffer.is_empty());
        outgoing.send(&[1]).await.unwrap();assert!(!outgoing.buffer.is_empty());
    }
    use crate::network::varint::encode_varint;

    #[tokio::test]
    async fn native_777_click_keeps_incremental_inventory_updates() {
        use mcsa_protocol::content::ContentCodec;
        for version in [JavaVersion::V26_1_2, JavaVersion::V26_3] {
            for preview in [false, true] {
                let content = if preview { ContentCodec::white_slab_preview() } else { ContentCodec::default() };
                let mut framed = Vec::new();
                // Окно0, state0, ячейка36, обычный щелчок, без предсказаний.
                frame(&mut framed, &[0x12,0,0,0,36,0,0,0,0], compression_threshold());
                let packet = read_play_packet_with_content(&mut framed.as_slice(), version, None, false, content).await.unwrap();
                assert_eq!(packet.id, 0x12);
                assert_eq!(packet.foreign_inventory, preview != (version == JavaVersion::V26_3));
            }
        }
    }

    #[tokio::test]
    async fn preview_damage_events_use_the_configuration_registry_for_both_clients() {
        use mcsa_protocol::content::ContentCodec;
        // Configuration официального777,24.10.2026: вставка sulfur_cube_hot42.
        for version in [JavaVersion::V26_1_2, JavaVersion::V26_3] {
            for preview in [false, true] {
                let content = if preview { ContentCodec::white_slab_preview() } else { ContentCodec::default() };
                for (old, name) in [(0,"arrow"),(41,"sting"),(42,"sweet_berry_bush"),(43,"thorns"),(44,"thrown"),(45,"trident"),(49,"wither_skull")] {
                    let mut out = Outgoing::capture_content(version, content);
                    out.send(&[0x19, 5, old, 7, 9, 0]).await.unwrap();
                    let mut bytes = out.queued();
                    let packet = read_packet(&mut bytes).await.unwrap();
                    let (_, used) = decode_varint(packet.payload()).unwrap();
                    let (kind, used_kind) = decode_varint(&packet.payload()[used..]).unwrap();
                    assert_eq!(&packet.payload()[used + used_kind..], &[7,9,0]);
                    assert!(bytes.is_empty());
                    if preview {
                        assert_eq!(super::super::configuration::DAMAGE_TYPES[kind as usize], format!("minecraft:{name}"));
                    } else { assert_eq!(kind, i32::from(old)); }
                }
            }
        }
    }

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
    async fn light_translation_crosses_encrypted_socket_with_observed_target_bytes() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let raw_client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (raw_server, _) = listener.accept().await.unwrap();
        let mut server = CryptoStream::new(raw_server);
        let mut client = CryptoStream::new(raw_client);
        server.protocol = Some(modern_profile());
        server.enable_encryption(&[19; 16]); client.enable_encryption(&[19; 16]);
        let mut old = vec![0x30, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15];
        old.extend(include_bytes!("../../tools/protocol-bridge/testdata/light-26.1.2-placed.bin"));
        let mut expected = vec![0x31, 255, 255, 255, 255, 15, 255, 255, 255, 255, 15];
        expected.extend(include_bytes!("../../tools/protocol-bridge/testdata/light-26.3-placed.bin"));
        let mut outgoing = Outgoing::new(&mut server);
        outgoing.send(&old).await.unwrap();
        outgoing.flush().await.unwrap();
        assert_eq!(read_packet(&mut client).await.unwrap().body, expected);
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
            for size in [1, 127, 256, 16000, 64000] {
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
        let alive = KeepAlive::new();
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

    #[tokio::test(start_paused = true)]
    async fn reader_ack_removes_stale_write_cap_without_repeating_encrypted_partial_bytes() {
        use crate::network::keep_alive::{KeepAlive, KEEP_ALIVE_INTERVAL};
        let alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let id = alive.send_due().unwrap();
        tokio::time::advance(Duration::from_secs(13)).await;
        let (writer, peer) = tokio::io::duplex(1);
        let mut writer = super::super::connection::CryptoStream::new(writer);
        let mut peer = super::super::connection::CryptoStream::new(peer);
        writer.enable_encryption(&[7; 16]);
        peer.enable_encryption(&[7; 16]);
        let writing_alive = alive.clone();
        let task = tokio::spawn(async move {
            write_until_alive(&mut writer, &[11, 22, 33, 44], Instant::now() + WRITE_TIMEOUT, Some(&writing_alive)).await
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(alive.answer(&id.to_be_bytes()));
        tokio::time::advance(Duration::from_secs(2)).await;
        let mut bytes = [0; 4];
        peer.read_exact(&mut bytes).await.unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(bytes, [11, 22, 33, 44]);
        assert_eq!(peer.read(&mut [0]).await.unwrap(), 0);
        assert!(!alive.expired());
    }

    #[tokio::test(start_paused = true)]
    async fn dynamic_write_without_reply_still_stops_at_original_keep_alive_deadline() {
        use crate::network::keep_alive::{KeepAlive, KEEP_ALIVE_INTERVAL};
        let alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        alive.send_due().unwrap();
        tokio::time::advance(Duration::from_secs(13)).await;
        let (mut writer, _peer) = tokio::io::duplex(1);
        let start = Instant::now();
        let error = write_until_alive(&mut writer, &[1, 2, 3], start + WRITE_TIMEOUT, Some(&alive)).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(start.elapsed(), Duration::from_secs(2));
        assert!(alive.expired());
        assert!(!alive.closed());
    }

    #[tokio::test(start_paused = true)]
    async fn reader_ack_does_not_extend_absolute_transport_timeout() {
        use crate::network::keep_alive::{KeepAlive, KEEP_ALIVE_INTERVAL};
        let alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let id = alive.send_due().unwrap();
        tokio::time::advance(Duration::from_secs(13)).await;
        let (mut writer, _peer) = tokio::io::duplex(1);
        let start = Instant::now();
        let writing_alive = alive.clone();
        let task = tokio::spawn(async move {
            write_until_alive(&mut writer, &[1, 2, 3], start + WRITE_TIMEOUT, Some(&writing_alive)).await
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(alive.answer(&id.to_be_bytes()));
        tokio::time::advance(WRITE_TIMEOUT - Duration::from_secs(1)).await;
        assert_eq!(task.await.unwrap().unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(start.elapsed(), WRITE_TIMEOUT);
        assert!(!alive.expired());
    }

    #[tokio::test(start_paused = true)]
    async fn reader_close_interrupts_blocked_output_and_disconnect_detaches_control() {
        use crate::network::keep_alive::KeepAlive;
        let alive = KeepAlive::new();
        let (mut writer, _peer) = tokio::io::duplex(1);
        let writing_alive = alive.clone();
        let start = Instant::now();
        let task = tokio::spawn(async move {
            write_until_alive(&mut writer, &[1, 2, 3], start + WRITE_TIMEOUT, Some(&writing_alive)).await
        });
        tokio::task::yield_now().await;
        alive.close();
        assert_eq!(task.await.unwrap().unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
        assert_eq!(start.elapsed(), Duration::ZERO);
        let (mut writer, mut peer) = tokio::io::duplex(1024);
        let mut out = Outgoing::duplex(&mut writer);
        out.keep_alive(alive);
        let component = crate::network::nbt::translated("disconnect.timeout");
        let mut body = vec![0x20];
        body.extend_from_slice(&component);
        out.disconnect(&body).await;
        assert!(out.keep_alive.is_none());
        assert_eq!(read_packet(&mut peer).await.unwrap().payload(), component);
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

#[cfg(test)]
#[path = "packet_776_tests.rs"]
mod tests_776;

// Диагностика включается явно и не меняет порядок пакетов.
pub(super) fn timing_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("MCSA_NET_TIMING").is_some());
    *ENABLED
}

// Отдельные участки подключения: ожидание сокета в эти замеры не входит.
pub(super) struct TimingSpan { name: &'static str, start: Option<Instant> }
impl TimingSpan {
    pub(super) fn new(name: &'static str) -> Self { Self { name, start: timing_enabled().then(Instant::now) } }
}
impl Drop for TimingSpan {
    fn drop(&mut self) {
        if let Some(start) = self.start && start.elapsed() >= Duration::from_millis(5) {
            eprintln!("NET_TIMING stage={} us={}", self.name, start.elapsed().as_micros());
        }
    }
}
