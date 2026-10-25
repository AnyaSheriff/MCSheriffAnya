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
/// Жала пчёл в теле (свойство 13; чёрный ящик, bee-sting: `13=1` после жала).
const STINGER_COUNT_PROPERTY: u8 = 13;

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
/// Поза «полёт на элитрах» (minecraft-data 26.1: pose 1, fall_flying).
const POSE_FALL_FLYING: i32 = 1;
/// Поза «крадётся».
const POSE_CROUCHING: i32 = 5;
/// Флаг сущности «летит на элитрах».
const FLAG_FALL_FLYING: u8 = 0x80;
// Поза «спит», свойство 14 «место кровати» и его вид 11 — в play.rs
// (minecraft-data, protocol.json, entityMetadataEntry).

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
pub(super) const EVENT_TOTEM: i8 = 35;

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
    Brush,
    /// Мешок: по стопке за раз (bundle_use.rs).
    Bundle,
    Shield,
    Bow,
    /// Арбалет заряжается; уровень «быстрой перезарядки».
    Crossbow { quick_charge: i32 },
    /// Трезубец занесён для броска.
    Trident,
    /// Око Края: оригинал держит «использует предмет», пока кнопку не
    /// отпустят, — и после броска, и без крепости (чёрный ящик,
    /// eye-of-ender-far, eye-of-ender-none: Metadata 8=1).
    EnderEye,
    Spyglass,
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
    /// Начатая стопка и слот: смена предмета отменяет употребление.
    pub slot: i32,
    pub stack: Stack,
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
    /// Зловещая бутылка: «Дурное знамение» своего уровня, ничего не остаётся
    /// (вики, Ominous Bottle).
    Ominous,
    /// Вики, Honey Bottle: «2 seconds», снимает только Poison.
    Honey,
}

impl Drink {
    /// Что остаётся в руке после питья.
    pub(super) fn remainder(self) -> &'static str {
        match self {
            Drink::Milk => "bucket",
            Drink::Potion | Drink::Honey => "glass_bottle",
            Drink::Ominous => "",
        }
    }
}

/// Что клиент уже знает о свойствах игрока.
#[derive(Clone, Copy, PartialEq)]
struct Meta {
    score: i32,
    arrows: i32,
    stingers: i32,
    health: f32,
    air: i32,
    frozen: i32,
    flags: u8,
    /// Поза (свойство 6): своему клиенту уходит только поза вращения.
    pose: i32,
    living: u8,
    effects: [i32; survival::EFFECT_PARTICLES],
    /// Свойство 11: эффекты фоновые (см. `Vitals::effects_ambient`).
    ambient: bool,
    /// Свойство 17: поглощение (effect-absorption: Metadata 17=8).
    absorption: f32,
    /// Свойство 14: место кровати спящего (чёрный ящик, bed-sleep-night:
    /// «Metadata 6=2 14={…}»).
    bed: Option<(i32, i32, i32)>,
}

/// Сетевая сторона здоровья игрока.
pub(super) struct Net {
    /// Протокол дерева команд и последний действительно отправленный допуск.
    pub command_protocol: i32,
    pub command_access: Option<(i32, bool)>,
    pub completion_budget: super::command_completion::CompletionBudget,
    pub hits: Vec<Pending>,
    /// Урон, что сейчас применяется, уже разослан остальным (см. `Pending::announced`).
    pub announced: bool,
    /// Здоровье, которое остальные должны увидеть до применения шипов у самого игрока.
    pub announced_health: Option<f32>,
    /// Частицы приземления, ждущие своего места в такте: состояние блока
    /// под ногами и число.
    /// Третье поле — широкий разброс 0,3 (приземление после булавы).
    pub landings: Vec<(i32, i32, bool)>,
    /// sign-edit: звук /give у получателя следует за появлением вещи в том же такте.
    pub give_sounds: Vec<((f64, f64, f64), f32)>,
    /// Опыт, который должен появиться шариками (добытая руда): место и очки.
    pub new_orbs: Vec<((f64, f64, f64), i32)>,
    /// Очки, подобранные шариками за этот такт: прибавятся в `orbs_tick`.
    pub xp_taken: Vec<i32>,
    sent_health: f32,
    sent_food: i32,
    sent_saturation_zero: bool,
    meta: Meta,
    /// Горит ли он для свойств: обновляется в начале такта сущности.
    fire_flag: bool,
    /// Общие ticks_frozen; замедление клиента Bedrock [Bedrock: потом].
    frozen: i32,
    /// Скорость, которую надо сообщить после удара.
    pub velocity: Option<(f64, f64, f64)>,
    /// Вертикальная скорость, которую сервер ведёт сам после удара в воздухе:
    /// пока игрок не встал на землю, клиентский шаг её не заменяет
    /// (чёрный ящик, void-death: скорости следующих ударов — десять тактов
    /// свободного падения от прошлой, хотя сам клиент после каждого пакета
    /// скорости падает заново).
    pub(super) pin: Option<f64>,
    /// После удара клиенту надо сообщить скорость: считается в рассылке —
    /// по последнему шагу клиента к этому моменту, а не к моменту удара
    /// (чёрный ящик, void-death: скорость на такт позже удара несёт шаг этого
    /// же такта).
    pub(super) velocity_due: bool,
    /// После отложенного толчка777 повторно сверяется опора под ногами.
    pub(super) check_support: Option<u64>,
    prev_ground: bool,
    pub last_delta: (f64, f64, f64),
    /// fall-mode-teleport-motion: инерция creative → survival доживает
    /// до приземления; /tp её обнуляет. [проверить] Другие среды.
    mode_fall_velocity: Option<f64>,
    landing_velocity: Option<f64>,
    pub sweep_motion: super::sweep::Motion,
    pub eating: Option<Eating>,
    /// Поднятый щит или натянутый лук.
    pub using: Option<Using>,
    /// Рука, которой в последний раз что-то использовали: её бит остаётся
    /// в флагах и после отпускания (чёрный ящик, pvp-shield: 8=2).
    pub last_hand: i32,
    /// Сколько стрел торчит в игроке и с какого такта его свойства снова уходят
    /// (после попадания стрелы они идут на такт позже: чёрный ящик, pvp-bow).
    pub arrows: i32,
    /// Жала пчёл в теле: число и такты до убыли одного (как у стрел).
    pub stingers: i32,
    pub stinger_timer: i32,
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
    /// Сидел ли игрок в транспорте в прошлом такте (поза «присел» тогда не ставится).
    pub riding_prev: bool,
    pub sneak_riding: bool,
    /// С какого такта флаг «крадётся» скрыт после схода с транспорта (кнопка ещё зажата).
    pub hide_sneak_from: Option<u64>,
    pub spin_from: (f64, f64, f64),
    pub(super) slot_update: Option<i32>,
    /// Слоты, которые сервер поменял сам (износ вещей) и которые надо показать клиенту.
    pub(super) slots_dirty: Vec<i32>,
    /// Атрибуты брони и дальности, разосланные игроку: их увидят и остальные.
    pub(super) armor_out: Vec<crate::attributes::Attribute>,
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
    pub(super) in_lava: bool,
    pub in_water: bool,
    /// Drowning777: первое погружение создаёт синхронизируемый oxygen_bonus.
    oxygen_seen: bool,
    /// Ключ имени того, кто нанёс последний удар (моб, стрела, крипер):
    /// для сообщения о смерти; пусто — никто.
    pub killer: &'static str,
    /// Откуда пришёл удар моба или стрелы (только на время `mob_hit`).
    hurt_from: Option<(f64, f64, f64)>,
    command_origin: Option<crate::combat::DamageOrigin>,
    /// Стрел, застрявших в теле, и тактов до убыли одной.
    arrow_timer: i32,
    /// Свойства и скорость после удара моба уходят на такт позже урона
    /// (чёрный ящик: Set Health в такте удара, Metadata и скорость — в
    /// следующем).
    pub hold_meta: bool,
    /// Жемчуг попадает после хода игрока: стадия C уже выполнена до переноса.
    pearl_entity_ticked: bool,

    /// Последний удар моба нанёс урон (не погашен неуязвимостью): эффекты «от
    /// удара» (иссушение, голод) ложатся только тогда.
    pub mob_hit_landed: bool,

    /// Уровень «Знамения набега» (1…5): заявка на набег уходит, когда эффект
    /// уже снят (raid.rs).
    pub raid_omen_level: i32,

    /// «Дурное знамение» сменилось «Знамением набега» в этом такте: в
    /// следующем — повтор знамения и снятие дурного (omens).
    pub omen_switch: bool,
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
            command_protocol: 777,
            command_access: None,
            completion_budget: Default::default(),
            hits: Vec::new(),
            announced: false,
            announced_health: None,
            landings: Vec::new(),
            give_sounds: Vec::new(),
            new_orbs: Vec::new(),
            xp_taken: Vec::new(),
            sent_health: survival::MAX_HEALTH,
            sent_food: 20,
            sent_saturation_zero: false,
            meta: Meta { score: 0, arrows: 0, stingers: 0, health: survival::MAX_HEALTH, air: survival::MAX_AIR, frozen: 0, flags: 0, pose: 0, living: 0, effects: [0; survival::EFFECT_PARTICLES], ambient: false, absorption: 0.0, bed: None },
            fire_flag: false,
            frozen: 0,
            velocity: None,
            pin: None,
            velocity_due: false,
            check_support: None,
            prev_ground: false,
            last_delta: (0.0, 0.0, 0.0),
            mode_fall_velocity: None,
            landing_velocity: None,
            sweep_motion: Default::default(),
            eating: None,
            using: None,
            last_hand: 0,
            arrows: 0,
            stingers: 0,
            stinger_timer: 0,
            meta_hold: 0,
            delayed_velocity: None,
            shield_disabled_until: 0,
            spin_since: None,
            spin_pose: false,
            riding_prev: false,
            sneak_riding: false,
            hide_sneak_from: None,
            spin_from: (0.0, 0.0, 0.0),
            slot_update: None,
            slots_dirty: Vec::new(),
            armor_out: Vec::new(),
            slots_later: Vec::new(),
            respawn: false,
            died_at: None,
            teleported: false,
            in_lava: false,
            in_water: false,
            oxygen_seen: false,
            killer: "",
            hurt_from: None,
            command_origin: None,
            arrow_timer: 0,
            hold_meta: false,
            pearl_entity_ticked: false,
            mob_hit_landed: false,
            raid_omen_level: 0,
            omen_switch: false,
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
        net.meta.score = vit.xp_total;

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

/// Еда по вики (Food); эффекты — survival::food_effects/food_illness.
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
        "chorus_fruit" => (4, 2.4),
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
        "mushroom_stew" | "suspicious_stew" => (6, 7.2),
        "mutton" => (2, 1.2),
        "porkchop" => (3, 1.8),
        "potato" => (1, 0.6),
        "poisonous_potato" => (2, 1.2),
        "pufferfish" => (1, 0.2),
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
        always: matches!(name, "golden_apple" | "enchanted_golden_apple" | "suspicious_stew" | "chorus_fruit"),
    })
}

/// Играет ли игрок «по-настоящему»: в выживании и приключении урон и голод
/// работают, у творческого и наблюдателя — нет.
fn mortal(mode: i32) -> bool {
    mode == GAME_MODE_SURVIVAL || mode == GAME_MODE_ADVENTURE
}

fn freezing_step(ticks: i32, inside: bool, leather: bool) -> i32 {
    // С кожаной вещью лёд уходит так же, как вне снега: по 2 за такт (powder-snow-freezing).
    if !leather && inside { (ticks + 1).min(140) } else { (ticks - 2).max(0) }
}

fn powder_contact(world: &World, x: f64, y: f64, z: f64, height: f64) -> bool {
    for bx in ((x - 0.299).floor() as i32)..=((x + 0.299).floor() as i32) {
        for bz in ((z - 0.299).floor() as i32)..=((z + 0.299).floor() as i32) {
            for by in ((y + 0.001).floor() as i32)..=((y + height - 0.001).floor() as i32) {
                if blocks::block_at_state(world.get_block(bx, by, bz)) == Some("powder_snow") { return true; }
            }
        }
    }
    false
}

/// Что вокруг игрока: лава, вода в ногах и на уровне глаз, туловище в блоке.
struct Env {
    in_fire: f32,
    campfire: f32,
    powder_snow: bool,
    cactus: bool,
    /// Ящик игрока в кусте ягод не младше первой стадии.
    berries: bool,
    in_wall: bool,
    in_lava: bool,
    in_water: bool,
    /// Под дождём (World::in_rain): тушит, как вода.
    in_rain: bool,
    eyes_in_water: bool,
    /// Вики, Bubble Column: воздух восстанавливается как вне воды.
    in_bubbles: bool,
    soft: bool,
    landing: f32,
    below: i32,
}

