// Стражи и древние стражи (вики, Guardian и Elder Guardian): плавают в воде,
// на суше бьются и подпрыгивают, бьют лучом, а ударившего вплотную колют
// шипами; древний раз в минуту насылает «Утомление» III.
//
// Кода игры здесь нет: правила — со страниц вики, тайминги — с официального
// сервера чёрным ящиком (tools/blackbox/scenarios/guardian-beam.json,
// guardian-spikes.json, elder-fatigue.json):
//
// - луч: цель (свойство 17 — её номер) держится 80 тактов у стража и 60 у
//   древнего (вики: «4 seconds», «3 seconds»); урон — через 79 тактов после
//   свойства 17 (guardian-beam: 2730 → 2809, 3407 → 3486; у нас свойство
//   уходит на такт позже захвата — слежение шлёт мобов до их хода, — так что
//   выстрел на 80-м такте от захвата), вид indirect_magic
//   (Damage Event type=23), всего 6 на «нормальной» — атака 6 по сложности
//   (вики: 4/6/9, древний 5/8/12), толчок прочь от стража; через 2 такта
//   видно снятие цели (17=0), страж снова плывёт (16=true). События 21
//   оригинал не шлёт (guardian-beam): звук луча клиент играет по свойству 17;
// - шипы: удар игрока в ближнем бою, пока страж не движется (16=false),
//   ранит ударившего на 2 (вики: «Spikes defense»; guardian-spikes: Damage
//   Event type=43, звук enchant.thorns.hit у игрока, толчок 0,4) — раньше,
//   чем урон самому стражу;
// - проклятие древнего: раз в 1200 тактов (по возрасту и номеру сущности)
//   игрокам в выживании и приключении в 50 блоках — эффект id=3 amp=2
//   dur=6000 и событие игры 10 (elder-fatigue: эффект и событие в одном
//   такте, value=0 у тихого — звук играет клиент сам).
//
// Что не сверено, помечено «[проверить]».

use super::ai::{self, Ctx, Plan};
use super::{Kind, Mob, Rng, View};

/// Сколько тактов заряжается луч (вики: стража 4 с, древнего 3 с).
pub const BEAM_GUARDIAN: i32 = 80;
pub const BEAM_ELDER: i32 = 60;

/// Урон луча до поправки на сложность (вики: на «нормальной» 6 и 8; лёгкая и
/// трудная — общим правилом сложности, survival::scaled).
pub const DAMAGE_GUARDIAN: f32 = 6.0;
pub const DAMAGE_ELDER: f32 = 8.0;

/// Урон шипов по ударившему (вики: «2» на лёгкой и нормальной) [проверить:
/// на трудной вики пишет 3,5 и 3].
pub const SPIKES: f32 = 2.0;

/// Дальность луча (вики: «maximum range of 15 blocks», у древнего ~14) и
/// дальность, где страж замечает игрока [проверить: 16 — дальность преследования].
const BEAM_RANGE: f64 = 15.0;
const SEEK_RANGE: f64 = 16.0;

/// Шанс за такт взять цель (оригинал против оригинала, guardian-beam: захват
/// через 19–33 такта) [проверить].
const SEEK_CHANCE: u32 = 10;

/// Через сколько тактов после выстрела цель снимается (guardian-beam: 17=0
/// видно на +2 — у нас свойство уходит на такт позже).
const AFTER_SHOT: i32 = 1;

/// Проклятие древнего: период, дальность, эффект (вики, Elder Guardian;
/// elder-fatigue: id=3 amp=2 dur=6000).
pub const CURSE_EVERY: u64 = 1200;
pub const CURSE_RANGE: f64 = 50.0;
pub const CURSE_TICKS: i32 = 6000;
pub const CURSE_AMPLIFIER: i32 = 2;

/// Скорость плавания, блоков за такт [проверить: вики даёт только «abrupt
/// charges»; атрибут скорости 0,5].
const SWIM_SPEED: f64 = 0.12;

/// Опыт за стража (вики: «10XP when killed by a player»).
pub const XP: i32 = 10;

/// Состояние стража.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Кого держит луч (номер сущности игрока).
    pub target: Option<i32>,
    /// Тактов с захвата цели.
    pub beam: i32,
    /// Выстрел был: тактов до снятия цели.
    pub after: i32,
    /// Плывёт — шипы втянуты (свойство 16).
    pub moving: bool,
    /// В воде ли (голоса суши и воды).
    pub wet: bool,
    /// Куда плывёт.
    pub swim_to: Option<(f64, f64, f64)>,
    /// Игроки, на которых пало проклятие в этом такте (забирает step_all).
    pub cursed: Vec<i32>,
}

