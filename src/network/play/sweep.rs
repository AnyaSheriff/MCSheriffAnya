// Вики, Sword / Melee attack: «standing still or walking», «84.8%».
// Заряд >0,9 даёт >84,8% основного урона. Пройденный за такт путь
// учитывается целиком, даже если клиент прислал несколько пакетов.
// Масштаб пути 0,6 и граница такта [проверить] отдельным зондом движения.
// https://minecraft.wiki/w/Sword (до 26.1.2)
use super::*;

#[derive(Default)]
pub(super) struct Motion {
    tick: u64,
    distance: f64,
}

impl Motion {
    pub(super) fn moved(&mut self, now: u64, dx: f64, dz: f64) {
        if self.tick != now { self.tick = now; self.distance = 0.0; }
        self.distance += dx.hypot(dz) * 0.6;
    }

    fn distance(&self, now: u64) -> f64 {
        if now.saturating_sub(self.tick) <= 1 { self.distance } else { 0.0 }
    }
}

pub(super) fn eligible(player: &PlayerState, full: bool, critical: bool, now: u64) -> bool {
    full && !critical && player.on_ground && !player.vit.sprinting
        && player.net.sweep_motion.distance(now) < player.attrs.value(crate::attributes::MOVEMENT_SPEED)
        && player.inventory.held().and_then(|s| blocks::item_name(s.item)).is_some_and(|n| n.ends_with("_sword"))
}

/// Как при добыче блока: флаг клиента может быть false, хотя ноги уже
/// стоят на полу (itemaudit4-knockback). Проверяем реальную опору перед
/// ударом; вверх летящий игрок и незаконченный перенос не приземляются.
pub(super) fn settle(shared: &Shared, player: &mut PlayerState) {
    if player.on_ground || player.awaiting_teleport || player.abilities.flying || player.ride.vehicle.is_some()
        || player.net.last_delta.1 > 0.0 || (player.y-player.y.floor()).abs() >= 0.001 { return; }
    let world = shared.world_in(player.dimension).lock().expect("мир");
    let below = world.get_block(player.x.floor() as i32,(player.y-0.001).floor() as i32,player.z.floor() as i32);
    if blocks::full_cube(below) { player.on_ground = true; }
}
