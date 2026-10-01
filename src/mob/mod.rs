// Мобы: живые существа мира — корова, свинья, овца, курица.
//
// Кода игры и чужих ядер здесь нет: правила взяты со страниц minecraft.wiki
// (Entity, Cow, Pig, Sheep, Chicken, Knockback, Damage, Breeding, Mob AI),
// размеры и номера видов — из minecraft-data (entities.json, 26.1), а числа,
// которых на вики нет, сняты с официального сервера чёрным ящиком —
// сценарии tools/blackbox/scenarios/cow-physics.json, sheep-shear.json,
// mob-plate.json, chicken-fall.json.
//
// Устройство такое же, как у прочих сущностей (items.rs, minecart.rs, frame.rs):
// склад мобов лежит в `Items` рядом с предметами, там же общий журнал, по
// которому подключения узнают о появлении, ударах и смерти; движение же
// клиентам рассылает конец такта (network/play.rs, `send_mob_ticks`).
//
// Что не сверено, помечено «[проверить]».

pub mod ai;
pub mod dragon;
pub mod hostile;
pub mod path;
pub mod spawn;

use crate::blocks;
use crate::inventory::Stack;
use crate::survival::{Hit, Source, Vitals};

/// Вид моба.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Cow,
    Pig,
    Sheep,
    Chicken,
    Zombie,
    Skeleton,
    Creeper,
    Spider,
    /// Кристалл Края: не живое существо, а «сущность-другое» (minecraft-data:
    /// end_crystal), но живёт в списке мобов ради слежения и ударов.
    EndCrystal,
    /// Дракон Края (вики, Ender Dragon).
    EnderDragon,
}

impl Kind {
    /// Все виды: для проверок.
    #[cfg(test)]
    pub const ALL: [Kind; 10] = [Kind::Cow, Kind::Pig, Kind::Sheep, Kind::Chicken, Kind::Zombie, Kind::Skeleton, Kind::Creeper, Kind::Spider, Kind::EndCrystal, Kind::EnderDragon];

    /// Враждебный ли вид (вики, Monster): у него свои цели (mob/hostile.rs).
    pub fn hostile(self) -> bool {
        matches!(self, Kind::Zombie | Kind::Skeleton | Kind::Creeper | Kind::Spider)
    }

    /// По имени сущности, как в командах.
    pub fn from_name(name: &str) -> Option<Kind> {
        Some(match name.strip_prefix("minecraft:").unwrap_or(name) {
            "cow" => Kind::Cow,
            "pig" => Kind::Pig,
            "sheep" => Kind::Sheep,
            "chicken" => Kind::Chicken,
            "zombie" => Kind::Zombie,
            "skeleton" => Kind::Skeleton,
            "creeper" => Kind::Creeper,
            "spider" => Kind::Spider,
            "end_crystal" => Kind::EndCrystal,
            "ender_dragon" => Kind::EnderDragon,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Cow => "cow",
            Kind::Pig => "pig",
            Kind::Sheep => "sheep",
            Kind::Chicken => "chicken",
            Kind::Zombie => "zombie",
            Kind::Skeleton => "skeleton",
            Kind::Creeper => "creeper",
            Kind::Spider => "spider",
            Kind::EndCrystal => "end_crystal",
            Kind::EnderDragon => "ender_dragon",
        }
    }

    /// Номер вида сущности для клиента Java (minecraft-data 26.1, entities.json).
    pub fn java_type(self) -> i32 {
        match self {
            Kind::Chicken => 26,
            Kind::Creeper => 32,
            Kind::Skeleton => 115,
            Kind::Spider => 124,
            Kind::Zombie => 150,
            Kind::Cow => 30,
            Kind::Pig => 100,
            Kind::Sheep => 111,
            Kind::EnderDragon => 43,
            Kind::EndCrystal => 45,
        }
    }

    /// Имя вида для клиента Bedrock (AddActor).
    pub fn bedrock_name(self) -> &'static str {
        match self {
            Kind::Cow => "minecraft:cow",
            Kind::Pig => "minecraft:pig",
            Kind::Sheep => "minecraft:sheep",
            Kind::Chicken => "minecraft:chicken",
            Kind::Zombie => "minecraft:zombie",
            Kind::Skeleton => "minecraft:skeleton",
            Kind::Creeper => "minecraft:creeper",
            Kind::Spider => "minecraft:spider",
            Kind::EndCrystal => "minecraft:ender_crystal",
            Kind::EnderDragon => "minecraft:ender_dragon",
        }
    }

    /// Ширина ящика (minecraft-data, entities.json).
    pub fn width(self) -> f64 {
        match self {
            Kind::Chicken => 0.4,
            Kind::Zombie | Kind::Skeleton | Kind::Creeper => 0.6,
            Kind::Spider => 1.4,
            Kind::EndCrystal => 2.0,
            Kind::EnderDragon => 16.0,
            _ => 0.9,
        }
    }

    /// Высота ящика взрослого (minecraft-data, entities.json).
    pub fn height(self) -> f64 {
        match self {
            Kind::Chicken => 0.7,
            Kind::Cow => 1.4,
            Kind::Pig => 0.9,
            Kind::Sheep => 1.3,
            Kind::Zombie => 1.95,
            Kind::Skeleton => 1.99,
            Kind::Creeper => 1.7,
            Kind::Spider => 0.9,
            Kind::EndCrystal => 2.0,
            Kind::EnderDragon => 8.0,
        }
    }

    /// Здоровье (вики: корова и свинья 10, овца 8, курица 4).
    pub fn max_health(self) -> f32 {
        match self {
            Kind::Cow | Kind::Pig => 10.0,
            Kind::Sheep => 8.0,
            Kind::Chicken => 4.0,
            Kind::Zombie | Kind::Skeleton | Kind::Creeper => 20.0,
            Kind::Spider => 16.0,
            // У кристалла здоровья нет: любой удар его взрывает (вики, End Crystal).
            Kind::EndCrystal => 1.0,
            // Вики, Ender Dragon: «Health: 200»; чёрный ящик, dragon-summon: 9=200.
            Kind::EnderDragon => 200.0,
        }
    }

    /// Скорость шага, блоков за такт при множителе 1 (вики, Cow/Pig/Sheep/
    /// Chicken, «Attributes»: 0,2; 0,25; 0,23; 0,25).
    pub fn speed(self) -> f64 {
        match self {
            Kind::Cow => 0.2,
            Kind::Pig | Kind::Chicken => 0.25,
            Kind::Sheep | Kind::Zombie => 0.23,
            Kind::Skeleton | Kind::Creeper => 0.25,
            Kind::Spider => 0.3,
            Kind::EndCrystal => 0.0,
            // Дракон летает (вики, Ender Dragon); ходьбы у него нет.
            Kind::EnderDragon => 0.0,
        }
    }

    /// Съедобное для разведения (вики, Breeding).
    pub fn likes(self, item: &str) -> bool {
        match self {
            Kind::Cow | Kind::Sheep => item == "wheat",
            Kind::Pig => matches!(item, "carrot" | "potato" | "beetroot"),
            Kind::Chicken => {
                matches!(item, "wheat_seeds" | "beetroot_seeds" | "melon_seeds" | "pumpkin_seeds" | "torchflower_seeds" | "pitcher_pod")
            }
            Kind::Zombie | Kind::Skeleton | Kind::Creeper | Kind::Spider | Kind::EndCrystal | Kind::EnderDragon => false,
        }
    }

    /// Броня (чёрный ящик, zombie-hit: удар мечом 7 снимает 6,888 — как при
    /// 2 очках брони: `урон · (1 − max(брони/5, брони − урон/2) / 25)`).
    pub fn armor(self) -> f32 {
        match self {
            Kind::Zombie => 2.0,
            _ => 0.0,
        }
    }

