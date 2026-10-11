// RakNet поверх UDP — транспорт Bedrock Edition. Свой, с нуля, по описанию
// протокола на вики (minecraft.wiki/w/RakNet): пинг для списка серверов,
// рукопожатие, кадры с надёжной и упорядоченной доставкой, разбиение
// больших сообщений, подтверждения ACK/NACK и повторная отправка.
//
// Одна задача держит сокет и все сессии. Готовые игровые сообщения (всё,
// что начинается с 0xFE) уходят обработчику игрока по каналу; ответы
// приходят обратно по другому каналу и раскладываются по кадрам здесь.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::log_debug;

/// «Волшебные» байты неподключённых сообщений RakNet.
const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

/// Версия протокола RakNet у современного Bedrock.
const RAKNET_VERSION: u8 = 11;

const UNCONNECTED_PING: u8 = 0x01;
const UNCONNECTED_PING_OPEN: u8 = 0x02;
const UNCONNECTED_PONG: u8 = 0x1c;
const OPEN_REQUEST_1: u8 = 0x05;
const OPEN_REPLY_1: u8 = 0x06;
const OPEN_REQUEST_2: u8 = 0x07;
const OPEN_REPLY_2: u8 = 0x08;
const INCOMPATIBLE: u8 = 0x19;
const CONNECTED_PING: u8 = 0x00;
const CONNECTED_PONG: u8 = 0x03;
const CONNECTION_REQUEST: u8 = 0x09;
const CONNECTION_ACCEPTED: u8 = 0x10;
const NEW_INCOMING: u8 = 0x13;
const DISCONNECT: u8 = 0x15;
const GAME: u8 = 0xfe;
const ACK: u8 = 0xc0;
const NACK: u8 = 0xa0;

/// Надёжности кадров.
const UNRELIABLE: u8 = 0;
const RELIABLE_ORDERED: u8 = 3;

/// Заголовок UDP и IP, который клиент учитывает в размере MTU.
const UDP_OVERHEAD: usize = 28;

/// Сколько места уходит на заголовок набора кадров и кадра с разбиением.
const DATAGRAM_HEADER: usize = 4;
const FRAME_HEADER: usize = 3 + 3 + 4 + 10;

/// Через сколько без ответа от клиента сессия считается оборванной.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Пределы срока, после которого неподтверждённый набор уходит снова;
/// сам срок считается по времени отклика, как у TCP.
const RESEND_MIN: Duration = Duration::from_millis(100);
const RESEND_MAX: Duration = Duration::from_secs(2);

/// Запас срока повтора сверх времени отклика: клиент подтверждает наборы
/// пачками и с задержкой, и без запаса на дальней связи срок истекает
/// раньше подтверждения — повтор зря, окно сжимается. Настоящую потерю
/// клиент и так назовёт в NACK.
const RESEND_SLACK: Duration = Duration::from_millis(100);

/// Окно перегрузки: сколько наборов может быть в пути без подтверждения.
/// Растёт с каждым подтверждением и вдвое сжимается при потере — иначе
/// медленный клиент тонет в повторах, и всё приходит с опозданием в секунды.
const WINDOW_START: f64 = 32.0;
const WINDOW_MIN: f64 = 8.0;
const WINDOW_MAX: f64 = 4096.0;

/// Как часто сессии отправляют накопленное и подтверждения.
const FLUSH_EVERY: Duration = Duration::from_millis(10);

/// Номера наборов и кадров у RakNet — 24-битные и идут по кругу.
const U24: u32 = 0xFF_FFFF;

/// Насколько далеко вперёд от ожидаемого принимаются номера надёжных и
/// упорядоченных кадров. Что дальше — либо старый повтор, либо чужая
/// выдумка: хранить такое ни к чему.
const RECEIVE_WINDOW: u32 = 4096;

/// Пределы сборки разбитых сообщений: сколько собирается одновременно, из
/// скольких частей может быть одно сообщение и сколько байтов всего может
/// ждать сборки и своей очереди в одной сессии.
const MAX_SPLITS: usize = 16;
const MAX_SPLIT_PARTS: u32 = 1024;
const MAX_WAITING_BYTES: usize = 8 * 1024 * 1024;

/// Сколько обёрток может ждать игровую часть. Больше — клиент шлёт быстрее,
/// чем их можно разобрать: такое соединение закрывается.
const INBOUND_QUEUE: usize = 1024;

/// Сколько диапазонов разбирается в одном ACK или NACK.
const MAX_RANGES: u16 = 512;

/// На сколько номер `a` впереди `b` по кругу 24-битных номеров.
fn ahead(a: u32, b: u32) -> u32 {
    a.wrapping_sub(b) & U24
}

/// Что сервер говорит о себе в списке серверов.
pub struct Status {
    pub motd: String,
    pub world: String,
    pub online: usize,
    pub max: usize,
    pub port: u16,
}

/// Событие для игровой части.
pub enum Event {
    /// Клиент подключился по RakNet: дальше пойдут игровые сообщения.
    Connected { addr: SocketAddr, inbound: mpsc::Receiver<Vec<u8>>, outbound: Sender },
}

/// Куда игровая часть отдаёт сообщения для клиента (тело — без 0xFE).
///
/// Сессия с того же адреса может смениться (клиент переподключился), поэтому
/// у каждой — свой номер поколения: сообщения старой в новую не попадут.
#[derive(Clone)]
pub struct Sender {
    addr: SocketAddr,
    generation: u64,
    queue: mpsc::UnboundedSender<Queued>,
}

