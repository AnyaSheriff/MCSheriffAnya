// Приручение: волк и кошка. Общая часть (хозяин, сидение, следование,
// защита хозяина, запись в файл) — для будущих лошади, попугая и лисы тоже.
//
// Кода игры здесь нет; поведение — со страниц minecraft.wiki (Wolf, Cat,
// Tameable mobs) и из чёрного ящика (сценарии wolf-tame, cat-tame,
// wolf-sit-follow, wolf-defend; зонды probe-follow, probe-cats):
//
// - приручение — щелчок едой: у волка кость, у кошки сырая треска или лосось;
//   еда тратится, шанс 1/3 (вики: «1⁄3 chance»); удача — событие сущности 7
//   (сердечки), неудача — 6 (дымок); прирученному свойства 18 = 4 и 19 = UUID
//   хозяина, у волка ещё здоровье 40 (свойство 9 и атрибут max_health);
// - приказ «сидеть» ставит хозяин щелчком (на дикого и чужого — нет); позу
//   (бит 1 свойства 18) ставит уже ИИ: у NoAI-волка щелчки позы не меняют
//   (чёрный ящик, wolf-sit-follow);
// - следование: дальше 10 блоков идёт к хозяину, дальше 12 — прыгает к нему
//   в случайную точку рядом (чёрный ящик, probe-follow: через 2 такта после
//   ухода хозяина, в 3 блоках по x и z);
// - защита: цель — тот, кого ударил хозяин, и тот, кто ударил хозяина;
//   прыжок на цель (вертикаль 0,4), укус 4; пока есть цель, свойство 22 —
//   «злость до» = время мира + 400…780, заново каждый такт (чёрный ящик,
//   wolf-defend).
//
// Что не сверено, помечено «[проверить]».

use super::ai::{self, Ctx, Plan};
use super::{path, Event, Kind, Mob, Rng, View};

/// Шанс приручения: 1 к стольким (вики, Wolf и Cat: «1⁄3 chance»).
pub const TAME_CHANCE: u32 = 3;

/// Здоровье приручённого волка (вики, Wolf: «40»; чёрный ящик, wolf-tame: 9=40).
pub const TAME_HEALTH: f32 = 40.0;

/// Цвет ошейника по умолчанию — красный (вики, Wolf: «red collar»; чёрный
/// ящик, probe-cats: CollarColor 3 даёт 21=3, призванный красный не шлёт).
pub const RED: u8 = 14;

/// Варианты волка и кошки — по алфавиту, как номера реестров у оригинала
/// (чёрный ящик, probe-cats: ashen 0, у кошек all_black 0, tabby 9, white 10).
pub const WOLF_VARIANTS: [&str; 9] = ["ashen", "black", "chestnut", "pale", "rusty", "snowy", "spotted", "striped", "woods"];
pub const CAT_VARIANTS: [&str; 11] = ["all_black", "black", "british_shorthair", "calico", "jellie", "persian", "ragdoll", "red", "siamese", "tabby", "white"];

/// Вариант по умолчанию: волк — бледный, кошка — чёрная (чёрный ящик,
/// probe-cats: призванная с variant black свойство 20 не шлёт).
pub const WOLF_DEFAULT: u8 = 3;
pub const CAT_DEFAULT: u8 = 1;

/// Следование за хозяином: дальше этого идёт к нему, ближе этого
/// останавливается, дальше этого прыгает (вики, Wolf: «10 blocks», «2»,
/// «12 blocks»).
pub const FOLLOW_START: f64 = 10.0;
pub const FOLLOW_STOP: f64 = 2.0;
pub const TELEPORT: f64 = 12.0;

/// Укус волка (вики, Wolf: «Attack strength 4»; чёрный ящик, wolf-defend:
/// овца 7 → 3).
pub const BITE: f32 = 4.0;

/// Пауза между укусами, тактов (вики, Mob AI: «melee attack every 20 ticks»).
pub const BITE_COOLDOWN: i32 = 20;

/// Прыжок на цель: с 2–4 блоков, вверх 0,4, вбок 0,4 к цели (чёрный ящик,
/// wolf-defend: скорость после прыжка 0,218 / 0,314 — 0,4 после трения).
pub const LEAP_UP: f64 = 0.4;
pub const LEAP_SIDE: f64 = 0.4;

/// Злость: «до такта» = время мира + 400…780 (вики, Wolf: «20–39 seconds»;
/// чёрный ящик, wolf-defend: свойство 22 скачет в этих пределах каждый такт).
pub const ANGER_MIN: i64 = 400;
pub const ANGER_SPREAD: u32 = 381;