    /// Звук в покое, при ударе и при смерти (вики, Sounds).
    pub fn sounds(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Kind::Cow => ("minecraft:entity.cow.ambient", "minecraft:entity.cow.hurt", "minecraft:entity.cow.death"),
            Kind::Pig => ("minecraft:entity.pig.ambient", "minecraft:entity.pig.hurt", "minecraft:entity.pig.death"),
            Kind::Sheep => ("minecraft:entity.sheep.ambient", "minecraft:entity.sheep.hurt", "minecraft:entity.sheep.death"),
            Kind::Chicken => ("minecraft:entity.chicken.ambient", "minecraft:entity.chicken.hurt", "minecraft:entity.chicken.death"),
            Kind::Zombie => ("minecraft:entity.zombie.ambient", "minecraft:entity.zombie.hurt", "minecraft:entity.zombie.death"),
            Kind::Skeleton => ("minecraft:entity.skeleton.ambient", "minecraft:entity.skeleton.hurt", "minecraft:entity.skeleton.death"),
            // У крипера голоса в покое нет (вики, Creeper, «Sounds»).
            Kind::Creeper => ("", "minecraft:entity.creeper.hurt", "minecraft:entity.creeper.death"),
            Kind::Spider => ("minecraft:entity.spider.ambient", "minecraft:entity.spider.hurt", "minecraft:entity.spider.death"),
            // Кристалл молчит: его единственный звук — взрыв.
            Kind::EndCrystal => ("", "", ""),
            // Чёрный ящик, dragon-summon: голос в покое — entity.ender_dragon.ambient, громкость 5.
            Kind::EnderDragon => ("minecraft:entity.ender_dragon.ambient", "minecraft:entity.ender_dragon.hurt", "minecraft:entity.ender_dragon.death"),
        }
    }

    /// Звук шага (вики, Sounds).
    pub fn step_sound(self) -> &'static str {
        match self {
            Kind::Cow => "minecraft:entity.cow.step",
            Kind::Pig => "minecraft:entity.pig.step",
            Kind::Sheep => "minecraft:entity.sheep.step",
            Kind::Chicken => "minecraft:entity.chicken.step",
            Kind::Zombie => "minecraft:entity.zombie.step",
            Kind::Skeleton => "minecraft:entity.skeleton.step",
            // Шаги крипера играются звуком блока [проверить]; молча.
            Kind::Creeper => "",
            Kind::Spider => "minecraft:entity.spider.step",
            Kind::EndCrystal | Kind::EnderDragon => "",
        }
    }
}

/// Сколько тактов детёныш растёт: 20 минут (вики, Breeding: «-24000»).
pub const BABY_AGE: i32 = -24000;

/// Пауза взрослого после разведения: 5 минут (вики, Breeding).
pub const BREED_COOLDOWN: i32 = 6000;

/// Сколько тактов животное «влюблено» после еды: 30 секунд (вики, Breeding).
pub const LOVE_TICKS: i32 = 600;

/// Цвета овец: белая — по умолчанию у призванной командой (вики, Sheep).
pub const WHITE: u8 = 0;

/// Сила обычного удара рукой и оружием без чар (вики, Knockback: 0,4).
pub const BASE_KNOCKBACK: f64 = 0.4;

/// Тяжесть живой сущности за такт и сопротивление воздуха по вертикали
/// (вики, Entity, «Motion»: gravity 0.08, drag 0.98).
pub const GRAVITY: f64 = 0.08;
pub const VERTICAL_DRAG: f64 = 0.98;

/// Сколько тактов длится паника после удара (вики, Mob AI, «Panic»: ~5 с).
pub const PANIC_TICKS: i32 = 100;

/// Сколько тактов тело лежит после смерти, пока не пропадёт: 20 (вики,
/// Entity, «Death»: анимация смерти — секунда).
pub const DEATH_TICKS: i32 = 20;

/// Сколько частей у дракона: голова, шея, тело, три хвоста, два крыла
/// (вики, Ender Dragon, «Hitbox»); их номера идут следом за номером дракона,
/// голова — первая [проверить].
pub const DRAGON_PARTS: i32 = 8;

/// Сколько тактов дракон умирает (вики, Ender Dragon: «death animation … 10
/// seconds (200 game ticks)»).
pub const DRAGON_DEATH_TICKS: i32 = 200;

/// Опыт за дракона: 12000 за первого в бою Края, 500 за всякого другого
/// (вики, Ender Dragon: «12,000 … 500 for subsequent dragons»).
pub const DRAGON_XP_FIRST: i32 = 12000;
pub const DRAGON_XP: i32 = 500;

/// Сколько тактов после удара игрока смерть моба ещё даёт опыт (вики,
/// Experience, «Sources»: «within five seconds (100 game ticks)»).
pub const XP_CREDIT_TICKS: u64 = 100;

/// Насколько животное поднимается из клетки на ступеньку без прыжка.
pub const STEP_HEIGHT: f64 = 0.6;

/// Мелочь, что ставится между ящиком и стеной, чтобы не входить в неё.
const EPS: f64 = 1.0e-7;

/// Откуда моб видит мир: блоки и загруженные чанки.
pub trait View {
    fn block(&self, x: i32, y: i32, z: i32) -> i32;
    fn loaded(&self, chunk_x: i32, chunk_z: i32) -> bool;

    /// Открыто ли небо над клеткой (для солнца). Без мира — нет.
    fn open_sky(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }

    /// Небесный (с поправкой на час: `sky` — свет под открытым небом сейчас)
    /// и блочный свет в клетке.
    fn light(&self, _x: i32, _y: i32, _z: i32, _sky: i32) -> (u8, u8) {
        (0, 0)
    }
}

/// Упирается ли моб в блок: у всего, у чего есть ящик, — стена или пол
/// (как у выпавшего предмета; ступени и плиты считаются целыми [проверить]).
pub fn solid(state: i32) -> bool {
    blocks::stops_items(state)
}

/// Скользкость блока (вики, Block Slipperiness): обычный 0,6, лёд 0,98,
/// слизь 0,8, синий лёд 0,989.
pub fn slipperiness(state: i32) -> f64 {
    match blocks::block_at_state(state) {
        Some("ice" | "packed_ice" | "frosted_ice") => 0.98,
        Some("blue_ice") => 0.989,
        Some("slime_block") => 0.8,
        _ => 0.6,
    }
}

/// Вода ли в этом состоянии блока.
pub fn is_water(state: i32) -> bool {
    matches!(blocks::block_at_state(state), Some("water" | "bubble_column" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant"))
}

/// Лава ли.
pub fn is_lava(state: i32) -> bool {
    blocks::block_at_state(state) == Some("lava")
}

/// Частицы удара (minecraft-data 26.1, particles.json).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Particle {
    DamageIndicator,
    SweepAttack,
}

impl Particle {
    pub fn id(self) -> i32 {
        match self {
            Particle::DamageIndicator => 7,
            Particle::SweepAttack => 67,
        }
    }
}

/// Удар со стороны: куда толкает (единичное направление по горизонтали),
/// с какой силой и кто ударил (номер сущности, 0 — никто).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Push {
    pub dir: (f64, f64),
    pub strength: f64,
    pub cause: i32,
}