/// Сообщение для отправки: кому (адрес и поколение сессии) и что.
type Queued = (SocketAddr, u64, Outgoing);

enum Outgoing {
    Game(Vec<u8>),
    Close,
}

impl Sender {
    /// Отправить игровое сообщение: надёжно и по порядку.
    pub fn send(&self, body: Vec<u8>) -> bool {
        self.queue.send((self.addr, self.generation, Outgoing::Game(body))).is_ok()
    }

    /// Закрыть соединение.
    pub fn close(&self) {
        let _ = self.queue.send((self.addr, self.generation, Outgoing::Close));
    }
}

/// Состояние одного клиента.
struct Session {
    mtu: usize,
    guid: i64,
    /// Номер поколения: у каждой новой сессии свой.
    generation: u64,
    connected: bool,
    last_seen: Instant,
    ack_progress: Option<Instant>,

    // Отправка.
    send_sequence: u32,
    reliable_index: u32,
    order_index: u32,
    split_id: u16,
    /// Кадры, ждущие отправки (уже с заголовками).
    pending: VecDeque<Vec<u8>>,
    /// Отправленные наборы кадров с надёжными кадрами — на случай NACK.
    unacked: BTreeMap<u32, Sent>,
    /// Окно перегрузки и порог, после которого оно растёт медленно.
    window: f64,
    threshold: f64,
    /// Номер первого набора, отправленного после последнего сжатия окна:
    /// потери более ранних наборов — та же перегрузка, и окно второй раз
    /// за неё не сжимается (как у TCP). Иначе на дальней связи окно за
    /// несколько тактов отправки падает до минимума.
    recover: u32,
    /// Сглаженное время отклика и его разброс.
    rtt: Duration,
    rtt_spread: Duration,
    /// Сколько раз подряд срок повтора истёк без подтверждений: каждый раз
    /// срок вдвое длиннее, как у TCP, — иначе на медленной связи сервер
    /// забивает её повторами.
    backoff: u32,

    // Приём.
    /// Номера наборов, которые пора подтвердить.
    to_ack: Vec<u32>,
    /// Какой номер набора ждём следующим — для NACK на пропуски.
    expected_sequence: u32,
    /// Уже принятые надёжные кадры: повтор не отдаётся игре дважды.
    seen_reliable: HashSet<u32>,
    lowest_reliable: u32,
    /// Упорядоченные кадры, пришедшие раньше своей очереди.
    order_expected: u32,
    order_waiting: BTreeMap<u32, Vec<u8>>,
    /// Недособранные разбитые сообщения.
    splits: HashMap<u16, (u32, HashMap<u32, Vec<u8>>)>,
    /// Сколько байтов лежит в недособранных и ждущих очереди сообщениях.
    waiting_bytes: usize,

    /// Куда отдаются игровые сообщения.
    inbound: Option<mpsc::Sender<Vec<u8>>>,
}

impl Session {
    fn new(mtu: usize, guid: i64) -> Session {
        static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        Session {
            mtu,
            guid,
            generation: GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            connected: false,
            last_seen: Instant::now(),
            ack_progress: None,
            send_sequence: 0,
            reliable_index: 0,
            order_index: 0,
            split_id: 0,
            pending: VecDeque::new(),
            unacked: BTreeMap::new(),
            window: WINDOW_START,
            threshold: WINDOW_MAX,
            recover: 0,
            rtt: Duration::from_millis(100),
            rtt_spread: Duration::from_millis(50),
            backoff: 0,
            to_ack: Vec::new(),
            expected_sequence: 0,
            seen_reliable: HashSet::new(),
            lowest_reliable: 0,
            order_expected: 0,
            order_waiting: BTreeMap::new(),
            splits: HashMap::new(),
            waiting_bytes: 0,
            inbound: None,
        }
    }

