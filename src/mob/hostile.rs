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
        Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk => 35.0,
        // Вики, Enderman: злится на взгляд с 64 блоков и гонится за целью там же.
        Kind::Enderman => 64.0,
        Kind::Creaking => 32.0,
        Kind::Blaze | Kind::Ghast => super::nether::follow_range(kind),
        Kind::Phantom => 64.0,
        // Вики, Pillager: «32 blocks»; заклинатель 12, поборник 12 [проверить].
        Kind::Pillager | Kind::Ravager => 32.0,
        Kind::Evoker | Kind::Vindicator | Kind::Illusioner => 12.0,
        // [проверить] дальность слежения бриза: вики называет 16 для выстрела.
        Kind::Breeze => 24.0,
        Kind::WitherSkeleton => 16.0,
        Kind::ZombifiedPiglin => 35.0,
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
        Kind::Llama => "entity.minecraft.llama",
        Kind::TraderLlama => "entity.minecraft.trader_llama",
        Kind::WanderingTrader => "entity.minecraft.wandering_trader",
        Kind::Camel => "entity.minecraft.camel",
        Kind::Goat => "entity.minecraft.goat",
        Kind::PolarBear => "entity.minecraft.polar_bear",
        Kind::Panda => "entity.minecraft.panda",
        Kind::Ocelot => "entity.minecraft.ocelot",
        Kind::Mooshroom => "entity.minecraft.mooshroom",
        Kind::Endermite => "entity.minecraft.endermite",
        Kind::Zoglin => "entity.minecraft.zoglin",
        Kind::Creaking => "entity.minecraft.creaking",
        Kind::HappyGhast => "entity.minecraft.happy_ghast",
        Kind::CopperGolem => "entity.minecraft.copper_golem",
        Kind::Zombie => "entity.minecraft.zombie",
        Kind::ZombieVillager => "entity.minecraft.zombie_villager",
        Kind::Skeleton => "entity.minecraft.skeleton",
        Kind::Stray => "entity.minecraft.stray",
        Kind::Bogged => "entity.minecraft.bogged",
        Kind::Creeper => "entity.minecraft.creeper",
        Kind::Spider => "entity.minecraft.spider",
        Kind::CaveSpider => "entity.minecraft.cave_spider",
        Kind::Rabbit => "entity.minecraft.rabbit",
        Kind::Fox => "entity.minecraft.fox",
        Kind::Bee => "entity.minecraft.bee",
        Kind::Silverfish => "entity.minecraft.silverfish",
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
        Kind::Blaze => "entity.minecraft.blaze",
        Kind::WitherSkeleton => "entity.minecraft.wither_skeleton",
        Kind::MagmaCube => "entity.minecraft.magma_cube",
        Kind::Slime => "entity.minecraft.slime",
        Kind::Ghast => "entity.minecraft.ghast",
        Kind::ZombifiedPiglin => "entity.minecraft.zombified_piglin",
        Kind::Piglin => "entity.minecraft.piglin",
        Kind::PiglinBrute => "entity.minecraft.piglin_brute",
        Kind::Hoglin => "entity.minecraft.hoglin",
        Kind::Strider => "entity.minecraft.strider",
        Kind::Drowned => "entity.minecraft.drowned",
        Kind::Husk => "entity.minecraft.husk",
        Kind::Wolf => "entity.minecraft.wolf",
        Kind::Cat => "entity.minecraft.cat",
        Kind::Phantom => "entity.minecraft.phantom",
        Kind::Breeze => "entity.minecraft.breeze",
        Kind::LeashKnot => "entity.minecraft.leash_knot",
        Kind::IronGolem => "entity.minecraft.iron_golem",
        Kind::SnowGolem => "entity.minecraft.snow_golem",
        Kind::Guardian => "entity.minecraft.guardian",
        Kind::ElderGuardian => "entity.minecraft.elder_guardian",
        Kind::Wither => "entity.minecraft.wither",
        Kind::Pillager => "entity.minecraft.pillager",
        Kind::Vindicator => "entity.minecraft.vindicator",
        Kind::Evoker | Kind::EvokerFangs => "entity.minecraft.evoker",
        Kind::Vex => "entity.minecraft.vex",
        Kind::Illusioner => "entity.minecraft.illusioner",
        Kind::Ravager => "entity.minecraft.ravager",
        Kind::Cod => "entity.minecraft.cod",
        Kind::Salmon => "entity.minecraft.salmon",
        Kind::TropicalFish => "entity.minecraft.tropical_fish",
        Kind::Pufferfish => "entity.minecraft.pufferfish",
        Kind::Squid => "entity.minecraft.squid",
        Kind::GlowSquid => "entity.minecraft.glow_squid",
        Kind::Dolphin => "entity.minecraft.dolphin",
        Kind::Turtle => "entity.minecraft.turtle",
        Kind::Axolotl => "entity.minecraft.axolotl",
        Kind::Frog => "entity.minecraft.frog",
        Kind::Tadpole => "entity.minecraft.tadpole",
        Kind::Warden => "entity.minecraft.warden",
        Kind::Horse => "entity.minecraft.horse",
        Kind::Donkey => "entity.minecraft.donkey",
        Kind::Mule => "entity.minecraft.mule",
        Kind::SkeletonHorse => "entity.minecraft.skeleton_horse",
        Kind::ZombieHorse => "entity.minecraft.zombie_horse",
        Kind::Armadillo => "entity.minecraft.armadillo",
        Kind::Sniffer => "entity.minecraft.sniffer",
        Kind::Parrot => "entity.minecraft.parrot",
        Kind::Bat => "entity.minecraft.bat",
        Kind::Allay => "entity.minecraft.allay",

    }
}

