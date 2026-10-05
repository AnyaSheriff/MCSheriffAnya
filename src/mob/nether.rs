// Мобы Незера: ифрит, скелет-иссушитель, магмовый куб, гаст, зомби-пиглин,
// пиглин, брут пиглинов, хоглин, страйдер.
//
// Кода игры здесь нет: правила — со страниц minecraft.wiki (Blaze, Wither
// Skeleton, Magma Cube, Ghast, Zombified Piglin, Piglin, Piglin Brute, Hoglin,
// Strider, Fireball, Small Fireball, Bartering), числа, которых там нет, сняты
// с официального сервера чёрным ящиком (сценарии blaze-*, ghast-*, …-stand).
// Что не сверено, помечено «[проверить]».
//
// - ифрит: парит (падает вдвое медленнее и подскакивает, если цель выше),
//   на расстоянии стреляет «заряд — три малых шара через 6 тактов — пауза 100»,
//   вплотную бьёт сам (вики, Blaze);
// - гаст: плывёт к случайным точкам, при цели в 64 блоках и в прямой
//   видимости копит 20 тактов и пускает шар силой 1, потом −40 (вики, Ghast);
// - зомби-пиглин: нейтрален, пока его не ударили, тогда злится на 20–39 с
//   вместе со всеми зомби-пиглинами в 32 блоках (вики, Zombified Piglin);
// - пиглин: не трогает игроков в золотой броне, прочих бьёт (вики, Piglin);
//   золотой слиток, брошенный рядом, он берёт, 6 с рассматривает и выбрасывает
//   добычу из таблицы обмена (вики, Bartering);
// - брут и хоглин нападают на всякого; страйдер ходит по лаве.

use super::ai::{Ctx, Plan, Watcher};
use super::hostile;
use super::{Event, Kind, Mob, Rng, View};
use crate::blocks;
use crate::inventory::Stack;

/// Огненный шар: откуда, куда лететь (не единичное направление) и каков он:
/// малый ифрита (поджигает) или большой гаста (взрыв).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub from: (f64, f64, f64),
    pub dir: (f64, f64, f64),
    pub small: bool,
    pub power: i32,
}

/// Состояние мобов Незера.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Размер магмового куба: 1, 2 или 4.
    pub size: i32,

    /// Сколько тактов ещё злится зомби-пиглин (0 — спокоен).
    pub anger: i32,

    /// Только что ударили: стая узнает об этом в этом же такте.
    pub alert: bool,

    /// Ифрит и гаст: счётчик выстрела и шаг очереди.
    pub charge: i32,
    pub step: i32,

    /// Заряжен: ифрит горит (свойство 16 бит 1), гаст «заряжает» (свойство 16).
    pub charging: bool,

    /// Куда плывёт гаст.
    pub wander: Option<(f64, f64, f64)>,
    pub wander_wait: i32,

    /// Магмовый куб: тактов до прыжка.
    pub hop: i32,

    /// Что держит пиглин (рассматривает золото или добыча обмена) и сколько
    /// тактов ещё рассматривает; кому выбросит.
    pub held: Vec<Stack>,
    pub admiring: i32,

    /// Рассматривание кончилось: пора выбросить добычу обмена (tick.rs).
    pub barter_done: bool,

    /// Страйдер: мёрзнет ли вне лавы; седло.
    pub cold: bool,
    pub saddled: bool,

    /// Не превращается в зомби-пиглина вне Незера (`IsImmuneToZombification`).
    /// [проверить] превращение вне Незера, охота пиглинов на хоглинов и страх
    /// хоглина перед искажённым грибом не сделаны.
    pub immune: bool,
}

/// Радиус, в котором зомби-пиглины зовут друг друга (вики, Zombified Piglin).
pub const CALL_RANGE: f64 = 32.0;

/// Гнев зомби-пиглина: 20–39 секунд (вики, Zombified Piglin).
pub fn anger_ticks(rng: &mut Rng) -> i32 {
    400 + rng.below(380) as i32
}

/// Размер нового куба при естественном появлении: 1, 2 или 4 поровну
/// (вики, Magma Cube, «Spawning»).
pub fn roll_size(rng: &mut Rng) -> i32 {
    [1, 2, 4][rng.below(3) as usize]
}