    // Пинги и пустые ACK не означают, что клиент принимает наши данные.
    // [проверить] Для отсутствия продвижения используем тот же срок RakNet.
    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.last_seen) >= TIMEOUT
            || self.ack_progress.is_some_and(|at| now.duration_since(at) >= TIMEOUT)
    }

    /// Кладёт сообщение в очередь на отправку, при надобности разбивая его.
    fn queue(&mut self, body: &[u8], reliability: u8) {
        let room = self.mtu - UDP_OVERHEAD - DATAGRAM_HEADER - FRAME_HEADER;

        if body.len() <= room {
            let frame = self.frame(body, reliability, None);
            self.pending.push_back(frame);
            return;
        }

        // Большое сообщение — частями. Разбитые кадры всегда надёжные.
        let parts: Vec<&[u8]> = body.chunks(room).collect();
        let id = self.split_id;
        self.split_id = self.split_id.wrapping_add(1);
        let order = self.order_index;

        for (index, part) in parts.iter().enumerate() {
            let frame = self.frame_with_order(part, RELIABLE_ORDERED, Some((parts.len() as u32, id, index as u32)), order);
            self.pending.push_back(frame);
        }

        self.order_index = (self.order_index + 1) & U24;
    }

    fn frame(&mut self, body: &[u8], reliability: u8, split: Option<(u32, u16, u32)>) -> Vec<u8> {
        let order = self.order_index;

        if reliability == RELIABLE_ORDERED {
            self.order_index = (self.order_index + 1) & U24;
        }

        self.frame_with_order(body, reliability, split, order)
    }

    fn frame_with_order(&mut self, body: &[u8], reliability: u8, split: Option<(u32, u16, u32)>, order: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(body.len() + FRAME_HEADER);
        let flags = (reliability << 5) | if split.is_some() { 0x10 } else { 0 };

        out.push(flags);
        out.extend_from_slice(&((body.len() * 8) as u16).to_be_bytes());

        if reliability == RELIABLE_ORDERED {
            push_u24(&mut out, self.reliable_index);
            self.reliable_index = (self.reliable_index + 1) & U24;
            push_u24(&mut out, order);
            out.push(0); // канал упорядочивания
        }

        if let Some((count, id, index)) = split {
            out.extend_from_slice(&count.to_be_bytes());
            out.extend_from_slice(&id.to_be_bytes());
            out.extend_from_slice(&index.to_be_bytes());
        }

        out.extend_from_slice(body);
        out
    }

    /// Собирает накопленные кадры в наборы по размеру MTU — сколько
    /// позволяет окно; остальное ждёт подтверждений.
    fn take_datagrams(&mut self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let room = self.mtu - UDP_OVERHEAD - DATAGRAM_HEADER;

        while !self.pending.is_empty() && (self.unacked.len() as f64) < self.window {
            let mut current: Vec<Vec<u8>> = Vec::new();
            let mut size = 0;

            while let Some(frame) = self.pending.front() {
                if size + frame.len() > room && !current.is_empty() {
                    break;
                }

                size += frame.len();
                current.extend(self.pending.pop_front());
            }

            out.push(self.datagram(current, false));
        }

        out
    }

    fn datagram(&mut self, frames: Vec<Vec<u8>>, resent: bool) -> Vec<u8> {
        let sequence = self.send_sequence;
        self.send_sequence = (self.send_sequence + 1) & U24;
        let bytes = encode_datagram(sequence, &frames);

        // Надёжные кадры храним до подтверждения.
        if frames.iter().any(|frame| frame[0] >> 5 != UNRELIABLE) {
            self.ack_progress.get_or_insert_with(Instant::now);
            self.unacked.insert(sequence, Sent { at: Instant::now(), frames, resent });
        }

        bytes
    }

    /// Через сколько без подтверждения набор считается потерянным.
    fn resend_after(&self) -> Duration {
        ((self.rtt + (self.rtt_spread * 4).max(RESEND_SLACK)) * (1 << self.backoff.min(5))).clamp(RESEND_MIN, RESEND_MAX)
    }

    /// Клиент подтвердил набор: окно растёт, время отклика уточняется.
    fn acked(&mut self, sequence: u32) {
        let Some(sent) = self.unacked.remove(&sequence) else {
            return;
        };

        self.ack_progress = if self.unacked.is_empty() { None } else { Some(Instant::now()) };
        self.backoff = 0;

        // По повторам время отклика не мерим: неясно, на какую отправку ответ.
        if !sent.resent {
            let sample = sent.at.elapsed();
            let difference = sample.abs_diff(self.rtt);
            self.rtt_spread = (self.rtt_spread * 3 + difference) / 4;
            self.rtt = (self.rtt * 7 + sample) / 8;
        }

        self.window += if self.window < self.threshold { 1.0 } else { 1.0 / self.window };
        self.window = self.window.min(WINDOW_MAX);
    }

    /// Потеря наборов `numbers`: окно вдвое меньше — один раз на каждую
    /// перегрузку. Ответ — сжалось ли окно.
    fn lost(&mut self, numbers: &[u32]) -> bool {
        if !numbers.iter().any(|&number| ahead(number, self.recover) < U24 / 2) {
            return false;
        }

        self.window = (self.window / 2.0).max(WINDOW_MIN);
        self.threshold = self.window;
        self.recover = self.send_sequence;
        true
    }

    /// Снова отправить набор под новым номером.
    fn resend(&mut self, sequence: u32) -> Option<Vec<u8>> {
        let sent = self.unacked.remove(&sequence)?;
        Some(self.datagram(sent.frames, true))
    }
}

/// Отправленный набор, ждущий подтверждения.
struct Sent {
    at: Instant,
    frames: Vec<Vec<u8>>,
    resent: bool,
}

fn encode_datagram(sequence: u32, frames: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0x84];
    push_u24(&mut out, sequence);

    for frame in frames {
        out.extend_from_slice(frame);
    }

    out
}

fn push_u24(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes()[..3]);
}

/// Читатель байтов с проверкой границ.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, at: 0 }
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let slice = self.data.get(self.at..self.at + count)?;
        self.at += count;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn u16_be(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32_be(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i64_be(&mut self) -> Option<i64> {
        self.take(8).map(|b| i64::from_be_bytes(b.try_into().unwrap_or([0; 8])))
    }

    fn u24(&mut self) -> Option<u32> {
        self.take(3).map(|b| u32::from_le_bytes([b[0], b[1], b[2], 0]))
    }

    fn rest(&self) -> &'a [u8] {
        &self.data[self.at.min(self.data.len())..]
    }
}

