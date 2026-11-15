// Эксперимент: дополнительные исполнители только после превышения
// 50 мс и заметной независимой работе мобов внутри одного измерения.
use std::time::Duration;

pub(super) const MAX_THREADS: usize = 8;
const LIMIT: Duration = Duration::from_millis(50);
const MIN_WORK: Duration = Duration::from_millis(1);
const GROW_AFTER: u16 = 20;
const SHRINK_AFTER: u16 = 200;

pub(super) struct Policy {
    cap: usize,
    pub(super) width: usize,
    pressure: u16,
    quiet: u16,
}

impl Policy {
    pub(super) fn new(available: usize, configured: usize) -> Self {
        let requested = if configured == 0 { MAX_THREADS } else { configured.clamp(1, MAX_THREADS) };
        Self { cap: available.max(1).min(requested), width: 1, pressure: 0, quiet: 0 }
    }

    pub(super) fn reset(&mut self) {
        self.width = 1;
        self.pressure = 0;
        self.quiet = 0;
    }

    /// Длительность — вся работа такта, без ожидания следующего. Один сетевой
    /// таймаут не включает помощников: нужны замеренные расчёты мобов.
    pub(super) fn observe(&mut self, tick: Duration, work: Duration, items: usize, allowed: bool) {
        if !allowed {
            self.reset();
            return;
        }
        let useful = items >= 64 && work >= MIN_WORK;
        if tick > LIMIT && useful && self.width < self.cap {
            self.quiet = 0;
            self.pressure += 1;
            let required = if self.width == 1 { 1 } else { GROW_AFTER };
            if self.pressure >= required {
                self.width += 1;
                self.pressure = 0;
            }
        } else {
            self.pressure = 0;
            if !useful || tick < Duration::from_millis(40) {
                self.quiet += 1;
                if self.quiet >= SHRINK_AFTER {
                    self.width = self.width.saturating_sub(1).max(1);
                    self.quiet = 0;
                }
            } else {
                self.quiet = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BUSY: Duration = Duration::from_millis(51);
    const WORK: Duration = Duration::from_millis(2);

    #[test]
    fn strict_fifty_ms_threshold_then_gradual_growth() {
        let mut policy = Policy::new(8, 0);
        for _ in 0..200 { policy.observe(LIMIT, WORK, 100, true); }
        assert_eq!(policy.width, 1);
        policy.observe(LIMIT + Duration::from_nanos(1), WORK, 100, true);
        assert_eq!(policy.width, 2);
        policy.observe(LIMIT, WORK, 100, true);
        for _ in 0..GROW_AFTER - 1 { policy.observe(BUSY, WORK, 100, true); }
        assert_eq!(policy.width, 2);
    }

    #[test]
    fn unrelated_lag_and_small_batches_do_not_activate_workers() {
        for (cost, items) in [(Duration::ZERO, 1000), (WORK, 63)] {
            let mut policy = Policy::new(8, 0);
            for _ in 0..1000 { policy.observe(BUSY, cost, items, true); }
            assert_eq!(policy.width, 1);
        }
    }

    #[test]
    fn respects_available_cpus_and_manual_cap() {
        for (available, configured, expected) in [(0, 0, 1), (1, 8, 1), (2, 0, 2), (20, 0, 8), (8, 3, 3), (8, 1, 1)] {
            let mut policy = Policy::new(available, configured);
            for _ in 0..1000 { policy.observe(BUSY, WORK, 1000, true); }
            assert_eq!(policy.width, expected);
        }
    }

    #[test]
    fn quiet_period_shrinks_gradually_and_forbidden_resets_immediately() {
        let mut policy = Policy::new(3, 0);
        for _ in 0..=GROW_AFTER { policy.observe(BUSY, WORK, 1000, true); }
        assert_eq!(policy.width, 3);
        for _ in 0..SHRINK_AFTER - 1 { policy.observe(Duration::from_millis(30), WORK, 1000, true); }
        assert_eq!(policy.width, 3);
        policy.observe(Duration::from_millis(30), WORK, 1000, true);
        assert_eq!(policy.width, 2);
        policy.observe(BUSY, WORK, 1000, false);
        assert_eq!(policy.width, 1);
        policy.observe(LIMIT, WORK, 1000, true);
        assert_eq!(policy.width, 1);
        policy.observe(BUSY, WORK, 1000, true);
        assert_eq!(policy.width, 2);
    }
}