/// Что случилось с мобом — запись, которую подключения перескажут клиентам
/// в порядке, в каком оно произошло.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Урон: вид, номер ударившей сущности (0 — никто) и место.
    Hurt { source: Source, cause: i32, at: (f64, f64, f64) },

    /// Смерть: клиенты играют падение набок.
    Death,

    /// Частицы приземления: блок под ногами и их число.
    Landing { block: i32, count: i32 },

    /// Звук в месте моба: имя, громкость, высота.
    Sound { name: &'static str, volume: f32, pitch: f32 },

    /// Звук игрока в его точке — удар по животному (раздел «игроки»): к
    /// моб-журналу привязан ради порядка пакетов (чёрный ящик, cow-hit).
    PlayerSound { name: &'static str, at: (f64, f64, f64) },

    /// Частицы: вид, место, разброс, скорость, число.
    Particle { kind: Particle, at: (f64, f64, f64), offset: (f32, f32, f32), speed: f32, count: i32 },

    /// Звук, привязанный к сущности (Entity Sound Effect): стрижка овцы —
    /// раздел «игроки» (чёрный ящик, sheep-shear). Не глушится `Silent`.
    EntitySound { name: &'static str, category: i32, volume: f32, pitch: f32 },

    /// Добыча погибшего: падает в его месте, в порядке журнала — между звуком
    /// смерти и событием смерти (чёрный ящик, cow-physics).
    Loot(Vec<Stack>),

    /// Анимация сущности (Entity Animation): 4 — критический удар (частицы
    /// вокруг раненого; чёрный ящик, cow-crit).
    Animation(u8),

    /// Тело убрано: облачко (событие сущности 60) перед исчезновением.
    Poof,

    /// Сердечки над головой (событие сущности 18) — влюблён.
    Hearts,

    /// Овца щиплет траву (событие сущности 10).
    Eating,

    /// Взмах рукой (Entity Animation 0): удар зомби, выстрел скелета.
    Swing,
}

/// Моб в мире.
#[derive(Clone, Debug)]
pub struct Mob {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub kind: Kind,

    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Скорость, блоков за такт.
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,

    /// Поворот тела и наклон головы в градусах, поворот головы.
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,

    pub on_ground: bool,

    /// Здоровье, неуязвимость после удара, высота падения — та же
    /// модель, что у игрока (survival.rs).
    pub vitals: Vitals,

    /// `NoAI:1b`: ни мысли, ни движения (вики, Mob: «NoAI»).
    pub no_ai: bool,

    /// `Silent:1b`: моб не издаёт звуков.
    pub silent: bool,

    /// Возраст: отрицательный — детёныш, растёт к нулю; положительный у
    /// взрослого — пауза после разведения.
    pub age: i32,

    /// Сколько тактов ещё влюблён (0 — нет).
    pub love: i32,

    /// Овца: остригнута ли, цвет шерсти (0 — белая).
    pub sheared: bool,
    pub color: u8,

    /// Курица: сколько тактов до яйца.
    pub egg_timer: i32,

    /// Такты со смерти (0 — жив): на 20-м моб пропадает.
    pub dead_for: i32,

    /// Сколько тактов моб живёт.
    pub ticks: u64,

    /// Пройденное по земле для звука шагов и когда играть следующий.
    pub move_dist: f64,
    pub next_step: f64,

    /// Сколько раз моба толкнуло (удар): клиентам после толчка скорость
    /// уходит сразу, вне очереди.
    pub impulses: u32,

    /// Что произошло за такт — забирает `Items`.
    pub events: Vec<Event>,

    /// Мысли и цель ходьбы.
    pub brain: ai::Brain,

    /// Тот, на кого сердится (для паники): куда бежать от него.
    pub fright: Option<(f64, f64)>,

    /// Сохраняется ли с чанком.
    pub persistent: bool,

    /// Шаг с оглядкой на цель (скелет): вперёд и влево, в долях скорости;
    /// действует, пока нет пути.
    pub strafe: (f64, f64),

    /// Последний сдвиг упёрся в стену: паук по ней лезет.
    pub wall: bool,

    /// Состояние охоты у враждебных (mob/hostile.rs).
    pub h: hostile::State,

    /// Последний удар нанёс игрок: без него у паука и прочих часть добычи
    /// не падает (вики, Spider: «spider eye — только если убит игроком»).
    pub by_player: bool,

    /// Родился во время хода мобов: слежение покажет его в следующем такте.
    pub late: bool,

    /// Сколько раз ему правили данные (`summon … {…}`, `data merge`):
    /// клиентам после этого уходит пакет атрибутов.
    pub attr_changes: u32,

    /// Сколько раз моба переносили командой: клиентам нужен перенос
    /// целиком, а не сдвиг.
    pub teleports: u32,

    /// До какого такта жизни смерть моба даёт опыт: игрок ударил его не
    /// раньше чем за 100 тактов (вики, Experience, «Sources»); 0 — не бил.
    pub xp_until: u64,

    /// Опыт, который надо ронять шариками: ставит смерть, забирает
    /// `Items::flush_mob_events`.
    pub pending_xp: i32,

    /// Дракон боя Края, ещё не побеждённого: за него 12000 опыта.
    pub first_dragon: bool,

    /// Кристалл: куда идёт луч (цель исцеления); дракон: номер кристалла,
    /// что его лечит (0 — никто).
    pub beam_target: Option<(i32, i32, i32)>,
    pub healer: i32,

    /// Кристалл ударили: взрывается в этот же такт и исчезает.
    pub exploding: bool,

    /// Дракон: фаза полёта и память боя (mob/dragon.rs).
    pub dragon: dragon::State,
}

impl Mob {
    pub fn new(entity_id: i32, kind: Kind, at: (f64, f64, f64)) -> Self {
        let mut vitals = Vitals::new();

        vitals.health = kind.max_health();

        let h = hostile::State { bow: kind == Kind::Skeleton, ..hostile::State::default() };

        Self {
            entity_id,
            uuid: uuid_for(entity_id),
            kind,
            x: at.0,
            y: at.1,
            z: at.2,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            on_ground: false,
            vitals,
            no_ai: false,
            silent: false,
            age: 0,
            love: 0,
            sheared: false,
            color: WHITE,
            egg_timer: 0,
            dead_for: 0,
            ticks: 0,
            move_dist: 0.0,
            next_step: 1.0,
            impulses: 0,
            events: Vec::new(),
            brain: ai::Brain::new(entity_id),
            fright: None,
            persistent: true,
            strafe: (0.0, 0.0),
            wall: false,
            h,
            by_player: false,
            late: false,
            attr_changes: 0,
            teleports: 0,
            xp_until: 0,
            pending_xp: 0,
            first_dragon: false,
            beam_target: None,
            healer: 0,
            exploding: false,
            dragon: dragon::State::default(),
        }
    }

    /// Лечение кристаллом: ближайший живой кристалл не дальше 32 блоков по
    /// каждой оси светит в дракона лучом и даёт 1 здоровья раз в 10 тактов
    /// (вики, Ender Dragon: «1HP every 10 game ticks»; End Crystal: «cuboid
    /// extending 32 blocks»). Кто лечит — `healer`; луч ставит такт боя.
    fn crystal_heal(&mut self, mates: &[ai::Mate]) {
        let nearest = mates
            .iter()
            .filter(|mate| mate.kind == Kind::EndCrystal)
            .filter(|mate| (mate.x - self.x).abs() <= dragon::HEAL_RANGE && (mate.y - self.y).abs() <= dragon::HEAL_RANGE && (mate.z - self.z).abs() <= dragon::HEAL_RANGE)
            .min_by(|a, b| {
                let da = (a.x - self.x).powi(2) + (a.y - self.y).powi(2) + (a.z - self.z).powi(2);
                let db = (b.x - self.x).powi(2) + (b.y - self.y).powi(2) + (b.z - self.z).powi(2);
                da.total_cmp(&db)
            })
            .map(|mate| mate.id);

        self.healer = nearest.unwrap_or(0);

        if self.healer != 0 && self.ticks.is_multiple_of(dragon::HEAL_EVERY) {
            self.vitals.health = (self.vitals.health + 1.0).min(self.kind.max_health());
        }
    }

    /// Ударил игрок: смерть в ближайшие 100 тактов даст опыт.
    pub fn credit_player(&mut self) {
        self.xp_until = self.ticks + XP_CREDIT_TICKS;
    }

    pub fn is_baby(&self) -> bool {
        self.age < 0
    }

    pub fn is_dead(&self) -> bool {
        self.dead_for > 0 || self.vitals.dead
    }

    /// Ширина и высота ящика: у детёныша вдвое меньше (вики, Baby mobs).
    pub fn size(&self) -> (f64, f64) {
        let scale = if self.is_baby() { 0.5 } else { 1.0 };

        (self.kind.width() * scale, self.kind.height() * scale)
    }

    /// Высота глаз: 0,85 роста (вики, Entity: «eye height»); у враждебных —
    /// свои: зомби и скелет 1,74 (чёрный ящик, skeleton-arrow: стрела вылетает
    /// на 1,64 = глаза − 0,1), крипер 1,445, паук 0,65 [проверить].
    pub fn eye(&self) -> f64 {
        match self.kind {
            Kind::Zombie | Kind::Skeleton => 1.74,
            Kind::Creeper => 1.445,
            Kind::Spider => 0.65,
            _ => self.size().1 * 0.85,
        }
    }

    /// Клетка, в которой стоит моб.
    pub fn cell(&self) -> (i32, i32, i32) {
        (self.x.floor() as i32, (self.y + 1.0e-3).floor() as i32, self.z.floor() as i32)
    }

    /// Чанк, в котором стоит моб.
    pub fn chunk(&self) -> (i32, i32) {
        ((self.x.floor() as i32).div_euclid(16), (self.z.floor() as i32).div_euclid(16))
    }

    /// Ставит поворот тела по направлению взгляда.
    pub fn face(&mut self, yaw: f32) {
        self.yaw = yaw;
        self.head_yaw = yaw;
    }

    /// Такт моба: рост, мысли, движение, урон от падения. Возвращает то, что
    /// случилось вовне (детёныш, съеденная трава, яйцо), — исполняет вызывающий.
    pub fn tick(&mut self, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
        self.ticks += 1;
        self.late = false;

        // Мёртвый лежит и падает набок; тело пропадает через секунду.
        if self.is_dead() {
            self.dead_for += 1;

            if self.kind == Kind::EnderDragon {
                dragon::die_step(self);
            } else if !self.no_ai {
                self.physics(view, 0.0, false);
            }

            return ai::Plan::default();
        }

        let loaded = {
            let (cx, cz) = self.chunk();
            view.loaded(cx, cz)
        };

        // В выгруженном чанке мира нет: моб стоит, как и всё там.
        if !loaded {
            return ai::Plan::default();
        }

        self.vitals.tick_invulnerable();

        // Рост и пауза после разведения (вики, Breeding).
        if self.age < 0 {
            self.age += 1;
        } else if self.age > 0 {
            self.age -= 1;
        }

        if self.love > 0 {
            self.love -= 1;
        }

        // NoAI: ни мысли, ни хода — и тяжести тоже: призванный в воздухе
        // висит, а толчок скорость копит, но не двигает (чёрный ящик,
        // cow-physics и cow-hit: корова NoAI висит на месте, скорость после
        // двух ударов 0,6).
        // Огонь: раз в 20 тактов на 1 (вики, Fire) — и у NoAI; а солнце
        // поджигает только мобов с ИИ (чёрный ящик, zombie-burn: зомби NoAI
        // в полдень цел).
        if self.kind.hostile() && hostile::burn(self, view, ctx, rng, !self.no_ai) {
            self.hurt(1.0, Source::OnFire, None);
        }

        // Кристалл Края стоит, где поставлен: ни тяжести, ни мыслей, ни голоса
        // (вики, End Crystal; чёрный ящик, end-crystal-explode: висит без сверок скорости).
        if self.kind == Kind::EndCrystal {
            return ai::Plan::default();
        }

        // Дракон: полёта по кругу у нас нет — висит на месте (фаза 10,
        // «hovering»), как призванный [проверить: ИИ боя только по вики]. Голос
        // в покое — и у NoAI (чёрный ящик, dragon-summon: NoAI рычит).
        if self.kind == Kind::EnderDragon {
            ai::voice(self, rng);
            dragon::think(self, ctx.players, rng);

            // Без ИИ кристаллы его не лечат (чёрный ящик, dragon-crystal-heal:
            // у NoAI-дракона ни луча, ни здоровья).
            if !self.no_ai {
                self.crystal_heal(ctx.mates);
            }

            return ai::Plan::default();
        }

        if self.no_ai {
            // Запал крипера — не мысль, а его такт: подожжённый NoAI взрывается
            // (чёрный ящик, creeper-explode).
            let mut plan = ai::Plan::default();

            if self.kind == Kind::Creeper && (self.h.lit || self.h.fuse > 0) {
                hostile::swell(self, None, &mut plan);
            }

            return plan;
        }

        let plan = ai::think(self, view, ctx, rng);

        self.physics(view, plan.speed, plan.jump);
        plan
    }

    /// Движение живой сущности за такт (вики, Entity, «Motion»; чёрный
    /// ящик, cow-physics): сперва сдвиг на нынешнюю скорость (в первом такте
    /// падения она нулевая), потом тяжесть и сопротивление
    /// `v.y = (v.y − 0,08)·0,98`, трение `v.xz *= f·0,91`, где f —
    /// скользкость блока под ногами (в воздухе 1). Разгон по ходу — до сдвига:
    /// на земле `speed²·(0,6/f)³` (чёрный ящик, probe-cow-plain: у коровы
    /// 0,2 даёт 0,04 за такт — шаг 0,088 блока), в воздухе 0,02.
    pub fn physics(&mut self, view: &dyn View, speed: f64, jump: bool) {
        let (cell_x, cell_y, cell_z) = self.cell();
        let here = view.block(cell_x, cell_y, cell_z);
        let in_water = is_water(here);
        let in_lava = is_lava(here);

        // Паук лезет вверх, пока упирается в стену: 0,2 за такт (вики, Spider:
        // «can climb any solid block»; скорость лестницы) [проверить].
        if self.kind == Kind::Spider {
            self.h.climbing = self.wall && speed != 0.0;

            // Лезет: перед сдвигом скорость — 0,2 после тяжести и сопротивления,
            // то есть (0,2 − 0,08)·0,98 = 0,1176 за такт, и клиентам уходит та
            // же (чёрный ящик, spider-climb: сдвиг 1445/4096 за три такта).
            if self.h.climbing {
                self.vy = (0.2 - GRAVITY) * VERTICAL_DRAG;
            }
        }

        if jump && self.on_ground {
            // Прыжок: 0,42 вверх (вики, Jumping: «0.42»).
            self.vy = 0.42;
        }

        let slip = if self.on_ground { slipperiness(view.block(cell_x, (self.y - 0.5000001).floor() as i32, cell_z)) } else { 1.0 };
        let friction = if self.on_ground { slip * 0.91 } else { 0.91 };

        if speed != 0.0 {
            let effective = speed * self.kind.speed();
            let accel = if self.on_ground { effective * effective * (0.6 / slip).powi(3) } else { 0.02 };
            let rad = f64::from(self.yaw).to_radians();

            self.vx += -rad.sin() * accel;
            self.vz += rad.cos() * accel;
        } else if self.strafe != (0.0, 0.0) {
            // Шаг вбок и вперёд-назад: тело смотрит на цель, идёт боком.
            let (forward, left) = self.strafe;
            let length = (forward * forward + left * left).sqrt();
            let effective = length * self.kind.speed();
            let accel = if self.on_ground { effective * effective * (0.6 / slip).powi(3) } else { 0.02 };
            let rad = f64::from(self.yaw).to_radians();
            let (fx, fz) = (-rad.sin(), rad.cos());
            let (lx, lz) = (fz, -fx);

            self.vx += (fx * forward + lx * left) / length * accel;
            self.vz += (fz * forward + lz * left) / length * accel;
        }

        let before = (self.x, self.z);

        if in_water || in_lava {
            self.vitals.fall = 0.0;
            self.step(view, self.vx, self.vy, self.vz);

            let drag = if in_lava { 0.5 } else { 0.8 };

            self.vx *= drag;
            self.vz *= drag;
            self.vy = self.vy * drag - 0.02;

            // «Float»: животное всплывает (вики, Mob AI: «Float» — плавать).
            if !in_lava && self.vy < 0.04 {
                self.vy += 0.04;
            }

            return;
        }

        self.step(view, self.vx, self.vy, self.vz);

        // Шаги: звук раз в 1/0,6 пройденного по земле (чёрный ящик,
        // probe-cow-plain: громкость 0,15, шаг раз в 1,6 блока).
        if self.on_ground {
            self.move_dist += ((self.x - before.0).powi(2) + (self.z - before.1).powi(2)).sqrt() * 0.6;

            if self.move_dist > self.next_step {
                self.next_step = self.move_dist.floor() + 1.0;

                if !self.kind.step_sound().is_empty() {
                    self.events.push(Event::Sound { name: self.kind.step_sound(), volume: 0.15, pitch: 1.0 });
                }
            }
        }

        if !(self.kind == Kind::Spider && self.h.climbing) {
            self.vy = (self.vy - GRAVITY) * VERTICAL_DRAG;
        }

        // Курица падает медленно (вики, Chicken: «slow falling»): в воздухе
        // падение после тяжести замедляется ещё в 0,6 — предельная скорость
        // −0,114 (чёрный ящик, chicken-fall).
        if self.kind == Kind::Chicken && !self.on_ground && self.vy < 0.0 {
            self.vy *= 0.6;
        }

        self.vx *= friction;
        self.vz *= friction;

        if self.vx.abs() < 0.003 {
            self.vx = 0.0;
        }

        if self.vz.abs() < 0.003 {
            self.vz = 0.0;
        }
    }

    /// Сдвиг с упором в блоки: сперва по вертикали, потом по большей из
    /// горизонтальных осей (вики, Entity: порядок осей y, x, z). Сдвиг вбок,
    /// упёршийся в ступеньку ниже 0,6, поднимает ящик на неё.
    pub fn step(&mut self, view: &dyn View, dx: f64, dy: f64, dz: f64) {
        let was_on_ground = self.on_ground;
        let (width, height) = self.size();
        let half = width / 2.0;
        let start = (self.x - half, self.y, self.z - half);

        // Вертикаль, потом горизонталь: по большей оси первой.
        let mut at = start;
        let mut moved_y = clip_y(view, at, half, height, dy);

        at.1 += moved_y;

        let (mut moved_x, mut moved_z) = slide(view, &mut at, half, height, dx, dz);
        let landed = dy < 0.0 && (moved_y - dy).abs() > 1.0e-9;
        let bumped = (moved_x - dx).abs() > 1.0e-9 || (moved_z - dz).abs() > 1.0e-9;

        // Ступенька: упёрлись, стоя на земле, — поднимаемся на STEP_HEIGHT и
        // проходим тот же путь, потом опускаемся до опоры [проверить: у
        // оригинала так же].
        if bumped && (was_on_ground || landed) {
            let lift = clip_y(view, start, half, height, STEP_HEIGHT);
            let mut up = (start.0, start.1 + lift, start.2);
            let (ux, uz) = slide(view, &mut up, half, height, dx, dz);
            let down = clip_y(view, up, half, height, dy.min(0.0) - lift);

            if ux * ux + uz * uz > moved_x * moved_x + moved_z * moved_z + 1.0e-12 {
                at = (up.0, up.1 + down, up.2);
                moved_x = ux;
                moved_z = uz;
                moved_y = lift + down;
            }
        }

        self.x = at.0 + half;
        self.y = at.1;
        self.z = at.2 + half;

        self.wall = (moved_x - dx).abs() > 1.0e-9 || (moved_z - dz).abs() > 1.0e-9;

        // Упёрлись в стену — по этой оси скорость гасится.
        if (moved_x - dx).abs() > 1.0e-9 {
            self.vx = 0.0;
        }

        if (moved_z - dz).abs() > 1.0e-9 {
            self.vz = 0.0;
        }

        // Упор вниз — на земле; вверх — головой.
        let down_hit = dy < 0.0 && (moved_y - dy).abs() > 1.0e-9;

        if down_hit {
            self.on_ground = true;
            self.vy = 0.0;
        } else {
            self.on_ground = false;

            if dy > 0.0 && (moved_y - dy).abs() > 1.0e-9 {
                self.vy = 0.0;
            }
        }

        // Высота падения копится по пройденному вниз, и такт приземления
        // в счёт урона (чёрный ящик, cow-physics: с шести блоков — урон 3)
        // (вики, Fall damage); для частиц он не в счёте — берётся высота до него.
        let before = self.vitals.fall;

        if moved_y < 0.0 {
            self.vitals.fall += (-moved_y) as f32;
        }

        if !self.on_ground {
            return;
        }

        let fall = self.vitals.fall;

        self.vitals.fall = 0.0;

        if fall <= 0.0 {
            return;
        }

        // Частицы приземления: та же формула, что у игрока (survival::
        // landing_particles; чёрный ящик probe-land-cow, probe-land-chicken).
        let below = view.block(self.x.floor() as i32, (self.y - 0.001).floor() as i32, self.z.floor() as i32);
        let count = crate::survival::landing_particles(before);

        if count > 0 && blocks::block_at_state(below).is_some_and(|name| !name.ends_with("air")) {
            self.events.push(Event::Landing { block: below, count });
        }

        // Урон падения: ceil(высота − 3) (вики, Fall damage); курица не
        // разбивается (вики, Chicken).
        if fall > crate::survival::SAFE_FALL && self.kind != Kind::Chicken {
            let damage = (fall - crate::survival::SAFE_FALL - 1.0e-4).ceil();

            self.hurt(damage, Source::Fall, None);
        }
    }

    /// Удар по части дракона: в голову — полный, в прочие части — четверть
    /// и ещё до одного очка (вики, Ender Dragon: «reduces all incoming damage
    /// to it by ~75% unless the attack is hitting its head»; формула
    /// `damage/4 + min(1, damage)`) [проверить: у оригинала NoAI-дракона
    /// ударить нельзя — части стоят в нуле мира, сверить не удалось].
    pub fn hurt_part(&mut self, amount: f32, head: bool, source: Source, push: Option<Push>) -> Option<Hit> {
        let amount = if head { amount } else { amount / 4.0 + amount.min(1.0) };

        self.hurt(amount, source, push)
    }

    /// Удар. `push` — кто и как толкнул (None — урон без толчка: падение,
    /// команда). Возвращает исход или None, если удар не прошёл (вики,
    /// Damage: «Invulnerability frames»).
    pub fn hurt(&mut self, amount: f32, source: Source, push: Option<Push>) -> Option<Hit> {
        if self.is_dead() {
            return None;
        }

        // Кристалл: любой урон — взрыв, даже нулевой (вики, End Crystal:
        // «explode when attacked or damaged in most ways, even by attacks that
        // normally do zero damage»). Самого взрыва здесь нет: его делает
        // ударивший (strike_mob) или такт (tick::detonate_crystals), по `exploding`.
        if self.kind == Kind::EndCrystal {
            self.exploding = true;
            return None;
        }

        // Дракону вредят только игроки и взрывы (вики, Ender Dragon: «Only
        // explosions and players deal damage»; чёрный ящик, dragon-die:
        // `damage … minecraft:generic` ему нипочём).
        if self.kind == Kind::EnderDragon && !matches!(source, Source::PlayerAttack | Source::Arrow | Source::Explosion | Source::PlayerExplosion | Source::GenericKill) {
            return None;
        }

        let before = self.vitals.health;
        let amount = if armored(source) { after_armor(amount, self.kind.armor()) } else { amount };
        let mut hit = self.vitals.hurt(amount, source, false)?;

        // Снято на самом деле — не больше оставшегося здоровья.
        hit.taken = hit.taken.min(before);

        if self.kind == Kind::EnderDragon {
            let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0xD4A);

            dragon::on_hurt(self, hit.taken, push.map(|push| push.cause).filter(|cause| *cause != 0), &mut rng);
        }

        self.events.push(Event::Hurt { source, cause: push.map_or(0, |push| push.cause), at: (self.x, self.y, self.z) });

        // Ударил игрок: враждебный мстит ему (паук нападает и при свете).
        if let Some(push) = push.filter(|push| push.cause != 0) {
            self.by_player = matches!(source, Source::PlayerAttack | Source::MaceSmash);

            if self.kind.hostile() && !self.no_ai {
                self.h.target = Some(push.cause);
                self.h.provoked = true;
            }
        }

        // Всякий урон — «толчок» для слежения: скорость уходит клиентам вне
        // очереди, даже нулевая (чёрный ящик, cow-physics: падение).
        self.impulses += 1;

        if let Some(push) = push {
            self.knockback(push.dir.0, push.dir.1, push.strength);
        }

        if hit.died {
            self.die();
        } else {
            let (_, hurt, _) = self.kind.sounds();

            self.events.push(Event::Sound { name: hurt, volume: self.sound_volume(), pitch: self.voice_pitch() });

            // Паника — только от удара существа: падение и команда не пугают
            // (чёрный ящик, cow-physics: упавшая корова стоит).
            if let Some(push) = push.filter(|push| push.cause != 0 && !self.kind.hostile()) {
                self.fright = Some((self.x - push.dir.0 * 8.0, self.z - push.dir.1 * 8.0));
                self.brain.panic(PANIC_TICKS);
            }
        }

        Some(hit)
    }

    /// Толчок: скорость делится пополам и складывается с ударом; вверх
    /// не больше 0,4 (вики, Knockback).
    pub fn knockback(&mut self, ux: f64, uz: f64, strength: f64) {
        self.vx = self.vx / 2.0 + ux * strength;
        self.vz = self.vz / 2.0 + uz * strength;

        if self.on_ground {
            self.vy = (self.vy / 2.0 + strength).min(0.4);
        }

    }

    /// Громкость голоса: у коровы 0,4, у прочих 1 (чёрный ящик: голос
    /// коровы при смерти — 0,4).
    pub fn sound_volume(&self) -> f32 {
        match self.kind {
            Kind::Cow => 0.4,
            // Чёрный ящик, dragon-summon: голос дракона громкостью 5,00.
            Kind::EnderDragon => 5.0,
            _ => 1.0,
        }
    }

    /// Высота голоса: `1 + (случайное − случайное)·0,2`, у детёныша на 0,5
    /// выше (вики, Sounds: «babies have higher-pitched sounds»; чёрный
    /// ящик, cow-hit: голос коровы 0,97–1,13).
    pub fn voice_pitch(&self) -> f32 {
        let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks.wrapping_mul(0x9E37) ^ ((self.events.len() as u64) << 40));
        let spread = ((rng.unit() - rng.unit()) * 0.2) as f32;

        if self.is_baby() { 1.5 + spread } else { 1.0 + spread }
    }

    /// Смерть: клиентам — событие, миру — добыча.
    pub fn die(&mut self) {
        if self.dead_for > 0 {
            return;
        }

        self.dead_for = 1;
        let (_, _, death) = self.kind.sounds();
        let drops = loot(self, &mut Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0x5EED));

        self.events.push(Event::Sound { name: death, volume: self.sound_volume(), pitch: self.voice_pitch() });

        // Опыт: только убитый игроком, не детёныш; корова, свинья, овца,
        // курица — 1–3 (вики, «Drops»); враждебный — 5 и ещё 1–3 за каждую
        // вещь снаряжения (вики, Experience, «Killing mobs»; чёрный ящик:
        // зомби — шарики 3+1+1, скелет с луком — 3+3).
        // Дракон даёт опыт всегда — и убитый командой [проверить]; за первого
        // в бою Края больше (ставит бой, tick.rs).
        if self.kind == Kind::EnderDragon {
            // Шарики — по ходу анимации смерти (tick.rs, dragon_xp).
            self.dragon.xp = if self.first_dragon { DRAGON_XP_FIRST } else { DRAGON_XP };
        } else if self.xp_until > self.ticks && !self.is_baby() {
            let mut rng = Rng::new(u64::from(self.entity_id as u32) ^ self.ticks ^ 0xE4B);

            self.pending_xp = if self.kind.hostile() {
                let worn = u32::from(self.h.bow) + u32::from(self.h.helmet);

                5 + (0..worn).map(|_| crate::survival::passive_mob_xp(rng.unit() as f32)).sum::<i32>()
            } else {
                crate::survival::passive_mob_xp(rng.unit() as f32)
            };
        }

        if !drops.is_empty() {
            self.events.push(Event::Loot(drops));
        }

        self.events.push(Event::Death);
    }

    /// Пропал ли труп: 20 тактов после смерти.
    pub fn gone(&self) -> bool {
        self.dead_for > if self.kind == Kind::EnderDragon { DRAGON_DEATH_TICKS } else { DEATH_TICKS }
    }

    /// Запись для файла региона: вид, место, скорость не пишется, здоровье,
    /// возраст, любовь, признаки, цвет, яйцо.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.push(match self.kind {
            Kind::Cow => 0,
            Kind::Pig => 1,
            Kind::Sheep => 2,
            Kind::Chicken => 3,
            Kind::Zombie => 4,
            Kind::Skeleton => 5,
            Kind::Creeper => 6,
            Kind::Spider => 7,
            Kind::EndCrystal => 8,
            Kind::EnderDragon => 9,
        });
        out.extend_from_slice(&self.x.to_be_bytes());
        out.extend_from_slice(&self.y.to_be_bytes());
        out.extend_from_slice(&self.z.to_be_bytes());
        out.extend_from_slice(&self.yaw.to_be_bytes());
        out.extend_from_slice(&self.vitals.health.to_be_bytes());
        out.extend_from_slice(&self.age.to_be_bytes());
        out.extend_from_slice(&self.love.to_be_bytes());
        out.push(u8::from(self.no_ai) | (u8::from(self.sheared) << 1) | (u8::from(self.h.helmet) << 2) | (u8::from(self.h.lit) << 3) | (u8::from(self.h.bow) << 4));
        out.push(self.color);
        out.extend_from_slice(&self.egg_timer.to_be_bytes());
    }

    /// Читает запись `write`; None — она испорчена.
    pub fn read(bytes: &mut &[u8]) -> Option<Mob> {
        let mut take = |count: usize| -> Option<&[u8]> {
            let (head, rest) = bytes.split_at_checked(count)?;
            *bytes = rest;
            Some(head)
        };

        let kind = match take(1)?[0] {
            0 => Kind::Cow,
            1 => Kind::Pig,
            2 => Kind::Sheep,
            3 => Kind::Chicken,
            4 => Kind::Zombie,
            5 => Kind::Skeleton,
            6 => Kind::Creeper,
            7 => Kind::Spider,
            8 => Kind::EndCrystal,
            9 => Kind::EnderDragon,
            _ => return None,
        };
        let x = f64::from_be_bytes(take(8)?.try_into().ok()?);
        let y = f64::from_be_bytes(take(8)?.try_into().ok()?);
        let z = f64::from_be_bytes(take(8)?.try_into().ok()?);
        let yaw = f32::from_be_bytes(take(4)?.try_into().ok()?);
        let health = f32::from_be_bytes(take(4)?.try_into().ok()?);
        let age = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let love = i32::from_be_bytes(take(4)?.try_into().ok()?);
        let flags = take(1)?[0];
        let color = take(1)?[0];
        let egg_timer = i32::from_be_bytes(take(4)?.try_into().ok()?);

        if !(x.is_finite() && y.is_finite() && z.is_finite() && health.is_finite()) || color > 15 {
            return None;
        }

        let mut mob = Mob::new(0, kind, (x, y, z));

        mob.yaw = yaw;
        mob.head_yaw = yaw;
        mob.vitals.health = health.clamp(0.5, kind.max_health());
        mob.age = age;
        mob.love = love.max(0);
        mob.no_ai = flags & 1 != 0;
        mob.sheared = flags & 2 != 0;
        mob.h.helmet = flags & 4 != 0;
        mob.h.lit = flags & 8 != 0;
        mob.h.bow = flags & 16 != 0;
        mob.color = color;
        mob.egg_timer = egg_timer;

        Some(mob)
    }
}

