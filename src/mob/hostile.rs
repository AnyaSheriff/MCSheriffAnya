// Враждебные мобы: зомби, скелет, крипер, паук.
//
// Кода игры здесь нет: правила — со страниц minecraft.wiki (Zombie, Skeleton,
// Creeper, Spider, Arrow, Mob AI, Daylight cycle), числа, которых там нет, сняты
// с официального сервера чёрным ящиком (сценарии zombie-hit, zombie-burn,
// creeper-explode, skeleton-arrow, spider-climb). Что не сверено, помечено
// «[проверить]».
//
// Цели по старшинству те же, что у животных (ai.rs), но вместо прогулки и
// приманки — охота:
//
// - цель — ближайший игрок в выживании (или приключении) в дальности
//   слежения (зомби 35 блоков, прочие 16) в прямой видимости; творческих и
//   наблюдателей мобы не видят (вики, Mob AI, «Targeting»);
// - зомби и паук идут к цели и бьют раз в 20 тактов, когда она в досягаемости;
// - крипер идёт к цели, ближе 3 блоков зажигает запал на 30 тактов и
//   взрывается силой 3; дальше 7 блоков или без видимости запал гаснет;
// - скелет стоит в 15 блоках и стреляет: 20 тактов натяжки и пауза 40 (20 на
//   трудной), то есть выстрел раз в 3 (2) секунды;
// - паук лезет по стенам, а при свете 12 и выше нападает, только если его
//   ударили (вики, Spider);
// - зомби и скелет горят на солнце (вики, Zombie: «burn when exposed to direct
//   sunlight»), если нет шлема и они не в воде.

use super::ai::{self, Ctx, Plan, Watcher};
use super::path;
use super::{Event, Kind, Mob, Rng, View};

/// Дальность слежения за целью (вики, Zombie: «Follow range 35»; у прочих 16).
pub fn follow_range(kind: Kind) -> f64 {
    if kind == Kind::Zombie { 35.0 } else { 16.0 }
}

/// Запал крипера в тактах (вики, Creeper: «Fuse time 30 ticks»).
pub const FUSE: i32 = 30;

/// Сила взрыва крипера (вики, Creeper: «Explosion power 3»).
pub const CREEPER_POWER: f32 = 3.0;

/// Сколько тактов зомби и паук ждут между ударами.
pub const MELEE_COOLDOWN: i32 = 20;

/// Скорость стрелы скелета, блоков за такт (вики, Arrow: «Skeletons fire
/// arrows at approximately 20 m/s» — 1,0 блока за такт; у игрока 3 м/с
/// умножены на 1,6 у оригинала [проверить: опорное значение 1,6]).
pub const ARROW_SPEED: f64 = 1.6;

/// Натяжка лука до выстрела в тактах.
pub const DRAW_TICKS: i32 = 20;

/// Ждать после выстрела: 40 тактов, на трудной 20 (вики, Skeleton: «every 3
/// seconds (Easy/Normal) or 2 seconds (Hard)» вместе с натяжкой).
pub fn shoot_pause(difficulty: u8) -> i32 {
    if difficulty >= crate::survival::HARD { 20 } else { 40 }
}

/// Разброс стрелы: 14 − 4·сложность (вики, Skeleton: «error values of 10
/// (Easy), 6 (Normal), and 2 (Hard)»).
pub fn arrow_error(difficulty: u8) -> f64 {
    f64::from(14 - 4 * i32::from(difficulty.clamp(1, 3)))
}

/// Ключ имени для сообщения о смерти игрока (вики, Death messages).
pub fn killer_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Zombie => "entity.minecraft.zombie",
        Kind::Skeleton => "entity.minecraft.skeleton",
        Kind::Creeper => "entity.minecraft.creeper",
        Kind::Spider => "entity.minecraft.spider",
        Kind::Cow => "entity.minecraft.cow",
        Kind::Pig => "entity.minecraft.pig",
        Kind::Sheep => "entity.minecraft.sheep",
        Kind::Chicken => "entity.minecraft.chicken",
    }
}

/// Удар в ближнем бою: кому, сколько (до поправки на сложность) и куда толкает.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Melee {
    pub target: i32,
    pub damage: f32,
    pub push: (f64, f64),
}

/// Выстрел скелета: откуда, куда целится (точка на цели) и скорость.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub from: (f64, f64, f64),
    pub aim: (f64, f64, f64),
    pub speed: f64,
}