/// Адрес в виде RakNet: семейство, байты (у IPv4 — инвертированные), порт.
fn push_address(out: &mut Vec<u8>, addr: SocketAddr) {
    match addr {
        SocketAddr::V4(v4) => {
            out.push(4);

            for byte in v4.ip().octets() {
                out.push(!byte);
            }

            out.extend_from_slice(&v4.port().to_be_bytes());
        }
        SocketAddr::V6(v6) => {
            out.push(6);
            out.extend_from_slice(&23u16.to_le_bytes());
            out.extend_from_slice(&v6.port().to_be_bytes());
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&v6.ip().octets());
            out.extend_from_slice(&0u32.to_be_bytes());
        }
    }
}

/// Пропускает адрес при чтении.
fn skip_address(reader: &mut Reader) -> Option<()> {
    match reader.u8()? {
        4 => reader.take(6).map(|_| ()),
        _ => reader.take(28).map(|_| ()),
    }
}

/// Запускает приём Bedrock на этом порту. Подключения отдаются в `events`,
/// строку для списка серверов даёт `status`.
pub async fn serve<F>(port: u16, status: F, events: mpsc::UnboundedSender<Event>) -> std::io::Result<()>
where
    F: Fn() -> Status + Send + Sync + 'static,
{
    let socket = Arc::new(UdpSocket::bind(("0.0.0.0", port)).await?);
    let guid: i64 = {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        (now.as_nanos() as i64).wrapping_mul(0x5851_F42D_4C95_7F2D) ^ std::process::id() as i64
    };
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Queued>();
    let mut sessions: HashMap<SocketAddr, Session> = HashMap::new();
    let mut buffer = vec![0u8; 2048];
    let mut flush = tokio::time::interval(FLUSH_EVERY);

    // Запись UDP только неблокирующая: переполненный буфер не должен
    // останавливать сроки всех сессий. Надёжные кадры остаются в unacked
    // и повторяются, ACK и рукопожатие повторит протокол.
    loop {
        tokio::select! {
            received = socket.recv_from(&mut buffer) => {
                let Ok((size, addr)) = received else { continue };
                let data = &buffer[..size];

                if let Err(reason) = handle_datagram(&socket, guid, &status, &events, &out_tx, &mut sessions, addr, data).await {
                    log_debug!("Bedrock: {} — {}", addr, reason);
                }
            }
            Some((addr, generation, outgoing)) = out_rx.recv() => {
                // Сообщение прежней сессии с этого адреса — не для новой.
                if let Some(session) = sessions.get_mut(&addr).filter(|session| session.generation == generation) {
                    match outgoing {
                        Outgoing::Game(body) => {
                            let mut message = Vec::with_capacity(body.len() + 1);
                            message.push(GAME);
                            message.extend_from_slice(&body);
                            session.queue(&message, RELIABLE_ORDERED);
                        }
                        Outgoing::Close => {
                            session.queue(&[DISCONNECT], RELIABLE_ORDERED);
                            // Прощание — сразу, мимо окна.
                            session.window = WINDOW_MAX;
                            for datagram in session.take_datagrams() {
                                let _ = socket.try_send_to(&datagram, addr);
                            }
                            sessions.remove(&addr);
                        }
                    }
                }
            }
            _ = flush.tick() => {
                let now = Instant::now();
                let mut gone = Vec::new();

                for (addr, session) in sessions.iter_mut() {
                    if session.expired(now) {
                        gone.push(*addr);
                        continue;
                    }

                    if !session.to_ack.is_empty() {
                        let ack = encode_ranges(ACK, &mut session.to_ack);
                        let _ = socket.try_send_to(&ack, *addr);
                    }

                    // Повтор наборов, которые долго не подтверждают: не больше
                    // окна за раз, и само окно сжимается.
                    let resend_after = session.resend_after();
                    let late: Vec<u32> = session
                        .unacked
                        .iter()
                        .filter(|(_, sent)| now.duration_since(sent.at) > resend_after)
                        .map(|(sequence, _)| *sequence)
                        .take(session.window as usize)
                        .collect();

                    if !late.is_empty() {
                        if session.lost(&late) {
                            session.backoff += 1;
                        }
                        log_debug!(
                            "Bedrock: {} — повтор {} наборов по сроку {:?}, окно {:.0}",
                            addr, late.len(), resend_after, session.window
                        );
                    }

                    for sequence in late {
                        if let Some(datagram) = session.resend(sequence) {
                            let _ = socket.try_send_to(&datagram, *addr);
                        }
                    }

                    for datagram in session.take_datagrams() {
                        let _ = socket.try_send_to(&datagram, *addr);
                    }
                }

                for addr in gone {
                    log_debug!("Bedrock: {} замолчал — соединение закрыто", addr);
                    sessions.remove(&addr);
                }
            }
        }
    }
}

/// ACK или NACK: номера наборов диапазонами.
fn encode_ranges(id: u8, numbers: &mut Vec<u32>) -> Vec<u8> {
    numbers.sort_unstable();
    numbers.dedup();

    let mut ranges: Vec<(u32, u32)> = Vec::new();

    for &number in numbers.iter() {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == number => *end = number,
            _ => ranges.push((number, number)),
        }
    }

    numbers.clear();

    let mut out = vec![id];
    out.extend_from_slice(&(ranges.len() as u16).to_be_bytes());

    for (start, end) in ranges {
        if start == end {
            out.push(1);
            push_u24(&mut out, start);
        } else {
            out.push(0);
            push_u24(&mut out, start);
            push_u24(&mut out, end);
        }
    }

    out
}

