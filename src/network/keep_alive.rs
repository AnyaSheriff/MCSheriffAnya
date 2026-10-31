use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tokio::time::{Duration, Instant};

// minecraft.wiki/w/Java_Edition_protocol/FAQ: "every 1-15 seconds".
// Прежние пять секунд попадают в документированный интервал.
pub const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(5);
// minecraft.wiki/w/Java_Edition_protocol/Packets#Keep_Alive_(clientbound):
// "within 15 seconds after it was sent, the server kicks the client".
pub const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct KeepAlive {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<State>,
    changed: watch::Sender<()>,
}

struct State {
    created: Instant,
    last_sent: Instant,
    latency: u32,
    next_id: i64,
    generation: u64,
    phase: Phase,
}

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    Sending { generation: u64, id: i64, started: Instant, received: Option<Instant> },
    Awaiting { id: i64, sent: Instant, deadline: Instant },
    Closed,
    Expired,
}

/// Один незавершённый flush. Отмена future не оставляет вечный Sending.
pub struct SendTicket {
    alive: KeepAlive,
    generation: u64,
    id: i64,
    finished: bool,
}

impl SendTicket {
    pub fn id(&self) -> i64 { self.id }

    /// Только успешная передача рамки сокету начинает срок ответа.
    /// Ранний ответ хранится до подтверждения этой же отправки.
    pub fn sent(mut self) -> bool {
        let now = Instant::now();
        let mut state = self.alive.inner.state.lock().expect("Keep Alive");
        let created = state.created;
        let Phase::Sending { generation, id, started, received } = state.phase else { return false; };
        if generation != self.generation || id != self.id { return false; }
        state.last_sent = now;
        if let Some(received) = received {
            state.latency = elapsed_ms(started, received);
            state.phase = Phase::Idle;
        } else {
            state.phase = Phase::Awaiting { id, sent: now, deadline: now + KEEP_ALIVE_TIMEOUT };
        }
        self.finished = true;
        drop(state);
        self.alive.wake();
        if super::packet::timing_enabled() { eprintln!("NET_TIMING keep_alive sent id={id} at_us={} early_answer={}", now.saturating_duration_since(created).as_micros(), received.is_some()); }
        true
    }
}

impl Drop for SendTicket {
    fn drop(&mut self) {
        if self.finished { return; }
        let mut state = self.alive.inner.state.lock().expect("Keep Alive");
        if matches!(state.phase, Phase::Sending { generation, id, .. } if generation == self.generation && id == self.id) {
            state.phase = Phase::Closed;
            drop(state);
            self.alive.wake();
        }
    }
}

/// Нештатный выход читателя закрывает control; переданный FIFO read error
/// сохраняет прежний порядок и снимает guard.
pub(super) struct ReaderGuard { alive: KeepAlive, armed: bool }
impl ReaderGuard {
    /// Успешно переданный read error остаётся за предшествующим вводом игры.
    pub(super) fn disarm(&mut self) { self.armed = false; }
}
impl Drop for ReaderGuard {
    fn drop(&mut self) { if self.armed { self.alive.close(); } }
}

fn elapsed_ms(sent: Instant, received: Instant) -> u32 {
    received.saturating_duration_since(sent).as_millis().min(u128::from(u32::MAX)) as u32
}

impl KeepAlive {
    pub fn new() -> Self {
        let created = Instant::now();
        Self { inner: Arc::new(Inner {
            state: Mutex::new(State { created, latency: 0, last_sent: created, next_id: 1, generation: 0, phase: Phase::Idle }),
            changed: watch::channel(()).0,
        }) }
    }

    fn wake(&self) { self.inner.changed.send_replace(()); }
    pub fn subscribe(&self) -> watch::Receiver<()> { self.inner.changed.subscribe() }
    pub(super) fn reader_guard(&self) -> ReaderGuard { ReaderGuard { alive: self.clone(), armed: true } }
    pub fn latency(&self) -> u32 { self.inner.state.lock().expect("Keep Alive").latency }
    pub fn closed(&self) -> bool { matches!(self.inner.state.lock().expect("Keep Alive").phase, Phase::Closed) }

    pub fn close(&self) {
        let mut state = self.inner.state.lock().expect("Keep Alive");
        if matches!(state.phase, Phase::Closed | Phase::Expired) { return; }
        state.phase = Phase::Closed;
        drop(state);
        self.wake();
    }