impl PartialEq for Mob {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.x == other.x
            && self.y == other.y
            && self.z == other.z
            && self.vitals.health == other.vitals.health
            && self.age == other.age
            && self.no_ai == other.no_ai
            && self.sheared == other.sheared
            && self.color == other.color
    }
}

/// Выпадающее после смерти (вики: Cow, Pig, Sheep, Chicken, «Drops»):
/// корова — 0–2 кожи и 1–3 говядины, свинья — 1–3 свинины, овца — шерсть
/// своего цвета (если не острижена) и 1–2 баранины, курица — 0–2 пера и
/// курятина. Детёныши ничего не роняют. Горящие роняют жареное [проверить:
/// огня у мобов ещё нет].
pub fn loot(mob: &Mob, rng: &mut Rng) -> Vec<Stack> {
    if mob.is_baby() && !mob.kind.hostile() {
        return Vec::new();
    }

    let mut drops = Vec::new();
    let mut give = |name: &str, low: i32, high: i32, rng: &mut Rng| {
        let count = low + rng.below((high - low + 1) as u32) as i32;

        if let (Some(item), true) = (blocks::item_named(name), count > 0) {
            drops.push(Stack::new(item, count));
        }
    };

    match mob.kind {
        Kind::Cow => {
            give("leather", 0, 2, rng);
            give("beef", 1, 3, rng);
        }
        Kind::Pig => give("porkchop", 1, 3, rng),
        Kind::Sheep => {
            if !mob.sheared {
                give(WOOL[usize::from(mob.color) % 16], 1, 1, rng);
            }

            give("mutton", 1, 2, rng);
        }
        Kind::Chicken => {
            give("feather", 0, 2, rng);
            give("chicken", 1, 1, rng);
        }
        // Вики: зомби — 0–2 гнилой плоти, скелет — 0–2 костей и 0–2 стрел,
        // крипер — 0–2 пороха, паук — 0–2 нити и 0–1 паучий глаз (глаз — только
        // если убил игрок).
        Kind::Zombie => give("rotten_flesh", 0, 2, rng),
        Kind::Skeleton => {
            give("bone", 0, 2, rng);
            give("arrow", 0, 2, rng);
        }
        Kind::Creeper => give("gunpowder", 0, 2, rng),
        Kind::Spider => {
            give("string", 0, 2, rng);

            if mob.by_player {
                give("spider_eye", 0, 1, rng);
            }
        }
        // Кристалл и дракон предметов не роняют (вики: у дракона только опыт,
        // яйцо — блок на портале).
        Kind::EndCrystal | Kind::EnderDragon => {}
    }

    drops
}