/// Диапазоны номеров из ACK или NACK — как есть, не разворачивая: клиент
/// может назвать диапазон в миллионы номеров. Диапазон через ноль (конец
/// меньше начала) делится на два.
fn decode_ranges(reader: &mut Reader) -> Option<Vec<(u32, u32)>> {
    let count = reader.u16_be()?;

    if count > MAX_RANGES {
        return None;
    }

    let mut out = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let single = reader.u8()? != 0;
        let start = reader.u24()?;
        let end = if single { start } else { reader.u24()? };

        if end < start {
            out.push((start, U24));
            out.push((0, end));
        } else {
            out.push((start, end));
        }
    }

    Some(out)
}

/// Какие из отправленных и ещё не подтверждённых наборов попадают в
/// диапазоны: перебираются только они, а не все номера диапазонов.
fn in_ranges(unacked: &BTreeMap<u32, Sent>, ranges: &[(u32, u32)]) -> Vec<u32> {
    ranges.iter().flat_map(|&(start, end)| unacked.range(start..=end).map(|(sequence, _)| *sequence)).collect()
}

#[allow(clippy::too_many_arguments)]
async fn handle_datagram<F>(
    socket: &UdpSocket,
    guid: i64,
    status: &F,
    events: &mpsc::UnboundedSender<Event>,
    out_tx: &mpsc::UnboundedSender<Queued>,
    sessions: &mut HashMap<SocketAddr, Session>,
    addr: SocketAddr,
    data: &[u8],
) -> Result<(), &'static str>
where
    F: Fn() -> Status,
{
    let Some(&id) = data.first() else {
        return Err("пустой пакет");
    };
    let mut reader = Reader::new(&data[1..]);

    // Подтверждения и пинги списка серверов идут потоком — их не пишем.
    if id & 0x80 == 0 && id != UNCONNECTED_PING && id != UNCONNECTED_PING_OPEN {
        log_debug!("Bedrock: {} → {:#04x} ({} байт)", addr, id, data.len());
    }

    match id {
        UNCONNECTED_PING | UNCONNECTED_PING_OPEN => {
            let time = reader.i64_be().ok_or("короткий пинг")?;
            let status = status();
            let text = format!(
                "MCPE;{};{};{};{};{};{};{};Survival;1;{};{};",
                status.motd.replace(';', ","),
                super::PROTOCOL,
                super::GAME_VERSION,
                status.online,
                status.max,
                guid as u64,
                status.world.replace(';', ","),
                status.port,
                status.port
            );
            let mut out = vec![UNCONNECTED_PONG];
            out.extend_from_slice(&time.to_be_bytes());
            out.extend_from_slice(&guid.to_be_bytes());
            out.extend_from_slice(&MAGIC);
            out.extend_from_slice(&(text.len() as u16).to_be_bytes());
            out.extend_from_slice(text.as_bytes());
            let _ = socket.try_send_to(&out, addr);
        }
        OPEN_REQUEST_1 => {
            reader.take(16).ok_or("нет magic")?;
            let version = reader.u8().ok_or("нет версии")?;

            // Настоящий клиент Bedrock говорит 11; сторонние клиенты бывают
            // и на 10 — различий в рукопожатии между ними нет.
            if !(10..=RAKNET_VERSION).contains(&version) {
                let mut out = vec![INCOMPATIBLE, RAKNET_VERSION];
                out.extend_from_slice(&MAGIC);
                out.extend_from_slice(&guid.to_be_bytes());
                let _ = socket.try_send_to(&out, addr);
                return Err("другая версия RakNet");
            }

            let mtu = (data.len() + UDP_OVERHEAD).min(1492) as u16;
            let mut out = vec![OPEN_REPLY_1];
            out.extend_from_slice(&MAGIC);
            out.extend_from_slice(&guid.to_be_bytes());
            out.push(0); // без защиты cookie
            out.extend_from_slice(&mtu.to_be_bytes());
            let _ = socket.try_send_to(&out, addr);
        }
        OPEN_REQUEST_2 => {
            reader.take(16).ok_or("нет magic")?;
            skip_address(&mut reader).ok_or("нет адреса")?;
            let mtu = reader.u16_be().ok_or("нет MTU")?.clamp(576, 1492);
            let client_guid = reader.i64_be().ok_or("нет guid")?;

            // С этого адреса уже идёт игра: чужой запрос её не заменит.
            // Тот же клиент (тот же guid) может начать заново — например,
            // после вылета, не дождавшись тайм-аута.
            if sessions.get(&addr).is_some_and(|session| session.connected && session.guid != client_guid) {
                return Err("запрос на подключение поверх чужой сессии");
            }

            sessions.insert(addr, Session::new(mtu as usize, client_guid));

            let mut out = vec![OPEN_REPLY_2];
            out.extend_from_slice(&MAGIC);
            out.extend_from_slice(&guid.to_be_bytes());
            push_address(&mut out, addr);
            out.extend_from_slice(&mtu.to_be_bytes());
            out.push(0); // шифрования RakNet нет
            let _ = socket.try_send_to(&out, addr);
        }
        ACK => {
            let Some(session) = sessions.get_mut(&addr) else { return Ok(()) };
            session.last_seen = Instant::now();

            let ranges = decode_ranges(&mut reader).ok_or("битый ACK")?;

            for number in in_ranges(&session.unacked, &ranges) {
                session.acked(number);
            }
        }
        NACK => {
            let Some(session) = sessions.get_mut(&addr) else { return Ok(()) };
            session.last_seen = Instant::now();

            let ranges = decode_ranges(&mut reader).ok_or("битый NACK")?;
            let numbers = in_ranges(&session.unacked, &ranges);
            log_debug!("Bedrock: {} просит повторить {:?}", addr, numbers);

            session.lost(&numbers);

            for number in numbers {
                if let Some(datagram) = session.resend(number) {
                    let _ = socket.try_send_to(&datagram, addr);
                }
            }
        }
        id if id & 0x80 != 0 && id & 0x60 == 0 => {
            let Some(session) = sessions.get_mut(&addr) else { return Ok(()) };
            session.last_seen = Instant::now();
            let sequence = reader.u24().ok_or("нет номера набора")?;
            session.to_ack.push(sequence);

            // Пропуск в номерах — просим повторить недостающие. Номера идут
            // по кругу: «впереди» — это меньше половины круга.
            let gap = ahead(sequence, session.expected_sequence);

            if gap > 0 && gap < 1024 {
                let mut missing: Vec<u32> = (0..gap).map(|step| (session.expected_sequence + step) & U24).collect();
                let nack = encode_ranges(NACK, &mut missing);
                let _ = socket.try_send_to(&nack, addr);
            }

            if gap < U24 / 2 {
                session.expected_sequence = (sequence + 1) & U24;
            }

            let mut messages = Vec::new();

            while !reader.rest().is_empty() {
                read_frame(session, &mut reader, &mut messages).ok_or("битый кадр")?;
            }

            for message in messages {
                handle_message(socket, guid, events, out_tx, session, addr, &message).await;
            }

            if sessions.get(&addr).is_some_and(|session| !session.connected && session.inbound.is_none()) {
                // ждём Connection Request
            }
        }
        _ => {}
    }

    Ok(())
}

