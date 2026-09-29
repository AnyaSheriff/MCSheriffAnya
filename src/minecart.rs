// Вагонетки: сущность, что катится по рельсам.
//
// Физика — старая, та, что у 26.1.2 по умолчанию (новая спрятана за опытом
// «minecart_improvements» и выключена). Кода игры и чужих ядер здесь нет:
// правила взяты со страниц minecraft.wiki (Minecart, Rail, Powered Rail,
// Activator Rail, Detector Rail, Entity), а числа, которых на вики нет,
// сняты с официального сервера чёрным ящиком — зондом, который каждый такт
// спрашивает у вагонетки её точное место и скорость (`data get entity`),
// и сценариями tools/blackbox/scenarios/minecart-*.
//
// Что не сверено, помечено «[проверить]».

use crate::blocks;
use crate::inventory::Stack;

/// Вид вагонетки.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Plain,
    Chest,
    Hopper,
    Tnt,
    Furnace,
}

impl Kind {
    /// Все виды — для подсказок в командах.
    pub const ALL: [Kind; 5] = [Kind::Plain, Kind::Chest, Kind::Hopper, Kind::Tnt, Kind::Furnace];

    /// По имени сущности, как в командах: `minecart`, `chest_minecart`…
    pub fn from_name(name: &str) -> Option<Kind> {
        Some(match name.strip_prefix("minecraft:").unwrap_or(name) {
            "minecart" => Kind::Plain,
            "chest_minecart" => Kind::Chest,
            "hopper_minecart" => Kind::Hopper,
            "tnt_minecart" => Kind::Tnt,
            "furnace_minecart" => Kind::Furnace,
            _ => return None,
        })
    }

    /// Имя сущности — оно же имя предмета, которым вагонетку ставят и
    /// который из неё выпадает.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Plain => "minecart",
            Kind::Chest => "chest_minecart",
            Kind::Hopper => "hopper_minecart",
            Kind::Tnt => "tnt_minecart",
            Kind::Furnace => "furnace_minecart",
        }
    }

    /// Номер вида сущности для клиента Java (minecraft-data 26.1,
    /// entities.json).
    pub fn java_type(self) -> i32 {
        match self {
            Kind::Plain => 85,
            Kind::Chest => 25,
            Kind::Hopper => 65,
            Kind::Tnt => 133,
            Kind::Furnace => 56,
        }
    }

    /// Сколько ячеек у вагонетки-хранилища: у сундука 27, у воронки 5 (вики,
    /// Minecart with Chest, Minecart with Hopper).
    pub fn slots(self) -> usize {
        match self {
            Kind::Chest => 27,
            Kind::Hopper => 5,
            _ => 0,
        }
    }
}

/// Ящик вагонетки: 0,98 × 0,7 (вики, Minecart: «Height: 0.7 blocks, Width:
/// 0.98 blocks») — в одинарной точности: упёршаяся в стену вагонетка
/// у оригинала стоит в 0,49000000953674 от неё (зонд minecart-uphill).
pub const WIDTH: f64 = 0.98_f32 as f64;
pub const HEIGHT: f64 = 0.7_f32 as f64;

/// Тяжесть: 0,04 блока за такт за такт (вики, Entity, таблица движения).
const GRAVITY: f64 = 0.04;

/// Сопротивление вне рельс: 0,95 по всем осям, после шага (вики, Entity:
/// «Minecarts … 0.95»). На земле скорость сперва делится пополам (зонд
/// minecart-end: 0,4 → 0,2 → 0,1 …, и у лежащей на земле сопротивления
/// воздуха уже нет).
const AIR_DRAG: f64 = 0.95;
const GROUND_DRAG: f64 = 0.5;

/// Предел скорости по каждой оси: 0,4 блока за такт, 8 блоков в секунду
/// (вики, Minecart: «a predefined speed limit of exactly 8 blocks per second
/// per axis of travel»; Entity: «a maximum horizontal velocity of 0.4»). На
/// рельсах предел — только у шага, сама скорость может быть больше (зонд
/// minecart-powered: скорость дорастает до 0,555, шаг остаётся 0,4); вне
/// рельс урезается и скорость (зонд minecart-end: 0,513 → 0,4).
const MAX_SPEED: f64 = 0.4;

/// Толчок запитанной рельсы: к скорости вдоль хода добавляется 0,06 за такт,
/// после сопротивления (вики, Powered Rail: «its x velocity is increased by
/// (xspeed * 0.06 / speed)», если «speed > 0.01»; порядок — зонд
/// minecart-powered: 0,02 → 0,02·0,96 + 0,06 = 0,0792).
const BOOST: f64 = 0.06;
const BOOST_FROM: f64 = 0.01;

/// Сопротивление на рельсах у пустой вагонетки: за такт скорость умножается
/// на 0,96, после шага (зонд minecart-powered: 0,5554 → 0,5332 → 0,5119).
const RAIL_DRAG: f64 = 0.96;

/// Наклон: до шага скорость получает 0,0078125 в сторону спуска (зонд
/// minecart-slope: с места — ровно 0,0078125).
const SLOPE: f64 = 0.0078125;

/// Спуск разгоняет, подъём тормозит: после сопротивления к скорости
/// прибавляется 0,05 × (на сколько вагонетка опустилась по рельсе за такт).
/// Зонд minecart-slope: на спуске в 45° скорость за такт становится
/// (v + 0,0078125) · 1,01 = 0,96 + 0,05 от шага; на выходе со спуска —
/// 0,96·v + 0,05·0,177, где 0,177 — остаток спуска.
const DROP_GAIN: f64 = 0.05;

/// Вагонетка стоит над рельсой на 1/16 блока (зонд: на ровной рельсе высота
/// вагонетки — низ клетки + 0,0625, на наклоне — высота рельсы + 0,0625).
const RIDE: f64 = 0.0625;

/// Стоящая на запитанной рельсе вагонетка, у которой с одной стороны
/// твёрдый блок, трогается от него со скоростью 0,02 (вики, Powered Rail:
/// «accelerate a non-moving minecart away from a solid opaque block»; число
/// — зонд minecart-powered).
const START_PUSH: f64 = 0.02;

/// Незапитанная рельса с питанием тормозит (вики, Powered Rail: «usually
/// stopping it within one block's distance»): медленнее этого — стоп,
/// иначе скорость делится пополам. [проверить]: числа не сняты.
const BRAKE_STOP: f64 = 0.03;
const BRAKE: f64 = 0.5;

/// Вагонетка на рельсах перешагивает блоки, чей верх не выше низа ящика
/// больше чем на полблока: на подъёме ящик иначе упирался бы в опору
/// следующей ступени, а оригинал въезжает (зонд minecart-uphill; там
/// нужно не меньше 0,49). Стена в рост блока останавливает. [проверить]:
/// сама высота шага.
const STEP_UP: f64 = 0.5;

/// Сдвиг меньше этого (в квадрате), урезанный упором, не делается вовсе
/// (зонд minecart-end: скорость 0,000195 вагонетку на земле уже не
/// сдвигает, 0,00039 — ещё сдвигает).
const NO_STEP: f64 = 1e-7;

/// Поворот обновляется, только если за такт вагонетка сдвинулась по
/// горизонтали больше чем на корень из этого. [проверить]
const TURN_FROM: f64 = 0.001;

/// Столкновение вагонеток (зонд minecart-collide): ящик идущей по рельсам
/// вагонетки раздувается по горизонтали на 0,2; при встрече обе получают
/// среднюю из двух скоростей плюс 0,2 своей, и толчок друг от друга 0,05 на
/// расстояние между серединами (не больше 0,05). Вагонетки бок о бок —
/// смотрящие друг на друга под углом — не сталкиваются (косинус меньше
/// 0,8). [проверить]: угол и раздутие у стоящей.
const REACH: f64 = 0.2;
const KEEP_OWN: f64 = 0.2;

/// Толчок: 0,05 — но как 0,1 одинарной точности на 0,5: с точным 0,05
/// расходится с оригиналом в восьмом знаке (зонд minecart-collide).
const PUSH: f64 = 0.1_f32 as f64 * 0.5;
const HEAD_ON: f64 = 0.8;

/// Урон от удара рукой полной силы — 10 (зонд minecart-hit: удар рукой после
/// паузы даёт 10, удар через такт — 2,72, как 10·(0,2 + 0,8·0,3²): сила
/// удара растёт с перезарядкой руки в 5 тактов). Урон спадает на 1 за такт.
/// [проверить]: при каком уроне вагонетка ломается — берём больше 40.
const HIT: f32 = 10.0;
const BREAKS_OVER: f32 = 40.0;
const HAND_COOLDOWN: f32 = 5.0;

/// Сколько тактов вагонетка качается после удара (зонд: 10, по одному за
/// такт).
const HURT_TIME: i32 = 10;

