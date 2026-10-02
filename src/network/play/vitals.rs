// Здоровье, урон, смерть, возрождение, еда и голод игрока: то, что подключение
// рассказывает своему клиенту. Сам счёт — в survival.rs; здесь порядок и
// пакеты, сверенные с оригиналом чёрным ящиком.
//
// Порядок пакетов внутри такта (снят с оригинала по времени прихода):
//   A. удары от пакетов клиента (приземление) и команд — Damage Event и звуки;
//   B. рассылка сущности: свойства (здоровье, воздух, флаги), скорость после
//      удара, новые модификаторы бега;
//   C. такт сущности: огонь, лава, бездна, воздух, еда, недоеденное;
//   D. Set Health, если здоровье, еда или насыщение изменились.
// Поэтому изменённое в C уходит клиенту свойствами только в следующем такте.

use super::*;

use crate::attributes::{Modifier, Operation};
use crate::survival::{self, death_text, landing_multiplier, soft_block, Source, Vitals};

/// Serverbound Player Command: бег и прочее.
pub(super) const PLAYER_COMMAND: i32 = 0x2A;

/// Serverbound Client Command: «возродиться» (действие 0).
pub(super) const CLIENT_COMMAND: i32 = 0x0C;

/// Hurt Animation (клиентский пакет 0x2a) и свойство «число стрел в теле».
const HURT_ANIMATION: i32 = 0x2A;
const ARROW_COUNT_PROPERTY: u8 = 12;

const DAMAGE_EVENT: i32 = 0x19;
const ENTITY_EVENT: i32 = 0x22;
const SET_COOLDOWN: i32 = 0x16;
const DEATH_COMBAT_EVENT: i32 = 0x44;
const SET_EXPERIENCE: i32 = 0x67;
const UPDATE_ATTRIBUTES: i32 = 0x83;
const ENTITY_EFFECT: i32 = 0x84;
const REMOVE_ENTITY_EFFECT: i32 = 0x4E;

/// Свойства сущности: воздух, «использует предмет», здоровье; виды значений.
const AIR_PROPERTY: u8 = 1;
const LIVING_FLAGS_PROPERTY: u8 = 8;
const POSE_PROPERTY: u8 = 6;
/// Вид значения «поза» и сама поза вращения «тягуна» (minecraft-data:
/// pose, spin_attack — 4).
const PROPERTY_POSE: i32 = 20;
pub(super) const POSE_SPIN_ATTACK: i32 = 4;

/// Флаг живой сущности «вращается» (тягун) — свойство 8, бит 4.
const LIVING_SPIN: u8 = 4;

/// Сколько тактов длится вращение «тягуна»: флаг 8=4 при броске, поза
/// вращения со следующего такта, оба сняты на 20-м (чёрный ящик,
/// trident-riptide: 8=4 на +0, 6=4 на +1, «6=0 8=0» на +20).
pub(super) const SPIN_TICKS: u32 = 20;
const HEALTH_PROPERTY: u8 = 9;
const AMBIENT_PROPERTY: u8 = 11;
const ABSORPTION_PROPERTY: u8 = 17;
const SCORE_PROPERTY: u8 = 18;
const PROPERTY_VARINT: i32 = 1;
const PROPERTY_FLOAT: i32 = 3;
const PROPERTY_BOOLEAN: i32 = 8;

/// Общие флаги сущности: горит, бежит.
const FLAG_ON_FIRE: u8 = 0x01;
const FLAG_SPRINTING: u8 = 0x08;
const FLAG_INVISIBLE: u8 = 0x20;
const FLAG_GLOWING: u8 = 0x40;

/// События сущности: обычная смерть, «доел» и утопление.
const EVENT_DEATH: i8 = 3;
const EVENT_FINISHED_EATING: i8 = 9;
const EVENT_DROWN: i8 = 67;
/// Тотем бессмертия сработал (вики, Entity statuses: 35).
const EVENT_TOTEM: i8 = 35;

/// Номера свойств в пакете Update Attributes (порядок minecraft-data,
/// названия там сдвинуты: 22 — скорость движения, 23 — кислородный бонус).
const ATTRIBUTE_ATTACK_SPEED: i32 = 4;
const ATTRIBUTE_INTERACTION_RANGE: i32 = 10;
const ATTRIBUTE_BLOCK_RANGE: i32 = 6;
const ATTRIBUTE_MOVEMENT_SPEED: i32 = 22;
const ATTRIBUTE_OXYGEN: i32 = 23;

/// Сколько тактов длится еда обычной пищи; сушёная ламинария — вдвое короче.
const EAT_TICKS: i32 = 32;

/// Высота, ниже которой начинается бездна: дно мира минус 64 блока. У
/// Незера и Энда настоящее дно — y 0, у нас они лежат в общей сетке
/// с −64, поэтому бездна у них начинается на −64, как у оригинала.
fn void_below(dimension: crate::dimension::Dimension) -> f64 {
    match dimension {
        crate::dimension::Dimension::Overworld => world::MIN_Y as f64 - 64.0,
        _ => -64.0,
    }
}

/// Удар, ждущий своего места в такте.
pub(super) struct Pending {
    pub amount: f32,
    pub source: Source,
    /// Номер сущности виновника (динамит); 0 — нет.
    pub direct: i32,
    /// Направление анимации ушиба (шипы) и виновник для сообщения о смерти.
    pub hurt_yaw: Option<f32>,
    pub killer: Option<(String, [u8; 16])>,
    /// Отдача от ударившего (шипы): единичное направление по горизонтали.
    pub push: Option<(f64, f64)>,
    /// Остальным игрокам об этом уроне уже сообщено в такт удара (шипы).
    pub announced: bool,
}

/// Чем занята рука, пока держится кнопка «использовать».
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UseKind {
    Shield,
    Bow,
    /// Арбалет заряжается; уровень «быстрой перезарядки».
    Crossbow { quick_charge: i32 },
    /// Трезубец занесён для броска.
    Trident,
}

/// Использование предмета, которое тянется: щит поднят, лук натянут.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Using {
    /// Рука: 0 — основная, 1 — вторая.
    pub hand: i32,
    /// С какого такта.
    pub since: u64,
    pub kind: UseKind,
}

/// Идущая еда.
pub(super) struct Eating {
    pub ticks_left: i32,
    pub nutrition: i32,
    pub saturation: f32,
    /// Эффекты, которые даёт еда (золотые яблоки).
    pub effects: Vec<survival::Effect>,
    /// Питьё: молоко снимает эффекты, после питья остаётся посуда.
    pub drink: Option<Drink>,
}

/// Что пьётся (вики, Milk Bucket и Potion: пьётся 32 такта).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Drink {
    /// Ведро молока: снимает все эффекты, остаётся ведро.
    Milk,
    /// Зелье: даёт свои эффекты, остаётся пузырёк.
    Potion,
}

impl Drink {
    /// Что остаётся в руке после питья.
    fn remainder(self) -> &'static str {
        match self {
            Drink::Milk => "bucket",
            Drink::Potion => "glass_bottle",
        }
    }
}

/// Что клиент уже знает о свойствах игрока.
#[derive(Clone, Copy, PartialEq)]
struct Meta {
    score: i32,
    arrows: i32,
    health: f32,
    air: i32,
    flags: u8,
    /// Поза (свойство 6): своему клиенту уходит только поза вращения.
    pose: i32,
    living: u8,
    effects: [i32; survival::EFFECT_PARTICLES],
    /// Свойство 11: эффекты фоновые (см. `Vitals::effects_ambient`).
    ambient: bool,
    /// Свойство 17: поглощение (effect-absorption: Metadata 17=8).
    absorption: f32,
}

/// Сетевая сторона здоровья игрока.
pub(super) struct Net {
    pub hits: Vec<Pending>,
    /// Урон, что сейчас применяется, уже разослан остальным (см. `Pending::announced`).
    pub announced: bool,
    /// Здоровье, которое остальные должны увидеть до применения шипов у самого игрока.
    pub announced_health: Option<f32>,
    /// Частицы приземления, ждущие своего места в такте: состояние блока
    /// под ногами и число.
    pub landings: Vec<(i32, i32)>,
    /// Опыт, который должен появиться шариками (добытая руда): место и очки.
    pub new_orbs: Vec<((f64, f64, f64), i32)>,
    /// Очки, подобранные шариками за этот такт: прибавятся в `orbs_tick`.
    pub xp_taken: i32,
    sent_health: f32,
    sent_food: i32,
    sent_saturation_zero: bool,
    meta: Meta,
    /// Горит ли он для свойств: обновляется в начале такта сущности.
    fire_flag: bool,
    /// Скорость, которую надо сообщить после удара.
    pub velocity: Option<(f64, f64, f64)>,
    /// Вертикальная скорость, которую сервер ведёт сам после удара в воздухе:
    /// пока игрок не встал на землю, клиентский шаг её не заменяет
    /// (чёрный ящик, void-death: скорости следующих ударов — десять тактов
    /// свободного падения от прошлой, хотя сам клиент после каждого пакета
    /// скорости падает заново).
    pin: Option<f64>,
    /// После удара клиенту надо сообщить скорость: считается в рассылке —
    /// по последнему шагу клиента к этому моменту, а не к моменту удара
    /// (чёрный ящик, void-death: скорость на такт позже удара несёт шаг этого
    /// же такта).
    velocity_due: bool,
    prev_ground: bool,
    pub last_delta: (f64, f64, f64),
    pub eating: Option<Eating>,
    /// Поднятый щит или натянутый лук.
    pub using: Option<Using>,
    /// Рука, которой в последний раз что-то использовали: её бит остаётся
    /// в флагах и после отпускания (чёрный ящик, pvp-shield: 8=2).
    pub last_hand: i32,
    /// Сколько стрел торчит в игроке и с какого такта его свойства снова уходят
    /// (после попадания стрелы они идут на такт позже: чёрный ящик, pvp-bow).
    pub arrows: i32,
    pub meta_hold: u64,
    /// Скорость после попадания стрелы уходит на такт позже, уже пройдя шаг.
    pub delayed_velocity: Option<(u64, (f64, f64, f64))>,
    /// До какого такта щит отключён ударом топора.
    pub shield_disabled_until: u64,
    /// «Тягун»: сколько тактов здоровья прошло с броска (пока идёт
    /// вращение) и где игрок был в прошлом такте — по пути между ними ищутся
    /// задетые вращением.
    pub spin_since: Option<u32>,
    pub spin_pose: bool,
    pub spin_from: (f64, f64, f64),
    slot_update: Option<i32>,
    /// Слоты, которые сервер поменял сам (износ вещей) и которые надо показать клиенту.
    slots_dirty: Vec<i32>,
    /// Слоты, поменявшиеся в такте сущности, которые клиенту уходят
    /// следующим кругом (чёрный ящик, crossbow-shoot: зарядка на +25, ячейки
    /// арбалета и стрел — на +26).
    slots_later: Vec<i32>,
    /// Клиент нажал «возродиться».
    pub respawn: bool,
    /// Где умер: точка смерти в пакете Respawn.
    pub died_at: Option<(Dimension, (i32, i32, i32))>,
    /// Игрока только что перенесли: первый пакет места после этого не считается движением.
    pub teleported: bool,
    /// Среда игрока по последнему такту: лава, вода.
    in_lava: bool,
    pub in_water: bool,
    /// Ключ имени того, кто нанёс последний удар (моб, стрела, крипер):
    /// для сообщения о смерти; пусто — никто.
    pub killer: &'static str,
    /// Откуда пришёл удар моба или стрелы (только на время `mob_hit`).
    hurt_from: Option<(f64, f64, f64)>,
    /// Стрел, застрявших в теле, и тактов до убыли одной.
    arrow_timer: i32,
    /// Свойства и скорость после удара моба уходят на такт позже урона
    /// (чёрный ящик: Set Health в такте удара, Metadata и скорость — в
    /// следующем).
    pub hold_meta: bool,
}

