// Здоровье, еда, урон и смерть игрока в выживании.
//
// Здесь только счёт — без сети и без мира: подключение (network/play/vitals.rs)
// каждый такт спрашивает у него, что случилось, и рассказывает клиенту. Всё
// сверено с оригиналом чёрным ящиком (сценарии fall-*, lava-burn, drowning,
// void-death, hunger-sprint, eat-food, tnt-damage) и вики: Health, Damage,
// Hunger, Drowning, Fire, Tutorial:Fall damage.
//
// Порядок работы за такт (у оригинала он такой, сверено по времени прихода
// пакетов):
//   1. пакеты клиента (падение считается по пакетам движения) — `on_move`;
//   2. рассылка сущности: обновлённые свойства и скорость после удара;
//   3. такт сущности (`tick`): огонь, лава, бездна, воздух, голод;
//   4. Set Health, если здоровье, еда или насыщение изменились.

/// Здоровье при рождении (вики, Health: «20 health points»).
pub const MAX_HEALTH: f32 = 20.0;

/// Запас воздуха в тактах (вики, Drowning: «300 ticks»).
pub const MAX_AIR: i32 = 300;

/// После этого значения воздух сбрасывается в 0 и наносится удар.
const DROWN_AT: i32 = -20;

/// Безопасная высота падения (вики, Fall damage: «the first three blocks»).
pub const SAFE_FALL: f32 = 3.0;

/// Число частиц блока при приземлении (LEVEL_PARTICLES, скорость 0,15).
/// `before` — высота падения, накопленная ДО шага, в котором тело встало на
/// землю (сам последний шаг в счёт не идёт: у урона он в счёте, у частиц
/// нет). Чёрный ящик (probe-land-*, игрок, корова и курица, ~60 точек):
/// частицы есть, когда целая часть высоты не меньше 4, и их 10 на целый блок,
/// не больше 375; высота 3,35 — без частиц, 4,06 — 40; сено, мёд, кровать,
/// слизь число не меняют. Не зависит от урона и режима.
pub fn landing_particles(before: f32) -> i32 {
    let blocks = before.floor();

    if blocks < 4.0 {
        return 0;
    }

    ((blocks * 10.0) as i32).min(375)
}

/// Сколько тактов горит поджжённый лавой: 15 секунд (вики, Lava).
pub const LAVA_FIRE_TICKS: i32 = 300;

/// Виды урона, которые знает сервер. Номера — по реестру damage_type
/// (configuration.rs), они идут в пакете Damage Event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Стрела (скелет или игрок из лука).
    Arrow,
    Cactus,
    /// Куст сладких ягод (вики, Sweet Berry Bush: «1♥ damage» в движении).
    SweetBerryBush,
    Drown,
    Explosion,
    Fall,
    /// Прочий урон (команда `damage` без вида).
    Generic,
    GenericKill,
    InFire,
    InWall,
    Lava,
    OnFire,
    OutOfWorld,
    /// Удар моба в ближнем бою.
    MobAttack,
    /// Взрыв, вызванный существом (крипер).
    PlayerExplosion,
    /// Удар игрока по существу.
    PlayerAttack,
    /// Сокрушающий удар булавой с падения (вики, Mace).
    MaceSmash,
    Starve,
    /// Шипы: урон ударившему по надетой броне с «шипами».
    Thorns,
    /// Магия: яд, мгновенный урон от команды (damage_type magic).
    Magic,
    /// Магия от существа: луч стража (damage_type indirect_magic; чёрный ящик,
    /// guardian-beam: Damage Event type=23).
    IndirectMagic,
    /// Череп иссушителя (damage_type wither_skull).
    WitherSkull,
    /// Иссушение (damage_type wither).
    Wither,
    /// Брошенный трезубец.
    Trident,
    /// Взрыв ракеты фейерверка (damage_type fireworks).
    Fireworks,
    /// За рамкой мира (damage_type outside_border).
    OutsideBorder,
    /// Молния (damage_type lightning_bolt; чёрный ящик, trident-channeling:
    /// Damage Event type=25).
    Lightning,
    /// Огненный шар ифрита (damage_type fireball, 14 по алфавиту реестра).
    Fireball,
    /// Удар о стену на элитрах (damage_type fly_into_wall).
    FlyIntoWall,
    /// Брошенное: снежок (вики, Snowball; реестр damage_type: thrown).
    Thrown,
}

impl Source {
    /// Номер вида в реестре damage_type.
    pub fn type_id(self) -> i32 {
        match self {
            Source::Arrow => 0,
            Source::Cactus => 2,
            // Реестр damage_type по алфавиту: starve 40, sting 41,
            // sweet_berry_bush 42, thorns 43.
            Source::SweetBerryBush => 42,
            Source::Drown => 6,
            Source::Explosion => 9,
            Source::Fall => 10,
            Source::Generic => 18,
            Source::GenericKill => 19,
            Source::InFire => 21,
            Source::InWall => 22,
            Source::Lava => 24,
            Source::OnFire => 31,
            Source::OutOfWorld => 32,
            Source::MobAttack => 28,
            Source::PlayerExplosion => 35,
            Source::PlayerAttack => 34,
            Source::MaceSmash => 26,
            Source::Starve => 40,
            Source::Thorns => 43,
            Source::Magic => 27,
            // Реестр по алфавиту: in_wall 22, indirect_magic 23, lava 24.
            Source::IndirectMagic => 23,
            // Реестр по алфавиту: wither 48, wither_skull 49 [проверить].
            Source::WitherSkull => 49,
            // Номер снят с оригинала (effect-regen: Damage Event type=48).
            Source::Wither => 48,
            Source::Trident => 45,
            // Реестр по алфавиту: thorns 43, thrown 44, trident 45.
            Source::Thrown => 44,
            // Место в реестре damage_type (configuration.rs: 15).
            Source::Fireworks => 15,
            // Реестр damage_type по алфавиту: out_of_world 32, outside_border
            // 33, player_attack 34.
            Source::OutsideBorder => 33,
            Source::Lightning => 25,
            Source::Fireball => 14,
            // Реестр по алфавиту: fireworks 15, fly_into_wall 16.
            Source::FlyIntoWall => 16,
        }
    }

    /// Ключ сообщения о смерти (вики, Death messages).
    pub fn death_key(self) -> &'static str {
        match self {
            Source::Arrow => "death.attack.arrow",
            Source::Cactus => "death.attack.cactus",
            Source::SweetBerryBush => "death.attack.sweetBerryBush",
            Source::Drown => "death.attack.drown",
            Source::Explosion => "death.attack.explosion",
            // Падение без лестницы под ногами: «fell from a high place».
            Source::Fall => "death.fell.accident.generic",
            Source::Generic => "death.attack.generic",
            Source::GenericKill => "death.attack.genericKill",
            Source::InFire => "death.attack.inFire",
            Source::InWall => "death.attack.inWall",
            Source::Lava => "death.attack.lava",
            Source::OnFire => "death.attack.onFire",
            Source::OutOfWorld => "death.attack.outOfWorld",
            Source::MobAttack => "death.attack.mob",
            Source::PlayerExplosion => "death.attack.explosion.player",
            Source::PlayerAttack => "death.attack.player",
            Source::MaceSmash => "death.attack.mace_smash",
            Source::Starve => "death.attack.starve",
            Source::Thorns => "death.attack.thorns",
            Source::Magic => "death.attack.magic",
            Source::IndirectMagic => "death.attack.indirectMagic",
            Source::WitherSkull => "death.attack.witherSkull",
            Source::Wither => "death.attack.wither",
            Source::Trident => "death.attack.trident",
            Source::Thrown => "death.attack.thrown",
            Source::Fireworks => "death.attack.fireworks",
            Source::FlyIntoWall => "death.attack.flyIntoWall",
            Source::OutsideBorder => "death.attack.outsideBorder",
            Source::Lightning => "death.attack.lightningBolt",
            Source::Fireball => "death.attack.fireball",
        }
    }

    /// Снаряд: стрела или брошенный трезубец (тег is_projectile).
    pub fn is_projectile(self) -> bool {
        matches!(self, Source::Arrow | Source::Trident | Source::Fireball | Source::Thrown)
    }

    /// Урон не ждёт конца неуязвимости после прошлого удара (`bypasses_cooldown`).
    /// [проверить]: у оригинала это признак только у /kill (generic_kill);
    /// бездна бьёт раз в 10 тактов, как и обычный урон (void-death).
    pub fn bypasses_cooldown(self) -> bool {
        matches!(self, Source::GenericKill)
    }

    /// Урон проходит сквозь неуязвимость творческого режима.
    pub fn bypasses_invulnerability(self) -> bool {
        matches!(self, Source::OutOfWorld | Source::GenericKill)
    }

    /// Голод от удара: 0,1, у падения, утопления, голода и бездны — нет
    /// (сверено: fall-damage сохраняет насыщение до первого лечения).
    pub fn exhaustion(self) -> f32 {
        match self {
            // Магия и иссушение изнурения не дают (вики, Hunger: «Taking
            // damage… except… poison, wither») [проверить].
            Source::Fall | Source::Drown | Source::Starve | Source::OutOfWorld | Source::GenericKill | Source::Magic | Source::Wither => 0.0,
            _ => 0.1,
        }
    }

    /// Урон растёт со сложностью (вики, Difficulty: «Explosion»): мирная — 0,
    /// лёгкая — min(d/2+1, d), нормальная — d, трудная — 1,5d.
    pub fn scales_with_difficulty(self) -> bool {
        // Луч стража — тоже (вики, Guardian: 4/6/9 по сложности).
        matches!(self, Source::Explosion | Source::PlayerExplosion | Source::MobAttack | Source::Arrow | Source::IndirectMagic | Source::WitherSkull)
    }
}

/// Виды блоков, гасящие падение, и множители урона от блока внизу.
pub fn soft_block(name: &str) -> bool {
    matches!(
        name,
        "cobweb" | "ladder" | "vine" | "scaffolding" | "sweet_berry_bush" | "powder_snow" | "twisting_vines" | "weeping_vines"
            | "twisting_vines_plant" | "weeping_vines_plant" | "cave_vines" | "cave_vines_plant" | "bubble_column"
    )
}