/// Разбирает один кадр; готовые сообщения (собранные и по порядку) —
/// в `messages`.
fn read_frame(session: &mut Session, reader: &mut Reader, messages: &mut Vec<Vec<u8>>) -> Option<()> {
    let flags = reader.u8()?;
    let reliability = flags >> 5;
    let split = flags & 0x10 != 0;
    let length = (reader.u16_be()? as usize).div_ceil(8);

    let reliable = matches!(reliability, 2 | 3 | 4 | 6 | 7);
    let sequenced = matches!(reliability, 1 | 4);
    let ordered = matches!(reliability, 1 | 3 | 4 | 7);

    let reliable_index = if reliable { Some(reader.u24()?) } else { None };

    if sequenced {
        reader.u24()?;
    }

    let order_index = if ordered {
        let index = reader.u24()?;
        reader.u8()?; // канал
        Some(index)
    } else {
        None
    };

    let split_info = if split { Some((reader.u32_be()?, reader.u16_be()?, reader.u32_be()?)) } else { None };
    let body = reader.take(length)?.to_vec();

    // Повтор надёжного кадра — выбрасываем; номер вне окна — тоже: это
    // старый повтор или выдумка.
    if let Some(index) = reliable_index {
        if ahead(index, session.lowest_reliable) >= RECEIVE_WINDOW || !session.seen_reliable.insert(index) {
            return Some(());
        }

        // Держим окно небольшим.
        while session.seen_reliable.remove(&session.lowest_reliable) {
            session.lowest_reliable = (session.lowest_reliable + 1) & U24;
        }
    }

    // Сборка разбитого сообщения.
    let body = match split_info {
        Some((count, id, index)) => {
            if count == 0 || count > MAX_SPLIT_PARTS || index >= count {
                return Some(());
            }

            if !session.splits.contains_key(&id) && session.splits.len() >= MAX_SPLITS {
                return Some(());
            }

            if session.waiting_bytes + body.len() > MAX_WAITING_BYTES {
                return Some(());
            }

            let entry = session.splits.entry(id).or_insert_with(|| (count, HashMap::new()));

            // Другое число частей у того же сообщения или повтор части.
            if entry.0 != count || entry.1.contains_key(&index) {
                return Some(());
            }

            session.waiting_bytes += body.len();
            entry.1.insert(index, body);

            if entry.1.len() < entry.0 as usize {
                return Some(());
            }

            let (count, mut parts) = session.splits.remove(&id)?;
            let mut whole = Vec::new();

            for index in 0..count {
                whole.extend_from_slice(&parts.remove(&index)?);
            }

            session.waiting_bytes = session.waiting_bytes.saturating_sub(whole.len());
            whole
        }
        None => body,
    };

    match order_index {
        Some(index) if !sequenced => {
            let distance = ahead(index, session.order_expected);

            if distance == 0 {
                messages.push(body);
                session.order_expected = (session.order_expected + 1) & U24;

                while let Some(next) = session.order_waiting.remove(&session.order_expected) {
                    session.waiting_bytes = session.waiting_bytes.saturating_sub(next.len());
                    messages.push(next);
                    session.order_expected = (session.order_expected + 1) & U24;
                }
            } else if distance < RECEIVE_WINDOW
                && session.waiting_bytes + body.len() <= MAX_WAITING_BYTES
                && !session.order_waiting.contains_key(&index)
            {
                session.waiting_bytes += body.len();
                session.order_waiting.insert(index, body);
            }
        }
        _ => messages.push(body),
    }

    Some(())
}