impl Net {
    /// Слот поменялся не по воле клиента: в конце такта он уйдёт ему целиком.
    /// Частицы эффектов уйдут клиенту ещё раз, даже без перемен (эффект от
    /// зелья обновлён: lingering-potion-feet).
    pub fn renew_effect_particles(&mut self) {
        self.meta.effects[0] = i32::MIN;
    }

    pub fn slot_changed(&mut self, slot: i32) {
        if !self.slots_dirty.contains(&slot) {
            self.slots_dirty.push(slot);
        }
    }

    /// Слот поменялся в такте сущности: клиенту — на круг позже.
    pub fn slot_changed_later(&mut self, slot: i32) {
        if !self.slots_later.contains(&slot) {
            self.slots_later.push(slot);
        }
    }

    pub fn new() -> Self {
        Self {
            hits: Vec::new(),
            announced: false,
            announced_health: None,
            landings: Vec::new(),
            new_orbs: Vec::new(),
            xp_taken: 0,
            sent_health: survival::MAX_HEALTH,
            sent_food: 20,
            sent_saturation_zero: false,
            meta: Meta { score: 0, arrows: 0, health: survival::MAX_HEALTH, air: survival::MAX_AIR, flags: 0, pose: 0, living: 0, effects: [0; survival::EFFECT_PARTICLES], ambient: false, absorption: 0.0 },
            fire_flag: false,
            velocity: None,
            pin: None,
            velocity_due: false,
            prev_ground: false,
            last_delta: (0.0, 0.0, 0.0),
            eating: None,
            using: None,
            last_hand: 0,
            arrows: 0,
            meta_hold: 0,
            delayed_velocity: None,
            shield_disabled_until: 0,
            spin_since: None,
            spin_pose: false,
            spin_from: (0.0, 0.0, 0.0),
            slot_update: None,
            slots_dirty: Vec::new(),
            slots_later: Vec::new(),
            respawn: false,
            died_at: None,
            teleported: false,
            in_lava: false,
            in_water: false,
            killer: "",
            hurt_from: None,
            arrow_timer: 0,
            hold_meta: false,
        }
    }

    /// Клиент только вошёл: Set Health с этими числами уйдёт при входе, так
    /// что «уже сообщено» — они.
    pub fn synced(vit: &Vitals) -> Self {
        let mut net = Self::new();

        net.sent_health = vit.health;
        net.sent_food = vit.food;
        net.sent_saturation_zero = vit.saturation == 0.0;
        net.meta.health = vit.health;
        net.meta.air = vit.air;
        net.meta.score = vit.xp;

        net
    }

    /// После возрождения клиент знает всё заново.
    fn reset(&mut self) {
        let (respawn, died_at) = (self.respawn, self.died_at);

        *self = Self::new();
        self.respawn = respawn;
        self.died_at = died_at;
    }
}

/// Еда: питательность, насыщение и время на еду.
struct Food {
    nutrition: i32,
    saturation: f32,
    ticks: i32,
    always: bool,
}

/// Еда по вики (Food). У ели с эффектами (сырая курица, ядовитый картофель,
/// иглобрюх…) эффекты пока не считаются [проверить].
fn food_of(name: &str) -> Option<Food> {
    let (nutrition, saturation) = match name {
        "apple" => (4, 2.4),
        "baked_potato" => (5, 6.0),
        "beef" => (3, 1.8),
        "beetroot" => (1, 1.2),
        "beetroot_soup" => (6, 7.2),
        "bread" => (5, 6.0),
        "carrot" => (3, 3.6),
        "chicken" => (2, 1.2),
        "cod" => (2, 0.4),
        "cooked_beef" => (8, 12.8),
        "cooked_chicken" => (6, 7.2),
        "cooked_cod" => (5, 6.0),
        "cooked_mutton" => (6, 9.6),
        "cooked_porkchop" => (8, 12.8),
        "cooked_rabbit" => (5, 6.0),
        "cooked_salmon" => (6, 9.6),
        "cookie" => (2, 0.4),
        "dried_kelp" => (1, 0.6),
        "glow_berries" => (2, 0.4),
        "golden_apple" | "enchanted_golden_apple" => (4, 9.6),
        "golden_carrot" => (6, 14.4),
        "melon_slice" => (2, 1.2),
        "mushroom_stew" => (6, 7.2),
        "mutton" => (2, 1.2),
        "porkchop" => (3, 1.8),
        "potato" => (1, 0.6),
        "pumpkin_pie" => (8, 4.8),
        "rabbit" => (3, 1.8),
        "rabbit_stew" => (10, 12.0),
        "rotten_flesh" => (4, 0.8),
        "salmon" => (2, 0.4),
        "spider_eye" => (2, 3.2),
        "sweet_berries" => (2, 0.4),
        "tropical_fish" => (1, 0.2),
        _ => return None,
    };

    Some(Food {
        nutrition,
        saturation,
        ticks: if name == "dried_kelp" { EAT_TICKS / 2 } else { EAT_TICKS },
        always: matches!(name, "golden_apple" | "enchanted_golden_apple"),
    })
}

/// Играет ли игрок «по-настоящему»: в выживании и приключении урон и голод
/// работают, у творческого и наблюдателя — нет.
fn mortal(mode: i32) -> bool {
    mode == GAME_MODE_SURVIVAL || mode == GAME_MODE_ADVENTURE
}

/// Что вокруг игрока: лава, вода в ногах и на уровне глаз, туловище в блоке.
struct Env {
    in_fire: bool,
    cactus: bool,
    in_wall: bool,
    in_lava: bool,
    in_water: bool,
    eyes_in_water: bool,
    soft: bool,
    landing: f32,
    below: i32,
}

/// Высота жидкости в клетке: источник 8/9, течение — по уровню, под такой же
/// жидкостью — полная клетка.
fn fluid_height(world: &World, x: i32, y: i32, z: i32, kind: fluids::Kind, level: i32) -> f64 {
    let above = fluids::fluid_at(world.get_block(x, y + 1, z));

    if above.is_some_and(|(above_kind, _)| above_kind == kind) {
        return 1.0;
    }

    if level >= 8 {
        return 8.0 / 9.0;
    }

    f64::from(8 - level) / 9.0
}

fn look_around(world: &World, x: f64, y: f64, z: f64, sneaking: bool) -> Env {
    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    let mut env = Env { in_fire: false, cactus: false, in_wall: false, in_lava: false, in_water: false, eyes_in_water: false, soft: false, landing: 1.0, below: 0 };

    // Ящик игрока: от ног до макушки; жидкость считается, если её
    // поверхность выше ног.
    let top = y + if sneaking { 1.5 } else { 1.8 };

    for by in (y.floor() as i32)..=(top.floor() as i32) {
        let state = world.get_block(bx, by, bz);

        if let Some((kind, level)) = fluids::fluid_at(state) {
            let surface = f64::from(by) + fluid_height(world, bx, by, bz, kind, level);

            if surface > y + 0.001 {
                match kind {
                    fluids::Kind::Water => env.in_water = true,
                    fluids::Kind::Lava => env.in_lava = true,
                }
            }
        }
    }

    let feet = world.get_block(bx, y.floor() as i32, bz);

    // Огонь в клетке ног или головы; кактус в клетках вокруг ног (ящик
    // шириной 0,6 задевает соседние клетки у края); туловище в целом блоке.
    for by in (y.floor() as i32)..=((y + 1.0).floor() as i32) {
        if blocks::block_at_state(world.get_block(bx, by, bz)).is_some_and(|name| name == "fire" || name == "soul_fire") {
            env.in_fire = true;
        }

        for (cx, cz) in [(x - 0.31, z), (x + 0.31, z), (x, z - 0.31), (x, z + 0.31)] {
            let name = blocks::block_at_state(world.get_block(cx.floor() as i32, by, cz.floor() as i32));

            if name == Some("cactus") {
                env.cactus = true;
            }
        }
    }

    let head = world.get_block(bx, (y + if sneaking { 1.27 } else { 1.62 }).floor() as i32, bz);

    env.in_wall = blocks::full_cube(head) && fluids::fluid_at(head).is_none();

    env.soft = env.in_water || env.in_lava || blocks::block_at_state(feet).is_some_and(soft_block);

    // Глаза: 1,62 у стоящего, 1,27 у присевшего.
    let eye = y + if sneaking { 1.27 } else { 1.62 };
    let (ex, ey, ez) = (bx, eye.floor() as i32, bz);

    if let Some((fluids::Kind::Water, level)) = fluids::fluid_at(world.get_block(ex, ey, ez)) {
        env.eyes_in_water = eye < f64::from(ey) + fluid_height(world, ex, ey, ez, fluids::Kind::Water, level);
    }

    let below = world.get_block(bx, (y - 0.2).floor() as i32, bz);

    env.below = below;
    env.landing = blocks::block_at_state(below).map(landing_multiplier).unwrap_or(1.0);

    env
}

