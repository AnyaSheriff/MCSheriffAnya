use tokio::time::{Duration, Instant};

// minecraft.wiki/w/Java_Edition_protocol/FAQ: "every 1-15 seconds".
// Прежние пять секунд попадают в документированный интервал.
pub const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(5);
// minecraft.wiki/w/Java_Edition_protocol/Packets#Keep_Alive_(clientbound):
// "within 15 seconds after it was sent, the server kicks the client".
pub const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(15);

pub struct KeepAlive {
    last_sent: Instant,
    next_id: i64,
    pending: Option<(i64, Instant)>,
}

impl KeepAlive {
    pub fn new() -> Self {
        Self { last_sent: Instant::now(), next_id: 1, pending: None }
    }

    pub fn deadline(&self) -> Option<Instant> { self.pending.map(|(_, deadline)| deadline) }

    pub fn expired(&self) -> bool { self.deadline().is_some_and(|deadline| Instant::now() >= deadline) }

    pub fn next_wake(&self) -> Instant {
        self.deadline().unwrap_or(self.last_sent + KEEP_ALIVE_INTERVAL)
    }

    pub fn send_due(&mut self) -> Option<i64> {
        if self.pending.is_some() || self.last_sent.elapsed() < KEEP_ALIVE_INTERVAL { return None; }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.last_sent = Instant::now();
        self.pending = Some((id, self.last_sent + KEEP_ALIVE_TIMEOUT));
        Some(id)
    }

    pub fn answer(&mut self, payload: &[u8]) -> bool {
        let Ok(bytes) = <[u8; 8]>::try_from(payload) else { return false; };
        if self.pending.is_some_and(|(id, deadline)| id == i64::from_be_bytes(bytes) && Instant::now() < deadline) {
            self.pending = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn only_matching_reply_clears_the_fifteen_second_deadline() {
        let mut alive = KeepAlive::new();
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
        assert_eq!(alive.deadline(), None);
        assert!(!alive.answer(&id.to_be_bytes()));
        let next = alive.send_due().unwrap();
        tokio::time::advance(KEEP_ALIVE_TIMEOUT).await;
        assert!(alive.expired());
        assert!(!alive.answer(&next.to_be_bytes()));
    }
}