/// Голова под водой: для добычи важны глаза, а не касание ногами воды.
pub(super) fn eyes_in_water(world: &World, player: &PlayerState) -> bool {
    let eye = player.y + if player.sneaking { 1.27 } else { 1.62 };
    let (x, y, z) = (player.x.floor() as i32, eye.floor() as i32, player.z.floor() as i32);
    fluids::fluid_at(world.get_block(x, y, z)).is_some_and(|(kind, level)| {
        kind == fluids::Kind::Water && eye < f64::from(y) + fluid_height(world, x, y, z, kind, level)
    })
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

/// Вики, Damage: «Suffocation»; tmp/bb/throw2-ender_pearl-up45-yaw0-standing.md:
/// 31–45: z=39,836, стена z=40, затем type=22, хотя центр головы в воздухе.
/// throw3-pearl-wall-edge.md:16–36: z=39,76 не душит, 39,77 душит.
/// Касание границы без пересечения не считается: верхний край открыт.
fn head_in_wall(world: &World, x: f64, eye: f64, z: f64) -> bool {
    for bx in (x - 0.24).floor() as i32..=(x + 0.24).ceil() as i32 - 1 {
        for bz in (z - 0.24).floor() as i32..=(z + 0.24).ceil() as i32 - 1 {
            let head = world.get_block(bx, eye.floor() as i32, bz);
            if blocks::full_cube(head) && fluids::fluid_at(head).is_none() && blocks::block_at_state(head) != Some("powder_snow") {
                return true;
            }
        }
    }
    false
}

fn look_around(world: &World, x: f64, y: f64, z: f64, sneaking: bool, spin: bool) -> Env {
    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    let mut env = Env { in_fire: 0.0, campfire: 0.0, powder_snow: false, cactus: false, berries: false, in_wall: false, in_lava: false, in_water: false, in_rain: false, eyes_in_water: false, in_bubbles: false, soft: false, landing: 1.0, below: 0 };

    // Ящик игрока: от ног до макушки; жидкость считается, если её
    // поверхность выше ног.
    // В позе вращения «тягуна» ящик 0,6 × 0,6, глаза на 0,4 (вики, Riptide;
    // чёрный ящик, trident-riptide-hit: воздух 1=299… убывает, пока глаза в воде).
    let top = y + if spin { 0.6 } else if sneaking { 1.5 } else { 1.8 };

    env.in_rain = world.in_rain(x, y, z, top - y);

    for by in (y.floor() as i32)..=(top.floor() as i32) {
        let state = world.get_block(bx, by, bz);

        if blocks::block_at_state(state) == Some("bubble_column") { env.in_bubbles = true; }

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
    env.campfire = crate::campfire::damage(feet);

    // Куст ягод: в клетке ног, возраст от 1 (вики, Sweet Berry Bush:
    // «except when it is in its first stage»).
    env.berries = blocks::orientation_of(feet).is_some_and(|block| block.name == "sweet_berry_bush" && block.value(feet, "age") != Some("0"));

    // Огонь в клетке ног или головы; кактус в клетках вокруг ног (ящик
    // шириной 0,6 задевает соседние клетки у края); туловище в целом блоке.
    for by in (y.floor() as i32)..=((y + 1.0).floor() as i32) {
        let fire = match blocks::block_at_state(world.get_block(bx, by, bz)) {
            Some("soul_fire") => 2.0_f32,
            Some("fire") => 1.0,
            _ => 0.0,
        };
        env.in_fire = env.in_fire.max(fire);

        for (cx, cz) in [(x - 0.31, z), (x + 0.31, z), (x, z - 0.31), (x, z + 0.31)] {
            let name = blocks::block_at_state(world.get_block(cx.floor() as i32, by, cz.floor() as i32));

            if name == Some("cactus") {
                env.cactus = true;
            }
        }
    }

    // Снег задевает весь ящик; через его край тоже начинают замерзать.
    env.powder_snow = powder_contact(world, x, y, z, top - y);
    env.in_wall = head_in_wall(world, x, y + if spin { 0.4 } else if sneaking { 1.27 } else { 1.62 }, z);

    env.soft = env.in_water || env.in_lava || blocks::block_at_state(feet).is_some_and(soft_block);

    // Глаза: 1,62 у стоящего, 1,27 у присевшего.
    let eye = y + if spin { 0.4 } else if sneaking { 1.27 } else { 1.62 };
    let (ex, ey, ez) = (bx, eye.floor() as i32, bz);

    if let Some((fluids::Kind::Water, level)) = fluids::fluid_at(world.get_block(ex, ey, ez)) {
        env.eyes_in_water = eye < f64::from(ey) + fluid_height(world, ex, ey, ez, fluids::Kind::Water, level);
    }

    let below = world.get_block(bx, (y - 0.2).floor() as i32, bz);

    env.below = below;
    env.landing = blocks::block_at_state(below).map(landing_multiplier).unwrap_or(1.0);
    // Слизь гасит урон и с Shift (чёрный ящик, slime-sneak-landing).


    env
}

/// Вики, Frost Walker: «eliminates all damage from magma blocks and
/// campfires». Лава от этого зачарования не защищена. Общая проверка для
/// обоих представлений игрока, включая огнестойкость.
fn protected_from_hot_blocks(player: &PlayerState) -> bool {
    player.vit.has_effect(survival::EffectKind::FireResistance)
        || crate::frost::protects(&player.inventory, Source::HotFloor)
}

/// Вики, Magma Block: урон при ходьбе, кроме крадущихся и носящих Ледоход.
/// [проверить] Точная фаза касания края блока и верхних покрытий.
fn magma_contact(player: &PlayerState, env: &Env) -> bool {
    player.on_ground && !player.sneaking && player.ride.vehicle.is_none()
        && blocks::block_at_state(env.below) == Some("magma_block")
        && !protected_from_hot_blocks(player)
}

/// Смена режима общая для своей команды и поручения другого игрока.
pub(super) fn change_game_mode(player: &mut PlayerState, mode: i32) {
    if player.game_mode == mode { return; }
    if player.game_mode != mode {
        // fall-mode-teleport-reset, 26.1.2: creative → survival сохраняет
        // пролёт целиком (12 блоков дают урон 9, а не 5).
        if player.game_mode == GAME_MODE_CREATIVE && mortal(mode)
            && !player.abilities.flying && !player.on_ground && player.net.last_delta.1 < 0.0
        {
            player.net.mode_fall_velocity = Some(medium_step(player.net.last_delta.1, player.net.in_lava, player.net.in_water));
        }
        player.prev_mode = player.game_mode;
        player.game_mode = mode;
    }
    player.abilities.apply_mode(mode);
    player.flight_owners[0] = None;
    player.flight_owners[1] = None;
    player.attrs.set_creative_ranges(mode == GAME_MODE_CREATIVE);
}

/// fall-mode-teleport-reset: Abilities, Game Event, снова Abilities; дальность
/// взаимодействия повторяется через два такта после команды.
pub(super) async fn game_mode_order(stream: &mut Outgoing<'_>, player: &mut PlayerState, mode: i32, now: u64) -> io::Result<()> {
    change_game_mode(player, mode);
    send_player_abilities(stream, player).await?;
    send_game_event_game_mode(stream, mode).await?;
    send_player_abilities(stream, player).await?;
    for attribute in [crate::attributes::BLOCK_INTERACTION_RANGE, crate::attributes::ENTITY_INTERACTION_RANGE] {
        player.attrs_touched.push((attribute, now + 2));
    }
    Ok(())
}

/// Пакет движения от клиента: прыжок, бег по земле и приземление.
pub(super) fn moved(player: &mut PlayerState, shared: &Shared, from: (f64, f64, f64)) {
    // Едущий не бежит ногами и не получает вторично урон падения транспорта.
    if player.ride.vehicle.is_some() { player.vit.fall = 0.0; return; }
    let (dx, dy, dz) = (player.x - from.0, player.y - from.1, player.z - from.2);
    let on_ground = player.on_ground;

    // После переноса первый пакет места — не шаг: прежнее место сервер мог
    // успеть перезаписать пакетом со старого места.
    // Шаг длиннее десяти блоков за такт не движение, а перенос (падающий не
    // разгоняется быстрее четырёх блоков за такт).
    let jump = dx * dx + dy * dy + dz * dz > 100.0;

    if std::mem::take(&mut player.net.teleported) || jump {
        player.net.pin = None;
        player.net.mode_fall_velocity = None;
        player.net.prev_ground = on_ground;
        player.net.last_delta = (0.0, 0.0, 0.0);
        player.net.sweep_motion = Default::default();
        // fall-mode-teleport-reset: /tp не прибавляет скачок координат,
        // но сохраняет уже накопленный пролёт (после /tp вниз урон 1).

        return;
    }

    let jumped = player.net.prev_ground && !on_ground && dy > 0.0;

    if !player.vit.dead && player.sleeping.is_none() && player.ride.vehicle.is_none() {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let env = look_around(&world, player.x, player.y, player.z, player.sneaking, player.net.spin_pose);
        let feet = blocks::block_at_state(world.get_block(player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32));
        let mode = if player.glide.active { Some("aviate_one_cm") }
            else if env.eyes_in_water { Some(if player.vit.sprinting { "swim_one_cm" } else { "walk_under_water_one_cm" }) }
            else if env.in_water { Some("walk_on_water_one_cm") }
            else if feet.is_some_and(|name| matches!(name, "ladder" | "vine" | "scaffolding" | "weeping_vines" | "weeping_vines_plant" | "twisting_vines" | "twisting_vines_plant" | "cave_vines" | "cave_vines_plant")) { Some("climb_one_cm") }
            else { None };
        if let Some(mode) = mode {
            let metres = match mode { "climb_one_cm" => dy.max(0.0), "walk_on_water_one_cm" => dx.hypot(dz), _ => dx.hypot(dy).hypot(dz) };
            player.progress.custom(mode, (metres * 100.0).round() as i32);
        } else {
            player.progress.moved(dx, dy, dz, on_ground, player.vit.sprinting, player.sneaking, jumped);
        }
    }

    if mortal(player.game_mode) && !player.vit.dead {
        if jumped {
            player.vit.jumped();
        }

        if on_ground && player.vit.sprinting {
            player.vit.ran(dx.hypot(dz));
        }

        let env = {
            let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            look_around(&world, player.x, player.y, player.z, player.sneaking, player.net.spin_pose && player.net.spin_since.is_some_and(|since| since >= 3))
        };

        // Высота падения вместе с этим шагом — для грядки.
        let fallen = (player.vit.fall + (-dy).max(0.0)) as f32;
        let smashed = player.vit.smash_landing;
        let damage = if player.fall_damage_allowed() { player.vit.land_java(dy, on_ground, env.soft, env.landing) } else { player.vit.fall = 0.0; None };
        if env.soft { player.net.mode_fall_velocity = None; }
        if on_ground && fallen >= 2.0 {
            player.progress.custom("fall_one_cm", (fallen * 100.0).round() as i32);
        }
        if on_ground || env.soft { super::progression::landed(player, fallen); }

        if on_ground && player.game_mode != GAME_MODE_SPECTATOR {
            let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
            let egg = (player.x.floor() as i32,player.y.floor() as i32,player.z.floor() as i32);
            crate::mob::water::eggs::trample(&mut world,egg,fallen > 0.0);
        }

        // Приземлился на грядку — может её вытоптать (farming.rs).
        if on_ground && !env.soft && fallen > 0.5 && blocks::block_at_state(env.below) == Some("farmland") {
            let below = (player.x.floor() as i32, (player.y - 0.2).floor() as i32, player.z.floor() as i32);
            let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            if crate::farming::trample(&mut world, below, fallen) {
                redstone::settle(&mut world);
            }
        }

        // Частицы приземления не зависят от урона (сено, мёд, стойкость их не
        // отменяют); число — по высоте до последнего шага (survival::
        // landing_particles).
        let count = survival::landing_particles(player.vit.landed_from as f32);

        if on_ground && player.vit.landed_from > 0.0 {
            log_debug!("Play: {} встал на землю, падение до шага {:.2}, частиц {}", player.name, player.vit.landed_from, count);
        }

        // Встал на землю после сокрушающего удара: ещё частицы блока с
        // разбросом 0,3 — раньше обычных (чёрный ящик, pvp-mace-wind-burst:
        // `block 200 (0.3…)` и `block 90 (0…)` в одном такте).
        if on_ground && smashed && !env.soft {
            landing(player, shared, env.below, smash_landing_particles(player.vit.landed_from), true);
        }

        if count > 0 {
            landing(player, shared, env.below, count, false);
        }

        if let Some(damage) = damage {
            player.net.landing_velocity = player.net.mode_fall_velocity;
            log_debug!("Play: {} приземлился с уроном {} на {:.2} {:.2} {:.2} (шаг {:.3})", player.name, damage, player.x, player.y, player.z, dy);

            let announced = announce_fall(player, shared, damage, env.below);
            player.net.hits.push(Pending { amount: damage, source: Source::Fall, direct: 0, hurt_yaw: None, killer: None, push: None, announced });
        }
    } else if player.game_mode == GAME_MODE_CREATIVE && !player.vit.dead {
        // Творческий: урона от падения нет, но высота падения копится — от неё
        // зависит крит (вики, Critical hit: в творческом криты есть); в полёте
        // высота не копится.
        if player.abilities.flying {
            player.vit.fall = 0.0;
        } else {
            let _ = player.vit.land_java(dy, on_ground, player.net.in_water, 1.0);
        }
    }

    if on_ground {
        player.net.pin = None;
        player.net.mode_fall_velocity = None;
    }

    player.net.prev_ground = on_ground;
    player.net.last_delta = (dx, dy, dz);
    player.net.sweep_motion.moved(*shared.ticks.borrow(), dx, dz);
    super::movement_enchantments::update(shared, player);
}

/// pvp-mace-landing-count: наблюдателю в такт приземления идут small_fall,
/// stone.fall, Damage Event, hurt и здоровье. Самому звуки не посылаются.
fn announce_fall(player: &mut PlayerState, shared: &Shared, amount: f32, below: i32) -> bool {
    let points: i32 = crate::combat::ARMOR_SLOTS.iter().filter_map(|(slot, _)| player.inventory.slot(*slot))
        .map(|stack| crate::combat::protection_points(&stack.extras(), Source::Fall)).sum();
    let amount_after_armor = crate::combat::mitigated_damage(amount, 0.0, 0.0, points, Source::Fall, 1.0);
    let mut probe = player.vit.clone();
    let Some(hit) = probe.hurt(amount_after_armor, Source::Fall, !mortal(player.game_mode)) else { return false; };
    // Смерть/тотем имеют свою публикацию в hit_by; здесь только обычный ушиб.
    if hit.died { return false; }
    let at = (player.x, player.y, player.z);
    let mut world = shared.world_in(player.dimension).lock().expect("мир");
    let sound = if amount > 4.0 { "minecraft:entity.player.big_fall" } else { "minecraft:entity.player.small_fall" };
    world.note_sound_player_except(at, sound, 1.0, 1.0, player.entity_id);
    // Запись сделана на камне; остальные группы звуков блока [проверить].
    if blocks::block_at_state(below) == Some("stone") {
        world.note_sound_player_except(at, "minecraft:block.stone.fall", 0.5, 0.75, player.entity_id);
    }
    world.note_command_damage(player.entity_id, Source::Fall.type_id(), 0, 0, None);
    world.note_sound_player_except(at, "minecraft:entity.player.hurt", 1.0, 1.0, player.entity_id);
    player.net.announced_health = Some(probe.health);
    shared.players.lock().expect("игроки").set_health(&player.uuid, probe.health);
    true
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
            1 if !player.vit.has_effect(survival::EffectKind::Blindness) => player.vit.sprinting = true,
            2 => player.vit.sprinting = false,
            _ => {}
        }
    }
}

pub(super) fn start_eating_at(player: &mut PlayerState, hand: i32, now: u64) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    if let Some(stack) = player.inventory.slot(slot)
        && let Some(name) = blocks::item_name(stack.item)
        && player.item_cooldowns.get(name).is_some_and(|until| now < *until) { return false; }
    start_eating(player, hand)
}

/// Правая кнопка с едой в руке: начать есть. Возвращает, началась ли еда.
pub(super) fn start_eating(player: &mut PlayerState, hand: i32) -> bool {
    if player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead || player.net.eating.is_some() || player.net.using.is_some() || !(0..=1).contains(&hand) {
        return false;
    }

    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else {
        return false;
    };
    let name = blocks::item_name(stack.item).map_or("", |name| name.strip_prefix("minecraft:").unwrap_or(name));

    // Молоко и зелья пьются в любой сытости, 32 такта (вики, Milk Bucket, Potion).
    let drink = match name {
        "milk_bucket" => Some((Drink::Milk, Vec::new())),
        "honey_bottle" => Some((Drink::Honey, Vec::new())),
        "potion" => Some((Drink::Potion, stack.extras().potion_effects())),
        // Вики, Ominous Bottle: «Bad Omen … 100 minutes», уровень — из бутылки.
        "ominous_bottle" => Some((Drink::Ominous, vec![survival::Effect {
            kind: survival::EffectKind::BadOmen,
            amplifier: i32::from(stack.extras().ominous.unwrap_or(0)),
            ticks_left: 120_000,
            ambient: false,
            visible: true, show_icon: true,
        }])),
        _ => None,
    };

    if let Some((drink, effects)) = drink {
        player.net.last_hand = hand;
        player.net.eating = Some(Eating { slot, stack, ticks_left: if drink == Drink::Honey { 40 } else { 32 }, nutrition: 0, saturation: 0.0, effects, drink: Some(drink) });

        return true;
    }

    let Some(food) = food_of(name) else {
        return false;
    };

    // Сытому обычную еду не дают (вики, Food: «unless hungry»).
    if !food.always && !player.vit.hungry() && player.game_mode != GAME_MODE_CREATIVE {
        return false;
    }

    player.net.last_hand = hand;
    player.net.eating = Some(Eating { slot, stack, ticks_left: food.ticks, nutrition: food.nutrition, saturation: food.saturation, effects: survival::food_effects(name), drink: None });

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
        (None, true) => 1 | hand,
        (None, false) => hand,
    }
}

/// Правая кнопка со щитом или луком в руке `hand`: щит поднят, лук
/// натягивается. Возвращает, началось ли использование.
pub(super) fn start_using(shared: &Shared, player: &mut PlayerState, hand: i32, now: u64) -> bool {
    // Творческий тоже натягивает лук (без стрел) и поднимает щит; не может
    // только наблюдатель (вики, Bow: «Players in Creative mode do not need
    // arrows»).
    if player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead || player.net.using.is_some() || player.net.eating.is_some() {
        return false;
    }

    let stack = if hand == 1 { player.inventory.slot(inventory::OFFHAND) } else { player.inventory.held() };
    let Some(name) = stack.and_then(|stack| crate::combat::item_name(stack.item)) else {
        return false;
    };

    let kind = match name {
        "shield" if now >= player.net.shield_disabled_until => UseKind::Shield,
        "bow" => UseKind::Bow,
        "spyglass" => UseKind::Spyglass,
        // Незаряженный арбалет заряжается, если есть чем (вики, Crossbow);
        // заряженный стреляет (`bow::crossbow_fire`).
        "crossbow" => {
            let data = stack.map(|stack| stack.extras()).unwrap_or_default();

            if data.charged > 0 || (player.game_mode != GAME_MODE_CREATIVE && super::bow::crossbow_ammo(player, hand).is_none()) {
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

    if kind == UseKind::Spyglass {
        shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x,player.y,player.z), "minecraft:item.spyglass.use", 1.0, 1.0);
        if let Some(stack) = stack { player.progress.used(stack.item); }
    }
    player.net.using = Some(Using { hand, since: now, kind });
    player.net.last_hand = hand;

    true
}

/// Правая кнопка с оком Края: «использует предмет» до отпускания кнопки
/// (eye-of-ender-none: Metadata 8=1 и без броска).
pub(super) fn start_using_eye(player: &mut PlayerState, hand: i32, now: u64) {
    let stack = if hand == 1 { player.inventory.slot(inventory::OFFHAND) } else { player.inventory.held() };

    if stack.and_then(|stack| crate::combat::item_name(stack.item)) != Some("ender_eye") || player.vit.dead || player.net.using.is_some() || player.game_mode == GAME_MODE_SPECTATOR {
        return;
    }

    player.net.using = Some(Using { hand, since: now, kind: UseKind::EnderEye });
    player.net.last_hand = hand;
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
        UseKind::Spyglass => shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x,player.y,player.z), "minecraft:item.spyglass.stop_using", 1.0, 1.0),
        UseKind::Crossbow { .. } | UseKind::Shield | UseKind::EnderEye | UseKind::Brush | UseKind::Bundle => {}
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
    let _publication = shared.publication.lock().expect("публикация арбалета");
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
        super::publish_player_inner(shared, player);
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

pub(super) async fn send_entity_event(stream: &mut Outgoing<'_>, entity_id: i32, status: i8) -> io::Result<()> {
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
    } else if matches!(source, Source::MobAttack | Source::Sting | Source::Arrow | Source::Trident | Source::PlayerExplosion | Source::Thorns | Source::IndirectMagic | Source::SonicBoom) && direct != 0 {
        direct + 1
    } else {
        0
    };

    (source.type_id(), culprit, if direct == 0 { 0 } else { direct + 1 })
}

pub(super) fn damage_packet(entity_id: i32, source: Source, direct: i32, cause: i32, at: Option<(f64, f64, f64)>) -> Vec<u8> {
    let (type_id, culprit, direct) = damage_fields(source, direct, cause);
    let mut body = encode_varint(DAMAGE_EVENT);
    for value in [entity_id, type_id, culprit, direct] { body.extend_from_slice(&encode_varint(value)); }
    body.push(u8::from(at.is_some()));
    if let Some((x, y, z)) = at { push_f64(&mut body, x); push_f64(&mut body, y); push_f64(&mut body, z); }
    body
}