/// Пакет движения от клиента: прыжок, бег по земле и приземление.
pub(super) fn moved(player: &mut PlayerState, shared: &Shared, from: (f64, f64, f64)) {
    let (dx, dy, dz) = (player.x - from.0, player.y - from.1, player.z - from.2);
    let on_ground = player.on_ground;

    // После переноса первый пакет места — не шаг: прежнее место сервер мог
    // успеть перезаписать пакетом со старого места.
    // Шаг длиннее десяти блоков за такт не движение, а перенос (падающий не
    // разгоняется быстрее четырёх блоков за такт).
    let jump = dx * dx + dy * dy + dz * dz > 100.0;

    if std::mem::take(&mut player.net.teleported) || jump {
        player.net.pin = None;
        player.net.prev_ground = on_ground;
        player.net.last_delta = (0.0, 0.0, 0.0);
        player.vit.fall = 0.0;

        return;
    }

    if mortal(player.game_mode) && !player.vit.dead {
        if player.net.prev_ground && !on_ground && dy > 0.0 {
            player.vit.jumped();
        }

        if on_ground && player.vit.sprinting {
            player.vit.ran(dx.hypot(dz));
        }

        let env = {
            let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            look_around(&world, player.x, player.y, player.z, player.sneaking)
        };

        let damage = player.vit.land(dy, on_ground, env.soft, env.landing);

        // Частицы приземления не зависят от урона (сено, мёд, стойкость их не
        // отменяют); число — по высоте до последнего шага (survival::
        // landing_particles).
        let count = survival::landing_particles(player.vit.landed_from);

        if count > 0 {
            player.net.landings.push((env.below, count));
        }

        if let Some(damage) = damage {
            log_debug!("Play: {} приземлился с уроном {} на {:.2} {:.2} {:.2} (шаг {:.3})", player.name, damage, player.x, player.y, player.z, dy);

            player.net.hits.push(Pending { amount: damage, source: Source::Fall, direct: 0, hurt_yaw: None, killer: None, push: None, announced: false });
        }
    }

    if on_ground {
        player.net.pin = None;
    }

    player.net.prev_ground = on_ground;
    player.net.last_delta = (dx, dy, dz);
}

/// Шаг вертикальной скорости за такт: воздух — тяжесть 0,08 и сопротивление
/// 0,98; лава — вдвое медленнее и 0,02 вниз; вода — 0,8 и 0,02 вниз [проверить].
/// Чёрный ящик: void-death (воздух), lava-burn (лава: −0,652·0,5−0,02 = −0,346).
fn medium_step(v: f64, in_lava: bool, in_water: bool) -> f64 {
    if in_lava {
        v * 0.5 - 0.02
    } else if in_water {
        v * 0.8 - 0.02
    } else {
        (v - 0.08) * 0.98
    }
}

/// Разбор Player Command: бег.
pub(super) fn player_command(payload: &[u8], player: &mut PlayerState) {
    let mut cursor = Cursor::new(payload);

    if let (Some(_entity), Some(action)) = (cursor.varint(), cursor.varint()) {
        match action {
            1 => player.vit.sprinting = true,
            2 => player.vit.sprinting = false,
            _ => {}
        }
    }
}

/// Правая кнопка с едой в руке: начать есть. Возвращает, началась ли еда.
pub(super) fn start_eating(player: &mut PlayerState) -> bool {
    if !mortal(player.game_mode) || player.vit.dead || player.net.eating.is_some() {
        return false;
    }

    let Some(stack) = player.inventory.held() else {
        return false;
    };
    let name = blocks::item_name(stack.item).map_or("", |name| name.strip_prefix("minecraft:").unwrap_or(name));

    // Молоко и зелья пьются в любой сытости, 32 такта (вики, Milk Bucket, Potion).
    let drink = match name {
        "milk_bucket" => Some((Drink::Milk, Vec::new())),
        "potion" => Some((Drink::Potion, survival::potion_effects(stack.extras().potion.unwrap_or(0)))),
        _ => None,
    };

    if let Some((drink, effects)) = drink {
        player.net.eating = Some(Eating { ticks_left: 32, nutrition: 0, saturation: 0.0, effects, drink: Some(drink) });

        return true;
    }

    let Some(food) = food_of(name) else {
        return false;
    };

    // Сытому обычную еду не дают (вики, Food: «unless hungry»).
    if !food.always && !player.vit.hungry() {
        return false;
    }

    player.net.eating = Some(Eating { ticks_left: food.ticks, nutrition: food.nutrition, saturation: food.saturation, effects: survival::food_effects(name), drink: None });

    true
}

/// Отпустили кнопку: еда прервана.
pub(super) fn stop_eating(player: &mut PlayerState) {
    player.net.eating = None;
}

/// Флаги живой сущности в свойствах: 1 — использует предмет, 2 — левой рукой.
pub(super) fn living_flags(player: &PlayerState) -> u8 {
    let hand = if player.net.last_hand == 1 { 2 } else { 0 };

    if player.net.spin_since.is_some() {
        return LIVING_SPIN;
    }

    match (player.net.using, player.net.eating.is_some()) {
        (Some(using), _) => 1 | if using.hand == 1 { 2 } else { 0 },
        (None, true) => 1,
        (None, false) => hand,
    }
}

/// Правая кнопка со щитом или луком в руке `hand`: щит поднят, лук
/// натягивается. Возвращает, началось ли использование.
pub(super) fn start_using(shared: &Shared, player: &mut PlayerState, hand: i32, now: u64) -> bool {
    if !mortal(player.game_mode) || player.vit.dead || player.net.using.is_some() || player.net.eating.is_some() {
        return false;
    }

    let stack = if hand == 1 { player.inventory.slot(inventory::OFFHAND) } else { player.inventory.held() };
    let Some(name) = stack.and_then(|stack| crate::combat::item_name(stack.item)) else {
        return false;
    };

    let kind = match name {
        "shield" if now >= player.net.shield_disabled_until => UseKind::Shield,
        "bow" => UseKind::Bow,
        // Незаряженный арбалет заряжается, если есть чем (вики, Crossbow);
        // заряженный стреляет (`bow::crossbow_fire`).
        "crossbow" => {
            let data = stack.map(|stack| stack.extras()).unwrap_or_default();

            if data.charged > 0 || (player.game_mode != GAME_MODE_CREATIVE && super::bow::arrow_slot(player).is_none()) {
                return false;
            }

            UseKind::Crossbow { quick_charge: data.level("quick_charge") }
        }
        // Трезубец с одной единицей прочности не бросается (вики, Trident:
        // «1 durability or less remaining cannot be thrown»).
        "trident" => {
            let stack = stack.expect("имя есть только у предмета");

            if crate::combat::durability_left(&stack).is_some_and(|left| left <= 1) {
                return false;
            }

            // С «тягуном» — только в воде или под дождём (вики, Riptide).
            if stack.extras().level("riptide") > 0 && !super::bow::riptide_allowed(shared, player) {
                return false;
            }

            UseKind::Trident
        }
        _ => return false,
    };

    player.net.using = Some(Using { hand, since: now, kind });
    player.net.last_hand = hand;

    true
}

/// Отпустили кнопку: щит опущен, лук спущен (выстрел — arrows).
pub(super) fn release_use(player: &mut PlayerState, shared: &Shared) {
    let Some(using) = player.net.using.take() else {
        return;
    };

    let now = *shared.ticks.borrow();
    let held = now.saturating_sub(using.since) as u32;

    match using.kind {
        UseKind::Bow => super::bow::release(shared, player, using.hand, held),
        UseKind::Trident => super::bow::throw_trident(shared, player, using.hand, held),
        // Арбалет заряжается сам, пока кнопка держится (`tick_crossbow`);
        // отпустили — только опустили руки.
        UseKind::Crossbow { .. } | UseKind::Shield => {}
    }
}

/// Такт вращения «тягуна»: поза со следующего такта после броска, конец
/// через `SPIN_TICKS`; задетого по пути бьёт трезубцем, как рукой (вики,
/// Riptide: «the mob takes melee damage»), и вращение на этом кончается
/// [проверить]. Стоящий на месте никого не задевает: у оригинала жертва
/// вплотную к неподвижному роботу урона не получила (чёрный ящик,
/// trident-riptide-hit) — сам рывок считает клиент.
/// Счёт — по тактам здоровья: первый из них идёт в такт броска.
pub(super) fn tick_spin(shared: &Shared, player: &mut PlayerState, now: u64) {
    let Some(elapsed) = player.net.spin_since else {
        return;
    };
    let (pose, flag) = (player.net.spin_pose, true);

    player.net.spin_since = Some(elapsed + 1);
    let here = (player.x, player.y, player.z);
    let from = std::mem::replace(&mut player.net.spin_from, here);

    player.net.spin_pose = (1..SPIN_TICKS).contains(&elapsed);

    if elapsed >= SPIN_TICKS {
        player.net.spin_since = None;
    } else if elapsed >= 1 && from != here {
        // Путь за такт: ящик игрока в позе вращения (0,6 × 0,6) от прошлого
        // места до нынешнего.
        let low = (from.0.min(here.0) - 0.3, from.1.min(here.1), from.2.min(here.2) - 0.3);
        let high = (from.0.max(here.0) + 0.3, from.1.max(here.1) + 0.6, from.2.max(here.2) + 0.3);
        let touched = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .members()
            .iter()
            .filter(|member| member.uuid != player.uuid && member.dimension == player.dimension && member.game_mode != 3)
            .find(|member| {
                let (x, y, z) = (member.position.x, member.position.y, member.position.z);
                let height = if member.sneaking { 1.5 } else { 1.8 };

                x + 0.3 > low.0 && x - 0.3 < high.0 && y + height > low.1 && y < high.1 && z + 0.3 > low.2 && z - 0.3 < high.2
            })
            .map(|member| member.entity_id);

        if let Some(target) = touched {
            player.net.spin_since = None;
            player.net.spin_pose = false;
            super::pvp::strike_player(shared, player, target, now);
        }
    }

    if player.net.spin_pose != pose || player.net.spin_since.is_some() != flag {
        super::publish_player(shared, player);
    }
}