/// Вагонетка с динамитом: запал от активирующей рельсы — 80 тактов, как
/// у блока (вики, Minecart with TNT: «4 seconds»); сила взрыва — 4.
pub const RAIL_FUSE: i32 = 80;
const TNT_POWER: f32 = 4.0;

/// Запал вагонетки с динамитом, подожжённой взрывом: меньше 40 тактов.
const BLAST_FUSE: i32 = 40;

/// Пролёт, после которого вагонетка с динамитом взрывается о землю.
/// [проверить]
const FALL_BLAST: f64 = 3.0;

/// Печь: уголь даёт 3600 тактов, больше 32767 не копится (вики, Minecart
/// with Furnace).
const FUEL_PER_COAL: i32 = 3600;
const FUEL_MAX: i32 = 32767;

/// Сила взрыва вагонетки с динамитом: 4 и случайная прибавка до 1,5
/// скорости (скорость — не больше 5; вики, Minecart with TNT: «a random
/// bonus value up to 1.5 times velocity, but no higher than 7.5»). `luck` —
/// случай от 0 до 1. [проверить]: как именно берётся случай.
pub fn blast_power(bonus: f64, luck: f32) -> f32 {
    TNT_POWER + luck * 1.5 * bonus.min(5.0) as f32
}

/// Печь: шаг по рельсе — не больше 0,2 по оси (вики, Minecart with
/// Furnace: «4 m/s»; зонд furnace: ровно 0,2 за такт), скорость — не больше
/// 2; при толчке от скорости остаётся 0,8. Толчок меньше этого (в квадрате)
/// не считается, стоящую печь (скорость в квадрате не больше этого) толчок
/// не поворачивает. [проверить]: оба порога.
const FURNACE_STEP: f64 = 0.2;
const FURNACE_LIMIT: f64 = 2.0;
const FURNACE_KEEP: f64 = 0.8;
const PUSH_FROM: f64 = 1e-4;
const STEER_FROM: f64 = 1e-3;

/// Топливо печи — уголь и древесный уголь (вики, Minecart with Furnace:
/// «can only be powered with coal and charcoal»).
pub fn is_furnace_fuel(item: i32) -> bool {
    matches!(blocks::item_name(item), Some("coal" | "charcoal"))
}

/// Насколько ниже мира вагонетка считается пропавшей.
const LOST_BELOW: f64 = 64.0;

/// Вагонетка в мире.
#[derive(Clone, Debug)]
pub struct Minecart {
    /// Номер сущности и опознаватель — для клиентов.
    pub entity_id: i32,
    pub uuid: [u8; 16],

    pub kind: Kind,

    /// Середина низа ящика.
    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Скорость, блоков за такт.
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,

    /// Поворот и наклон, градусы: куда смотрит вагонетка (для клиентов).
    /// `flipped` — развёрнута ли вагонетка задом наперёд.
    pub yaw: f32,
    pub pitch: f32,
    flipped: bool,

    /// Стоит ли на земле (вне рельс) и на рельсах ли.
    pub on_ground: bool,
    pub on_rail: bool,

    /// Удар: сколько тактов ещё качаться, в какую сторону (1 или −1) и
    /// сколько урона накоплено (вики, Entity: «boats and minecarts appear to
    /// recover health over time»).
    pub hurt_time: i32,
    pub hurt_dir: i32,
    pub damage: f32,
    pub hits: u32,

    /// Какие свойства удара менялись с прошлой рассылки (`DIRTY_*`):
    /// оригинал шлёт свойство, если его записали, даже если значение
    /// вернулось к прежнему (зонд minecart-hit: удар через такт снова
    /// шлёт «удар 10»).
    dirty: u8,

    /// Сколько раз её толкала другая вагонетка: после толчка клиентам сразу
    /// уходит скорость, не дожидаясь очередной рассылки (minecart-collide).
    pub impulses: u32,

    /// Содержимое вагонетки с сундуком или воронкой.
    pub slots: Vec<Option<Stack>>,

    /// Работает ли воронка: запитанная активирующая рельса её выключает,
    /// незапитанная — включает (вики, Minecart with Hopper; зонд
    /// minecart-hopper: выключается в первый же такт на рельсе).
    pub enabled: bool,

    /// Сколько раз её переносили командой: клиент, знающий меньший счёт,
    /// получает перенос, а не шаг.
    pub teleports: u32,

    /// Сколько тактов она в мире: от него считаются рассылки её движения.
    pub age: u64,

    /// Запал вагонетки с динамитом: −1 — не подожжена, иначе тактов до
    /// взрыва.
    pub fuse: i32,

    /// Сколько раз её поджигали: клиентам уходит событие «подожжена».
    pub primes: u32,

    /// Подожжена в этом такте: миру пора сыграть звук поджога.
    pub just_primed: bool,

    /// Пора взорваться: прибавка к силе взрыва (скорость или что вместо
    /// неё; такт мира делает взрыв и убирает её).
    pub blast: Option<f64>,

    /// Вагонетка с печью: сколько тактов ещё гореть и куда толкать (от
    /// игрока, заправившего её, к ней) и чем толкает в этом такте.
    pub fuel: i32,
    fed: i32,
    pub push_x: f64,
    pub push_z: f64,
    shove_x: f64,
    shove_z: f64,

    /// Сколько пролетела вниз с последней опоры.
    fall: f64,

    /// Через какую ступень ящик перешагивает на этом шаге (на рельсах —
    /// `STEP_UP`, иначе 0).
    step_up: f64,

    /// Какой она была в конце прошлого такта: оригинал рассказывает
    /// клиентам о сущностях в начале такта, то есть о прошлом такте
    /// (minecart-powered: пакет и `data get` одного такта сходятся).
    pub last: Snapshot,
}

/// Место, скорость и поворот вагонетки на конец такта.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub hurt_time: i32,
    pub hurt_dir: i32,
    pub damage: f32,

    /// Сколько раз её били: после удара клиентам уходит и скорость.
    pub hits: u32,

    /// Какие свойства удара менялись за такт (`DIRTY_*`).
    pub dirty: u8,

    /// Сколько раз её толкали.
    pub impulses: u32,

    /// Сколько раз поджигали и горит ли печь.
    pub primes: u32,
    pub burning: bool,
}

/// Биты изменившихся свойств удара.
pub const DIRTY_HURT: u8 = 1;
pub const DIRTY_DIR: u8 = 2;
pub const DIRTY_DAMAGE: u8 = 4;

/// Что на рельсе под вагонеткой.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RailKind {
    Plain,
    Powered { on: bool },
    Activator { on: bool },
    Detector,
}

/// Форма рельсы — значение свойства `shape`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    NorthSouth,
    EastWest,
    AscendingEast,
    AscendingWest,
    AscendingNorth,
    AscendingSouth,
    SouthEast,
    SouthWest,
    NorthWest,
    NorthEast,
}

impl Shape {
    fn parse(name: &str) -> Option<Shape> {
        Some(match name {
            "north_south" => Shape::NorthSouth,
            "east_west" => Shape::EastWest,
            "ascending_east" => Shape::AscendingEast,
            "ascending_west" => Shape::AscendingWest,
            "ascending_north" => Shape::AscendingNorth,
            "ascending_south" => Shape::AscendingSouth,
            "south_east" => Shape::SouthEast,
            "south_west" => Shape::SouthWest,
            "north_west" => Shape::NorthWest,
            "north_east" => Shape::NorthEast,
            _ => return None,
        })
    }

    /// Два конца рельсы: смещения от середины клетки по x и z.
    fn ends(self) -> [(f64, f64); 2] {
        match self {
            Shape::NorthSouth | Shape::AscendingNorth | Shape::AscendingSouth => [(0.0, -0.5), (0.0, 0.5)],
            Shape::EastWest | Shape::AscendingEast | Shape::AscendingWest => [(-0.5, 0.0), (0.5, 0.0)],
            Shape::SouthEast => [(0.0, 0.5), (0.5, 0.0)],
            Shape::SouthWest => [(0.0, 0.5), (-0.5, 0.0)],
            Shape::NorthWest => [(0.0, -0.5), (-0.5, 0.0)],
            Shape::NorthEast => [(0.0, -0.5), (0.5, 0.0)],
        }
    }

    /// Наклон: в какую сторону по x и z поднимается рельса.
    fn rise(self) -> Option<(f64, f64)> {
        match self {
            Shape::AscendingEast => Some((1.0, 0.0)),
            Shape::AscendingWest => Some((-1.0, 0.0)),
            Shape::AscendingNorth => Some((0.0, -1.0)),
            Shape::AscendingSouth => Some((0.0, 1.0)),
            _ => None,
        }
    }

    /// Высота рельсы над низом клетки в точке с этим смещением от середины:
    /// у наклона — от 0 до 1 вдоль подъёма, у прочих — 0.
    fn height(self, dx: f64, dz: f64) -> f64 {
        match self.rise() {
            Some((rx, rz)) => 0.5 + dx * rx + dz * rz,
            None => 0.0,
        }
    }
}

