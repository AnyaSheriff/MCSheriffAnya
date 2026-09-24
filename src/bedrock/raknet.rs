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

/// Окно перегрузки: сколько наборов может быть в пути без подтверждения.
/// Растёт с каждым подтверждением и вдвое сжимается при потере — иначе
/// медленный клиент тонет в повторах, и всё приходит с опозданием в секунды.
const WINDOW_START: f64 = 32.0;
const WINDOW_MIN: f64 = 8.0;
const WINDOW_MAX: f64 = 4096.0;

/// Как часто сессии отправляют накопленное и подтверждения.
const FLUSH_EVERY: Duration = Duration::from_millis(10);

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
    Connected { addr: SocketAddr, inbound: mpsc::UnboundedReceiver<Vec<u8>>, outbound: Sender },
}

/// Куда игровая часть отдаёт сообщения для клиента (тело — без 0xFE).
#[derive(Clone)]
pub struct Sender {
    addr: SocketAddr,
    queue: mpsc::UnboundedSender<(SocketAddr, Outgoing)>,
}

enum Outgoing {
    Game(Vec<u8>),
    Close,
}

impl Sender {
    /// Отправить игровое сообщение: надёжно и по порядку.
    pub fn send(&self, body: Vec<u8>) -> bool {
        self.queue.send((self.addr, Outgoing::Game(body))).is_ok()
    }

    /// Закрыть соединение.
    pub fn close(&self) {
        let _ = self.queue.send((self.addr, Outgoing::Close));
    }
}

/// Состояние одного клиента.
struct Session {
    mtu: usize,
    guid: i64,
    connected: bool,
    last_seen: Instant,

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
    /// Сглаженное время отклика и его разброс.
    rtt: Duration,
    rtt_spread: Duration,

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

    /// Куда отдаются игровые сообщения.
    inbound: Option<mpsc::UnboundedSender<Vec<u8>>>,
}

impl Session {
    fn new(mtu: usize, guid: i64) -> Session {
        Session {
            mtu,
            guid,
            connected: false,
            last_seen: Instant::now(),
            send_sequence: 0,
            reliable_index: 0,
            order_index: 0,
            split_id: 0,
            pending: VecDeque::new(),
            unacked: BTreeMap::new(),
            window: WINDOW_START,
            threshold: WINDOW_MAX,
            rtt: Duration::from_millis(100),
            rtt_spread: Duration::from_millis(50),
            to_ack: Vec::new(),
            expected_sequence: 0,
            seen_reliable: HashSet::new(),
            lowest_reliable: 0,
            order_expected: 0,
            order_waiting: BTreeMap::new(),
            splits: HashMap::new(),
            inbound: None,
        }
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

        self.order_index = self.order_index.wrapping_add(1);
    }