/// Такт зарядки арбалета: звуки натяжения (вики, Crossbow: loading_start,
/// loading_middle, loading_end; с «быстрой перезарядкой» — quick_charge_N)
/// и сама зарядка, когда натянуто до конца, — кнопку при этом не отпускают
/// (чёрный ящик, crossbow-shoot: стрела и арбалет в ячейках на +25, при
/// отпускании на +30 — только флаг «использует» снят).
pub(super) fn tick_crossbow(shared: &Shared, player: &mut PlayerState, now: u64) {
    let Some(Using { since, hand, kind: UseKind::Crossbow { quick_charge } }) = player.net.using else {
        return;
    };
    let held = now.saturating_sub(since) as u32;

    if let Some((name, volume)) = crate::combat::crossbow_loading_sound(held, quick_charge) {
        let dimension = player.dimension;
        let pitch = 1.0 / (world_random(shared, dimension) * 0.5 + 1.0) + 0.2;

        shared
            .world_in(dimension)
            .lock()
            .expect("мир захвачен другим потоком")
            .note_sound_player_at((player.x, player.y, player.z), name, volume, pitch);
    }

    // Заряженный арбалет соседи видят в том же такте (чёрный ящик,
    // crossbow-shoot: экипировка на +25 вместе с loading_end).
    if held == crate::combat::crossbow_charge_ticks(quick_charge) {
        super::bow::crossbow_load(player, hand);
        super::publish_player(shared, player);
    }
}

/// Hurt Animation: клиент наклоняет игрока; угол — откуда пришёл удар.
pub(super) async fn send_hurt_animation(stream: &mut Outgoing<'_>, entity_id: i32, yaw: f32) -> io::Result<()> {
    let mut body = encode_varint(HURT_ANIMATION);

    body.extend_from_slice(&encode_varint(entity_id));
    push_f32(&mut body, yaw);

    write_body(stream, body).await
}

/// Set Cooldown: предмет (группа) нельзя использовать столько тактов.
pub(super) async fn send_cooldown(stream: &mut Outgoing<'_>, group: &str, ticks: i32) -> io::Result<()> {
    let mut body = encode_varint(SET_COOLDOWN);

    body.extend_from_slice(&encode_string(group));
    body.extend_from_slice(&encode_varint(ticks));

    write_body(stream, body).await
}

/// Поднят ли щит так, чтобы держать удар (прошла задержка).
pub(super) fn shield_up(player: &PlayerState, now: u64) -> Option<i32> {
    let using = player.net.using?;

    (using.kind == UseKind::Shield && now >= using.since + u64::from(crate::combat::SHIELD_DELAY) && now >= player.net.shield_disabled_until).then_some(using.hand)
}

/// Кто убил игрока: его имя, опознаватель и оружие (для сообщения о смерти).
#[derive(Clone, Copy)]
pub(super) struct Killer<'a> {
    pub name: &'a str,
    pub uuid: [u8; 16],
    pub weapon: Option<Stack>,
}

/// Строка о смерти для чата и журнала.
fn death_line(source: Source, name: &str, killer: Option<Killer<'_>>) -> String {
    match killer {
        Some(killer) => survival::death_text_by(source, name, killer.name, killer.weapon.filter(|_| source != Source::MaceSmash).and_then(|stack| crate::combat::item_name(stack.item))),
        None => death_text(source, name),
    }
}

/// Сообщение о смерти для клиента: перевод с именем игрока (нажатие
/// предлагает `/tell`, наведение показывает игрока), как у оригинала.
fn death_message(source: Source, player: &PlayerState, mob: &str, killer: Option<Killer<'_>>) -> Vec<u8> {
    let named = |name: &str, uuid: [u8; 16]| {
        let ints = [
            i32::from_be_bytes([uuid[0], uuid[1], uuid[2], uuid[3]]),
            i32::from_be_bytes([uuid[4], uuid[5], uuid[6], uuid[7]]),
            i32::from_be_bytes([uuid[8], uuid[9], uuid[10], uuid[11]]),
            i32::from_be_bytes([uuid[12], uuid[13], uuid[14], uuid[15]]),
        ];

        nbt::Compound::new()
            .compound("click_event", nbt::Compound::new().string("action", "suggest_command").string("command", &format!("/tell {} ", name)))
            .string("insertion", name)
            .string("text", name)
            .compound(
                "hover_event",
                nbt::Compound::new().string("name", name).string("action", "show_entity").string("id", "minecraft:player").int_array("uuid", &ints),
            )
    };
    let mut with = vec![named(&player.name, player.uuid)];
    let mut key = source.death_key().to_string();

    if let Some(killer) = killer {
        with.push(named(killer.name, killer.uuid));

        // С оружием в руке: «… using [Меч]» (ключ с суффиксом `.item`); у
        // сокрушающего удара булавы оружие не называется (чёрный ящик,
        // pvp-mace-heavy: death.attack.mace_smash с двумя именами).
        if source != Source::MaceSmash
            && let Some(item) = killer.weapon.and_then(|stack| crate::combat::item_name(stack.item))
        {
            key.push_str(".item");
            with.push(nbt::Compound::new().string("text", &format!("[{}]", survival::item_title(item))));
        }
    } else if !mob.is_empty() {
        // Виновник — существо: его имя вторым аргументом («… убит зомби»).
        with.push(nbt::Compound::new().string("translate", mob));
    }

    nbt::Compound::new().compounds("with", with).string("translate", &key).encode_network()
}

async fn send_entity_event(stream: &mut Outgoing<'_>, entity_id: i32, status: i8) -> io::Result<()> {
    let mut body = encode_varint(ENTITY_EVENT);

    push_i32(&mut body, entity_id);
    body.push(status as u8);

    write_body(stream, body).await
}

/// Поля Damage Event: вид, виновник и прямой виновник (оба со сдвигом +1, 0 — нет).
pub(super) fn damage_fields(source: Source, direct: i32, cause: i32) -> (i32, i32, i32) {
    // Виновник: другой игрок (`cause`); у удара моба, стрелы, шипов и взрыва
    // крипера — он сам (чёрный ящик: cause=entity); у динамита и прочего — нет.
    let culprit = if cause != 0 {
        cause + 1
    } else if matches!(source, Source::MobAttack | Source::Arrow | Source::Trident | Source::PlayerExplosion | Source::Thorns) && direct != 0 {
        direct + 1
    } else {
        0
    };

    (source.type_id(), culprit, if direct == 0 { 0 } else { direct + 1 })
}

async fn send_damage_event(stream: &mut Outgoing<'_>, entity_id: i32, source: Source, direct: i32, cause: i32) -> io::Result<()> {
    let (type_id, culprit, direct) = damage_fields(source, direct, cause);
    let mut body = encode_varint(DAMAGE_EVENT);

    body.extend_from_slice(&encode_varint(entity_id));
    body.extend_from_slice(&encode_varint(type_id));
    body.extend_from_slice(&encode_varint(culprit)); // виновник
    body.extend_from_slice(&encode_varint(direct)); // прямой виновник
    body.push(0); // места удара нет

    write_body(stream, body).await
}

/// Звук в точке: раздел, громкость, высота.
async fn send_sound_at(
    stream: &mut Outgoing<'_>,
    name: &str,
    category: i32,
    (x, y, z): (f64, f64, f64),
    volume: f32,
    pitch: f32,
) -> io::Result<()> {
    let mut body = encode_varint(SOUND_EFFECT);

    body.extend_from_slice(&encode_varint(0));
    body.extend_from_slice(&encode_string(name));
    body.push(0);
    body.extend_from_slice(&encode_varint(category));
    push_i32(&mut body, (x * 8.0) as i32);
    push_i32(&mut body, (y * 8.0) as i32);
    push_i32(&mut body, (z * 8.0) as i32);
    body.extend_from_slice(&volume.to_be_bytes());
    body.extend_from_slice(&pitch.to_be_bytes());
    body.extend_from_slice(&0i64.to_be_bytes());

    write_body(stream, body).await
}

/// Частицы блока под ногами при приземлении.
async fn send_landing_particles(stream: &mut Outgoing<'_>, at: (f64, f64, f64), block: i32, count: i32) -> io::Result<()> {
    const PARTICLE_BLOCK: i32 = 1;

    let mut body = encode_varint(LEVEL_PARTICLES);

    body.push(0); // не издалека
    body.push(0); // не всегда
    for v in [at.0, at.1, at.2] {
        body.extend_from_slice(&v.to_be_bytes());
    }
    // Смещение 0, скорость 0,15 (чёрный ящик: probe-land-player).
    for v in [0.0f32, 0.0, 0.0, 0.15] {
        body.extend_from_slice(&v.to_be_bytes());
    }
    push_i32(&mut body, count);
    body.extend_from_slice(&encode_varint(PARTICLE_BLOCK));
    body.extend_from_slice(&encode_varint(block));

    write_body(stream, body).await
}

/// Set Health: здоровье, еда, насыщение.
pub(super) async fn send_health(stream: &mut Outgoing<'_>, vit: &Vitals) -> io::Result<()> {
    let mut body = encode_varint(SET_HEALTH);

    push_f32(&mut body, vit.health);
    body.extend_from_slice(&encode_varint(vit.food));
    push_f32(&mut body, vit.saturation);

    write_body(stream, body).await
}

/// Set Experience: доля шкалы, уровень, всего очков.
async fn send_experience(stream: &mut Outgoing<'_>, vit: &Vitals) -> io::Result<()> {
    let (level, bar) = survival::level_of(vit.xp);
    let mut body = encode_varint(SET_EXPERIENCE);

    push_f32(&mut body, bar);
    body.extend_from_slice(&encode_varint(level));
    body.extend_from_slice(&encode_varint(vit.xp));

    write_body(stream, body).await
}

/// При входе опыт сообщается, только если он есть.
pub(super) async fn send_experience_if_any(stream: &mut Outgoing<'_>, vit: &Vitals) -> io::Result<()> {
    if vit.xp > 0 { send_experience(stream, vit).await } else { Ok(()) }
}

/// Команда `experience`: очки или уровни, клиенту — Set Experience.
pub(super) async fn command_xp(stream: &mut Outgoing<'_>, player: &mut PlayerState, set: bool, amount: i32, levels: bool) -> io::Result<()> {
    player.vit.command_xp(set, amount, levels);
    send_experience(stream, &player.vit).await
}

/// Свойство 10 «частицы эффектов»: по частице `entity_effect` с цветом ARGB
/// на эффект (чёрный ящик, pvp-strength, effect-speed) или пустой список.
pub(super) fn push_effect_particles(body: &mut Vec<u8>, colors: &[i32]) {
    let shown: Vec<i32> = colors.iter().copied().filter(|color| *color != 0).collect();

    body.push(10);
    body.extend_from_slice(&encode_varint(17));
    body.extend_from_slice(&encode_varint(shown.len() as i32));

    for color in shown {
        body.extend_from_slice(&encode_varint(21));
        body.extend_from_slice(&color.to_be_bytes());
    }
}