/// Заряд руки: доля от 0,1 до 1 — сколько тактов прошло с прошлого удара
/// из перезарядки в 5 тактов (зонд minecart-hit). Полный — удар «сильный».
pub fn hand_charge(since_attack: u32) -> f32 {
    ((since_attack as f32 + 0.5) / HAND_COOLDOWN).min(1.0)
}

/// Рельса в этом состоянии блока: вид и форма.
fn rail(state: i32) -> Option<(RailKind, Shape)> {
    let name = blocks::block_at_state(state)?;
    let value = |property| blocks::orientation(name).and_then(|block| block.value(state, property));
    let on = || value("powered") == Some("true");

    let kind = match name {
        "rail" => RailKind::Plain,
        "powered_rail" => RailKind::Powered { on: on() },
        "activator_rail" => RailKind::Activator { on: on() },
        "detector_rail" => RailKind::Detector,
        _ => return None,
    };

    Some((kind, Shape::parse(value("shape")?)?))
}

/// Куда встаёт вагонетка, поставленная предметом на рельсу в клетке `at`:
/// середина клетки на 1/16 выше рельсы (зонд minecart-hit: поставленная на
/// ровную рельсу появляется на y + 0,0625), над наклоном — ещё на полблока
/// выше. None — это не рельса. [проверить]: наклон.
pub fn place_on_rail(state: i32, (x, y, z): (i32, i32, i32)) -> Option<(f64, f64, f64)> {
    let (_, shape) = rail(state)?;
    let lift = if shape.rise().is_some() { 0.5 } else { 0.0 };

    Some((x as f64 + 0.5, y as f64 + RIDE + lift, z as f64 + 0.5))
}

/// Ось шага.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
}

/// Взгляд на мир для шага вагонетки: какой блок где стоит.
pub trait View {
    fn block(&self, x: i32, y: i32, z: i32) -> i32;
}

impl<F: Fn(i32, i32, i32) -> i32> View for F {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        self(x, y, z)
    }
}

/// Упирается ли вагонетка в этот блок. [проверить]: у нас всякий блок
/// с ящиком — полный куб; рельсы, трава, провод пропускают.
fn blocks_cart(state: i32) -> bool {
    blocks::has_box(state) && rail(state).is_none()
}

/// Рельса под вагонеткой в точке (x, y, z): в своей клетке или клеткой ниже
/// (вагонетка въезжает на наклон, спускающийся вниз). Возвращает клетку
/// рельсы, вид и форму.
fn rail_under<V: View>(view: &V, x: f64, y: f64, z: f64) -> Option<((i32, i32, i32), RailKind, Shape)> {
    let (bx, by, bz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);

    [by, by - 1]
        .into_iter()
        .find_map(|at_y| rail(view.block(bx, at_y, bz)).map(|(kind, shape)| ((bx, at_y, bz), kind, shape)))
}

/// Высота вагонетки на рельсе в клетке `at` в точке (x, z).
fn ride_height((bx, by, bz): (i32, i32, i32), shape: Shape, x: f64, z: f64) -> f64 {
    by as f64 + shape.height(x - (bx as f64 + 0.5), z - (bz as f64 + 0.5)) + RIDE
}

impl Minecart {
    pub fn new(entity_id: i32, kind: Kind, (x, y, z): (f64, f64, f64)) -> Self {
        let mut cart = Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            kind,
            x,
            y,
            z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            flipped: false,
            on_ground: false,
            on_rail: false,
            hurt_time: 0,
            hurt_dir: 1,
            damage: 0.0,
            hits: 0,
            dirty: 0,
            impulses: 0,
            slots: vec![None; kind.slots()],
            enabled: true,
            teleports: 0,
            age: 0,
            fuse: -1,
            primes: 0,
            just_primed: false,
            blast: None,
            fuel: 0,
            fed: 0,
            push_x: 0.0,
            push_z: 0.0,
            shove_x: 0.0,
            shove_z: 0.0,
            fall: 0.0,
            step_up: 0.0,
            last: Snapshot {
                x,
                y,
                z,
                vx: 0.0,
                vy: 0.0,
                vz: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
                hurt_time: 0,
                hurt_dir: 1,
                damage: 0.0,
                hits: 0,
                dirty: 0,
                impulses: 0,
                primes: 0,
                burning: false,
            },
        };

