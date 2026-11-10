// Ограниченный native BlockBreakEvent; mandatory world остаётся прежним.
use std::time::Duration;

#[derive(Clone, Copy, Debug, wasmtime::component::ComponentType, wasmtime::component::Lift, wasmtime::component::Lower)]
// Варианты создаёт component lifting, не обычный Rust constructor.
#[allow(dead_code)]
#[component(enum)]
#[repr(u8)]
pub(super) enum FrameError {
    #[component(name = "missing")]
    Missing,
    #[component(name = "duplicate")]
    Duplicate,
    #[component(name = "capacity")]
    Capacity,
    #[component(name = "order")]
    Order,
    #[component(name = "priority")]
    Priority,
}

pub(super) type Begin = wasmtime::component::TypedFunc<(u64, super::api::mcsa::api::types::Player, super::api::mcsa::api::types::BlockPos, String, bool), (Result<(), FrameError>,)>;
pub(super) type Phase = wasmtime::component::TypedFunc<(u64, u8, bool), (Result<bool, FrameError>,)>;
pub(super) type Finish = wasmtime::component::TypedFunc<(u64, bool), (Result<(), FrameError>,)>;
#[derive(Clone, Copy)]
pub(super) struct Extension { pub begin: Begin, pub phase: Phase, pub finish: Finish }

// Остатки одного логического события, не восемь отдельных полных выдач.
pub(super) struct Budget { pub fuel: u64, pub guest: Duration, pub native: u64, pub native_time: Duration }
impl Budget {
    pub fn new(fuel: u64, guest: Duration, native: u64, epoch: Duration) -> Self { Self { fuel, guest, native, native_time: epoch.saturating_mul(native.min(u32::MAX as u64) as u32) } }
    pub fn available(&self) -> bool { self.fuel > 0 && !self.guest.is_zero() }
    pub fn charge(&mut self, fuel: u64, elapsed: Duration, native: u64, native_elapsed: Duration) -> Result<(), String> {
        if fuel > self.fuel || native > self.native { return Err("Рост остатка бюджета native frame".into()); }
        let credited=native_elapsed.min(self.native_time);
        self.native_time=self.native_time.saturating_sub(credited);
        let guest = elapsed.saturating_sub(credited);
        let exceeded = guest > self.guest;
        self.fuel = fuel; self.native = native; self.guest = self.guest.saturating_sub(guest);
        if exceeded { Err("Исчерпан суммарный срок native frame".into()) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_budget_never_refills_between_phases_or_finish() {
        let mut b=Budget::new(100,Duration::from_millis(20),10,Duration::from_millis(10));
        b.charge(60,Duration::from_millis(12),10,Duration::ZERO).unwrap();
        assert_eq!(b.guest,Duration::from_millis(8));
        b.charge(20,Duration::from_millis(15),9,Duration::from_millis(10)).unwrap();
        assert_eq!(b.guest,Duration::from_millis(3));assert_eq!(b.native,9);
        assert!(b.charge(19,Duration::from_millis(4),9,Duration::ZERO).is_err());
        assert!(!b.available());
    }
    #[test]
    fn native_time_after_allowance_is_charged_to_guest_across_exports() {
        let mut b=Budget::new(100,Duration::from_millis(20),1,Duration::from_millis(10));
        b.charge(90,Duration::from_millis(15),0,Duration::from_millis(15)).unwrap();
        assert_eq!(b.native_time,Duration::ZERO);assert_eq!(b.guest,Duration::from_millis(15));
        b.charge(80,Duration::from_millis(15),0,Duration::from_millis(15)).unwrap();
        assert!(!b.available());
    }
    #[test]
    fn fuel_native_and_guest_have_independent_finite_remainders() {
        let mut b=Budget::new(10,Duration::from_millis(20),2,Duration::from_millis(10));
        assert!(b.charge(11,Duration::ZERO,2,Duration::from_millis(10)).is_err());
        assert!(b.charge(9,Duration::ZERO,3,Duration::from_millis(10)).is_err());
        b.charge(0,Duration::from_millis(1),0,Duration::from_millis(10)).unwrap();
        assert!(!b.available());assert_eq!(b.native,0);
    }
}