/// Шерсть по цветам, по порядку цветов красителей (вики, Wool).
pub const WOOL: [&str; 16] = [
    "white_wool",
    "orange_wool",
    "magenta_wool",
    "light_blue_wool",
    "yellow_wool",
    "lime_wool",
    "pink_wool",
    "gray_wool",
    "light_gray_wool",
    "cyan_wool",
    "purple_wool",
    "blue_wool",
    "brown_wool",
    "green_wool",
    "red_wool",
    "black_wool",
];

/// Урон оружия в руке и перезарядка (вики, Damage, «Weapon damage»): кулак 1,
/// мечи 4–8. Возвращает (урон, тактов перезарядки).
#[cfg(test)]
pub fn weapon(item: Option<&str>) -> (f32, f32) {
    match item {
        Some("wooden_sword" | "golden_sword") => (4.0, 12.5),
        Some("stone_sword") => (5.0, 12.5),
        Some("iron_sword") => (6.0, 12.5),
        Some("diamond_sword") => (7.0, 12.5),
        Some("netherite_sword") => (8.0, 12.5),
        _ => (1.0, 5.0),
    }
}

/// Доля урона при неполном заряде руки (вики, Damage: `0.2 + charge²·0.8`).
#[cfg(test)]
pub fn charged(damage: f32, since_attack: u32, cooldown: f32) -> f32 {
    let charge = ((since_attack as f32 + 0.5) / cooldown).clamp(0.0, 1.0);

    damage * (0.2 + charge * charge * 0.8)
}

