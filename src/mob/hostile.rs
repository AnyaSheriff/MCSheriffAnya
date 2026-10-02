// Враждебные мобы: зомби, скелет, крипер, паук, эндермен, шалкер, ведьма.
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
    match kind {
        Kind::Zombie => 35.0,
        // Вики, Enderman: злится на взгляд с 64 блоков и гонится за целью там же.
        Kind::Enderman => 64.0,
        _ => 16.0,
    }
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
        Kind::EndCrystal => "entity.minecraft.end_crystal",
        Kind::EnderDragon => "entity.minecraft.ender_dragon",
        Kind::Enderman => "entity.minecraft.enderman",
        Kind::Shulker => "entity.minecraft.shulker",
        Kind::ShulkerBullet => "entity.minecraft.shulker_bullet",
        Kind::Witch => "entity.minecraft.witch",
        Kind::Villager => "entity.minecraft.villager",
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

    /// Эндермен: блок в руках (состояние), смотрит ли на него игрок сейчас
    /// и сколько тактов подряд, пора ли прыгнуть прочь (удар, вода), тактов
    /// до прыжка к цели.
    pub carried: Option<i32>,
    pub stared_at: bool,
    pub stare: i32,
    pub want_teleport: bool,
    pub chase_timer: i32,

    /// Шалкер: грань крепления (Direction: 0 низ, 1 верх, 2 север, 3 юг,
    /// 4 запад, 5 восток), приоткрытость 0…100, сколько тактов ещё открыт,
    /// цвет (16 — без цвета).
    pub face: u8,
    pub peek: u8,
    pub peek_timer: i32,
    pub shell_color: u8,

    /// Шалкер закрылся, побыв открытым: модификатор «covered» +20 к броне.
    /// Призванный закрытым его не носит (чёрный ящик, shulker-stand и
    /// shulker-ai: первый пакет атрибутов — без брони, «covered» приходит с
    /// первым закрытием).
    pub covered: bool,

    /// Снаряд шалкера: кто выпустил.
    pub owner: i32,

    /// Ведьма: сколько тактов ещё пьёт и какое зелье (номер в `POTIONS`).
    pub drinking: i32,
    pub potion: u16,
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

    // Эндермен нейтрален: нападает, только если его разозлили (взгляд, удар).
    if mob.kind == Kind::Enderman && !mob.h.provoked {
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

    // Свои такты до охоты: эндермен (вода, взгляд, блоки, прыжки), шалкер
    // (весь его ход), ведьма (зелья).
    match mob.kind {
        Kind::Enderman => {
            if enderman_pre(mob, view, ctx, rng, &mut plan) {
                return plan;
            }
        }
        Kind::Shulker => return shulker(mob, view, ctx, rng),
        _ => {}
    }

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
        Kind::Enderman => enderman_chase(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
        Kind::Witch => witch_keep(mob, view, &target, &mut plan, dist_sq, grounded),
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
            // Вики: паук и скелет 2, зомби 3, эндермен 7 (на нормальной).
            let damage = match mob.kind {
                Kind::Spider | Kind::Skeleton => 2.0,
                Kind::Enderman => 7.0,
                _ => 3.0,
            };

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

    // Вода и дождь тушат (вики, Weather: «Extinguishes fires»).
    if super::is_water(view.block(x, y, z)) || in_rain(mob, view) {
        mob.vitals.extinguish();
        return false;
    }

    let burns = matches!(mob.kind, Kind::Zombie | Kind::Skeleton) && !mob.h.helmet;

    if sun && burns && ctx.sun_up && view.open_sky(x, y, z) && rng.below(SUN_CHANCE) == 0 {
        mob.vitals.ignite(SUN_FIRE_TICKS);
    }

    mob.vitals.fire_tick(false)
}

/// Под дождём ли моб: в клетке ног или макушки [проверить]: какие клетки
/// смотрит оригинал.
pub fn in_rain(mob: &Mob, view: &dyn View) -> bool {
    let (x, y, z) = mob.cell();
    let top = (mob.y + mob.kind.height()).floor() as i32;

    view.rain(x, y, z) || view.rain(x, top, z)
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

// ---------------------------------------------------------------------------
// Эндермен
// ---------------------------------------------------------------------------
//
// Вики (Enderman): нейтрален, злится на взгляд в голову с 64 блоков дольше
// 5 тактов (тыква на голове спасает) и на удар; от любого урона, от воды и от
// снарядов прыгает прочь — место выбирается в кубе ±32 блока; раз в такт с
// шансом 1/20 пробует взять блок из «съедобных» в области 4×3×4 вокруг, с
// шансом 1/2000 ставит его на целый пол рядом. Под взглядом цели ближе 16
// блоков замирает. Дождя у нас нет — от него не прыгает [проверить].

/// Высота глаз эндермена (minecraft-data, entities.json: рост 2,9) [проверить: 2,55].
pub const ENDERMAN_EYE: f64 = 2.55;

/// На каком такте взгляда эндермен злится: у оригинала 17=true приходит на
/// третьем такте после 18=true (чёрный ящик, enderman-stare; вики, Enderman:
/// «for 5 game ticks» — считая такт, когда взгляд замечен, и такт рассылки).
pub const STARE_TICKS: i32 = 4;

/// Размах прыжка эндермена по каждой оси (вики: «32 blocks along each axis»).
pub const TELEPORT_SPREAD: f64 = 32.0;

/// Блоки, что эндермен берёт в руки (вики, Enderman, «Picking up blocks»).
pub fn holdable(state: i32) -> bool {
    matches!(
        crate::blocks::block_at_state(state),
        Some(
            "grass_block"
                | "dirt"
                | "coarse_dirt"
                | "podzol"
                | "rooted_dirt"
                | "mycelium"
                | "sand"
                | "red_sand"
                | "gravel"
                | "clay"
                | "pumpkin"
                | "carved_pumpkin"
                | "melon"
                | "tnt"
                | "cactus"
                | "dandelion"
                | "poppy"
                | "blue_orchid"
                | "allium"
                | "azure_bluet"
                | "red_tulip"
                | "orange_tulip"
                | "white_tulip"
                | "pink_tulip"
                | "oxeye_daisy"
                | "cornflower"
                | "lily_of_the_valley"
                | "wither_rose"
                | "torchflower"
                | "brown_mushroom"
                | "red_mushroom"
                | "crimson_fungus"
                | "warped_fungus"
                | "crimson_nylium"
                | "warped_nylium"
                | "crimson_roots"
                | "warped_roots"
                | "moss_block"
                | "mud"
                | "muddy_mangrove_roots"
        )
    )
}

/// Смотрит ли игрок в голову эндермена: луч взгляда входит в ящик головы —
/// верхние полблока, полблока в ширину (вики: «Looking anywhere at the head
/// aggravates it, including the back») [проверить: размер ящика].
pub fn looks_at_head(mob: &Mob, watcher: &Watcher) -> bool {
    let (yaw, pitch) = (f64::from(watcher.look.0).to_radians(), f64::from(watcher.look.1).to_radians());
    let dir = (-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());
    let from = (watcher.x, watcher.y + watcher.eye, watcher.z);
    let (_, height) = mob.size();
    let low = [mob.x - 0.25, mob.y + height - 0.5, mob.z - 0.25];
    let high = [mob.x + 0.25, mob.y + height, mob.z + 0.25];
    let origin = [from.0, from.1, from.2];
    let ray = [dir.0, dir.1, dir.2];
    let (mut enter, mut leave) = (0.0f64, 64.0f64);

    for axis in 0..3 {
        if ray[axis].abs() < 1.0e-9 {
            if origin[axis] < low[axis] || origin[axis] > high[axis] {
                return false;
            }

            continue;
        }

        let (a, b) = ((low[axis] - origin[axis]) / ray[axis], (high[axis] - origin[axis]) / ray[axis]);

        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }

    enter <= leave
}

/// Такт эндермена до охоты: вода, прыжок прочь, взгляд, блоки. true — ход
/// окончен (прыгнул).
fn enderman_pre(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, plan: &mut Plan) -> bool {
    let (x, y, z) = mob.cell();

    // Вода и дождь жгут: 1 урона (вики: «take 1HP damage»), и он прыгает
    // прочь («In rain, endermen continuously teleport until they find a dry
    // spot»); вид урона — утопление [проверить].
    if super::is_water(view.block(x, y, z)) || super::is_water(view.block(x, y + 1, z)) || in_rain(mob, view) {
        mob.hurt(1.0, crate::survival::Source::Drown, None);
    }

    if mob.is_dead() {
        return true;
    }

    if mob.h.want_teleport {
        mob.h.want_teleport = false;

        if teleport(mob, view, rng, (mob.x, mob.y, mob.z), TELEPORT_SPREAD) {
            return true;
        }
    }

    stare(mob, view, ctx);
    grief(mob, view, rng, plan);
    false
}

/// Взгляд игроков: глядящий в голову дольше 5 тактов становится целью.
fn stare(mob: &mut Mob, view: &dyn View, ctx: &Ctx) {
    let eye = (mob.x, mob.y + ENDERMAN_EYE, mob.z);
    let staring = ctx
        .players
        .iter()
        .filter(|player| victim(player) && !player.pumpkin)
        .filter(|player| dist2(mob, player.x, player.y, player.z) <= 64.0 * 64.0)
        .find(|player| looks_at_head(mob, player) && sees(view, (player.x, player.y + player.eye, player.z), eye))
        .map(|player| player.id);

    mob.h.stare = if staring.is_some() { mob.h.stare + 1 } else { 0 };

    if let Some(id) = staring
        && mob.h.target.is_none()
        && mob.h.stare >= STARE_TICKS
    {
        mob.h.target = Some(id);
        mob.h.provoked = true;
    }

    // «На него смотрят» — сразу, ещё не сердит (чёрный ящик, enderman-stare:
    // 18=true на такт раньше 17=true).
    mob.h.stared_at = staring.is_some();
}

/// Видна ли клетка блока из точки: луч до её середины не входит в твёрдое,
/// кроме самой клетки.
fn sees_cell(view: &dyn View, from: (f64, f64, f64), cell: (i32, i32, i32)) -> bool {
    let to = (f64::from(cell.0) + 0.5, f64::from(cell.1) + 0.5, f64::from(cell.2) + 0.5);
    let (dx, dy, dz) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let steps = ((dx * dx + dy * dy + dz * dz).sqrt() / 0.25).ceil().max(1.0) as i32;

    for step in 1..steps {
        let t = f64::from(step) / f64::from(steps);
        let at = ((from.0 + dx * t).floor() as i32, (from.1 + dy * t).floor() as i32, (from.2 + dz * t).floor() as i32);

        if at != cell && super::solid(view.block(at.0, at.1, at.2)) {
            return false;
        }
    }

    true
}

/// Блоки: взять (1/20 за такт) или поставить (1/2000 за такт).
pub fn grief(mob: &mut Mob, view: &dyn View, rng: &mut Rng, plan: &mut Plan) {
    match mob.h.carried {
        None => {
            if rng.below(20) != 0 {
                return;
            }

            let cell = (
                (mob.x - 2.0 + rng.unit() * 4.0).floor() as i32,
                (mob.y + rng.unit() * 3.0).floor() as i32,
                (mob.z - 2.0 + rng.unit() * 4.0).floor() as i32,
            );
            let state = view.block(cell.0, cell.1, cell.2);

            if holdable(state) && sees_cell(view, (mob.x, mob.y + ENDERMAN_EYE, mob.z), cell) {
                mob.h.carried = Some(state);
                plan.take = Some(cell);
            }
        }
        Some(state) => {
            if rng.below(2000) != 0 {
                return;
            }

            let cell = (
                (mob.x - 1.0 + rng.unit() * 2.0).floor() as i32,
                (mob.y + rng.unit() * 2.0).floor() as i32,
                (mob.z - 1.0 + rng.unit() * 2.0).floor() as i32,
            );
            let here = crate::blocks::block_at_state(view.block(cell.0, cell.1, cell.2));
            let below = view.block(cell.0, cell.1 - 1, cell.2);

            if matches!(here, Some("air" | "cave_air")) && crate::blocks::solid_top(below) && crate::blocks::block_at_state(below) != Some("bedrock") {
                mob.h.carried = None;
                plan.put = Some((cell, state));
            }
        }
    }
}

/// Сердитый эндермен: бьёт, как зомби; под взглядом цели ближе 16 блоков
/// стоит; цель дальше 16 блоков — раз в 30–40 тактов прыгает к ней (вики:
/// «teleport … toward targets every 1.5-2 seconds») [проверить].
#[allow(clippy::too_many_arguments)]
fn enderman_chase(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, flat: f64, rng: &mut Rng, grounded: bool) {
    if mob.h.stared_at && dist_sq < 16.0 * 16.0 {
        mob.brain.stop();
        ai::look_at_point(mob, target.x, target.y + target.eye, target.z);
        return;
    }

    if dist_sq > 16.0 * 16.0 {
        mob.h.chase_timer += 1;

        if mob.h.chase_timer >= 30 + rng.below(11) as i32 {
            mob.h.chase_timer = 0;

            let len = dist_sq.sqrt();
            let toward = (mob.x + (target.x - mob.x) / len * 16.0, mob.y + (target.y - mob.y) / len * 16.0, mob.z + (target.z - mob.z) / len * 16.0);

            if teleport(mob, view, rng, toward, 8.0) {
                return;
            }
        }
    }

    melee(mob, view, target, plan, dist_sq, flat, rng, grounded);

    // Сердитый быстрее: модификатор «attacking» +0,15 к скорости 0,3
    // (чёрный ящик, enderman-stare).
    plan.speed *= (Kind::Enderman.speed() + ATTACKING_BONUS) / Kind::Enderman.speed();
}

/// Прибавка к скорости сердитого эндермена (чёрный ящик, enderman-stare).
pub const ATTACKING_BONUS: f64 = 0.15;

/// Прыжок: до 64 попыток найти место в кубе ±`spread` вокруг `centre`.
/// Эндермену — сухое место в рост над целым полом (опускается до пола);
/// шалкеру — пустая клетка у твёрдой грани. Звук — только в месте прихода
/// (вики, Enderman: «play a sound exclusively at the teleportation
/// destination»), частицы — событие сущности 46 [проверить].
pub fn teleport(mob: &mut Mob, view: &dyn View, rng: &mut Rng, centre: (f64, f64, f64), spread: f64) -> bool {
    let free = |state: i32| !super::solid(state) && !super::is_water(state) && !super::is_lava(state);

    for _ in 0..64 {
        let x = centre.0 + (rng.unit() - 0.5) * 2.0 * spread;
        let y = centre.1 + (rng.unit() - 0.5) * 2.0 * spread;
        let z = centre.2 + (rng.unit() - 0.5) * 2.0 * spread;
        let (cx, mut cy, cz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);

        if !view.loaded(cx.div_euclid(16), cz.div_euclid(16)) || !(crate::world::MIN_Y..crate::world::MIN_Y + crate::world::WORLD_HEIGHT).contains(&cy) {
            continue;
        }

        let to = if mob.kind == Kind::Shulker {
            let Some(face) = attach_face(view, (cx, cy, cz), rng).filter(|_| free(view.block(cx, cy, cz))) else {
                continue;
            };

            mob.h.face = face;
            (f64::from(cx) + 0.5, f64::from(cy), f64::from(cz) + 0.5)
        } else {
            while cy > crate::world::MIN_Y && !super::solid(view.block(cx, cy - 1, cz)) {
                cy -= 1;
            }

            if !super::solid(view.block(cx, cy - 1, cz)) || !(0..3).all(|dy| free(view.block(cx, cy + dy, cz))) {
                continue;
            }

            (x, f64::from(cy), z)
        };

        (mob.x, mob.y, mob.z) = to;
        (mob.vx, mob.vy, mob.vz) = (0.0, 0.0, 0.0);
        mob.on_ground = mob.kind != Kind::Shulker;
        mob.vitals.fall = 0.0;
        mob.teleports += 1;
        mob.brain.stop();

        let name = if mob.kind == Kind::Shulker { "minecraft:entity.shulker.teleport" } else { "minecraft:entity.enderman.teleport" };

        mob.events.push(Event::Status(46));

        if !mob.silent {
            mob.events.push(Event::Sound { name, volume: 1.0, pitch: 1.0 });
        }

        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Шалкер
// ---------------------------------------------------------------------------
//
// Вики (Shulker): сидит на грани блока; закрытый — броня 20, открытый — 0;
// сам по себе изредка приоткрывается на 1–3 с; видя игрока в 16 блоках,
// открывается и раз в 1–5,5 с (20–110 тактов) пускает самонаводящийся
// снаряд: 4 урона и левитация на 10 с. Ниже половины здоровья удар с шансом
// 25 % гонит его прочь; без опоры под гранью он тоже прыгает.

/// Сдвиг к блоку, на котором держится шалкер, по грани Direction.
pub fn face_offset(face: u8) -> (i32, i32, i32) {
    match face {
        1 => (0, 1, 0),
        2 => (0, 0, -1),
        3 => (0, 0, 1),
        4 => (-1, 0, 0),
        5 => (1, 0, 0),
        _ => (0, -1, 0),
    }
}

/// Грань, к которой шалкер в клетке может прилипнуть: сперва пол, потом
/// потолок, потом стены в случайном порядке [проверить: порядок у оригинала].
pub fn attach_face(view: &dyn View, (x, y, z): (i32, i32, i32), rng: &mut Rng) -> Option<u8> {
    let mut sides = [2u8, 3, 4, 5];
    let shift = rng.below(4) as usize;

    sides.rotate_left(shift);

    [0u8, 1].into_iter().chain(sides).find(|face| {
        let (dx, dy, dz) = face_offset(*face);

        super::solid(view.block(x + dx, y + dy, z + dz))
    })
}

/// Приоткрытость: смена со звуком открытия или закрытия (вики, Shulker, «Sounds»).
fn set_peek(mob: &mut Mob, peek: u8) {
    if peek == mob.h.peek {
        return;
    }

    let name = match (mob.h.peek, peek) {
        (0, _) => Some("minecraft:entity.shulker.open"),
        (_, 0) => Some("minecraft:entity.shulker.close"),
        _ => None,
    };

    mob.h.peek = peek;
    mob.h.covered = peek == 0;

    if let Some(name) = name
        && !mob.silent
    {
        mob.events.push(Event::Sound { name, volume: 1.0, pitch: 1.0 });
    }
}

/// Такт шалкера.
fn shulker(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();
    let (x, y, z) = mob.cell();
    let (dx, dy, dz) = face_offset(mob.h.face);

    if !super::solid(view.block(x + dx, y + dy, z + dz)) {
        match attach_face(view, (x, y, z), rng) {
            Some(face) => mob.h.face = face,
            None => mob.h.want_teleport = true,
        }
    }

    if mob.h.want_teleport {
        mob.h.want_teleport = false;

        if teleport(mob, view, rng, (mob.x, mob.y, mob.z), 8.0) {
            return plan;
        }
    }

    let had = mob.h.target;
    let target = choose_target(mob, view, ctx, rng).copied();

    let Some(target) = target else {
        if mob.h.peek == 100 {
            set_peek(mob, 0);
        }

        if mob.h.peek_timer > 0 {
            mob.h.peek_timer -= 1;

            if mob.h.peek_timer == 0 {
                set_peek(mob, 0);
            }
        } else if mob.h.peek == 0 && rng.below(40) == 0 {
            // Сам по себе приоткрывается на 1–3 с (вики); замер мира оригинала:
            // приоткрытые шалкеры стоят с Peek 30.
            set_peek(mob, 30);
            mob.h.peek_timer = 20 + rng.below(41) as i32;
        }

        return plan;
    };

    if had.is_none() {
        mob.h.cooldown = 20;
    }

    // Тело шалкера не поворачивается, голова смотрит на цель сразу
    // (чёрный ящик, shulker-ai: поворот головы одним пакетом, тела — нет).
    mob.head_yaw = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;
    set_peek(mob, 100);
    mob.h.peek_timer = 0;

    if mob.h.cooldown == 0 && line_of_sight(mob, view, &target) {
        mob.h.cooldown = 20 + rng.below(91) as i32;
        plan.bullet = Some(target.id);

        if !mob.silent {
            mob.events.push(Event::Sound { name: "minecraft:entity.shulker.shoot", volume: 2.0, pitch: mob.voice_pitch() });
        }
    }

    plan
}

/// Скорость снаряда шалкера, блоков за такт [проверить].
pub const BULLET_SPEED: f64 = 0.15;

/// Сколько тактов снаряд живёт без попадания [проверить].
pub const BULLET_LIFE: u64 = 300;

/// Урон снаряда и срок левитации (вики, Shulker: «4HP», «Levitation for 10 seconds»).
pub const BULLET_DAMAGE: f32 = 4.0;
pub const LEVITATION_TICKS: i32 = 200;

/// Такт снаряда: летит к цели, сворачивая к ней; в блоке гаснет, в цели —
/// попадание.
pub fn bullet(mob: &mut Mob, view: &dyn View, ctx: &Ctx) -> Plan {
    let mut plan = Plan::default();
    let target = mob.h.target.and_then(|id| ctx.players.iter().find(|player| player.id == id && victim(player)));

    let Some(target) = target.filter(|_| mob.ticks <= BULLET_LIFE) else {
        mob.h.despawn = true;
        return plan;
    };

    let aim = (target.x - mob.x, target.y + 0.9 - mob.y, target.z - mob.z);
    let len = (aim.0 * aim.0 + aim.1 * aim.1 + aim.2 * aim.2).sqrt().max(1.0e-6);

    mob.vx += (aim.0 / len * BULLET_SPEED - mob.vx) * 0.2;
    mob.vy += (aim.1 / len * BULLET_SPEED - mob.vy) * 0.2;
    mob.vz += (aim.2 / len * BULLET_SPEED - mob.vz) * 0.2;
    mob.x += mob.vx;
    mob.y += mob.vy;
    mob.z += mob.vz;

    let hit_sound = Event::Sound { name: "minecraft:entity.shulker_bullet.hit", volume: 1.0, pitch: 1.0 };

    if super::solid(view.block(mob.x.floor() as i32, mob.y.floor() as i32, mob.z.floor() as i32)) {
        mob.h.despawn = true;
        mob.events.push(hit_sound);
        return plan;
    }

    let reach = 0.3 + mob.kind.width() / 2.0;

    if (mob.x - target.x).abs() < reach && (mob.z - target.z).abs() < reach && mob.y > target.y - mob.kind.height() && mob.y < target.y + 1.8 {
        mob.h.despawn = true;
        mob.events.push(hit_sound);
        plan.bullet_hit = Some(target.id);
    }

    plan
}

// ---------------------------------------------------------------------------
// Ведьма
// ---------------------------------------------------------------------------
//
// Вики (Witch): пьёт зелья — подводного дыхания (под водой, 15 %), огнестойкости
// (горит, 15 %), лечения (ранена, 5 %), стремительности (цель в 11 блоках и
// дальше, 50 %); пьёт 1,6 с. Бросок взрывных зелий — когда они будут в ядре
// [проверить: бросков пока нет]; к цели подходит на 10 блоков и стоит.

/// Сколько тактов ведьма пьёт: у оригинала от «пьёт» (свойство 17) до
/// «допила» 33 такта (чёрный ящик, witch-stand; вики: «1.6 seconds»).
pub const DRINK_TICKS: i32 = 33;

/// Такт зелий ведьмы.
pub fn witch_drink(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) {
    use crate::survival::{EffectKind, potion_effects, potion_number};

    if mob.is_dead() {
        return;
    }

    // Допив, ведьма получает эффекты зелья; мгновенное лечение действует
    // тактом позже, как у игрока (чёрный ящик, witch-stand: частицы
    // лечения в такте конца питья, здоровье +4 — в следующем).
    if mob.h.drinking > 0 {
        mob.h.drinking -= 1;

        if mob.h.drinking == 0 {
            for effect in potion_effects(mob.h.potion) {
                mob.vitals.give_effect_shown(effect);
            }
        }

        return;
    }

    let (x, y, z) = mob.cell();
    let under = super::is_water(view.block(x, y + 1, z));
    let far = mob.h.target.and_then(|id| ctx.players.iter().find(|player| player.id == id)).is_some_and(|player| dist2(mob, player.x, player.y, player.z) >= 11.0 * 11.0);
    let has = |kind| mob.vitals.has_effect(kind);
    let potion = if under && !has(EffectKind::WaterBreathing) && rng.unit() < 0.15 {
        "water_breathing"
    } else if mob.vitals.fire > 0 && !has(EffectKind::FireResistance) && rng.unit() < 0.15 {
        "fire_resistance"
    } else if mob.vitals.health < mob.kind.max_health() && rng.unit() < 0.05 {
        "healing"
    } else if far && !has(EffectKind::Speed) && rng.unit() < 0.5 {
        "swiftness"
    } else {
        return;
    };

    mob.h.potion = potion_number(potion).unwrap_or(0);
    mob.h.drinking = DRINK_TICKS;

    if !mob.silent {
        let pitch = mob.voice_pitch();

        mob.events.push(Event::Sound { name: "minecraft:entity.witch.drink", volume: 1.0, pitch });
    }
}

/// Ведьма с целью: подходит на 10 блоков и смотрит на неё.
fn witch_keep(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, grounded: bool) {
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    if dist_sq <= 10.0 * 10.0 && line_of_sight(mob, view, target) {
        mob.brain.stop();
        return;
    }

    if grounded {
        approach(mob, view, target, 1.0);
    }

    *plan = ai::walk(mob, std::mem::take(plan));
}