        cart.remember();
        cart
    }

    /// Запомнить, какой вагонетка стала к концу такта.
    fn remember(&mut self) {
        self.last = Snapshot {
            x: self.x,
            y: self.y,
            z: self.z,
            vx: self.vx,
            vy: self.vy,
            vz: self.vz,
            yaw: self.yaw,
            pitch: self.pitch,
            on_ground: self.on_ground,
            hurt_time: self.hurt_time,
            hurt_dir: self.hurt_dir,
            damage: self.damage,
            hits: self.hits,
            dirty: self.dirty,
            impulses: self.impulses,
            primes: self.primes,
            burning: self.fuel > 0,
        };

        self.dirty = 0;
    }

    /// Провалилась в пустоту.
    pub fn lost(&self) -> bool {
        self.y < crate::world::MIN_Y as f64 - LOST_BELOW
    }

    /// Что читает сравнитель через детекторную рельсу: заполненность
    /// хранилища, как у сундука; у прочих вагонеток — None (вики, Detector
    /// Rail).
    pub fn reading(&self) -> Option<u8> {
        if self.slots.is_empty() {
            return None;
        }

        Some(crate::containers::Container::signal(&self.slots) as u8)
    }

    /// Удар игрока: `since_attack` — сколько тактов прошло с его прошлого
    /// удара (сила удара растёт с перезарядкой руки). Возвращает true, если
    /// вагонетка сломана: урон перевалил за предел, а творческий игрок
    /// ломает её сразу (зонд minecart-hit).
    pub fn hit(&mut self, creative: bool, since_attack: u32) -> bool {
        let charge = hand_charge(since_attack);

        self.hurt_dir = -self.hurt_dir;
        self.hurt_time = HURT_TIME;
        self.hits += 1;
        self.damage += HIT * (0.2 + 0.8 * charge * charge);
        self.dirty |= DIRTY_HURT | DIRTY_DIR | DIRTY_DAMAGE;

        creative || self.damage > BREAKS_OVER
    }

    /// Что выпадает из сломанной: содержимое, потом сама вагонетка одним
    /// предметом (вики, Minecart with Chest: «drops 1 Minecart with Chest
    /// plus contents»; порядок — зонд t-act, взрыв).
    pub fn drops(&self) -> Vec<Stack> {
        let own = blocks::item_named(self.kind.name()).map(|item| Stack::new(item, 1));

        self.slots.iter().flatten().copied().chain(own).collect()
    }

    /// Поджечь вагонетку с динамитом: запал `fuse` тактов, если она ещё не
    /// горит.
    pub fn prime(&mut self, fuse: i32) {
        if self.kind == Kind::Tnt && self.fuse < 0 {
            self.fuse = fuse;
            self.primes += 1;
            self.just_primed = true;
        }
    }

    /// Взорваться: `bonus` — скорость по горизонтали или что вместо неё.
    fn detonate(&mut self, bonus: f64) {
        if self.kind == Kind::Tnt && self.blast.is_none() {
            self.blast = Some(bonus);
        }
    }

    /// Заправка печи щелчком игрока, стоящего в (x, z): уголь добавляет
    /// 3600 тактов (не больше 32767), толчок — от игрока к вагонетке (вики,
    /// Minecart with Furnace). Возвращает, израсходован ли уголь.
    pub fn refuel(&mut self, (x, z): (f64, f64), coal: bool) -> bool {
        if self.kind != Kind::Furnace {
            return false;
        }

        if coal {
            self.fed += FUEL_PER_COAL;
        }

        if self.fuel + self.fed > 0 {
            self.push_x = self.x - x;
            self.push_z = self.z - z;
        }

        coal
    }

    /// Взрыв рядом: толчок и урон (урон вагонетки — вдесятеро, как у удара
    /// рукой в 1 единицу). [проверить]
    pub fn blasted(&mut self, (x, y, z): (f64, f64, f64), damage: f32) {
        self.vx += x;
        self.vy += y;
        self.vz += z;
        self.impulses += 1;
        self.hurt_dir = -self.hurt_dir;
        self.hurt_time = HURT_TIME;
        self.damage += damage * 10.0;
        self.dirty |= DIRTY_HURT | DIRTY_DIR | DIRTY_DAMAGE;
    }

    /// Разбита: урон перевалил за предел. Вагонетка с динамитом от
    /// взрыва не ломается, а загорается (`blast_prime`).
    pub fn wrecked(&self) -> bool {
        self.damage > BREAKS_OVER && self.kind != Kind::Tnt
    }

    /// Вагонетку с динамитом разбил взрыв: она загорается со случайным
    /// запалом до 40 тактов (вики, Minecart with TNT: «destroyed by … an
    /// explosion» — «a random delay between 0 and 1.95 seconds»; зонд
    /// t-fall: задетая взрывом не пропала, а подожглась, со звуком). `luck`
    /// — случай от 0 до 1. [проверить]: распределение запала.
    pub fn blast_prime(&mut self, luck: f32) {
        if self.kind == Kind::Tnt && self.damage > BREAKS_OVER {
            self.prime((luck * BLAST_FUSE as f32) as i32);
        }
    }

    /// Такт вагонетки без соседей: удар спадает, тяжесть, затем — по
    /// рельсе, если она есть в своей клетке или клеткой ниже, иначе как
    /// обычная сущность: шаг и сопротивление. Поворот — по сдвигу за такт.
    pub fn step<V: View>(&mut self, view: &V) {
        self.age += 1;

        if self.hurt_time > 0 {
            self.hurt_time -= 1;
            self.dirty |= DIRTY_HURT;
        }

        if self.damage > 0.0 {
            self.damage -= 1.0;
            self.dirty |= DIRTY_DAMAGE;
        }

        let (from_x, from_y, from_z) = (self.x, self.y, self.z);

        if self.kind == Kind::Furnace {
            self.burn_fuel();
        }

        self.vy -= GRAVITY;

        match rail_under(view, self.x, self.y, self.z) {
            Some((at, kind, shape)) => {
                self.on_rail = true;
                self.fall = 0.0;
                self.along_rail(view, at, kind, shape);
            }
            None => {
                self.on_rail = false;
                self.off_rail(view);
                self.fell(from_y - self.y);
            }
        }

        self.turn(self.x - from_x, self.z - from_z);

        if self.kind == Kind::Tnt {
            self.burn();
        }
    }

    /// Падение вне рельс: пролетевшая вниз больше трёх блоков вагонетка
    /// с динамитом взрывается о землю сразу, прибавка к силе — пролёт / 10
    /// (вики, Minecart with TNT: «hits the ground with a downward velocity
    /// of it falling more than three blocks»; «the fall distance divided by
    /// 10»). [проверить]
    fn fell(&mut self, dropped: f64) {
        if dropped > 0.0 {
            self.fall += dropped;
        }

        if self.on_ground {
            if self.fall > FALL_BLAST {
                self.detonate(self.fall / 10.0);
            }

            self.fall = 0.0;
        }
    }

    /// Запал вагонетки с динамитом убывает после шага; дошедший до нуля
    /// взрывается тактом позже (зонд: подожжённая на рельсе в такте
    /// появления уже показывает 79, ноль — через 79 тактов, взрыв — в
    /// следующем, на 80-м). Сила — по скорости на конец такта.
    ///
    /// Вики обещает взрыв о стену с ходу («pressed into a block … and has
    /// velocity») — у оригинала 26.1.2 его нет: вагонетка на 0,29 в стену
    /// встаёт и не взрывается (зонд t-wall).
    fn burn(&mut self) {
        if self.fuse > 0 {
            self.fuse -= 1;
        } else if self.fuse == 0 {
            self.detonate(self.vx.hypot(self.vz));
        }
    }

    /// Поворот по сдвигу за такт (зонд minecart-curve: 52,9°, 45°, 110,1°,
    /// 90° — угол сдвига). Угол считается от запада: у едущей на восток он
    /// 180°; если новый угол отличается от прежнего больше чем на 170°,
    /// вагонетка «разворачивается» — угол +180° (у вагонетки нет переда).
    fn turn(&mut self, dx: f64, dz: f64) {
        if dx * dx + dz * dz <= TURN_FROM {
            return;
        }

        let mut yaw = dz.atan2(dx).to_degrees() as f32 + 180.0;

        if self.flipped {
            yaw += 180.0;
        }

        let turned = (yaw - self.yaw + 180.0).rem_euclid(360.0) - 180.0;

        if turned.abs() >= 170.0 {
            yaw += 180.0;
            self.flipped = !self.flipped;
        }

        self.yaw = (yaw + 180.0).rem_euclid(360.0) - 180.0;
    }

    /// Шаг по рельсе. Порядок снят зондами: тяга наклона, скорость вдоль
    /// рельсы, торможение, вагонетка на линию рельсы, шаг (не больше 0,4 по
    /// оси), высота по рельсе, где оказалась, сопротивление, разгон спуском,
    /// толчок запитанной рельсы.
    fn along_rail<V: View>(&mut self, view: &V, at: (i32, i32, i32), kind: RailKind, shape: Shape) {
        let (bx, by, bz) = at;
        let (cx, cz) = (bx as f64 + 0.5, bz as f64 + 0.5);

        if self.kind == Kind::Furnace {
            self.steer();
        }

        self.on_ground = false;
        self.y = by as f64;
        self.vy = 0.0;

        // На наклоне вагонетку тянет вниз.
        if let Some((rx, rz)) = shape.rise() {
            self.vx -= rx * SLOPE;
            self.vz -= rz * SLOPE;
        }

        // Скорость ложится вдоль рельсы, сохраняя величину: поворот ведёт
        // вагонетку, не тормозя (зонд minecart-curve).
        let [a, b] = shape.ends();
        let (lx, lz) = (b.0 - a.0, b.1 - a.1);
        let length = lx.hypot(lz);
        let speed = self.vx.hypot(self.vz);
        let along = if self.vx * lx + self.vz * lz < 0.0 { -1.0 } else { 1.0 };

        self.vx = speed * along * lx / length;
        self.vz = speed * along * lz / length;

        // Активирующая рельса включает и выключает воронку, запитанная —
        // поджигает динамит.
        if let RailKind::Activator { on } = kind {
            match self.kind {
                Kind::Hopper => self.enabled = !on,
                Kind::Tnt if on => self.prime(RAIL_FUSE),
                _ => {}
            }
        }

        // Незапитанная рельса с питанием тормозит.
        if kind == (RailKind::Powered { on: false }) {
            if speed < BRAKE_STOP {
                self.vx = 0.0;
                self.vz = 0.0;
            } else {
                self.vx *= BRAKE;
                self.vz *= BRAKE;
            }
        }

        // Вагонетка встаёт на линию рельсы.
        let (ax, az) = (cx + a.0, cz + a.1);
        let t = ((self.x - ax) * lx + (self.z - az) * lz) / (length * length);

        self.x = ax + lx * t;
        self.z = az + lz * t;

        let high = ride_height(at, shape, self.x, self.z);

        // Шаг — не быстрее предела по каждой оси. На наклоне ящик идёт
        // вдоль рельсы и по высоте: иначе на подъёме он упирался бы в опору
        // следующей ступени (зонд minecart-uphill: оригинал въезжает).
        let limit = if self.kind == Kind::Furnace { FURNACE_STEP } else { MAX_SPEED };
        let step_x = self.vx.clamp(-limit, limit);
        let step_z = self.vz.clamp(-limit, limit);
        let climb = shape.rise().map_or(0.0, |(rx, rz)| step_x * rx + step_z * rz);

        self.y = high - RIDE;
        self.step_up = STEP_UP;
        self.move_by(view, step_x, climb, step_z);
        self.step_up = 0.0;
        self.on_ground = false;

        if climb == 0.0 {
            // На ровной рельсе ящик идёт на уровне низа клетки: сошедшая с
            // конца рельс вагонетка остаётся на нём (зонд minecart-end).
            self.y = by as f64;
        }

        // Ушла с поворота через его конец — дальше едет в сторону этого
        // конца (зонд minecart-curve: на выходе скорость сразу вдоль новой
        // рельсы, хотя сама вагонетка ещё не на её линии).
        let (nx, nz) = (self.x.floor() as i32, self.z.floor() as i32);

        if (nx, nz) != (bx, bz) {
            let out = (f64::from((nx - bx).signum()), f64::from((nz - bz).signum()));

            if let Some(end) = shape.ends().into_iter().find(|end| (end.0 * 2.0, end.1 * 2.0) == out) {
                let speed = self.vx.hypot(self.vz);

                self.vx = end.0 * 2.0 * speed;
                self.vz = end.1 * 2.0 * speed;
            }
        }

        // Высота — по рельсе, куда пришли. Ушла с рельс — остаётся на уровне
        // низа прежней клетки (зонд minecart-end: y = −60, а не −59,9375).
        let low = match rail_under(view, self.x, self.y, self.z) {
            Some((there, _, there_shape)) => {
                let height = ride_height(there, there_shape, self.x, self.z);

                self.y = height;
                Some(height)
            }
            None => None,
        };

        let drag = self.drag();

        // Горящая печь толкает: от скорости остаётся 0,8, толчок
        // прибавляется целиком, потом обычное сопротивление (зонд furnace:
        // 0,96 · (0,8 · 1,92 + 1,99957) = 3,39414).
        if self.kind == Kind::Furnace && self.shove_x * self.shove_x + self.shove_z * self.shove_z > PUSH_FROM {
            self.vx = self.vx * FURNACE_KEEP + self.shove_x;
            self.vz = self.vz * FURNACE_KEEP + self.shove_z;
        }

        self.vx *= drag;
        self.vz *= drag;

        // Спуск разгоняет, подъём тормозит — по разнице высот рельсы.
        if let Some(low) = low {
            let speed = self.vx.hypot(self.vz);

            if speed > 0.0 {
                let gained = speed + (high - low) * DROP_GAIN;

                self.vx = self.vx / speed * gained;
                self.vz = self.vz / speed * gained;
            }
        }

        // Запитанная рельса разгоняет.
        if kind == (RailKind::Powered { on: true }) {
            let speed = self.vx.hypot(self.vz);

            if speed > BOOST_FROM {
                self.vx += self.vx / speed * BOOST;
                self.vz += self.vz / speed * BOOST;
            } else {
                self.start_from_wall(view, at, shape);
            }
        }
    }

    /// Печь перед шагом по рельсе: скорость не больше 2 по каждой оси, а
    /// толчок этого такта — толчок от игрока, умноженный на косинус угла
    /// между ним и скоростью вместе с тяжестью. Считается каждый такт от
    /// толчка игрока заново, не копится (зонд furnace: толчок 2 при
    /// скорости 1,92 и тяжести 0,04 — 2 · 1,92 / √(1,92² + 0,04²) =
    /// 1,99957; при скорости 2 — 1,99960 и дальше так и остаётся; толчок
    /// 6,6 — 6,59868). Стоящую толчок не поворачивает. Против хода —
    /// толчка нет. [проверить]: против хода.
    fn steer(&mut self) {
        self.vx = self.vx.clamp(-FURNACE_LIMIT, FURNACE_LIMIT);
        self.vz = self.vz.clamp(-FURNACE_LIMIT, FURNACE_LIMIT);

        (self.shove_x, self.shove_z) = (self.push_x, self.push_z);

        let push = self.push_x.hypot(self.push_z);
        let flat = self.vx * self.vx + self.vz * self.vz;

        if push * push <= PUSH_FROM || flat <= STEER_FROM {
            return;
        }

        let along = (self.vx * self.push_x + self.vz * self.push_z) / push;
        let whole = (flat + self.vy * self.vy).sqrt();

        if along <= 0.0 {
            self.push_x = 0.0;
            self.push_z = 0.0;
            (self.shove_x, self.shove_z) = (0.0, 0.0);
            return;
        }

        let scale = along / whole;

        self.shove_x = self.push_x * scale;
        self.shove_z = self.push_z * scale;
    }

    /// Такт печи: уголь, брошенный за такт, идёт в топливо — клиентам в
    /// следующем такте уходят «горит» и скорость (зонд furnace: свойство и
    /// скорость 2,88 — тактом позже щелчка); топливо убывает, кончилось —
    /// толчка нет. [проверить]: такт, в котором гаснет.
    fn burn_fuel(&mut self) {
        if self.fed > 0 {
            self.fuel = (self.fuel + self.fed).min(FUEL_MAX);
            self.fed = 0;
            self.impulses += 1;
        }

        if self.fuel > 0 {
            self.fuel -= 1;
        }

        if self.fuel <= 0 {
            self.push_x = 0.0;
            self.push_z = 0.0;
        }
    }

    /// Сопротивление на рельсах. У вагонетки с сундуком или воронкой оно
    /// тем меньше, чем она легче: 0,98 + (15 − сила сравнителя) · 0,001, в
    /// одинарной точности (зонд minecart-chest: пустая — 0,995, заполненная
    /// на 10 из 27 ячеек — 0,989).
    fn drag(&self) -> f64 {
        if self.slots.is_empty() {
            return RAIL_DRAG;
        }

        let signal = crate::containers::Container::signal(&self.slots) as f32;

        f64::from(0.98_f32 + (15.0 - signal) * 0.001_f32)
    }

    /// Стоящая на запитанной рельсе вагонетка трогается от твёрдого блока,
    /// в который рельса упирается концом.
    fn start_from_wall<V: View>(&mut self, view: &V, (bx, by, bz): (i32, i32, i32), shape: Shape) {
        match shape {
            Shape::EastWest => {
                if blocks::conductive(view.block(bx - 1, by, bz)) {
                    self.vx = START_PUSH;
                } else if blocks::conductive(view.block(bx + 1, by, bz)) {
                    self.vx = -START_PUSH;
                }
            }
            Shape::NorthSouth => {
                if blocks::conductive(view.block(bx, by, bz - 1)) {
                    self.vz = START_PUSH;
                } else if blocks::conductive(view.block(bx, by, bz + 1)) {
                    self.vz = -START_PUSH;
                }
            }
            _ => {}
        }
    }

    /// Шаг вне рельс: предел скорости, трение о землю, шаг, сопротивление
    /// воздуха — если после шага не на земле (зонд minecart-end).
    fn off_rail<V: View>(&mut self, view: &V) {
        self.vx = self.vx.clamp(-MAX_SPEED, MAX_SPEED);
        self.vz = self.vz.clamp(-MAX_SPEED, MAX_SPEED);

        if self.on_ground {
            self.vx *= GROUND_DRAG;
            self.vy *= GROUND_DRAG;
            self.vz *= GROUND_DRAG;
        }

        self.move_by(view, self.vx, self.vy, self.vz);

        if !self.on_ground {
            self.vx *= AIR_DRAG;
            self.vy *= AIR_DRAG;
            self.vz *= AIR_DRAG;
        }
    }

    /// Сдвигает ящик вагонетки, упираясь в блоки: сперва по вертикали,
    /// потом по горизонтали — по большей из осей первой. Каждая ось идёт до
    /// первого блока впереди; блок, в который ящик уже залез, не мешает
    /// (так вагонетка съезжает с наклона мимо опоры верхней ступени).
    /// Упёршаяся ось теряет скорость.
    fn move_by<V: View>(&mut self, view: &V, dx: f64, dy: f64, dz: f64) {
        let start = (self.x, self.y, self.z);
        let dy_done = self.sweep(view, Axis::Y, dy);

        self.y += dy_done;

        let order = if dx.abs() < dz.abs() { [Axis::Z, Axis::X] } else { [Axis::X, Axis::Z] };
        let (mut dx_done, mut dz_done) = (0.0, 0.0);

        for axis in order {
            if axis == Axis::X {
                dx_done = self.sweep(view, axis, dx);
                self.x += dx_done;
            } else {
                dz_done = self.sweep(view, axis, dz);
                self.z += dz_done;
            }
        }

        // Совсем малый сдвиг, урезанный упором, не делается вовсе: стоящая
        // на земле вагонетка со скоростью 0,0002 стоит (зонд minecart-end),
        // хотя тяжесть и тянет её в пол.
        let done = dx_done * dx_done + dy_done * dy_done + dz_done * dz_done;
        let wanted = dx * dx + dy * dy + dz * dz;

        if done <= NO_STEP && wanted - done >= NO_STEP {
            (self.x, self.y, self.z) = start;
        }

        if dy != 0.0 {
            self.on_ground = dy < 0.0 && dy_done != dy;
        }

        if dy_done != dy {
            self.vy = 0.0;
        }

        if dx_done != dx {
            self.vx = 0.0;
        }

        if dz_done != dz {
            self.vz = 0.0;
        }
    }

    /// Насколько ящик может сдвинуться по оси, не входя в блок впереди.
    fn sweep<V: View>(&self, view: &V, axis: Axis, wanted: f64) -> f64 {
        if wanted == 0.0 {
            return 0.0;
        }

        let half = WIDTH / 2.0;
        let low = [self.x - half, self.y, self.z - half];
        let high = [self.x + half, self.y + HEIGHT, self.z + half];
        let along = axis as usize;

        // Клетки, которые ящик заденет по пути.
        let mut from = low;
        let mut to = high;

        if wanted > 0.0 {
            to[along] += wanted;
        } else {
            from[along] += wanted;
        }

        let cells = |index: usize| (from[index].floor() as i32)..=((to[index] - 1e-9).floor() as i32);
        let mut allowed = wanted;

        for bx in cells(0) {
            for by in cells(1) {
                for bz in cells(2) {
                    if !blocks_cart(view.block(bx, by, bz)) {
                        continue;
                    }

                    let cube = [bx as f64, by as f64, bz as f64];

                    // По прочим осям куб должен перекрываться с ящиком. Куб
                    // не выше шага через ступень не мешает (на рельсах).
                    let across = (0..3).filter(|index| *index != along).all(|index| cube[index] + 1.0 > low[index] && cube[index] < high[index]);

                    if along != Axis::Y as usize && cube[1] + 1.0 <= low[1] + self.step_up {
                        continue;
                    }

                    if !across {
                        continue;
                    }

                    if wanted > 0.0 && cube[along] >= high[along] {
                        allowed = allowed.min(cube[along] - high[along]);
                    } else if wanted < 0.0 && cube[along] + 1.0 <= low[along] {
                        allowed = allowed.max(cube[along] + 1.0 - low[along]);
                    }
                }
            }
        }

        allowed
    }

    /// Раздут ли ящик для встреч: идёт по рельсам. [проверить]: у стоящей.
    fn reaches(&self) -> f64 {
        if self.on_rail && self.vx * self.vx + self.vz * self.vz > 0.01 { REACH } else { 0.0 }
    }

    /// Пересекается ли ящик (раздутый на `grow`) с ящиком другой.
    fn touches(&self, grow: f64, other: &Minecart) -> bool {
        let reach = WIDTH / 2.0 + grow + WIDTH / 2.0;

        (self.x - other.x).abs() < reach
            && (self.z - other.z).abs() < reach
            && self.y < other.y + HEIGHT
            && other.y < self.y + HEIGHT
    }

    /// Встреча с другой вагонеткой (зонд minecart-collide).
    fn meet(&mut self, other: &mut Minecart) {
        let (mut dx, mut dz) = (other.x - self.x, other.z - self.z);
        let distance = (dx * dx + dz * dz).sqrt();

        if distance < 0.01 {
            return;
        }

        // Бок о бок — не встреча: сосед должен быть впереди или сзади.
        let facing = (f64::from(self.yaw).to_radians().cos(), f64::from(self.yaw).to_radians().sin());

        if ((dx * facing.0 + dz * facing.1) / distance).abs() < HEAD_ON {
            return;
        }

        let scale = (1.0 / distance).min(1.0) * PUSH / distance;

        dx *= scale;
        dz *= scale;

        let (mean_x, mean_z) = ((self.vx + other.vx) / 2.0, (self.vz + other.vz) / 2.0);

        self.vx = self.vx * KEEP_OWN + mean_x - dx;
        self.vz = self.vz * KEEP_OWN + mean_z - dz;
        other.vx = other.vx * KEEP_OWN + mean_x + dx;
        other.vz = other.vz * KEEP_OWN + mean_z + dz;
        other.impulses += 1;
    }

    /// Игрок, стоящий вплотную, отталкивает вагонетку от себя (вики,
    /// Minecart: «mobs that move up against a still cart set it in
    /// motion»). Толчков два, они складываются (зонд minecart-push: игрок в
    /// 0,3–0,82 блока позади вагонетки, толчки 0,05, 0,05, 0,05, 0,0886,
    /// 0,0452, потом ни одного):
    ///
    /// - ящики касаются — 0,05 от игрока, как ни близко;
    /// - вагонетка едет по рельсам (`grow` > 0) и игрок в её ящике, раздутом
    ///   на `grow` вбок, — ещё толчок, как сущности толкают друг друга:
    ///   0,05 · √d, где d — большая из разниц по X и Z (0,05 · √0,592 =
    ///   0,0385, 0,05 · √0,819 = 0,0452).
    fn pushed_by(&mut self, (x, y, z): (f64, f64, f64), grow: f64) {
        const PLAYER_WIDTH: f64 = 0.6;
        const PLAYER_HEIGHT: f64 = 1.8;

        let reach = WIDTH / 2.0 + PLAYER_WIDTH / 2.0;
        let (dx, dz) = (self.x - x, self.z - z);

        if y >= self.y + HEIGHT || self.y >= y + PLAYER_HEIGHT {
            return;
        }

        if grow > 0.0 && dx.abs() < reach + grow && dz.abs() < reach + grow {
            let far = dx.abs().max(dz.abs());

            if far >= 0.01 {
                let root = far.sqrt();
                let scale = (1.0 / root).min(1.0) * PUSH / root;

                self.vx += dx * scale;
                self.vz += dz * scale;
                self.impulses += 1;
            }
        }

        if dx.abs() >= reach || dz.abs() >= reach {
            return;
        }

        let distance = (dx * dx + dz * dz).sqrt();

        if distance < 0.01 {
            return;
        }

        let scale = (1.0 / distance).min(1.0) * PUSH / distance;

        self.vx += dx * scale;
        self.vz += dz * scale;

        // Толкнутую игроком клиентам сообщают сразу: скорость и место каждый
        // такт, пока игрок её толкает (minecart-push).
        self.impulses += 1;
    }
}