/// Стая: дикие волки в стольких блоках злятся на обидчика вместе (вики, Wolf:
/// «all wild wolves within … become hostile») [проверить: радиус].
pub const PACK_RANGE: f64 = 10.0;

/// Цель нападения.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Player(i32),
    Mob(i32),
}

/// Состояние приручаемого (у остальных мобов — пустое, кроме `hurt_by`).
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// Хозяин (UUID) и его номер сущности в этом измерении (0 — нет рядом).
    pub owner: Option<[u8; 16]>,
    pub owner_id: i32,

    /// Хозяин был далеко прошлый такт: прыжок к нему — на второй такт
    /// (чёрный ящик, probe-follow и wolf-sit-follow: через 2–4 такта после `tp`).
    pub owner_far: bool,

    /// Приказ сидеть (сохраняется) и поза сидя (бит 1 свойства 18, ставит ИИ).
    pub ordered_sit: bool,
    pub sitting: bool,

    /// Ошейник (цвет красителя) и вариант (номер в `WOLF_VARIANTS`/`CAT_VARIANTS`).
    pub collar: u8,
    pub variant: u8,

    /// Цель и «злость до такта мира» (свойство 22 у волка; 0 — не зол).
    pub target: Option<Target>,
    pub anger_until: i64,

    /// Время мира этого такта (для злости).
    pub clock: i64,

    /// Тактов до следующего укуса.
    pub cooldown: i32,

    /// В ближнем бою (после первого укуса, пока цель есть): бит 4 свойства 15
    /// (чёрный ящик, wolf-defend: 15=4 в такте после укуса).
    pub melee: bool,

    /// Волк выпрашивает: смотрит на игрока с костью или мясом (свойство 20).
    pub begging: bool,

    /// Броня волка: прочность (None — нет) [проверить: поглощение урона].
    pub armor: Option<i32>,

    /// Волк мокрый (был в воде): выйдя, отряхивается (событие 8).
    pub wet: bool,

    /// Кто ударил этого моба в этом такте (номер сущности, 0 — никто): по
    /// нему волки хозяина выбирают цель. Ставит `on_hurt`, снимает `step_all`.
    pub hurt_by: i32,

    /// Кошка: блок, на который идёт сесть (сундук, подножие кровати, горящая
    /// печь), и сидит ли уже на нём (поза по ИИ, не по приказу).
    pub perch: Option<(i32, i32, i32)>,
    pub perched: bool,

    /// Тактов осталось искать путь к блоку.
    pub perch_wait: i32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            owner: None,
            owner_id: 0,
            owner_far: false,
            ordered_sit: false,
            sitting: false,
            collar: RED,
            variant: 0,
            target: None,
            anger_until: 0,
            clock: 0,
            cooldown: 0,
            melee: false,
            begging: false,
            armor: None,
            wet: false,
            hurt_by: 0,
            perch: None,
            perched: false,
            perch_wait: 0,
        }
    }
}

impl State {
    pub fn tamed(&self) -> bool {
        self.owner.is_some()
    }

    /// Флаги свойства 18: 1 — сидит, 4 — приручён. Злость у волка — не бит,
    /// а свойство 22 (чёрный ящик, wolf-defend: 18 не меняется).
    pub fn flags(&self) -> u8 {
        u8::from(self.sitting) | (u8::from(self.tamed()) << 2)
    }
}

/// Вариант по умолчанию для вида.
pub fn default_variant(kind: Kind) -> u8 {
    if kind == Kind::Cat { CAT_DEFAULT } else { WOLF_DEFAULT }
}

/// Номер варианта по имени (`minecraft:` можно опустить).
pub fn variant_named(kind: Kind, name: &str) -> Option<u8> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    let list: &[&str] = if kind == Kind::Cat { &CAT_VARIANTS } else { &WOLF_VARIANTS };

    list.iter().position(|entry| *entry == name).map(|index| index as u8)
}

/// Вариант волка по биому появления (вики, Wolf, «Variants»): тайга — бледный,
/// лес — лесной, заснеженная тайга — пепельный, старовозрастная сосновая
/// тайга — чёрный, еловая — каштановый, редкие джунгли — рыжий, плато
/// саванны — пятнистый, лесистые бесплодные земли — полосатый, роща — снежный.
pub fn wolf_variant_for(biome: &str) -> u8 {
    let name = match biome.strip_prefix("minecraft:").unwrap_or(biome) {
        "forest" => "woods",
        "snowy_taiga" => "ashen",
        "old_growth_pine_taiga" => "black",
        "old_growth_spruce_taiga" => "chestnut",
        "sparse_jungle" => "rusty",
        "savanna_plateau" => "spotted",
        "wooded_badlands" => "striped",
        "grove" => "snowy",
        _ => "pale",
    };

    variant_named(Kind::Wolf, name).unwrap_or(WOLF_DEFAULT)
}