    pub fn deadline(&self) -> Option<Instant> {
        match self.inner.state.lock().expect("Keep Alive").phase {
            Phase::Awaiting { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    /// Финальная проверка и приём ответа сериализованы одним коротким замком.
    pub fn expired(&self) -> bool {
        let mut state = self.inner.state.lock().expect("Keep Alive");
        let now = Instant::now();
        let at = now.saturating_duration_since(state.created).as_micros();
        let expired = matches!(state.phase, Phase::Awaiting { deadline, .. } if now >= deadline);
        if expired { state.phase = Phase::Expired; }
        let timed_out = matches!(state.phase, Phase::Expired);
        drop(state);
        if expired {
            self.wake();
            if super::packet::timing_enabled() { eprintln!("NET_TIMING keep_alive expired at_us={at}"); }
        }
        timed_out
    }

    pub fn next_wake(&self) -> Instant {
        let state = self.inner.state.lock().expect("Keep Alive");
        match state.phase {
            Phase::Awaiting { deadline, .. } => deadline,
            Phase::Sending { started, .. } => started + super::packet::WRITE_TIMEOUT,
            Phase::Idle => state.last_sent + KEEP_ALIVE_INTERVAL,
            Phase::Closed | Phase::Expired => Instant::now(),
        }
    }

    pub fn due(&self) -> bool {
        let state = self.inner.state.lock().expect("Keep Alive");
        matches!(state.phase, Phase::Idle) && state.last_sent.elapsed() >= KEEP_ALIVE_INTERVAL
    }

    pub fn begin_send(&self) -> Option<SendTicket> {
        let mut state = self.inner.state.lock().expect("Keep Alive");
        if !matches!(state.phase, Phase::Idle) || state.last_sent.elapsed() < KEEP_ALIVE_INTERVAL { return None; }
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        let started = Instant::now();
        let at = started.saturating_duration_since(state.created).as_micros();
        state.phase = Phase::Sending { generation, id, started, received: None };
        drop(state);
        self.wake();
        if super::packet::timing_enabled() { eprintln!("NET_TIMING keep_alive sending id={id} at_us={at}"); }
        Some(SendTicket { alive: self.clone(), generation, id, finished: false })
    }

    /// Вызывается читателем сразу после полного декодирования, до FIFO игры.
    /// Время сервера снимается под замком публикации, не при позднем разборе.
    pub fn answer(&self, payload: &[u8]) -> bool {
        let Ok(bytes) = <[u8; 8]>::try_from(payload) else { return false; };
        let answer = i64::from_be_bytes(bytes);
        let mut state = self.inner.state.lock().expect("Keep Alive");
        let received = Instant::now();
        let at = received.saturating_duration_since(state.created).as_micros();
        let latency = match &mut state.phase {
            Phase::Awaiting { id, sent, deadline } if *id == answer && received < *deadline => Some(elapsed_ms(*sent, received)),
            Phase::Sending { id, started, received: early, .. }
                if *id == answer && early.is_none() && received < *started + super::packet::WRITE_TIMEOUT => {
                    *early = Some(received);
                    None
                }
            _ => return false,
        };
        if let Some(latency) = latency {
            state.latency = latency;
            state.phase = Phase::Idle;
        }
        drop(state);
        self.wake();
        if super::packet::timing_enabled() { eprintln!("NET_TIMING keep_alive received id={answer} at_us={at} rtt_ms={latency:?}"); }
        true
    }

    // Старые codec/timeout fixtures моделируют уже завершённую отправку.
    #[cfg(test)]
    pub(super) fn make_due_for_test(&self) {
        self.inner.state.lock().expect("Keep Alive").last_sent = Instant::now() - KEEP_ALIVE_INTERVAL;
    }

    #[cfg(test)]
    pub fn send_due(&self) -> Option<i64> {
        let ticket = self.begin_send()?;
        let id = ticket.id();
        ticket.sent().then_some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn only_matching_reply_clears_the_fifteen_second_deadline() {
        let alive = KeepAlive::new();
        assert_eq!(alive.send_due(), None);
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let id = alive.send_due().unwrap();
        let deadline = alive.deadline();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(alive.send_due(), None);
        assert!(!alive.answer(&(id + 1).to_be_bytes()));
        assert!(!alive.answer(&[]));
        assert_eq!(alive.deadline(), deadline);
        assert!(!alive.expired());
        assert!(alive.answer(&id.to_be_bytes()));
        assert_eq!(alive.latency(), 10_000);
        assert_eq!(alive.deadline(), None);
        assert!(!alive.answer(&id.to_be_bytes()));
        let next = alive.send_due().unwrap();
        tokio::time::advance(KEEP_ALIVE_TIMEOUT).await;
        assert!(alive.expired());
        assert!(!alive.answer(&next.to_be_bytes()));
    }

    #[tokio::test(start_paused = true)]
    async fn timely_reader_receipt_survives_late_gameplay_processing_and_duplicates() {
        let alive = KeepAlive::new();
        let reader = alive.clone();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let id = alive.send_due().unwrap();
        tokio::time::advance(KEEP_ALIVE_TIMEOUT - Duration::from_millis(1)).await;
        assert!(reader.answer(&id.to_be_bytes()));
        tokio::time::advance(Duration::from_secs(20)).await;
        assert!(!alive.expired());
        assert_eq!(alive.latency(), 14_999);
        assert!(!reader.answer(&id.to_be_bytes()));
    }

    #[tokio::test(start_paused = true)]
    async fn deadline_equal_and_late_receipts_never_revive_expired_control() {
        for late in [Duration::ZERO, Duration::from_millis(1)] {
            let alive = KeepAlive::new();
            tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
            let id = alive.send_due().unwrap();
            tokio::time::advance(KEEP_ALIVE_TIMEOUT + late).await;
            assert!(!alive.answer(&id.to_be_bytes()));
            assert!(alive.expired());
            assert!(!alive.answer(&id.to_be_bytes()));
            assert!(alive.begin_send().is_none());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn sending_has_no_response_deadline_and_retains_one_early_receipt() {
        let alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let ticket = alive.begin_send().unwrap();
        let id = ticket.id();
        assert!(alive.begin_send().is_none());
        assert_eq!(alive.deadline(), None);
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(!alive.expired());
        assert!(alive.answer(&id.to_be_bytes()));
        assert!(!alive.answer(&id.to_be_bytes()));
        assert_eq!(alive.latency(), 0);
        assert!(ticket.sent());
        assert_eq!(alive.latency(), 10_000);
        assert_eq!(alive.deadline(), None);
        assert_eq!(alive.next_wake(), Instant::now() + KEEP_ALIVE_INTERVAL);
    }

    #[tokio::test(start_paused = true)]
    async fn response_deadline_starts_on_send_completion_and_cancel_closes_ticket() {
        let alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let ticket = alive.begin_send().unwrap();
        tokio::time::advance(Duration::from_secs(10)).await;
        assert!(ticket.sent());
        assert_eq!(alive.deadline(), Some(Instant::now() + KEEP_ALIVE_TIMEOUT));
        let cancelled = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let ticket = cancelled.begin_send().unwrap();
        let id = ticket.id();
        assert!(cancelled.answer(&id.to_be_bytes()));
        drop(ticket);
        assert!(cancelled.closed());
        assert_eq!(cancelled.latency(), 0);
        assert!(!cancelled.answer(&id.to_be_bytes()));
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_is_not_expiration_and_close_does_not_overwrite_timeout() {
        let closed = KeepAlive::new();
        closed.close();
        assert!(closed.closed());
        assert!(!closed.expired());
        assert!(!closed.answer(&1i64.to_be_bytes()));
        let expired = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let id = expired.send_due().unwrap();
        tokio::time::advance(KEEP_ALIVE_TIMEOUT).await;
        assert!(expired.expired());
        expired.close();
        assert!(expired.expired());
        assert!(!expired.closed());
        assert!(!expired.answer(&id.to_be_bytes()));
    }

    #[tokio::test(start_paused = true)]
    async fn wrong_reply_flood_cannot_replace_receipt_and_reader_drop_closes_control() {
        let alive = KeepAlive::new();
        tokio::time::advance(KEEP_ALIVE_INTERVAL).await;
        let id = alive.send_due().unwrap();
        for _ in 0..10_000 { assert!(!alive.answer(&(id + 1).to_be_bytes())); }
        assert!(alive.answer(&id.to_be_bytes()));
        assert!(!alive.expired());
        let guard = alive.reader_guard();
        drop(guard);
        assert!(alive.closed());
        assert!(!alive.answer(&id.to_be_bytes()));
    }
}