/// Звук в точке: раздел, громкость, высота.
pub(super) async fn send_sound_at(
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

/// pvp-mace-landing-motion, 26.1.2: floor(50h), предел 200.
/// h — пролёт после смаша ДО касания; 2,69456 → 134, 3,21151 → 160.
/// Шаг в полёте, пришедший после атаки до Velocity, тоже входит в h.
fn smash_landing_particles(before: f64) -> i32 {
    ((before * 50.0) as i32).clamp(0, 200)
}

fn landing(player: &mut PlayerState, shared: &Shared, block: i32, count: i32, wide: bool) {
    player.net.landings.push((block, count, wide));
    // pvp-mace / pvp-mace-wind-burst: наблюдатель видит приземление в тот
    // же такт. Событие нужно до publish_events, а не из позднего vitals::tick.
    let spread = if wide { 0.3 } else { 0.0 };
    shared.world_in(player.dimension).lock().expect("мир").note_particle_except(
        player.entity_id, 1, Some(block), (player.x, player.y, player.z), (spread, spread, spread), 0.15, count);
}

/// Частицы блока под ногами при приземлении.
async fn send_landing_particles(stream: &mut Outgoing<'_>, at: (f64, f64, f64), block: i32, count: i32, wide: bool) -> io::Result<()> {
    const PARTICLE_BLOCK: i32 = 1;

    let mut body = encode_varint(LEVEL_PARTICLES);

    body.push(0); // не издалека
    body.push(0); // не всегда
    for v in [at.0, at.1, at.2] {
        body.extend_from_slice(&v.to_be_bytes());
    }
    // Смещение 0, скорость 0,15 (чёрный ящик: probe-land-player).
    let spread = if wide { 0.3f32 } else { 0.0 };

    for v in [spread, spread, spread, 0.15] {
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
pub(super) async fn send_experience(stream: &mut Outgoing<'_>, vit: &Vitals) -> io::Result<()> {
    let (level, bar) = survival::level_of(vit.xp);
    let mut body = encode_varint(SET_EXPERIENCE);

    push_f32(&mut body, bar);
    body.extend_from_slice(&encode_varint(level));
    body.extend_from_slice(&encode_varint(vit.xp_total));

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
        // Знамение набега — своя частица raid_omen без цвета
        // (minecraft-data 26.1, particles.json: 113; raid-start-wave1).
        if color == survival::RAID_OMEN_PARTICLE {
            body.extend_from_slice(&encode_varint(113));
            continue;
        }

        body.extend_from_slice(&encode_varint(21));
        body.extend_from_slice(&color.to_be_bytes());
    }
}

/// Сошедший с транспорта нажатием «присесть» получает флаг «крадётся» раньше
/// списка седоков и сверки места, а в следующий такт флаг снимается, пока
/// кнопка зажата (чёрный ящик, horse-jump: Metadata 0=2 перед Passengers,
/// 0=0 на такт позже; boat-ride — так же).
pub(super) async fn announce_dismount_sneak(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    if !player.sneaking || player.net.meta.flags & 0x02 != 0 {
        return Ok(());
    }

    let before = player.net.meta;
    let mut now = before;

    now.flags |= 0x02;
    send_meta(stream, player.entity_id, now, before).await?;
    player.net.meta = now;
    player.net.sneak_riding = true;
    player.net.hide_sneak_from = Some(*shared.ticks.borrow() + 1);
    Ok(())
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

    if now.frozen != before.frozen {
        body.push(7); // Entity format: ticks frozen, VarInt.
        body.extend_from_slice(&encode_varint(PROPERTY_VARINT));
        body.extend_from_slice(&encode_varint(now.frozen));
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

    if now.stingers != before.stingers {
        body.push(STINGER_COUNT_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_VARINT));
        body.extend_from_slice(&encode_varint(now.stingers));
    }

    // Место кровати: необязательное место блока (вид 11) — есть ли и само место.
    if now.bed != before.bed {
        body.push(BED_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_OPTIONAL_BLOCK_POS));

        match now.bed {
            Some((x, y, z)) => {
                body.push(1);
                push_packed_position(&mut body, x, y, z);
            }
            None => body.push(0),
        }
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

    if player.net.frozen > 0 && mortal(player.game_mode) {
        // Как у оригинала, в одинарной точности (powder-snow-freezing: -0.0003571428533177823).
        let amount = f64::from(-0.05f32 * (player.net.frozen as f32 / 140.0f32));
        player.attrs.set_modifier(crate::attributes::MOVEMENT_SPEED, Modifier::new("minecraft:powder_snow", amount, Operation::AddValue));
    } else {
        player.attrs.remove_modifier(crate::attributes::MOVEMENT_SPEED, "minecraft:powder_snow");
    }

    // itemaudit4-knockback-phase, 26.1.2: удары +15/56/98/139,
    // Attributes +17/57/99/141 (задержки 2/1/1/2). Период — два такта,
    // но начало фазы по этим записям не установить: у экипировки рассылки
    // чётные по возрасту мира, у смены режима — нечётные. [проверить]
    // itemaudit4-attribute-phase-equipment; прежнюю задержку пока сохраняем.
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

    // Броню, стойкость и дальность видят и остальные игроки; стойкость первая.
    // Дальность подтверждена itemaudit4-attribute-phase-equipment, 26.1.2.
    let mut armor: Vec<_> = changed.iter().filter(|a| matches!(a.id,
        crate::attributes::ARMOR | crate::attributes::ARMOR_TOUGHNESS
        | crate::attributes::BLOCK_INTERACTION_RANGE | crate::attributes::ENTITY_INTERACTION_RANGE
    )).cloned().collect();

    armor.sort_by_key(|a| a.id != crate::attributes::ARMOR_TOUGHNESS);
    player.net.armor_out = armor;
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

    // Модификатор только сняли (меч убран из руки) — уходит на такт раньше
    // (чёрный ящик, craft-repair-tools: оригинал +1, надевание — +2).
    let applied: Vec<i32> = player.attrs_applied.iter().map(|(attribute, _)| *attribute).collect();

    //777, itemaudit4-knockback-phase: после ударов15/56/98/139 рассылка
    // идёт16/58/100/140 — один общий двухтактовый период, не постоянные+2.
    let tracking_phase = player.attrs_tracking_phase;
    // Sword-sweep-secondary-motion: износ20/21 объединяется в Attributes23;
    // второй износ не должен попадать в ещё ожидающий пакет первого.
    let consecutive = player.attrs_equipment_tick.is_some_and(|before| now.saturating_sub(before) == 1);
    player.attrs_equipment_tick = Some(now);
    player.attrs_touched.extend(touched.into_iter().map(|attribute| {
        let delay = tracking_phase.map_or_else(|| if applied.contains(&attribute) { 2 } else { 1 },
            |phase| if !consecutive && (now + 1) % 2 == phase { 1 } else { 2 });
        (attribute, now + delay)
    }));
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
/// `after_event` — пакет сразу за событием прав: звук якоря, отдавшего заряд
/// (чёрный ящик, anchor-nether).
pub(super) async fn send_respawn_extras(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, after_event: Option<Vec<u8>>) -> io::Result<()> {
    let level = shared.ops.lock().expect("права захвачены другим потоком").level_of(&player.uuid);

    send_entity_event(stream, player.entity_id, (24 + level.clamp(0, 4)) as i8).await?;

    if let Some(body) = after_event {
        write_body(stream, body).await?;
    }

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
    player.abilities = playerdata::Abilities::for_mode(player.game_mode);
    player.flight_owners[0] = None;
    player.flight_owners[1] = None;
    player.net.reset();
    // Официальный340 после смерти создаёт новое окно инвентаря:
    // незавершённый ACK прежней жизни больше не блокирует щелчки.
    player.legacy_transaction = None;
    player.foot_enchants = Default::default();

    // Клиент после возрождения знает атрибуты по умолчанию.
    player.attrs = crate::attributes::Attributes::player();
    player.attrs_applied.clear();
    player.attrs_sent = crate::attributes::Sent::of(&player.attrs);
    player.attrs.set_creative_ranges(player.game_mode == GAME_MODE_CREATIVE);
    player.equip_seen.clear();
}

/// Смерть: сообщение, событие и выпавшие вещи.
async fn die(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, source: Source, killer: Option<Killer<'_>>) -> io::Result<()> {
    super::vehicle::dismount(shared, player);
    effect_reactions(shared, player, true);
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

    // Статистика: смерть, и кому засчитано убийство игрока.
    player.progress.died(killer.as_ref().map(|_| "player").or_else(|| mob.strip_prefix("entity.minecraft.")));

    if let Some(killer) = &killer {
        shared.players.lock().expect("список игроков захвачен другим потоком").order(&killer.uuid, crate::players::Order::KilledPlayer);
    }

    // Табло: смерти и убийства (вики, Scoreboard#Criteria: deathCount,
    // playerKillCount, totalKillCount, teamkill.<цвет>, killedByTeam.<цвет>).
    {
        let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");
        board.bump_criteria(&player.name, "deathCount", 1);

        if let Some(killer) = killer {
            board.bump_criteria(killer.name, "playerKillCount", 1);
            board.bump_criteria(killer.name, "totalKillCount", 1);

            let victim_color = board.team_of(&player.name).map(|team| team.color);
            let killer_color = board.team_of(killer.name).map(|team| team.color);

            if let Some(color) = victim_color.filter(|color| (0..16).contains(color)) {
                board.bump_criteria(killer.name, &format!("teamkill.{}", crate::scoreboard::COLORS[color as usize]), 1);
            }
            if let Some(color) = killer_color.filter(|color| (0..16).contains(color)) {
                board.bump_criteria(&player.name, &format!("killedByTeam.{}", crate::scoreboard::COLORS[color as usize]), 1);
            }
        }
    }

    log_info!("{}", death_line(source, &player.name, killer));

    player.net.died_at = Some((player.dimension, block_of(player.x, player.y, player.z)));
    player.net.eating = None;
    player.net.using = None;
    player.net.hits.clear();
    player.net.landings.clear();

    // Сетка — входы, результат лишь предпросмотр, его ронять нельзя.
    super::close_own_window(shared, player);
    for stack in player.inventory.close_window().thrown { super::throw_out(shared, player, stack); }

    // Вещи падают, если не включено `keepInventory` (вики, Keep inventory).
    {
        let keep = shared.keep_inventory.load(std::sync::atomic::Ordering::Relaxed);
        for stack in crate::combat::death_drops(&mut player.inventory, keep) {
            let spread = |shared: &Shared| {
                let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

                (f64::from(world.random_unit()) - 0.5) * 0.4
            };
            let velocity = (spread(shared), 0.2, spread(shared));

            drop_into_world(shared, player.dimension, stack, (player.x, player.y + 0.5, player.z), velocity, items::THROWN_DELAY);
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
    enchantments: crate::world::ProjectileEnchantments,
) -> io::Result<()> {
    player.net.killer = killer;
    player.net.mob_hit_landed = false;

    // Вики, Shield: передний удар моба поглощается; укус, остановленный
    // щитом, не накладывает яд. Износ — тот же, что в бою игроков.
    let now = *shared.ticks.borrow();
    if !enchantments.ignores_shield && (matches!(source, Source::MobAttack | Source::Sting) || source.is_projectile()) && amount > 0.0 && !player.vit.dead
        && matches!(player.game_mode, GAME_MODE_SURVIVAL | GAME_MODE_ADVENTURE)
        && let Some(hand) = shield_up(player, now)
        && crate::combat::shield_covers(player.yaw, (player.x, player.z), (at.0, at.2))
    {
        if killer == "entity.minecraft.warden" {
            player.net.shield_disabled_until = now + 100;
            player.net.using = None;
            send_cooldown(stream, "minecraft:shield", 100).await?;
        }
        let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
        let scaled = survival::scaled(amount, shared.difficulty.load(std::sync::atomic::Ordering::Relaxed));
        player.progress.custom("damage_blocked_by_shield", (scaled * 10.0).round() as i32);
        if source.is_projectile() { player.progress.trigger("story/deflect_arrow"); }
        if let Some(shield) = player.inventory.slot(slot) {
            super::pvp::wear_slot(shared, player, slot, shield, crate::combat::shield_wear(scaled), false);
        }
        // Разоритель толкает и сквозь щит (mob/illager.rs, RAVAGER_SHOVE):
        // толчок длиннее единицы — без оглушения, сильный.
        if killer == "entity.minecraft.ravager" && let Some((ux, uz)) = push {
            let strong = ux.hypot(uz) > 1.5;

            player.net.velocity = Some((ux * 0.218, if strong { crate::mob::illager::RAVAGER_SHOVE_UP } else { 0.275 }, uz * 0.218));
            player.net.velocity_due = false;
            // Скорость уходит следующим тактом, как после обычного удара
            // (ravager-roar: self vel на такт позже ravager.attack).
            player.net.hold_meta = true;
        }

        // Звук щита — в такте удара: себе сразу, прочим — журналом (чёрный
        // ящик, ravager-roar: item.shield.block в такте ravager.attack).
        let pitch = {
            let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
            let pitch = 0.8 + world.random_unit() * 0.4;

            world.note_sound_player_except((player.x, player.y, player.z), "minecraft:item.shield.block", 1.0, pitch, player.entity_id);
            pitch
        };

        return send_sound_at(stream, "minecraft:item.shield.block", SOUND_PLAYERS, (player.x, player.y, player.z), 1.0, pitch).await;
    }

    // Жало пчелы — свой вид урона (41), остальное как удар моба.
    let source = if source == Source::MobAttack && killer == "entity.minecraft.bee" { Source::Sting } else { source };

    // Направление на виновника для «Hurt Animation» и звук стрелы — раньше
    // пакетов урона: они уходят изнутри `hit` (чёрный ящик, skeleton-arrow).
    player.net.hurt_from = Some(at);

    let done = hit(stream, shared, player, amount, source, attacker).await?;

    player.net.hurt_from = None;
    player.net.hold_meta = done.is_some();
    player.net.mob_hit_landed = done.is_some();

    // Слизень звучит только после принятого контактного урона; иммунитет
    // игрока гасит повторные касания. Чёрный ящик, slime-hop: звук уходит в том
    // же такте, что и урон, из точки слизня.
    if done.is_some() && killer == "entity.minecraft.slime" {
        let pitch = {
            let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
            items.mobs_mut().iter_mut().find(|mob| mob.entity_id == attacker).map(|mob| mob.voice_pitch())
        };

        if let Some(pitch) = pitch {
            send_sound_at(stream, "minecraft:entity.slime.attack", 5, at, 1.0, pitch).await?;
        }
    }

    // Вики, Pufferfish: «When a pufferfish attacks something». [проверить] Порядок пакетов.
    if done.is_some() && killer == "entity.minecraft.pufferfish" {
        // Silent:1b глушит и укол (чёрный ящик pufferfish-sting).
        let silent={
            let items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
            items.mobs().iter().find(|mob| mob.entity_id == attacker).is_some_and(|mob| mob.silent)
        };
        if !silent { send_sound_at(stream,"minecraft:entity.puffer_fish.sting",6,at,1.0,1.0).await?; }
    }

    // Вики, Cave Spider: яд только после состоявшегося укуса, не от
    // отклонённого урона (творческий, неуязвимость). Срок — по сложности.
    if done.is_some() && source == Source::MobAttack && killer == "entity.minecraft.cave_spider"
        && let Some(effect) = crate::mob::arthropod::poison(shared.difficulty.load(std::sync::atomic::Ordering::Relaxed))
    {
        give_effect(stream, player, effect).await?;
    }

    if done.is_some() && source == Source::Sting {
        // Жало остаётся в теле: 13=1; убывает по одному, как стрелы [проверить: срок].
        player.net.stingers += 1;
        player.net.stinger_timer = 20 * (30 - player.net.stingers);
        {
            let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
            if let Some(bee) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == attacker) {
                crate::mob::wildlife::sting(bee);
            }
        }
        if let Some(effect) = crate::mob::wildlife::poison(shared.difficulty.load(std::sync::atomic::Ordering::Relaxed)) {
            give_effect(stream, player, effect).await?;
        }
    }

    if done.is_some() {
        let mut items=shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
        let excluded=matches!(killer,"entity.minecraft.iron_golem"|"entity.minecraft.wolf"|"entity.minecraft.bee"|"entity.minecraft.rabbit"|"entity.minecraft.slime"|"entity.minecraft.magma_cube"|"entity.minecraft.hoglin");
        crate::mob::wildlife::defend(items.mobs_mut(),player.entity_id,crate::mob::tame::Target::Mob(attacker),excluded);
    }

    if done.is_some() && matches!(source, Source::MobAttack | Source::Sting) {
        let scaled = survival::scaled(amount, shared.difficulty.load(std::sync::atomic::Ordering::Relaxed));
        super::pvp::wear_armor(shared, player, scaled);
    }

    // Стрела застряла в теле: счёт в свойстве 12, убывает через 20·(30 − число)
    // тактов (вики, Arrow: «stuck in the body») [проверить].
    if done.is_some() && source == Source::Arrow {
        player.net.arrows += 1;
        player.net.arrow_timer = 20 * (30 - player.net.arrows);
    }

    // Слизень отбрасывает полным ударом 0,4 без трения такта: у оригинала
    // (0,-0.03; 0,361; -0,399) при стоящем игроке (чёрный ящик, slime-hop).
    if done.is_some() && source == Source::SonicBoom {
        // Сценарий warden-sonic-boom: оригинал шлёт (0; 0,277; −1,583) стоящему
        // игроку. Это обычный толчок удара 0,4 от хранителя (по вертикали 0,3606)
        // плюс толчок звука, уже после трения такта: 0,546 по земле, тяжесть и
        // 0,98 по вертикали.
        let delta = (player.x - at.0, player.y + 1.62 - at.1, player.z - at.2);
        let sonic = crate::mob::warden::sonic_velocity(delta, player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE));
        let flat = (player.x - at.0).hypot(player.z - at.2).max(1.0e-9);
        let (ux, uz) = ((player.x - at.0) / flat, (player.z - at.2) / flat);
        player.net.velocity = Some(((ux * 0.4 + sonic.0) * 0.546, (0.3606 + sonic.1 - 0.08) * 0.98, (uz * 0.4 + sonic.2) * 0.546));
        player.net.velocity_due = false;
    } else if let (Some(_), Some(dir), true) = (done, push, source == Source::Thorns) {
        // enchant-mob-thorns-wear: отражение приходит без трения такта,
        // как PvP Thorns: в записи (0;0.361;-0.4), а не обычный mob_hit .218.
        // До округления: -0.0784 / 2 + 0.4 = 0.3608 по вертикали.
        knock_back(player, dir, 0.4, player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE));
    } else if let (Some(_), Some(dir), true) = (done, push, killer == "entity.minecraft.warden") {
        // Сценарий warden-melee: после двух толчков (0,4 и 0,75) клиенту уходит
        // скорость после трения такта: (0; 0,314; 0,519).
        let resistance = player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE);
        knock_back(player, dir, 0.4, resistance);
        knock_back(player, dir, 0.75, resistance);
        if let Some((vx, _, vz)) = player.net.velocity {
            player.net.velocity = Some((vx * 0.546, (0.4 - 0.08) * 0.98, vz * 0.546));
        }
        player.net.velocity_due = false;
    } else if let (Some(_), Some(dir), true) = (done, push, killer == "entity.minecraft.slime" || killer == "entity.minecraft.pufferfish") {
        knock_back(player, dir, 0.4, 0.0);
    } else if let (Some(_), Some((ux, uz))) = (done, push) {
        // Клиенту уходит скорость уже после трения такта: 0,4 · 0,546 = 0,218
        // по горизонтали и 0,275 вверх (чёрный ящик, skeleton-arrow: стоящий
        // игрок после стрелы получает (0,218; 0,275)) [проверить: на ходу].
        let (dx, dy, dz) = player.net.last_delta;
        let up = if player.on_ground { 0.275 } else { dy };
        // Усиленный толчок разорителя — только сквозь щит; здесь — единичный.
        let length = ux.hypot(uz).max(1.0);
        let (ux, uz) = (ux / length, uz / length);

        player.net.velocity = Some((dx / 2.0 * 0.546 + ux * 0.218, up, dz / 2.0 * 0.546 + uz * 0.218));
        // Иначе конец такта заменит отдачу «скоростью после удара без толчка»
        // (чёрный ящик, cavespider-poison: укус толкает на 0,275 вверх).
        player.net.velocity_due = false;
    }

    if done.is_some() {
        if enchantments.melee_knockback > 0.0 && let Some(dir) = push {
            enchanted_mob_knockback(player, dir, enchantments.melee_knockback);
        }
        if enchantments.fire_ticks > 0 { ignite(player, enchantments.fire_ticks); }
        if source == Source::Arrow && let Some(dir) = push {
            let extra = crate::combat::punch_impulse(enchantments.punch as f32 * 1.2, dir, player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE));
            if let Some(velocity) = player.net.velocity.as_mut() {
                // mob_hit уже передал скорость через трение и тяжесть такта.
                *velocity = (velocity.0 + extra.0 * 0.546, velocity.1 + extra.1 * 0.98, velocity.2 + extra.2 * 0.546);
            }
        }
    }

    Ok(())
}