    fn frame(&mut self, body: &[u8], reliability: u8, split: Option<(u32, u16, u32)>) -> Vec<u8> {
        let order = self.order_index;

        if reliability == RELIABLE_ORDERED {
            self.order_index = self.order_index.wrapping_add(1);
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
            self.reliable_index = self.reliable_index.wrapping_add(1);
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
        self.send_sequence = (self.send_sequence + 1) & 0xFF_FFFF;
        let bytes = encode_datagram(sequence, &frames);

        // Надёжные кадры храним до подтверждения.
        if frames.iter().any(|frame| frame[0] >> 5 != UNRELIABLE) {
            self.unacked.insert(sequence, Sent { at: Instant::now(), frames, resent });
        }

        bytes
    }

    /// Через сколько без подтверждения набор считается потерянным.
    fn resend_after(&self) -> Duration {
        (self.rtt + self.rtt_spread * 4).clamp(RESEND_MIN, RESEND_MAX)
    }

    /// Клиент подтвердил набор: окно растёт, время отклика уточняется.
    fn acked(&mut self, sequence: u32) {
        let Some(sent) = self.unacked.remove(&sequence) else {
            return;
        };

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

    /// Потеря: окно вдвое меньше.
    fn lost(&mut self) {
        self.window = (self.window / 2.0).max(WINDOW_MIN);
        self.threshold = self.window;
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
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<(SocketAddr, Outgoing)>();
    let mut sessions: HashMap<SocketAddr, Session> = HashMap::new();
    let mut buffer = vec![0u8; 2048];
    let mut flush = tokio::time::interval(FLUSH_EVERY);

    loop {
        tokio::select! {
            received = socket.recv_from(&mut buffer) => {
                let Ok((size, addr)) = received else { continue };
                let data = &buffer[..size];

                if let Err(reason) = handle_datagram(&socket, guid, &status, &events, &out_tx, &mut sessions, addr, data).await {
                    log_debug!("Bedrock: {} — {}", addr, reason);
                }
            }
            Some((addr, outgoing)) = out_rx.recv() => {
                if let Some(session) = sessions.get_mut(&addr) {
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
                                let _ = socket.send_to(&datagram, addr).await;
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
                    if now.duration_since(session.last_seen) > TIMEOUT {
                        gone.push(*addr);
                        continue;
                    }

                    if !session.to_ack.is_empty() {
                        let ack = encode_ranges(ACK, &mut session.to_ack);
                        let _ = socket.send_to(&ack, *addr).await;
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
                        session.lost();
                        log_debug!(
                            "Bedrock: {} — повтор {} наборов по сроку {:?}, окно {:.0}",
                            addr, late.len(), resend_after, session.window
                        );
                    }

                    for sequence in late {
                        if let Some(datagram) = session.resend(sequence) {
                            let _ = socket.send_to(&datagram, *addr).await;
                        }
                    }

                    for datagram in session.take_datagrams() {
                        let _ = socket.send_to(&datagram, *addr).await;
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

fn decode_ranges(reader: &mut Reader) -> Option<Vec<u32>> {
    let count = reader.u16_be()?;
    let mut out = Vec::new();

    for _ in 0..count {
        let single = reader.u8()? != 0;
        let start = reader.u24()?;
        let end = if single { start } else { reader.u24()? };

        // Защита от огромных диапазонов.
        for number in start..=end.min(start + 4096) {
            out.push(number);
        }
    }

    Some(out)
}

#[allow(clippy::too_many_arguments)]
async fn handle_datagram<F>(
    socket: &UdpSocket,
    guid: i64,
    status: &F,
    events: &mpsc::UnboundedSender<Event>,
    out_tx: &mpsc::UnboundedSender<(SocketAddr, Outgoing)>,
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
            let _ = socket.send_to(&out, addr).await;
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
                let _ = socket.send_to(&out, addr).await;
                return Err("другая версия RakNet");
            }

            let mtu = (data.len() + UDP_OVERHEAD).min(1492) as u16;
            let mut out = vec![OPEN_REPLY_1];
            out.extend_from_slice(&MAGIC);
            out.extend_from_slice(&guid.to_be_bytes());
            out.push(0); // без защиты cookie
            out.extend_from_slice(&mtu.to_be_bytes());
            let _ = socket.send_to(&out, addr).await;
        }
        OPEN_REQUEST_2 => {
            reader.take(16).ok_or("нет magic")?;
            skip_address(&mut reader).ok_or("нет адреса")?;
            let mtu = reader.u16_be().ok_or("нет MTU")?.clamp(576, 1492);
            let client_guid = reader.i64_be().ok_or("нет guid")?;

            sessions.insert(addr, Session::new(mtu as usize, client_guid));

            let mut out = vec![OPEN_REPLY_2];
            out.extend_from_slice(&MAGIC);
            out.extend_from_slice(&guid.to_be_bytes());
            push_address(&mut out, addr);
            out.extend_from_slice(&mtu.to_be_bytes());
            out.push(0); // шифрования RakNet нет
            let _ = socket.send_to(&out, addr).await;
        }
        ACK => {
            let Some(session) = sessions.get_mut(&addr) else { return Ok(()) };
            session.last_seen = Instant::now();

            for number in decode_ranges(&mut reader).ok_or("битый ACK")? {
                session.acked(number);
            }
        }
        NACK => {
            let Some(session) = sessions.get_mut(&addr) else { return Ok(()) };
            session.last_seen = Instant::now();

            let numbers = decode_ranges(&mut reader).ok_or("битый NACK")?;
            log_debug!("Bedrock: {} просит повторить {:?}", addr, numbers);

            if !numbers.is_empty() {
                session.lost();
            }

            for number in numbers {
                if let Some(datagram) = session.resend(number) {
                    let _ = socket.send_to(&datagram, addr).await;
                }
            }
        }
        id if id & 0x80 != 0 && id & 0x60 == 0 => {
            let Some(session) = sessions.get_mut(&addr) else { return Ok(()) };
            session.last_seen = Instant::now();
            let sequence = reader.u24().ok_or("нет номера набора")?;
            session.to_ack.push(sequence);

            // Пропуск в номерах — просим повторить недостающие.
            if sequence > session.expected_sequence && sequence - session.expected_sequence < 1024 {
                let mut missing: Vec<u32> = (session.expected_sequence..sequence).collect();
                let nack = encode_ranges(NACK, &mut missing);
                let _ = socket.send_to(&nack, addr).await;
            }

            if sequence >= session.expected_sequence {
                session.expected_sequence = sequence + 1;
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

    // Повтор надёжного кадра — выбрасываем.
    if let Some(index) = reliable_index {
        if index < session.lowest_reliable || !session.seen_reliable.insert(index) {
            return Some(());
        }

        // Держим окно небольшим.
        while session.seen_reliable.contains(&session.lowest_reliable) {
            session.seen_reliable.remove(&session.lowest_reliable);
            session.lowest_reliable += 1;
        }
    }

    // Сборка разбитого сообщения.
    let body = match split_info {
        Some((count, id, index)) => {
            if count == 0 || count > 8192 {
                return Some(());
            }

            let entry = session.splits.entry(id).or_insert_with(|| (count, HashMap::new()));
            entry.1.insert(index, body);

            if entry.1.len() < entry.0 as usize {
                return Some(());
            }

            let (count, mut parts) = session.splits.remove(&id)?;
            let mut whole = Vec::new();

            for index in 0..count {
                whole.extend_from_slice(&parts.remove(&index)?);
            }

            whole
        }
        None => body,
    };

    match order_index {
        Some(index) if !sequenced => {
            if index == session.order_expected {
                messages.push(body);
                session.order_expected += 1;

                while let Some(next) = session.order_waiting.remove(&session.order_expected) {
                    messages.push(next);
                    session.order_expected += 1;
                }
            } else if index > session.order_expected {
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
    out_tx: &mpsc::UnboundedSender<(SocketAddr, Outgoing)>,
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
            let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
            session.inbound = Some(inbound_tx);
            let sender = Sender { addr, queue: out_tx.clone() };
            let _ = events.send(Event::Connected { addr, inbound: inbound_rx, outbound: sender });
            log_debug!("Bedrock: {} подключился по RakNet (guid {})", addr, session.guid);
        }
        DISCONNECT => {
            session.inbound = None;
            session.last_seen = Instant::now() - TIMEOUT - Duration::from_secs(1);
        }
        GAME => {
            if let Some(inbound) = &session.inbound {
                let _ = inbound.send(message[1..].to_vec());
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

        assert_eq!(decode_ranges(&mut reader), Some(vec![1, 2, 3, 5, 9, 10]));
    }
}