pub fn landing_multiplier(name: &str) -> f32 {
    match name {
        "hay_block" | "honey_block" => 0.2,
        // Слизь подкидывает вместо удара, если не красться.
        "slime_block" => 0.0,
        name if name.ends_with("_bed") => 0.5,
        _ => 1.0,
    }
}

/// Текст сообщения о смерти для остальных игроков.
pub fn death_text(source: Source, name: &str) -> String {
    let what = match source {
        Source::Fall => "fell from a high place",
        Source::Lava => "tried to swim in lava",
        Source::Drown => "drowned",
        Source::OutOfWorld => "fell out of the world",
        Source::Explosion => "blew up",
        Source::GenericKill => "was killed",
        Source::Starve => "starved to death",
        Source::OnFire => "burned to death",
        Source::InFire => "went up in flames",
        Source::Cactus => "was pricked to death",
        Source::SweetBerryBush => "was poked to death by a sweet berry bush",
        Source::InWall => "suffocated in a wall",
        Source::Generic => "died",
        Source::PlayerAttack => "was slain",
        Source::MaceSmash => "was smashed",
        Source::MobAttack => "was slain by a monster",
        Source::Arrow => "was shot",
        Source::PlayerExplosion => "was blown up",
        Source::Thorns => "was killed",
        Source::Magic | Source::IndirectMagic => "was killed by magic",
        Source::Wither => "withered away",
        Source::WitherSkull => "was shot by a skull",
        Source::Trident => "was impaled",
        Source::Thrown => "was pummeled",
        Source::Fireworks => "went off with a bang",
        Source::FlyIntoWall => "experienced kinetic energy",
        Source::OutsideBorder => "left the confines of this world",
        Source::Lightning => "was struck by lightning",
        Source::Fireball => "was fireballed",
    };

    format!("{} {}", name, what)
}

/// Название предмета для текста: `diamond_sword` → `Diamond Sword`.
pub fn item_title(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();

            chars.next().map_or_else(String::new, |first| first.to_uppercase().collect::<String>() + chars.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Сообщение о смерти от другого игрока: «X was slain by Y [using Z]»
/// (вики, Death messages).
pub fn death_text_by(source: Source, name: &str, killer: &str, item: Option<&str>) -> String {
    // Ракета: ключ death.attack.fireworks и с пустившим — «went off with a
    // bang»; пустивший называется только у ракеты с именем (`.item`) [проверить].
    if source == Source::Fireworks {
        return death_text(source, name);
    }

    let what = match source {
        Source::Arrow => "was shot by",
        Source::Thorns => "was killed trying to hurt",
        Source::MaceSmash => "was smashed by",
        Source::Trident => "was impaled by",
        Source::Thrown => "was pummeled by",
        Source::Fireball => "was fireballed by",
        _ => "was slain by",
    };

    match item {
        Some(item) => format!("{} {} {} using [{}]", name, what, killer, item_title(item)),
        None => format!("{} {} {}", name, what, killer),
    }
}

/// Размеры шариков опыта, от больших к меньшим (вики, Experience, «Orb
/// size»): значение делится на них жадно.
const ORB_SIZES: [i32; 11] = [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3, 1];

/// На какие шарики раскладывается `value` очков опыта.
pub fn orb_values(mut value: i32) -> Vec<i32> {
    let mut orbs = Vec::new();

    for size in ORB_SIZES {
        while value >= size {
            orbs.push(size);
            value -= size;
        }
    }

    orbs
}

/// Сколько очков нужно, чтобы перейти с уровня `level` на следующий (вики,
/// Experience, «Leveling up»): 2L+7 до 15-го, 5L−38 до 30-го, дальше 9L−158.
pub fn level_size(level: i32) -> i32 {
    match level {
        ..=15 => 2 * level + 7,
        16..=30 => 5 * level - 38,
        _ => 9 * level - 158,
    }
}

/// Всего очков, чтобы достичь уровня `level`: L²+6L, 2,5L²−40,5L+360,
/// 4,5L²−162,5L+2220 (вики, Experience).
pub fn total_at_level(level: i32) -> i32 {
    let l = f64::from(level);

    (match level {
        ..=16 => l * l + 6.0 * l,
        17..=31 => 2.5 * l * l - 40.5 * l + 360.0,
        _ => 4.5 * l * l - 162.5 * l + 2220.0,
    }) as i32
}

/// Уровень и доля шкалы (0…1) по общему числу очков.
pub fn level_of(total: i32) -> (i32, f32) {
    let total = total.max(0);
    let mut level = 0;

    while total >= total_at_level(level + 1) {
        level += 1;
    }

    (level, (total - total_at_level(level)) as f32 / level_size(level) as f32)
}

/// Опыт за добытую руду без «шёлкового касания» (вики, Experience, «Sources»):
/// промежуток `(наименьшее, наибольшее)`; `roll` — случайное от 0 до 1.
pub fn ore_xp(block: &str, roll: f32) -> i32 {
    let (low, high) = match block {
        "coal_ore" | "deepslate_coal_ore" => (0, 2),
        "nether_gold_ore" => (0, 1),
        "diamond_ore" | "deepslate_diamond_ore" | "emerald_ore" | "deepslate_emerald_ore" => (3, 7),
        "lapis_ore" | "deepslate_lapis_ore" | "nether_quartz_ore" => (2, 5),
        "redstone_ore" | "deepslate_redstone_ore" => (1, 5),
        "spawner" => (15, 43),
        _ => return 0,
    };

    (low + (roll.clamp(0.0, 0.999_999) * (high - low + 1) as f32) as i32).min(high)
}

/// Опыт за убитую корову, свинью, овцу или курицу: от 1 до 3 (вики, Cow,
/// Pig, Sheep, Chicken, «Drops»); `roll` — случайное от 0 до 1. Игроку он
/// отдаётся шариками — `Order::Orb { at, value }` тому, кто убил.
pub fn passive_mob_xp(roll: f32) -> i32 {
    1 + (roll.clamp(0.0, 0.999_999) * 3.0) as i32
}

/// Сложность: 0 — мирная, 1 — лёгкая, 2 — нормальная, 3 — трудная.
pub const PEACEFUL: u8 = 0;
pub const EASY: u8 = 1;
pub const NORMAL: u8 = 2;
pub const HARD: u8 = 3;

/// Урон после поправки на сложность.
pub fn scaled(amount: f32, difficulty: u8) -> f32 {
    match difficulty {
        PEACEFUL => 0.0,
        EASY => (amount / 2.0 + 1.0).min(amount),
        NORMAL => amount,
        _ => amount * 1.5,
    }
}

/// Урон взрыва по игроку: `(impact² + impact)/2 · 7 · (2·power) + 1`
/// (вики, Explosion: «Damage»). Дробный: чёрный ящик (tnt-damage) — оригинал
/// снимает 14,466, а не целое.
pub fn explosion_damage(impact: f64, power: f32) -> f32 {
    ((impact * impact + impact) / 2.0 * 7.0 * f64::from(2.0 * power) + 1.0) as f32
}

/// Урон после сопротивления: `урон · (25 − 5·уровень) / 25`, в f32 как у
/// оригинала (чёрный ящик, pvp-resistance: 7 · 15 / 25 = 4,2).
pub fn after_resistance(amount: f32, level: i32, source: Source) -> f32 {
    if level <= 0 || matches!(source, Source::Starve | Source::OutOfWorld | Source::GenericKill) {
        return amount;
    }

    amount * ((25 - 5 * level).max(0)) as f32 / 25.0
}

/// Исход удара.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Сколько здоровья снято на самом деле.
    pub taken: f32,
    /// Игрок умер от этого удара.
    pub died: bool,
}

/// Сколько частиц эффектов ведётся (по одной на эффект; видов 29).
pub const EFFECT_PARTICLES: usize = 32;

/// Вид эффекта, который считает сервер.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    /// Голод: каждый такт добавляет 0,005·(уровень) к изнурению.
    Hunger,
    /// Насыщение: еда и насыщение растут каждый такт.
    Saturation,
    /// Скорость: +20 % к скорости движения за уровень (вики, Speed).
    Speed,
    /// Замедление: −15 % к скорости движения за уровень (вики, Slowness).
    Slowness,
    /// Сила: +3 к урону ближнего боя за уровень (вики, Strength).
    Strength,
    /// Слабость: −4 к урону ближнего боя за уровень (вики, Weakness).
    Weakness,
    /// Сопротивление: −20 % входящего урона за уровень (вики, Resistance).
    Resistance,
    /// Спешка: +10 % к скорости удара за уровень (вики, Haste).
    Haste,
    /// Усталость: −10 % к скорости удара за уровень (вики, Mining Fatigue).
    MiningFatigue,
    /// Мгновенное лечение: 4·2^уровень сразу (вики, Instant Health).
    InstantHealth,
    /// Мгновенный урон: 6·2^уровень сразу (вики, Instant Damage).
    InstantDamage,
    /// Прыгучесть: +0,1 к силе прыжка и +1 к безопасной высоте за уровень
    /// (вики, Jump Boost).
    JumpBoost,
    /// Тошнота: только у клиента.
    Nausea,
    /// Регенерация: +1 здоровья раз в 50 ≫ уровень тактов (вики, Regeneration).
    Regeneration,
    /// Огнестойкость: нет урона от огня и лавы.
    FireResistance,
    /// Подводное дыхание: воздух не убывает.
    WaterBreathing,
    /// Невидимость: флаг сущности 0x20.
    Invisibility,
    /// Слепота: только у клиента.
    Blindness,
    /// Ночное зрение: только у клиента.
    NightVision,
    /// Яд: 1 урона раз в 25 ≫ уровень тактов, не ниже 1 здоровья (вики, Poison).
    Poison,
    /// Иссушение: 1 урона раз в 40 ≫ уровень тактов (вики, Wither).
    Wither,
    /// Прирост здоровья: +4 к наибольшему здоровью за уровень.
    HealthBoost,
    /// Поглощение: 4 золотых сердца за уровень.
    Absorption,
    /// Свечение: флаг сущности 0x40.
    Glowing,
    /// Левитация: подъём у клиента.
    Levitation,
    /// Удача: +1 к атрибуту luck за уровень.
    Luck,
    /// Неудача: −1 к атрибуту luck за уровень.
    Unluck,
    /// Плавное падение: −0,07 к тяготению, без урона от падения.
    SlowFalling,
    /// Тьма: только у клиента.
    Darkness,
}