/// Критический удар (вики, Critical hit): рука заряжена больше чем на 0,9,
/// игрок падает (высота падения накоплена), не стоит на земле, не бежит и не
/// в воде; урон тогда ×1,5 (чёрный ящик, cow-crit, cow-crit-fist).
#[cfg(test)]
pub fn is_critical(charge: f32, fall: f32, on_ground: bool, sprinting: bool, in_water: bool) -> bool {
    charge > 0.9 && fall > 0.0 && !on_ground && !sprinting && !in_water
}

/// Кто рядом мог бы толкнуть: единичное направление взгляда игрока по
/// горизонтали (вперёд по yaw) — им и толкает удар (вики, Knockback).
pub fn look_direction(yaw_degrees: f32) -> (f64, f64) {
    let rad = f64::from(yaw_degrees).to_radians();

    (-rad.sin(), rad.cos())
}

/// Сдвиг по горизонтали с упором: по большей оси первой. Возвращает, на
/// сколько сдвинулись по x и z; `at` — угол ящика, он обновляется.
fn slide(view: &dyn View, at: &mut (f64, f64, f64), half: f64, height: f64, dx: f64, dz: f64) -> (f64, f64) {
    if dx.abs() >= dz.abs() {
        let mx = clip_x(view, *at, half, height, dx);

        at.0 += mx;

        let mz = clip_z(view, *at, half, height, dz);

        at.2 += mz;
        (mx, mz)
    } else {
        let mz = clip_z(view, *at, half, height, dz);

        at.2 += mz;

        let mx = clip_x(view, *at, half, height, dx);

        at.0 += mx;
        (mx, mz)
    }
}

