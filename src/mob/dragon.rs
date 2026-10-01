//! Дракон Края: фазы полёта и посадки (вики, Ender Dragon, «AI phases»).
//! Номера фаз — как в DragonPhase: 0 — кружит над столбами («holding
//! pattern»), 1 — заходит на игрока («strafing»), 2 — летит к порталу,
//! 3 — садится, 4 — взлетает, 5 — дышит сидя, 6 — сидит и ищет игрока,
//! 7 — рычит перед дыханием, 8 — несётся на игрока, 9 — умирает,
//! 10 — висит (призванный). Тактов по фазам вики не даёт: ход, скорости
//! и выбор точек круга — наши [проверить], с вики взяты расстояния
//! (10/150/64/20 блоков), 1,25 с до дыхания, 3 с дыхания, четыре
//! дыхания подряд, 50 урона на посадке — взлёт, вероятность посадки
//! «1 in (3 + crystals alive)».

use super::{ai, Mob, Rng};
use crate::world::the_end::CENTER_TOP;

pub const CIRCLING: u8 = 0;
pub const STRAFING: u8 = 1;
pub const APPROACH: u8 = 2;
pub const LANDING: u8 = 3;
pub const TAKEOFF: u8 = 4;
pub const BREATH: u8 = 5;
pub const SCANNING: u8 = 6;
pub const ROAR: u8 = 7;
pub const CHARGING: u8 = 8;
pub const DYING: u8 = 9;
pub const HOVER: u8 = 10;

/// Где дракон сидит на портале: верх бедрокового столба подставки плюс один.
pub const PERCH_Y: f64 = (CENTER_TOP + 4) as f64;

/// Высота круга над столбами и радиус круга [проверить].
const RING_Y: f64 = 100.0;
const RING_R: f64 = 50.0;

/// Скорость по горизонтали за такт и разгон [проверить].
const SPEED: f64 = 0.6;
const CHARGE_SPEED: f64 = 1.0;
const ACCEL: f64 = 0.05;
const CLIMB: f64 = 0.35;

/// Дальность, с которой дракон лечится кристаллом (вики, End Crystal:
/// «cuboid extending 32 blocks from the dragon in all directions»).
pub const HEAL_RANGE: f64 = 32.0;

/// Сколько тактов между очками лечения (вики: «1HP every 10 game ticks»).
pub const HEAL_EVERY: u64 = 10;

/// Что помнит дракон между тактами.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub phase: u8,

    /// Куда летит сейчас.
    pub target: Option<(f64, f64, f64)>,

    /// Счётчик тактов текущей фазы.
    pub timer: i32,

    /// Номер узла круга, к которому летит.
    pub node: i32,

    /// Сколько дыханий сделано за эту посадку.
    pub breaths: u8,

    /// Урон, накопленный на посадке: с 50 взлетает (вики).
    pub perch_damage: f32,

    /// Сколько кристаллов живо — для вероятности посадки.
    pub crystals: u8,

    /// Дракон боя Края: кружит и садится; призванный вне боя висит.
    pub fighting: bool,

    /// Кого преследует (номер игрока).
    pub prey: Option<i32>,

    /// Опыт, который ещё предстоит выронить по ходу смерти.
    pub xp: i32,
}

impl Default for State {
    fn default() -> Self {
        State { phase: HOVER, target: None, timer: 0, node: 0, breaths: 0, perch_damage: 0.0, crystals: 0, fighting: false, prey: None, xp: 0 }
    }
}

impl State {
    /// Переводит в фазу, сбрасывая счётчик.
    pub fn enter(&mut self, phase: u8) {
        self.phase = phase;
        self.timer = 0;
        self.target = None;
    }
}

/// Узел круга над столбами: по кругу радиусом `RING_R` с переменной
/// высотой [проверить].
fn ring_node(node: i32) -> (f64, f64, f64) {
    let angle = f64::from(node.rem_euclid(12)) * std::f64::consts::TAU / 12.0;
    let y = RING_Y + f64::from(node.rem_euclid(12) % 3) * 8.0;

    (RING_R * angle.cos(), y, RING_R * angle.sin())
}

/// Ближайший игрок (не наблюдатель) не дальше `range`.
fn nearest(mob: &Mob, players: &[ai::Watcher], range: f64) -> Option<(i32, (f64, f64, f64))> {
    players
        .iter()
        .filter(|player| !player.spectator)
        .map(|player| (player.id, (player.x, player.y, player.z), (player.x - mob.x).hypot(player.z - mob.z)))
        .filter(|(_, _, dist)| *dist <= range)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(id, at, _)| (id, at))
}