impl EffectKind {
    /// Номер эффекта в реестре (пакет Entity Effect).
    pub fn id(self) -> i32 {
        match self {
            EffectKind::Speed => 0,
            EffectKind::Slowness => 1,
            EffectKind::Strength => 4,
            EffectKind::Resistance => 10,
            EffectKind::Hunger => 16,
            EffectKind::Weakness => 17,
            EffectKind::Saturation => 22,
            EffectKind::Haste => 2,
            EffectKind::MiningFatigue => 3,
            EffectKind::InstantHealth => 5,
            EffectKind::InstantDamage => 6,
            EffectKind::JumpBoost => 7,
            EffectKind::Nausea => 8,
            EffectKind::Regeneration => 9,
            EffectKind::FireResistance => 11,
            EffectKind::WaterBreathing => 12,
            EffectKind::Invisibility => 13,
            EffectKind::Blindness => 14,
            EffectKind::NightVision => 15,
            EffectKind::Poison => 18,
            EffectKind::Wither => 19,
            EffectKind::HealthBoost => 20,
            EffectKind::Absorption => 21,
            EffectKind::Glowing => 23,
            EffectKind::Levitation => 24,
            EffectKind::Luck => 25,
            EffectKind::Unluck => 26,
            EffectKind::SlowFalling => 27,
            EffectKind::Darkness => 32,
        }
    }

    /// Все виды (для перебора модификаторов).
    pub const ALL: [EffectKind; 29] = [
        EffectKind::Speed, EffectKind::Slowness, EffectKind::Haste, EffectKind::MiningFatigue, EffectKind::Strength,
        EffectKind::InstantHealth, EffectKind::InstantDamage, EffectKind::JumpBoost, EffectKind::Nausea, EffectKind::Regeneration,
        EffectKind::Resistance, EffectKind::FireResistance, EffectKind::WaterBreathing, EffectKind::Invisibility, EffectKind::Blindness,
        EffectKind::NightVision, EffectKind::Hunger, EffectKind::Weakness, EffectKind::Poison, EffectKind::Wither,
        EffectKind::HealthBoost, EffectKind::Absorption, EffectKind::Saturation, EffectKind::Glowing, EffectKind::Levitation,
        EffectKind::Luck, EffectKind::Unluck, EffectKind::SlowFalling, EffectKind::Darkness,
    ];

    /// Мгновенный: действует при наложении, не хранится.
    pub fn instant(self) -> bool {
        matches!(self, EffectKind::InstantHealth | EffectKind::InstantDamage)
    }

    /// Цвет частиц эффекта (RGB, вики, страницы эффектов).
    pub fn color(self) -> i32 {
        match self {
            EffectKind::Speed => 0x33EBFF,
            EffectKind::Slowness => 0x8BAFE0,
            EffectKind::Strength => 0xFFC700,
            EffectKind::Resistance => 0x9146F0,
            EffectKind::Hunger => 0x587653,
            EffectKind::Weakness => 0x484D48,
            EffectKind::Saturation => 0xF82423,
            // Остальные — по вики (Effect, «Particle color»), все сверены с
            // оригиналом (effect-flags, effect-regen, effect-instant, effect-absorption).
            EffectKind::Haste => 0xD9C043,
            EffectKind::MiningFatigue => 0x4A4217,
            EffectKind::InstantHealth => 0xF82423,
            EffectKind::InstantDamage => 0xA9656A,
            EffectKind::JumpBoost => 0xFDFF84,
            EffectKind::Nausea => 0x551D4A,
            EffectKind::Regeneration => 0xCD5CAB,
            EffectKind::FireResistance => 0xFF9900,
            EffectKind::WaterBreathing => 0x98DAC0,
            EffectKind::Invisibility => 0xF6F6F6,
            EffectKind::Blindness => 0x1F1F23,
            EffectKind::NightVision => 0xC2FF66,
            EffectKind::Poison => 0x87A363,
            EffectKind::Wither => 0x736156,
            EffectKind::HealthBoost => 0xF87D23,
            EffectKind::Absorption => 0x2552A5,
            EffectKind::Glowing => 0x94A061,
            EffectKind::Levitation => 0xCEFFFF,
            EffectKind::Luck => 0x59C106,
            EffectKind::Unluck => 0xC0A44D,
            EffectKind::SlowFalling => 0xF3CFB9,
            EffectKind::Darkness => 0x292721,
        }
    }

    /// Атрибут, который эффект меняет, опознаватель и величина модификатора
    /// на уровень (вики, Attribute; страницы эффектов).
    pub fn modifier(self) -> Option<(i32, &'static str, f64, crate::attributes::Operation)> {
        use crate::attributes::{Operation, ATTACK_DAMAGE, ATTACK_SPEED, GRAVITY, JUMP_STRENGTH, LUCK, MAX_ABSORPTION, MAX_HEALTH, MOVEMENT_SPEED};

        match self {
            EffectKind::Haste => Some((ATTACK_SPEED, "minecraft:effect.haste", 0.1, Operation::AddMultipliedTotal)),
            EffectKind::MiningFatigue => Some((ATTACK_SPEED, "minecraft:effect.mining_fatigue", -0.1, Operation::AddMultipliedTotal)),
            EffectKind::HealthBoost => Some((MAX_HEALTH, "minecraft:effect.health_boost", 4.0, Operation::AddValue)),
            EffectKind::Absorption => Some((MAX_ABSORPTION, "minecraft:effect.absorption", 4.0, Operation::AddValue)),
            EffectKind::JumpBoost => Some((JUMP_STRENGTH, "minecraft:effect.jump_boost", 0.1, Operation::AddValue)),
            EffectKind::SlowFalling => Some((GRAVITY, "minecraft:effect.slow_falling", -0.07, Operation::AddValue)),
            EffectKind::Luck => Some((LUCK, "minecraft:effect.luck", 1.0, Operation::AddValue)),
            EffectKind::Unluck => Some((LUCK, "minecraft:effect.unluck", -1.0, Operation::AddValue)),
            EffectKind::Speed => Some((MOVEMENT_SPEED, "minecraft:effect.speed", 0.2, Operation::AddMultipliedTotal)),
            EffectKind::Slowness => Some((MOVEMENT_SPEED, "minecraft:effect.slowness", -0.15, Operation::AddMultipliedTotal)),
            EffectKind::Strength => Some((ATTACK_DAMAGE, "minecraft:effect.strength", 3.0, Operation::AddValue)),
            EffectKind::Weakness => Some((ATTACK_DAMAGE, "minecraft:effect.weakness", -4.0, Operation::AddValue)),
            _ => None,
        }
    }