/// Где появляются волки и стаей скольких (вики, Wolf, «Spawning»: вес и
/// размер группы) [проверить: веса].
pub fn wolf_spawn(biome: &str) -> Option<(u32, u32, u32)> {
    Some(match biome.strip_prefix("minecraft:").unwrap_or(biome) {
        "forest" => (5, 4, 4),
        "taiga" | "snowy_taiga" | "old_growth_pine_taiga" | "old_growth_spruce_taiga" | "grove" => (8, 4, 4),
        "sparse_jungle" => (8, 2, 4),
        "savanna_plateau" | "wooded_badlands" => (8, 4, 8),
        _ => return None,
    })
}

/// Еда волка: мясо и гнилая плоть; питательность — столько же здоровья
/// вернёт (вики, Wolf: «heals … by the food's hunger value») [проверить].
pub fn wolf_food(item: &str) -> Option<i32> {
    Some(match item {
        "beef" | "porkchop" | "rabbit" => 3,
        "cooked_beef" | "cooked_porkchop" => 8,
        "chicken" | "mutton" => 2,
        "cooked_chicken" | "cooked_mutton" => 6,
        "cooked_rabbit" => 5,
        "rotten_flesh" => 4,
        _ => return None,
    })
}

/// Еда кошки: сырая треска и лосось (вики, Cat: «raw cod or raw salmon»).
pub fn cat_food(item: &str) -> Option<i32> {
    Some(match item {
        "cod" | "salmon" => 2,
        _ => return None,
    })
}

/// Цвет красителя по имени предмета (`<цвет>_dye`).
pub fn dye_colour(item: &str) -> Option<u8> {
    let colour = item.strip_suffix("_dye")?;

    super::WOOL.iter().position(|wool| wool.strip_suffix("_wool") == Some(colour)).map(|index| index as u8)
}

/// Голос по состоянию (вики, Wolf и Cat, «Sounds»): злой волк рычит, раненый
/// скулит, прирученная кошка мурлычет, бродячая — своё.
pub fn sounds(mob: &Mob) -> (&'static str, &'static str, &'static str) {
    let (_, hurt, death) = mob.kind.sounds();
    let ambient = match mob.kind {
        Kind::Wolf if mob.tame.anger_until > mob.tame.clock => "minecraft:entity.wolf.growl",
        Kind::Wolf if mob.tame.tamed() && mob.vitals.health < 20.0 => "minecraft:entity.wolf.whine",
        Kind::Wolf => "minecraft:entity.wolf.ambient",
        Kind::Cat if mob.tame.tamed() => "minecraft:entity.cat.ambient",
        _ => "minecraft:entity.cat.stray_ambient",
    };

    (ambient, hurt, death)
}

/// Удар по мобу: кто ударил (для волков хозяина). Волк встаёт и мстит, дикий
/// злится (вики, Wolf: «attacking a wild wolf … makes it hostile»).
pub fn on_hurt(mob: &mut Mob, cause: i32) {
    mob.tame.hurt_by = cause;

    if mob.kind == Kind::Wolf && !mob.no_ai {
        mob.tame.ordered_sit = false;

        if Some(cause) != Some(mob.tame.owner_id).filter(|id| *id != 0) {
            mob.tame.target = Some(Target::Player(cause));
        }
    }

    if mob.kind == Kind::Cat {
        mob.tame.ordered_sit = false;
        // Удар сгоняет кошку с блока (вики, Cat: «Attacking it moves it»).
        mob.tame.perched = false;
        mob.tame.perch = None;
    }
}

/// Что сделал щелчок по приручаемому.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Used {
    /// Не про этого моба: щелчок идёт дальше по общим правилам.
    Pass,
    /// Предмет потрачен (в выживании — одна штука).
    Spent,
    /// Сработало, предмет цел.
    Kept,
}

