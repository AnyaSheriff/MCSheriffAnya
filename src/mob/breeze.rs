// Бриз (вики, Breeze): скользит к цели, вдыхает и стреляет зарядом ветра,
// потом отпрыгивает длинным прыжком.
//
// Кода игры здесь нет: правила — со страницы вики и из чёрного ящика
// (сценарий breeze-wind-charge, такты от вдоха):
//
// - скользит к цели — поза 15 (SLIDING), звук `slide` [проверить разгон];
// - выстрел: звук `inhale` (I), поза 16 (SHOOTING) с I+1, заряд — на I+16
//   из 1,185 над ногами к трети роста цели со скоростью 0,7, звук `shoot`
//   (1,5); поза 0 — на 6-м такте после выстрела;
// - прыжок: звук `charge`, со следующего такта поза 17 (INHALING), на 11-м
//   такте после звука — `jump`, поза 6 (LONG_JUMPING), скорость к точке
//   прыжка; в полёте без сопротивления, тяжесть 0,08 (ход по горизонтали
//   постоянен); приземление — `land`, поза 0, вдох через 2–3 такта;
// - выстрелы не чаще раза в 32 такта (вики: «cooldown of 32 game ticks»),
//   дальность 16 (вики); падения не боится, урон заряда 1.
//
// Где именно бриз выбирает точку прыжка, у оригинала случайно (два прыжка
// записи: 10 и 1,2 блока) — у нас точка в 8 блоках от бриза [проверить].

use super::ai::{Ctx, Plan};
use super::{Event, Mob, Rng, View};

/// Позы (protocol.json 26.1, Pose): 0 — стоит, 6 — длинный прыжок,
/// 15 — скольжение, 16 — выстрел, 17 — вдох.
pub const POSE_STANDING: i32 = 0;
pub const POSE_LONG_JUMPING: i32 = 6;
pub const POSE_SLIDING: i32 = 15;
pub const POSE_SHOOTING: i32 = 16;
pub const POSE_INHALING: i32 = 17;

/// Раздел звуков «враждебные» (протокол: hostile 5).
const SOUND_HOSTILE: i32 = 5;

/// Здоровье и скорость (вики: 30; чёрный ящик: атрибут скорости 0,63).
pub const HEALTH: f32 = 30.0;
pub const SPEED: f64 = 0.63;
/// Дальность выстрела (вики: «within a distance of 16 blocks»).
const SHOOT_RANGE: f64 = 16.0;
/// Пауза между выстрелами (вики: 32 такта).
const SHOOT_COOLDOWN: i32 = 32;
/// Скорость заряда (breeze-wind-charge: 0,699).
const CHARGE_SPEED: f64 = 0.7;
/// Откуда вылетает заряд: над ногами (breeze-wind-charge: 1,185).
const MUZZLE: f64 = 1.185;
/// Тяжесть в длинном прыжке (breeze-wind-charge: вертикаль −0,08 за такт).
const JUMP_GRAVITY: f64 = 0.08;

/// Что делает бриз.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    /// Вдох перед выстрелом: тактов с начала.
    Shooting(i32),
    /// Выстрел сделан: тактов после него.
    Shot(i32),
    /// Сжался перед прыжком: тактов со звука.
    Charging(i32),
    /// В длинном прыжке.
    Jumping,
    /// Приземлился: тактов после.
    Landed(i32),
}

/// Состояние бриза.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub pose: i32,
    pub phase: Phase,
    /// Тактов до права выстрелить.
    pub cooldown: i32,
    /// Скорость длинного прыжка.
    pub leap: (f64, f64, f64),
    /// Скользил ли в прошлом такте (звук `slide` — в начале скольжения).
    pub sliding: bool,
}

fn pose(mob: &mut Mob, pose: i32) {
    mob.breeze.pose = pose;
}