/// Такт всех вагонеток мира по порядку появления: каждая шагает, потом
/// сталкивается с соседками и игроками. `active` — считать ли её в этом
/// такте (в выгруженном чанке вагонетка стоит). Состояние на конец такта
/// запоминается для рассылки следующего такта.
pub fn step_all<V: View>(carts: &mut [Minecart], view: &V, players: &[(f64, f64, f64)], active: impl Fn(&Minecart) -> bool) {
    for cart in carts.iter_mut() {
        cart.remember();
    }

    for index in 0..carts.len() {
        if !active(&carts[index]) {
            continue;
        }

        carts[index].step(view);

        let grow = carts[index].reaches();

        for other in 0..carts.len() {
            if other == index {
                continue;
            }

            let (this, that) = if index < other {
                let (left, right) = carts.split_at_mut(other);
                (&mut left[index], &mut right[0])
            } else {
                let (left, right) = carts.split_at_mut(index);
                (&mut right[0], &mut left[other])
            };

            if this.touches(grow, that) {
                this.meet(that);
            }
        }

        for &player in players {
            carts[index].pushed_by(player, grow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Площадка: каменный пол ниже `floor`, выше — воздух и то, что поставили.
    struct Scene {
        floor: i32,
        blocks: HashMap<(i32, i32, i32), i32>,
    }

    impl Scene {
        fn new(floor: i32) -> Self {
            Self { floor, blocks: HashMap::new() }
        }

        fn set(&mut self, at: (i32, i32, i32), text: &str) -> &mut Self {
            let state = blocks::state_from_text(text).unwrap_or_else(|| panic!("нет блока {}", text));

            self.blocks.insert(at, state);
            self
        }

        /// Заливка: от `from` до `to` включительно.
        fn fill(&mut self, from: (i32, i32, i32), to: (i32, i32, i32), text: &str) -> &mut Self {
            for x in from.0..=to.0 {
                for y in from.1..=to.1 {
                    for z in from.2..=to.2 {
                        self.set((x, y, z), text);
                    }
                }
            }

            self
        }

        fn view(&self) -> impl Fn(i32, i32, i32) -> i32 + '_ {
            let stone = blocks::state_by_name("stone").unwrap_or(1);

            move |x, y, z| match self.blocks.get(&(x, y, z)) {
                Some(state) => *state,
                None if y < self.floor => stone,
                None => crate::world::AIR,
            }
        }
    }

    /// Прогон: вагонетки появляются в своих точках (на такте из списка) и
    /// катятся `ticks` тактов. Возвращает состояние после каждого такта.
    fn run(scene: &Scene, spawns: &[(u64, (f64, f64, f64))], ticks: u64) -> Vec<Vec<Minecart>> {
        let carts: Vec<(u64, Minecart)> =
            spawns.iter().enumerate().map(|(id, (at, place))| (*at, Minecart::new(id as i32, Kind::Plain, *place))).collect();

        run_carts(scene, &carts, ticks)
    }

    /// То же с готовыми вагонетками (любого вида, с содержимым).
    fn run_carts(scene: &Scene, spawns: &[(u64, Minecart)], ticks: u64) -> Vec<Vec<Minecart>> {
        let view = scene.view();
        let mut carts: Vec<Minecart> = Vec::new();
        let mut trace = Vec::new();

        for tick in 0..ticks {
            for (at, cart) in spawns {
                if *at == tick {
                    carts.push(cart.clone());
                }
            }

            step_all(&mut carts, &view, &[], |_| true);
            trace.push(carts.clone());
        }

        trace
    }

    /// Разгон с места у стены на запитанных рельсах, дальше обычные: сцена
    /// зонда minecart-powered (место — как у оригинала, чтобы сошлось до
    /// последнего знака).
    fn powered_scene() -> Scene {
        let mut scene = Scene::new(-60);

        scene
            .fill((399, -60, 420), (399, -59, 420), "stone")
            .fill((400, -61, 420), (402, -61, 420), "redstone_block")
            .fill((400, -60, 420), (402, -60, 420), "powered_rail[shape=east_west,powered=true]")
            .fill((403, -60, 420), (420, -60, 420), "rail[shape=east_west]");
        scene
    }

    /// С места у стены запитанная рельса толкает на 0,02, дальше каждый такт
    /// v·0,96 + 0,06; шаг — не больше 0,4, хотя скорость растёт дальше.
    /// Числа — оригинал (зонд minecart-powered).
    #[test]
    fn a_powered_rail_starts_and_boosts_like_the_original() {
        let trace = run(&powered_scene(), &[(0, (400.5, -60.0, 420.5))], 14);
        let at = |tick: usize| (trace[tick][0].x, trace[tick][0].y, trace[tick][0].vx);

        assert_eq!(at(0), (400.5, -59.9375, 0.02));
        assert_eq!(at(1), (400.52, -59.9375, 0.079_199_999_999_999_99));
        assert_eq!(at(2).2, 0.136_032);
        assert_eq!(at(3).0, 400.735_232);
        assert_eq!(at(9).0, 402.591_414_422_401_84);
        assert_eq!(at(9).2, 0.475_049_686_179_769_1);
        // Сошла с запитанных — только сопротивление.
        assert_eq!(at(11).2, 0.555_405_790_783_275_2);
        assert_eq!(at(12).2, 0.555_405_790_783_275_2 * RAIL_DRAG);
    }

    /// Спуск разгоняет: с места на наклоне скорость за такт (v + 0,0078125)
    /// · 1,01, высота — по рельсе + 1/16 (зонд minecart-slope).
    #[test]
    fn a_slope_accelerates_like_the_original() {
        let scene = slope_scene();

        let trace = run(&scene, &[(0, (401.5, -58.0, 400.5))], 30);
        let at = |tick: usize| (trace[tick][0].x, trace[tick][0].y, trace[tick][0].vx);

        assert_eq!(at(0), (401.507_812_5, -57.445_312_5, 0.007_890_625));
        assert_eq!(at(1).0, 401.523_515_625);
        assert_eq!(at(20).0, 403.430_926_543_040_8);
        assert!((at(20).2 - 0.183_371_765_430_410_73).abs() < 1e-12, "{:?}", at(20));
        assert_eq!(at(23).1, -59.9375);
        assert!((at(23).2 - 0.211_150_015_571_142_57).abs() < 1e-12, "{:?}", at(23));
    }

    /// Поворот ведёт вагонетку, не тормозя; на выходе скорость сразу вдоль
    /// новой рельсы (зонд minecart-curve).
    #[test]
    fn a_curve_turns_the_cart_like_the_original() {
        let scene = curve_scene();

        let trace = run(&scene, &[(0, (440.5, -60.0, 400.5))], 30);
        let cart = |tick: usize| trace[tick][0].clone();

        // Въехала в поворот: идёт наискось с той же скоростью.
        let turned = (1..30).find(|tick| cart(*tick).vz != 0.0).expect("не повернула");
        let (before, after) = (cart(turned - 1), cart(turned));

        assert!((after.vx.hypot(after.vz) - before.vx * RAIL_DRAG).abs() < 1e-12);
        assert!((after.x - 446.222_189_272_667_26).abs() < 1e-9, "{} {}", after.x, after.z);

        // Вышла на юг: скорость только по z.
        let out = cart(turned + 2);

        assert_eq!(out.vx, 0.0);
        assert!((out.vz - 0.244_132_792_799_042_18).abs() < 1e-9, "{}", out.vz);
        assert!((out.yaw - 45.0).abs() < 1e-3);
    }

    /// Сошла с конца рельс: скорость урезается до 0,4, на земле делится
    /// пополам каждый такт (зонд minecart-end).
    #[test]
    fn a_cart_leaving_the_rails_slides_and_stops() {
        let scene = end_scene();

        let trace = run(&scene, &[(0, (401.5, -60.0, 440.5))], 40);
        let off = (0..40).find(|tick| !trace[*tick][0].on_rail).expect("не сошла");
        let cart = |tick: usize| trace[tick][0].clone();

        assert_eq!(cart(off - 1).y, -60.0);
        assert_eq!(cart(off).vx, 0.4);
        assert!(cart(off).on_ground);
        assert_eq!(cart(off + 1).vx, 0.2);
        assert!((cart(39).x - 407.991_023_797_401_6).abs() < 1e-9, "{}", cart(39).x);
    }

    /// Встреча: догнавшая вагонетка отдаёт стоящей часть скорости (зонд
    /// minecart-collide).
    #[test]
    fn a_moving_cart_pushes_a_standing_one_like_the_original() {
        let scene = collide_scene();

        let trace = run(&scene, &[(0, (448.5, -60.0, 430.5)), (1, (440.5, -60.0, 430.5))], 40);
        let hit = (2..40).find(|tick| trace[*tick][0].vx != 0.0).expect("не столкнулись");

        assert!((trace[hit][0].vx - 0.155_150_768_108_266_22).abs() < 1e-12, "{}", trace[hit][0].vx);
        assert!((trace[hit][1].vx - 0.114_840_570_104_050_47).abs() < 1e-12, "{}", trace[hit][1].vx);
    }

    /// Игрок стоит в 0,3 блока позади вагонетки на ровной рельсе и не
    /// двигается: скорость по тактам — как у оригинала (зонд minecart-push:
    /// 0,05, 0,098, 0,144, 0,227, 0,263, дальше только сопротивление).
    #[test]
    fn a_player_pushes_a_cart_like_the_original() {
        let scene = collide_scene();
        let view = scene.view();
        let mut carts = vec![Minecart::new(0, Kind::Plain, (450.5, -60.0, 430.5))];

        step_all(&mut carts, &view, &[], |_| true);

        let player = [(450.2, -60.0, 430.5)];
        let wanted = [0.05, 0.098, 0.144, 0.227, 0.263, 0.252, 0.242];

        for speed in wanted {
            step_all(&mut carts, &view, &player, |_| true);
            assert!((carts[0].vx - speed).abs() < 0.0006, "{} вместо {speed}", carts[0].vx);
            assert_eq!(carts[0].vz, 0.0);
        }
    }

    /// Сцены зондов — на тех же местах, что у оригинала.
    fn slope_scene() -> Scene {
        let mut scene = Scene::new(-60);

        scene
            .fill((400, -60, 400), (400, -57, 400), "stone")
            .set((400, -57, 400), "rail[shape=east_west]")
            .fill((401, -60, 400), (401, -59, 400), "stone")
            .set((402, -60, 400), "stone")
            .set((401, -58, 400), "rail[shape=ascending_west]")
            .set((402, -59, 400), "rail[shape=ascending_west]")
            .set((403, -60, 400), "rail[shape=ascending_west]")
            .fill((404, -60, 400), (460, -60, 400), "rail[shape=east_west]");
        scene
    }

    fn curve_scene() -> Scene {
        let mut scene = powered_scene_at(400);

        scene
            .fill((442, -60, 400), (445, -60, 400), "rail[shape=east_west]")
            .set((446, -60, 400), "rail[shape=south_west]")
            .fill((446, -60, 401), (446, -60, 412), "rail[shape=north_south]");
        scene
    }

    fn end_scene() -> Scene {
        let mut scene = Scene::new(-60);

        scene
            .fill((400, -60, 440), (400, -59, 440), "stone")
            .fill((401, -61, 440), (404, -61, 440), "redstone_block")
            .fill((401, -60, 440), (404, -60, 440), "powered_rail[shape=east_west,powered=true]")
            .fill((405, -60, 440), (406, -60, 440), "rail[shape=east_west]");
        scene
    }

    fn collide_scene() -> Scene {
        let mut scene = powered_scene_at(430);

        scene.fill((442, -60, 430), (465, -60, 430), "rail[shape=east_west]");
        scene
    }

    fn brake_scene() -> Scene {
        let mut scene = Scene::new(-60);

        scene
            .fill((399, -60, 460), (399, -59, 460), "stone")
            .fill((400, -61, 460), (406, -61, 460), "redstone_block")
            .fill((400, -60, 460), (406, -60, 460), "powered_rail[shape=east_west,powered=true]")
            .fill((407, -60, 460), (412, -60, 460), "rail[shape=east_west]")
            .fill((413, -60, 460), (414, -60, 460), "powered_rail[shape=east_west,powered=false]")
            .fill((415, -60, 460), (430, -60, 460), "rail[shape=east_west]");
        scene
    }

    fn chest_scene() -> Scene {
        let mut scene = Scene::new(-60);

        for z in [480, 482] {
            scene
                .fill((399, -60, z), (399, -59, z), "stone")
                .fill((400, -61, z), (402, -61, z), "redstone_block")
                .fill((400, -60, z), (402, -60, z), "powered_rail[shape=east_west,powered=true]")
                .fill((403, -60, z), (438, -60, z), "rail[shape=east_west]");
        }

        scene.fill((399, -60, 481), (399, -59, 481), "stone");
        scene
    }

    /// Вагонетки с сундуком зонда minecart-chest: пустая и с 10 стопками
    /// камня.
    fn chest_carts() -> Vec<(u64, Minecart)> {
        let empty = Minecart::new(0, Kind::Chest, (400.5, -60.0, 480.5));
        let mut full = Minecart::new(1, Kind::Chest, (400.5, -60.0, 482.5));
        let stone = blocks::item_named("stone").expect("камень");

        for slot in 0..10 {
            full.slots[slot] = Some(Stack::new(stone, 64));
        }

        vec![(0, empty), (0, full)]
    }

    fn uphill_scene() -> Scene {
        let mut scene = Scene::new(-60);

        scene
            .fill((439, -60, 470), (439, -59, 470), "stone")
            .fill((440, -61, 470), (442, -61, 470), "redstone_block")
            .fill((440, -60, 470), (442, -60, 470), "powered_rail[shape=east_west,powered=true]")
            .fill((443, -60, 470), (445, -60, 470), "rail[shape=east_west]")
            .set((447, -60, 470), "stone")
            .fill((448, -60, 470), (448, -59, 470), "stone")
            .fill((449, -60, 470), (449, -58, 470), "stone")
            .fill((450, -60, 470), (450, -57, 470), "stone")
            .set((446, -60, 470), "rail[shape=ascending_east]")
            .set((447, -59, 470), "rail[shape=ascending_east]")
            .set((448, -58, 470), "rail[shape=ascending_east]")
            .set((449, -57, 470), "rail[shape=ascending_east]")
            .set((450, -56, 470), "rail[shape=east_west]");
        scene
    }

    fn powered_scene_at(z: i32) -> Scene {
        let mut scene = Scene::new(-60);

        scene
            .fill((439, -60, z), (439, -59, z), "stone")
            .fill((440, -61, z), (441, -61, z), "redstone_block")
            .fill((440, -60, z), (441, -60, z), "powered_rail[shape=east_west,powered=true]");
        scene
    }

    /// Незапитанная рельса с питанием останавливает медленную вагонетку.
    #[test]
    fn an_unpowered_powered_rail_brakes() {
        let mut scene = Scene::new(0);

        scene.set((0, 0, 0), "powered_rail[shape=east_west,powered=false]");

        let mut cart = Minecart::new(1, Kind::Plain, (0.5, 0.0625, 0.5));
        cart.vx = 0.02;
        cart.step(&scene.view());

        assert_eq!(cart.vx, 0.0);
    }

    /// Удар рукой: 10 урона после паузы, через такт — 2,72; творческий
    /// ломает сразу (зонд minecart-hit).
    #[test]
    fn hits_add_damage_by_the_hand_charge() {
        let mut cart = Minecart::new(1, Kind::Plain, (0.5, 0.0, 0.5));

        assert!(!cart.hit(false, 100));
        assert_eq!((cart.damage, cart.hurt_time, cart.hurt_dir), (10.0, 10, -1));
        assert!(!cart.hit(false, 1));
        assert!((cart.damage - 12.72).abs() < 1e-5);
        assert!(cart.hit(true, 0));
    }

    /// Вагонетка с динамитом на запитанной активирующей рельсе: в первом
    /// же такте запал 79, ноль — на 80-м такте, взрыв — на 81-м, сила без
    /// прибавки: стоит (зонд t-act).
    #[test]
    fn an_activator_rail_primes_a_tnt_cart_like_the_original() {
        let mut scene = Scene::new(0);

        scene.set((0, -1, 0), "redstone_block").set((0, 0, 0), "activator_rail[shape=east_west,powered=true]");

        let trace = run_carts(&scene, &[(0, Minecart::new(1, Kind::Tnt, (0.5, 0.0, 0.5)))], 82);

        assert_eq!((trace[0][0].fuse, trace[0][0].primes), (79, 1));
        assert_eq!(trace[79][0].fuse, 0);
        assert_eq!(trace[79][0].blast, None);
        assert_eq!(trace[80][0].blast, Some(0.0));
        assert_eq!(blast_power(0.0, 0.7), 4.0);
    }

    /// С пяти блоков вагонетка с динамитом взрывается о землю, с двух — нет
    /// (зонд t-fall).
    #[test]
    fn a_tnt_cart_falling_far_explodes_on_landing() {
        let scene = Scene::new(0);
        let high = run_carts(&scene, &[(0, Minecart::new(1, Kind::Tnt, (0.5, 5.0, 0.5)))], 40);
        let low = run_carts(&scene, &[(0, Minecart::new(1, Kind::Tnt, (0.5, 2.0, 0.5)))], 40);
        let landed = (0..40).find(|tick| high[*tick][0].on_ground).expect("не упала");

        assert!(high[landed][0].blast.is_some_and(|bonus| bonus > 0.4));
        assert!(high[landed - 1][0].blast.is_none());
        assert!(low.iter().all(|carts| carts[0].blast.is_none()));
    }

    /// Взрыв разбивает вагонетку с динамитом не насовсем: она загорается.
    #[test]
    fn a_blasted_tnt_cart_is_primed_not_broken() {
        let mut cart = Minecart::new(1, Kind::Tnt, (0.5, 0.0, 0.5));

        cart.blasted((0.19, 0.0, 0.0), 22.4);
        assert!(!cart.wrecked());
        cart.blast_prime(0.4);
        assert_eq!(cart.fuse, 16);

        let mut plain = Minecart::new(2, Kind::Plain, (0.5, 0.0, 0.5));

        plain.blasted((0.19, 0.0, 0.0), 22.4);
        assert!(plain.wrecked());
    }

    /// Печь: игрок в двух блоках позади, уголь. Первый такт — скорость
    /// 0,96 · 2, места не меняет; дальше шаг 0,2 и числа оригинала (зонд
    /// furnace).
    #[test]
    fn a_fuelled_furnace_cart_moves_like_the_original() {
        let mut scene = Scene::new(0);

        scene.fill((0, 0, 0), (20, 0, 0), "rail[shape=east_west]");

        let view = scene.view();
        let mut carts = vec![Minecart::new(1, Kind::Furnace, (0.5, 0.0, 0.5))];

        step_all(&mut carts, &view, &[], |_| true);
        assert!(carts[0].refuel((-1.5, 0.5), true));
        step_all(&mut carts, &view, &[], |_| true);

        assert_eq!((carts[0].x, carts[0].fuel), (0.5, 3599));
        assert!((carts[0].vx - 1.92).abs() < 1e-12, "{}", carts[0].vx);

        step_all(&mut carts, &view, &[], |_| true);
        assert!((carts[0].x - 0.7).abs() < 1e-12);
        assert!((carts[0].vx - 3.394_143_468_917_975_2).abs() < 1e-12, "{}", carts[0].vx);
        assert!((carts[0].shove_x - 1.999_566_113_456_224_1).abs() < 1e-12, "{}", carts[0].shove_x);

        step_all(&mut carts, &view, &[], |_| true);
        step_all(&mut carts, &view, &[], |_| true);
        assert!((carts[0].vx - 3.455_616_115_161_613).abs() < 1e-12, "{}", carts[0].vx);
        assert!((carts[0].shove_x - 1.999_600_119_960_014).abs() < 1e-12, "{}", carts[0].shove_x);
    }

    /// Из сломанной выпадает сперва содержимое, потом сама вагонетка.
    #[test]
    fn contents_drop_before_the_cart_itself() {
        let mut cart = Minecart::new(1, Kind::Chest, (0.5, 0.0, 0.5));
        let stone = blocks::item_named("stone").expect("камень");

        cart.slots[3] = Some(Stack::new(stone, 5));

        let drops = cart.drops();

        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0], Stack::new(stone, 5));
        assert_eq!(Some(drops[1].item), blocks::item_named("chest_minecart"));
    }

    /// Выгрузка трассы для сверки с оригиналом: `MINECART_TRACE=файл cargo
    /// test -- --ignored minecart_trace`.
    #[test]
    #[ignore]
    fn minecart_trace() {
        use std::fmt::Write;

        let Ok(path) = std::env::var("MINECART_TRACE") else {
            return;
        };

        let mut out = String::new();
        type Spawns = Vec<(u64, (f64, f64, f64))>;

        let scenes: Vec<(&str, Scene, Spawns)> = vec![
            ("powered", powered_scene(), vec![(0, (400.5, -60.0, 420.5))]),
            ("slope", slope_scene(), vec![(0, (401.5, -58.0, 400.5))]),
            ("curve", curve_scene(), vec![(0, (440.5, -60.0, 400.5))]),
            ("offend", end_scene(), vec![(0, (401.5, -60.0, 440.5))]),
            ("collide", collide_scene(), vec![(0, (448.5, -60.0, 430.5)), (1, (440.5, -60.0, 430.5))]),
        ];

        let mut runs: Vec<(&str, Vec<Vec<Minecart>>)> =
            scenes.iter().map(|(name, scene, spawns)| (*name, run(scene, spawns, 150))).collect();

        runs.push(("brake", run(&brake_scene(), &[(0, (400.5, -60.0, 460.5))], 150)));
        runs.push(("uphill", run(&uphill_scene(), &[(0, (440.5, -60.0, 470.5))], 150)));
        runs.push(("chest", run_carts(&chest_scene(), &chest_carts(), 150)));

        for (name, trace) in &runs {
            for (tick, carts) in trace.iter().enumerate() {
                for cart in carts {
                    let _ = writeln!(out, "{} {} {} {:?} {:?} {:?} {:?} {:?} {:?} {}", name, tick, cart.entity_id, cart.x, cart.y, cart.z, cart.vx, cart.vy, cart.vz, cart.yaw);
                }
            }
        }

        std::fs::write(path, out).expect("трасса не записалась");
    }
}