/// /damage: тот же расчёт защиты и неуязвимости, с явным источником.
/// [проверить] Отдача и порядок звуков команды относительно обычного удара.
pub(super) async fn command_hit(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, amount: f32, source: Source, origin: crate::combat::DamageOrigin) -> io::Result<()> {
    if player.vit.dead || (matches!(player.game_mode, GAME_MODE_CREATIVE | GAME_MODE_SPECTATOR) && !source.bypasses_invulnerability()) { return Ok(()); }
    // enchant-frost-immunity: отклонённые HotFloor/Campfire не изнашивают
    // обувь. Проверяем до износа брони и щита, а не только внутри hit_by.
    if crate::frost::protects(&player.inventory, source) { return Ok(()); }
    update_equipment(player, *shared.ticks.borrow());
    let at = origin.at.or(origin.entity_at);
    if amount > 0.0 && at.is_some() && !crate::combat::bypasses_armor(source)
        && let Some(hand) = shield_up(player, *shared.ticks.borrow())
        && at.is_some_and(|at| crate::combat::shield_covers(player.yaw, (player.x, player.z), (at.0, at.2)))
    {
        let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
        if let Some(shield) = player.inventory.slot(slot) {
            super::pvp::wear_slot(shared, player, slot, shield, crate::combat::shield_wear(amount), false);
        }
        shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x, player.y, player.z), "minecraft:item.shield.block", 1.0, 1.0);
        return Ok(());
    }
    // Броню и защитные чары считает hit_by_breached (общий путь); здесь — только износ.
    if source != Source::MobAttack && !crate::combat::bypasses_armor(source) {
        super::pvp::wear_armor(shared, player, amount);
    }
    player.net.hurt_from = at;
    player.net.command_origin = Some(origin.clone());
    let killer = origin.player.as_ref().map(|(name, uuid)| Killer { name, uuid: *uuid, weapon: None });
    // itemaudit2-shield-magic, официальный777: /damage indirect_magic без
    // источника даёт Hurt Animation yaw0 и случайную горизонтальную отдачу0,4.
    // Щит без позиции источника её не блокирует; прежний основной не меняем.
    let magic_feedback = shared.catalog().profile() == crate::gameplay_catalog::Profile::Preview26_3
        && source == Source::IndirectMagic && at.is_none() && origin.direct == 0 && origin.cause == 0;
    let result = hit_by(stream, shared, player, amount, source, origin.direct, origin.cause, killer, magic_feedback.then_some(0.0)).await;
    if magic_feedback && matches!(result, Ok(Some(_))) {
        let angle = f64::from(world_random(shared, player.dimension)) * std::f64::consts::TAU;
        let resistance = player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE);
        knock_back(player, (angle.cos(), angle.sin()), crate::mob::BASE_KNOCKBACK, resistance);
        player.net.check_support = Some(*shared.ticks.borrow() + 1);
    }
    player.net.command_origin = None;
    player.net.hurt_from = None;
    result.map(|_| ())
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
    amount: f32,
    source: Source,
    direct: i32,
    cause: i32,
    killer: Option<Killer<'_>>,
    hurt_yaw: Option<f32>,
) -> io::Result<Option<survival::Hit>> {
    hit_by_breached(stream, shared, player, amount, source, direct, cause, killer, hurt_yaw, 1.0).await
}