/// Щелчок игрока `who` (UUID, номер) с предметом `held` по волку или кошке.
/// Порядок — по вики (Wolf, Cat, «Taming», «Feeding»): лечение едой, краситель
/// ошейника, броня волка, приказ сидеть; дикому — приручение.
pub fn interact(mob: &mut Mob, who: ([u8; 16], i32), held: Option<&str>, rng: &mut Rng) -> Used {
    if !matches!(mob.kind, Kind::Wolf | Kind::Cat) || mob.is_dead() {
        return Used::Pass;
    }

    let food = held.and_then(|item| if mob.kind == Kind::Wolf { wolf_food(item) } else { cat_food(item) });
    let owner = mob.tame.owner == Some(who.0);

    if !mob.tame.tamed() {
        let bait = if mob.kind == Kind::Wolf { held == Some("bone") } else { food.is_some() };

        // Злой волк кость не берёт (вики, Wolf: «cannot be tamed while angry»).
        if !bait || mob.tame.anger_until > mob.tame.clock {
            return Used::Pass;
        }

        if rng.below(TAME_CHANCE) == 0 {
            tame(mob, who);
            mob.events.push(Event::Status(7));
        } else {
            mob.events.push(Event::Status(6));
        }

        return Used::Spent;
    }

    // Еда: раненому — лечение, здоровому взрослому — любовь (чёрный ящик,
    // cat-tame: треска здоровой прирученной кошке — сердечки, событие 18).
    if let Some(points) = food {
        if mob.vitals.health < mob.vitals.max_health {
            mob.vitals.health = (mob.vitals.health + points as f32).min(mob.vitals.max_health);
            return Used::Spent;
        }

        if mob.age == 0 && mob.love == 0 {
            mob.love = super::LOVE_TICKS;
            mob.events.push(Event::Hearts);
            return Used::Spent;
        }

        if !owner {
            return Used::Kept;
        }
    }

    if !owner {
        return Used::Pass;
    }

    // Краситель: ошейник (вики, Wolf/Cat: «Using dye on a tamed wolf»).
    if let Some(colour) = held.and_then(dye_colour) {
        if colour == mob.tame.collar {
            return Used::Kept;
        }

        mob.tame.collar = colour;
        return Used::Spent;
    }

    // Броня волка: только взрослому, только хозяин (вики, Wolf Armor).
    if mob.kind == Kind::Wolf && held == Some("wolf_armor") && mob.tame.armor.is_none() && !mob.is_baby() {
        mob.tame.armor = Some(ARMOR_DURABILITY);
        mob.events.push(Event::Sound { name: "minecraft:item.armor.equip_wolf", volume: 1.0, pitch: 1.0 });
        return Used::Spent;
    }

    // Приказ: сесть или встать; цель и путь бросаются.
    mob.tame.ordered_sit = !mob.tame.ordered_sit;
    mob.tame.target = None;
    mob.brain.stop();
    Used::Kept
}

/// Прочность брони волка (вики, Wolf Armor: «Durability 64»).
pub const ARMOR_DURABILITY: i32 = 64;

/// Приручить: хозяин, приказ сидеть, у волка — 40 здоровья (вики, Wolf и Cat;
/// чёрный ящик, wolf-tame: 9=40, 18=4, 19=UUID, атрибут max_health 40).
pub fn tame(mob: &mut Mob, who: ([u8; 16], i32)) {
    mob.tame.owner = Some(who.0);
    mob.tame.owner_id = who.1;
    mob.tame.ordered_sit = true;
    mob.tame.target = None;
    mob.tame.anger_until = 0;
    mob.persistent = true;
    mob.brain.stop();

    if mob.kind == Kind::Wolf {
        mob.vitals.max_health = TAME_HEALTH;
        mob.vitals.health = TAME_HEALTH;
        mob.attr_changes += 1;
    }
}

/// Номера сущностей хозяев: по списку игроков измерения (UUID, номер).
pub fn bind_owners(mobs: &mut [Mob], players: &[([u8; 16], i32)]) {
    for mob in mobs.iter_mut() {
        if let Some(owner) = mob.tame.owner {
            mob.tame.owner_id = players.iter().find(|(uuid, _)| *uuid == owner).map_or(0, |(_, id)| *id);
        }

        if let Some(super::leash::Holder::Player { uuid, id }) = mob.leash
            && let Some((_, now)) = players.iter().find(|(player, _)| *player == uuid)
            && *now != id
        {
            mob.leash = Some(super::leash::Holder::Player { id: *now, uuid });
        }
    }
}

/// Приказы хозяев за такт: кого хозяин ударил (`hurt_by` у жертв) и кто ударил
/// хозяина (`strikes`: напавший, игрок) — на тех кидаются его волки (вики, Wolf:
/// «attack any mob its owner attacks … or that attacks its owner»). Криперов и
/// своих не трогают [проверить: гаст].
pub fn orders(mobs: &mut [Mob], strikes: &[(i32, i32)]) {
    let victims: Vec<(i32, i32, Kind, Option<[u8; 16]>)> = mobs.iter().filter(|mob| mob.tame.hurt_by != 0).map(|mob| (mob.tame.hurt_by, mob.entity_id, mob.kind, mob.tame.owner)).collect();

    for mob in mobs.iter_mut() {
        mob.tame.hurt_by = 0;
    }

    for wolf in mobs.iter_mut().filter(|mob| mob.kind == Kind::Wolf && mob.tame.tamed() && mob.tame.owner_id != 0 && !mob.no_ai && !mob.is_dead()) {
        if wolf.tame.ordered_sit {
            continue;
        }

        for (attacker, victim, kind, owner) in &victims {
            if *attacker == wolf.tame.owner_id && *victim != wolf.entity_id && *kind != Kind::Creeper && *owner != wolf.tame.owner {
                wolf.tame.target = Some(Target::Mob(*victim));
            }
        }

        for (attacker, player) in strikes {
            if *player == wolf.tame.owner_id {
                wolf.tame.target = Some(Target::Mob(*attacker));
            }
        }
    }
}