    /// Место в списке частиц: порядок, который прислал оригинал (slowness,
    /// speed, weakness, resistance, strength; чёрный ящик, effect-speed) —
    /// похоже на хеш-порядок, не на имена и не на номера [проверить].
    fn particle_rank(self) -> u8 {
        match self {
            EffectKind::Slowness => 0,
            EffectKind::Speed => 1,
            EffectKind::Weakness => 2,
            // Поглощение и регенерация идут перед сопротивлением и
            // огнестойкостью (golden-apple), поглощение — перед приростом
            // здоровья (effect-absorption).
            EffectKind::Absorption => 3,
            EffectKind::Regeneration => 4,
            EffectKind::Resistance => 5,
            EffectKind::Strength => 6,
            EffectKind::Hunger => 7,
            EffectKind::Saturation => 8,
            // Порядок снят с оригинала (effect-flags, до 12 эффектов; при
            // 13 и больше оригинал перекладывает список) [проверить].
            EffectKind::Blindness => 10,
            EffectKind::Luck => 11,
            EffectKind::Glowing => 12,
            EffectKind::NightVision => 13,
            EffectKind::Unluck => 14,
            EffectKind::WaterBreathing => 15,
            EffectKind::FireResistance => 16,
            EffectKind::SlowFalling => 17,
            EffectKind::Invisibility => 18,
            EffectKind::MiningFatigue => 19,
            EffectKind::JumpBoost => 20,
            EffectKind::Haste => 21,
            other => 30 + other.id() as u8,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.strip_prefix("minecraft:").unwrap_or(name) {
            "hunger" => Some(EffectKind::Hunger),
            "saturation" => Some(EffectKind::Saturation),
            "speed" => Some(EffectKind::Speed),
            "slowness" => Some(EffectKind::Slowness),
            "strength" => Some(EffectKind::Strength),
            "weakness" => Some(EffectKind::Weakness),
            "resistance" => Some(EffectKind::Resistance),
            "haste" => Some(EffectKind::Haste),
            "mining_fatigue" => Some(EffectKind::MiningFatigue),
            "instant_health" => Some(EffectKind::InstantHealth),
            "instant_damage" => Some(EffectKind::InstantDamage),
            "jump_boost" => Some(EffectKind::JumpBoost),
            "nausea" => Some(EffectKind::Nausea),
            "regeneration" => Some(EffectKind::Regeneration),
            "fire_resistance" => Some(EffectKind::FireResistance),
            "water_breathing" => Some(EffectKind::WaterBreathing),
            "invisibility" => Some(EffectKind::Invisibility),
            "blindness" => Some(EffectKind::Blindness),
            "night_vision" => Some(EffectKind::NightVision),
            "poison" => Some(EffectKind::Poison),
            "wither" => Some(EffectKind::Wither),
            "health_boost" => Some(EffectKind::HealthBoost),
            "absorption" => Some(EffectKind::Absorption),
            "glowing" => Some(EffectKind::Glowing),
            "levitation" => Some(EffectKind::Levitation),
            "luck" => Some(EffectKind::Luck),
            "unluck" => Some(EffectKind::Unluck),
            "slow_falling" => Some(EffectKind::SlowFalling),
            "darkness" => Some(EffectKind::Darkness),
            _ => None,
        }
    }
}

/// Эффекты еды (вики, Golden Apple и Enchanted Golden Apple).
pub fn food_effects(name: &str) -> Vec<Effect> {
    let effect = |kind, amplifier, seconds| Effect { kind, amplifier, ticks_left: seconds * 20, ambient: false, visible: true };

    match name {
        "golden_apple" => vec![effect(EffectKind::Regeneration, 1, 5), effect(EffectKind::Absorption, 0, 120)],
        "enchanted_golden_apple" => vec![
            effect(EffectKind::Regeneration, 1, 20),
            effect(EffectKind::Absorption, 3, 120),
            effect(EffectKind::Resistance, 0, 300),
            effect(EffectKind::FireResistance, 0, 300),
        ],
        _ => Vec::new(),
    }
}

/// Зелья 26.1.2 по номерам реестра `minecraft:potion` (отчёт `--reports`
/// официального сервера, registries.json): номер — место в списке.
pub const POTIONS: [&str; 46] = [
    "water", "mundane", "thick", "awkward", "night_vision", "long_night_vision", "invisibility", "long_invisibility",
    "leaping", "long_leaping", "strong_leaping", "fire_resistance", "long_fire_resistance", "swiftness", "long_swiftness",
    "strong_swiftness", "slowness", "long_slowness", "strong_slowness", "turtle_master", "long_turtle_master",
    "strong_turtle_master", "water_breathing", "long_water_breathing", "healing", "strong_healing", "harming",
    "strong_harming", "poison", "long_poison", "strong_poison", "regeneration", "long_regeneration", "strong_regeneration",
    "strength", "long_strength", "strong_strength", "weakness", "long_weakness", "luck", "slow_falling", "long_slow_falling",
    "wind_charged", "weaving", "oozing", "infested",
];

/// Номер зелья по имени без пространства имён.
pub fn potion_number(name: &str) -> Option<u16> {
    POTIONS.iter().position(|known| *known == name).map(|number| number as u16)
}

/// Эффекты зелья (вики, Potion: таблица «Effects»; сроки в тактах). У
/// ветряного, ткущего, слизистого и заражающего эффектов сервер пока не
/// считает — у таких зелий эффекта нет [проверить].
pub fn potion_effects(number: u16) -> Vec<Effect> {
    use EffectKind::*;

    let effect = |kind, amplifier, ticks| Effect { kind, amplifier, ticks_left: ticks, ambient: false, visible: true };
    let one = |kind, amplifier, ticks| vec![effect(kind, amplifier, ticks)];

    match POTIONS.get(usize::from(number)).copied().unwrap_or("water") {
        "night_vision" => one(NightVision, 0, 3600),
        "long_night_vision" => one(NightVision, 0, 9600),
        "invisibility" => one(Invisibility, 0, 3600),
        "long_invisibility" => one(Invisibility, 0, 9600),
        "leaping" => one(JumpBoost, 0, 3600),
        "long_leaping" => one(JumpBoost, 0, 9600),
        "strong_leaping" => one(JumpBoost, 1, 1800),
        "fire_resistance" => one(FireResistance, 0, 3600),
        "long_fire_resistance" => one(FireResistance, 0, 9600),
        "swiftness" => one(Speed, 0, 3600),
        "long_swiftness" => one(Speed, 0, 9600),
        "strong_swiftness" => one(Speed, 1, 1800),
        "slowness" => one(Slowness, 0, 1800),
        "long_slowness" => one(Slowness, 0, 4800),
        "strong_slowness" => one(Slowness, 3, 400),
        "turtle_master" => vec![effect(Slowness, 3, 400), effect(Resistance, 2, 400)],
        "long_turtle_master" => vec![effect(Slowness, 3, 800), effect(Resistance, 2, 800)],
        "strong_turtle_master" => vec![effect(Slowness, 5, 400), effect(Resistance, 3, 400)],
        "water_breathing" => one(WaterBreathing, 0, 3600),
        "long_water_breathing" => one(WaterBreathing, 0, 9600),
        "healing" => one(InstantHealth, 0, 1),
        "strong_healing" => one(InstantHealth, 1, 1),
        "harming" => one(InstantDamage, 0, 1),
        "strong_harming" => one(InstantDamage, 1, 1),
        "poison" => one(Poison, 0, 900),
        "long_poison" => one(Poison, 0, 1800),
        "strong_poison" => one(Poison, 1, 432),
        "regeneration" => one(Regeneration, 0, 900),
        "long_regeneration" => one(Regeneration, 0, 1800),
        "strong_regeneration" => one(Regeneration, 1, 450),
        "strength" => one(Strength, 0, 3600),
        "long_strength" => one(Strength, 0, 9600),
        "strong_strength" => one(Strength, 1, 1800),
        "weakness" => one(Weakness, 0, 1800),
        "long_weakness" => one(Weakness, 0, 4800),
        "luck" => one(Luck, 0, 6000),
        "slow_falling" => one(SlowFalling, 0, 1800),
        "long_slow_falling" => one(SlowFalling, 0, 4800),
        _ => Vec::new(),
    }
}

/// Действующий эффект.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    pub kind: EffectKind,
    pub amplifier: i32,
    /// Осталось тактов; `INFINITE` — бессрочный.
    pub ticks_left: i32,
    /// Фоновый (от маяка; `ambient` в пакете Entity Effect).
    pub ambient: bool,
    /// Показывать частицы и значок (`effect give … <секунды> <уровень> true`
    /// прячет и то и другое: чёрный ящик, hunger-probe-a — flags=8).
    pub visible: bool,
}

/// Срок «бессрочно» (вики, Commands/effect: «infinite» — duration −1).
pub const INFINITE: i32 = -1;

/// Итог такта эффектов.
#[derive(Debug, Default)]
pub struct EffectTick {
    /// Удары от яда и иссушения.
    pub hurt: Vec<(f32, Source)>,
    /// Эффекты, у которых вышел срок.
    pub gone: Vec<EffectKind>,
}

impl Effect {
    /// Флаги пакета Entity Effect: 0x01 фоновый, 0x02 частицы, 0x04 значок,
    /// 0x08 плавное появление (у оригинала от команды стоит всегда: чёрный
    /// ящик, pvp-strength flags=14, hunger-probe-a flags=8; у еды два прогона
    /// оригинала дали 6 и 14 — golden-apple) [проверить].
    pub fn flags(&self) -> u8 {
        let mut flags = 0x08;

        if self.ambient {
            flags |= 0x01;
        }

        if self.visible {
            flags |= 0x02 | 0x04;
        }

        flags
    }
}

/// Состояние игрока, которое считает сервер.
#[derive(Clone, Debug)]
pub struct Vitals {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    /// Счётчик лечения и голода (вики, Hunger: «food tick timer»).
    food_timer: i32,

    /// Запас воздуха в тактах.
    pub air: i32,
    /// Сколько тактов ещё горит (0 — не горит).
    pub fire: i32,

    /// Такты неуязвимости после удара: пока больше 10, приходит только
    /// разница между ударами.
    pub invulnerable: i32,
    /// Самый сильный удар за время неуязвимости.
    last_hurt: f32,

    /// Накопленная высота падения.
    pub fall: f32,
    /// Сокрушающий удар булавы прощает ближайшее приземление: оригинал не
    /// ранит после смаша с 3,2 блока (чёрный ящик, pvp-mace-heavy; вики,
    /// Mace: «nullifying any fall damage») [проверить: предел].
    pub smash_forgives: bool,

    /// Высота падения до последнего шага приземления (для частиц), 0 — не
    /// приземлялся в этом шаге.
    pub landed_from: f32,
    /// Весь пролёт до приземления, с последним шагом (для частиц после булавы).
    pub landed_total: f32,

    /// Игрок мёртв и ждёт возрождения.
    pub dead: bool,

    /// Бежит ли игрок (по пакету Player Command).
    pub sprinting: bool,

    /// Опыт: всего очков (по нему считаются уровень и шкала).
    pub xp: i32,

    /// Семя зачарования (`XpSeed`): по нему стол предлагает зачарования
    /// (enchanting.rs). 0 — ещё не заведено: заводится при входе.
    pub xp_seed: i32,

    /// Действующие эффекты.
    pub effects: Vec<Effect>,

    /// Поглощение (золотые сердца).
    pub absorption: f32,
    /// Наибольшее здоровье с учётом прироста (атрибут max_health).
    pub max_health: f32,
    /// Безопасная высота падения с учётом прыгучести (safe_fall_distance).
    pub safe_fall: f32,

    /// Возраст сущности в тактах: от него зависит лечение при мирной
    /// сложности (раз в 20 тактов).
    pub age: u32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self::new()
    }
}

impl Vitals {
    /// Частицы эффектов (ARGB): по одной на эффект, в порядке `particle_rank`;
    /// нули в хвосте.
    pub fn effect_colors(&self) -> [i32; EFFECT_PARTICLES] {
        let mut out = [0; EFFECT_PARTICLES];

        let mut kinds: Vec<EffectKind> = self.effects.iter().filter(|effect| effect.visible).map(|effect| effect.kind).collect();

        kinds.sort_by_key(|kind| kind.particle_rank());

        for (slot, kind) in out.iter_mut().zip(kinds) {
            *slot = (0xFF00_0000u32 | kind.color() as u32) as i32;
        }

        out
    }

    /// Свойство 11 «эффекты фоновые»: None — эффектов нет, свойство не
    /// трогается (чёрный ящик, hunger-probe-a: по истечении скрытого эффекта
    /// 11 не сбрасывается); иначе — все видимые эффекты фоновые (при одних
    /// скрытых — true: там же, Metadata 11=true на +1).
    pub fn effects_ambient(&self) -> Option<bool> {
        if self.effects.is_empty() {
            return None;
        }

        Some(self.effects.iter().filter(|effect| effect.visible).all(|effect| effect.ambient))
    }

