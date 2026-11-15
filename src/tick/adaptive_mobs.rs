// Общий бюджет независимых фаз: блочные миры и наблюдение мобов не перекрываются.
use std::time::Duration;

pub(super) struct Policy {
    cap: usize,
    pub(super) width: usize,
    pressure: u16,
    quiet: u16,
    #[cfg(test)]
    pub(super) block_phases: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
    #[cfg(test)]
    pub(super) mob_jobs: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}
impl Policy {
    pub(super) fn new(available: usize) -> Self {
        Self { cap: available.clamp(1, 8), width: 1, pressure: 0, quiet: 0, #[cfg(test)] block_phases: None, #[cfg(test)] mob_jobs: None }
    }
    pub(super) fn reset(&mut self) { self.width = 1; self.pressure = 0; self.quiet = 0; }
    pub(super) fn phase_width(&self, configured: usize, maximum: usize, useful: usize, allowed: bool) -> usize {
        if !allowed { return 1; }
        self.width.min(self.cap_for_phase(configured, maximum, useful))
    }
    pub(super) fn cap_for_phase(&self, configured: usize, maximum: usize, useful: usize) -> usize {
        let cap = if configured == 0 { maximum } else { configured.clamp(1, maximum) };
        self.cap.min(cap).min(useful.max(1))
    }
    // Результат влияет только на следующий такт; ожидание сетки сюда не входит.
    pub(super) fn observe(&mut self, tick: Duration, useful_cap: usize, allowed: bool) {
        if !allowed { self.reset(); return; }
        let useful_cap = useful_cap.min(self.cap);
        if tick > Duration::from_millis(50) && useful_cap > self.width {
            self.quiet = 0;
            self.pressure += 1;
            if self.pressure >= if self.width == 1 { 1 } else { 20 } {
                self.width += 1; self.pressure = 0;
            }
        } else {
            self.pressure = 0;
            if useful_cap < self.width || useful_cap <= 1 || tick < Duration::from_millis(40) {
                self.quiet += 1;
                if self.quiet >= 200 { self.width = self.width.saturating_sub(1).max(1); self.quiet = 0; }
            } else { self.quiet = 0; }
        }
    }
}

pub(super) fn block_useful(costs: [Duration; 3], enabled: [bool; 3]) -> usize {
    let costs: Vec<_> = costs.into_iter().zip(enabled).filter_map(|(cost, enabled)| enabled.then_some(cost)).collect();
    let busy = costs.iter().filter(|&&cost| cost >= Duration::from_micros(250)).count();
    if busy >= 2 && costs.into_iter().sum::<Duration>() >= Duration::from_millis(1) { busy } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_threshold_next_tick_and_consecutive_growth() {
        let mut p = Policy::new(8);
        for _ in 0..30 { p.observe(Duration::from_millis(50), 8, true); }
        assert_eq!(p.phase_width(0, 8, 64, true), 1);
        p.observe(Duration::from_millis(50)+Duration::from_nanos(1), 8, true);
        assert_eq!(p.width, 2);
        for _ in 0..19 { p.observe(Duration::from_millis(51), 8, true); }
        assert_eq!(p.width, 2);
        p.observe(Duration::from_millis(50), 8, true);
        for _ in 0..20 { p.observe(Duration::from_millis(51), 8, true); }
        assert_eq!(p.width, 3);
        for _ in 0..199 { p.observe(Duration::from_millis(30), 8, true); }
        assert_eq!(p.width, 3);
        p.observe(Duration::from_millis(30), 8, true); assert_eq!(p.width, 2);
        p.observe(Duration::from_millis(100), 8, false); assert_eq!(p.width, 1);
    }
    #[test]
    fn shared_budget_respects_phase_caps_flags_failure_and_single_world() {
        let mut p = Policy::new(4);
        for _ in 0..100 { p.observe(Duration::from_millis(51), 8, true); }
        assert_eq!(p.width, 4);
        assert_eq!(p.phase_width(0, 3, 3, true), 3);
        assert_eq!(p.phase_width(2, 8, 64, true), 2);
        assert_eq!(p.cap_for_phase(1, 3, 3), 1);
        assert_eq!(p.cap_for_phase(1, 8, 64), 1);
        assert_eq!(p.cap_for_phase(0, 3, 3).max(p.cap_for_phase(0, 8, 64)), 4);
        assert_eq!(p.phase_width(0, 3, 1, true), 1);
        assert_eq!(p.phase_width(0, 8, 64, false), 1);
        assert_eq!(p.phase_width(0, 8, 64, true), 4);
        for cpu in [0,1,2,16] {
            let mut p=Policy::new(cpu);
            for _ in 0..200 { p.observe(Duration::from_millis(51),8,true); }
            assert_eq!(p.width,cpu.clamp(1,8));
        }
    }
    #[test]
    fn only_enabled_busy_worlds_and_real_useful_work_count() {
        let ms=Duration::from_millis(1);
        assert_eq!(block_useful([ms;3],[true,false,false]),1);
        assert_eq!(block_useful([ms;3],[true,false,true]),2);
        assert_eq!(block_useful([Duration::from_micros(250);3],[true;3]),1);
        assert_eq!(block_useful([ms,Duration::ZERO,Duration::ZERO],[true;3]),1);
        assert_eq!(block_useful([Duration::from_micros(250),Duration::from_micros(750),Duration::ZERO],[true;3]),2);
        assert_eq!(block_useful([Duration::from_micros(250),Duration::from_micros(749),Duration::ZERO],[true;3]),1);
        assert_eq!(block_useful([Duration::from_micros(249),Duration::from_micros(751),Duration::ZERO],[true;3]),1);
        let mut p=Policy::new(8);
        for _ in 0..100 { p.observe(Duration::from_secs(1),1,true); }
        assert_eq!(p.width,1);
    }
}