/// Ставит размер куба: здоровье равно квадрату размера (вики, Magma Cube).
pub fn set_size(mob: &mut Mob, size: i32) {
    mob.nether.size = size;
    mob.vitals.max_health = (size * size) as f32;
    mob.vitals.health = mob.vitals.max_health;
}

/// Урон куба: по размеру 3, 4, 6 при обычной сложности (вики, Magma Cube).
pub fn cube_damage(size: i32) -> f32 {
    match size {
        1 => 3.0,
        2 => 4.0,
        _ => 6.0,
    }
}

/// Броня куба: 3 за размер (вики, Magma Cube, «Attributes»).
pub fn cube_armor(size: i32) -> f32 {
    (3 * size) as f32
}

/// Дальность слежения (вики, страницы мобов: ифрит 48, гаст 100).
pub fn follow_range(kind: Kind) -> f64 {
    match kind {
        Kind::Blaze => 48.0,
        Kind::Ghast => 100.0,
        Kind::ZombifiedPiglin => 35.0,
        Kind::MagmaCube => 16.0,
        _ => 16.0,
    }
}

/// Нейтрален ли этот игрок для моба: не цель, пока не ударил (вики).
pub fn spares(mob: &Mob, watcher: &Watcher) -> bool {
    match mob.kind {
        Kind::ZombifiedPiglin => mob.nether.anger == 0,
        Kind::Piglin => !mob.h.provoked && watcher.gold_armor,
        _ => false,
    }
}

/// Урон мобов Незера в ближнем бою при обычной сложности (вики, «Attack
/// damage»): иссушитель 5, зомби-пиглин 5, пиглин 5, брут 7, хоглин 6, ифрит 6.
pub fn melee_damage(mob: &Mob) -> f32 {
    match mob.kind {
        Kind::WitherSkeleton | Kind::ZombifiedPiglin | Kind::Piglin => 5.0,
        Kind::PiglinBrute => 7.0,
        Kind::Hoglin => 6.0,
        Kind::Blaze => 6.0,
        Kind::MagmaCube => cube_damage(mob.nether.size),
        _ => 3.0,
    }
}

/// Мысль мобов Незера за такт: цель выбирает общая охота (hostile.rs),
/// дальше — своё.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, mut plan: Plan) -> Plan {
    // Гнев убывает; злой зомби-пиглин не видит других целей.
    if mob.nether.anger > 0 {
        mob.nether.anger -= 1;

        if mob.nether.anger == 0 {
            mob.h.target = None;
            mob.brain.stop();
        }
    }

    if mob.nether.admiring > 0 {
        mob.nether.admiring -= 1;

        if mob.nether.admiring == 0 && !mob.nether.held.is_empty() {
            mob.nether.barter_done = true;
        }
    }

    // Рассматривающий золото пиглин стоит и смотрит (вики, Bartering).
    if mob.kind == Kind::Piglin && mob.nether.admiring > 0 {
        mob.brain.stop();
        return plan;
    }

    let here = mob.cell();
    let in_water = super::is_water(view.block(here.0, here.1, here.2));
    let grounded = mob.on_ground || in_water;
    let target = hostile::choose_target(mob, view, ctx, rng);

    let Some(target) = target else {
        mob.nether.charge = (mob.nether.charge - 1).max(if mob.kind == Kind::Ghast { -40 } else { 0 });
        mob.nether.charging = false;

        return match mob.kind {
            Kind::Ghast => float(mob, view, rng, plan),
            _ => hostile::idle_walk(mob, view, ctx, rng, plan, grounded || mob.kind == Kind::Blaze),
        };
    };

    let (dx, dz) = (target.x - mob.x, target.z - mob.z);
    let flat = (dx * dx + dz * dz).sqrt();
    let dist_sq = (target.x - mob.x).powi(2) + (target.y - mob.y).powi(2) + (target.z - mob.z).powi(2);
    let sight = hostile::sees(view, (mob.x, mob.y + mob.eye(), mob.z), (target.x, target.y + target.eye, target.z));

    mob.h.seen = if sight { 0 } else { mob.h.seen + 1 };

    match mob.kind {
        Kind::Blaze => blaze(mob, view, &target, &mut plan, dist_sq, sight, flat, rng, grounded),
        Kind::Ghast => ghast(mob, view, &target, &mut plan, dist_sq, sight, rng),
        Kind::MagmaCube => cube(mob, view, &target, &mut plan, dist_sq, flat, rng),
        _ => hostile::melee(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
    }

    plan
}