/// Set Entity Data: только то, что изменилось.
async fn send_meta(stream: &mut Outgoing<'_>, entity_id: i32, now: Meta, before: Meta) -> io::Result<()> {
    let mut body = encode_varint(SET_ENTITY_DATA);

    body.extend_from_slice(&encode_varint(entity_id));

    if now.flags != before.flags {
        body.push(FLAGS_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_BYTE));
        body.push(now.flags);
    }

    if now.air != before.air {
        body.push(AIR_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_VARINT));
        body.extend_from_slice(&encode_varint(now.air));
    }

    if now.score != before.score {
        body.push(SCORE_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_VARINT));
        body.extend_from_slice(&encode_varint(now.score));
    }

    if now.pose != before.pose {
        body.push(POSE_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_POSE));
        body.extend_from_slice(&encode_varint(now.pose));
    }

    if now.living != before.living {
        body.push(LIVING_FLAGS_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_BYTE));
        body.push(now.living);
    }

    // Здоровье (9) раньше частиц (10): свойства идут по номерам
    // (effect-instant: Metadata 9=14 10=[]).
    if now.health != before.health {
        body.push(HEALTH_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_FLOAT));
        push_f32(&mut body, now.health);
    }

    if now.effects != before.effects {
        push_effect_particles(&mut body, &now.effects);
    }

    if now.ambient != before.ambient {
        body.push(AMBIENT_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
        body.push(u8::from(now.ambient));
    }

    if now.absorption != before.absorption {
        body.push(ABSORPTION_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_FLOAT));
        push_f32(&mut body, now.absorption);
    }

    if now.arrows != before.arrows {
        body.push(ARROW_COUNT_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_VARINT));
        body.extend_from_slice(&encode_varint(now.arrows));
    }

    body.push(PROPERTIES_END);

    write_body(stream, body).await
}

/// Атрибуты игрока к концу такта: модификаторы от предметов в руках и на
/// теле, модификатор бега, и клиенту уходит то, что изменилось с прошлой
/// рассылки (как у оригинала — Update Attributes только с изменившимся).
async fn sync_attributes(stream: &mut Outgoing<'_>, player: &mut PlayerState, now: u64) -> io::Result<()> {
    update_equipment(player, now);

    if player.vit.sprinting {
        player.attrs.set_modifier(crate::attributes::MOVEMENT_SPEED, Modifier::new("minecraft:sprinting", f64::from(0.3f32), Operation::AddMultipliedTotal));
    } else {
        player.attrs.remove_modifier(crate::attributes::MOVEMENT_SPEED, "minecraft:sprinting");
    }

    // Изменённое предметами уходит через два такта после смены (чёрный
    // ящик, pvp-armor: атрибуты брони после износа — на +2).
    let (due, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut player.attrs_touched).into_iter().partition(|(_, due)| *due <= now);
    let touched: Vec<i32> = due.iter().map(|(attribute, _)| *attribute).collect();
    let skip: Vec<i32> = waiting.iter().map(|(attribute, _)| *attribute).collect();

    player.attrs_touched = waiting;

    // Чужим клиентам скорость удара повторяется через такт после смены.
    let ready = player.speed_due.iter().filter(|due| **due <= now).count();

    player.speed_due.retain(|due| *due > now);
    player.speed_touch = player.speed_touch.wrapping_add(ready as u32);

    let changed = player.attrs_sent.changed_with(&player.attrs, &touched, &skip);

    if changed.is_empty() {
        return Ok(());
    }

    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&crate::attributes::encode_update(player.entity_id, &changed));
    write_body(stream, body).await
}

/// Пересчитывает модификаторы от предметов, если рука или броня сменились.
/// Зовётся и в конце такта, и перед ударом: урон берётся из атрибутов.
pub(super) fn update_equipment(player: &mut PlayerState, now: u64) {
    let equipment = equipment_of(player);

    if equipment == player.equip_seen {
        return;
    }

    // Смена вещи в руке сбрасывает перезарядку удара; износ той же вещи — нет.
    let held = |list: &[Option<Stack>]| list.first().copied().flatten().map(|stack| stack.item);

    if held(&equipment) != held(&player.equip_seen) && !player.equip_seen.is_empty() {
        player.last_attack = now;
    }

    player.equip_seen = equipment;

    // Атрибуты, которых касались предметы до и после: снятый и вновь
    // поставленный модификатор — изменение, даже с тем же значением.
    let mut touched: Vec<i32> = player.attrs_applied.iter().map(|(attribute, _)| *attribute).collect();

    crate::combat::refresh_equipment(&mut player.attrs, &mut player.attrs_applied, &player.inventory);
    touched.extend(player.attrs_applied.iter().map(|(attribute, _)| *attribute));
    touched.sort_unstable();
    touched.dedup();

    if touched.contains(&crate::attributes::ATTACK_SPEED) {
        player.speed_due.push(now + 1);
    }

    for attribute in &touched {
        player.inventory.note_attribute(*attribute as u8);
    }

    player.attrs_touched.extend(touched.into_iter().map(|attribute| (attribute, now + 2)));
}

/// Что на игроке из вещей, дающих атрибуты: рука, вторая рука, броня.
fn equipment_of(player: &PlayerState) -> Vec<Option<Stack>> {
    let mut out = vec![player.inventory.held(), player.inventory.slot(inventory::OFFHAND)];

    out.extend(crate::combat::ARMOR_SLOTS.iter().map(|(slot, _)| player.inventory.slot(*slot)));
    out
}

/// Update Attributes при возрождении: все свойства игрока.
async fn send_base_attributes(stream: &mut Outgoing<'_>, player: &PlayerState) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    // Основные пять и все, что когда-либо имели модификаторы (броня и
    // твёрдость после надетой брони): у оригинала они лежат в файле игрока,
    // и при возрождении уходят все, по номерам, до Set Health.
    let mut ids: Vec<i32> = vec![ATTRIBUTE_ATTACK_SPEED, ATTRIBUTE_INTERACTION_RANGE, ATTRIBUTE_BLOCK_RANGE, ATTRIBUTE_MOVEMENT_SPEED, ATTRIBUTE_OXYGEN];

    ids.extend(player.inventory.seen_attributes().iter().map(|id| i32::from(*id)).filter(|id| crate::attributes::syncable(*id)));
    ids.sort_unstable();
    ids.dedup();

    body.extend_from_slice(&encode_varint(player.entity_id));
    body.extend_from_slice(&encode_varint(ids.len() as i32));

    for id in ids {
        let base = match id {
            ATTRIBUTE_ATTACK_SPEED => 4.0,
            ATTRIBUTE_INTERACTION_RANGE => 3.0,
            ATTRIBUTE_BLOCK_RANGE => 4.5,
            ATTRIBUTE_MOVEMENT_SPEED => f64::from(0.1f32),
            _ => 0.0,
        };

        body.extend_from_slice(&encode_varint(id));
        push_f64(&mut body, base);
        body.extend_from_slice(&encode_varint(0));
    }

    write_body(stream, body).await
}

/// Что клиент должен знать о здоровье после возрождения: свойства и права.
pub(super) async fn send_respawn_extras(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let level = shared.ops.lock().expect("права захвачены другим потоком").level_of(&player.uuid);

    send_entity_event(stream, player.entity_id, (24 + level.clamp(0, 4)) as i8).await?;

    // Здоровье и части скина — всё, что не по умолчанию.
    let mut body = encode_varint(SET_ENTITY_DATA);

    body.extend_from_slice(&encode_varint(player.entity_id));
    body.push(HEALTH_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_FLOAT));
    push_f32(&mut body, survival::MAX_HEALTH);
    body.push(SKIN_PARTS_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_BYTE));
    body.push(player.skin_parts);
    body.push(PROPERTIES_END);
    write_body(stream, body).await?;

    send_base_attributes(stream, player).await?;
    send_health(stream, &player.vit).await?;
    send_experience(stream, &player.vit).await
}

/// Событие прав игрока (`24 + уровень`): после Respawn оригинал шлёт его заново.
pub(super) async fn send_op_level(stream: &mut Outgoing<'_>, shared: &Shared, player: &PlayerState) -> io::Result<()> {
    let level = shared.ops.lock().expect("права захвачены другим потоком").level_of(&player.uuid);

    send_entity_event(stream, player.entity_id, (24 + level.clamp(0, 4)) as i8).await
}

/// Здоровье и опыт заново: клиент после смены измерения знает их заново.
pub(super) async fn send_vitals(stream: &mut Outgoing<'_>, player: &PlayerState) -> io::Result<()> {
    send_health(stream, &player.vit).await?;
    send_experience(stream, &player.vit).await
}

/// Всё, что сообщается сразу после Respawn (до чанков): опыт и точка
/// возрождения — как у оригинала, по два раза.
pub(super) async fn send_respawn_intro(stream: &mut Outgoing<'_>, player: &PlayerState) -> io::Result<()> {
    send_default_spawn_position(stream, player.spawn_point).await?;
    send_experience(stream, &player.vit).await?;
    send_default_spawn_position(stream, player.spawn_point).await
}

/// Возрождение: всё заново, точка смерти остаётся в пакете Respawn.
pub(super) fn respawned(player: &mut PlayerState) {
    player.vit.respawned();
    player.net.reset();

    // Клиент после возрождения знает атрибуты по умолчанию.
    player.attrs = crate::attributes::Attributes::player();
    player.attrs_applied.clear();
    player.attrs_sent = crate::attributes::Sent::of(&player.attrs);
    player.equip_seen.clear();
}

/// Смерть: сообщение, событие и выпавшие вещи.
async fn die(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, source: Source, killer: Option<Killer<'_>>) -> io::Result<()> {
    let mob = player.net.killer;
    let message = death_message(source, player, mob, killer);

    let mut body = encode_varint(DEATH_COMBAT_EVENT);

    body.extend_from_slice(&encode_varint(player.entity_id));
    body.extend_from_slice(&message);
    write_body(stream, body).await?;

    // Сообщение о смерти уходит всем в чат; самому игроку — своим путём.
    let mut body = encode_varint(SYSTEM_CHAT);

    body.extend_from_slice(&message);
    body.push(0x00);
    write_body(stream, body).await?;

    shared
        .chat
        .lock()
        .expect("чат захвачен другим потоком")
        .push_rich(death_line(source, &player.name, killer), Some(player.uuid), Some(message.clone()));

    send_entity_event(stream, player.entity_id, EVENT_DEATH).await?;

    log_info!("{}", death_line(source, &player.name, killer));

    player.net.died_at = Some((player.dimension, block_of(player.x, player.y, player.z)));
    player.net.eating = None;
    player.net.using = None;
    player.net.hits.clear();
    player.net.landings.clear();

    // Вещи падают, если не включено `keepInventory` (вики, Keep inventory).
    if !shared.keep_inventory.load(std::sync::atomic::Ordering::Relaxed) {
        for slot in 0..inventory::SLOTS as i32 {
            if let Some(stack) = player.inventory.slot(slot) {
                let spread = |shared: &Shared| {
                    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

                    (f64::from(world.random_unit()) - 0.5) * 0.4
                };
                let velocity = (spread(shared), 0.2, spread(shared));

                drop_into_world(shared, player.dimension, stack, (player.x, player.y + 0.5, player.z), velocity, items::THROWN_DELAY);
                player.inventory.set(slot, None);
            }
        }
    }

    Ok(())
}