/// Стая: дикий волк, ударенный игроком, зовёт диких в `PACK_RANGE` (вики, Wolf).
pub fn pack_anger(mobs: &mut [Mob]) {
    let angry: Vec<((f64, f64, f64), Target)> = mobs
        .iter()
        .filter(|mob| mob.kind == Kind::Wolf && !mob.tame.tamed())
        .filter_map(|mob| mob.tame.target.filter(|target| matches!(target, Target::Player(_))).map(|target| ((mob.x, mob.y, mob.z), target)))
        .collect();

    for (at, target) in angry {
        for mate in mobs.iter_mut().filter(|mob| mob.kind == Kind::Wolf && !mob.tame.tamed() && !mob.no_ai && mob.tame.target.is_none()) {
            if (mate.x - at.0).powi(2) + (mate.y - at.1).powi(2) + (mate.z - at.2).powi(2) <= PACK_RANGE * PACK_RANGE {
                mate.tame.target = Some(target);
            }
        }
    }
}

/// Мысль волка и кошки: сидение, нападение, следование за хозяином; иначе —
/// обычная мысль животного (ai::think).
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();

    // Позу сидя ставит ИИ по приказу (чёрный ящик, wolf-sit-follow: у NoAI
    // поза не меняется).
    if mob.kind == Kind::Cat {
        perch_check(mob, view);
    }

    mob.tame.sitting = (mob.tame.ordered_sit && mob.tame.tamed()) || mob.tame.perched;
    mob.tame.cooldown = (mob.tame.cooldown - 1).max(0);

    shake(mob, view);

    if mob.tame.sitting {
        mob.brain.stop();
        mob.tame.target = None;
        mob.tame.melee = false;
        ai::voice(mob, rng);
        return plan;
    }

    // Ручная кошка лезет на сундук, подножие кровати или горящую печь рядом и
    // садится (вики, Cat: «attempts to climb onto a chest, the foot of a bed, or an active furnace»).
    if mob.kind == Kind::Cat
        && mob.tame.tamed()
        && let Some(done) = seek_perch(mob, view, plan.clone(), rng)
    {
        return done;
    }

    // Выпрашивание: игрок в 8 блоках с костью или мясом (вики, Wolf: «begging»).
    if mob.kind == Kind::Wolf {
        mob.tame.begging = ctx.players.iter().any(|player| {
            !player.spectator
                && (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2) <= 64.0
                && player.holding.iter().flatten().any(|item| *item == "bone" || (mob.tame.tamed() && wolf_food(item).is_some()))
        });
    }

    // Цель: живая и в 32 блоках — нападаем; иначе забываем.
    if mob.kind == Kind::Wolf
        && let Some(target) = mob.tame.target
    {
        let place = match target {
            Target::Player(id) => ctx.players.iter().find(|player| player.id == id && player.targetable).map(|player| (player.x, player.y, player.z, 0.6)),
            Target::Mob(id) => ctx.mates.iter().find(|mate| mate.id == id).map(|mate| (mate.x, mate.y, mate.z, mate.kind.width())),
        };

        match place {
            Some(at) if (at.0 - mob.x).powi(2) + (at.2 - mob.z).powi(2) <= 32.0 * 32.0 => {
                mob.tame.anger_until = mob.tame.clock + ANGER_MIN + i64::from(rng.below(ANGER_SPREAD));
                return attack(mob, view, target, at, plan, rng);
            }
            _ => {
                mob.tame.target = None;
                mob.tame.anger_until = 0;
                mob.tame.melee = false;
            }
        }
    }

    // Дикий волк охотится на овец (вики, Wolf: «attack sheep, rabbits, foxes»;
    // цель ищется с шансом 1/10 за такт) [проверить], на скелетов — всякий.
    if mob.kind == Kind::Wolf && rng.below(10) == 0 {
        let prey = ctx
            .mates
            .iter()
            .filter(|mate| (!mob.tame.tamed() && mate.kind == Kind::Sheep) || matches!(mate.kind, Kind::Skeleton | Kind::Stray | Kind::Bogged | Kind::WitherSkeleton))
            .map(|mate| (mate.id, (mate.x - mob.x).powi(2) + (mate.y - mob.y).powi(2) + (mate.z - mob.z).powi(2)))
            .filter(|(_, dist)| *dist <= 16.0 * 16.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((id, _)) = prey {
            mob.tame.target = Some(Target::Mob(id));
        }
    }

    // Следование за хозяином.
    if mob.tame.tamed()
        && let Some(owner) = ctx.players.iter().find(|player| player.id == mob.tame.owner_id && !player.spectator)
    {
        let dist = ((owner.x - mob.x).powi(2) + (owner.y - mob.y).powi(2) + (owner.z - mob.z).powi(2)).sqrt();
        let far = dist >= TELEPORT;

        // Прыжок к хозяину: далеко второй такт подряд.
        let was_far = std::mem::replace(&mut mob.tame.owner_far, far);

        if far && was_far && mob.leash.is_none() && teleport_near(mob, view, (owner.x, owner.y, owner.z), rng) {
            mob.tame.owner_far = false;
            return plan;
        }

        if dist >= FOLLOW_START {
            mob.brain.doing = ai::Doing::Follow;
            mob.brain.repath -= 1;

            if !mob.brain.walking() || mob.brain.repath <= 0 {
                mob.brain.repath = 10;
                let route = path::find(view, mob.cell(), (owner.x.floor() as i32, owner.y.floor() as i32, owner.z.floor() as i32), path::MAX_NODES);

                mob.brain.go(route, 1.0);
            }

            return ai::walk(mob, plan);
        }

        if dist <= FOLLOW_STOP && mob.brain.doing == ai::Doing::Follow {
            mob.brain.stop();
            mob.brain.doing = ai::Doing::Idle;
            plan.speed = 0.0;
            return plan;
        }
    }

    ai::think(mob, view, ctx, rng)
}