/// Такт дракона: мысль и ход. Призванный вне боя висит на месте (чёрный
/// ящик, dragon-summon). Возвращает, нужно ли выдохнуть (облако дыхания).
pub fn think(mob: &mut Mob, players: &[ai::Watcher], rng: &mut Rng) {
    if mob.no_ai || !mob.dragon.fighting {
        return;
    }

    let mut state = std::mem::take(&mut mob.dragon);
    state.timer += 1;

    match state.phase {
        HOVER => {
            state.enter(CIRCLING);
            state.target = Some(ring_node(state.node));
        }
        CIRCLING => {
            let target = *state.target.get_or_insert_with(|| ring_node(state.node));

            if fly_to(mob, target, SPEED) < 10.0 {
                state.node += 1;
                state.target = Some(ring_node(state.node));

                // Посадка с вероятностью «1 in (3 + crystals alive)» на
                // каждом узле [проверить: у оригинала — при смене цели].
                if rng.below(3 + u32::from(state.crystals)) == 0 {
                    state.enter(APPROACH);
                } else if state.timer > 200 && rng.below(4) == 0 {
                    state.enter(STRAFING);
                }
            }
        }
        STRAFING => {
            // Заходит на игрока не дальше 64 блоков; без цели — назад в круг.
            match nearest(mob, players, 150.0) {
                Some((id, at)) => {
                    state.prey = Some(id);
                    let dist = fly_to(mob, (at.0, at.1 + 10.0, at.2), SPEED);

                    if dist < 20.0 || state.timer > 300 {
                        state.enter(CIRCLING);
                    }
                }
                None => state.enter(CIRCLING),
            }
        }
        APPROACH => {
            // К точке над порталом, потом вниз.
            if fly_to(mob, (0.0, PERCH_Y + 30.0, 0.0), SPEED) < 6.0 {
                state.enter(LANDING);
            }
        }
        LANDING => {
            mob.vx = 0.0;
            mob.vz = 0.0;
            mob.x += (0.0 - mob.x).clamp(-0.2, 0.2);
            mob.z += (0.0 - mob.z).clamp(-0.2, 0.2);
            mob.y += (PERCH_Y - mob.y).clamp(-0.3, 0.3);

            if (mob.y - PERCH_Y).abs() < 0.05 && mob.x.abs() < 0.3 && mob.z.abs() < 0.3 {
                state.enter(SCANNING);
                state.breaths = 0;
                state.perch_damage = 0.0;
            }
        }
        SCANNING => {
            // 1,25 с на посадке; игрок в 20 блоках от портала — рык и дыхание.
            if state.perch_damage >= 50.0 {
                state.enter(TAKEOFF);
            } else if state.timer >= 25 {
                if let Some((id, at)) = nearest(mob, players, 20.0) {
                    state.prey = Some(id);
                    face_at(mob, at);
                    state.enter(ROAR);
                } else if state.timer >= 100 {
                    state.enter(TAKEOFF);
                }
            }
        }
        ROAR => {
            if state.timer == 1 {
                mob.events.push(super::Event::Sound { name: "minecraft:entity.ender_dragon.growl", volume: 2.5, pitch: 0.8 + rng.unit() as f32 * 0.3 });
            }

            if state.timer >= 10 {
                state.enter(BREATH);
            }
        }
        BREATH => {
            // 3 с дыхания, потом снова ищет; четыре дыхания — взлёт (вики).
            if state.perch_damage >= 50.0 {
                state.enter(TAKEOFF);
            } else if state.timer >= 60 {
                state.breaths += 1;
                state.enter(if state.breaths >= 4 { TAKEOFF } else { SCANNING });
            }
        }
        TAKEOFF => {
            if state.timer == 1 {
                mob.events.push(super::Event::Sound { name: "minecraft:entity.ender_dragon.flap", volume: 5.0, pitch: 0.8 + rng.unit() as f32 * 0.3 });
            }

            if fly_to(mob, (0.0, PERCH_Y + 30.0, 0.0), SPEED * 0.6) < 6.0 {
                state.enter(CIRCLING);
            }
        }
        CHARGING => match state.prey.and_then(|id| players.iter().find(|player| player.id == id)) {
            Some(player) if state.timer < 100 => {
                if fly_to(mob, (player.x, player.y + 2.0, player.z), CHARGE_SPEED) < 4.0 {
                    state.enter(CIRCLING);
                }
            }
            _ => state.enter(CIRCLING),
        },
        _ => {}
    }

    mob.dragon = state;
}

/// Умирающий дракон тянется к порталу и поднимается (вики, Ender Dragon:
/// «ascends at (0,0) over 10 seconds») [проверить: скорость].
pub fn die_step(mob: &mut Mob) {
    if !mob.dragon.fighting {
        return;
    }

    mob.dragon.phase = DYING;
    mob.x += (0.0 - mob.x).clamp(-0.3, 0.3);
    mob.z += (0.0 - mob.z).clamp(-0.3, 0.3);
    mob.y += 0.1;
    mob.vx = 0.0;
    mob.vy = 0.1;
    mob.vz = 0.0;
}