/// Вики, Difficulty/Arrow: урон снаряда игрока не является уроном моба.
/// Стрелы скелета и заряды бриза сохраняют масштабирование [проверить].
fn projectile_difficulty(amount: f32, source: Source, difficulty: u8, player_owned: bool) -> f32 {
    if source.scales_with_difficulty() && !(player_owned && matches!(source, Source::Arrow | Source::WindCharge)) {
        survival::scaled(amount, difficulty)
    } else { amount }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn hit_by_breached(
    stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, mut amount: f32,
    source: Source, direct: i32, cause: i32, killer: Option<Killer<'_>>, hurt_yaw: Option<f32>, breach: f32,
) -> io::Result<Option<survival::Hit>> {
    if crate::frost::protects(&player.inventory, source) { return Ok(None); }
    amount = projectile_difficulty(amount, source, shared.difficulty.load(std::sync::atomic::Ordering::Relaxed), killer.is_some());

    // Броня и защитные чары — разные слои: Feather Falling действует
    // на падение, Protection — на магию, Fire Protection — на горение.
    {
        let armor = player.attrs.value(crate::attributes::ARMOR) as f32;
        let toughness = player.attrs.value(crate::attributes::ARMOR_TOUGHNESS) as f32;
        let points: i32 = crate::combat::ARMOR_SLOTS.iter().filter_map(|(slot, _)| player.inventory.slot(*slot)).map(|stack| crate::combat::protection_points(&stack.extras(), source)).sum();
        amount = crate::combat::mitigated_damage(amount, armor, toughness, points, source, breach);
    }

    let immune = player.game_mode == GAME_MODE_CREATIVE || player.game_mode == GAME_MODE_SPECTATOR;
    let announced = std::mem::take(&mut player.net.announced);
    // Взрыв во время неуязвимости снимает лишь разницу с прошлым ударом и
    // молча: без события урона (чёрный ящик, wither-skull-hit: череп — магия
    // 5 и событие, взрыв следом — только здоровье) [проверить: у остальных
    // источников].
    let quiet = player.vit.invulnerable > 10 && matches!(source, Source::Explosion | Source::PlayerExplosion);
    let Some(mut result) = player.vit.hurt(amount, source, immune) else {
        return Ok(None);
    };

    super::pvp::retaliate(shared, player, source, if cause != 0 { cause } else { direct });

    let absorbed = result.absorbed;
    let resisted = result.resisted;
    player.progress.custom("damage_absorbed", (absorbed * 10.0).round() as i32);
    player.progress.custom("damage_resisted", (resisted * 10.0).round() as i32);
    if let Some(killer) = &killer {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
        for (name, value) in [("damage_dealt", result.taken - absorbed), ("damage_dealt_absorbed", absorbed), ("damage_dealt_resisted", resisted)] {
            players.order(&killer.uuid, crate::players::Order::Progress(crate::progress::Event::Stat(crate::stats::Cat::Custom, name, (value * 10.0).round() as i32)));
        }
        if matches!(source, Source::Arrow | Source::Trident) {
            players.order(&killer.uuid, crate::players::Order::Progress(crate::progress::Event::ProjectileHit {
                trident: source == Source::Trident, kind: "player", killed: result.died, distance: 0.0, crossbow: false,
            }));
        }
    }
    if quiet && !result.died {
        shared.players.lock().expect("список игроков захвачен другим потоком").set_health(&player.uuid, player.vit.health);
        player.progress.took(result.taken - absorbed);

        return Ok(Some(result));
    }
    let totem = result.died && !source.bypasses_invulnerability() && take_totem(player);

    if totem {
        result.died = false;
        player.vit.dead = false;
        player.vit.health = 1.0;
        player.progress.used_totem();
    }

    // `/kill` и пустота урона не считают (чёрный ящик: после /kill damage_taken не менялся).
    if !matches!(source, Source::GenericKill | Source::OutOfWorld) {
        player.progress.took(result.taken - absorbed);
    }

    let damage_at = player.net.command_origin.as_ref().and_then(|origin| origin.at);
    write_body(stream, damage_packet(player.entity_id, source, direct, cause, damage_at)).await?;

    {
        let (type_id, culprit, direct) = damage_fields(source, direct, cause);

        // Остальным о шипах сообщено в такт удара: здесь, в конце такта, их
        // рассылка уже ушла бы на такт позже (чёрный ящик, pvp-thorns).
        if !announced {
            shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_command_damage(player.entity_id, type_id, culprit, direct, damage_at);
        }
        shared.players.lock().expect("список игроков захвачен другим потоком").set_health(&player.uuid, player.vit.health);
        let mut world = shared.world_in(player.dimension).lock().expect("мир");
        crate::redstone::game_event_from(&mut world, (player.x, player.y, player.z), if result.died { 15 } else { 7 }, Some(player.entity_id), false);
    }

    // Анимация ушиба идёт сразу за событием урона, раньше сообщения о смерти.
    if let Some(yaw) = hurt_yaw {
        send_hurt_animation(stream, player.entity_id, yaw).await?;
    }

    // Удар моба или стрелы: анимация «ранен» с направлением на виновника,
    // у стрелы ещё звук попадания (чёрный ящик, skeleton-arrow).
    if let Some(from) = player.net.hurt_from
        && matches!(source, Source::MobAttack | Source::Sting | Source::Arrow | Source::Trident | Source::WindCharge | Source::SonicBoom | Source::Thorns)
    {
        // Угол приведён к [-180; 180): у оригинала игрок с рысканьем 180 видит 90, не −270.
        let toward = (from.2 - player.z).atan2(from.0 - player.x).to_degrees() - f64::from(player.yaw);
        // Отражение шипов использует тот же диапазон [0;360), что PvP:
        // enchant-mob-thorns-wear даёт yaw270, а не эквивалентное -90.
        let yaw = if source == Source::Thorns { toward.rem_euclid(360.0) }
            else { (toward + 180.0).rem_euclid(360.0) - 180.0 };
        let yaw = yaw as f32;
        let mut body = encode_varint(HURT_ANIMATION);

        body.extend_from_slice(&encode_varint(player.entity_id));
        push_f32(&mut body, yaw);
        write_body(stream, body).await?;

        if source == Source::Arrow {
            let pitch = 1.2 + (world_random(shared, player.dimension) - 0.5) * 0.4;

            // Стрела арбалета разбойника: item.crossbow.hit, раздел «нейтральные»
            // (чёрный ящик, pillager-crossbow).
            if player.net.killer == "entity.minecraft.pillager" {
                send_sound_at(stream, "minecraft:item.crossbow.hit", SOUND_NEUTRAL, from, 1.0, pitch).await?;
            } else {
                send_sound_at(stream, "minecraft:entity.arrow.hit", SOUND_PLAYERS, from, 1.0, pitch).await?;
            }
        }
    }

    // Скорость после удара клиенту сообщается всегда, кроме ударов без
    // отдачи (утопление, голод, взрыв): чёрный ящик, drowning и fall-*.
    if !matches!(source, Source::Drown | Source::Starve | Source::Explosion | Source::PlayerExplosion) {
        player.net.velocity_due = true;
    }

    if totem {
        revive_by_totem(stream, shared, player).await?;
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
async fn revive_by_totem(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    clear_effects(stream, player, None).await?;
    player.vit.absorption = 0.0;

    for (kind, amplifier, ticks) in [
        (survival::EffectKind::Regeneration, 1, 900),
        (survival::EffectKind::Absorption, 1, 100),
        (survival::EffectKind::FireResistance, 0, 800),
    ] {
        give_effect(stream, player, survival::Effect { kind, amplifier, ticks_left: ticks, ambient: false, visible: true, show_icon: true }).await?;
    }

    shared.world_in(player.dimension).lock().expect("мир").note_totem(player.entity_id);
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

/// Вики, Blast Protection: защита гасит именно добавленный толчок,
/// включая Wind Burst, а не собственную скорость игрока [проверить].
pub(super) fn protected_blast_push(player: &PlayerState, push: (f64, f64, f64)) -> (f64, f64, f64) {
    let scale = 1.0 - player.attrs.value(crate::attributes::EXPLOSION_KNOCKBACK_RESISTANCE);
    (push.0 * scale, push.1 * scale, push.2 * scale)
}

/// Вики, Fire Protection: сокращение срока поджога, без тушения уже
/// идущего огня. Округление дробного числа тактов [проверить].
pub(super) fn ignite(player: &mut PlayerState, ticks: i32) {
    // Модификатор в float (−0,6000000238…) даёт 31,9999…: прибавка гасит округление вниз [проверить: у оригинала].
    let ticks = (f64::from(ticks) * player.attrs.value(crate::attributes::BURNING_TIME) + 1.0e-4).floor() as i32;
    player.vit.ignite(ticks);
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

/// Второй толчок зачарованного оружия моба. Основной mob_hit уже применил
/// трение такта, поэтому сначала возвращаем скорость до этой стадии.
/// [проверить] Фаза движущейся/воздушной цели в enchant-mob-melee-effects.
pub(super) fn enchanted_mob_knockback(player: &mut PlayerState, dir: (f64, f64), level: f32) {
    let Some(velocity) = player.net.velocity else { return; };
    let base = (velocity.0 / 0.546, velocity.1 / 0.98 + 0.08, velocity.2 / 0.546);
    let after = combat::knockback_velocity(base, dir, f64::from(level) * 0.5,
        player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE), player.on_ground);
    player.net.velocity = Some((after.0 * 0.546, (after.1 - 0.08) * 0.98, after.2 * 0.546));
    player.net.velocity_due = false;
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

/// tmp/bb/throw2-ender_pearl-down30-yaw0-standing.md:12,18–19: урон
/// сейчас, свойства и скорость — следующим тактом, после Set Health.
pub(super) async fn pearl_hit(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    if hit(stream, shared, player, 5.0, Source::EnderPearl, 0).await?.is_some() {
        player.net.hold_meta = true;
        // В нашей стадии C таймер убывает в конце, а следующий урон
        // среды проверяется до убывания. Сохраняем эту фазу и при ранней C
        // (throw2-ender_pearl-up45-yaw0-standing.md:32,42: интервал 10).
        if player.net.pearl_entity_ticked { player.vit.tick_invulnerable(); }
    }
    Ok(())
}

/// tmp/bb/throw2-ender_pearl-down30-yaw0-standing.md:15–20: в такт
/// попадания 15 HP, у нас было сразу 16 — мирное лечение ошибочно шло
/// ПОСЛЕ урона жемчуга. Выполняем ход игрока в старой точке до переноса;
/// ниже стадию C повторять нельзя. Одновременные другие удары [проверить].
pub(super) async fn before_pearl_teleport(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    if !player.net.pearl_entity_ticked && !player.vit.dead {
        let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
        if mortal(player.game_mode) {
            entity_tick(stream, shared, player, difficulty).await?;
        } else if player.game_mode == GAME_MODE_CREATIVE {
            creative_tick(stream, shared, player, difficulty).await?;
        }
        player.net.pearl_entity_ticked = true;
    }
    Ok(())
}

/// Такт здоровья игрока: стадии A–D (см. начало файла).
pub(super) async fn tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let entity = player.entity_id;
    let tick_now = player.publication.as_ref().map_or_else(|| *shared.ticks.borrow(), |p| p.tick);

    tick_spin(shared, player, tick_now);
    let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);

    // Творческий игрок сыт полностью: насыщение 20 (чёрный ящик, pvp-thorns:
    // после возврата из творческого режима у оригинала saturation=20, у нас 5).
    if player.game_mode == GAME_MODE_CREATIVE && shared.catalog().profile() != crate::gameplay_catalog::Profile::Preview26_3 {
        player.vit.food = 20;
        player.vit.saturation = 20.0;
    }

    // A. удары, ждавшие своего такта.
    for (block, count, wide) in std::mem::take(&mut player.net.landings) {
        send_landing_particles(stream, (player.x, player.y, player.z), block, count, wide).await?;
    }

    for pending in std::mem::take(&mut player.net.hits) {
        let killer = pending.killer.as_ref().map(|(name, uuid)| Killer { name, uuid: *uuid, weapon: None });

        player.net.announced = pending.announced;
        player.net.announced_health = None;

        let done = hit_by(stream, shared, player, pending.amount, pending.source, pending.direct, 0, killer, pending.hurt_yaw).await?;

        if pending.source == Source::Fall
            && let Some(vy) = player.net.landing_velocity.take()
            && done.is_some()
        {
            // fall-mode-teleport-motion: -1,195 после смены режима,
            // затем обычные приземления после /tp снова дают -0,078.
            player.net.velocity_due = false;
            player.net.velocity = Some((player.net.last_delta.0, vy, player.net.last_delta.2));
        }

        // Шипы отбрасывают ударившего, как обычный удар: скорость идёт сразу
        // за анимацией ушиба (чёрный ящик, pvp-thorns: (0; 0,361; −0,4)).
        if let (Some(_), Some((ux, uz)), true) = (done, pending.push, pending.source == Source::IndirectMagic) {
            // Брошенное зелье толкает, как стрела или удар моба: скорость после
            // трения такта (0,218; 0,275), одним тактом позже урона (чёрный ящик,
            // itemaudit2-splash-mob-harming: «робот: скорость 0,001; 0,275; −0,218»).
            let (dx, dy, dz) = player.net.last_delta;
            let up = if player.on_ground { 0.275 } else { dy };

            player.net.velocity = Some((dx / 2.0 * 0.546 + ux * 0.218, up, dz / 2.0 * 0.546 + uz * 0.218));
            player.net.velocity_due = false;
            player.net.hold_meta = true;
        } else if let (Some(_), Some(dir)) = (done, pending.push) {
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

    if !player.sneaking {
        player.net.hide_sneak_from = None;
    }

    if player.sneaking && !player.net.hide_sneak_from.is_some_and(|from| tick_now >= from) {
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

    // Полёт на элитрах: флаг 0x80 — в такте начала, поза «полёт» (1) —
    // тактом позже (elytra-glide: Metadata 0=-128 на +0, 6=1 на +1).
    if player.glide.active {
        flags |= FLAG_FALL_FLYING;
    }

    // Начал приседать, сидя в транспорте (или сойдя в этом же такте), — позы нет.
    if flags & 0x02 != 0 && player.net.meta.flags & 0x02 == 0 {
        player.net.sneak_riding = player.net.riding_prev || player.ride.vehicle.is_some();
    }

    let now = Meta {
        score: player.vit.xp_total,
        arrows: player.net.arrows,
        stingers: player.net.stingers,
        health: player.vit.health,
        air: player.vit.air,
        frozen: player.net.frozen,
        flags,
        pose: if player.sleeping.is_some() {
            POSE_SLEEPING
        } else if player.net.spin_pose {
            POSE_SPIN_ATTACK
        } else if player.glide.active && player.glide.ticks > 0 {
            POSE_FALL_FLYING
        } else if player.net.meta.flags & 0x02 != 0 && player.ride.vehicle.is_none() && !player.net.sneak_riding {
            // Поза «присел» — тактом позже флага и не у седока (чёрный ящик,
            // donkey-chest: 0=2 на +20, 6=5 на +21; на лошади позы нет) и сходит
            // тоже тактом позже (чёрный ящик, slime-sneak-landing: 0=2 на +0, 6=5 на +1).
            POSE_CROUCHING
        } else {
            0
        },
        living: living_flags(player),
        effects: player.vit.effect_colors(),
        ambient: player.vit.effects_ambient().unwrap_or(player.net.meta.ambient),
        absorption: player.vit.absorption,
        bed: player.sleeping,
    };

    player.net.riding_prev = player.ride.vehicle.is_some();
    let held = std::mem::take(&mut player.net.hold_meta);

    // Флаг «крадётся» оригинал шлёт и самому (boat-ride, boat-drive:
    // приседание в лодке — Metadata 0=2, в следующий такт 0=0).
    if !held && now != player.net.meta && tick_now >= player.net.meta_hold {
        let before = player.net.meta;

        if now != before {
            send_meta(stream, entity, now, before).await?;
        }

        player.net.meta = now;
    }

    let now = tick_now;

    super::movement_enchantments::update(shared, player);
    sync_attributes(stream, player, now).await?;

    if !player.net.armor_out.is_empty() {
        let entries = std::mem::take(&mut player.net.armor_out);

        shared.players.lock().expect("игроки").armor_changed(&player.uuid, player.entity_id, entries);
    }
    crate::commands_world::publish_attributes(shared, &player.uuid, &player.attrs);

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

    // Окно рассылает изменения по возрастанию номеров мест (чёрный ящик,
    // tails-honey-offhand: пустая бутылка в 37 раньше мёда в 45).
    let mut dirty = std::mem::take(&mut player.net.slots_dirty);

    dirty.sort_unstable();
    for slot in dirty {
        send_container_slot(stream, player, slot).await?;
    }

    for slot in std::mem::take(&mut player.net.slots_later) {
        player.net.slot_changed(slot);
    }

    if !mortal(player.game_mode) { player.net.frozen = 0; }

    // C. такт сущности.
    if !std::mem::take(&mut player.net.pearl_entity_ticked) {
        if !player.vit.dead && mortal(player.game_mode) {
            entity_tick(stream, shared, player, difficulty).await?;
        } else if !player.vit.dead && player.game_mode == GAME_MODE_CREATIVE {
            creative_tick(stream, shared, player, difficulty).await?;
        }
    }

    // Шарики опыта: появление, полёт к игроку, подбор.
    if !player.vit.dead {
        orbs_tick(stream, shared, player).await?;
    }

    if let Some(vy) = player.net.mode_fall_velocity {
        player.net.mode_fall_velocity = Some(medium_step(vy, player.net.in_lava, player.net.in_water));
    }

    // Скорость, которую сервер ведёт после удара, за такт делает шаг.
    if let Some(pinned) = player.net.pin {
        player.net.pin = Some(medium_step(pinned, player.net.in_lava, player.net.in_water));
    }

    // Бой и экипировка публикуются вместе со звуком надевания.
    super::publish_player(shared, player);

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
    for points in std::mem::take(&mut player.net.xp_taken) {
        let (slots, remaining) = crate::combat::mend_all(&mut player.inventory, points, || world_random(shared, player.dimension));
        for slot in slots {
            player.net.slot_changed(slot);
        }
        if remaining > 0 {
            player.vit.add_xp(remaining);
            send_experience(stream, &player.vit).await?;
        }
    }

    Ok(())
}

/// Творческий режим: тонуть не тонет, но бездна бьёт и здесь.
async fn creative_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, difficulty: u8) -> io::Result<()> {
    player.vit.age = player.vit.age.wrapping_add(1);
    // Оригинал263: creative не ставит насыщение20 сразу (mace:12/16).
    // saturation-ticks: мирное восстановление по1; свежий pvp-thorns263
    // на easy сохраняет5, пока прежнее ядро ошибочно накапливало8.
    // С выключенной natural_health_regeneration значение сохраняется.
    // [проверить] Фаза20 тактов вне этой записи.
    if shared.catalog().profile() == crate::gameplay_catalog::Profile::Preview26_3
        && difficulty == survival::PEACEFUL
        && shared.natural_regeneration.load(std::sync::atomic::Ordering::Relaxed)
        && player.vit.age.is_multiple_of(20)
    {
        player.vit.heal(1.0);
        player.vit.saturation = (player.vit.saturation + 1.0).min(player.vit.food as f32);
    }
    effects_and_eating_tick(stream, shared, player, difficulty).await?;
    if shared.catalog().profile() == crate::gameplay_catalog::Profile::Preview26_3 {
        // Creative тоже лечится едой: после thorns/падения оригинал263
        // расходует насыщение, а мирное восстановление восполняет его.
        player.vit.food_tick(difficulty, shared.natural_regeneration.load(std::sync::atomic::Ordering::Relaxed));
    }
    if player.y < void_below(player.dimension) {
        hit(stream, shared, player, 4.0, Source::OutOfWorld, 0).await?;
    }

    player.vit.tick_invulnerable();

    if player.net.stingers > 0 {
        player.net.stinger_timer -= 1;

        if player.net.stinger_timer <= 0 {
            player.net.stingers -= 1;
            player.net.stinger_timer = 20 * (30 - player.net.stingers);
        }
    }

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

/// Молния задела игрока (вики, Lightning): поджигает (горящему — ещё
/// такт огня) и бьёт на 5.
pub(super) async fn struck(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    if player.vit.dead || !mortal(player.game_mode) {
        return Ok(());
    }

    if player.vit.fire > 0 {
        player.vit.fire += 1;
    } else {
        ignite(player, crate::lightning::STRIKE_FIRE_TICKS);
    }

    hit(stream, shared, player, crate::lightning::STRIKE_DAMAGE, Source::Lightning, 0).await?;

    Ok(())
}

fn observe_oxygen(shared: &Shared, player: &mut PlayerState, submerged: bool) {
    if submerged && !player.net.oxygen_seen && shared.catalog().changed_rules() {
        player.net.oxygen_seen = true;
        player.attrs_touched.push((ATTRIBUTE_OXYGEN, *shared.ticks.borrow() + 1));
    }
}

async fn entity_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, difficulty: u8) -> io::Result<()> {
    let entity = player.entity_id;
    let at = (player.x, player.y, player.z);
    let env = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        look_around(&world, player.x, player.y, player.z, player.sneaking, player.net.spin_pose && player.net.spin_since.is_some_and(|since| since >= 3))
    };

    player.vit.age = player.vit.age.wrapping_add(1);

    // Powder Snow: «140 game ticks», затем 1 HP раз в 40 тактов.
    // Любая надетая кожаная вещь защищает, предмет в руке — нет.
    // [проверить]: фаза первого урона и пакет замедления.
    let leather = crate::combat::ARMOR_SLOTS.iter().any(|(slot, _)| {
        player.inventory.slot(*slot).and_then(|s| blocks::item_name(s.item)).is_some_and(|n| matches!(n,
            "leather_helmet" | "leather_chestplate" | "leather_leggings" | "leather_boots"))
    });
    player.net.frozen = freezing_step(player.net.frozen, env.powder_snow, leather);
    if player.net.frozen == 140 && player.vit.age.is_multiple_of(40)
        && shared.freeze_damage.load(std::sync::atomic::Ordering::Relaxed)
    {
        hit(stream, shared, player, 1.0, Source::Freeze, 0).await?;
    }
    if env.powder_snow && player.vit.fire > 0 {
        player.vit.extinguish();
        let mut w = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let (x, y, z) = (player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32);
        for by in y..=y + 1 {
            if blocks::block_at_state(w.get_block(x, by, z)) == Some("powder_snow") { w.set_block(x, by, z, world::AIR); }
        }
        redstone::settle(&mut w);
    }


    player.net.in_lava = env.in_lava;
    player.net.in_water = env.in_water;

    // Дождь тушит огонь молча (вики, Weather: «Extinguishes fires»; чёрный
    // ящик, rain-fire-player: ни звука, ни признака «горит»).
    if env.in_rain && !env.in_water && player.vit.fire > 0 {
        player.vit.extinguish();
    }

    // Вода тушит огонь со звуком.
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

    if env.in_fire > 0.0 && !player.vit.dead && !player.vit.has_effect(survival::EffectKind::FireResistance) {
        ignite(player, 160);
        hit(stream, shared, player, env.in_fire, Source::InFire, 0).await?;
    }

    if env.campfire > 0.0 && !player.vit.dead && !protected_from_hot_blocks(player) {
        hit(stream, shared, player, env.campfire, Source::Campfire, 0).await?;
    }

    if !player.vit.dead && magma_contact(player, &env) {
        hit(stream, shared, player, 1.0, Source::HotFloor, 0).await?;
    }

    if env.cactus && !player.vit.dead {
        hit(stream, shared, player, 1.0, Source::Cactus, 0).await?;
    }

    // Куст ягод колет того, кто в нём движется (вики, Sweet Berry Bush:
    // «moving… at least 0.003 blocks»); замедление клиент делает сам
    // [проверить чёрным ящиком].
    let (mx, _, mz) = player.net.last_delta;

    if env.berries && !player.vit.dead && (mx.abs() >= 0.003 || mz.abs() >= 0.003) {
        hit(stream, shared, player, 1.0, Source::SweetBerryBush, 0).await?;
    }

    if env.in_wall && !player.vit.dead {
        hit(stream, shared, player, 1.0, Source::InWall, 0).await?;
    }

    // За рамкой мира дальше запаса — урон (commands_world::Border::damage_at).
    if !player.vit.dead
        && let Some(amount) = crate::commands_world::border_of(shared, player.dimension).damage_at(player.x, player.z)
    {
        hit(stream, shared, player, amount, Source::OutsideBorder, 0).await?;
    }

    if env.in_lava && !player.vit.dead {
        ignite(player, survival::LAVA_FIRE_TICKS);
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

    if let Some(effect) = turtle_shell_effect(player, env.eyes_in_water) {
        give_effect(stream, player, effect).await?;
    }

    observe_oxygen(shared, player, env.eyes_in_water && !env.in_bubbles);
    let oxygen = player.attrs.value(crate::attributes::OXYGEN_BONUS);
    let oxygen_roll = if env.eyes_in_water && !env.in_bubbles && oxygen > 0.0 { world_random(shared, player.dimension) } else { 0.0 };
    if !player.vit.dead && player.vit.air_tick_with_bonus(env.eyes_in_water && !env.in_bubbles, false, oxygen, oxygen_roll) {
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

    effects_and_eating_tick(stream, shared, player, difficulty).await?;

    if let Some(amount) = player.vit.food_tick(difficulty, shared.natural_regeneration.load(std::sync::atomic::Ordering::Relaxed)) {
        hit(stream, shared, player, amount, Source::Starve, 0).await?;
    }

    player.vit.tick_invulnerable();

    Ok(())
}

// Реакции общие для Java и адаптера Bedrock.
pub(super) fn effect_reactions(shared: &Shared, player: &mut PlayerState, dead: bool) {
    let reaction = crate::effect_reactions::take(&mut player.vit, dead, (player.x, player.y, player.z), 1.8, player.yaw, true);
    if !reaction.wind && !reaction.weaving && !reaction.oozing && reaction.infested_hits == 0 { return; }
    let targets = crate::tick::blast_targets(shared, player.dimension);
    let mut world = shared.world_in(player.dimension).lock().expect("мир");
    let mut rng = crate::mob::Rng::new(u64::from(world.random_bits()));
    let mut items = shared.items_in(player.dimension).lock().expect("сущности");
    crate::effect_reactions::apply(&mut world, &mut items, reaction, shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed), &targets, &mut || shared.next_entity_id(), &mut rng);
}

/// Эффекты и использование работают и в творческом режиме: предметы
/// сохраняются, сроки эффектов убывают. Порядок пакетов [проверить].
async fn effects_and_eating_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, difficulty: u8) -> io::Result<()> {
    effect_reactions(shared, player, false);
    let entity = player.entity_id;
    let survival::EffectTick { hurt, gone, restored } = player.vit.effects_tick();

    trial_omen(stream, shared, player, difficulty).await?;
    omens(stream, shared, player, difficulty, &gone).await?;

    for (amount, source) in hurt {
        hit(stream, shared, player, amount, source, 0).await?;
    }

    if !gone.is_empty() || !restored.is_empty() {
        apply_effect_modifiers(player);
    }

    for effect in restored {
        let mut body = encode_varint(ENTITY_EFFECT);
        body.extend_from_slice(&encode_varint(entity));
        body.extend_from_slice(&encode_varint(effect.kind.id()));
        body.extend_from_slice(&encode_varint(effect.amplifier));
        body.extend_from_slice(&encode_varint(effect.ticks_left));
        body.push(effect.flags() & !0x08);
        write_body(stream, body).await?;
    }

    for gone in gone {
        let mut body = encode_varint(REMOVE_ENTITY_EFFECT);

        body.extend_from_slice(&encode_varint(entity));
        body.extend_from_slice(&encode_varint(gone.id()));
        write_body(stream, body).await?;
    }

    tick_crossbow(shared, player, *shared.ticks.borrow());
    super::brush::tick(player,shared,*shared.ticks.borrow());
    super::bundle_use::tick(player,shared,*shared.ticks.borrow());

    // Недоеденное — после такта эффектов: эффект от еды впервые действует
    // в следующем такте (golden-apple: регенерация лечит на +1 после Effect).
    if player.net.eating.as_ref().is_some_and(|eating| !eating_matches(player, eating)) {
        stop_eating(player);
    }
    if let Some(eating) = &mut player.net.eating {
        eating.ticks_left -= 1;

        if eating.ticks_left <= 0 {
            let (slot, nutrition, saturation, drink) = (eating.slot, eating.nutrition, eating.saturation, eating.drink);
            let effects = std::mem::take(&mut eating.effects);

            player.net.eating = None;
            {
                let mut world = shared.world_in(player.dimension).lock().expect("мир");
                crate::redstone::game_event_from(&mut world, (player.x, player.y, player.z), 8, Some(player.entity_id), false);
            }

            match drink {
                Some(drink) => finish_drinking(stream, shared, player, slot, drink).await?,
                None => finish_eating(stream, shared, player, slot, nutrition, saturation).await?,
            }

            // Эффекты еды — после звуков еды [проверить] (golden-apple).
            // Мгновенные из зелья действуют сразу, без пакета эффекта
            // (potion-drink: Set Health в такте глотка, Entity Effect нет).
            for effect in effects {
                match effect.kind {
                    survival::EffectKind::InstantHealth => player.vit.heal(crate::survival::instant_base(crate::survival::EffectKind::InstantHealth, effect.amplifier) as f32),
                    survival::EffectKind::InstantDamage => {
                        hit(stream, shared, player, crate::survival::instant_base(crate::survival::EffectKind::InstantDamage, effect.amplifier) as f32, Source::Magic, 0).await?;
                    }
                    _ => give_effect(stream, player, effect).await?,
                }
            }
        }
    }

    Ok(())
}

async fn trial_omen(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, difficulty: u8) -> io::Result<()> {
    use survival::EffectKind::{BadOmen, TrialOmen};
    if !mortal(player.game_mode) || player.vit.dead || difficulty == survival::PEACEFUL { return Ok(()); }
    let bad=player.vit.effect_level(BadOmen);
    if bad==0 && !player.vit.has_effect(TrialOmen) { return Ok(()); }
    let places={
        let world=shared.world_in(player.dimension).lock().expect("мир");
        crate::trial::omen_candidates(&world,(player.x,player.y,player.z),if player.sneaking {1.27} else {1.62})
    };
    if places.is_empty() { return Ok(()); }
    if bad>0 {
        let remaining=player.vit.effects.iter().find(|e|e.kind==TrialOmen).map_or(0,|e|e.ticks_left);
        clear_effects(stream,player,Some(BadOmen)).await?;
        give_effect(stream,player,survival::Effect {kind:TrialOmen,amplifier:0,ticks_left:crate::trial::omen_duration(bad,remaining),ambient:false,visible:true, show_icon: true}).await?;
    }
    crate::trial::activate_omen(&mut shared.world_in(player.dimension).lock().expect("мир"),&places);
    Ok(())
}

/// Знамения (вики, Bad Omen и Raid Omen): «Дурное знамение» в деревне
/// Обычного мира сменяется «Знамением набега» на 30 секунд того же уровня;
/// кончилось «Знамение набега» — заявка на набег в мире (raid.rs).
async fn omens(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, difficulty: u8, gone: &[survival::EffectKind]) -> io::Result<()> {
    if matches!(player.game_mode,0|2) && difficulty!=survival::PEACEFUL {
        let bad=player.vit.effects.iter().find(|e|e.kind==survival::EffectKind::BadOmen).copied();
        if bad.is_some() || player.vit.has_effect(survival::EffectKind::TrialOmen) {
            let removed=crate::trial::make_ominous(&mut shared.world_in(player.dimension).lock().expect("мир"),(player.x,player.y,player.z),*shared.ticks.borrow());
            if let Some(ids)=removed {
                shared.items_in(player.dimension).lock().expect("сущности").remove_mobs(|m|ids.contains(&m.entity_id));
                if let Some(bad)=bad {
                    clear_effects(stream,player,Some(survival::EffectKind::BadOmen)).await?;
                    give_effect(stream,player,survival::Effect {kind:survival::EffectKind::TrialOmen,amplifier:0,ticks_left:18000*(bad.amplifier+1),ambient:false,visible:true, show_icon:true}).await?;
                }
            }
        }
    }
    let raids_on = shared.raids.load(std::sync::atomic::Ordering::Relaxed);

    if gone.contains(&survival::EffectKind::RaidOmen) && raids_on && player.dimension == Dimension::Overworld && difficulty != survival::PEACEFUL {
        // Уровень уже снятого эффекта не известен: берётся запомненный.
        let level = player.net.raid_omen_level.max(1);

        shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком").raids.requests.push(crate::raid::Request { player: player.entity_id, at: (player.x, player.y, player.z), level });
    }

    // Знамение набега пришло прошлым тактом: оригинал шлёт его ещё раз уже
    // без плавного появления (flags 6) с тем же сроком и лишь теперь снимает
    // «Дурное знамение» (чёрный ящик, raid-start-wave1: такт 1).
    if player.net.omen_switch {
        player.net.omen_switch = false;

        if let Some(raid) = player.vit.effects.iter().find(|effect| effect.kind == survival::EffectKind::RaidOmen).copied() {
            let mut body = encode_varint(ENTITY_EFFECT);

            body.extend_from_slice(&encode_varint(player.entity_id));
            body.extend_from_slice(&encode_varint(raid.kind.id()));
            body.extend_from_slice(&encode_varint(raid.amplifier));
            body.extend_from_slice(&encode_varint(raid.ticks_left));
            body.push(raid.flags() & !0x08);
            write_body(stream, body).await?;
        }

        return clear_effects(stream, player, Some(survival::EffectKind::BadOmen)).await;
    }

    let Some(bad) = player.vit.effects.iter().find(|effect| effect.kind == survival::EffectKind::BadOmen).copied() else {
        return Ok(());
    };

    if !raids_on || player.dimension != Dimension::Overworld || difficulty == survival::PEACEFUL || player.game_mode == 3 {
        return Ok(());
    }

    // Знамение набега уже есть: ждём его конца.
    if player.vit.effects.iter().any(|effect| effect.kind == survival::EffectKind::RaidOmen) {
        return Ok(());
    }

    let in_village = {
        let items = shared.items_in(Dimension::Overworld).lock().expect("предметы захвачены другим потоком");
        let pois: Vec<(i32, i32, i32)> = items.mobs().iter().filter(|mob| mob.kind == crate::mob::Kind::Villager && !mob.is_dead()).filter_map(|mob| mob.villager.job_site).collect();

        crate::raid::in_village(&pois, (player.x, player.y, player.z))
    };

    if !in_village {
        return Ok(());
    }

    player.net.raid_omen_level = bad.amplifier + 1;
    give_effect(stream, player, survival::Effect { kind: survival::EffectKind::RaidOmen, amplifier: bad.amplifier, ticks_left: crate::raid::RAID_OMEN_TICKS, ambient: false, visible: true, show_icon: true }).await?;

    // Первый такт новое знамение не убывает: снимается оно через 601 такт
    // (raid-start-wave1: Remove Effect в такте 601).
    if let Some(raid) = player.vit.effects.iter_mut().find(|effect| effect.kind == survival::EffectKind::RaidOmen) {
        raid.ticks_left += 1;
    }

    player.net.omen_switch = true;
    Ok(())
}

pub(super) fn world_random(shared: &Shared, dimension: Dimension) -> f32 {
    shared.world_in(dimension).lock().expect("мир захвачен другим потоком").random_unit()
}

pub(super) fn eating_matches(player: &PlayerState, eating: &Eating) -> bool {
    player.inventory.slot(eating.slot).is_some_and(|stack| stack.stacks_with(&eating.stack))
        && (eating.slot == inventory::OFFHAND || eating.slot == player.inventory.selected_slot())
}

/// Расходует одну порцию, возвращает посуду в освободившийся слот или
/// рюкзак. Полный инвентарь возвращает остаток для выброса [проверить].
pub(super) fn consume_stack(player: &mut PlayerState, slot: i32, remainder: &str) -> Option<Stack> {
    let stack = player.inventory.slot(slot)?;
    player.progress.consumed(stack.item);
    if player.game_mode == GAME_MODE_CREATIVE {
        return None;
    }
    player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
    player.net.slot_changed(slot);
    let item = blocks::item_named(remainder)?;
    let left = Stack::new(item, 1);
    if player.inventory.slot(slot).is_none() {
        player.inventory.set(slot, Some(left));
        None
    } else {
        let (slots, left) = player.inventory.put(left);
        for slot in slots { player.net.slot_changed(slot); }
        left
    }
}

async fn finish_eating(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, slot: i32, nutrition: i32, saturation: f32) -> io::Result<()> {
    let at = (player.x, player.y, player.z);
    let stack = player.inventory.slot(slot);
    let name = stack.and_then(|stack| blocks::item_name(stack.item)).unwrap_or("");

    player.vit.eat(nutrition, saturation);
    if name == "chorus_fruit" {
        let destination = {
            let mut world = shared.world_in(player.dimension).lock().expect("мир");
            let mut rng = crate::mob::Rng::new(u64::from(world.random_bits()));
            crate::teleport_item::chorus_destination(&world, at, &mut rng)
        };
        if let Some(to) = destination {
            super::vehicle::dismount(shared, player);
            shared.players.lock().expect("игроки").order(&player.uuid, crate::players::Order::Teleport { x:to.0,y:to.1,z:to.2 });
            player.vit.fall = 0.0;
            shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at(at, "minecraft:item.chorus_fruit.teleport", 1.0, 1.0);
        }
        player.item_cooldowns.insert("chorus_fruit", *shared.ticks.borrow() + 20);
        send_cooldown(stream,"minecraft:chorus_fruit",20).await?;
    }
    if let Some(stack)=stack.filter(|_| name == "suspicious_stew") {
        for (id,ticks) in stack.extras().stew {
            if let Some(kind)=survival::EffectKind::ALL.into_iter().find(|k| k.id()==i32::from(id)) {
                give_effect(stream,player,survival::Effect { kind,amplifier:0,ticks_left:ticks,ambient:false,visible:true, show_icon: true }).await?;
            }
        }
    }
    if let Some(effect) = survival::food_illness(name, world_random(shared, player.dimension)) {
        give_effect(stream, player, effect).await?;
    }

    let remainder = if matches!(name, "suspicious_stew" | "mushroom_stew" | "rabbit_stew" | "beetroot_soup") { "bowl" } else { "" };
    if let Some(left) = consume_stack(player, slot, remainder) {
        super::throw_out(shared, player, left);
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
async fn finish_drinking(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, slot: i32, drink: Drink) -> io::Result<()> {
    if let Some(left) = consume_stack(player, slot, drink.remainder()) {
        super::throw_out(shared, player, left);
    }

    send_entity_event(stream, player.entity_id, EVENT_FINISHED_EATING).await?;

    if drink == Drink::Milk {
        clear_effects(stream, player, None).await?;
    } else if drink == Drink::Honey {
        player.vit.eat(6, 1.2);
        // Вики, Honey Bottle: «unique sound»; после него — отрыжка, как у еды
        // (чёрный ящик, tails-honey-offhand: оба звука до снятия яда).
        let at = (player.x, player.y, player.z);
        let (a, b, c) = {
            let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            (world.random_unit(), world.random_unit(), world.random_unit())
        };

        send_sound_at(stream, "minecraft:item.honey_bottle.drink", SOUND_PLAYERS, at, 1.0, 1.0 + (a - b) * 0.4).await?;
        send_sound_at(stream, "minecraft:entity.player.burp", SOUND_PLAYERS, at, 0.5, c * 0.1 + 0.9).await?;
        clear_effects(stream, player, Some(survival::EffectKind::Poison)).await?;
    }

    Ok(())
}

/// Эффект от команды: клиенту — Entity Effect.
pub(super) async fn give_effect(stream: &mut Outgoing<'_>, player: &mut PlayerState, effect: survival::Effect) -> io::Result<()> {
    if !player.vit.effect_replaced_by(effect.kind, effect.amplifier, effect.ticks_left) {
        player.vit.give_effect_shown(effect);
        return Ok(());
    }

    // Обновлённый эффект того же вида приходит без плавного появления
    // (lingering-potion-feet: первое наложение flags=14, повторные — 6).
    let renewed = player.vit.effects.iter().any(|old| old.kind == effect.kind);

    player.vit.give_effect_shown(effect);
    apply_effect_modifiers(player);

    // Знамения звучат при наложении (чёрный ящик, raid-start-wave1:
    // event.mob_effect.bad_omen и raid_omen, раздел «игроки»).
    let omen = match effect.kind {
        survival::EffectKind::BadOmen => Some("minecraft:event.mob_effect.bad_omen"),
        survival::EffectKind::RaidOmen => Some("minecraft:event.mob_effect.raid_omen"),
        _ => None,
    };

    let mut body = encode_varint(ENTITY_EFFECT);

    body.extend_from_slice(&encode_varint(player.entity_id));
    body.extend_from_slice(&encode_varint(effect.kind.id()));
    body.extend_from_slice(&encode_varint(effect.amplifier));
    body.extend_from_slice(&encode_varint(if effect.kind.instant() { 1 } else { effect.ticks_left }));
    body.push(if renewed { effect.flags() & !0x08 } else { effect.flags() });

    write_body(stream, body).await?;

    match omen {
        Some(sound) if !renewed => send_sound_at(stream, sound, SOUND_PLAYERS, (player.x, player.y, player.z), 1.0, 1.0).await,
        _ => Ok(()),
    }
}

/// Модификаторы атрибутов от действующих эффектов (сила, слабость, скорость,
/// замедление): ставятся по уровню, снятые эффекты убираются.
pub(super) fn apply_effect_modifiers(player: &mut PlayerState) {
    use crate::attributes::{MAX_ABSORPTION, MAX_HEALTH, SAFE_FALL_DISTANCE};

    for kind in survival::EffectKind::ALL {
        let Some((attribute, id, per_level, operation)) = kind.modifier() else { continue };

        match player.vit.effects.iter().find(|effect| effect.kind == kind) {
            // Множитель уровня — в двойной точности (чёрный ящик, elder-fatigue:
            // «Утомление» III — −0,30000000447 = 3·(−0,1f), а не −0,3000000119).
            Some(effect) => player.attrs.set_modifier(attribute, Modifier::new(id, f64::from(per_level as f32) * f64::from(effect.amplifier + 1), operation)),
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

/// Вики, Turtle Shell: «immediately recharges after exposure to air».
/// Только игрок и только надетый шлем; под водой срок идёт обычно [проверить].
fn turtle_shell_effect(player: &PlayerState, eyes_in_water: bool) -> Option<survival::Effect> {
    (!eyes_in_water && !player.vit.dead && player.inventory.slot(5).is_some_and(|stack| blocks::item_name(stack.item) == Some("turtle_helmet")))
        .then_some(survival::Effect { kind: survival::EffectKind::WaterBreathing, amplifier: 0, ticks_left: 200, ambient: false, visible: false, show_icon: true })
}

/// Снять все эффекты (или один).
pub(super) async fn clear_effects(stream: &mut Outgoing<'_>, player: &mut PlayerState, only: Option<survival::EffectKind>) -> io::Result<()> {
    let cleared = player.vit.remove_effects(only);

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
    fn equipment_attribute_ticks_follow_263_cycle_and_keep_2612_delays() {
        use crate::gameplay_catalog::{Catalog, Profile};
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let mut player=PlayerState::new_for_catalog([74;16],"AttrCycle".into(),None,None,0x7f,74,(0.5,64.0,0.5),Catalog::new(profile));
            let sword=blocks::item_named("wooden_sword").unwrap();
            let mut due=Vec::new();
            for (damage,tick) in [15,56,98,139].into_iter().enumerate() {
                let data=crate::stackdata::Data {damage:damage as i32+1,..Default::default()};
                player.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(sword,1).with_extras(data)));
                player.attrs_touched.clear();update_equipment(&mut player,tick);
                due.push(player.attrs_touched.iter().find(|(id,_)| *id==crate::attributes::ATTACK_SPEED).unwrap().1);
            }
            assert_eq!(due,if profile==Profile::Preview26_3 {vec![16,58,100,140]} else {vec![17,58,100,141]});
        }
    }

    #[test]
    fn repeated_equipment_wear_waits_for_the_last_change() {
        use crate::gameplay_catalog::{Catalog,Profile};
        for profile in [Profile::Java26_1_2,Profile::Preview26_3] {
            let mut player=PlayerState::new_for_catalog([75;16],"AttrMerge".into(),None,None,0x7f,75,(0.5,64.0,0.5),Catalog::new(profile));
            let sword=blocks::item_named("wooden_sword").unwrap();
            for tick in [20,21] {
                let data=crate::stackdata::Data {damage:tick as i32,..Default::default()};
                player.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(sword,1).with_extras(data)));
                update_equipment(&mut player,tick);
            }
            assert_eq!(player.attrs_touched.iter().filter(|(id,_)| *id==crate::attributes::ATTACK_SPEED).map(|(_,due)|*due).max(),Some(23));
        }
    }

    #[test]
    fn game_modes_update_reach_without_changing_its_base() {
        use crate::attributes::{BLOCK_INTERACTION_RANGE, ENTITY_INTERACTION_RANGE};
        let mut player = PlayerState::new([94; 16], "Range".into(), None, None, 0x7f, 94, (0.5, 64.0, 0.5));
        player.attrs.set_base(ENTITY_INTERACTION_RANGE, 4.0);
        for mode in [GAME_MODE_SURVIVAL, GAME_MODE_CREATIVE, GAME_MODE_CREATIVE, GAME_MODE_SPECTATOR, GAME_MODE_ADVENTURE] {
            change_game_mode(&mut player, mode);
            assert_eq!(player.attrs.value(BLOCK_INTERACTION_RANGE), if mode == GAME_MODE_CREATIVE { 5.0 } else { 4.5 });
            assert_eq!(player.attrs.value(ENTITY_INTERACTION_RANGE), if mode == GAME_MODE_CREATIVE { 6.0 } else { 4.0 });
        }
        change_game_mode(&mut player, GAME_MODE_CREATIVE);
        respawned(&mut player);
        assert_eq!(player.attrs.value(BLOCK_INTERACTION_RANGE), 5.0);
        assert_eq!(player.attrs.value(ENTITY_INTERACTION_RANGE), 5.0);
    }

    #[test]
    fn smash_particle_counts_follow_pre_landing_descent() {
        // pvp-mace-landing-count: дискретный пролёт до касания, не высота /tp.
        let mut speed = 0.0f64;
        let mut before = 0.0f64;
        for step in 1..=12 {
            before -= speed;
            speed = (speed - 0.08) * 0.98;
            let expected = match step { 8 => Some(105), 10 => Some(167), 11 | 12 => Some(200), _ => None };
            if let Some(expected) = expected { assert_eq!(smash_landing_particles(before), expected); }
        }
        assert_eq!(smash_landing_particles(0.0), 0);
        assert_eq!(smash_landing_particles(3.999), 199);
        assert_eq!(smash_landing_particles(4.0), 200);
    }

    #[test]
    fn honey_drinks_at_full_food_for_forty_ticks() {
        let mut player=PlayerState::new([8;16],"Honey".into(),None,None,0x7F,1,(0.5,64.0,0.5));
        change_game_mode(&mut player,GAME_MODE_SURVIVAL);
        player.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("honey_bottle").unwrap(),2)));
        assert!(start_eating(&mut player, 0));
        let drink=player.net.eating.as_ref().unwrap();
        assert_eq!(drink.ticks_left,40);assert_eq!(drink.drink,Some(Drink::Honey));
        assert_eq!(Drink::Honey.remainder(),"glass_bottle");
    }

    #[test]
    fn command_damage_packet_preserves_direct_cause_and_optional_position() {
        let body = damage_packet(10, Source::PlayerAttack, 20, 30, Some((1.25, 64.0, -2.5)));
        let mut cursor = Cursor::new(&body);
        assert_eq!(cursor.varint(), Some(DAMAGE_EVENT));
        for value in [10, Source::PlayerAttack.type_id(), 31, 21] { assert_eq!(cursor.varint(), Some(value)); }
        assert_eq!(cursor.u8(), Some(1));
        for coordinate in [1.25, 64.0, -2.5] {
            assert_eq!(f64::from_be_bytes(cursor.bytes(8).unwrap().try_into().unwrap()), coordinate);
        }
        assert_eq!(cursor.u8(), None);
        let body = damage_packet(10, Source::Generic, 0, 0, None);
        assert_eq!(body.last(), Some(&0));
    }

    #[test]
    fn powder_snow_freezes_thaws_and_leather_protects() {
        let mut frozen = 0;
        for _ in 0..139 { frozen = freezing_step(frozen, true, false); }
        assert_eq!(frozen, 139);
        assert_eq!(freezing_step(frozen, true, false), 140);
        assert_eq!(freezing_step(140, true, false), 140);
        assert_eq!(freezing_step(140, true, true), 138);
        assert_eq!(freezing_step(140, false, false), 138);
        assert_eq!(freezing_step(1, false, false), 0);
        assert_eq!(Source::Freeze.type_id(), 17);
        assert!(crate::combat::bypasses_armor(Source::Freeze));
        assert_eq!(death_text(Source::Freeze, "Anya"), "Anya froze to death");
    }

    #[test]
    fn powder_snow_contacts_edges_but_not_boots_on_top() {
        let mut w = World::in_memory();
        w.set_block_silently(0, 10, 0, blocks::state_by_name("powder_snow").unwrap());
        assert!(powder_contact(&w, 1.2, 10.0, 0.5, 1.8));
        assert!(!powder_contact(&w, 1.4, 10.0, 0.5, 1.8));
        assert!(!powder_contact(&w, 0.5, 11.0, 0.5, 1.8));
        assert!(!look_around(&w, 0.5, 9.0, 0.5, false, false).in_wall);
        w.set_block_silently(0, 9, 0, blocks::state_by_name("slime_block").unwrap());
        assert_eq!(look_around(&w, 0.5, 10.0, 0.5, true, false).landing, 0.0);
        assert_eq!(look_around(&w, 0.5, 10.0, 0.5, false, false).landing, 0.0);
    }

    #[test]
    fn protection_side_effects_stack_and_end_when_armor_is_removed() {
        let mut player = archer(GAME_MODE_SURVIVAL, false);
        let mut data = crate::stackdata::Data::default();
        for name in ["fire_protection", "blast_protection"] {
            data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), 4);
        }
        player.inventory.set(6, Some(Stack::new(blocks::item_named("diamond_chestplate").unwrap(), 1).with_extras(data.clone())));
        update_equipment(&mut player, 0);
        ignite(&mut player, 80);
        assert_eq!(player.vit.fire, 32);
        let push = protected_blast_push(&player, (1.0, 2.0, -1.0));
        assert!((push.0 - 0.4).abs() < 1e-6 && (push.1 - 0.8).abs() < 1e-6 && (push.2 + 0.4).abs() < 1e-6);
        player.inventory.set(7, Some(Stack::new(blocks::item_named("diamond_leggings").unwrap(), 1).with_extras(data)));
        update_equipment(&mut player, 1);
        ignite(&mut player, 160);
        assert_eq!(player.vit.fire, 32, "новые чары не тушат уже идущий огонь");
        player.vit.extinguish();
        ignite(&mut player, 160);
        assert_eq!(player.vit.fire, 0);
        assert_eq!(protected_blast_push(&player, (1.0, 2.0, -1.0)), (0.0, 0.0, 0.0));
        player.inventory.set(6, None);
        player.inventory.set(7, None);
        update_equipment(&mut player, 2);
        ignite(&mut player, 80);
        assert_eq!(player.vit.fire, 80);
        assert_eq!(protected_blast_push(&player, (1.0, 2.0, -1.0)), (1.0, 2.0, -1.0));
    }

    #[test]
    fn foods_follow_the_wiki_table() {
        let bread = food_of("bread").expect("хлеб — еда");

        assert_eq!((bread.nutrition, bread.saturation, bread.ticks, bread.always), (5, 6.0, 32, false));
        assert_eq!(food_of("dried_kelp").expect("ламинария").ticks, 16);
        assert!(food_of("golden_apple").expect("яблоко").always);
        assert!(food_of("stone").is_none());
        assert!(food_of("suspicious_stew").unwrap().always);
        assert_eq!(food_of("poisonous_potato").unwrap().nutrition, 2);
        assert_eq!(food_of("pufferfish").unwrap().nutrition, 1);
    }

    #[test]
    fn honey_and_stew_are_usable_at_full_hunger_in_either_hand() {
        for hand in [0, 1] {
            for (name, ticks) in [("honey_bottle", 40), ("suspicious_stew", 32), ("milk_bucket", 32)] {
                let mut player = archer(GAME_MODE_SURVIVAL, false);
                let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
                player.inventory.set(slot, Some(Stack::new(blocks::item_named(name).unwrap(), 1)));
                assert!(start_eating(&mut player, hand), "{name}, {hand}");
                assert_eq!(player.net.eating.as_ref().unwrap().ticks_left, ticks);
                assert_eq!(living_flags(&player), if hand == 1 { 3 } else { 1 });
                assert!(eating_matches(&player, player.net.eating.as_ref().unwrap()));
                player.inventory.set(slot, None);
                assert!(!eating_matches(&player, player.net.eating.as_ref().unwrap()));
            }
        }
    }

    #[test]
    fn containers_return_to_the_used_hand_inventory_or_overflow() {
        let mut player = archer(GAME_MODE_SURVIVAL, false);
        let honey = blocks::item_named("honey_bottle").unwrap();
        let glass = blocks::item_named("glass_bottle").unwrap();
        let slot = inventory::OFFHAND;
        player.inventory.set(slot, Some(Stack::new(honey, 2)));
        assert!(consume_stack(&mut player, slot, "glass_bottle").is_none());
        assert_eq!(player.inventory.slot(slot), Some(Stack::new(honey, 1)));
        assert!(player.inventory.slots().contains(&Some(Stack::new(glass, 1))));
        assert!(consume_stack(&mut player, slot, "glass_bottle").is_none());
        assert_eq!(player.inventory.slot(slot), Some(Stack::new(glass, 1)));
        let stone = blocks::item_named("stone").unwrap();
        for filled in inventory::FIRST_MAIN..=inventory::LAST_HOTBAR { player.inventory.set(filled, Some(Stack::new(stone, 64))); }
        player.inventory.set(slot, Some(Stack::new(honey, 2)));
        assert_eq!(consume_stack(&mut player, slot, "glass_bottle"), Some(Stack::new(glass, 1)));
        player.game_mode = GAME_MODE_CREATIVE;
        assert!(consume_stack(&mut player, slot, "glass_bottle").is_none());
        assert_eq!(player.inventory.slot(slot), Some(Stack::new(honey, 1)));
    }

    #[test]
    fn changed_selected_slot_cannot_finish_an_old_meal() {
        let mut player = archer(GAME_MODE_SURVIVAL, false);
        player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("golden_apple").unwrap(), 1)));
        assert!(start_eating(&mut player, 0));
        player.inventory.select(1);
        assert!(!eating_matches(&player, player.net.eating.as_ref().unwrap()));
    }

    #[test]
    fn eating_accepts_more_of_the_same_item_but_not_a_changed_component() {
        let mut player = archer(GAME_MODE_SURVIVAL, false);
        let slot = player.inventory.selected_slot();
        let apple = Stack::new(blocks::item_named("golden_apple").unwrap(), 1);
        player.inventory.set(slot, Some(apple));
        assert!(start_eating(&mut player, 0));
        player.inventory.set(slot, Some(apple.with_count(2)));
        assert!(eating_matches(&player, player.net.eating.as_ref().unwrap()));
        player.inventory.set(slot, Some(apple.with_extras(crate::stackdata::Data { damage: 1, ..Default::default() })));
        assert!(!eating_matches(&player, player.net.eating.as_ref().unwrap()));
    }

    #[test]
    fn creative_can_consume_food_and_drinks_without_losing_them() {
        for name in ["bread", "golden_apple", "honey_bottle", "potion", "milk_bucket"] {
            let mut player = archer(GAME_MODE_CREATIVE, false);
            let slot = player.inventory.selected_slot();
            let stack = Stack::new(blocks::item_named(name).unwrap(), 1);
            player.inventory.set(slot, Some(stack));
            assert!(start_eating(&mut player, 0), "{name}");
            assert!(consume_stack(&mut player, slot, "glass_bottle").is_none());
            assert_eq!(player.inventory.slot(slot), Some(stack));
            stop_eating(&mut player);
            player.game_mode = GAME_MODE_SPECTATOR;
            assert!(!start_eating(&mut player, 0));
        }
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
    fn bubble_columns_are_water_but_supply_air() {
        for drag in ["true", "false"] {
            let mut world = World::in_memory();
            let bubble = blocks::state_from_text(&format!("bubble_column[drag={drag}]")).unwrap();
            world.set_block_silently(0, 0, 0, bubble);
            world.set_block_silently(0, 1, 0, bubble);
            let env = look_around(&world, 0.5, 0.0, 0.5, false, false);
            assert!(env.in_water && env.eyes_in_water && env.in_bubbles && env.soft);

            let water = fluids::state_of(fluids::Kind::Water, 0);
            world.set_block_silently(0, 0, 0, water);
            world.set_block_silently(0, 1, 0, water);
            let env = look_around(&world, 0.5, 0.0, 0.5, false, false);
            assert!(env.eyes_in_water && !env.in_bubbles);
        }
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

    fn bow_shared() -> Shared {
        Shared::new(
            crate::world::World::in_memory(),
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(std::path::Path::new("/tmp/mcsheriffanya-bow-ops.json")),
            crate::skins::Settings::default(),
        )
    }

    fn archer(mode: i32, arrows: bool) -> PlayerState {
        let mut player = PlayerState::new([7; 16], "Анна".to_string(), None, None, 0x7F, 1, (0.5, 64.0, 0.5));
        let bow = crate::blocks::item_named("bow").expect("лук");

        player.game_mode = mode;
        player.inventory.set(player.inventory.selected_slot(), Some(crate::inventory::Stack::new(bow, 1)));

        if arrows {
            player.inventory.set(inventory::FIRST_MAIN, Some(crate::inventory::Stack::new(crate::blocks::item_named("arrow").expect("стрела"), 5)));
        }

        player
    }

    #[test]
    fn a_bow_is_drawn_in_survival_and_in_creative_but_not_by_a_spectator() {
        let shared = bow_shared();

        for (mode, drawn) in [(GAME_MODE_SURVIVAL, true), (GAME_MODE_ADVENTURE, true), (GAME_MODE_CREATIVE, true), (GAME_MODE_SPECTATOR, false)] {
            let mut player = archer(mode, true);

            assert_eq!(start_using(&shared, &mut player, 0, 10), drawn, "режим {mode}");
        }
    }

    #[test]
    fn a_creative_bow_shoots_without_arrows_and_keeps_the_stack() {
        let shared = bow_shared();
        let mut player = archer(GAME_MODE_CREATIVE, false);

        assert!(start_using(&shared, &mut player, 0, 10));
        shared.ticks.send_replace(30);
        release_use(&mut player, &shared);

        assert!(player.net.using.is_none());
        assert!(player.inventory.held().is_some());
    }

    #[test]
    fn a_survival_bow_spends_one_arrow() {
        let shared = bow_shared();
        let mut player = archer(GAME_MODE_SURVIVAL, true);

        assert!(start_using(&shared, &mut player, 0, 10));
        shared.ticks.send_replace(30);
        release_use(&mut player, &shared);

        assert_eq!(player.inventory.slot(inventory::FIRST_MAIN).map(|stack| stack.count), Some(4));
    }

    #[test]
    fn death_text_names_the_player() {
        assert_eq!(death_text(Source::Fall, "Anya"), "Anya fell from a high place");
        assert_eq!(death_text(Source::Lava, "Anya"), "Anya tried to swim in lava");
    }
}

#[cfg(test)]
mod itemaudit2_tests {
    use super::*;
    #[test]
    fn player_projectiles_ignore_difficulty_while_mob_projectiles_scale() {
        for source in [Source::Arrow, Source::WindCharge] {
            for difficulty in 0..=3 {
                assert_eq!(projectile_difficulty(8.0, source, difficulty, true), 8.0);
                assert_eq!(projectile_difficulty(8.0, source, difficulty, false), survival::scaled(8.0, difficulty));
            }
        }
    }

    #[test]
    fn turtle_shell_refreshes_water_breathing_only_on_head_in_air() {
        let mut player = PlayerState::new([51; 16], "ItemAudit2".into(), None, None, 0x7F, 51, (0.0, 64.0, 0.0));
        let shell = Stack::new(blocks::item_named("turtle_helmet").unwrap(), 1);
        player.inventory.set(inventory::OFFHAND, Some(shell));
        assert!(turtle_shell_effect(&player, false).is_none());
        player.inventory.set(5, Some(shell));
        let effect = turtle_shell_effect(&player, false).unwrap();
        assert_eq!((effect.kind, effect.ticks_left), (survival::EffectKind::WaterBreathing, 200));
        player.vit.give_effect_shown(effect);
        assert!(turtle_shell_effect(&player, true).is_none());
        for _ in 0..200 { player.vit.effects_tick(); }
        assert!(!player.vit.has_effect(survival::EffectKind::WaterBreathing));
        assert!(turtle_shell_effect(&player, false).is_some());
    }
}

#[cfg(test)]
mod totem_itemaudit_tests {
    use super::*;
    #[test]
    fn totem_consumption_prefers_main_hand_and_preserves_offhand_until_needed() {
        let mut player = PlayerState::new([52; 16], "ItemAudit2".into(), None, None, 0x7F, 52, (0.0, 64.0, 0.0));
        let totem = Stack::new(blocks::item_named("totem_of_undying").unwrap(), 1);
        player.inventory.set(player.inventory.selected_slot(), Some(totem));
        player.inventory.set(inventory::OFFHAND, Some(totem));
        assert!(take_totem(&mut player));
        assert!(player.inventory.held().is_none());
        assert_eq!(player.inventory.slot(inventory::OFFHAND), Some(totem));
        assert!(take_totem(&mut player));
        assert!(player.inventory.slot(inventory::OFFHAND).is_none());
        assert!(!take_totem(&mut player));
        let mut vit = survival::Vitals::new();
        vit.give_effect(survival::EffectKind::FireResistance, 0, 800);
        assert!(vit.hurt(100.0, Source::Lava, false).is_none());
    }
}

#[cfg(test)]
mod itemaudit3_tests {
    use super::*;
    fn shared() -> Shared {
        Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/itemaudit3/unused-ops.json")),crate::skins::Settings::default())
    }
    fn player(name: &str, hand: i32) -> PlayerState {
        let mut p=PlayerState::new([93;16],"ItemAudit3".into(),None,None,0x7f,93,(0.5,1.0,0.5));
        p.game_mode=GAME_MODE_SURVIVAL;
        let slot=if hand==1 {inventory::OFFHAND} else {p.inventory.selected_slot()};
        p.inventory.set(slot,Some(Stack::new(blocks::item_named(name).unwrap(),3)));p
    }
    #[test]
    fn spyglass_uses_either_hand_and_stops_without_wear() {
        let shared=shared();
        for hand in [0,1] {
            let mut p=player("spyglass",hand);
            assert!(start_using(&shared,&mut p,hand,0));
            assert_eq!(p.net.using.unwrap().kind,UseKind::Spyglass);
            assert_eq!(p.net.using.unwrap().hand,hand);
            assert!(!start_using(&shared,&mut p,hand,1));
            release_use(&mut p,&shared);assert!(p.net.using.is_none());
            let slot=if hand==1 {inventory::OFFHAND} else {p.inventory.selected_slot()};
            assert_eq!(p.inventory.slot(slot).unwrap().count,1);
            assert_eq!(p.inventory.slot(slot).unwrap().extras().damage,0);
            p.game_mode=GAME_MODE_SPECTATOR;assert!(!start_using(&shared,&mut p,hand,0));
        }
    }
    #[test]
    fn chorus_is_food_at_full_hunger_and_cooldown_does_not_block_other_food() {
        for hand in [0,1] {
            let mut p=player("chorus_fruit",hand);
            assert!(start_eating_at(&mut p,hand,0));
            let e=p.net.eating.as_ref().unwrap();assert_eq!((e.nutrition,e.saturation,e.ticks_left),(4,2.4,32));
            stop_eating(&mut p);p.item_cooldowns.insert("chorus_fruit",20);
            assert!(!start_eating_at(&mut p,hand,19));
            assert!(start_eating_at(&mut p,hand,20));stop_eating(&mut p);
            let slot=if hand==1 {inventory::OFFHAND} else {p.inventory.selected_slot()};
            p.inventory.set(slot,Some(Stack::new(blocks::item_named("golden_apple").unwrap(),1)));
            assert!(start_eating_at(&mut p,hand,19));
        }
    }
    async fn eat(name: &str, hand: i32, creative: bool) -> PlayerState {
        // Пакетный буфер в памяти, без сокетов.
        let mut out = Outgoing::capture();
        let shared=shared();let mut p=player(name,hand);
        if creative {p.game_mode=GAME_MODE_CREATIVE;}
        assert!(start_eating(&mut p,hand));
        for _ in 0..32 {effects_and_eating_tick(&mut out,&shared,&mut p,2).await.unwrap();}
        assert!(p.net.eating.is_none());
        let slot=if hand==1 {inventory::OFFHAND} else {p.inventory.selected_slot()};
        assert_eq!(p.inventory.slot(slot).unwrap().count,if creative {3} else {2});
        p
    }
    #[tokio::test]
    async fn golden_apple_finishes_in_both_hands_and_applies_exact_buffs() {
        for hand in [0,1] {for creative in [false,true] {
            let p=eat("golden_apple",hand,creative).await;
            assert_eq!(p.vit.absorption,4.0);
            assert!(p.vit.effects.iter().any(|e|e.kind==survival::EffectKind::Regeneration && e.amplifier==1 && e.ticks_left==100));
            assert!(p.vit.effects.iter().any(|e|e.kind==survival::EffectKind::Absorption && e.amplifier==0 && e.ticks_left==2400));
        }}
    }
    #[tokio::test]
    async fn enchanted_apple_applies_all_four_buffs_and_protects_from_fire() {
        for hand in [0,1] {for creative in [false,true] {
            let mut p=eat("enchanted_golden_apple",hand,creative).await;
            assert_eq!(p.vit.absorption,16.0);
            for (kind,a,t) in [(survival::EffectKind::Regeneration,1,400),(survival::EffectKind::Absorption,3,2400),(survival::EffectKind::Resistance,0,6000),(survival::EffectKind::FireResistance,0,6000)] {
                assert!(p.vit.effects.iter().any(|e|(e.kind,e.amplifier,e.ticks_left)==(kind,a,t)));
            }
            assert!(p.vit.hurt(4.0,Source::Lava,false).is_none());
        }}
    }
    fn waiting_trial(shared: &Shared) {
        let mut w=shared.world_in(Dimension::Overworld).lock().unwrap();
        w.set_block(3,1,0,blocks::state_from_text("trial_spawner[trial_spawner_state=waiting_for_players]").unwrap());
        w.with_trial(3,1,0,|data| {
            let crate::trial::Data::Spawner(s)=data else {panic!("спавнер");};
            s.normal.entity="minecraft:zombie".into();s.ominous.entity="minecraft:zombie".into();
        }).unwrap();
    }
    #[tokio::test]
    async fn bad_omen_converts_at_visible_waiting_trial_and_preserves_longer_omen() {
        let mut out = Outgoing::capture();
        let shared=shared();waiting_trial(&shared);let mut p=player("apple",0);
        p.vit.give_effect(survival::EffectKind::BadOmen,2,20);
        {let mut w=shared.world_in(p.dimension).lock().unwrap();for y in 0..=3 {w.set_block_silently(2,y,0,blocks::state_by_name("stone").unwrap());}}
        trial_omen(&mut out,&shared,&mut p,2).await.unwrap();assert!(p.vit.has_effect(survival::EffectKind::BadOmen));
        {let mut w=shared.world_in(p.dimension).lock().unwrap();for y in 0..=3 {w.set_block_silently(2,y,0,crate::world::AIR);}}
        trial_omen(&mut out,&shared,&mut p,2).await.unwrap();
        assert!(!p.vit.has_effect(survival::EffectKind::BadOmen));
        assert_eq!(p.vit.effects.iter().find(|e|e.kind==survival::EffectKind::TrialOmen).unwrap().ticks_left,54000);
        assert_eq!(crate::trial::omen_duration(1,80000),80000);
        assert_eq!(crate::trial::omen_duration(5,survival::INFINITE),survival::INFINITE);
    }
    #[tokio::test]
    async fn trial_omen_marks_waiting_spawner_without_consuming_effect() {
        let mut out = Outgoing::capture();
        let shared=shared();waiting_trial(&shared);let mut p=player("apple",0);
        p.vit.give_effect(survival::EffectKind::TrialOmen,0,300);
        trial_omen(&mut out,&shared,&mut p,2).await.unwrap();
        assert!(crate::trial::ominous_state(shared.world_in(p.dimension).lock().unwrap().get_block(3,1,0)));
        assert_eq!(p.vit.effects[0].ticks_left,300);
    }
    #[tokio::test]
    async fn ominous_bottle_drinks_then_starts_trial_at_component_level() {
        let mut out = Outgoing::capture();
        let shared=shared();waiting_trial(&shared);let mut p=player("ominous_bottle",0);
        let slot=p.inventory.selected_slot();
        p.inventory.set(slot,Some(p.inventory.held().unwrap().with_extras(crate::stackdata::Data {ominous:Some(2),..Default::default()})));
        assert!(start_eating(&mut p,0));
        for _ in 0..32 {effects_and_eating_tick(&mut out,&shared,&mut p,2).await.unwrap();}
        assert_eq!(p.vit.effect_level(survival::EffectKind::BadOmen),3);
        effects_and_eating_tick(&mut out,&shared,&mut p,2).await.unwrap();
        assert!(!p.vit.has_effect(survival::EffectKind::BadOmen));
        assert_eq!(p.vit.effects.iter().find(|e|e.kind==survival::EffectKind::TrialOmen).unwrap().ticks_left,54000);
        assert_eq!(p.inventory.held().unwrap().count,2);
    }
    #[tokio::test]
    async fn feather_falling_reduces_pearl_cost_with_protection_cap() {
        let mut out = Outgoing::capture();
        let shared=shared();
        for (feather,protection,expected) in [(0,0,5.0),(4,0,2.6),(4,4,1.8)] {
            let mut p=player("ender_pearl",0);let mut data=crate::stackdata::Data::default();
            data.set_enchant(crate::stackdata::enchantment_number("feather_falling").unwrap(),feather);
            data.set_enchant(crate::stackdata::enchantment_number("protection").unwrap(),protection);
            p.inventory.set(8,Some(Stack::new(blocks::item_named("diamond_boots").unwrap(),1).with_extras(data)));
            hit(&mut out,&shared,&mut p,5.0,Source::Fall,0).await.unwrap();
            assert!((p.vit.health-(20.0-expected)).abs()<0.001);
        }
    }

}

#[cfg(test)]
#[path = "effect_audit_tests.rs"]
mod itemaudit4_effect_tests;

// Среда для второго представления игрока: те же измерения контакта и
// функции Vitals. Java entity_tick выше остаётся без изменений.
pub(super) fn bedrock_frozen(player: &PlayerState) -> i32 { player.net.frozen }

pub(super) fn bedrock_environment(shared: &Shared, player: &mut PlayerState) -> Vec<(f32, Source)> {
    let mut hits = Vec::new();
    if player.vit.dead { return hits; }
    if !mortal(player.game_mode) { player.net.frozen = 0; return hits; }
    let env = {
        let world = shared.world_in(player.dimension).lock().expect("мир");
        look_around(&world, player.x, player.y, player.z, player.sneaking, player.net.spin_pose)
    };
    player.net.in_water = env.in_water;
    player.net.in_lava = env.in_lava;
    let leather = crate::combat::ARMOR_SLOTS.iter().any(|(slot, _)| player.inventory.slot(*slot)
        .and_then(|s| blocks::item_name(s.item)).is_some_and(|n| matches!(n,
            "leather_helmet" | "leather_chestplate" | "leather_leggings" | "leather_boots")));
    player.net.frozen = freezing_step(player.net.frozen, env.powder_snow, leather);
    if player.net.frozen == 140 && (player.vit.age + 1).is_multiple_of(40)
        && shared.freeze_damage.load(std::sync::atomic::Ordering::Relaxed) { hits.push((1.0, Source::Freeze)); }
    if env.in_rain || env.in_water || env.powder_snow { player.vit.extinguish(); }
    if player.vit.fire_tick(env.in_lava) { hits.push((1.0, Source::OnFire)); }
    if env.in_fire > 0.0 { ignite(player, 160); hits.push((env.in_fire, Source::InFire)); }
    if env.campfire > 0.0 && !protected_from_hot_blocks(player) {
        hits.push((env.campfire, Source::Campfire));
    }
    if magma_contact(player, &env) { hits.push((1.0, Source::HotFloor)); }
    if env.cactus { hits.push((1.0, Source::Cactus)); }
    if env.in_wall { hits.push((1.0, Source::InWall)); }
    if env.in_lava { ignite(player, survival::LAVA_FIRE_TICKS); player.vit.fall *= 0.5; hits.push((4.0, Source::Lava)); }
    if let Some(amount) = crate::commands_world::border_of(shared, player.dimension).damage_at(player.x, player.z) { hits.push((amount, Source::OutsideBorder)); }
    if let Some(effect) = turtle_shell_effect(player, env.eyes_in_water) { player.vit.give_effect_shown(effect); }
    observe_oxygen(shared, player, env.eyes_in_water && !env.in_bubbles);
    let oxygen = player.attrs.value(crate::attributes::OXYGEN_BONUS);
    let roll = if oxygen > 0.0 && env.eyes_in_water && !env.in_bubbles { world_random(shared, player.dimension) } else { 0.0 };
    if player.vit.air_tick_with_bonus(env.eyes_in_water && !env.in_bubbles, false, oxygen, roll) { hits.push((2.0, Source::Drown)); }
    hits
}

#[cfg(test)]
mod pearl_wall_tests {
    use super::*;

    #[test]
    fn a_pearl_can_leave_the_eye_centre_outside_but_the_head_inside_a_wall() {
        // tmp/bb/throw2-ender_pearl-up45-yaw0-standing.md:31,42:
        // (3.562,13.358,39.836), Damage Event type=22 через 10 тактов.
        let mut world = World::in_memory();
        world.set_block_silently(3, 14, 40, blocks::state_by_name("stone").unwrap());
        assert!(look_around(&world, 3.562, 13.358, 39.836, false, false).in_wall);
        for z in [39.7, 39.74, 39.76] {
            assert!(!look_around(&world, 3.562, 13.358, z, false, false).in_wall, "z={z}");
        }
        for z in [39.77, 39.8, 39.836, 39.9] {
            assert!(look_around(&world, 3.562, 13.358, z, false, false).in_wall, "z={z}");
        }
        assert!(!look_around(&world, 3.562, 12.0, 39.836, false, false).in_wall);
        world.set_block_silently(3, 14, 40, world::AIR);
        assert!(!look_around(&world, 3.562, 13.358, 39.836, false, false).in_wall);
    }
}

#[cfg(test)]
mod enchanted_mob_arrow_tests {
    use super::*;

    #[tokio::test]
    async fn piercing_mob_arrow_bypasses_a_raised_shield() {
        for piercing in [false, true] {
            let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/enchant-mob-piercing/ops.json")), crate::skins::Settings::default());
            let mut player = PlayerState::new([75; 16], "Shield".into(), None, None, 0, 75, (0.5, 64.0, 0.5));
            player.game_mode = GAME_MODE_SURVIVAL; player.on_ground = true; player.yaw = 0.0;
            player.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("shield").unwrap(), 1)));
            player.net.using = Some(Using { kind: UseKind::Shield, hand: 1, since: 0 });
            shared.ticks.send_replace(10);
            let mut out = Outgoing::capture();
            mob_hit(&mut out, &shared, &mut player, 4.0, Source::Arrow, 42, "entity.minecraft.pillager", Some((0.0, -1.0)),
                (0.5, 64.0, 2.5), crate::world::ProjectileEnchantments { ignores_shield: piercing, ..Default::default() }).await.unwrap();
            assert_eq!(player.net.mob_hit_landed, piercing);
            assert_eq!(player.vit.health < 20.0, piercing);
            assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().extras().damage == 0, piercing);
        }
    }

    #[tokio::test]
    async fn mob_thorns_use_the_measured_unattenuated_velocity() {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/enchant-mob-thorns/ops.json")), crate::skins::Settings::default());
        let mut player = PlayerState::new([74; 16], "Thorns".into(), None, None, 0, 74, (0.5, 64.0, 0.5));
        player.game_mode = GAME_MODE_SURVIVAL;
        player.on_ground = true;
        player.yaw = 180.0;
        let mut out = Outgoing::capture();
        mob_hit(&mut out, &shared, &mut player, 2.0, Source::Thorns, 42, "entity.minecraft.zombie", Some((0.0, -1.0)),
            (0.5, 64.0, 2.5), Default::default()).await.unwrap();
        let velocity = player.net.velocity.unwrap();
        assert!(velocity.0.abs() < 1e-9);
        // Стоящий игрок имеет скорость -0.08 * 0.98; толчок делит её
        // пополам и добавляет 0.4. Запись blackbox округлена до 0.361.
        let expected_y = (-0.08 * 0.98) / 2.0 + 0.4;
        assert!((velocity.1 - expected_y).abs() < 1e-9);
        assert_eq!((velocity.1 * 1000.0).round() / 1000.0, 0.361);
        assert!((velocity.2 + 0.4).abs() < 1e-9);
        let mut bytes = out.queued();
        let mut animations = Vec::new();
        while !bytes.is_empty() {
            let packet = read_packet(&mut bytes).await.unwrap();
            if packet.id == HURT_ANIMATION { animations.push(packet.body); }
        }
        let mut expected = encode_varint(HURT_ANIMATION);
        expected.extend(encode_varint(player.entity_id));
        push_f32(&mut expected, 270.0);
        assert_eq!(animations, vec![expected], "отражение моба посылает Hurt Animation ровно один раз");
    }

    #[tokio::test]
    async fn mob_arrow_flame_and_punch_require_an_accepted_hit() {
        for blocked in [false, true] {
            let mut velocities = Vec::new();
            for level in [0, 2] {
                let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                    crate::ops::Ops::open(std::path::Path::new("tmp/enchant-mob-arrow/ops.json")), crate::skins::Settings::default());
                let mut player = PlayerState::new([71; 16], "Лучник".into(), None, None, 0x7f, 71, (0.5, 64.0, 0.5));
                player.game_mode = GAME_MODE_SURVIVAL;
                player.on_ground = true;
                player.yaw = 0.0;
                if blocked {
                    player.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("shield").unwrap(), 1)));
                    player.net.using = Some(Using { kind: UseKind::Shield, hand: 1, since: 0 });
                    shared.ticks.send_replace(10);
                }
                let mut out = Outgoing::capture();
                let enchants = crate::world::ProjectileEnchantments { punch: level, fire_ticks: 100, ..Default::default() };
                mob_hit(&mut out, &shared, &mut player, 4.0, Source::Arrow, 42, "entity.minecraft.skeleton", Some((0.0, -1.0)), (0.5, 64.0, 2.5), enchants).await.unwrap();
                assert_eq!(player.net.mob_hit_landed, !blocked);
                assert_eq!(player.vit.fire > 0, !blocked);
                if blocked { assert!(player.net.velocity.is_none()); }
                else { velocities.push(player.net.velocity.unwrap()); }
            }
            if !blocked {
                let (plain, punch) = (velocities[0], velocities[1]);
                assert!((punch.2 - plain.2 + 1.2 * 0.546).abs() < 1e-6);
                assert!((punch.1 - plain.1 - 0.098).abs() < 1e-6);
            }
        }
    }
}