/// Что кошка кладёт в подарок: таблица cat_morning_gift (вики, Cat: кроличья лапка,
/// кроличья шкура, курятина, перо, гнилая плоть, нить — по 10; мембрана — 2).
pub fn morning_gift(rng: &mut Rng) -> &'static str {
    match rng.below(62) {
        0..=9 => "rabbit_hide",
        10..=19 => "rabbit_foot",
        20..=29 => "chicken",
        30..=39 => "feather",
        40..=49 => "rotten_flesh",
        50..=59 => "string",
        _ => "phantom_membrane",
    }
}

/// Годится ли блок кошке в сиденья: сундук, подножие кровати, горящая печь
/// (вики, Cat, «Sitting»).
pub fn perch_block(state: i32) -> bool {
    let Some(name) = crate::blocks::block_at_state(state) else {
        return false;
    };

    match name {
        "chest" | "trapped_chest" => true,
        "furnace" | "blast_furnace" | "smoker" => crate::blocks::orientation_of(state).and_then(|o| o.value(state, "lit")) == Some("true"),
        _ if name.ends_with("_bed") => crate::blocks::orientation_of(state).and_then(|o| o.value(state, "part")) == Some("foot"),
        _ => false,
    }
}

/// Сидящая на блоке кошка слезает, если блока нет или над ним что-то есть
/// (вики, Cat: «won't sit on a block that has another block above it»).
fn perch_check(mob: &mut Mob, view: &dyn View) {
    if !mob.tame.perched {
        return;
    }

    let (x, y, z) = (mob.x.floor() as i32, (mob.y - 0.1).floor() as i32, mob.z.floor() as i32);

    if !perch_block(view.block(x, y, z)) || mob.tame.ordered_sit {
        mob.tame.perched = false;
    }
}