/// Состояние охоты.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Номер игрока-цели.
    pub target: Option<i32>,

    /// Тактов до следующего удара или выстрела.
    pub cooldown: i32,

    /// Запал крипера: 0…30.
    pub fuse: i32,

    /// Направление запала для клиента: −1 гаснет, 1 растёт.
    pub swell_dir: i32,

    /// Крипер подожжён (огниво или данные): горит, пока не взорвётся.
    pub lit: bool,

    /// Паук лезет по стене.
    pub climbing: bool,

    /// Паука ударили: нападает и при свете.
    pub provoked: bool,

    /// Шлем на голове: не горит на солнце.
    pub helmet: bool,

    /// Лук в руке у скелета: без него он бьёт в ближнем бою, как зомби
    /// (чёрный ящик, skeleton-arrow: призванный с данными скелет без лука
    /// подошёл и бил на 2 [проверить: урон]).
    pub bow: bool,

    /// Скелет натягивает лук: тактов натяжки (0 — нет).
    pub drawing: i32,

    /// Скелет кружит вокруг цели: тактов в одну сторону, по часовой ли и
    /// назад ли (вики, Skeleton: «strafes»; чёрный ящик, skeleton-arrow:
    /// стрелок идёт боком со скоростью ~0,1 за такт) [проверить].
    pub strafe_time: i32,
    pub clockwise: bool,
    pub backwards: bool,

    /// Сколько тактов подряд моб стоит вдали от игроков (для исчезновения).
    pub idle: i32,

    /// Цель видна: счётчик потери видимости.
    pub seen: i32,

    /// Счёт для ambient-звука.
    pub ambient: i32,

    /// Пора убрать тихо: исчез вдали от игроков или взорвался.
    pub despawn: bool,
}

/// Игрок, которого можно атаковать: жив и в выживании.
fn victim(watcher: &Watcher) -> bool {
    watcher.targetable && !watcher.spectator
}

/// Расстояние в квадрате от ног моба до точки.
fn dist2(mob: &Mob, x: f64, y: f64, z: f64) -> f64 {
    (x - mob.x).powi(2) + (y - mob.y).powi(2) + (z - mob.z).powi(2)
}

/// Прямая видимость от глаз моба до глаз игрока: луч шагом в четверть блока
/// не должен входить в твёрдое (вики, Mob AI: «line of sight»).
pub fn sees(view: &dyn View, from: (f64, f64, f64), to: (f64, f64, f64)) -> bool {
    let (dx, dy, dz) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    let steps = (length / 0.25).ceil().max(1.0) as i32;

    for step in 1..steps {
        let t = f64::from(step) / f64::from(steps);
        let (x, y, z) = (from.0 + dx * t, from.1 + dy * t, from.2 + dz * t);

        if super::solid(view.block(x.floor() as i32, y.floor() as i32, z.floor() as i32)) {
            return false;
        }
    }

    true
}

/// Видит ли моб этого игрока глазами.
fn line_of_sight(mob: &Mob, view: &dyn View, watcher: &Watcher) -> bool {
    sees(view, (mob.x, mob.y + mob.eye(), mob.z), (watcher.x, watcher.y + watcher.eye, watcher.z))
}

/// Паук нападает сам, если вокруг темно: свет 11 и ниже (вики, Spider).
fn spider_bold(mob: &Mob, view: &dyn View, ctx: &Ctx) -> bool {
    let (x, y, z) = mob.cell();

    let (sky, block) = view.light(x, y, z, ctx.sky);

    sky.max(block) <= 11
}

/// Подходит ли игрок в цели этому мобу.
fn eligible(mob: &Mob, view: &dyn View, ctx: &Ctx, watcher: &Watcher, need_sight: bool) -> bool {
    if !victim(watcher) {
        return false;
    }

    let range = follow_range(mob.kind);

    if dist2(mob, watcher.x, watcher.y, watcher.z) > range * range {
        return false;
    }

    if mob.kind == Kind::Spider && !mob.h.provoked && !spider_bold(mob, view, ctx) {
        return false;
    }

    !need_sight || line_of_sight(mob, view, watcher)
}