/// Свободный ход по оси вверх/вниз: насколько можно сдвинуться, не входя в блоки.
fn clip_y(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dy: f64) -> f64 {
    if dy == 0.0 {
        return 0.0;
    }

    let x0 = (min.0 + EPS).floor() as i32;
    let x1 = (min.0 + 2.0 * half - EPS).floor() as i32;
    let z0 = (min.2 + EPS).floor() as i32;
    let z1 = (min.2 + 2.0 * half - EPS).floor() as i32;

    if dy > 0.0 {
        let top = min.1 + height;
        let from = (top - EPS).floor() as i32 + 1;
        let to = (top + dy).floor() as i32;
        let mut allowed = dy;

        for y in from..=to.max(from - 1) {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.min((f64::from(y) - top).max(0.0));
                    }
                }
            }
        }

        allowed
    } else {
        let bottom = min.1;
        let from = (bottom + EPS).floor() as i32 - 1;
        let to = (bottom + dy).floor() as i32;
        let mut allowed = dy;
        let mut y = from;

        while y >= to {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.max(f64::from(y + 1) - bottom);
                    }
                }
            }

            y -= 1;
        }

        allowed
    }
}

fn clip_x(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dx: f64) -> f64 {
    if dx == 0.0 {
        return 0.0;
    }

    let y0 = (min.1 + EPS).floor() as i32;
    let y1 = (min.1 + height - EPS).floor() as i32;
    let z0 = (min.2 + EPS).floor() as i32;
    let z1 = (min.2 + 2.0 * half - EPS).floor() as i32;
    let mut allowed = dx;

    if dx > 0.0 {
        let edge = min.0 + 2.0 * half;
        let from = (edge - EPS).floor() as i32 + 1;
        let to = (edge + dx).floor() as i32;

        for x in from..=to.max(from - 1) {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.min((f64::from(x) - edge).max(0.0));
                    }
                }
            }
        }
    } else {
        let edge = min.0;
        let from = (edge + EPS).floor() as i32 - 1;
        let to = (edge + dx).floor() as i32;
        let mut x = from;

        while x >= to {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.max(f64::from(x + 1) - edge);
                    }
                }
            }

            x -= 1;
        }
    }

    allowed
}