/// Поиск сиденья в 4 блоках на том же уровне и ход к нему; Some — кошка занята
/// этим в такт (вики, Cat: «within a 4-block square radius»). Раз в ~400 тактов
/// [проверить: частота и дальность поиска у оригинала].
fn seek_perch(mob: &mut Mob, view: &dyn View, plan: Plan, rng: &mut Rng) -> Option<Plan> {
    if mob.tame.ordered_sit || mob.tame.perched {
        return None;
    }

    let here = mob.cell();

    // Уже стоит на сиденье — садится сразу (чёрный ящик, cat-chest-block: после
    // Sitting:0b кошка на сундуке села снова через 2 такта).
    if mob.on_ground && perch_block(view.block(here.0, here.1 - 1, here.2)) {
        mob.tame.perched = true;
        mob.tame.perch = None;
        mob.brain.stop();

        return Some(plan);
    }

    if mob.tame.perch.is_none() {
        if rng.below(400) != 0 {
            return None;
        }

        // Сиденье — блок на уровне ног; кошка встаёт на его верх.
        let found = (-4..=4)
            .flat_map(|dx| (-4..=4).map(move |dz| (here.0 + dx, here.1, here.2 + dz)))
            .filter(|&(x, y, z)| perch_block(view.block(x, y, z)) && view.block(x, y + 1, z) == 0)
            .min_by_key(|&(x, _, z)| (x - here.0).pow(2) + (z - here.2).pow(2));
        let block = found?;
        let route = path::find(view, here, (block.0, block.1 + 1, block.2), path::MAX_NODES);

        if route.is_empty() {
            return None;
        }

        mob.tame.perch = Some(block);
        mob.tame.perch_wait = 200;
        mob.brain.go(route, 0.8);
    }

    let (bx, by, bz) = mob.tame.perch?;
    mob.tame.perch_wait -= 1;

    // Блок исчез или время вышло — отказ.
    if mob.tame.perch_wait <= 0 || !perch_block(view.block(bx, by, bz)) {
        mob.tame.perch = None;
        mob.brain.stop();

        return None;
    }

    // Пришла: на верху блока — садится.
    if (mob.x - (f64::from(bx) + 0.5)).abs() < 0.7 && (mob.z - (f64::from(bz) + 0.5)).abs() < 0.7 && mob.y >= f64::from(by) + 0.5 {
        mob.tame.perch = None;
        mob.tame.perched = true;
        mob.tame.sitting = true;
        mob.brain.stop();

        return Some(plan);
    }

    if !mob.brain.walking() {
        mob.tame.perch = None;

        return None;
    }

    Some(ai::walk(mob, plan))
}

/// Нападение: идти к цели, с 2–4 блоков прыгнуть, вблизи укусить.
fn attack(mob: &mut Mob, view: &dyn View, target: Target, at: (f64, f64, f64, f64), mut plan: Plan, rng: &mut Rng) -> Plan {
    let (dx, dz) = (at.0 - mob.x, at.2 - mob.z);
    let flat = (dx * dx + dz * dz).sqrt();

    ai::look_at_point(mob, at.0, at.1 + 0.5, at.2);

    // Укус: ближе «двойной ширины» с поправкой на ширину цели (вики, Mob AI,
    // «Melee attack»: reach = (2·ширина)² + ширина цели).
    let reach = (2.0 * mob.kind.width()).powi(2) + at.3;

    if flat * flat + (at.1 - mob.y).powi(2) <= reach && mob.tame.cooldown == 0 {
        mob.tame.cooldown = BITE_COOLDOWN;
        mob.tame.melee = true;
        mob.events.push(Event::Swing);

        let push = if flat > 1.0e-6 { (dx / flat, dz / flat) } else { (0.0, 0.0) };

        match target {
            Target::Player(id) => plan.melee = Some(super::hostile::Melee { target: id, damage: BITE, push, wither: false, hunger: false }),
            Target::Mob(id) => plan.bite = Some((id, BITE, push)),
        }
    }

    // Прыжок на цель с земли, с 2–4 блоков, шанс 1/5 за такт (вики, Mob AI,
    // «Leap at target») [проверить].
    if mob.on_ground && (2.0..=4.0).contains(&flat) && rng.below(5) == 0 {
        mob.vx = dx / flat * LEAP_SIDE + mob.vx * 0.2;
        mob.vz = dz / flat * LEAP_SIDE + mob.vz * 0.2;
        mob.vy = LEAP_UP;
        mob.impulses += 1;
        mob.brain.stop();
        return plan;
    }

    if !mob.on_ground {
        return plan;
    }

    mob.brain.repath -= 1;

    if !mob.brain.walking() || mob.brain.repath <= 0 {
        mob.brain.repath = 4;
        let route = path::find(view, mob.cell(), (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32), path::MAX_NODES);

        mob.brain.go(route, 1.0);
    }

    ai::walk(mob, plan)
}

/// Прыжок к хозяину: случайная клетка в 3 блоках по x и z (не вплотную), по
/// высоте ±1, где можно стоять (вики, Wolf: «teleports to its owner»; чёрный
/// ящик, probe-follow: на 3 блока в сторону). 10 попыток.
fn teleport_near(mob: &mut Mob, view: &dyn View, owner: (f64, f64, f64), rng: &mut Rng) -> bool {
    let base = (owner.0.floor() as i32, owner.1.floor() as i32, owner.2.floor() as i32);

    for _ in 0..10 {
        let dx = rng.below(7) as i32 - 3;
        let dz = rng.below(7) as i32 - 3;
        let dy = rng.below(3) as i32 - 1;

        if dx.abs() < 2 && dz.abs() < 2 {
            continue;
        }

        let cell = (base.0 + dx, base.1 + dy, base.2 + dz);

        if !path::standable(view, cell) {
            continue;
        }

        mob.x = f64::from(cell.0) + 0.5;
        mob.y = f64::from(cell.1);
        mob.z = f64::from(cell.2) + 0.5;
        // Скорость вбок гаснет (чёрный ящик, probe-follow: сверка после прыжка
        // со скоростью 0, −0,078, 0).
        mob.vx = 0.0;
        mob.vz = 0.0;
        mob.teleports += 1;
        mob.brain.stop();
        return true;
    }

    false
}