/// Удар моба или стрелы: как `hit`, плюс отдача (вики, Knockback: скорость
/// делится пополам и складывается с ударом 0,4 от виновника, вверх — не
/// больше 0,4, если игрок на земле).
#[allow(clippy::too_many_arguments)]
pub(super) async fn mob_hit(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    amount: f32,
    source: Source,
    attacker: i32,
    killer: &'static str,
    push: Option<(f64, f64)>,
    at: (f64, f64, f64),
) -> io::Result<()> {
    player.net.killer = killer;

    // Направление на виновника для «Hurt Animation» и звук стрелы — раньше
    // пакетов урона: они уходят изнутри `hit` (чёрный ящик, skeleton-arrow).
    player.net.hurt_from = Some(at);

    let done = hit(stream, shared, player, amount, source, attacker).await?;

    player.net.hurt_from = None;
    player.net.hold_meta = done.is_some();

    // Стрела застряла в теле: счёт в свойстве 12, убывает через 20·(30 − число)
    // тактов (вики, Arrow: «stuck in the body») [проверить].
    if done.is_some() && source == Source::Arrow {
        player.net.arrows += 1;
        player.net.arrow_timer = 20 * (30 - player.net.arrows);
    }

    if let (Some(_), Some((ux, uz))) = (done, push) {
        // Клиенту уходит скорость уже после трения такта: 0,4 · 0,546 = 0,218
        // по горизонтали и 0,275 вверх (чёрный ящик, skeleton-arrow: стоящий
        // игрок после стрелы получает (0,218; 0,275)) [проверить: на ходу].
        let (dx, dy, dz) = player.net.last_delta;
        let up = if player.on_ground { 0.275 } else { dy };

        player.net.velocity = Some((dx / 2.0 * 0.546 + ux * 0.218, up, dz / 2.0 * 0.546 + uz * 0.218));
    }

    Ok(())
}

/// Наносит удар прямо сейчас (Damage Event, гибель). Возвращает исход.
pub(super) async fn hit(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    amount: f32,
    source: Source,
    direct: i32,
) -> io::Result<Option<survival::Hit>> {
    hit_by(stream, shared, player, amount, source, direct, 0, None, None).await
}

#[allow(clippy::too_many_arguments)]
/// То же от другого игрока: `cause` — его сущность (виновник в Damage Event),
/// `killer` — кто и чем ударил, для сообщения о смерти.
pub(super) async fn hit_by(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    mut amount: f32,
    source: Source,
    direct: i32,
    cause: i32,
    killer: Option<Killer<'_>>,
    hurt_yaw: Option<f32>,
) -> io::Result<Option<survival::Hit>> {
    if source.scales_with_difficulty() {
        amount = survival::scaled(amount, shared.difficulty.load(std::sync::atomic::Ordering::Relaxed));
    }

    let immune = player.game_mode == GAME_MODE_CREATIVE || player.game_mode == GAME_MODE_SPECTATOR;
    let announced = std::mem::take(&mut player.net.announced);
    let Some(mut result) = player.vit.hurt(amount, source, immune) else {
        return Ok(None);
    };
    let totem = result.died && !source.bypasses_invulnerability() && take_totem(player);

    if totem {
        result.died = false;
        player.vit.dead = false;
        player.vit.health = 1.0;
    }

    send_damage_event(stream, player.entity_id, source, direct, cause).await?;

    {
        let (type_id, culprit, direct) = damage_fields(source, direct, cause);

        // Остальным о шипах сообщено в такт удара: здесь, в конце такта, их
        // рассылка уже ушла бы на такт позже (чёрный ящик, pvp-thorns).
        if !announced {
            shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_player_damaged(player.entity_id, type_id, culprit, direct);
        }
        shared.players.lock().expect("список игроков захвачен другим потоком").set_health(&player.uuid, player.vit.health);
    }

    // Анимация ушиба идёт сразу за событием урона, раньше сообщения о смерти.
    if let Some(yaw) = hurt_yaw {
        send_hurt_animation(stream, player.entity_id, yaw).await?;
    }

    // Удар моба или стрелы: анимация «ранен» с направлением на виновника,
    // у стрелы ещё звук попадания (чёрный ящик, skeleton-arrow).
    if let Some(from) = player.net.hurt_from
        && matches!(source, Source::MobAttack | Source::Arrow)
    {
        let yaw = ((from.2 - player.z).atan2(from.0 - player.x).to_degrees() - f64::from(player.yaw)) as f32;
        let mut body = encode_varint(HURT_ANIMATION);

        body.extend_from_slice(&encode_varint(player.entity_id));
        push_f32(&mut body, yaw);
        write_body(stream, body).await?;

        if source == Source::Arrow {
            let pitch = 1.2 + (world_random(shared, player.dimension) - 0.5) * 0.4;

            send_sound_at(stream, "minecraft:entity.arrow.hit", SOUND_PLAYERS, from, 1.0, pitch).await?;
        }
    }

    // Скорость после удара клиенту сообщается всегда, кроме ударов без
    // отдачи (утопление, голод, взрыв): чёрный ящик, drowning и fall-*.
    if !matches!(source, Source::Drown | Source::Starve | Source::Explosion | Source::PlayerExplosion) {
        player.net.velocity_due = true;
    }

    if totem {
        revive_by_totem(stream, player).await?;
    }

    if result.died {
        die(stream, shared, player, source, killer).await?;
    }

    Ok(Some(result))
}

/// Тотем бессмертия в руке (вики, Totem of Undying: «held in either hand»;
/// сперва основная рука [проверить]): расходуется, возвращает, был ли он.
fn take_totem(player: &mut PlayerState) -> bool {
    let Some(totem) = blocks::item_named("totem_of_undying") else {
        return false;
    };

    if player.inventory.held().is_some_and(|stack| stack.item == totem) {
        player.net.slot_update = player.inventory.spend_held();

        return true;
    }

    match player.inventory.slot(inventory::OFFHAND) {
        Some(stack) if stack.item == totem => {
            player.inventory.set(inventory::OFFHAND, inventory::take_one(Some(stack)));
            player.net.slots_dirty.push(inventory::OFFHAND);

            true
        }
        _ => false,
    }
}

/// Тотем сработал: здоровье 1, все эффекты сняты, затем регенерация II
/// на 45 с, поглощение II на 5 с и огнестойкость на 40 с (вики, Totem of
/// Undying), событие 35 — клиент сам показывает тотем и играет звук.
async fn revive_by_totem(stream: &mut Outgoing<'_>, player: &mut PlayerState) -> io::Result<()> {
    clear_effects(stream, player, None).await?;
    player.vit.absorption = 0.0;

    for (kind, amplifier, ticks) in [
        (survival::EffectKind::Regeneration, 1, 900),
        (survival::EffectKind::Absorption, 1, 100),
        (survival::EffectKind::FireResistance, 0, 800),
    ] {
        give_effect(stream, player, survival::Effect { kind, amplifier, ticks_left: ticks, ambient: false, visible: true }).await?;
    }

    send_entity_event(stream, player.entity_id, EVENT_TOTEM).await
}


/// Скорость, которую сервер сообщает игроку на следующем такте после
/// толчка взрывом: толчок прибавляется к скорости стоящего, и сервер
/// проживает один такт движения — тяжесть 0,08 и сопротивление 0,98 по
/// вертикали, трение 0,546 (земля) или 0,91 (воздух) по горизонтали.
/// Чёрный ящик, tnt-damage: толчок (−0,340; 0,103; −0,009) на земле дал
/// (−0,186; −0,055; −0,005).
pub(super) fn blast_velocity(player: &PlayerState, (px, py, pz): (f64, f64, f64)) -> (f64, f64, f64) {
    let (dx, dy, dz) = player.net.last_delta;
    let rest_y = if player.on_ground { -0.0784 } else { dy };
    let friction = if player.on_ground { 0.546 } else { 0.91 };

    ((dx + px) * friction, (rest_y + py - 0.08) * 0.98, (dz + pz) * friction)
}

/// Скорость, которую сервер сообщает после удара. Она берётся после одного
/// шага среды от прошлой скорости игрока: стоящий на земле — от нуля
/// (−0,0784 в воздухе, −0,02 в лаве), летящий — от последнего шага клиента,
/// а если сервер уже ведёт скорость сам (`pin`) — она и есть ответ.
fn hurt_velocity(player: &mut PlayerState) -> (f64, f64, f64) {
    let (dx, dy, dz) = player.net.last_delta;
    let vy = match player.net.pin {
        Some(pinned) => pinned,
        None => medium_step(if player.on_ground { 0.0 } else { dy }, player.net.in_lava, player.net.in_water),
    };

    if !player.on_ground {
        player.net.pin = Some(vy);
    }

    (dx, vy, dz)
}

/// Отдача от удара: скорость игрока гасится вдвое и получает толчок
/// силой `strength` по направлению `dir` (от нападающего), ослабленный
/// стойкостью (вики, Knockback). Клиенту она уйдёт в конце такта.
pub(super) fn knock_back(player: &mut PlayerState, dir: (f64, f64), strength: f64, resistance: f64) {
    let base = match player.net.velocity {
        Some(velocity) => velocity,
        None => hurt_velocity(player),
    };

    player.net.velocity = Some(crate::combat::knockback_velocity(base, dir, strength, resistance, player.on_ground));
    player.net.velocity_due = false;
}

/// Толчок взрывом принят: скорость уйдёт клиенту в рассылке следующего такта.
pub(super) fn blasted(player: &mut PlayerState, push: (f64, f64, f64)) {
    if mortal(player.game_mode) && !player.vit.dead {
        player.net.velocity = Some(blast_velocity(player, push));
    }
}