/// Ифрит: вплотную бьёт, дальше — очередь малых шаров (вики, Blaze).
#[allow(clippy::too_many_arguments)]
fn blaze(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, flat: f64, rng: &mut Rng, grounded: bool) {
    mob.nether.charge -= 1;

    ai_look(mob, target);

    if dist_sq < 4.0 {
        if mob.nether.charge <= 0 {
            mob.nether.charge = 20;
            mob.events.push(Event::Swing);

            let (ux, uz) = if flat > 1.0e-6 { ((target.x - mob.x) / flat, (target.z - mob.z) / flat) } else { (0.0, 1.0) };

            plan.melee = Some(hostile::Melee { target: target.id, damage: melee_damage(mob), push: (ux, uz), wither: false });
        }

        *plan = super::ai::walk(mob, std::mem::take(plan));
        return;
    }

    let range = follow_range(Kind::Blaze);

    if dist_sq < range * range && sight {
        if mob.nether.charge <= 0 {
            mob.nether.step += 1;

            match mob.nether.step {
                1 => {
                    mob.nether.charge = 60;
                    mob.nether.charging = true;
                }
                2..=4 => mob.nether.charge = 6,
                _ => {
                    mob.nether.charge = 100;
                    mob.nether.step = 0;
                    mob.nether.charging = false;
                }
            }

            if mob.nether.step > 1 {
                let from = (mob.x, mob.y + mob.size().1 * 0.5 + 0.5, mob.z);
                let spread = dist_sq.sqrt().sqrt() * 0.5;
                let mut gauss = || {
                    let (a, b) = (rng.unit().max(1.0e-12), rng.unit());

                    (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
                };
                let dir = (target.x - mob.x + gauss() * spread, target.y + target.eye * 0.5 - (mob.y + mob.size().1 * 0.5), target.z - mob.z + gauss() * spread);

                mob.events.push(Event::Swing);
                plan.launch = Some(Launch { from, dir, small: true, power: 0 });
            }
        }

        // Ифрит держится на месте и стреляет; цель далеко — подходит.
        if dist_sq > 36.0 && grounded {
            hostile::approach(mob, view, target, 1.0);
            *plan = super::ai::walk(mob, std::mem::take(plan));
        }

        return;
    }

    if mob.h.seen < 5 && grounded {
        hostile::approach(mob, view, target, 1.0);
    }

    *plan = super::ai::walk(mob, std::mem::take(plan));
}

fn ai_look(mob: &mut Mob, target: &Watcher) {
    super::ai::look_at_point(mob, target.x, target.y + target.eye, target.z);
}

/// Гаст копит выстрел, пока цель видна в 64 блоках (вики, Ghast).
fn ghast(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, rng: &mut Rng) {
    ai_look(mob, target);

    // Тело поворачивается к цели: шар вылетает по взгляду.
    let want = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;

    mob.yaw = super::ai::turn(mob.yaw, want, 20.0);

    if dist_sq < 64.0 * 64.0 && sight {
        mob.nether.charge += 1;

        if mob.nether.charge == 10 {
            mob.events.push(Event::Sound { name: "minecraft:entity.ghast.warn", volume: 10.0, pitch: 1.0 });
        }

        if mob.nether.charge == 20 {
            let look = f64::from(mob.yaw).to_radians();
            let (vx, vz) = (-look.sin(), look.cos());
            let from = (mob.x + vx * 4.0, mob.y + mob.size().1 * 0.5 + 0.5, mob.z + vz * 4.0);
            let dir = (target.x - from.0, target.y + target.eye * 0.5 - (mob.y + mob.size().1 * 0.5 + 0.5), target.z - from.2);

            mob.events.push(Event::Sound { name: "minecraft:entity.ghast.shoot", volume: 10.0, pitch: 1.0 });
            plan.launch = Some(Launch { from, dir, small: false, power: 1 });
            mob.nether.charge = -40;
        }
    } else if mob.nether.charge > 0 {
        mob.nether.charge -= 1;
    }

    mob.nether.charging = mob.nether.charge > 10;

    // Плывёт, как без цели.
    *plan = float(mob, view, rng, std::mem::take(plan));
}

/// Блуждание гаста: точка в 16 блоках, к ней разгон 0,1 за полёт в 5–6 тактов.
fn float(mob: &mut Mob, view: &dyn View, rng: &mut Rng, plan: Plan) -> Plan {
    if mob.nether.wander_wait > 0 {
        mob.nether.wander_wait -= 1;
    }

    let near = mob.nether.wander.is_none_or(|(x, y, z)| (x - mob.x).powi(2) + (y - mob.y).powi(2) + (z - mob.z).powi(2) < 4.0);

    if near || mob.nether.wander_wait == 0 && rng.below(40) == 0 {
        let to = (mob.x + (rng.unit() * 2.0 - 1.0) * 16.0, mob.y + (rng.unit() * 2.0 - 1.0) * 16.0, mob.z + (rng.unit() * 2.0 - 1.0) * 16.0);

        // Путь свободен: по лучу нет твёрдого (вики, Ghast: «random point … if unobstructed»).
        if hostile::sees(view, (mob.x, mob.y + 2.0, mob.z), (to.0, to.1 + 2.0, to.2)) {
            mob.nether.wander = Some(to);
            mob.nether.wander_wait = 60;
        }
    }

    plan
}

/// Магмовый куб: прыгает к цели, вплотную кусает (вики, Magma Cube).
fn cube(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, _flat: f64, rng: &mut Rng) {
    let _ = view;

    ai_look(mob, target);

    let size = f64::from(mob.nether.size.max(1));
    let reach = 0.6 * size;

    mob.nether.hop -= 1;

    // Укус: касание ящиками, раз в 20 тактов, только на земле (вики, Magma Cube:
    // «attacks by jumping into the player»).
    if dist_sq <= (reach + 0.3).powi(2) + 1.0 && mob.h.cooldown == 0 {
        mob.h.cooldown = hostile::MELEE_COOLDOWN;

        let (dx, dz) = (target.x - mob.x, target.z - mob.z);
        let flat = (dx * dx + dz * dz).sqrt().max(1.0e-6);

        plan.melee = Some(hostile::Melee { target: target.id, damage: cube_damage(mob.nether.size), push: (dx / flat, dz / flat), wither: false });
    }

    if mob.on_ground && mob.nether.hop <= 0 {
        // Прыжок: 0,42 + 0,1 на единицу размера [проверить], вперёд 0,2+0,1·размер.
        mob.nether.hop = 10 + rng.below(21) as i32;

        let (dx, dz) = (target.x - mob.x, target.z - mob.z);
        let flat = (dx * dx + dz * dz).sqrt().max(1.0e-6);
        let speed = 0.2 + 0.1 * size;

        mob.vy = 0.42 + 0.1 * size.min(2.0);
        mob.vx += dx / flat * speed;
        mob.vz += dz / flat * speed;
        mob.on_ground = false;

        mob.events.push(Event::Sound { name: "minecraft:entity.magma_cube.jump", volume: 1.0, pitch: 1.0 });
    }
}

/// Куда плывёт летун в этом такте: разгон к точке и торможение (гаст).
pub fn drift(mob: &mut Mob, view: &dyn View) {
    if let Some((x, y, z)) = mob.nether.wander {
        let (dx, dy, dz) = (x - mob.x, y - mob.y, z - mob.z);
        let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-6);

        mob.vx += dx / length * 0.02;
        mob.vy += dy / length * 0.02;
        mob.vz += dz / length * 0.02;
    }

    mob.step(view, mob.vx, mob.vy, mob.vz);

    mob.vx *= 0.91;
    mob.vy *= 0.91;
    mob.vz *= 0.91;

    if mob.on_ground {
        mob.vy = 0.0;
    }
}