/// Отряхивание: мокрый волк вышел на сушу — событие 8 (вики, Wolf: «shake
/// off water») [проверить: такт].
fn shake(mob: &mut Mob, view: &dyn View) {
    if mob.kind != Kind::Wolf {
        return;
    }

    let (x, y, z) = mob.cell();

    if super::is_water(view.block(x, y, z)) {
        mob.tame.wet = true;
    } else if mob.tame.wet && mob.on_ground {
        mob.tame.wet = false;
        mob.events.push(Event::Status(8));
    }
}

/// Запись для файла: хозяин, приказ, ошейник, вариант, броня.
pub fn write(state: &State, out: &mut Vec<u8>) {
    out.push(u8::from(state.owner.is_some()) | (u8::from(state.ordered_sit) << 1) | (u8::from(state.armor.is_some()) << 2));
    out.extend_from_slice(&state.owner.unwrap_or([0; 16]));
    out.push(state.collar);
    out.push(state.variant);
    out.extend_from_slice(&state.armor.unwrap_or(0).to_be_bytes());
}

/// Читает запись `write`; None — испорчена.
pub fn read(bytes: &mut &[u8]) -> Option<State> {
    let mut take = |count: usize| -> Option<&[u8]> {
        let (head, rest) = bytes.split_at_checked(count)?;
        *bytes = rest;
        Some(head)
    };

    let flags = take(1)?[0];
    let owner: [u8; 16] = take(16)?.try_into().ok()?;
    let collar = take(1)?[0];
    let variant = take(1)?[0];
    let armor = i32::from_be_bytes(take(4)?.try_into().ok()?);

    if collar > 15 {
        return None;
    }

    Some(State {
        owner: (flags & 1 != 0).then_some(owner),
        ordered_sit: flags & 2 != 0,
        armor: (flags & 4 != 0).then_some(armor),
        collar,
        variant,
        ..State::default()
    })
}

/// Кладёт прочитанное в моба: у прирученного волка — 40 здоровья.
pub fn apply_read(mob: &mut Mob, state: State) {
    mob.tame = state;

    if mob.kind == Kind::Wolf && mob.tame.tamed() {
        mob.vitals.max_health = TAME_HEALTH;
    } else {
        mob.vitals.health = mob.vitals.health.min(mob.kind.max_health());
    }
}

/// Данные команды `summon`/`data merge` (вики, Wolf и Cat, «Entity data»):
/// `Owner:[I;…]`, `Sitting:1b`, `CollarColor:3b`, `variant:"minecraft:…"`.
pub fn apply_data(mob: &mut Mob, data: &str) {
    if let Some(at) = data.find("Owner:[I;") {
        let ints: Vec<i32> = data[at + 9..].split(']').next().unwrap_or("").split(',').filter_map(|part| part.trim().parse::<i32>().ok()).collect();

        if ints.len() == 4 {
            let mut uuid = [0u8; 16];

            for (index, value) in ints.iter().enumerate() {
                uuid[index * 4..index * 4 + 4].copy_from_slice(&value.to_be_bytes());
            }

            mob.tame.owner = Some(uuid);
            mob.persistent = true;
        }
    }

    if let Some(rest) = data.split("Sitting:").nth(1) {
        mob.tame.ordered_sit = rest.starts_with('1') || rest.starts_with("true");
        // Поза ставится сразу, и у NoAI тоже (чёрный ящик, cat-chest-block: 18=5 в такт призыва).
        mob.tame.sitting = mob.tame.ordered_sit && mob.tame.tamed();
        mob.tame.perched = false;
    }

    if let Some(rest) = data.split("CollarColor:").nth(1)
        && let Ok(colour) = rest.trim_start().split(|c: char| !c.is_ascii_digit()).next().unwrap_or("").parse::<u8>()
    {
        mob.tame.collar = colour.min(15);
    }

    if let Some(rest) = data.split("variant:\"").nth(1)
        && let Some(variant) = rest.split('"').next().and_then(|name| variant_named(mob.kind, name))
    {
        mob.tame.variant = variant;
    }
}