/// Такт здоровья игрока: стадии A–D (см. начало файла).
pub(super) async fn tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let entity = player.entity_id;
    let tick_now = *shared.ticks.borrow();

    tick_spin(shared, player, tick_now);
    let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);

    // Творческий игрок сыт полностью: насыщение 20 (чёрный ящик, pvp-thorns:
    // после возврата из творческого режима у оригинала saturation=20, у нас 5).
    if player.game_mode == GAME_MODE_CREATIVE {
        player.vit.food = 20;
        player.vit.saturation = 20.0;
    }

    // A. удары, ждавшие своего такта.
    for (block, count) in std::mem::take(&mut player.net.landings) {
        send_landing_particles(stream, (player.x, player.y, player.z), block, count).await?;
    }

    for pending in std::mem::take(&mut player.net.hits) {
        let killer = pending.killer.as_ref().map(|(name, uuid)| Killer { name, uuid: *uuid, weapon: None });

        player.net.announced = pending.announced;
        player.net.announced_health = None;

        let done = hit_by(stream, shared, player, pending.amount, pending.source, pending.direct, 0, killer, pending.hurt_yaw).await?;

        // Шипы отбрасывают ударившего, как обычный удар: скорость идёт сразу
        // за анимацией ушиба (чёрный ящик, pvp-thorns: (0; 0,361; −0,4)).
        if let (Some(_), Some(dir)) = (done, pending.push) {
            let resistance = player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE);

            knock_back(player, dir, crate::mob::BASE_KNOCKBACK, resistance);

            if let Some(velocity) = player.net.velocity.take() {
                send_velocity(stream, player.entity_id, velocity).await?;
            }
        }
    }

    // B. свойства сущности, скорость после удара, бег.
    let sprinting = player.vit.sprinting;
    let mut flags = 0;

    if player.net.fire_flag {
        flags |= FLAG_ON_FIRE;
    }

    if player.sneaking {
        flags |= 0x02;
    }

    if sprinting {
        flags |= FLAG_SPRINTING;
    }

    if player.vit.has_effect(survival::EffectKind::Invisibility) {
        flags |= FLAG_INVISIBLE;
    }

    if player.vit.has_effect(survival::EffectKind::Glowing) {
        flags |= FLAG_GLOWING;
    }

    let now = Meta {
        score: player.vit.xp,
        arrows: player.net.arrows,
        health: player.vit.health,
        air: player.vit.air,
        flags,
        pose: if player.net.spin_pose { POSE_SPIN_ATTACK } else { 0 },
        living: living_flags(player),
        effects: player.vit.effect_colors(),
        ambient: player.vit.effects_ambient().unwrap_or(player.net.meta.ambient),
        absorption: player.vit.absorption,
    };

    let held = std::mem::take(&mut player.net.hold_meta);

    if !held && now != player.net.meta && tick_now >= player.net.meta_hold {
        let before = player.net.meta;

        // Флаг «крадётся» клиент знает сам; учитываем его в «было».
        let mut before = before;
        before.flags = (before.flags & !0x02) | (now.flags & 0x02);

        if now != before {
            send_meta(stream, entity, now, before).await?;
        }

        player.net.meta = now;
    }

    let now = tick_now;

    sync_attributes(stream, player, now).await?;

    if std::mem::take(&mut player.net.velocity_due) {
        player.net.velocity = Some(hurt_velocity(player));
    }

    if !held && let Some(velocity) = player.net.velocity.take() {
        send_velocity(stream, entity, velocity).await?;
    }

    if let Some((due, velocity)) = player.net.delayed_velocity
        && tick_now >= due
    {
        player.net.delayed_velocity = None;
        send_velocity(stream, entity, velocity).await?;
    }

    if let Some(slot) = player.net.slot_update.take() {
        send_container_slot(stream, player, slot).await?;
    }

    for slot in std::mem::take(&mut player.net.slots_dirty) {
        send_container_slot(stream, player, slot).await?;
    }

    for slot in std::mem::take(&mut player.net.slots_later) {
        player.net.slot_changed(slot);
    }

    // C. такт сущности.
    if !player.vit.dead && mortal(player.game_mode) {
        entity_tick(stream, shared, player, difficulty).await?;
    } else if !player.vit.dead && player.game_mode == GAME_MODE_CREATIVE {
        creative_tick(stream, shared, player).await?;
    }

    // Шарики опыта: появление, полёт к игроку, подбор.
    if !player.vit.dead {
        orbs_tick(stream, shared, player).await?;
    }

    // Скорость, которую сервер ведёт после удара, за такт делает шаг.
    if let Some(pinned) = player.net.pin {
        player.net.pin = Some(medium_step(pinned, player.net.in_lava, player.net.in_water));
    }

    // Бой для нападающих: неуязвимость и щит (players.rs, CombatView).
    {
        let gear = player.gear();

        shared.players.lock().expect("список игроков захвачен другим потоком").set_gear(&player.uuid, gear);
    }

    // D. Set Health.
    let vit = &player.vit;

    if vit.health != player.net.sent_health
        || vit.food != player.net.sent_food
        || (vit.saturation == 0.0) != player.net.sent_saturation_zero
    {
        player.net.sent_health = vit.health;
        player.net.sent_food = vit.food;
        player.net.sent_saturation_zero = vit.saturation == 0.0;

        send_health(stream, vit).await?;
    }

    Ok(())
}

/// Шарики опыта игрока за такт: добытый опыт падает шариками в мир (их
/// увидят все), а игрок подбирает по одному в 2 такта тот, до которого
/// дотянулся (xp.rs, чёрный ящик cow-xp: подбор на 5-м и 7-м такте).
async fn orbs_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    for (at, points) in std::mem::take(&mut player.net.new_orbs) {
        let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ (*shared.ticks.borrow() << 20));

        shared
            .items_in(player.dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_experience(points, at, &mut || shared.next_entity_id(), &mut rng);
    }

    // Подобранное шариком (`take_orbs`) клиенту сообщается здесь, после
    // пакета подбора из журнала (чёрный ящик, cow-xp: подбор, потом Set
    // Experience).
    if player.net.xp_taken > 0 {
        player.vit.add_xp(std::mem::take(&mut player.net.xp_taken));
        send_experience(stream, &player.vit).await?;
    }

    Ok(())
}

/// Творческий режим: тонуть не тонет, но бездна бьёт и здесь.
async fn creative_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    if player.y < void_below(player.dimension) {
        hit(stream, shared, player, 4.0, Source::OutOfWorld, 0).await?;
    }

    player.vit.tick_invulnerable();

    // Застрявшие стрелы выпадают по одной.
    if player.net.arrows > 0 {
        player.net.arrow_timer -= 1;

        if player.net.arrow_timer <= 0 {
            player.net.arrows -= 1;
            player.net.arrow_timer = 20 * (30 - player.net.arrows);
        }
    }

    Ok(())
}

async fn entity_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, difficulty: u8) -> io::Result<()> {
    let entity = player.entity_id;
    let at = (player.x, player.y, player.z);
    let env = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        look_around(&world, player.x, player.y, player.z, player.sneaking)
    };

    player.vit.age = player.vit.age.wrapping_add(1);

    player.net.in_lava = env.in_lava;
    player.net.in_water = env.in_water;

    // Вода тушит огонь.
    if env.in_water && player.vit.fire > 0 {
        player.vit.extinguish();

        let pitch = 1.6 + world_random(shared, player.dimension) * 0.4;

        send_sound_at(stream, "minecraft:entity.generic.extinguish_fire", SOUND_PLAYERS, at, 0.7, pitch).await?;
    }

    // Огнестойкость: огонь горит, но не бьёт (вики, Fire Resistance) [проверить].
    if player.vit.fire_tick(env.in_lava) && !player.vit.has_effect(survival::EffectKind::FireResistance) {
        hit(stream, shared, player, 1.0, Source::OnFire, 0).await?;
    }

    // Признак «горит» — после убывания огня за такт, но до поджога этого
    // такта: загорание в лаве видно через такт (lava-burn: свойство через
    // 2 такта), а погасший от времени гаснет сразу (pvp-fire-aspect: +80).
    player.net.fire_flag = player.vit.fire > 0;

    if env.in_fire && !player.vit.dead {
        player.vit.ignite(160);
        hit(stream, shared, player, 1.0, Source::InFire, 0).await?;
    }

    if env.cactus && !player.vit.dead {
        hit(stream, shared, player, 1.0, Source::Cactus, 0).await?;
    }

    if env.in_wall && !player.vit.dead {
        hit(stream, shared, player, 1.0, Source::InWall, 0).await?;
    }

    if env.in_lava && !player.vit.dead {
        player.vit.ignite(survival::LAVA_FIRE_TICKS);
        player.vit.fall *= 0.5;

        if hit(stream, shared, player, 4.0, Source::Lava, 0).await?.is_some() {
            let pitch = 2.0 + world_random(shared, player.dimension) * 0.4;

            // Звук — на шаг впереди присланного места: последняя точка плюс
            // последний шаг клиента (чёрный ящик, lava-burn: робот прислал
            // −57,695 после шага −0,585, оригинал звучит на −58,25 — это
            // −57,695 − 0,585 = −58,28; в покое шаг нулевой и точки совпадают).
            let (dx, dy, dz) = player.net.last_delta;
            let ahead = (at.0 + dx, at.1 + dy, at.2 + dz);

            send_sound_at(stream, "minecraft:entity.generic.burn", SOUND_PLAYERS, ahead, 0.4, pitch).await?;

            // Погибший гаснет со звуком.
            if player.vit.dead {
                player.vit.extinguish();

                let pitch = 1.6 + world_random(shared, player.dimension) * 0.4;

                send_sound_at(stream, "minecraft:entity.generic.extinguish_fire", SOUND_PLAYERS, at, 0.7, pitch).await?;
            }
        }
    }

    if player.y < void_below(player.dimension) && !player.vit.dead {
        hit(stream, shared, player, 4.0, Source::OutOfWorld, 0).await?;
    }

    if !player.vit.dead && player.vit.air_tick(env.eyes_in_water, false) {
        send_entity_event(stream, entity, EVENT_DROWN).await?;
        hit(stream, shared, player, 2.0, Source::Drown, 0).await?;
    }

    // Еда считается и в такт смерти: изнурение успевает перейти в насыщение
    // (lava-burn: насыщение 1 в пакете смерти).
    if player.vit.dead {
        player.vit.food_tick(difficulty, shared.natural_regeneration.load(std::sync::atomic::Ordering::Relaxed));

        return Ok(());
    }

    // Мирная сложность лечит сама и кормит (вики, Difficulty: «Peaceful»).
    if difficulty == survival::PEACEFUL {
        if player.vit.health < survival::MAX_HEALTH && player.vit.age.is_multiple_of(20) {
            player.vit.heal(1.0);
        }

        if player.vit.food < 20 && player.vit.age.is_multiple_of(10) {
            player.vit.food += 1;
        }
    }

    let survival::EffectTick { hurt, gone } = player.vit.effects_tick();

    for (amount, source) in hurt {
        hit(stream, shared, player, amount, source, 0).await?;
    }

    if !gone.is_empty() {
        apply_effect_modifiers(player);
    }

    for gone in gone {
        let mut body = encode_varint(REMOVE_ENTITY_EFFECT);

        body.extend_from_slice(&encode_varint(entity));
        body.extend_from_slice(&encode_varint(gone.id()));
        write_body(stream, body).await?;
    }

    tick_crossbow(shared, player, *shared.ticks.borrow());

    // Недоеденное — после такта эффектов: эффект от еды впервые действует
    // в следующем такте (golden-apple: регенерация лечит на +1 после Effect).
    if let Some(eating) = &mut player.net.eating {
        eating.ticks_left -= 1;

        if eating.ticks_left <= 0 {
            let (nutrition, saturation, drink) = (eating.nutrition, eating.saturation, eating.drink);
            let effects = std::mem::take(&mut eating.effects);

            player.net.eating = None;

            match drink {
                Some(drink) => finish_drinking(stream, player, drink).await?,
                None => finish_eating(stream, shared, player, nutrition, saturation).await?,
            }

            // Эффекты еды — после звуков еды [проверить] (golden-apple).
            // Мгновенные из зелья действуют сразу, без пакета эффекта
            // (potion-drink: Set Health в такте глотка, Entity Effect нет).
            for effect in effects {
                match effect.kind {
                    survival::EffectKind::InstantHealth => player.vit.heal((4 << effect.amplifier.clamp(0, 20)) as f32),
                    survival::EffectKind::InstantDamage => {
                        hit(stream, shared, player, (6 << effect.amplifier.clamp(0, 20)) as f32, Source::Magic, 0).await?;
                    }
                    _ => give_effect(stream, player, effect).await?,
                }
            }
        }
    }

    if let Some(amount) = player.vit.food_tick(difficulty, shared.natural_regeneration.load(std::sync::atomic::Ordering::Relaxed)) {
        hit(stream, shared, player, amount, Source::Starve, 0).await?;
    }

    player.vit.tick_invulnerable();

    Ok(())
}