fn clip_z(view: &dyn View, min: (f64, f64, f64), half: f64, height: f64, dz: f64) -> f64 {
    if dz == 0.0 {
        return 0.0;
    }

    let y0 = (min.1 + EPS).floor() as i32;
    let y1 = (min.1 + height - EPS).floor() as i32;
    let x0 = (min.0 + EPS).floor() as i32;
    let x1 = (min.0 + 2.0 * half - EPS).floor() as i32;
    let mut allowed = dz;

    if dz > 0.0 {
        let edge = min.2 + 2.0 * half;
        let from = (edge - EPS).floor() as i32 + 1;
        let to = (edge + dz).floor() as i32;

        for z in from..=to.max(from - 1) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.min((f64::from(z) - edge).max(0.0));
                    }
                }
            }
        }
    } else {
        let edge = min.2;
        let from = (edge + EPS).floor() as i32 - 1;
        let to = (edge + dz).floor() as i32;
        let mut z = from;

        while z >= to {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    if solid(view.block(x, y, z)) {
                        allowed = allowed.max(f64::from(z + 1) - edge);
                    }
                }
            }

            z -= 1;
        }
    }

    allowed
}

/// Действует ли броня на урон: не действует на падение, огонь, лаву,
/// удушье и пустоту (вики, Armor: «bypasses armor»).
fn armored(source: Source) -> bool {
    matches!(source, Source::PlayerAttack | Source::MaceSmash | Source::MobAttack | Source::Arrow | Source::Explosion | Source::PlayerExplosion)
}

/// Урон после брони (вики, Armor, «Damage protection»).
pub fn after_armor(amount: f32, armor: f32) -> f32 {
    if armor <= 0.0 {
        return amount;
    }

    let reduction = (armor / 5.0).max(armor - amount / 2.0).min(20.0);

    amount * (1.0 - reduction / 25.0)
}

/// Опознаватель сущности.
pub fn uuid_for(entity_id: i32) -> [u8; 16] {
    let mut uuid = crate::items::uuid_for(entity_id);

    uuid[12..].copy_from_slice(b"mob!");
    uuid
}

/// Небольшой счётчик случайных чисел (xorshift): мобам нужна случайность
/// сама по себе, но не криптографическая; своё зерно позволяет проверкам
/// быть повторяемыми.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;

        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Число от 0 (включительно) до `n` (нет).
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 { 0 } else { ((self.next_u64() >> 32) % u64::from(n)) as u32 }
    }

    /// Дробь от 0 до 1.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Что мобы просят у мира за такт: съеденная овцами трава и снесённые яйца.
#[derive(Default, Debug)]
pub struct Outcome {
    pub eaten: Vec<ai::Eat>,
    pub eggs: Vec<(f64, f64, f64)>,

    /// Удары враждебных по игрокам, выстрелы скелетов, взрывы крипера.
    pub strikes: Vec<Strike>,
    pub shots: Vec<Fired>,
    pub blasts: Vec<Blast>,
}

/// Удар моба в ближнем бою.
#[derive(Debug)]
pub struct Strike {
    pub attacker: i32,
    pub at: (f64, f64, f64),
    pub kind: Kind,
    pub melee: hostile::Melee,
}

/// Выстрел скелета.
#[derive(Debug)]
pub struct Fired {
    pub shooter: i32,
    pub shot: hostile::Shot,
}

/// Взрыв крипера: центр, сила, кто.
#[derive(Debug)]
pub struct Blast {
    pub shooter: i32,
    pub at: (f64, f64, f64),
    pub power: f32,
}

/// Среда, что мобы чувствуют за такт: сложность, свет неба, солнце.
#[derive(Clone, Copy, Debug)]
pub struct Env {
    pub difficulty: u8,
    pub sky: i32,
    pub sun_up: bool,
}

/// Такт всех мобов склада: у каждого мысли и движение, потом разведение,
/// добыча погибших и уборка тел. События уходят в журнал в порядке, в каком
/// произошли. `next_id` выдаёт номера новым сущностям.
pub fn step_all(
    items: &mut crate::items::Items,
    view: &dyn View,
    players: &[ai::Watcher],
    env: Env,
    rng: &mut Rng,
    next_id: &mut dyn FnMut() -> i32,
) -> Outcome {
    let mut outcome = Outcome::default();

    if items.mobs().is_empty() {
        return outcome;
    }

    let mates: Vec<ai::Mate> = items
        .mobs()
        .iter()
        .filter(|mob| !mob.is_dead())
        .map(|mob| ai::Mate {
            id: mob.entity_id,
            kind: mob.kind,
            x: mob.x,
            y: mob.y,
            z: mob.z,
            love: mob.love,
            baby: mob.is_baby(),
            ready: mob.age == 0,
        })
        .collect();

    let ctx = ai::Ctx { players, mates: &mates, difficulty: env.difficulty, sky: env.sky, sun_up: env.sun_up };
    let mut mobs = std::mem::take(items.mobs_vec());
    let mut breeds: Vec<(i32, i32)> = Vec::new();

    for mob in &mut mobs {
        let plan = mob.tick(view, &ctx, rng);

        if let Some(melee) = plan.melee {
            outcome.strikes.push(Strike { attacker: mob.entity_id, at: (mob.x, mob.y, mob.z), kind: mob.kind, melee });
        }

        if let Some(shot) = plan.shot {
            outcome.shots.push(Fired { shooter: mob.entity_id, shot });
        }

        if let Some(power) = plan.blast {
            outcome.blasts.push(Blast { shooter: mob.entity_id, at: (mob.x, mob.y, mob.z), power });
            mob.h.despawn = true;
        }

        if mob.kind.hostile() && !mob.is_dead() {
            hostile::despawn_check(mob, players, rng, env.difficulty == crate::survival::PEACEFUL);
        }

        if let Some(partner) = plan.breed {
            breeds.push((mob.entity_id, partner));
        }

        if let Some(eat) = plan.eat {
            outcome.eaten.push(eat);

            if mob.kind == Kind::Sheep {
                mob.sheared = false;

                if mob.is_baby() {
                    mob.age = (mob.age + 60).min(0);
                }
            }
        }

        if plan.lay_egg {
            outcome.eggs.push((mob.x, mob.y, mob.z));
            mob.events.push(Event::Sound { name: "minecraft:entity.chicken.egg", volume: 1.0, pitch: (rng.unit() - rng.unit()) as f32 * 0.2 + 1.0 });
        }
    }

    // Разведение: оба родителя в паузу, детёныш рождается у первого. Пара,
    // из которой один уже занят, пропускается [проверить].
    let mut born = Vec::new();
    let mut used: Vec<i32> = Vec::new();

    for (a, b) in breeds {
        if used.contains(&a) || used.contains(&b) {
            continue;
        }

        let (Some(ia), Some(ib)) = (mobs.iter().position(|m| m.entity_id == a), mobs.iter().position(|m| m.entity_id == b)) else {
            continue;
        };

        used.extend([a, b]);

        let mut baby = Mob::new(next_id(), mobs[ia].kind, (mobs[ia].x, mobs[ia].y, mobs[ia].z));

        baby.age = BABY_AGE;
        baby.yaw = mobs[ia].yaw;
        baby.head_yaw = mobs[ia].yaw;
        baby.color = if mobs[ia].kind == Kind::Sheep { if rng.below(2) == 0 { mobs[ia].color } else { mobs[ib].color } } else { WHITE };
        baby.egg_timer = 6000 + rng.below(6001) as i32;

        for index in [ia, ib] {
            mobs[index].age = BREED_COOLDOWN;
            mobs[index].love = 0;
            mobs[index].brain.stop();
        }

        // Сердечки — событием сущности 18 у первого родителя (чёрный ящик, cow-breed).
        mobs[ia].events.push(Event::Hearts);
        born.push(baby);
    }

    *items.mobs_vec() = mobs;

    // Тела, пролежавшие своё, уходят облачком.
    for mob in items.mobs_mut() {
        if mob.gone() {
            mob.events.push(Event::Poof);
        }
    }

    items.flush_mob_events(next_id, rng);
    items.remove_mobs(|mob| mob.gone() || mob.h.despawn);

    for mut baby in born {
        baby.late = true;
        items.add_mob_late(baby);
    }

    outcome
}

/// Сколько нельзя поднимать выпавшее из моба: полсекунды, как у выпавшего
/// из блока (вики, Item: «pickup delay 10 ticks»).
pub const DROP_DELAY: u64 = 10;

#[cfg(test)]
mod tests;