#[cfg(test)]
mod hot_block_enchantment_tests {
    use super::*;

    #[tokio::test]
    async fn frost_immunity_applies_to_direct_damage_without_wear_or_hurt() {
        let (shared, mut player) = scene();
        boots(&mut player, 1);
        let before = player.inventory.slot(8);
        let mut stream = Outgoing::capture();
        for source in [Source::HotFloor, Source::Campfire] {
            assert!(hit(&mut stream, &shared, &mut player, 20.0, source, 0).await.unwrap().is_none());
            assert_eq!(player.vit.health, 20.0);
            assert_eq!(player.inventory.slot(8), before);
            assert_eq!(player.vit.invulnerable, 0);
        }
        assert!(hit(&mut stream, &shared, &mut player, 2.0, Source::Lava, 0).await.unwrap().is_some());
        assert!(player.vit.health < 20.0);
    }

    #[tokio::test]
    async fn frost_command_damage_rejects_before_armor_and_shield_wear() {
        for shield in [false, true] {
            let (shared, mut player) = scene();
            boots(&mut player, 1);
            if shield {
                player.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("shield").unwrap(), 1)));
                player.net.using = Some(Using { kind: UseKind::Shield, hand: 1, since: 0 });
                player.yaw = 0.0;
                shared.ticks.send_replace(10);
            }
            let before = (player.inventory.slot(8), player.inventory.slot(inventory::OFFHAND));
            let mut stream = Outgoing::capture();
            let origin = crate::combat::DamageOrigin { at: shield.then_some((0.5, 64.0, 2.5)), ..Default::default() };
            for source in [Source::HotFloor, Source::Campfire] {
                command_hit(&mut stream, &shared, &mut player, 2.0, source, origin.clone()).await.unwrap();
                assert_eq!(player.vit.health, 20.0);
                assert_eq!(player.vit.invulnerable, 0);
                assert_eq!((player.inventory.slot(8), player.inventory.slot(inventory::OFFHAND)), before);
            }
            // Тот же обработчик принимает лаву и расходует ровно1 прочности.
            player.net.using = None;
            command_hit(&mut stream, &shared, &mut player, 2.0, Source::Lava, Default::default()).await.unwrap();
            assert!((player.vit.health - 18.176).abs() < 0.0001);
            assert_eq!(player.inventory.slot(8).unwrap().extras().damage, 1);
        }
    }

    fn scene() -> (Shared, PlayerState) {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/frost-hot-blocks/ops.json")), crate::skins::Settings::default());
        let mut player = PlayerState::new([72; 16], "Ледоход".into(), None, None, 0x7f, 72, (0.5, 64.0, 0.5));
        player.game_mode = GAME_MODE_SURVIVAL;
        player.on_ground = true;
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(0, 63, 0, blocks::state_by_name("magma_block").unwrap());
        (shared, player)
    }

    fn boots(player: &mut PlayerState, level: i32) {
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("frost_walker").unwrap(), level);
        player.inventory.set(8, Some(Stack::new(blocks::item_named("diamond_boots").unwrap(), 1).with_extras(data)));
    }

    #[test]
    fn magma_requires_ground_contact_and_respects_frost_sneaking_and_fire_resistance() {
        let (shared, mut player) = scene();
        let has_magma = |p: &mut PlayerState| bedrock_environment(&shared, p).contains(&(1.0, Source::HotFloor));
        assert!(has_magma(&mut player));
        player.on_ground = false;
        assert!(!has_magma(&mut player));
        player.on_ground = true;
        player.sneaking = true;
        assert!(!has_magma(&mut player));
        player.sneaking = false;
        boots(&mut player, 1);
        assert!(!has_magma(&mut player));
        player.inventory.set(8, None);
        assert!(has_magma(&mut player));
        player.vit.give_effect(survival::EffectKind::FireResistance, 0, 100);
        assert!(!has_magma(&mut player));
    }

    #[test]
    fn frost_boots_protect_from_campfires_but_not_lava() {
        let (shared, mut player) = scene();
        {
            let mut world = shared.world_in(player.dimension).lock().unwrap();
            world.set_block_silently(0, 63, 0, blocks::state_by_name("stone").unwrap());
            world.set_block_silently(0, 64, 0, blocks::state_from_text("campfire[lit=true]").unwrap());
        }
        assert!(bedrock_environment(&shared, &mut player).iter().any(|(_, source)| *source == Source::Campfire));
        boots(&mut player, 2);
        assert!(!bedrock_environment(&shared, &mut player).iter().any(|(_, source)| *source == Source::Campfire));
        shared.world_in(player.dimension).lock().unwrap().set_block_silently(0, 64, 0, fluids::state_of(fluids::Kind::Lava, 0));
        assert!(bedrock_environment(&shared, &mut player).contains(&(4.0, Source::Lava)));
    }
}