/// Удар в ближнем бою: кому, сколько (до поправки на сложность) и куда толкает.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Melee {
    pub target: i32,
    pub damage: f32,
    pub push: (f64, f64),

    /// Удар иссушает: 10 секунд «иссушения» (вики, Wither Skeleton).
    pub wither: bool,

    /// Удар кадавра: «Голод» (вики, Husk; чёрный ящик, husk-hunger: id 16, 140 тактов).
    pub hunger: bool,
}

/// Выстрел скелета: откуда, куда целится (точка на цели) и скорость.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub from: (f64, f64, f64),
    pub aim: (f64, f64, f64),
    pub speed: f64,

    /// Бросок трезубца утопленника (вики, Drowned).
    pub trident: bool,

    /// Стрела из арбалета разбойника: звук попадания item.crossbow.hit
    /// (чёрный ящик, pillager-crossbow).
    pub crossbow: bool,
}

/// Состояние охоты.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Номер игрока-цели.
    pub target: Option<i32>,

    /// [проверить] задержка зова чешуйницы после урона.
    pub silverfish_call: i32,

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

    /// Снаряд шалкера: кто выпустил, ось полёта (0 x, 1 y, 2 z) и сколько
    /// тактов летит по ней.
    pub owner: i32,
    pub quiet: bool,
    pub axis: u8,
    pub leg: i32,

    /// Ведьма: сколько тактов ещё пьёт и какое зелье (номер в `POTIONS`).
    pub drinking: i32,
    pub potion: u16,

    /// Утопленник и кадавр (вики, Drowned и Husk): что в основной руке (`HAND_*`),
    /// ракушка во второй, шанс выпасть основной (у призванного без данных 8,5 %),
    /// ноги в воде, плывёт (флаг сущности 0x10), «использует предмет» (занесён
    /// трезубец; свойство 8), такты до броска, сколько тактов цель на виду.
    pub hand: u8,
    pub offhand_shell: bool,
    pub hand_drop: f32,
    pub wet: bool,
    pub swim: bool,
    pub using: bool,
    pub throw_wait: i32,
    pub see_time: i32,

    /// Доля подъёма к цели при плавании (dy / расстояние): тяга вверх и вниз.
    pub swim_rise: f64,

    /// Зомби и кадавр: сколько тактов голова в воде подряд, сколько тактов ещё
    /// трясётся до превращения (0 — не превращается) и во что превратился.
    pub water_ticks: i32,
    pub converting: i32,
    pub converted: Option<Kind>,
    /// Скелет в дрожи уже один такт вне снега (сброс дрожи позже на такт, skeleton-to-stray).
    pub snow_gap: bool,
}

/// Что в основной руке утопленника.
pub const HAND_NONE: u8 = 0;
pub const HAND_TRIDENT: u8 = 1;
pub const HAND_SHELL: u8 = 2;
pub const HAND_ROD: u8 = 3;

/// Имя предмета в руке по коду.
pub fn hand_item(code: u8) -> Option<&'static str> {
    match code {
        HAND_TRIDENT => Some("trident"),
        HAND_SHELL => Some("nautilus_shell"),
        HAND_ROD => Some("fishing_rod"),
        _ => None,
    }
}