/// Страж ли это.
pub fn guardian(kind: Kind) -> bool {
    matches!(kind, Kind::Guardian | Kind::ElderGuardian)
}

/// Голоса: в воде и на суше свои (вики, Guardian, «Sounds»).
pub fn sounds(mob: &Mob) -> (&'static str, &'static str, &'static str) {
    match (mob.kind == Kind::ElderGuardian, mob.guardian.wet) {
        (false, true) => ("minecraft:entity.guardian.ambient", "minecraft:entity.guardian.hurt", "minecraft:entity.guardian.death"),
        (false, false) => ("minecraft:entity.guardian.ambient_land", "minecraft:entity.guardian.hurt_land", "minecraft:entity.guardian.death_land"),
        (true, true) => ("minecraft:entity.elder_guardian.ambient", "minecraft:entity.elder_guardian.hurt", "minecraft:entity.elder_guardian.death"),
        (true, false) => ("minecraft:entity.elder_guardian.ambient_land", "minecraft:entity.elder_guardian.hurt_land", "minecraft:entity.elder_guardian.death_land"),
    }
}

/// Колют ли шипы: страж жив и стоит (вики: «while its spikes are extended»).
pub fn spikes_out(mob: &Mob) -> bool {
    guardian(mob.kind) && !mob.is_dead() && !mob.guardian.moving
}

/// В воде ли клетка тела.
fn in_water(mob: &Mob, view: &dyn View) -> bool {
    let (x, y, z) = mob.cell();

    super::is_water(view.block(x, y, z))
}

/// Обновляет «в воде» — каждый такт, и у стража без ИИ.
pub fn sense(mob: &mut Mob, view: &dyn View) {
    mob.guardian.wet = in_water(mob, view);
}

/// Мысль стража: проклятие (древний), луч по цели, иначе — плыть.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();

    ai::voice(mob, rng);

    if mob.kind == Kind::ElderGuardian && (mob.ticks + mob.entity_id as u64).is_multiple_of(CURSE_EVERY) {
        let (x, y, z) = (mob.x, mob.y, mob.z);

        mob.guardian.cursed = ctx
            .players
            .iter()
            .filter(|player| player.targetable && (player.x - x).powi(2) + (player.y - y).powi(2) + (player.z - z).powi(2) < CURSE_RANGE * CURSE_RANGE)
            .map(|player| player.id)
            .collect();
    }

    let eye = (mob.x, mob.y + mob.eye(), mob.z);
    let peaceful = ctx.difficulty == crate::survival::PEACEFUL;

    // Цель держится, пока видна и в дальности.
    if let Some(id) = mob.guardian.target {
        let keep = !peaceful
            && ctx.players.iter().find(|player| player.id == id).is_some_and(|player| {
                let at = (player.x, player.y + player.eye, player.z);

                player.targetable
                    && (at.0 - eye.0).powi(2) + (at.1 - eye.1).powi(2) + (at.2 - eye.2).powi(2) <= BEAM_RANGE * BEAM_RANGE
                    && super::hostile::sees(view, eye, at)
            });

        // Исчезнувшая цель (в том числе притворившийся мёртвым аксолотль)
        // не оставляет луч висеть в паузе после выстрела. [проверить] Пакеты.
        if !keep && (mob.guardian.after == 0 || !ctx.players.iter().any(|p|p.id==id)) {
            mob.guardian.target = None;
            mob.guardian.beam = 0;
            mob.guardian.after = 0;
        }
    }

    if mob.guardian.target.is_none() && !peaceful && rng.below(SEEK_CHANCE) == 0 {
        let near = ctx
            .players
            .iter()
            .filter(|player| player.targetable && !player.spectator)
            .map(|player| (player, (player.x - eye.0).powi(2) + (player.y + player.eye - eye.1).powi(2) + (player.z - eye.2).powi(2)))
            .filter(|(player, dist)| *dist <= SEEK_RANGE * SEEK_RANGE && super::hostile::sees(view, eye, (player.x, player.y + player.eye, player.z)))
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((player, _)) = near {
            mob.guardian.target = Some(player.id);
            mob.guardian.beam = 0;
            mob.guardian.after = 0;
            mob.guardian.moving = false;
            mob.guardian.swim_to = None;

            // Счёт луча — со следующего такта.
            return plan;
        }
    }

    if let Some(id) = mob.guardian.target {
        let Some(player) = ctx.players.iter().find(|player| player.id == id) else {
            return plan;
        };
        let (dx, dy, dz) = (player.x - mob.x, player.y + player.eye - eye.1, player.z - mob.z);

        mob.face((-dx.atan2(dz).to_degrees()) as f32);
        mob.pitch = (-(dy.atan2(dx.hypot(dz))).to_degrees()) as f32;

        if mob.guardian.after > 0 {
            mob.guardian.after -= 1;

            if mob.guardian.after == 0 {
                mob.guardian.target = None;
                mob.guardian.beam = 0;
                // После выстрела отплывает (guardian-beam: 16=true вместе с 17=0).
                mob.guardian.swim_to = Some((mob.x + f64::from(rng.below(9) as i32 - 4), mob.y, mob.z + f64::from(rng.below(9) as i32 - 4)));
                mob.guardian.moving = mob.guardian.wet;
            }

            return plan;
        }

        mob.guardian.beam += 1;

        let duration = if mob.kind == Kind::ElderGuardian { BEAM_ELDER } else { BEAM_GUARDIAN };

        if mob.guardian.beam >= duration {
            let length = dx.hypot(dz);
            let push = if length > 1.0e-6 { (dx / length, dz / length) } else { (0.0, 0.0) };
            let damage = if mob.kind == Kind::ElderGuardian { DAMAGE_ELDER } else { DAMAGE_GUARDIAN };

            plan.melee = Some(super::hostile::Melee { target: id, damage, push, wither: false, hunger: false });
            mob.guardian.after = AFTER_SHOT;
        }

        return plan;
    }

    // Без цели: в воде — плыть к случайной точке воды; на суше — биться.
    if mob.guardian.wet {
        let arrived = mob.guardian.swim_to.is_none_or(|(x, y, z)| (x - mob.x).powi(2) + (y - mob.y).powi(2) + (z - mob.z).powi(2) < 1.0);

        if arrived && rng.below(40) == 0 {
            let pick = (
                mob.x + f64::from(rng.below(21) as i32 - 10),
                mob.y + f64::from(rng.below(15) as i32 - 7),
                mob.z + f64::from(rng.below(21) as i32 - 10),
            );

            if super::is_water(view.block(pick.0.floor() as i32, pick.1.floor() as i32, pick.2.floor() as i32)) {
                mob.guardian.swim_to = Some(pick);
            }
        } else if arrived {
            mob.guardian.swim_to = None;
        }

        mob.guardian.moving = mob.guardian.swim_to.is_some();
    } else {
        mob.guardian.swim_to = None;
        mob.guardian.moving = false;
    }

    plan
}