#[cfg(test)]
mod combat_777_command_tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile};

    #[tokio::test]
    async fn creative_263_preserves_saturation_when_natural_regeneration_is_disabled() {
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let catalog=Catalog::new(profile);
            let shared=Shared::new(World::in_memory_with_catalog(catalog),World::in_memory_with_catalog(catalog),
                Default::default(),Default::default(),Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/combat777-creative/ops.json")),crate::skins::Settings::default());
            shared.natural_regeneration.store(false,std::sync::atomic::Ordering::Relaxed);
            let mut player=PlayerState::new_for_catalog([76;16],"Creative".into(),None,None,0,76,(0.5,64.0,0.5),catalog);
            player.game_mode=GAME_MODE_CREATIVE;
            player.vit.saturation=6.0;
            for now in 0..340 {
                shared.ticks.send_replace(now);
                tick(&mut Outgoing::capture(),&shared,&mut player).await.unwrap();
            }
            assert_eq!(player.vit.saturation,if profile==Profile::Preview26_3 {6.0} else {20.0});
        }
    }

    #[tokio::test]
    async fn creative_263_does_not_gain_saturation_on_easy() {
        let catalog = Catalog::new(Profile::Preview26_3);
        let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
            Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/combat777-creative/ops.json")), crate::skins::Settings::default());
        let mut player = PlayerState::new_for_catalog([77;16], "CreativeEasy".into(), None, None, 0, 77, (0.5,64.0,0.5), catalog);
        player.game_mode = GAME_MODE_CREATIVE;
        for _ in 0..340 {
            creative_tick(&mut Outgoing::capture(), &shared, &mut player, survival::EASY).await.unwrap();
        }
        assert_eq!(player.vit.saturation, 5.0);
    }

    #[tokio::test]
    async fn unattributed_magic_has_hurt_feedback_only_in_the_263_primary() {
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let catalog = Catalog::new(profile);
            let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
                Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/combat777-command/ops.json")), crate::skins::Settings::default());
            shared.difficulty.store(2, std::sync::atomic::Ordering::Relaxed);
            let mut player = PlayerState::new_for_catalog([73;16], "Magic".into(), None, None, 0, 73, (0.5,64.0,0.5), catalog);
            player.game_mode = GAME_MODE_SURVIVAL;
            player.on_ground = true;
            player.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("shield").unwrap(),1)));
            player.net.using = Some(Using { kind: UseKind::Shield, hand: 1, since: 0 });
            shared.ticks.send_replace(10);
            let mut out = Outgoing::capture();
            command_hit(&mut out, &shared, &mut player, 4.0, Source::IndirectMagic, Default::default()).await.unwrap();
            assert_eq!(player.vit.health, 16.0);
            assert_eq!(player.inventory.slot(inventory::OFFHAND).unwrap().extras().damage, 0);
            let preview = profile == Profile::Preview26_3;
            assert_eq!(player.net.velocity.is_some(), preview);
            if let Some((x,y,z)) = player.net.velocity {
                assert!((x.hypot(z)-0.4).abs()<1e-9);
                assert!((y-0.3608).abs()<1e-9);
            }
            let mut bytes = out.queued();
            let mut animations = 0;
            while !bytes.is_empty() {
                let packet = read_packet(&mut bytes).await.unwrap();
                if packet.id == HURT_ANIMATION { animations += 1; }
            }
            assert_eq!(animations, usize::from(preview));
            player.net.velocity = None;
            command_hit(&mut out, &shared, &mut player, 4.0, Source::IndirectMagic, Default::default()).await.unwrap();
            assert_eq!(player.vit.health,16.0);
            assert!(player.net.velocity.is_none(), "отклонённый повторный урон не толкает");
        }
    }
}