/// Удар по дракону: на посадке копит урон; в полёте с четвертью случаев
/// кидается на обидчика [проверить].
pub fn on_hurt(mob: &mut Mob, amount: f32, attacker: Option<i32>, rng: &mut Rng) {
    if matches!(mob.dragon.phase, SCANNING | ROAR | BREATH) {
        mob.dragon.perch_damage += amount;
    } else if matches!(mob.dragon.phase, CIRCLING | STRAFING)
        && mob.dragon.fighting
        && let Some(id) = attacker
        && rng.below(4) == 0
    {
        mob.dragon.prey = Some(id);
        mob.dragon.enter(CHARGING);
    }
}

/// Поворачивает дракона к точке.
fn face_at(mob: &mut Mob, at: (f64, f64, f64)) {
    let yaw = (-(at.0 - mob.x)).atan2(at.2 - mob.z).to_degrees() as f32;
    mob.face(yaw);
}

/// Летит к точке: разгон до `speed` по горизонтали, подъём до `CLIMB`,
/// нос — по ходу. Возвращает расстояние до точки до сдвига.
fn fly_to(mob: &mut Mob, target: (f64, f64, f64), speed: f64) -> f64 {
    let (dx, dy, dz) = (target.0 - mob.x, target.1 - mob.y, target.2 - mob.z);
    let flat = dx.hypot(dz);
    let dist = (flat * flat + dy * dy).sqrt();

    if flat > 1.0e-6 {
        let (ux, uz) = (dx / flat, dz / flat);
        mob.vx += (ux * speed - mob.vx).clamp(-ACCEL, ACCEL);
        mob.vz += (uz * speed - mob.vz).clamp(-ACCEL, ACCEL);
    }

    mob.vy = dy.clamp(-CLIMB, CLIMB);
    mob.x += mob.vx;
    mob.y += mob.vy;
    mob.z += mob.vz;

    if mob.vx.hypot(mob.vz) > 1.0e-3 {
        // Дракон летит хвостом вперёд по yaw оригинала: его нос смотрит
        // против направления тела, поэтому поворот — к движению плюс 180°
        // [проверить].
        let yaw = (-mob.vx).atan2(mob.vz).to_degrees() as f32 + 180.0;
        mob.face(yaw);
    }

    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::Kind;

    fn dragon() -> Mob {
        let mut mob = Mob::new(10, Kind::EnderDragon, (0.0, 128.0, 0.0));
        mob.dragon.fighting = true;
        mob
    }

    #[test]
    fn summoned_dragon_hovers() {
        let mut mob = Mob::new(10, Kind::EnderDragon, (0.0, 128.0, 0.0));
        let mut rng = Rng::new(1);

        think(&mut mob, &[], &mut rng);
        assert_eq!(mob.dragon.phase, HOVER);
        assert_eq!((mob.x, mob.y, mob.z), (0.0, 128.0, 0.0));
    }

    #[test]
    fn fight_dragon_starts_circling_and_moves() {
        let mut mob = dragon();
        let mut rng = Rng::new(1);

        for _ in 0..40 {
            think(&mut mob, &[], &mut rng);
        }

        assert_ne!(mob.dragon.phase, HOVER);
        assert!((mob.x, mob.z) != (0.0, 0.0), "дракон не сдвинулся");
    }

    #[test]
    fn perch_damage_makes_it_take_off() {
        let mut mob = dragon();
        let mut rng = Rng::new(1);

        mob.dragon.enter(SCANNING);
        mob.y = PERCH_Y;
        on_hurt(&mut mob, 60.0, None, &mut rng);
        think(&mut mob, &[], &mut rng);
        assert_eq!(mob.dragon.phase, TAKEOFF);
    }

    #[test]
    fn scanning_roars_at_close_player_after_25_ticks() {
        let mut mob = dragon();
        let mut rng = Rng::new(1);
        let player = ai::Watcher { id: 1, x: 5.0, y: 62.0, z: 0.0, eye: 1.62, holding: [None, None], spectator: false, targetable: true };

        mob.dragon.enter(SCANNING);
        mob.y = PERCH_Y;

        for _ in 0..24 {
            think(&mut mob, std::slice::from_ref(&player), &mut rng);
        }
        assert_eq!(mob.dragon.phase, SCANNING);
        think(&mut mob, std::slice::from_ref(&player), &mut rng);
        assert_eq!(mob.dragon.phase, ROAR);
    }
}