/// Сколько тактов трясётся зомби перед превращением в воде и сколько
/// тактов голова должна быть в воде: 15 и 30 секунд (вики, Drowned,
/// «Spawning»: «zombie ... underwater for 30 seconds ... shakes for 15»).
pub const CONVERT_AFTER: i32 = 600;
pub const CONVERT_SHAKE: i32 = 302;

/// Счёт воды у оригинала идёт со 3-го такта жизни: флаг «трясётся» встаёт на
/// 602-м такте записи, превращение — на 903-м (чёрный ящик, zombie-to-drowned).
pub const CONVERT_FROM_TICK: u64 = 2;

/// Расстояние броска трезубца и пауза между бросками (вики, Drowned:
/// «throws ... every 2 seconds»; чёрный ящик, drowned-trident: 40 тактов).
pub const TRIDENT_RANGE: f64 = 10.0;
pub const TRIDENT_PAUSE: i32 = 40;
pub const TRIDENT_SPEED: f64 = 1.6;

/// «Голод» от кадавра: 7 секунд на нормальной (чёрный ящик, husk-hunger: 140
/// тактов, амплитуда 0), на трудной вдвое дольше [проверить: местная сложность
/// не ведётся], на лёгкой нет.
pub fn hunger_ticks(difficulty: u8) -> i32 {
    match difficulty {
        0 | 1 => 0,
        2 => 140,
        _ => 280,
    }
}

/// Игрок, которого можно атаковать: жив и в выживании.
fn victim(watcher: &Watcher) -> bool {
    watcher.targetable && !watcher.spectator
}