/// Страйдер за такт: мёрзнет и дрожит вне лавы, в лаве согревается (вики,
/// Strider: «shivers … when not in lava»); в воде получает урон.
pub fn strider_tick(mob: &mut Mob, view: &dyn View) {
    let (x, y, z) = mob.cell();
    let in_lava = super::is_lava(view.block(x, y, z)) || super::is_lava(view.block(x, y - 1, z));

    mob.nether.cold = !in_lava;
}

/// Подъём ифрита: падает медленнее, цель выше — подскакивает (вики, Blaze).
pub fn blaze_lift(mob: &mut Mob, target_eye: Option<f64>) {
    if !mob.on_ground && mob.vy < 0.0 {
        mob.vy *= 0.6;
    }

    if let Some(eye) = target_eye
        && eye > mob.y + mob.eye()
        && mob.vy < 0.3
    {
        mob.vy += (0.3 - mob.vy) * 0.3;
        mob.on_ground = false;
    }
}

/// Стопка предмета по имени.
fn stack(name: &str, count: i32) -> Option<Stack> {
    blocks::item_named(name).map(|item| Stack::new(item, count))
}

/// Таблица обмена с пиглином: (вес, предмет, наименьшее и наибольшее число)
/// (вики, Bartering: общий вес 459). Зачарованные ботинки и книга, зелья —
/// простыми предметами [проверить: довески].
pub const BARTER: [(u32, &str, i32, i32); 17] = [
    (5, "enchanted_book", 1, 1),
    (8, "iron_boots", 1, 1),
    (8, "splash_potion", 1, 1),
    (8, "potion", 1, 1),
    (10, "potion", 1, 1),
    (10, "iron_nugget", 10, 36),
    (10, "ender_pearl", 4, 8),
    (20, "string", 3, 9),
    (20, "quartz", 5, 12),
    (40, "obsidian", 1, 1),
    (40, "crying_obsidian", 1, 3),
    (40, "fire_charge", 1, 5),
    (40, "leather", 2, 4),
    (40, "soul_sand", 2, 8),
    (40, "nether_brick", 2, 8),
    (40, "spectral_arrow", 6, 12),
    (40, "gravel", 8, 16),
];