/// Сообщение внутри соединения: служебное RakNet или игровое.
#[allow(clippy::too_many_arguments)]
async fn handle_message(
    socket: &UdpSocket,
    guid: i64,
    events: &mpsc::UnboundedSender<Event>,
    out_tx: &mpsc::UnboundedSender<Queued>,
    session: &mut Session,
    addr: SocketAddr,
    message: &[u8],
) {
    let Some(&id) = message.first() else {
        return;
    };
    let mut reader = Reader::new(&message[1..]);

    if id != GAME && id != CONNECTED_PING {
        log_debug!("Bedrock: {} сообщение {:#04x} ({} байт)", addr, id, message.len());
    }

    match id {
        CONNECTED_PING => {
            let Some(time) = reader.i64_be() else { return };
            let mut out = vec![CONNECTED_PONG];
            out.extend_from_slice(&time.to_be_bytes());
            out.extend_from_slice(&now_millis().to_be_bytes());
            session.queue(&out, UNRELIABLE);
        }
        CONNECTION_REQUEST => {
            let _client = reader.i64_be();
            let time = reader.i64_be().unwrap_or(0);
            let mut out = vec![CONNECTION_ACCEPTED];
            push_address(&mut out, addr);
            out.extend_from_slice(&0u16.to_be_bytes());

            for _ in 0..10 {
                push_address(&mut out, "255.255.255.255:19132".parse().expect("адрес"));
            }

            out.extend_from_slice(&time.to_be_bytes());
            out.extend_from_slice(&now_millis().to_be_bytes());
            session.queue(&out, RELIABLE_ORDERED);
            let _ = guid;
        }
        NEW_INCOMING => {
            if session.connected {
                return;
            }

            session.connected = true;
            let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_QUEUE);
            session.inbound = Some(inbound_tx);
            let sender = Sender { addr, generation: session.generation, queue: out_tx.clone() };
            let _ = events.send(Event::Connected { addr, inbound: inbound_rx, outbound: sender });
            log_debug!("Bedrock: {} подключился по RakNet (guid {})", addr, session.guid);
        }
        DISCONNECT => {
            session.inbound = None;
            session.last_seen = Instant::now() - TIMEOUT - Duration::from_secs(1);
        }
        GAME => {
            // Игровая часть не успевает разбирать — клиент заваливает сервер:
            // закрываем, как если бы он замолчал.
            if let Some(inbound) = &session.inbound
                && let Err(mpsc::error::TrySendError::Full(_)) = inbound.try_send(message[1..].to_vec())
            {
                log_debug!("Bedrock: {} шлёт быстрее, чем их можно разобрать — соединение закрыто", addr);
                session.inbound = None;
                session.last_seen = Instant::now() - TIMEOUT - Duration::from_secs(1);
            }
        }
        _ => {}
    }

    let _ = socket;
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_traffic_and_resends_do_not_hide_missing_ack_progress() {
        let mut session = Session::new(1400, 1);
        session.queue(&[GAME, 1], RELIABLE_ORDERED);
        session.take_datagrams();
        let stalled = Instant::now() - TIMEOUT;
        session.ack_progress = Some(stalled);
        session.last_seen = Instant::now();
        assert!(session.expired(Instant::now()));
        let sequence = *session.unacked.keys().next().unwrap();
        session.resend(sequence).unwrap();
        assert_eq!(session.ack_progress, Some(stalled));
        assert!(session.expired(Instant::now()));
        session.acked(sequence); // Старый номер ничего не подтверждает.
        assert!(session.expired(Instant::now()));
        let sequence = *session.unacked.keys().next().unwrap();
        session.acked(sequence);
        assert!(!session.expired(Instant::now()));
        assert_eq!(session.ack_progress, None);
    }

    #[tokio::test]
    async fn removing_timed_out_session_closes_game_channel() {
        let (sender, mut receiver) = mpsc::channel(1);
        let mut session = Session::new(1400, 1);
        session.inbound = Some(sender);
        session.last_seen = Instant::now() - TIMEOUT;
        assert!(session.expired(Instant::now()));
        drop(session);
        assert_eq!(receiver.recv().await, None);
    }

    /// Набор кадров, собранный сессией, разбирается обратно в те же
    /// сообщения — и маленькие, и разбитые на части.
    #[test]
    fn frames_round_trip() {
        let mut sender = Session::new(1400, 1);
        let mut receiver = Session::new(1400, 2);
        let small = vec![0xfe, 1, 2, 3];
        let big: Vec<u8> = (0..5000u32).map(|i| i as u8).collect();

        sender.queue(&small, RELIABLE_ORDERED);
        sender.queue(&big, RELIABLE_ORDERED);

        let mut got = Vec::new();

        for datagram in sender.take_datagrams() {
            assert!(datagram.len() <= 1400 - UDP_OVERHEAD, "набор больше MTU: {}", datagram.len());
            let mut reader = Reader::new(&datagram[4..]);

            while !reader.rest().is_empty() {
                read_frame(&mut receiver, &mut reader, &mut got).expect("кадр читается");
            }
        }

        assert_eq!(got, vec![small, big]);
    }

    /// Номера для ACK сворачиваются в диапазоны и читаются обратно.
    #[test]
    fn ack_ranges_round_trip() {
        let mut numbers = vec![5, 1, 2, 3, 9, 10];
        let bytes = encode_ranges(ACK, &mut numbers);
        let mut reader = Reader::new(&bytes[1..]);

        assert_eq!(decode_ranges(&mut reader), Some(vec![(1, 3), (5, 5), (9, 10)]));
    }

    /// Огромный диапазон в ACK не разворачивается: перебираются только
    /// отправленные наборы, что в него попали.
    #[test]
    fn a_huge_ack_range_touches_only_sent_datagrams() {
        let mut session = Session::new(1400, 1);
        session.queue(&[0xfe, 1], RELIABLE_ORDERED);
        session.take_datagrams();
        session.queue(&[0xfe, 2], RELIABLE_ORDERED);
        session.take_datagrams();
        assert_eq!(session.unacked.len(), 2);

        let bytes = [0, 1, 0, 0, 0, 0, 0xff, 0xff, 0xff];
        let ranges = decode_ranges(&mut Reader::new(&bytes)).expect("диапазон");
        assert_eq!(in_ranges(&session.unacked, &ranges), vec![0, 1]);

        // Диапазон через ноль делится на два.
        let bytes = [0, 1, 0, 0xfe, 0xff, 0xff, 0, 0, 0];
        assert_eq!(decode_ranges(&mut Reader::new(&bytes)), Some(vec![(0xff_fffe, U24), (0, 0)]));
    }

    /// Кадр одной надёжности: номер надёжности, номер очереди, тело.
    fn reliable_frame(reliable: u32, order: u32, split: Option<(u32, u16, u32)>, body: &[u8]) -> Vec<u8> {
        let mut out = vec![(RELIABLE_ORDERED << 5) | if split.is_some() { 0x10 } else { 0 }];
        out.extend_from_slice(&((body.len() * 8) as u16).to_be_bytes());
        push_u24(&mut out, reliable);
        push_u24(&mut out, order);
        out.push(0);

        if let Some((count, id, index)) = split {
            out.extend_from_slice(&count.to_be_bytes());
            out.extend_from_slice(&id.to_be_bytes());
            out.extend_from_slice(&index.to_be_bytes());
        }

        out.extend_from_slice(body);
        out
    }

    fn receive(session: &mut Session, frame: &[u8]) -> Vec<Vec<u8>> {
        let mut messages = Vec::new();
        read_frame(session, &mut Reader::new(frame), &mut messages).expect("кадр читается");
        messages
    }

    /// Части с номером вне сообщения, повторы частей и лишние одновременные
    /// сборки отбрасываются.
    #[test]
    fn broken_splits_are_dropped() {
        let mut session = Session::new(1400, 1);

        // Номер части не меньше их числа.
        receive(&mut session, &reliable_frame(0, 0, Some((2, 7, 2)), b"x"));
        assert!(session.splits.is_empty());

        // Повтор части не засчитывается за вторую.
        receive(&mut session, &reliable_frame(1, 0, Some((2, 7, 0)), b"a"));
        receive(&mut session, &reliable_frame(2, 0, Some((2, 7, 0)), b"a"));
        assert_eq!(session.splits[&7].1.len(), 1);
        assert_eq!(receive(&mut session, &reliable_frame(3, 0, Some((2, 7, 1)), b"b")), vec![b"ab".to_vec()]);
        assert_eq!(session.waiting_bytes, 0);

        // Не больше MAX_SPLITS сборок сразу.
        for id in 0..MAX_SPLITS as u16 + 4 {
            receive(&mut session, &reliable_frame(4 + id as u32, 1, Some((2, 100 + id, 0)), b"z"));
        }
        assert_eq!(session.splits.len(), MAX_SPLITS);
    }

    /// Номера надёжности и очереди идут по кругу 24 бит; далёкие вперёд
    /// номера не копятся.
    #[test]
    fn one_congestion_shrinks_the_window_once() {
        let mut session = Session::new(1400, 1);
        session.window = 64.0;
        session.send_sequence = 100;

        // Наборы 10 и 20 ушли до сжатия: их потеря — та же перегрузка.
        assert!(session.lost(&[10]));
        assert_eq!(session.window, 32.0);
        assert!(!session.lost(&[20]));
        assert_eq!(session.window, 32.0);

        // Потерян набор, отправленный уже после сжатия, — новая перегрузка.
        assert!(session.lost(&[100]));
        assert_eq!(session.window, 16.0);
    }

    #[test]
    fn frame_numbers_wrap_and_stay_in_window() {
        let mut session = Session::new(1400, 1);
        session.lowest_reliable = U24;
        session.order_expected = U24;

        assert_eq!(receive(&mut session, &reliable_frame(U24, U24, None, b"a")), vec![b"a".to_vec()]);
        assert_eq!((session.lowest_reliable, session.order_expected), (0, 0));
        assert_eq!(receive(&mut session, &reliable_frame(0, 0, None, b"b")), vec![b"b".to_vec()]);

        // Далеко впереди — не принимается и не хранится.
        receive(&mut session, &reliable_frame(RECEIVE_WINDOW + 5, 3, None, b"c"));
        receive(&mut session, &reliable_frame(2, RECEIVE_WINDOW + 5, None, b"d"));
        assert!(session.order_waiting.is_empty());
        assert!(!session.seen_reliable.contains(&(RECEIVE_WINDOW + 5)));
    }
}