/// Движение стража за такт: в воде — без тяжести, к точке плавания; на суше —
/// обычная тяжесть и прыжки на месте (вики: «hop erratically»).
pub fn physics(mob: &mut Mob, view: &dyn View, rng: &mut Rng) {
    if mob.guardian.wet {
        if let Some((x, y, z)) = mob.guardian.swim_to {
            let (dx, dy, dz) = (x - mob.x, y - mob.y, z - mob.z);
            let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-6);

            mob.face((-dx.atan2(dz).to_degrees()) as f32);
            mob.vx += dx / length * SWIM_SPEED * 0.1;
            mob.vy += dy / length * SWIM_SPEED * 0.1;
            mob.vz += dz / length * SWIM_SPEED * 0.1;
        }

        mob.vitals.fall = 0.0;
        mob.step(view, mob.vx, mob.vy, mob.vz);
        mob.vx *= 0.9;
        mob.vy *= 0.9;
        mob.vz *= 0.9;

        return;
    }

    // На земле — подскок в случайную сторону и звук «flop» [проверить: сила].
    if mob.on_ground && mob.guardian.target.is_none() {
        mob.vx += (rng.unit() * 2.0 - 1.0) * 0.4;
        mob.vy += 0.5;
        mob.vz += (rng.unit() * 2.0 - 1.0) * 0.4;
        mob.face((rng.unit() * 360.0) as f32);
        mob.on_ground = false;

        let name = if mob.kind == Kind::ElderGuardian { "minecraft:entity.elder_guardian.flop" } else { "minecraft:entity.guardian.flop" };

        mob.events.push(super::Event::Sound { name, volume: mob.sound_volume(), pitch: mob.voice_pitch() });
    }

    mob.physics(view, 0.0, false);
}

