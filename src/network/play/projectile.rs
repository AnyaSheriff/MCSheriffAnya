// Общий бросок из рук: направление, разброс и движение стрелка.
// Вики, Projectile: «the projectile inherits the velocity of the player»;
// вертикаль прибавляется в воздухе. Распределение для 26.1.2 [проверить]
// большой выборкой throw2-*; масштаб из задания/вики — 0,0075 на ось.
use super::*;

pub(super) fn noise(roll: &mut dyn FnMut() -> f32) -> f64 {
    // Преобразование Бокса — Мюллера: нормальная величина, σ = 0,0075.
    // random_unit может дать ноль, поэтому логарифм берём от 1 − U.
    let u = (1.0 - f64::from(roll())).max(f64::EPSILON);
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * f64::from(roll())).cos() * 0.0075
}

pub(super) fn inherit(shot: (f64, f64, f64), movement: (f64, f64, f64), on_ground: bool) -> (f64, f64, f64) {
    (shot.0 + movement.0, shot.1 + if on_ground { 0.0 } else { movement.1 }, shot.2 + movement.2)
}

pub(super) fn velocity(shared: &Shared, player: &PlayerState, speed: f64, lifted: bool, yaw_offset: f32) -> (f64, f64, f64) {
    let mut roll = || vitals::world_random(shared, player.dimension);
    let mut wobble = || noise(&mut roll);
    let shot = if lifted {
        crate::thrown::throw_velocity(player.yaw + yaw_offset, player.pitch, speed, &mut wobble)
    } else {
        let (x, y, z) = crate::arrow::aim(player.yaw + yaw_offset, player.pitch, 1.0);
        ((x + wobble()) * speed, (y + wobble()) * speed, (z + wobble()) * speed)
    };
    inherit(shot, player.net.last_delta, player.on_ground)
}

/// Относительный пакет вмещает только [-8; 8) блоков. Обрезать нельзя:
/// клиент получил бы другую точку, а следующий сдвиг считался бы от полной.
pub(super) fn relative_step(from: (i64, i64, i64), to: (i64, i64, i64)) -> Option<[i16; 3]> {
    Some([i16::try_from(to.0 - from.0).ok()?, i16::try_from(to.1 - from.1).ok()?, i16::try_from(to.2 - from.2).ok()?])
}

/// Entity Position Sync (protocol.json 26.1): место и скорость одного снимка.
pub(super) fn position_sync(id: i32, at: (f64, f64, f64), velocity: (f64, f64, f64), (yaw, pitch): (f32, f32), on_ground: bool) -> Vec<u8> {
    let mut body = encode_varint(0x23);
    body.extend_from_slice(&encode_varint(id));
    for value in [at.0, at.1, at.2, velocity.0, velocity.1, velocity.2] { push_f64(&mut body, value); }
    push_f32(&mut body, yaw);
    push_f32(&mut body, pitch);
    body.push(u8::from(on_ground));
    body
}