pub(super) fn world_random(shared: &Shared, dimension: Dimension) -> f32 {
    shared.world_in(dimension).lock().expect("мир захвачен другим потоком").random_unit()
}

async fn finish_eating(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, nutrition: i32, saturation: f32) -> io::Result<()> {
    let at = (player.x, player.y, player.z);

    player.vit.eat(nutrition, saturation);

    if player.game_mode != GAME_MODE_CREATIVE {
        player.net.slot_update = player.inventory.spend_held();
    }

    send_entity_event(stream, player.entity_id, EVENT_FINISHED_EATING).await?;

    let (a, b, c) = {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        (world.random_unit(), world.random_unit(), world.random_unit())
    };

    send_sound_at(stream, "minecraft:entity.generic.eat", SOUND_NEUTRAL, at, 1.0, 1.0 + (a - b) * 0.4).await?;
    send_sound_at(stream, "minecraft:entity.player.burp", SOUND_PLAYERS, at, 0.5, c * 0.1 + 0.9).await
}

/// Допили: молоко снимает эффекты, в руке остаётся посуда (в творческом
/// режиме — прежний предмет). Звук питья клиент играет сам [проверить].
async fn finish_drinking(stream: &mut Outgoing<'_>, player: &mut PlayerState, drink: Drink) -> io::Result<()> {
    if player.game_mode != GAME_MODE_CREATIVE {
        let slot = player.inventory.selected_slot();

        player.inventory.set(slot, blocks::item_named(drink.remainder()).map(|item| Stack::new(item, 1)));
        player.net.slot_update = Some(slot);
    }

    send_entity_event(stream, player.entity_id, EVENT_FINISHED_EATING).await?;

    if drink == Drink::Milk {
        clear_effects(stream, player, None).await?;
    }

    Ok(())
}

/// Эффект от команды: клиенту — Entity Effect.
pub(super) async fn give_effect(stream: &mut Outgoing<'_>, player: &mut PlayerState, effect: survival::Effect) -> io::Result<()> {
    if !player.vit.effect_replaced_by(effect.kind, effect.amplifier, effect.ticks_left) {
        return Ok(());
    }

    // Обновлённый эффект того же вида приходит без плавного появления
    // (lingering-potion-feet: первое наложение flags=14, повторные — 6).
    let renewed = player.vit.effects.iter().any(|old| old.kind == effect.kind);

    player.vit.give_effect_shown(effect);
    apply_effect_modifiers(player);

    let mut body = encode_varint(ENTITY_EFFECT);

    body.extend_from_slice(&encode_varint(player.entity_id));
    body.extend_from_slice(&encode_varint(effect.kind.id()));
    body.extend_from_slice(&encode_varint(effect.amplifier));
    body.extend_from_slice(&encode_varint(if effect.kind.instant() { 1 } else { effect.ticks_left }));
    body.push(if renewed { effect.flags() & !0x08 } else { effect.flags() });

    write_body(stream, body).await
}

/// Модификаторы атрибутов от действующих эффектов (сила, слабость, скорость,
/// замедление): ставятся по уровню, снятые эффекты убираются.
pub(super) fn apply_effect_modifiers(player: &mut PlayerState) {
    use crate::attributes::{MAX_ABSORPTION, MAX_HEALTH, SAFE_FALL_DISTANCE};

    for kind in survival::EffectKind::ALL {
        let Some((attribute, id, per_level, operation)) = kind.modifier() else { continue };

        match player.vit.effects.iter().find(|effect| effect.kind == kind) {
            Some(effect) => player.attrs.set_modifier(attribute, Modifier::new(id, f64::from(per_level as f32 * (effect.amplifier + 1) as f32), operation)),
            None => player.attrs.remove_modifier(attribute, id),
        }
    }

    // Прыгучесть ещё и поднимает безопасную высоту на 1 за уровень (вики,
    // Jump Boost: «safe fall distance») [проверить].
    match player.vit.effects.iter().find(|effect| effect.kind == survival::EffectKind::JumpBoost) {
        Some(effect) => player.attrs.set_modifier(SAFE_FALL_DISTANCE, Modifier::new("minecraft:effect.jump_boost", f64::from(effect.amplifier + 1), Operation::AddValue)),
        None => player.attrs.remove_modifier(SAFE_FALL_DISTANCE, "minecraft:effect.jump_boost"),
    }

    player.vit.apply_limits(player.attrs.value(MAX_HEALTH) as f32, player.attrs.value(MAX_ABSORPTION) as f32, player.attrs.value(SAFE_FALL_DISTANCE) as f32);
}

/// Снять все эффекты (или один).
pub(super) async fn clear_effects(stream: &mut Outgoing<'_>, player: &mut PlayerState, only: Option<survival::EffectKind>) -> io::Result<()> {
    let (cleared, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut player.vit.effects).into_iter().partition(|effect| only.is_none_or(|kind| effect.kind == kind));

    player.vit.effects = kept;

    apply_effect_modifiers(player);

    for effect in cleared {
        let mut body = encode_varint(REMOVE_ENTITY_EFFECT);

        body.extend_from_slice(&encode_varint(player.entity_id));
        body.extend_from_slice(&encode_varint(effect.kind.id()));
        write_body(stream, body).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foods_follow_the_wiki_table() {
        let bread = food_of("bread").expect("хлеб — еда");

        assert_eq!((bread.nutrition, bread.saturation, bread.ticks, bread.always), (5, 6.0, 32, false));
        assert_eq!(food_of("dried_kelp").expect("ламинария").ticks, 16);
        assert!(food_of("golden_apple").expect("яблоко").always);
        assert!(food_of("stone").is_none());
    }

    #[test]
    fn soft_blocks_and_landing_multipliers() {
        assert!(soft_block("cobweb") && soft_block("ladder") && !soft_block("stone"));
        assert_eq!(landing_multiplier("hay_block"), 0.2);
        assert_eq!(landing_multiplier("slime_block"), 0.0);
        assert_eq!(landing_multiplier("red_bed"), 0.5);
        assert_eq!(landing_multiplier("stone"), 1.0);
    }

    #[test]
    fn a_blast_push_becomes_one_tick_of_movement() {
        let mut player = PlayerState::new([1; 16], "Анна".to_string(), None, None, 0x7F, 1, (0.5, 64.0, 0.5));

        player.on_ground = true;

        let (x, y, z) = blast_velocity(&player, (-0.340, 0.1028, -0.0086));

        assert!((x + 0.1856).abs() < 1e-3 && (z + 0.0047).abs() < 1e-3, "{x} {z}");
        assert!((y + 0.0545).abs() < 1e-3, "{y}");
    }

    #[test]
    fn the_server_steps_the_speed_by_the_medium() {
        // Воздух: (−2,576 − 0,08)·0,98 = −2,603 (void-death).
        assert!((medium_step(-2.576, false, false) + 2.603).abs() < 1e-3);
        // Стоящий на земле: −0,0784.
        assert!((medium_step(0.0, false, false) + 0.0784).abs() < 1e-9);
        // Лава: −0,652·0,5 − 0,02 = −0,346 (lava-burn); покой −0,02.
        assert!((medium_step(-0.652, true, false) + 0.346).abs() < 1e-9);
        assert!((medium_step(0.0, true, false) + 0.02).abs() < 1e-9);

        // Десять шагов свободного падения от −2,6034 дают −2,8442 (void-death, второй удар).
        let mut v = -2.6033693462735763;

        for _ in 0..10 {
            v = medium_step(v, false, false);
        }

        assert!((v + 2.8442).abs() < 1e-3, "{v}");
    }

    #[test]
    fn only_survival_and_adventure_are_mortal() {
        assert!(mortal(GAME_MODE_SURVIVAL) && mortal(GAME_MODE_ADVENTURE));
        assert!(!mortal(GAME_MODE_CREATIVE) && !mortal(GAME_MODE_SPECTATOR));
    }

    #[test]
    fn death_text_names_the_player() {
        assert_eq!(death_text(Source::Fall, "Anya"), "Anya fell from a high place");
        assert_eq!(death_text(Source::Lava, "Anya"), "Anya tried to swim in lava");
    }
}