    /// Уровень сопротивления (0 — нет эффекта).
    pub fn resistance_level(&self) -> i32 {
        self.effect_level(EffectKind::Resistance)
    }

    /// Уровень эффекта (0 — нет эффекта).
    pub fn effect_level(&self, kind: EffectKind) -> i32 {
        self.effects.iter().find(|effect| effect.kind == kind).map_or(0, |effect| effect.amplifier + 1)
    }

    pub fn has_effect(&self, kind: EffectKind) -> bool {
        self.effects.iter().any(|effect| effect.kind == kind)
    }

    /// Новорождённый: здоровье и еда полные, насыщение 5 (вики, Hunger).
    pub fn new() -> Self {
        Self {
            health: MAX_HEALTH,
            food: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            food_timer: 0,
            air: MAX_AIR,
            fire: 0,
            invulnerable: 0,
            last_hurt: 0.0,
            fall: 0.0,
            smash_forgives: false,
            landed_from: 0.0,
            landed_total: 0.0,
            dead: false,
            sprinting: false,
            xp: 0,
            xp_seed: 0,
            effects: Vec::new(),
            absorption: 0.0,
            max_health: MAX_HEALTH,
            safe_fall: SAFE_FALL,
            age: 0,
        }
    }

    /// Что уходит в файл игрока.
    pub fn saved(&self) -> crate::playerdata::SavedVitals {
        crate::playerdata::SavedVitals {
            health: self.health,
            food: self.food,
            saturation: self.saturation,
            exhaustion: self.exhaustion,
            air: self.air,
            fire: self.fire,
            fall: self.fall,
            experience: self.xp,
            xp_seed: self.xp_seed,
        }
    }

    /// Состояние из файла игрока. Погибший игрок (здоровье 0) заходит уже
    /// возрождённым: экран смерти после перезапуска не показывается [проверить].
    pub fn restored(saved: &crate::playerdata::SavedVitals) -> Self {
        if saved.health <= 0.0 {
            return Self::new();
        }

        Self {
            health: saved.health.min(MAX_HEALTH),
            food: saved.food.clamp(0, 20),
            saturation: saved.saturation.min(saved.food as f32),
            exhaustion: saved.exhaustion,
            air: saved.air.min(MAX_AIR),
            fire: saved.fire,
            fall: saved.fall,
            xp: saved.experience,
            xp_seed: saved.xp_seed,
            ..Self::new()
        }
    }

    /// Удар. `immune` — игрок неуязвим по режиму (творческий, наблюдатель).
    /// Возвращает исход или None, если удар не прошёл (вики, Damage:
    /// «Invulnerability frames»).
    pub fn hurt(&mut self, amount: f32, source: Source, immune: bool) -> Option<Hit> {
        if self.dead || amount <= 0.0 {
            return None;
        }

        // Сопротивление (вики, Resistance): −20 % за уровень, кроме голода,
        // пустоты и команды kill; на пятом уровне урон пропадает. Удар при этом
        // всё равно состоялся: у оригинала робот с сопротивлением V получает
        // событие урона, анимацию и отдачу, здоровье не меняется (чёрный ящик,
        // silverfish-chase) [проверить: у остальных источников].
        let amount = after_resistance(amount, self.resistance_level(), source);

        if immune && !source.bypasses_invulnerability() {
            return None;
        }

        let taken;

        if self.invulnerable > 10 && !source.bypasses_cooldown() {
            if amount <= self.last_hurt {
                return None;
            }

            taken = amount - self.last_hurt;
            self.last_hurt = amount;
        } else {
            self.last_hurt = amount;
            self.invulnerable = 20;
            taken = amount;
        }

        // Поглощение принимает удар первым (вики, Absorption); голод и
        // пустота его обходят (bypasses_effects), как и сопротивление [проверить].
        let absorbed = if matches!(source, Source::Starve | Source::OutOfWorld | Source::GenericKill) { 0.0 } else { taken.min(self.absorption) };

        self.absorption -= absorbed;
        self.health -= taken - absorbed;

        if taken > 0.0 {
            self.add_exhaustion(source.exhaustion());
        }

        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
        }