/// Расстояние в квадрате от ног моба до точки.
pub(super) fn dist2(mob: &Mob, x: f64, y: f64, z: f64) -> f64 {
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
pub(super) fn line_of_sight(mob: &Mob, view: &dyn View, watcher: &Watcher) -> bool {
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
    if !victim(watcher) || (mob.kind == Kind::Zoglin && mob.leash.is_some_and(|holder| holder.id() == watcher.id)) {
        return false;
    }

    let range = follow_range(mob.kind);

    if dist2(mob, watcher.x, watcher.y, watcher.z) > range * range {
        return false;
    }

    if mob.kind.spider() && !mob.h.provoked && !spider_bold(mob, view, ctx) {
        return false;
    }

    // Эндермен нейтрален: нападает, только если его разозлили (взгляд, удар).
    if mob.kind == Kind::Enderman && !mob.h.provoked {
        return false;
    }

    // Утопленник нападает на игрока ночью или в воде (вики, Drowned: «attack
    // players ... at night or if the player is in water»).
    if mob.kind == Kind::Drowned && ctx.sky >= 12 && !in_water_at(view, watcher) {
        return false;
    }

    // Зомби-пиглин спокоен без гнева, пиглин не трогает золотую броню.
    if super::nether::spares(mob, watcher) {
        return false;
    }

    // Вики, Silverfish: «can see the player through walls».
    mob.kind == Kind::Silverfish || !need_sight || line_of_sight(mob, view, watcher)
}

/// Выбор цели раз в 10 тактов; держится, пока игрок годится.
pub(super) fn choose_target(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Option<Watcher> {
    if let Some(id) = mob.h.target {
        let current = ctx.players.iter().find(|player| player.id == id);

        if let Some(player) = current.filter(|player| eligible(mob, view, ctx, player, false)) {
            return Some(*player);
        }

        // Зомби идёт и на жителя (вики, Zombie: цели — игрок, житель, железный
        // голем); видеть его не обязан.
        if let Some(villager) = prey(mob, ctx).into_iter().find(|villager| villager.id == id) {
            return Some(villager);
        }

        mob.h.target = None;
    }

    // Выбор цели — с шансом 1 к 10 за такт: у оригинала зомби в 6 блоках от
    // игрока трогается на 21-м такте (чёрный ящик, zombie-attack) [проверить].
    // Шалкер берёт цель сразу: у оригинала раскрывается на 1-м или 2-м такте
    // после появления (чёрный ящик, shulker-ai, два замера).
    if rng.below(if mob.kind == Kind::Shulker { 2 } else { 10 }) != 0 {
        return None;
    }

    let near = ctx
        .players
        .iter()
        .filter(|player| eligible(mob, view, ctx, player, true))
        .min_by(|a, b| dist2(mob, a.x, a.y, a.z).total_cmp(&dist2(mob, b.x, b.y, b.z)));

    // Игрока нет — ближайший житель в дальности слежения.
    if near.is_none() {
        let villager = prey(mob, ctx).into_iter().min_by(|a, b| dist2(mob, a.x, a.y, a.z).total_cmp(&dist2(mob, b.x, b.y, b.z)));

        mob.h.target = villager.map(|villager| villager.id);

        return villager;
    }

    mob.h.target = near.map(|player| player.id);
    near.copied()
}

/// Жители, на которых зомби может пойти: цели зомби и зомби-жителя
/// (вики, Zombie: «villagers»).
fn prey(mob: &Mob, ctx: &Ctx) -> Vec<Watcher> {
    // Разбойники тоже идут на жителей (вики, Illager: «attack villagers»).
    if !matches!(mob.kind, Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::Pillager | Kind::Vindicator | Kind::Ravager | Kind::Evoker | Kind::Illusioner) {
        return Vec::new();
    }

    let range = follow_range(mob.kind);
    // Поборник по имени «Johnny» нападает на всех мобов, кроме разбойников
    // (вики, Vindicator: «Johnny … attacks any mob except other illagers»).
    let johnny = mob.kind == Kind::Vindicator && mob.name.as_deref() == Some("Johnny");

    ctx.mates
        .iter()
        .filter(|mate| (mate.kind == Kind::Villager || (johnny && !crate::raid::joins(mate.kind) && mate.kind != Kind::Vex && !mate.kind.object())) && dist2(mob, mate.x, mate.y, mate.z) <= range * range)
        .map(|mate| Watcher { id: mate.id, x: mate.x, y: mate.y, z: mate.z, eye: if mate.baby { 0.81 } else { 1.62 }, holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false })
        .collect()
}

/// Мысль враждебного моба за такт.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    if super::slime::cube(mob.kind) {
        return super::slime::think(mob, view, ctx, rng);
    }
    let mut plan = Plan::default();


    // Голос в покое: как у животных (вики, Sounds), у крипера его нет.
    mob.h.ambient += 1;

    if rng.below(1000) < mob.h.ambient.max(0) as u32 {
        let (ambient, _, _) = mob.sounds();

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

    // Фантом летает по-своему (phantom.rs).
    if mob.kind == Kind::Phantom {
        return super::phantom::think(mob, view, ctx, rng, plan);
    }

    // Разбойники и вредина (illager.rs).
    if super::illager::raider(mob.kind) || mob.kind == Kind::Vex {
        return super::illager::think(mob, view, ctx, rng, plan);
    }

    // Мобы Незера думают в nether.rs.
    if mob.kind.nether() {
        return super::nether::think(mob, view, ctx, rng, plan);
    }

    let here = mob.cell();
    let in_water = super::is_water(view.block(here.0, here.1, here.2));

    mob.h.wet = in_water;

    // В воздухе путь ведётся, новый не строится (как у животных).
    let grounded = mob.on_ground || in_water || mob.kind.spider();
    if let Some(plan) = super::newmobs::hunt_mob(mob, view, ctx, rng) { return plan; }
    let target = choose_target(mob, view, ctx, rng);

    // Паук: днём цель забывается с шансом 1/100 за такт (вики, Spider).
    if mob.kind.spider() && target.is_some() && !spider_bold(mob, view, ctx) && rng.below(100) == 0 {
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
        Kind::Zombie | Kind::ZombieVillager | Kind::Husk | Kind::Spider | Kind::CaveSpider | Kind::Silverfish | Kind::Endermite | Kind::Creaking | Kind::Zoglin => melee(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
        Kind::Drowned => drowned(mob, view, ctx, &target, &mut plan, dist_sq, flat, sight, rng, grounded),
        Kind::Creeper => creeper(mob, view, ctx, &target, &mut plan, dist_sq, sight, grounded),
        Kind::Skeleton | Kind::Stray | Kind::Bogged if mob.h.bow => skeleton(mob, view, ctx, &target, &mut plan, dist_sq, sight, rng, grounded),
        Kind::Skeleton | Kind::Stray | Kind::Bogged => melee(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
        Kind::Enderman => enderman_chase(mob, view, &target, &mut plan, dist_sq, flat, rng, grounded),
        Kind::Witch => witch_keep(mob, view, &target, &mut plan, dist_sq, grounded),
        _ => {}
    }

    plan
}

/// То же для мобов Незера (nether.rs).
pub(super) fn idle_walk(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, plan: Plan, grounded: bool) -> Plan {
    idle(mob, view, ctx, rng, plan, grounded)
}

/// Нет цели: запал крипера гаснет, лук опускается, моб бродит.
fn idle(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng, mut plan: Plan, grounded: bool) -> Plan {
    mob.h.drawing = 0;
    mob.strafe = (0.0, 0.0);
    mob.h.swim = false;
    mob.h.using = false;
    mob.h.throw_wait = 0;
    mob.h.see_time = 0;

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
pub(super) fn approach(mob: &mut Mob, view: &dyn View, target: &Watcher, speed: f64) {
    let here = mob.cell();

    mob.brain.repath -= 1;

    if mob.brain.walking() && mob.brain.repath > 0 {
        return;
    }

    mob.brain.repath = 10;

    let goal = (target.x.floor() as i32, (target.y + 1.0e-3).floor() as i32, target.z.floor() as i32);

    // Паук идёт напрямую: путь по земле ему не нужен, стена — не преграда.
    let route = if mob.kind.spider() { vec![goal] } else { path::find(view, here, goal, path::MAX_NODES) };

    mob.brain.go(route, speed);
}

/// Зомби и паук: подойти и ударить.
#[allow(clippy::too_many_arguments)]
pub(super) fn melee(mob: &mut Mob, view: &dyn View, target: &Watcher, plan: &mut Plan, dist_sq: f64, flat: f64, rng: &mut Rng, grounded: bool) {
    // Досягаемость: (2·ширина)² + ширина цели, от ног до ног (вики, Mob AI,
    // «Melee attack») [проверить: по замеру zombie-hit].
    let (width, _) = mob.size();
    let reach_sq = (width * 2.0).powi(2) + 0.6;

    if dist_sq <= reach_sq {
        mob.brain.stop();
        ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

        if mob.h.cooldown == 0 && line_of_sight(mob, view, target) {
            mob.h.cooldown = if mob.kind == Kind::Zoglin && !mob.is_baby() { 40 } else { MELEE_COOLDOWN };

            let (ux, uz) = if flat > 1.0e-6 { ((target.x - mob.x) / flat, (target.z - mob.z) / flat) } else { (0.0, 1.0) };
            // Вики: паук и скелет 2, зомби 3, эндермен 7 (на нормальной).
            let damage = match mob.kind {
                Kind::Spider | Kind::CaveSpider | Kind::Skeleton | Kind::Stray | Kind::Bogged => 2.0,
                Kind::Silverfish => 1.0,
                Kind::Endermite => 2.0,
                Kind::Zoglin => if mob.is_baby() { 0.5 } else { 3.0 + rng.below(6) as f32 },
                Kind::Enderman => 7.0,
                Kind::WitherSkeleton | Kind::ZombifiedPiglin | Kind::Piglin | Kind::PiglinBrute | Kind::Hoglin => super::nether::melee_damage(mob),
                _ => 3.0,
            };

            mob.events.push(Event::Swing);
            if matches!(mob.kind,Kind::Creaking|Kind::Zoglin) {
                mob.events.push(Event::Sound{name:if mob.kind==Kind::Creaking {"minecraft:entity.creaking.attack"}else{"minecraft:entity.zoglin.attack"},volume:1.0,pitch:1.0});
            }
            plan.melee = Some(Melee { target: target.id, damage, push: (ux, uz), wither: mob.kind == Kind::WitherSkeleton, hunger: mob.kind == Kind::Husk });
        }

        return;
    }

    // Прыжок паука: на земле, цель в 2–4 блоках (вики, Spider: «leap»).
    if mob.kind.spider() && mob.on_ground && (4.0..=16.0).contains(&dist_sq) && rng.below(5) == 0 {
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

/// Вода ли в клетке ног игрока.
fn in_water_at(view: &dyn View, watcher: &Watcher) -> bool {
    super::is_water(view.block(watcher.x.floor() as i32, (watcher.y + 1.0e-3).floor() as i32, watcher.z.floor() as i32))
}

/// Утопленник (вики, Drowned): с трезубцем в руке стоит и бросает его с десяти
/// блоков раз в 40 тактов; иначе, когда цель в воде, плывёт к ней по прямой
/// (на ходу — `Mob::physics`), а когда нет — ходит по дну, как зомби.
#[allow(clippy::too_many_arguments)]
fn drowned(mob: &mut Mob, view: &dyn View, ctx: &Ctx, target: &Watcher, plan: &mut Plan, dist_sq: f64, flat: f64, sight: bool, rng: &mut Rng, grounded: bool) {
    mob.h.see_time = if sight { mob.h.see_time + 1 } else { 0 };

    let (width, _) = mob.size();
    let reach_sq = (width * 2.0).powi(2) + 0.6;
    let target_wet = in_water_at(view, target);

    // Трезубец: цель на виду уже 5 тактов и ближе десяти блоков — стоит
    // и целится; иначе идёт к ней. Счёт до броска идёт всё время охоты
    // (чёрный ящик, drowned-trident: бросок на 40-м такте после захвата цели, затем раз в 40).
    if mob.h.hand == HAND_TRIDENT {
        mob.h.using = true;
        mob.h.swim = false;

        if mob.h.throw_wait == 0 {
            mob.h.throw_wait = TRIDENT_PAUSE;
        } else {
            mob.h.throw_wait -= 1;

            if mob.h.throw_wait == 0 && sight {
                mob.h.throw_wait = TRIDENT_PAUSE;
                mob.events.push(Event::Swing);
                mob.events.push(Event::Sound { name: "minecraft:entity.drowned.shoot", volume: 1.0, pitch: 1.0 });
                plan.shot = Some(Shot { from: (mob.x, mob.y + mob.eye() - 0.1, mob.z), aim: (target.x, target.y + 0.6, target.z), speed: TRIDENT_SPEED, trident: true, crossbow: false });
            }
        }

        if dist_sq <= TRIDENT_RANGE * TRIDENT_RANGE && mob.h.see_time >= 5 {
            mob.brain.stop();
            ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

            let want = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;

            mob.yaw = ai::turn(mob.yaw, want, 30.0);

            return;
        }
    }

    if dist_sq <= reach_sq || !(mob.h.wet && target_wet) {
        mob.h.swim = false;
        melee(mob, view, target, plan, dist_sq, flat, rng, grounded);
        return;
    }

    // Плывёт к цели напрямую: тело поворачивается к ней, тяга — вперёд и по
    // вертикали на долю подъёма (вики, Drowned; `Mob::physics`).
    mob.brain.stop();
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    let want = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;

    mob.yaw = ai::turn(mob.yaw, want, 30.0);
    mob.h.swim = true;
    mob.h.swim_rise = (target.y - mob.y) / dist_sq.sqrt().max(1.0e-6);
    plan.speed = 1.0;
    let _ = ctx;
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
/// Крипер держится от кошек в 6 блоках, пока не запалён (вики, Cat: «Creepers
/// keep a distance of 6 blocks from any cat»); ближе 7 — бегом [проверить:
/// скорости 1,0 и 1,2 и точка бегства].
pub const CAT_AVOID: f64 = 6.0;

#[allow(clippy::too_many_arguments)]
fn creeper(mob: &mut Mob, view: &dyn View, ctx: &Ctx, target: &Watcher, plan: &mut Plan, dist_sq: f64, sight: bool, grounded: bool) {
    swell(mob, Some((dist_sq, sight)), plan);
    ai::look_at_point(mob, target.x, target.y + target.eye, target.z);

    if mob.h.swell_dir > 0 {
        mob.brain.stop();
        return;
    }

    let cat = ctx
        .mates
        .iter()
        .filter(|mate| mate.kind == Kind::Cat)
        .map(|mate| (mate, dist2(mob, mate.x, mate.y, mate.z)))
        .filter(|(_, dist)| *dist < CAT_AVOID * CAT_AVOID)
        .min_by(|a, b| a.1.total_cmp(&b.1));

    if let Some((cat, _)) = cat {
        // Прочь от кошки: точка в 8 блоках по линии от неё.
        if grounded && !mob.brain.walking() {
            let (dx, dz) = (mob.x - cat.x, mob.z - cat.z);
            let length = (dx * dx + dz * dz).sqrt().max(1.0e-6);
            let here = mob.cell();
            let away = ai::settle(view, (here.0 + (dx / length * 8.0).round() as i32, here.1, here.2 + (dz / length * 8.0).round() as i32), here.1);
            let route = path::find(view, here, away, path::MAX_NODES);

            mob.brain.go(route, 1.2);
        }
    } else if grounded {
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
            mob.h.cooldown = super::skeleton::shoot_pause(mob.kind, ctx.difficulty);
            mob.events.push(Event::Swing);
            // Вики, Skeleton/Stray/Bogged: общий звук выстрела, pitch 5/6…1,25.
            mob.events.push(Event::Sound { name: "minecraft:entity.skeleton.shoot", volume: 1.0, pitch: (1.0 / (rng.unit() * 0.4 + 0.8)) as f32 });
            plan.shot = Some(Shot {
                from: (mob.x, mob.y + mob.eye() - 0.1, mob.z),
                aim: (target.x, target.y + 0.6, target.z),
                speed: ARROW_SPEED,
                trident: false,
                crossbow: false,
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
    if mob.kind == Kind::Warden { mob.vitals.extinguish(); return false; }

    // Вода и дождь тушат (вики, Weather: «Extinguishes fires»).
    if super::is_water(view.block(x, y, z)) || in_rain(mob, view) {
        mob.vitals.extinguish();
        return false;
    }

    // Кадавр не горит (вики, Husk: «immune to burning in daylight»), утопленник горит
    // вне воды (вики, Drowned).
    let burns = matches!(mob.kind, Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Skeleton | Kind::Stray | Kind::Bogged | Kind::Phantom) && !mob.h.helmet;

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

    if mob.kind == Kind::Creaking && mob.newmobs.heart.is_some() { return; }

    if mob.kind == Kind::Slime && mob.nether.persistence_required {
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

    // Тело поворачивается к смотрящему на 10° за шаг: на втором такте взгляда
    // −10° (в пакете сверки на +3 у оригинала yaw −10), к моменту, когда
    // эндермен злится, — −20° и наклон на глаза игрока (rot −15/6 в байтах
    // угла, сверка 8,811°) (чёрный ящик, enderman-stare, три записи).
    let watcher = staring.and_then(|id| ctx.players.iter().find(|player| player.id == id));

    if let Some(player) = watcher
        && mob.h.target.is_none()
        && mob.h.stare == 2
    {
        turn_body(mob, player, 10.0);
    }

    if let Some(id) = staring
        && mob.h.target.is_none()
        && mob.h.stare >= STARE_TICKS
    {
        mob.h.target = Some(id);
        mob.h.provoked = true;

        if let Some(player) = watcher {
            turn_body(mob, player, 10.0);

            let horizontal = ((player.x - mob.x).powi(2) + (player.z - mob.z).powi(2)).sqrt().max(1.0e-6);

            mob.pitch = (-((player.y + player.eye) - (mob.y + ENDERMAN_EYE)).atan2(horizontal)).to_degrees() as f32;
        }
    }

    // «На него смотрят» — сразу, ещё не сердит (чёрный ящик, enderman-stare:
    // 18=true на такт раньше 17=true).
    mob.h.stared_at = staring.is_some();
}

/// Повернуть тело к игроку не больше чем на `step` градусов; при развороте
/// ровно на 180° — в отрицательную сторону (как у оригинала).
fn turn_body(mob: &mut Mob, player: &Watcher, step: f32) {
    let want = (-(player.x - mob.x).atan2(player.z - mob.z)).to_degrees() as f32;
    let mut diff = (want - mob.yaw) % 360.0;

    if diff > 180.0 {
        diff -= 360.0;
    } else if diff <= -180.0 {
        diff += 360.0;
    }

    if (diff.abs() - 180.0).abs() < 1.0e-3 {
        diff = -180.0;
    }

    mob.yaw += diff.clamp(-step, step);
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
    let target = choose_target(mob, view, ctx, rng);

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

    // Снаряд — через 18 тактов после раскрытия (чёрный ящик, shulker-ai:
    // раскрытие на +1 и +2, снаряд на +19 и +20).
    if had.is_none() {
        mob.h.cooldown = 19;
    }

    // Тело шалкера не поворачивается, голова смотрит на цель сразу
    // (чёрный ящик, shulker-ai: поворот головы одним пакетом, тела — нет).
    mob.head_yaw = (-(target.x - mob.x).atan2(target.z - mob.z)).to_degrees() as f32;

    // Ровно назад — чуть меньше +180° (байт 127, не −128): чёрный ящик, shulker-ai.
    if mob.head_yaw <= -179.999 {
        mob.head_yaw = 179.999;
    }

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

/// Скорость снаряда по оси за такт от начала участка: чёрный ящик, shulker-ai
/// (пакеты скорости раз в 3 такта: 0,077 0,123 0,152 0,174 0,193 0,211 0,228
/// 0,247, сдвиги за три такта сходятся до 1/4096; промежуточные такты —
/// гладкая подгонка). Это не формула оригинала, а таблица: точный закон разгона
/// неизвестен [проверить]; дальше таблицы (с 25-го такта) — подгонка по
/// длинному полёту второго снаряда: 0,259 / 0,279 / 0,301 / 0,324 на 26 / 29 /
/// 32 / 35-м такте участка.
const BULLET_RAMP: [f64; 24] = [
    0.03, 0.0571, 0.077, 0.0945, 0.1099, 0.123, 0.134, 0.1437, 0.152, 0.1604, 0.1678, 0.174, 0.181, 0.1873, 0.193, 0.1987, 0.2047, 0.211, 0.2168, 0.2225, 0.228, 0.2339, 0.2403, 0.247,
];

fn bullet_speed(leg: i32) -> f64 {
    let n = leg.max(1) as usize;

    if n <= BULLET_RAMP.len() {
        return BULLET_RAMP[n - 1];
    }

    let k = (n - BULLET_RAMP.len()) as f64;

    0.247 + 0.0058 * k + 0.00012 * k * k
}

/// Какая-то из сторон снаряда после сворачивания на другую ось гаснет: ×0,8
/// за такт (чёрный ящик, zz-shulker-side: 0,211 → 0,108 → 0,055 за три такта).
pub const BULLET_FADE: f64 = 0.8;

/// Сколько тактов снаряд живёт без попадания: у оригинала второй снаряд
/// ушёл на 119-м такте после появления (чёрный ящик, shulker-ai).
pub const BULLET_LIFE: u64 = 119;

/// Урон снаряда и срок левитации (вики, Shulker: «4HP», «Levitation for 10 seconds»).
pub const BULLET_DAMAGE: f32 = 4.0;
pub const LEVITATION_TICKS: i32 = 200;

/// Такт снаряда: летит вдоль одной оси к цели (вики, Shulker Bullet: «follows
/// the targeted entity along the X, Y, or Z axis»), по таблице разгона; в блоке
/// гаснет, в цели — попадание. Ось — по самому дальнему расстоянию, меняется,
/// когда по ней осталось меньше полублока [проверить: у оригинала участки
/// случайной длины — 18 и 10 тактов в двух замерах].
pub fn bullet(mob: &mut Mob, view: &dyn View, ctx: &Ctx) -> Plan {
    let mut plan = Plan::default();
    let target = mob.h.target.and_then(|id| ctx.players.iter().find(|player| player.id == id && victim(player)));

    let Some(target) = target.filter(|_| mob.ticks <= BULLET_LIFE) else {
        mob.h.despawn = true;
        return plan;
    };

    // Первый такт после появления снаряд стоит (чёрный ящик, shulker-ai:
    // сдвиг 0 на такт после появления).
    if mob.ticks < 1 {
        return plan;
    }

    let aim = [target.x - mob.x, target.y + 0.9 - mob.y, target.z - mob.z];
    let first = mob.h.leg == 0;

    if first || aim[usize::from(mob.h.axis)].abs() < 0.5 {
        let best = (0..3).max_by(|a, b| aim[*a].abs().total_cmp(&aim[*b].abs())).unwrap_or(2);

        if first || best != usize::from(mob.h.axis) {
            mob.h.axis = best as u8;
            mob.h.leg = 0;
        }
    }

    mob.h.leg += 1;

    // Наклон при первом движении −1 в байтах угла (чёрный ящик, shulker-ai:
    // первый сдвиг «поворот 0/-1»).
    if mob.h.leg == 1 && mob.pitch == 0.0 {
        mob.pitch = -1.0;
    }

    let axis = usize::from(mob.h.axis);
    let mut v = [mob.vx * BULLET_FADE, mob.vy * BULLET_FADE, mob.vz * BULLET_FADE];

    v[axis] = bullet_speed(mob.h.leg) * aim[axis].signum();
    (mob.vx, mob.vy, mob.vz) = (v[0], v[1], v[2]);
    mob.x += mob.vx;
    mob.y += mob.vy;
    mob.z += mob.vz;

    // Снаряд молчащего шалкера молчит: у оригинала при попадании звука нет
    // (чёрный ящик, shulker-ai, шалкер с Silent:1b).
    let hit_sound = Event::Sound { name: "minecraft:entity.shulker_bullet.hit", volume: 1.0, pitch: 1.0 };

    if super::solid(view.block(mob.x.floor() as i32, mob.y.floor() as i32, mob.z.floor() as i32)) {
        mob.h.despawn = true;

        if !mob.h.quiet {
            mob.events.push(hit_sound);
        }

        return plan;
    }

    // Ящик игрока, раздутый на 0,3 (как у снарядов), против ящика снаряда:
    // у оригинала снаряд ушёл на такте, где его середина ещё в 0,76 от оси
    // игрока (чёрный ящик, shulker-ai).
    let reach = 0.3 + 0.3 + mob.kind.width() / 2.0;

    if (mob.x - target.x).abs() < reach && (mob.z - target.z).abs() < reach && mob.y + mob.kind.height() > target.y - 0.3 && mob.y < target.y + 1.8 + 0.3 {
        mob.h.despawn = true;

        if !mob.h.quiet {
            mob.events.push(hit_sound);
        }

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