/// Добыча одного обмена: взвешенный выбор, число в пределах.
pub fn barter_loot(rng: &mut Rng) -> Vec<Stack> {
    // Вики: гравий и чёрный камень по 40 каждый — последние две строки.
    let table: Vec<(u32, &str, i32, i32)> = BARTER.iter().copied().chain([(40, "blackstone", 8, 16)]).collect();
    let total: u32 = table.iter().map(|row| row.0).sum();
    let mut roll = rng.below(total);

    for (weight, name, low, high) in table {
        if roll < weight {
            let count = low + rng.below((high - low + 1) as u32) as i32;

            return stack(name, count).into_iter().collect();
        }

        roll -= weight;
    }

    Vec::new()
}

/// Сколько тактов пиглин рассматривает золото перед выбросом: 6 с (вики, Bartering).
pub const ADMIRE_TICKS: i32 = 120;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_barter_table_weighs_459() {
        let total: u32 = BARTER.iter().map(|row| row.0).sum::<u32>() + 40;

        assert_eq!(total, 459);
    }

    #[test]
    fn a_cube_is_as_healthy_as_its_size_squared() {
        let mut cube = Mob::new(1, Kind::MagmaCube, (0.0, 0.0, 0.0));

        set_size(&mut cube, 4);

        assert_eq!(cube.vitals.health, 16.0);
        assert_eq!(cube.size(), (0.52 * 4.0, 0.52 * 4.0));
    }

    #[test]
    fn anger_lasts_between_twenty_and_thirty_nine_seconds() {
        let mut rng = Rng::new(7);

        assert!((0..200).all(|_| (400..780).contains(&anger_ticks(&mut rng))));
    }

    #[test]
    fn a_piglin_spares_the_gold_clad_unless_provoked() {
        let mut piglin = Mob::new(1, Kind::Piglin, (0.0, 0.0, 0.0));
        let player = Watcher { id: 2, x: 1.0, y: 0.0, z: 1.0, eye: 1.62, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, gold_armor: true };

        assert!(spares(&piglin, &player));

        piglin.h.provoked = true;

        assert!(!spares(&piglin, &player));
    }
}