        Some(Hit { taken, died: self.dead })
    }

    /// Самый сильный удар за время неуязвимости (нападающему видно, пройдёт ли его удар).
    pub fn last_hurt(&self) -> f32 {
        self.last_hurt
    }

    /// Изнурение растёт от бега, прыжков и ударов; сверх 4 съедает насыщение.
    pub fn add_exhaustion(&mut self, amount: f32) {
        self.exhaustion = (self.exhaustion + amount).min(40.0);
    }

    /// Лечение, не выше максимума.
    pub fn heal(&mut self, amount: f32) {
        if self.dead {
            return;
        }

        self.health = (self.health + amount).min(self.max_health);
    }

    /// Еда с такой питательностью и насыщением (в очках, по таблице вики):
    /// насыщение не выше еды (вики, Food).
    pub fn eat(&mut self, nutrition: i32, saturation: f32) {
        self.food = (self.food + nutrition).min(20);
        self.saturation = (self.saturation + saturation).min(self.food as f32);
    }

    /// Нужна ли еда: без голода обычную еду не едят.
    pub fn hungry(&self) -> bool {
        self.food < 20
    }

    /// Такт еды и лечения (вики, Hunger: «Hunger and healing»). `difficulty`
    /// — сложность: при мирной еда не убывает, при голоде на нуле урон
    /// зависит от неё. Возвращает удар, если голод бьёт в этот такт.
    pub fn food_tick(&mut self, difficulty: u8, regeneration: bool) -> Option<f32> {
        if self.exhaustion > 4.0 {
            self.exhaustion -= 4.0;

            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else if difficulty != PEACEFUL {
                self.food = (self.food - 1).max(0);
            }
        }

        let hurt = self.health > 0.0 && self.health < self.max_health;

        if regeneration && self.saturation > 0.0 && self.food >= 20 && hurt {
            self.food_timer += 1;

            if self.food_timer >= 10 {
                let amount = self.saturation.min(6.0);

                self.heal(amount / 6.0);
                self.add_exhaustion(amount);
                self.food_timer = 0;
            }
        } else if regeneration && self.food >= 18 && hurt {
            self.food_timer += 1;

            if self.food_timer >= 80 {
                self.heal(1.0);
                self.add_exhaustion(6.0);
                self.food_timer = 0;
            }
        } else if self.food <= 0 {
            self.food_timer += 1;

            if self.food_timer >= 80 {
                self.food_timer = 0;

                // Вики, Hunger: лёгкая — до 10 здоровья, нормальная — до 1,
                // трудная — насмерть.
                if self.health > 10.0 || difficulty == HARD || (difficulty == NORMAL && self.health > 1.0) {
                    return Some(1.0);
                }
            }
        } else {
            self.food_timer = 0;
        }

        None
    }

    /// Такт эффектов: голод копит изнурение, насыщение кормит, регенерация
    /// лечит, яд и иссушение бьют. Возвращает, кого бить и у кого вышел срок.
    pub fn effects_tick(&mut self) -> EffectTick {
        let mut gone = Vec::new();
        let mut hurt = Vec::new();
        let mut exhaustion = 0.0f32;
        let mut fed = 0;
        let mut heal = 0.0f32;
        let age = self.age;
        let health = self.health;
        let absorption = self.absorption;

        for effect in &mut self.effects {
            // Период считается по оставшемуся сроку (вики, Regeneration:
            // «every 50 ≫ amplifier ticks»); у бессрочного — по возрасту [проверить].
            let due = |period: i32| {
                let period = period >> effect.amplifier.min(30);
                let counter = if effect.ticks_left == INFINITE { age as i32 } else { effect.ticks_left };

                period <= 0 || counter % period == 0
            };

            match effect.kind {
                EffectKind::Hunger => exhaustion += 0.005 * (effect.amplifier + 1) as f32,
                EffectKind::Saturation => fed += effect.amplifier + 1,
                EffectKind::Regeneration if due(50) => heal += 1.0,
                // Мгновенные: срок 1, действуют в такте наложения и тут же
                // снимаются (effect-instant: Effect dur=1, Remove в том же такте).
                EffectKind::InstantHealth => heal += (4 << effect.amplifier.min(20)) as f32,
                EffectKind::InstantDamage => hurt.push(((6 << effect.amplifier.min(20)) as f32, Source::Magic)),
                // Поглощение снимается, когда золотые сердца кончились
                // (effect-absorption: Remove Effect на такт после удара).
                EffectKind::Absorption if absorption <= 0.0 => effect.ticks_left = 1,
                EffectKind::Poison if due(25) && health > 1.0 => hurt.push((1.0, Source::Magic)),
                EffectKind::Wither if due(40) => hurt.push((1.0, Source::Wither)),
                _ => {}
            }

            if effect.ticks_left != INFINITE {
                effect.ticks_left -= 1;

                if effect.ticks_left <= 0 {
                    gone.push(effect.kind);
                }
            }
        }

        self.effects.retain(|effect| effect.ticks_left == INFINITE || effect.ticks_left > 0);

        if exhaustion > 0.0 {
            self.add_exhaustion(exhaustion);
        }

        if fed > 0 {
            self.food = (self.food + fed).min(20);
            self.saturation = (self.saturation + fed as f32).min(self.food as f32);
        }

        if heal > 0.0 {
            self.heal(heal);
        }

        EffectTick { hurt, gone }
    }

    /// Заменит ли новый эффект прежний того же вида: слабее по уровню или
    /// не длиннее при том же уровне — нет (чёрный ящик, effect-speed: повторный
    /// speed 0 при действующем speed 1 молчит; скрытые эффекты не хранятся [проверить]).
    pub fn effect_replaced_by(&self, kind: EffectKind, amplifier: i32, ticks: i32) -> bool {
        self.effects.iter().find(|effect| effect.kind == kind).is_none_or(|old| {
            amplifier > old.amplifier || (amplifier == old.amplifier && old.ticks_left != INFINITE && (ticks == INFINITE || ticks > old.ticks_left))
        })
    }

    /// Дать видимый эффект (новый заменяет прежний того же вида).
    #[cfg(test)]
    pub fn give_effect(&mut self, kind: EffectKind, amplifier: i32, ticks: i32) {
        self.give_effect_shown(Effect { kind, amplifier, ticks_left: ticks, ambient: false, visible: true });
    }

    /// Дать эффект со всеми признаками (новый заменяет прежний того же вида).
    pub fn give_effect_shown(&mut self, effect: Effect) {
        if !self.effect_replaced_by(effect.kind, effect.amplifier, effect.ticks_left) {
            return;
        }

        self.effects.retain(|old| old.kind != effect.kind);
        // У мгновенного срок 1 (effect-instant: dur=1 при любых секундах).
        self.effects.push(Effect { ticks_left: if effect.kind.instant() { 1 } else { effect.ticks_left }, ..effect });

        // Поглощение: золотые сердца по уровню, не меньше уже имеющихся
        // (вики, Absorption: «4 × level absorption health»).
        if effect.kind == EffectKind::Absorption {
            self.absorption = self.absorption.max(4.0 * (effect.amplifier + 1) as f32);
        }
    }

    /// Пересчёт после смены эффектов: наибольшее здоровье и поглощение
    /// подрезаются под атрибуты (у оригинала — при снятии модификатора).
    pub fn apply_limits(&mut self, max_health: f32, max_absorption: f32, safe_fall: f32) {
        self.max_health = max_health;
        self.safe_fall = safe_fall;
        self.health = self.health.min(max_health);
        self.absorption = self.absorption.min(max_absorption);
    }

    /// Такт неуязвимости после удара.
    pub fn tick_invulnerable(&mut self) {
        if self.invulnerable > 0 {
            self.invulnerable -= 1;
        }
    }

    /// Воздух под водой (вики, Drowning): уменьшается на 1 за такт; дойдя
    /// до −20, сбрасывается в 0 и бьёт на 2. Над водой растёт на 4.
    /// `breathes` — игрок не тонет (творческий режим, водное дыхание).
    /// Возвращает true, если пришло время удара.
    pub fn air_tick(&mut self, eyes_in_water: bool, breathes: bool) -> bool {
        if eyes_in_water && !breathes && !self.has_effect(EffectKind::WaterBreathing) {
            self.air -= 1;

            if self.air == DROWN_AT {
                self.air = 0;

                return true;
            }
        } else if self.air < MAX_AIR {
            self.air = (self.air + 4).min(MAX_AIR);
        }

        false
    }

    /// Поджечь на столько тактов (не сокращает уже идущий огонь).
    pub fn ignite(&mut self, ticks: i32) {
        if self.fire < ticks {
            self.fire = ticks;
        }
    }

    /// Огонь за такт: раз в 20 тактов бьёт на 1 вне лавы (вики, Fire).
    /// Возвращает true, если пора бить.
    pub fn fire_tick(&mut self, in_lava: bool) -> bool {
        if self.fire <= 0 {
            return false;
        }

        let hit = self.fire % 20 == 0 && !in_lava;

        self.fire -= 1;

        hit
    }

    /// Погасить.
    pub fn extinguish(&mut self) {
        self.fire = 0;
    }

    /// Движение по пакету клиента. `dy` — сдвиг по высоте за пакет,
    /// `on_ground` — признак «на земле» из пакета, `soft` — падение гасится
    /// средой (вода, паутина, лестница…), `multiplier` — множитель урона от
    /// блока внизу (сено и мёд 0,2, кровать 0,5). Возвращает урон при
    /// приземлении (вики, Fall damage: `ceil(distance − 3)`).
    pub fn land(&mut self, dy: f64, on_ground: bool, soft: bool, multiplier: f32) -> Option<f32> {
        self.landed_from = 0.0;
        self.landed_total = 0.0;

        if soft {
            self.fall = 0.0;

            return None;
        }

        let before = self.fall;

        // Высота копится в float, как у оригинала: от этого зависит, на какой
        // высоте округление даёт лишний пункт урона.
        if dy < 0.0 {
            self.fall -= dy as f32;
        }

        if !on_ground {
            return None;
        }

        let distance = std::mem::take(&mut self.fall);

        self.landed_from = before;
        self.landed_total = distance;

        if std::mem::take(&mut self.smash_forgives) {
            return None;
        }

        // Плавное падение урона не даёт (вики, Slow Falling) [проверить].
        let multiplier = if self.has_effect(EffectKind::SlowFalling) { 0.0 } else { multiplier };

        if distance > self.safe_fall && multiplier > 0.0 {
            let damage = ((distance - self.safe_fall) * multiplier).ceil();

            if damage > 0.0 {
                return Some(damage);
            }
        }

        None
    }

    /// Прыжок и бег отнимают силы (вики, Hunger, «Exhaustion»): прыжок 0,05,
    /// прыжок на бегу 0,2.
    pub fn jumped(&mut self) {
        self.add_exhaustion(if self.sprinting { 0.2 } else { 0.05 });
    }

    /// Бег по земле: 0,1 за метр; путь в сантиметрах округляется на каждом
    /// пакете, как у оригинала.
    pub fn ran(&mut self, horizontal: f64) {
        let centimetres = (horizontal as f32 * 100.0).round();

        if centimetres > 0.0 {
            self.add_exhaustion(0.1 * centimetres * 0.01);
        }
    }

    /// Очки опыта прибавляются (или убавляются, но не ниже нуля).
    pub fn add_xp(&mut self, amount: i32) {
        self.xp = self.xp.saturating_add(amount).max(0);
    }

    /// Команда `experience add|set … points|levels` (вики, Commands/experience):
    /// очки прибавляются к общему числу; уровни меняют уровень и сохраняют
    /// долю шкалы; `set … points` задаёт очки внутри текущего уровня.
    pub fn command_xp(&mut self, set: bool, amount: i32, levels: bool) {
        let (level, bar) = level_of(self.xp);
        let keep = |level: i32| total_at_level(level) + (bar * level_size(level) as f32) as i32;

        self.xp = match (set, levels) {
            (false, false) => self.xp.saturating_add(amount),
            (true, false) => total_at_level(level) + amount.clamp(0, level_size(level) - 1),
            (false, true) => keep((level + amount).max(0)),
            (true, true) => keep(amount.max(0)),
        }
        .max(0);
    }

    /// Возрождение: всё заново.
    pub fn respawned(&mut self) {
        let effects = std::mem::take(&mut self.effects);
        let xp_seed = self.xp_seed;

        // Семя зачарования переживает смерть [проверить].
        *self = Self { xp_seed, ..Self::new() };
        drop(effects);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_player_is_full() {
        let v = Vitals::new();

        assert_eq!((v.health, v.food, v.saturation, v.air), (20.0, 20, 5.0, 300));
    }

    #[test]
    fn a_probe_copy_predicts_the_real_hit() {
        // Шипы объявляются остальным игрокам по копии состояния, до применения
        // у самого игрока (чёрный ящик, pvp-thorns): исход должен совпасть.
        let mut real = Vitals::new();
        let mut probe = real.clone();
        let predicted = probe.hurt(3.2, Source::Thorns, false);
        let actual = real.hurt(3.2, Source::Thorns, false);

        assert!(predicted.is_some());
        assert_eq!(predicted.map(|hit| hit.taken), actual.map(|hit| hit.taken));
        assert_eq!(probe.health, real.health);
        // творческому шипы не вредят — объявлять нечего
        assert!(Vitals::new().hurt(3.2, Source::Thorns, true).is_none());
    }

    #[test]
    fn fall_damage_is_distance_minus_three_rounded_up() {
        let mut v = Vitals::new();

        assert_eq!(v.land(-3.0, true, false, 1.0), None);
        assert_eq!(v.land(-0.5, false, false, 1.0), None);
        assert_eq!(v.land(-7.5, false, false, 1.0), None);
        assert_eq!(v.land(0.0, true, false, 1.0), Some(5.0));
        // после приземления счёт с нуля
        assert_eq!(v.land(-0.1, true, false, 1.0), None);
    }

    #[test]
    fn mace_smash_forgives_the_next_landing_only() {
        // pvp-mace-heavy: после смаша с 3,2 блока урона нет; следующее
        // падение — как обычно.
        let mut v = Vitals::new();

        v.smash_forgives = true;
        assert_eq!(v.land(-3.2, false, false, 1.0), None);
        assert_eq!(v.land(0.0, true, false, 1.0), None);
        assert!(!v.smash_forgives);
        assert_eq!(v.land(-5.0, false, false, 1.0), None);
        assert_eq!(v.land(0.0, true, false, 1.0), Some(2.0));
    }

    #[test]
    fn landing_particles_follow_the_black_box_table() {
        // (высота до последнего шага, число) — записи оригинала.
        for (before, count) in [(3.346, 0), (4.063, 40), (4.844, 40), (5.688, 50), (12.002, 120), (14.554, 140), (28.515, 280), (35.895, 350), (37.843, 370), (39.830, 375), (100.0, 375)] {
            assert_eq!(landing_particles(before), count, "{before}");
        }
    }

    #[test]
    fn the_last_step_counts_for_damage_but_not_for_particles() {
        // tp с 30 блоков: до последнего шага 28,515, с ним 30 (урон 27, 280 частиц).
        let mut v = Vitals::new();

        v.land(-28.515, false, false, 1.0);
        assert_eq!(v.land(-1.485, true, false, 1.0), Some(27.0));
        assert_eq!(landing_particles(v.landed_from), 280);

        // в воздухе и на мягком блоке приземления нет
        assert_eq!(v.land(-0.1, false, false, 1.0), None);
        assert_eq!(v.landed_from, 0.0);
    }

    #[test]
    fn water_and_soft_blocks_reset_the_fall() {
        let mut v = Vitals::new();

        v.land(-10.0, false, false, 1.0);
        assert_eq!(v.land(-1.0, true, true, 1.0), None);
        assert_eq!(v.fall, 0.0);
    }

    #[test]
    fn hay_and_beds_soften_the_landing() {
        let mut v = Vitals::new();

        v.land(-13.0, false, false, 1.0);
        assert_eq!(v.land(0.0, true, false, 0.2), Some(2.0)); // ceil(10 · 0,2)

        v.land(-13.0, false, false, 1.0);
        assert_eq!(v.land(0.0, true, false, 0.0), None); // слизь не бьёт
    }

    #[test]
    fn a_second_hit_in_the_window_only_counts_the_excess() {
        let mut v = Vitals::new();

        assert_eq!(v.hurt(4.0, Source::Generic, false).map(|hit| hit.taken), Some(4.0));
        // слабее или равен — не проходит
        assert_eq!(v.hurt(4.0, Source::Generic, false), None);
        assert_eq!(v.hurt(3.0, Source::Generic, false), None);
        // сильнее — только разница
        assert_eq!(v.hurt(6.0, Source::Generic, false).map(|hit| hit.taken), Some(2.0));
        assert_eq!(v.health, 14.0);

        // окно 20 тактов; после 10-го бьёт заново
        for _ in 0..10 {
            v.tick_invulnerable();
        }

        assert_eq!(v.hurt(4.0, Source::Generic, false).map(|hit| hit.taken), Some(4.0));
        assert_eq!(v.health, 10.0);
    }

    #[test]
    fn lethal_hit_kills_and_keeps_the_dead_dead() {
        let mut v = Vitals::new();
        let hit = v.hurt(30.0, Source::Fall, false).expect("удар прошёл");

        assert!(hit.died && v.dead && v.health == 0.0);
        assert_eq!(v.hurt(1.0, Source::Generic, false), None);
    }

    #[test]
    fn creative_players_only_take_void_and_kill_damage() {
        let mut v = Vitals::new();

        assert_eq!(v.hurt(5.0, Source::Lava, true), None);
        assert_eq!(v.hurt(4.0, Source::OutOfWorld, true).map(|hit| hit.taken), Some(4.0));
    }

    #[test]
    fn damage_scales_with_difficulty() {
        assert_eq!(scaled(10.0, PEACEFUL), 0.0);
        assert_eq!(scaled(10.0, EASY), 6.0);
        assert_eq!(scaled(10.0, NORMAL), 10.0);
        assert_eq!(scaled(10.0, HARD), 15.0);
        assert_eq!(scaled(1.0, EASY), 1.0);
    }

    #[test]
    fn explosion_damage_follows_the_wiki_formula() {
        // impact 0,625, сила 4: (0,390625+0,625)/2·7·8+1 = 29,4
        assert!((explosion_damage(0.625, 4.0) - 29.4375).abs() < 1e-3);
        assert_eq!(explosion_damage(0.0, 4.0), 1.0);
    }

    #[test]
    fn exhaustion_eats_saturation_then_food() {
        let mut v = Vitals::new();

        v.add_exhaustion(4.5);
        v.food_tick(NORMAL, true);
        assert_eq!((v.saturation, v.food, v.exhaustion), (4.0, 20, 0.5));

        v.saturation = 0.0;
        v.add_exhaustion(4.0);
        v.food_tick(NORMAL, true);
        assert_eq!(v.food, 19);

        // мирная сложность еду не отнимает
        v.add_exhaustion(4.5);
        v.food_tick(PEACEFUL, true);
        assert_eq!(v.food, 19);
    }

    #[test]
    fn fast_regeneration_needs_full_food_and_saturation() {
        let mut v = Vitals::new();

        v.health = 10.0;
        v.saturation = 5.0;

        for _ in 0..9 {
            v.food_tick(NORMAL, true);
        }
        assert_eq!(v.health, 10.0);

        v.food_tick(NORMAL, true);
        // min(5, 6)/6 = 0,8333, изнурение +5
        assert!((v.health - 10.833_334).abs() < 1e-5, "{}", v.health);
        assert_eq!(v.exhaustion, 5.0);
    }

    #[test]
    fn slow_regeneration_at_eighteen_food_takes_four_seconds() {
        let mut v = Vitals::new();

        v.health = 10.0;
        v.food = 18;
        v.saturation = 0.0;

        for _ in 0..79 {
            v.food_tick(NORMAL, true);
        }
        assert_eq!(v.health, 10.0);

        v.food_tick(NORMAL, true);
        assert_eq!(v.health, 11.0);
    }

    #[test]
    fn starvation_depends_on_difficulty() {
        let run = |difficulty: u8, health: f32| {
            let mut v = Vitals::new();

            v.food = 0;
            v.saturation = 0.0;
            v.health = health;

            (0..80).find_map(|_| v.food_tick(difficulty, true))
        };

        assert_eq!(run(EASY, 15.0), Some(1.0));
        assert_eq!(run(EASY, 10.0), None);
        assert_eq!(run(NORMAL, 2.0), Some(1.0));
        assert_eq!(run(NORMAL, 1.0), None);
        assert_eq!(run(HARD, 1.0), Some(1.0));
    }

    #[test]
    fn eating_fills_food_and_saturation_up_to_food() {
        let mut v = Vitals::new();

        v.food = 19;
        v.saturation = 0.0;
        v.eat(5, 6.0); // хлеб
        assert_eq!((v.food, v.saturation), (20, 6.0));

        v.food = 10;
        v.saturation = 0.0;
        v.eat(8, 12.8); // жареная свинина: 12,8 → не выше еды 18
        assert_eq!(v.food, 18);
        assert!((v.saturation - 12.8).abs() < 1e-5);
    }

    #[test]
    fn air_runs_out_after_three_hundred_and_twenty_ticks() {
        let mut v = Vitals::new();
        let mut ticks = 0;

        while !v.air_tick(true, false) {
            ticks += 1;
        }

        assert_eq!(ticks + 1, 320);
        assert_eq!(v.air, 0);
    }

    #[test]
    fn air_comes_back_four_at_a_time() {
        let mut v = Vitals::new();

        v.air = 100;
        v.air_tick(false, false);
        assert_eq!(v.air, 104);

        v.air = 299;
        v.air_tick(false, false);
        assert_eq!(v.air, 300);
    }

    #[test]
    fn creative_players_do_not_drown() {
        let mut v = Vitals::new();

        for _ in 0..400 {
            assert!(!v.air_tick(true, true));
        }
    }

    #[test]
    fn fire_hurts_once_a_second_outside_lava() {
        let mut v = Vitals::new();

        v.ignite(LAVA_FIRE_TICKS);
        v.ignite(100);
        assert_eq!(v.fire, 300);

        assert!(v.fire_tick(false)); // 300 % 20 == 0
        assert!(!v.fire_tick(false));

        v.fire = 40;
        assert!(!v.fire_tick(true)); // в лаве не бьёт
        v.fire = 20;
        assert!(v.fire_tick(false));
    }

    #[test]
    fn regeneration_heals_by_the_remaining_duration() {
        let mut v = Vitals::new();

        v.health = 10.0;
        v.give_effect(EffectKind::Regeneration, 1, 60);

        // Остаток 50 и 25 делится на 25: два лечения за 60 тактов
        // (effect-regen: Set Health на +10 и +35).
        for _ in 0..60 {
            v.effects_tick();
        }

        assert_eq!(v.health, 12.0);
        assert!(v.effects.is_empty());
    }

    #[test]
    fn poison_stops_at_half_a_heart_and_wither_does_not() {
        let mut v = Vitals::new();

        v.health = 2.0;
        v.give_effect(EffectKind::Poison, 0, 100);

        let mut hits = 0;

        for _ in 0..100 {
            for (amount, source) in v.effects_tick().hurt {
                assert_eq!(source, Source::Magic);
                v.hurt(amount, source, false);
                v.invulnerable = 0;
                hits += 1;
            }
        }

        assert_eq!((hits, v.health), (1, 1.0));

        v.give_effect(EffectKind::Wither, 0, 80);

        let hurt: Vec<_> = (0..80).flat_map(|_| v.effects_tick().hurt).collect();

        assert_eq!(hurt, vec![(1.0, Source::Wither), (1.0, Source::Wither)]);
    }

    #[test]
    fn absorption_soaks_damage_first_and_starvation_ignores_it() {
        let mut v = Vitals::new();

        v.give_effect(EffectKind::Absorption, 1, 100);
        assert_eq!(v.absorption, 8.0);

        v.hurt(5.0, Source::Generic, false);
        assert_eq!((v.health, v.absorption), (20.0, 3.0));

        v.invulnerable = 0;
        v.hurt(5.0, Source::Generic, false);
        assert_eq!((v.health, v.absorption), (18.0, 0.0));

        // Кончившееся поглощение снимает эффект на следующем такте.
        assert_eq!(v.effects_tick().gone, vec![EffectKind::Absorption]);

        v.absorption = 4.0;
        v.invulnerable = 0;
        v.hurt(1.0, Source::Starve, false);
        assert_eq!((v.health, v.absorption), (17.0, 4.0));

        v.apply_limits(20.0, 0.0, 3.0);
        assert_eq!(v.absorption, 0.0);
    }

    #[test]
    fn hidden_effects_send_no_particles_and_count_as_ambient() {
        let mut v = Vitals::new();

        assert_eq!(v.effects_ambient(), None);

        let hidden = Effect { kind: EffectKind::Hunger, amplifier: 255, ticks_left: 60, ambient: false, visible: false };

        assert_eq!(hidden.flags(), 8);
        v.give_effect_shown(hidden);
        assert_eq!(v.effect_colors(), [0; EFFECT_PARTICLES]);
        assert_eq!(v.effects_ambient(), Some(true));

        v.give_effect(EffectKind::Speed, 0, 60);
        assert_eq!(v.effects.last().expect("скорость").flags(), 14);
        assert_eq!(v.effects_ambient(), Some(false));
    }

    #[test]
    fn instant_effects_heal_and_hurt_by_powers_of_two() {
        let mut v = Vitals::new();

        v.health = 10.0;
        v.give_effect(EffectKind::InstantHealth, 1, 20);
        assert_eq!(v.effects[0].ticks_left, 1);

        let tick = v.effects_tick();

        assert_eq!((v.health, tick.gone), (18.0, vec![EffectKind::InstantHealth]));

        v.give_effect(EffectKind::InstantDamage, 0, 20);
        assert_eq!(v.effects_tick().hurt, vec![(6.0, Source::Magic)]);
        assert!(v.effects.is_empty());
    }

    #[test]
    fn potions_follow_the_registry_and_the_wiki_table() {
        assert_eq!(POTIONS.len(), 46);
        assert_eq!(potion_number("strong_healing"), Some(25));
        assert_eq!(potion_number("infested"), Some(45));

        let healing = potion_effects(25);

        assert_eq!((healing[0].kind, healing[0].amplifier), (EffectKind::InstantHealth, 1));

        let turtle = potion_effects(potion_number("strong_turtle_master").unwrap());

        assert_eq!(turtle.iter().map(|effect| (effect.kind, effect.amplifier, effect.ticks_left)).collect::<Vec<_>>(), vec![(EffectKind::Slowness, 5, 400), (EffectKind::Resistance, 3, 400)]);
        assert!(potion_effects(0).is_empty());
    }

    #[test]
    fn infinite_effects_never_run_out() {
        let mut v = Vitals::new();

        v.give_effect(EffectKind::Speed, 0, INFINITE);

        for _ in 0..100 {
            assert!(v.effects_tick().gone.is_empty());
        }

        assert!(v.has_effect(EffectKind::Speed));
        assert!(!v.effect_replaced_by(EffectKind::Speed, 0, 600));
    }

    #[test]
    fn hunger_effect_accumulates_exhaustion() {
        let mut v = Vitals::new();

        v.give_effect(EffectKind::Hunger, 255, 20);

        for _ in 0..20 {
            v.effects_tick();
        }

        assert!(v.effects.is_empty());
        assert!((v.exhaustion - 25.6).abs() < 1e-3, "{}", v.exhaustion);
    }

    #[test]
    fn running_costs_a_tenth_per_metre() {
        let mut v = Vitals::new();

        v.ran(0.28);
        v.ran(0.28);
        assert!((v.exhaustion - 0.056).abs() < 1e-6);

        v.sprinting = true;
        v.jumped();
        assert!((v.exhaustion - 0.256).abs() < 1e-6);
    }

    #[test]
    fn vitals_round_trip_through_the_saved_form() {
        let mut v = Vitals::new();

        v.health = 7.5;
        v.food = 9;
        v.saturation = 0.0;
        v.exhaustion = 2.25;
        v.air = 120;
        v.fire = 60;
        v.fall = 4.5;

        let back = Vitals::restored(&v.saved());

        assert_eq!(back.saved(), v.saved());
        assert!(!back.dead && back.invulnerable == 0);
    }

    #[test]
    fn a_dead_player_is_restored_alive_and_saturation_stays_under_food() {
        let mut saved = Vitals::new().saved();

        saved.health = 0.0;
        assert_eq!(Vitals::restored(&saved).health, MAX_HEALTH);

        saved.health = 5.0;
        saved.food = 3;
        saved.saturation = 5.0;
        assert_eq!(Vitals::restored(&saved).saturation, 3.0);
    }

    #[test]
    fn levels_follow_the_wiki_formulas() {
        assert_eq!((level_size(0), level_size(15), level_size(16), level_size(30), level_size(31)), (7, 37, 42, 112, 121));
        assert_eq!((total_at_level(1), total_at_level(16), total_at_level(17), total_at_level(30), total_at_level(31), total_at_level(32)), (7, 352, 394, 1395, 1507, 1628));
        // Шаги между общими числами — как размеры уровней.
        for level in 0..60 {
            assert_eq!(total_at_level(level + 1) - total_at_level(level), level_size(level), "уровень {level}");
        }
    }

    #[test]
    fn total_points_give_level_and_bar() {
        // чёрный ящик, xp-orb: 21 очко — уровень 2, шкала 5/11 = 0,455
        let (level, bar) = level_of(21);

        assert_eq!(level, 2);
        assert!((bar - 0.4545).abs() < 1e-3, "{bar}");
        assert_eq!(level_of(0), (0, 0.0));
        assert_eq!(level_of(352).0, 16);
        assert_eq!(level_of(351).0, 15);
    }

    #[test]
    fn orbs_split_greedily() {
        assert_eq!(orb_values(7), vec![7]);
        assert_eq!(orb_values(5), vec![3, 1, 1]);
        assert_eq!(orb_values(2500), vec![2477, 17, 3, 3]);
        assert!(orb_values(0).is_empty());
        assert_eq!(orb_values(43).iter().sum::<i32>(), 43);
    }

    #[test]
    fn ores_give_the_wiki_amounts() {
        assert_eq!((ore_xp("coal_ore", 0.0), ore_xp("coal_ore", 0.99)), (0, 2));
        assert_eq!((ore_xp("diamond_ore", 0.0), ore_xp("deepslate_diamond_ore", 0.999)), (3, 7));
        assert_eq!((ore_xp("lapis_ore", 0.0), ore_xp("lapis_ore", 0.999)), (2, 5));
        assert_eq!((ore_xp("redstone_ore", 0.0), ore_xp("redstone_ore", 0.999)), (1, 5));
        assert_eq!((ore_xp("spawner", 0.0), ore_xp("spawner", 0.999)), (15, 43));
        assert_eq!((ore_xp("iron_ore", 0.5), ore_xp("stone", 0.5)), (0, 0));
    }

    #[test]
    fn the_experience_command_changes_points_and_levels() {
        let mut v = Vitals::new();

        v.command_xp(false, 21, false); // уровень 2, шкала 5/11
        assert_eq!(v.xp, 21);

        v.command_xp(false, 1, true); // уровень 3, доля сохраняется
        assert_eq!(level_of(v.xp).0, 3);

        v.command_xp(true, 0, false); // очки уровня — в ноль
        assert_eq!(v.xp, total_at_level(3));

        v.command_xp(true, 10, true);
        assert_eq!(v.xp, total_at_level(10));

        v.command_xp(false, -1000, false);
        assert_eq!(v.xp, 0);
    }

    #[test]
    fn passive_mobs_give_one_to_three() {
        assert_eq!((passive_mob_xp(0.0), passive_mob_xp(0.5), passive_mob_xp(0.999)), (1, 2, 3));
    }

    #[test]
    fn respawn_resets_everything() {
        let mut v = Vitals::new();

        v.hurt(30.0, Source::Fall, false);
        v.food = 3;
        v.respawned();

        assert_eq!((v.health, v.food, v.dead), (20.0, 20, false));
    }

    #[test]
    fn resistance_cuts_twenty_percent_per_level() {
        let mut v = Vitals::new();

        v.give_effect(EffectKind::Resistance, 1, 100);

        // Сопротивление II: −40 % (вики, Resistance).
        let hit = v.hurt(10.0, Source::MobAttack, false).expect("удар прошёл");

        assert!((hit.taken - 6.0).abs() < 1e-5, "{}", hit.taken);
        // Пустота и голод его обходят.
        assert_eq!(after_resistance(10.0, 2, Source::OutOfWorld), 10.0);
        assert_eq!(after_resistance(10.0, 2, Source::Starve), 10.0);
        // Пятый уровень гасит урон целиком.
        assert_eq!(after_resistance(10.0, 5, Source::Fall), 0.0);
        assert_eq!(after_resistance(7.0, 2, Source::Fall), 4.2);
    }

    #[test]
    fn resistance_five_still_counts_as_a_hit_without_damage() {
        // Чёрный ящик, silverfish-chase: укус чешуйницы по роботу с сопротивлением V —
        // событие урона и отдача есть, здоровье не меняется.
        let mut v = Vitals::new();

        v.give_effect(EffectKind::Resistance, 4, 100);

        let hit = v.hurt(1.0, Source::MobAttack, false).expect("удар состоялся");

        assert_eq!((hit.taken, hit.died, v.health), (0.0, false, 20.0));
        // Дальше действует неуязвимость после удара.
        assert!(v.hurt(1.0, Source::MobAttack, false).is_none());
    }

    #[test]
    fn combat_effects_name_their_modifiers() {
        let (attribute, id, amount, _) = EffectKind::Strength.modifier().expect("у силы есть модификатор");

        assert_eq!((attribute, id, amount), (crate::attributes::ATTACK_DAMAGE, "minecraft:effect.strength", 3.0));
        assert_eq!(EffectKind::from_name("minecraft:weakness"), Some(EffectKind::Weakness));
        assert!(EffectKind::Resistance.modifier().is_none());
        assert_eq!(EffectKind::Resistance.id(), 10);
    }

    #[test]
    fn effect_particles_follow_the_recorded_order() {
        let mut v = Vitals::new();

        for kind in [EffectKind::Speed, EffectKind::Slowness, EffectKind::Strength] {
            v.give_effect(kind, 0, 600);
        }

        // slowness, speed, strength (чёрный ящик, effect-speed).
        let colors = v.effect_colors();

        assert_eq!(&colors[..3], &[0xFF8BAFE0u32 as i32, 0xFF33EBFFu32 as i32, 0xFFFFC700u32 as i32]);
        assert_eq!(colors[3], 0);
    }

    #[test]
    fn a_weaker_effect_does_not_replace_a_stronger_one() {
        let mut v = Vitals::new();

        v.give_effect(EffectKind::Speed, 1, 600);

        assert!(!v.effect_replaced_by(EffectKind::Speed, 0, 600));
        assert!(!v.effect_replaced_by(EffectKind::Speed, 1, 600));
        assert!(v.effect_replaced_by(EffectKind::Speed, 1, 601));
        assert!(v.effect_replaced_by(EffectKind::Speed, 2, 10));
        assert!(v.effect_replaced_by(EffectKind::Weakness, 0, 10));
    }
}