/// Добыча (вики, Guardian и Elder Guardian, «Drops»): осколки призмарина 0–2,
/// у стража кристаллы 0–1 (40 %) или треска (40 %), у древнего ещё мокрая
/// губка (убил игрок) и шаблон отделки «Прилив» (20 %). «Добыча» прибавляет
/// к верхней границе [проверить: шансы кристаллов и рыбы].
pub fn loot(mob: &Mob, rng: &mut Rng, give: &mut dyn FnMut(&str, i32, i32, &mut Rng)) {
    give("prismarine_shard", 0, 2 + mob.looting, rng);

    let roll = rng.unit();

    if mob.kind == Kind::ElderGuardian {
        if roll < 1.0 / 3.0 {
            give("prismarine_crystals", 1, 1 + mob.looting, rng);
        } else if roll < 1.0 / 3.0 + 0.5 {
            give("cod", 1, 1 + mob.looting, rng);
        }

        if mob.by_player {
            give("wet_sponge", 1, 1, rng);
        }

        if rng.unit() < 0.2 {
            give("tide_armor_trim_smithing_template", 1, 1, rng);
        }
    } else if roll < 0.4 {
        give("prismarine_crystals", 1, 1 + mob.looting, rng);
    } else if roll < 0.8 {
        give("cod", 1, 1 + mob.looting, rng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Pool;

    impl View for Pool {
        fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
            if y < 10 { crate::blocks::state_by_name("water").unwrap_or(0) } else { crate::world::AIR }
        }

        fn loaded(&self, _chunk_x: i32, _chunk_z: i32) -> bool {
            true
        }
    }

    fn watcher(id: i32, at: (f64, f64, f64)) -> ai::Watcher {
        ai::Watcher { id, x: at.0, y: at.1, z: at.2, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false }
    }

    /// Луч бьёт на 80-м такте после захвата (свойство 17 видно на такт позже:
    /// guardian-beam, 2730 → 2809), цель снимается на следующем.
    #[test]
    fn beam_fires_79_ticks_after_lock() {
        let mut mob = Mob::new(5, Kind::Guardian, (0.5, 5.0, 0.5));
        let players = [watcher(1, (0.5, 5.0, 5.5))];
        let ctx = Ctx { players: &players, mates: &[], difficulty: 2, sky: 15, sun_up: true };
        let mut rng = Rng::new(3);

        sense(&mut mob, &Pool);

        let mut locked = None;
        let mut fired = None;

        for tick in 0..400 {
            let plan = think(&mut mob, &Pool, &ctx, &mut rng);

            if locked.is_none() && mob.guardian.target.is_some() {
                locked = Some(tick);
            }

            if let Some(melee) = plan.melee {
                assert_eq!(melee.damage, DAMAGE_GUARDIAN);
                fired = Some(tick);
                break;
            }
        }

        assert_eq!(fired.unwrap() - locked.unwrap(), BEAM_GUARDIAN);
        assert!(!spikes_out(&mob) || mob.guardian.target.is_some());

        for _ in 0..AFTER_SHOT {
            think(&mut mob, &Pool, &ctx, &mut rng);
        }

        assert_eq!(mob.guardian.target, None);
        assert!(mob.guardian.moving);
    }

    /// Проклятие: раз в 1200 тактов, игрокам в 50 блоках.
    #[test]
    fn elder_curses_every_minute() {
        let mut mob = Mob::new(7, Kind::ElderGuardian, (0.5, 5.0, 0.5));
        let players = [watcher(1, (0.5, 20.0, 40.5)), watcher(2, (0.5, 20.0, 80.5))];
        let ctx = Ctx { players: &players, mates: &[], difficulty: 2, sky: 15, sun_up: true };
        let mut rng = Rng::new(3);
        let mut curses = Vec::new();

        for _ in 0..2500 {
            mob.ticks += 1;
            think(&mut mob, &Pool, &ctx, &mut rng);

            if !mob.guardian.cursed.is_empty() {
                curses.push((mob.ticks, std::mem::take(&mut mob.guardian.cursed)));
            }
        }

        assert_eq!(curses.len(), 2);
        assert_eq!(curses[1].0 - curses[0].0, CURSE_EVERY);
        assert_eq!(curses[0].1, vec![1]);
    }

    /// Голоса суши и воды.
    #[test]
    fn land_and_water_voices() {
        let mut mob = Mob::new(5, Kind::Guardian, (0.5, 50.0, 0.5));

        sense(&mut mob, &Pool);
        assert_eq!(sounds(&mob).1, "minecraft:entity.guardian.hurt_land");

        mob.y = 3.0;
        sense(&mut mob, &Pool);
        assert_eq!(sounds(&mob).1, "minecraft:entity.guardian.hurt");
    }
}