/// Выбор цели раз в 10 тактов; держится, пока игрок годится.
fn choose_target<'a>(mob: &mut Mob, view: &dyn View, ctx: &'a Ctx, rng: &mut Rng) -> Option<&'a Watcher> {
    if let Some(id) = mob.h.target {
        let current = ctx.players.iter().find(|player| player.id == id);

        if let Some(player) = current.filter(|player| eligible(mob, view, ctx, player, false)) {
            return Some(player);
        }

        mob.h.target = None;
    }

    // Выбор цели — с шансом 1 к 10 за такт: у оригинала зомби в 6 блоках от
    // игрока трогается на 21-м такте (чёрный ящик, zombie-attack) [проверить].
    if rng.below(10) != 0 {
        return None;
    }

    let near = ctx
        .players
        .iter()
        .filter(|player| eligible(mob, view, ctx, player, true))
        .min_by(|a, b| dist2(mob, a.x, a.y, a.z).total_cmp(&dist2(mob, b.x, b.y, b.z)));

    mob.h.target = near.map(|player| player.id);
    near
}

/// Мысль враждебного моба за такт.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();

    // Голос в покое: как у животных (вики, Sounds), у крипера его нет.
    mob.h.ambient += 1;

    if rng.below(1000) < mob.h.ambient.max(0) as u32 {
        let (ambient, _, _) = mob.kind.sounds();

        mob.h.ambient = -80;

        if !ambient.is_empty() {
            let pitch = mob.voice_pitch();

            mob.events.push(Event::Sound { name: ambient, volume: 1.0, pitch });
        }
    }

    mob.h.cooldown = (mob.h.cooldown - 1).max(0);

    let here = mob.cell();
    let in_water = super::is_water(view.block(here.0, here.1, here.2));

    // В воздухе путь ведётся, новый не строится (как у животных).
    let grounded = mob.on_ground || in_water || mob.kind == Kind::Spider;
    let target = choose_target(mob, view, ctx, rng).copied();

    // Паук: днём цель забывается с шансом 1/100 за такт (вики, Spider).
    if mob.kind == Kind::Spider && target.is_some() && !spider_bold(mob, view, ctx) && rng.below(100) == 0 {
        mob.h.target = None;
        mob.brain.stop();

        return plan;
    }

    let Some(target) = target else {
        return idle(mob, view, ctx, rng, plan, grounded);
    };

    // Сердитый: руки вперёд (флаг «агрессивен» у мобов).
    let (dx, dz) = (target.x - mob.x, target.z - mob.z);
    let flat = (dx * dx + dz * dz).sqrt();
    let dist_sq = dist2(mob, target.x, target.y, target.z);
    let sight = line_of_sight(mob, view, &target);

    mob.h.seen = if sight { 0 } else { mob.h.seen + 1 };

    match mob.kind {
        Kind::Zombie | Kind::Spider => melee(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
        Kind::Creeper => creeper(mob, view, &target, &mut plan, dist_sq, sight, grounded),
        Kind::Skeleton if mob.h.bow => skeleton(mob, view, ctx, &target, &mut plan, dist_sq, sight, rng, grounded),
        Kind::Skeleton => melee(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
        _ => {}
    }

    plan
}

/// Нет цели: запал крипера гаснет, лук опускается, моб бродит.
fn idle(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, mut plan: Plan, grounded: bool) -> Plan {
    mob.h.drawing = 0;
    mob.strafe = (0.0, 0.0);

    if mob.kind == Kind::Creeper {
        swell(mob, None, &mut plan);
    }

    if !grounded {
        if !mob.brain.walking() {
            return plan;
        }

        return ai::walk(mob, plan);
    }

    // Прогулка: как у животных, раз в 120 тактов.
    if mob.brain.walking() {
        return if mob.h.fuse > 0 { plan } else { ai::walk(mob, plan) };
    }

    if rng.below(120) == 0 {
        for _ in 0..10 {
            let here = mob.cell();
            let target = (here.0 + rng.below(21) as i32 - 10, here.1, here.2 + rng.below(21) as i32 - 10);
            let target = ai::settle(view, target, here.1);

            if !path::standable(view, target) {
                continue;
            }

            let route = path::find(view, here, target, path::MAX_NODES);

            if !route.is_empty() {
                mob.brain.go(route, 0.8);
                break;
            }
        }
    }

    let _ = ctx;
    ai::walk(mob, plan)
}

/// К цели: путь раз в 10 тактов, прямо — если цель рядом; у паука без пути
/// прямо на цель (он лезет по стенам).
fn approach(mob: &mut Mob, view: &dyn View, target: &Watcher, speed: f64) {
    let here = mob.cell();

    mob.brain.repath -= 1;

    if mob.brain.walking() && mob.brain.repath > 0 {
        return;
    }

    mob.brain.repath = 10;

    let goal = (target.x.floor() as i32, (target.y + 1.0e-3).floor() as i32, target.z.floor() as i32);

    // Паук идёт напрямую: путь по земле ему не нужен, стена — не преграда.
    let route = if mob.kind == Kind::Spider { vec![goal] } else { path::find(view, here, goal, path::MAX_NODES) };

    mob.brain.go(route, speed);
}

/// Зомби и паук: подойти и ударить.
#[allow(clippy::too_many_arguments)]
fn melee(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, flat: f64, rng: &mut Rng, grounded: bool) {
    // Досягаемость: (2·ширина)² + ширина цели, от ног до ног (вики, Mob AI,
    // «Melee attack») [проверить: по замеру zombie-hit].
    let (width, _) = mob.size();
    let reach_sq = (width * 2.0).powi(2) + 0.6;

    if dist_sq <= reach_sq {
        mob.brain.stop();
        ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

        if mob.h.cooldown == 0 && line_of_sight(mob, view, target) {
            mob.h.cooldown = MELEE_COOLDOWN;

            let (ux, uz) = if flat > 1.0e-6 { ((target.x - mob.x) / flat, (target.z - mob.z) / flat) } else { (0.0, 1.0) };
            let damage = if matches!(mob.kind, Kind::Spider | Kind::Skeleton) { 2.0 } else { 3.0 };

            mob.events.push(Event::Swing);
            plan.melee = Some(Melee { target: target.id, damage, push: (ux, uz) });
        }

        return;
    }

    // Прыжок паука: на земле, цель в 2–4 блоках (вики, Spider: «leap»).
    if mob.kind == Kind::Spider && mob.on_ground && (4.0..=16.0).contains(&dist_sq) && rng.below(5) == 0 {
        let (ux, uz) = (target.x - mob.x, target.z - mob.z);
        let len = (ux * ux + uz * uz).sqrt().max(1.0e-6);

        mob.vx += ux / len * 0.4 + mob.vx * 0.2;
        mob.vz += uz / len * 0.4 + mob.vz * 0.2;
        mob.vy = 0.4;
        mob.on_ground = false;
    }

    if grounded {
        approach(mob, view, target, 1.0);
    }

    *plan = ai::walk(mob, std::mem::take(plan));
}

/// Запал крипера: растёт рядом с целью, гаснет вдали (вики, Creeper).
pub fn swell(mob: &mut Mob, target: Option<(f64, bool)>, plan: &mut Plan) {
    let near = match target {
        Some((dist_sq, sight)) => dist_sq < 9.0 && sight,
        None => false,
    };
    let keeping = mob.h.fuse > 0 && target.is_some_and(|(dist_sq, sight)| dist_sq <= 49.0 && sight);
    let dir = if mob.h.lit || near || keeping { 1 } else { -1 };

    // Зашипел: звук в начале запала (вики, Creeper, «Sounds»: hiss).
    if dir > 0 && mob.h.fuse == 0 {
        mob.events.push(Event::Sound { name: "minecraft:entity.creeper.primed", volume: 1.0, pitch: 0.5 });
    }

    mob.h.swell_dir = dir;
    mob.h.fuse = (mob.h.fuse + dir).max(0);

    if mob.h.fuse >= FUSE {
        mob.h.fuse = FUSE;
        plan.blast = Some(CREEPER_POWER);
    }
}

/// Крипер: идти к цели, пока запал не пошёл; во время запала стоит.
fn creeper(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, grounded: bool) {
    swell(mob, Some((dist_sq, sight)), plan);
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    if mob.h.swell_dir > 0 {
        mob.brain.stop();
        return;
    }

    if grounded {
        approach(mob, view, target, 1.0);
    }

    *plan = ai::walk(mob, std::mem::take(plan));
}

/// Скелет: стоит, целится и стреляет (вики, Skeleton). Если не видит цель или
/// она дальше 15 блоков — идёт к ней.
#[allow(clippy::too_many_arguments)]
fn skeleton(mob: &mut Mob, view: &dyn View, ctx: &Ctx, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, rng: &mut Rng, grounded: bool) {
    let in_range = dist_sq <= 15.0 * 15.0;

    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    // Тело поворачивается к цели, чтобы лук смотрел на неё.
    let want = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;

    mob.yaw = ai::turn(mob.yaw, want, 30.0);

    if sight && in_range {
        mob.brain.stop();
        mob.h.strafe_time += 1;

        // Раз в 20 тактов: с шансом 0,3 сменить сторону и с шансом 0,3 —
        // вперёд/назад; ближе трёх с лишним блоков — не приближаться, дальше
        // 11 — идти к цели.
        if mob.h.strafe_time >= 20 {
            if rng.unit() < 0.3 {
                mob.h.clockwise = !mob.h.clockwise;
            }

            if rng.unit() < 0.3 {
                mob.h.backwards = !mob.h.backwards;
            }

            mob.h.strafe_time = 0;
        }

        if dist_sq > 15.0 * 15.0 * 0.5625 {
            mob.h.backwards = false;
        } else if dist_sq < 15.0 * 15.0 * 0.0625 {
            mob.h.backwards = true;
        }

        mob.strafe = (if mob.h.backwards { -0.5 } else { 0.5 }, if mob.h.clockwise { 0.5 } else { -0.5 });
    } else if grounded {
        mob.strafe = (0.0, 0.0);
        approach(mob, view, target, 1.0);
        *plan = ai::walk(mob, std::mem::take(plan));
    }

    if mob.h.drawing > 0 {
        if !sight && mob.h.seen > 60 {
            mob.h.drawing = 0;
            return;
        }

        mob.h.drawing += 1;

        if mob.h.drawing > DRAW_TICKS && sight {
            mob.h.drawing = 0;
            mob.h.cooldown = shoot_pause(ctx.difficulty);
            mob.events.push(Event::Swing);
            plan.shot = Some(Shot {
                from: (mob.x, mob.y + mob.eye() - 0.1, mob.z),
                aim: (target.x, target.y + 0.6, target.z),
                speed: ARROW_SPEED,
            });
        }
    } else if mob.h.cooldown == 0 && sight && in_range {
        mob.h.drawing = 1;
    }
}

/// Горение на солнце: зомби и скелет без шлема под открытым небом днём
/// загораются, в воде гаснут. Огонь — раз в 20 тактов на 1 (вики, Fire).
/// Возвращает true, если пора нанести урон огнём.
pub fn burn(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, sun: bool) -> bool {
    let (x, y, z) = mob.cell();

    if super::is_water(view.block(x, y, z)) {
        mob.vitals.extinguish();
        return false;
    }

    let burns = matches!(mob.kind, Kind::Zombie | Kind::Skeleton) && !mob.h.helmet;

    if sun && burns && ctx.sun_up && view.open_sky(x, y, z) && rng.below(SUN_CHANCE) == 0 {
        mob.vitals.ignite(SUN_FIRE_TICKS);
    }

    mob.vitals.fire_tick(false)
}

/// Шанс загореться на солнце за такт — 1 к 50: у оригинала между зажиганиями
/// проходит в среднем 30–50 тактов (чёрный ящик, zombie-burn: первый удар огня
/// на тактах 9 и 35, сдвиги фазы раз в 3–4 удара) [проверить].
pub const SUN_CHANCE: u32 = 50;

/// Сколько тактов горит поджжённый солнцем: 8 секунд (вики, Zombie).
pub const SUN_FIRE_TICKS: i32 = 160;

/// Исчезновение вдали от игроков (вики, Mob spawning, «Despawning»): дальше
/// 128 блоков от ближайшего игрока — сразу; дальше 32 — после 30 секунд
/// (600 тактов) вдали, с шансом 1/800 за такт; ближе — счёт сбрасывается.
pub fn despawn_check(mob: &mut Mob, players: &[Watcher], rng: &mut Rng, peaceful: bool) {
    // Мирная сложность: враждебные исчезают сразу (вики, Peaceful).
    if peaceful {
        mob.h.despawn = true;
        return;
    }

    // Без игроков в мире проверки нет: мобы стоят, как и всё вокруг.
    if players.is_empty() {
        return;
    }

    let near = players
        .iter()
        .map(|player| (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2))
        .fold(f64::INFINITY, f64::min);

    if near > 128.0 * 128.0 {
        mob.h.despawn = true;
    } else if near > 32.0 * 32.0 {
        mob.h.idle += 1;

        if mob.h.idle > 600 && rng.below(800) == 0 {
            mob.h.despawn = true;
        }
    } else {
        mob.h.idle = 0;
    }
}