/// Мысль бриза за такт.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();
    let state = mob.breeze;

    if mob.breeze.cooldown > 0 {
        mob.breeze.cooldown -= 1;
    }

    // Только что заметил цель: сперва либо скользит к ней, либо отпрыгивает
    // (breeze-wind-charge: в одном прогоне оригинал скользил 25 тактов и
    // выстрелил, в другом сжался на 13-м такте и прыгнул через робота).
    if mob.h.target.is_none() && mob.breeze.phase == Phase::Idle {
        mob.breeze.cooldown = mob.breeze.cooldown.max(20);
    }

    match state.phase {
        Phase::Jumping => {
            if mob.on_ground {
                mob.events.push(Event::Sound { name: "minecraft:entity.breeze.land", volume: 1.0, pitch: 1.0 });
                mob.breeze.phase = Phase::Landed(0);
            }

            return plan;
        }
        // Звук приземления у оригинала — в двух тактах подряд (breeze-wind-charge,
        // оба прогона).
        Phase::Landed(n) => {
            if n == 0 {
                mob.events.push(Event::Sound { name: "minecraft:entity.breeze.land", volume: 1.0, pitch: 1.0 });
            }

            if n >= 1 {
                pose(mob, POSE_STANDING);
                mob.breeze.phase = Phase::Idle;
            } else {
                mob.breeze.phase = Phase::Landed(n + 1);
            }

            return plan;
        }
        _ => {}
    }

    let target = super::hostile::choose_target(mob, view, ctx, rng);

    let Some(target) = target else {
        if !matches!(mob.breeze.phase, Phase::Idle) {
            mob.breeze.phase = Phase::Idle;
        }

        if mob.breeze.pose != POSE_STANDING && mob.breeze.pose != POSE_LONG_JUMPING {
            pose(mob, POSE_STANDING);
        }

        mob.breeze.sliding = false;
        return plan;
    };

    super::ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    let (dx, dz) = (target.x - mob.x, target.z - mob.z);
    let flat = dx.hypot(dz);
    let sight = super::hostile::sees(view, (mob.x, mob.y + mob.eye(), mob.z), (target.x, target.y + target.eye, target.z));

    match mob.breeze.phase {
        Phase::Shooting(n) => {
            // Поза выстрела — со следующего такта после вдоха.
            if n == 0 {
                pose(mob, POSE_SHOOTING);
            }

            if n + 1 >= 15 {
                let from = (mob.x, mob.y + MUZZLE, mob.z);
                let aim = (target.x - from.0, target.y + 1.8 / 3.0 - from.1, target.z - from.2);
                let length = (aim.0 * aim.0 + aim.1 * aim.1 + aim.2 * aim.2).sqrt().max(1.0e-9);
                // Разброс — как у снаряда с неточностью 1 [проверить].
                let mut wobble = || (rng.unit() + rng.unit() + rng.unit() + rng.unit() - 2.0) * 0.0172275 * 1.732;
                let velocity = ((aim.0 / length + wobble()) * CHARGE_SPEED, (aim.1 / length + wobble()) * CHARGE_SPEED, (aim.2 / length + wobble()) * CHARGE_SPEED);

                mob.events.push(Event::Sound { name: "minecraft:entity.breeze.shoot", volume: 1.5, pitch: 1.0 });
                plan.wind_charge = Some((from, velocity));
                mob.breeze.cooldown = SHOOT_COOLDOWN;
                mob.breeze.phase = Phase::Shot(0);
            } else {
                mob.breeze.phase = Phase::Shooting(n + 1);
            }
        }
        Phase::Shot(n) => {
            if n + 1 >= 6 {
                pose(mob, POSE_STANDING);
                mob.events.push(Event::EntitySound { name: "minecraft:entity.breeze.charge", category: SOUND_HOSTILE, volume: 1.0, pitch: 1.0 });
                mob.breeze.phase = Phase::Charging(0);
            } else {
                mob.breeze.phase = Phase::Shot(n + 1);
            }
        }
        Phase::Charging(n) => {
            if n == 0 {
                pose(mob, POSE_INHALING);
            }

            if n + 1 >= 11 && mob.on_ground {
                // Точка прыжка — случайная в 8 блоках; время полёта — по
                // горизонтальному ходу около 0,53 за такт (breeze-wind-charge).
                let angle = rng.unit() * std::f64::consts::TAU;
                let reach = 1.0 + rng.unit() * 7.0;
                let (jx, jz) = (angle.cos() * reach, angle.sin() * reach);
                let ticks = (reach / 0.53).clamp(4.0, 18.0);

                mob.breeze.leap = (jx / ticks, JUMP_GRAVITY * ticks / 2.0, jz / ticks);
                mob.vx = mob.breeze.leap.0;
                mob.vy = mob.breeze.leap.1;
                mob.vz = mob.breeze.leap.2;
                mob.on_ground = false;
                mob.events.push(Event::Sound { name: "minecraft:entity.breeze.jump", volume: 1.0, pitch: 1.0 });
                pose(mob, POSE_LONG_JUMPING);
                mob.breeze.phase = Phase::Jumping;
            } else {
                mob.breeze.phase = Phase::Charging(n + 1);
            }
        }
        _ => {
            let close = flat * flat + (target.y - mob.y).powi(2) < SHOOT_RANGE * SHOOT_RANGE;

            if mob.breeze.cooldown == 0 && close && sight && mob.on_ground {
                mob.events.push(Event::Sound { name: "minecraft:entity.breeze.inhale", volume: 1.0, pitch: 1.0 });
                pose(mob, POSE_STANDING);
                mob.breeze.sliding = false;
                mob.breeze.phase = Phase::Shooting(0);
                return plan;
            }

            // Скользит к цели (breeze-wind-charge: до ~2 блоков).
            if flat > 2.5 && mob.on_ground {
                let want = (-dx.atan2(dz)).to_degrees() as f32;

                mob.yaw = super::ai::turn(mob.yaw, want, 30.0);
                plan.speed = 0.44;

                if !mob.breeze.sliding {
                    mob.events.push(Event::Sound { name: "minecraft:entity.breeze.slide", volume: 1.0, pitch: 1.0 });
                    pose(mob, POSE_SLIDING);
                    mob.breeze.sliding = true;
                }
            } else if mob.breeze.sliding {
                mob.breeze.sliding = false;
                pose(mob, POSE_STANDING);
            }
        }
    }

    plan
}

/// Ход бриза в длинном прыжке: без сопротивления, тяжесть 0,08.
pub fn leap(mob: &mut Mob, view: &dyn View) -> bool {
    if mob.breeze.phase != Phase::Jumping {
        return false;
    }

    mob.step(view, mob.vx, mob.vy, mob.vz);
    mob.vy -= JUMP_GRAVITY;
    mob.vitals.fall = 0.0;

    if mob.on_ground {
        mob.vx *= 0.546;
        mob.vz *= 0.546;
        mob.vy = 0.0;
    }

    true
}
