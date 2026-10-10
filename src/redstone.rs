// Редстоун: сигнал и то, что его даёт, несёт и слушает.
//
// Всё здесь написано по разбору в tools/research/redstone.md (страницы
// minecraft.wiki). Кода игры и чужих ядер нет.
//
// Как это устроено. У сигнала есть сила от 0 до 15. Дают её источники — рычаг,
// кнопка, факел, блок редстоуна, повторитель и сравнитель на выходе. Несёт её
// провод, теряя по единице на блок, и запитанные твёрдые блоки. Слушают её
// механизмы — лампа, дверь — и те же повторители с факелами.
//
// Различие, на котором всё держится: блок бывает запитан сильно и слабо.
// Слабо его питает провод, лежащий на нём или смотрящий в него, — и такой блок
// не питает соседний провод, хотя механизмы и повторители от него работают.
// Сильно его питают повторитель, сравнитель, факел снизу, рычаг и кнопка на
// нём — такой блок питает всё.
//
// Порядок — второе, на чём всё держится. Когда блок меняется, об этом узнают
// его соседи, в порядке запад, восток, низ, верх, север, юг, и каждый сосед
// разбирается до конца прежде, чем очередь дойдёт до следующего. Задержки
// (факел, повторитель, сравнитель) идут через очередь запланированных тактов
// мира с приоритетами — какие именно, сказано у каждой детали.

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::blocks::{self, Orientation};
use crate::containers;
use crate::world::{AIR, World};

pub mod bookshelf;
mod components;
pub use components::{target_hit, target_power};
pub mod crafter;
mod jukebox;
pub mod lectern;
mod rails;
mod sculk;
pub mod sculk_family;
pub mod shelf;
pub mod statue;

pub use jukebox::{playing as jukebox_playing,jukebox_contents_changed, jukebox_reading, jukebox_song, tick_jukebox, use_jukebox};
pub use lectern::lectern_reading;

pub use components::{Occupant, ambient_sky, daylight_tick, disarm_tripwire, hook_broken_by_player, punched, random_ticks};
pub use rails::{Cart, holds_rail};
pub use sculk::{game_event, game_event_from, sensors_tick};

/// Место в мире.
pub type Pos = (i32, i32, i32);

/// Наибольшая сила сигнала.
pub const MAX_POWER: u8 = 15;

/// Что сообщается клиенту о ходе поршня: выдвигается или задвигается.
const PISTON_EXTENDS: u8 = 0;
const PISTON_RETRACTS: u8 = 1;

/// Действие поршня «отменить начатое выдвигание» (Block Action 2).
const PISTON_CANCELS: u8 = 2;

/// Сколько тактов занимает ход поршня — выдвигание и задвигание.
///
/// По вики ход длится два такта: `progress` растёт на половину за такт.
/// Плавную картинку клиенту даёт особый блок «движущийся поршень», которого
/// у нас пока нет, но время выдержано.
const PISTON_MOVE: u64 = 2;

/// Через сколько тактов наблюдатель отзывается на замеченное изменение.
const OBSERVER_DELAY: u64 = 2;

/// Сколько тактов держится его сигнал. По вики — «a redstone pulse of strong
/// power at signal strength 15 for 2 game ticks».
const OBSERVER_PULSE: u64 = 2;

/// Через сколько тактов гаснет лампа, потерявшая питание.
///
/// Загорается она сразу, а гаснет с задержкой: короткий разрыв сигнала
/// лампа просто не замечает.
const LAMP_DELAY: u64 = 4;

/// За сколько тактов переключается факел.
const TORCH_DELAY: u64 = 2;

/// Сколько переключений за какое окно выжигают факел.
const BURNOUT_TOGGLES: usize = 8;
const BURNOUT_WINDOW: u64 = 60;

/// За сколько тактов сравнитель применяет новое значение.
const COMPARATOR_DELAY: u64 = 2;

/// Сколько блоков поршень двигает за раз.
const PUSH_LIMIT: usize = 12;

/// Сколько держится нажатая кнопка: каменная и деревянная.
const STONE_BUTTON_TICKS: u64 = 20;
const WOODEN_BUTTON_TICKS: u64 = 30;

/// Насколько глубоко разбор обновлений идёт за один раз. Дальше остаток
/// откладывается на следующий разбор: иначе очень длинная цепочка могла бы
/// переполнить стек.
const MAX_DEPTH: usize = 256;

/// Приоритеты запланированных тактов — как на вики.
const PRIORITY_INTO_DIODE: i8 = -3;
const PRIORITY_DEPOWERING: i8 = -2;
const PRIORITY_DIODE: i8 = -1;
pub(crate) const PRIORITY_NORMAL: i8 = 0;

/// Частоты вибраций (вики, Vibration): `block_activate`, `block_open`,
/// `note_block_play` — 10; `block_deactivate`, `block_close` — 9;
/// `block_destroy` — 12; `block_place` — 13.
pub const FREQ_DEACTIVATE: u8 = 9;
pub const FREQ_ACTIVATE: u8 = 10;
pub const FREQ_DESTROY: u8 = 12;
pub const FREQ_PLACE: u8 = 13;

/// Звук отпущенной кнопки: у каменных свой, у деревянных — по породе.
fn button_click_off(name: &str) -> &'static str {
    if name.starts_with("cherry") {
        "minecraft:block.cherry_wood_button.click_off"
    } else if name.starts_with("bamboo") {
        "minecraft:block.bamboo_wood_button.click_off"
    } else if name.starts_with("crimson") || name.starts_with("warped") {
        "minecraft:block.nether_wood_button.click_off"
    } else if is_wooden(name) {
        "minecraft:block.wooden_button.click_off"
    } else {
        "minecraft:block.stone_button.click_off"
    }
}

/// Звук нажатой кнопки — пара к `button_click_off` (заряд ветра жмёт кнопку
/// не рукой: звук слышат все; чёрный ящик, wind-charge-throw).
pub(crate) fn button_click_on(name: &str) -> &'static str {
    match button_click_off(name) {
        "minecraft:block.cherry_wood_button.click_off" => "minecraft:block.cherry_wood_button.click_on",
        "minecraft:block.bamboo_wood_button.click_off" => "minecraft:block.bamboo_wood_button.click_on",
        "minecraft:block.nether_wood_button.click_off" => "minecraft:block.nether_wood_button.click_on",
        "minecraft:block.wooden_button.click_off" => "minecraft:block.wooden_button.click_on",
        _ => "minecraft:block.stone_button.click_on",
    }
}

/// Через сколько тактов падает песок, оставшийся без опоры.
const FALL_DELAY: u64 = 2;

/// Шесть сторон — в том порядке, в каком игра сообщает соседям об изменении.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    West,
    East,
    Down,
    Up,
    North,
    South,
}

/// Порядок обновления соседей: запад, восток, низ, верх, север, юг.
const NEIGHBOUR_ORDER: [Dir; 6] = [Dir::West, Dir::East, Dir::Down, Dir::Up, Dir::North, Dir::South];

/// Горизонтальные стороны.
const HORIZONTAL: [Dir; 4] = [Dir::West, Dir::East, Dir::North, Dir::South];

impl Dir {
    /// Сторона по часовой стрелке, если смотреть сверху (у вертикальных —
    /// она сама).
    fn clockwise(self) -> Dir {
        match self {
            Dir::North => Dir::East,
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            other => other,
        }
    }

    /// Лежит ли сторона в плоскости земли.
    fn is_horizontal(self) -> bool {
        !matches!(self, Dir::Up | Dir::Down)
    }

    fn offset(self) -> (i32, i32, i32) {
        match self {
            Dir::West => (-1, 0, 0),
            Dir::East => (1, 0, 0),
            Dir::Down => (0, -1, 0),
            Dir::Up => (0, 1, 0),
            Dir::North => (0, 0, -1),
            Dir::South => (0, 0, 1),
        }
    }

    fn opposite(self) -> Dir {
        match self {
            Dir::West => Dir::East,
            Dir::East => Dir::West,
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
        }
    }

    /// Имя стороны, как в свойствах блоков.
    fn name(self) -> &'static str {
        match self {
            Dir::West => "west",
            Dir::East => "east",
            Dir::Down => "down",
            Dir::Up => "up",
            Dir::North => "north",
            Dir::South => "south",
        }
    }

    fn from_name(name: &str) -> Option<Dir> {
        Some(match name {
            "west" => Dir::West,
            "east" => Dir::East,
            "down" => Dir::Down,
            "up" => Dir::Up,
            "north" => Dir::North,
            "south" => Dir::South,
            _ => return None,
        })
    }

    /// Сторона по её номеру.
    pub fn by_number(number: u8) -> Option<Dir> {
        Some(match number {
            0 => Dir::Down,
            1 => Dir::Up,
            2 => Dir::North,
            3 => Dir::South,
            4 => Dir::West,
            5 => Dir::East,
            _ => return None,
        })
    }

    /// Номер стороны — так их нумерует игра: низ, верх, север, юг,
    /// запад, восток.
    pub fn number(self) -> u8 {
        match self {
            Dir::Down => 0,
            Dir::Up => 1,
            Dir::North => 2,
            Dir::South => 3,
            Dir::West => 4,
            Dir::East => 5,
        }
    }

    /// Две горизонтальные стороны, перпендикулярные этой.
    fn sides(self) -> [Dir; 2] {
        match self {
            Dir::North | Dir::South => [Dir::West, Dir::East],
            _ => [Dir::North, Dir::South],
        }
    }
}

fn step(pos: Pos, dir: Dir) -> Pos {
    let (dx, dy, dz) = dir.offset();

    (pos.0 + dx, pos.1 + dy, pos.2 + dz)
}

/// Блоки прерванного выдвигания: срок, порядок хода, что куда едет.
type Detached = (u64, u64, Vec<(Pos, i32)>);

/// Память редстоуна: то, что не помещается в состояние блока.
pub struct Memory {
    /// Такты, в которые факел переключался: по ним считается выгорание.
    torch_toggles: HashMap<Pos, VecDeque<u64>>,

    /// Сила на выходе сравнителя: в состоянии блока есть только «включён».
    comparator_output: HashMap<Pos, u8>,

    /// Поршни в пути: пока идёт ход, поршень ничего нового не начинает.
    moving: HashMap<Pos, Moving>,

    /// Номер следующего хода — для порядка, в котором ходы кончаются.
    next_move: u64,

    /// Блоки прерванных выдвиганий, едущие без поршня: срок, порядок, что
    /// куда.
    detached: Vec<Detached>,

    /// Выгоревшие факелы: они не загораются, пока не остынут.
    burned_torches: std::collections::HashSet<Pos>,

    /// Кто где стоит в этом такте: игроки и то, что лежит в мире. Нужны и
    /// плитам с растяжками в фазе сущностей, и такту растяжки — он проверяет,
    /// не ушли ли с неё.
    occupants: Vec<Occupant>,

    /// Горящая редстоуновая руда: гаснет на случайном такте. Случайных
    /// тактов по всему миру у нас нет, поэтому руда помнится поимённо.
    lit_ores: BTreeSet<Pos>,
    pub mob_blocks: BTreeSet<Pos>,

    /// Датчики дневного света: у оригинала это блок-сущности, которые
    /// пересчитывают выход раз в 20 тактов. Порядок — по координатам.
    daylight: BTreeSet<Pos>,

    /// Вагонетки в этом такте, в порядке появления: их видят детекторные
    /// рельсы и сравнитель за ними.
    carts: Vec<Cart>,

    /// Рамки для предметов: клетка, сторона и сила, которую читает
    /// сравнитель (frame.rs).
    frames: Vec<FrameMark>,

    /// Датчики скалка и что они слышат — у оригинала это их блок-сущности.
    sensors: sculk::Sensors,
    pub sculk: sculk_family::Memory,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            torch_toggles: HashMap::new(),
            comparator_output: HashMap::new(),
            moving: HashMap::new(),
            next_move: 0,
            detached: Vec::new(),
            burned_torches: std::collections::HashSet::new(),
            occupants: Vec::new(),
            lit_ores: BTreeSet::new(),
            mob_blocks: BTreeSet::new(),
            daylight: BTreeSet::new(),
            carts: Vec::new(),
            frames: Vec::new(),
            sensors: sculk::Sensors::new(),
            sculk: sculk_family::Memory::default(),
        }
    }

    /// В этом месте встал блок: горящая руда и датчики света запоминаются,
    /// всё прочее — забывается. Зовёт мир при каждой правке блока.
    pub fn note_state(&mut self, pos: Pos, state: i32) {
        self.sculk.note_state(pos, state);
        if matches!(blocks::block_at_state(state), Some("creaking_heart" | "dried_ghast")) {
            self.mob_blocks.insert(pos);
        } else { self.mob_blocks.remove(&pos); }

        if components::is_lit_ore(state) {
            self.lit_ores.insert(pos);
        } else if !self.lit_ores.is_empty() {
            self.lit_ores.remove(&pos);
        }

        if components::is_daylight(state) {
            self.daylight.insert(pos);
        } else if !self.daylight.is_empty() {
            self.daylight.remove(&pos);
        }

        if sculk::is_sensor_state(state) {
            self.sensors.entry(pos).or_default();
        } else if !self.sensors.is_empty() {
            self.sensors.remove(&pos);
        }
    }

    /// Поршни, которые сейчас в пути, — чтобы записать их в файл мира.
    pub fn moving_pistons(&self) -> impl Iterator<Item = (&Pos, &Moving)> {
        self.moving.iter()
    }

    /// Возвращает ход на место — после перезапуска сервера.
    pub fn restore_moving(&mut self, pos: Pos, moving: Moving) {
        self.moving.insert(pos, moving);
    }

    /// Забывает, что какие-то поршни были в пути.
    ///
    /// Нужно только проверкам: так выглядит память после перезапуска.
    #[cfg(test)]
    pub fn forget_moving(&mut self) {
        self.moving.clear();
    }

    /// Забывает всё, что лежит вне оставшихся чанков.
    pub fn forget_outside<F>(&mut self, keep: F)
    where
        F: Fn(i32, i32) -> bool,
    {
        self.torch_toggles.retain(|(x, _, z), _| keep(*x, *z));
        self.comparator_output.retain(|(x, _, z), _| keep(*x, *z));
        self.burned_torches.retain(|(x, _, z)| keep(*x, *z));
        self.lit_ores.retain(|(x, _, z)| keep(*x, *z));
        self.mob_blocks.retain(|(x, _, z)| keep(*x, *z));
        self.daylight.retain(|(x, _, z)| keep(*x, *z));
        self.sensors.retain(|(x, _, z), _| keep(*x, *z));
        // Начатые ходы поршней не забываются: чанк вернётся — ход доиграется.
        // Они крошечные, а без них едущий блок пропал бы совсем.
    }
}

/// Поршень в пути: ход занимает время, и всё это время поршень занят.
///
/// Как в игре, едущие блоки снимаются со своих мест в начале хода: на их
/// местах остаётся служебный «движущийся поршень», а сами блоки лежат здесь
/// и встают на новые места в конце. Схемы, читающие исходные клетки, видят
/// их пустыми ровно тогда же, когда и в игре.
pub struct Moving {
    /// Выдвигается или задвигается.
    pub extending: bool,

    /// Куда едет и что именно: место назначения и состояние блока.
    pub cargo: Vec<(Pos, i32)>,

    /// Куда встанет голова — или откуда её убрать.
    head: Pos,

    /// Сторона, в которую идёт ход.
    pub facing: Dir,

    /// В каком такте ход кончается: блоки встают на места в фазе
    /// блок-сущностей этого такта.
    pub due: u64,

    /// Порядок появления хода: кончаются они в том же порядке, как
    /// блок-сущности у оригинала.
    order: u64,
}

impl Moving {
    /// Собирает ход заново — из того, что записано в файле мира. Доиграется
    /// он в первом же такте.
    pub fn restored(pos: Pos, extending: bool, facing: Dir, cargo: Vec<(Pos, i32)>) -> Self {
        Self {
            extending,
            cargo,
            head: step(pos, facing),
            facing,
            due: 0,
            order: 0,
        }
    }
}


/// Что за блок стоит в этом месте — с точки зрения редстоуна.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Wire,

    /// Командный блок любого вида (command_block.rs).
    CommandBlock,
    Piston,
    PistonHead,

    /// Служебный блок на месте едущего: ход идёт, блок уже снят с места.
    MovingPiston,
    Torch,
    Lever,
    Button,
    Plate,
    RedstoneBlock,
    Lamp,
    Repeater,
    Comparator,
    Door,
    Trapdoor,
    Gate,
    Observer,
    Bulb,
    NoteBlock,

    /// Сундук и бочка: своего сигнала нет, но у сундука свой такт —
    /// повтор крышки, пока он открыт.
    Chest,

    /// Сундук-ловушка: источник, сила — число смотрящих.
    TrappedChest,

    /// Выбрасыватель и раздатчик: срабатывают по фронту сигнала, слушают и
    /// клетку над собой (квазисвязность).
    Dispenser,

    /// Воронка: сигнал её запирает.
    Hopper,

    /// Мишень: проводящий блок и заодно источник — сила по попаданию.
    Target,

    /// Датчик дневного света: источник, сила по свету и времени суток.
    Daylight,

    /// Растяжка: сама не питает, будит крючки.
    Tripwire,

    /// Крючок растяжки: источник, как кнопка.
    Hook,

    /// Громоотвод со всеми стадиями окисления: источник на 8 тактов.
    LightningRod,

    /// Колокол: механизм, звонит по фронту сигнала.
    Bell,

    /// Обычная рельса: форма по соседкам, поворот в развилке — от сигнала.
    Rail,

    /// Рельса с питанием и активирующая: механизм, передаёт питание по
    /// цепочке соединённых рельс того же вида.
    PoweredRail,

    /// Детекторная рельса: источник, пока на ней вагонетка.
    DetectorRail,

    /// Динамит: механизм, непроводящий; сигнал его поджигает (вики, TNT).
    Tnt,

    /// Голова и череп: механизм — от сигнала у них `powered` (пасть
    /// дракона, уши пиглина).
    Head,

    /// Датчик скалка, обычный и калиброванный: источник, пока `active`.
    Sculk,

    /// Проигрыватель: проводящий блок и источник 15, пока играет песня.
    Jukebox,

    /// Кафедра: источник 15 на 2 такта при перелистывании (`powered`).
    Lectern,

    /// Крафтер: крафт по фронту сигнала (redstone/crafter.rs).
    Crafter,
    Other,
}

/// Блок в месте: состояние, имя, вид и свойства.
#[derive(Clone, Copy)]
struct Block {
    state: i32,
    name: &'static str,
    kind: Kind,
    orientation: Option<&'static Orientation>,
}

impl Block {
    fn at(world: &World, pos: Pos) -> Block {
        let state = world.get_block(pos.0, pos.1, pos.2);
        let name = blocks::block_at_state(state).unwrap_or("air");

        Block {
            state,
            name,
            kind: kind_of(name),
            orientation: blocks::orientation(name),
        }
    }

    fn value(&self, property: &str) -> Option<&'static str> {
        self.orientation?.value(self.state, property)
    }

    fn is(&self, property: &str, value: &str) -> bool {
        self.value(property) == Some(value)
    }

    fn with(&self, property: &str, value: &str) -> Option<i32> {
        self.orientation?.with(self.state, property, value)
    }

    fn facing(&self) -> Option<Dir> {
        Dir::from_name(self.value("facing")?)
    }

    fn conductive(&self) -> bool {
        blocks::conductive(self.state)
    }
}

fn kind_of(name: &str) -> Kind {
    match name {
        "redstone_wire" => Kind::Wire,
        "redstone_torch" | "redstone_wall_torch" => Kind::Torch,
        "lever" => Kind::Lever,
        "redstone_block" => Kind::RedstoneBlock,
        "observer" => Kind::Observer,
        _ if name.ends_with("copper_bulb") => Kind::Bulb,
        "redstone_lamp" => Kind::Lamp,
        "command_block" | "chain_command_block" | "repeating_command_block" => Kind::CommandBlock,
        "note_block" => Kind::NoteBlock,
        // Эндер-сундук и шулкер-ящик — с крышкой, как сундук (блочное
        // событие 1 с числом смотрящих).
        "chest" | "barrel" | "ender_chest" => Kind::Chest,
        _ if name.ends_with("shulker_box") => Kind::Chest,
        // Медный сундук любой стадии — как сундук (вики, Copper Chest).
        _ if name.ends_with("copper_chest") => Kind::Chest,
        "trapped_chest" => Kind::TrappedChest,
        "dropper" | "dispenser" => Kind::Dispenser,
        "hopper" => Kind::Hopper,
        "target" => Kind::Target,
        "daylight_detector" => Kind::Daylight,
        "tripwire" => Kind::Tripwire,
        "tripwire_hook" => Kind::Hook,
        "bell" => Kind::Bell,
        "tnt" => Kind::Tnt,
        "jukebox" => Kind::Jukebox,
        "lectern" => Kind::Lectern,
        "crafter" => Kind::Crafter,
        _ if sculk::is_sensor(name) => Kind::Sculk,
        _ if head_of(name).is_some() => Kind::Head,
        _ if name.ends_with("lightning_rod") => Kind::LightningRod,
        "repeater" => Kind::Repeater,
        "comparator" => Kind::Comparator,
        "piston" | "sticky_piston" => Kind::Piston,
        "piston_head" => Kind::PistonHead,
        "rail" => Kind::Rail,
        "powered_rail" | "activator_rail" => Kind::PoweredRail,
        "detector_rail" => Kind::DetectorRail,
        "moving_piston" => Kind::MovingPiston,
        _ if name.ends_with("_button") => Kind::Button,
        _ if name.ends_with("_pressure_plate") => Kind::Plate,
        _ if name.ends_with("_door") => Kind::Door,
        _ if name.ends_with("_fence_gate") => Kind::Gate,
        _ if name.ends_with("_trapdoor") => Kind::Trapdoor,
        _ => Kind::Other,
    }
}

/// Как блок ведёт себя, когда его толкает поршень.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Push {
    /// Двигается.
    Moves,

    /// Ломается и выпадает предметом, а поршень идёт дальше.
    Breaks,

    /// Не двигается: поршень не срабатывает вовсе.
    Stays,
}

/// Блоки, которые поршень не двигает вовсе.
///
/// Список с вики (страница Piston/Table). Сюда же по правилу попадают все
/// блоки с содержимым — сундуки, печи, воронки: в Java их двигать нельзя.
const IMMOVABLE: &[&str] = &[
    "barrier",
    "beacon",
    "bedrock",
    "calibrated_sculk_sensor",
    "command_block",
    "chain_command_block",
    "repeating_command_block",
    "creaking_heart",
    "crying_obsidian",
    "enchanting_table",
    "end_gateway",
    "end_portal",
    "end_portal_frame",
    "ender_chest",
    "grindstone",
    "jigsaw",
    "jukebox",
    "light",
    "lodestone",
    "spawner",
    "trial_spawner",
    "moving_piston",
    "nether_portal",
    "obsidian",
    "piston_head",
    "reinforced_deepslate",
    "respawn_anchor",
    "sculk_catalyst",
    "sculk_sensor",
    "sculk_shrieker",
    "structure_block",
    "vault",
];

/// Блоки с содержимым: в Java поршень их не двигает.
const WITH_CONTENTS: &[&str] = &[
    "barrel",
    "beehive",
    "bee_nest",
    "blast_furnace",
    "brewing_stand",
    "chest",
    "trapped_chest",
    "chiseled_bookshelf",
    "conduit",
    "crafter",
    "daylight_detector",
    "dispenser",
    "dropper",
    "furnace",
    "hopper",
    "lectern",
    "smoker",
];


/// Блоки, которые поршень ломает, хотя ящик столкновений у них есть.
///
/// Общее правило вики: «Pistons cannot move blocks that require a support
/// block, as they break and drop as an item (when applicable)». По виду блока
/// это не угадать, поэтому имена выписаны со страницы Piston/Table —
/// из графы «Breaks when pushed, turning into drops when applicable».
const BREAKS_ANYWAY: &[&str] = &[
    "melon",
    "pumpkin",
    "carved_pumpkin",
    "jack_o_lantern",
    "moss_block",
    "pale_moss_block",
    "moss_carpet",
    "pale_moss_carpet",
    "pale_hanging_moss",
    "cactus",
    "ladder",
    "bell",
    "lantern",
    "soul_lantern",
    "copper_lantern",
    "amethyst_cluster",
    "budding_amethyst",
    "turtle_egg",
    "flower_pot",
    "chorus_plant",
    "chorus_flower",
    "sea_pickle",
    "campfire",
    "soul_campfire",
    "pointed_dripstone",
    "lily_pad",
    "bamboo",
    "bamboo_sapling",
    "copper_golem_statue",
    "scaffolding",
    "suspicious_sand",
    "suspicious_gravel",
    "dragon_egg",
    "anvil",
    "chipped_anvil",
    "damaged_anvil",
    "cobweb",
    "resin_clump",
    "sculk_vein",
    "glow_lichen",
    "frogspawn",
    "candle",
];

/// То же по окончанию имени: листва, головы, свечи, почки аметиста,
/// цветы в горшках.
const BREAKS_ANYWAY_SUFFIXES: &[&str] = &[
    "_leaves",
    "_head",
    "_skull",
    "_candle",
    "_amethyst_bud",
];

/// Рельсы ящика столкновений не имеют, но поршень их двигает:
/// «Can be pushed, but breaks if unsupported» (Piston/Table).
const MOVES_ANYWAY: &[&str] = &["rail", "powered_rail", "activator_rail", "detector_rail"];

/// Вторая половина поршня — та, что должна исчезнуть вместе с этой.
///
/// Поршень состоит из основания и головы. Ломая любую из половин, игрок
/// убирает поршень целиком; а вот от обычного обновления основание без
/// головы не рушится — так в игре с беты 1.7_01.
pub fn piston_pair(world: &World, pos: (i32, i32, i32), state: i32) -> Option<(i32, i32, i32)> {
    let name = blocks::block_at_state(state)?;
    let block = Block {
        state,
        name,
        kind: kind_of(name),
        orientation: blocks::orientation(name),
    };

    let facing = block.facing()?;

    match block.kind {
        Kind::Piston if block.is("extended", "true") => {
            let head = step(pos, facing);

            head_in_front(world, pos, &block).then_some(head)
        }
        Kind::PistonHead => {
            let base = step(pos, facing.opposite());

            piston_behind(world, pos, &block).then_some(base)
        }
        _ => None,
    }
}

/// Блоки, которые падают, если под ними пусто.
///
/// Список со страницы Falling block: песок и красный песок, гравий, яйцо
/// дракона, бетонная пыль, подозрительные песок и гравий, наковальни.
/// Сталактиты и леса падают по своим правилам — их пока нет.
const FALLS: &[&str] = &[
    "sand",
    "red_sand",
    "gravel",
    "dragon_egg",
    "suspicious_sand",
    "suspicious_gravel",
    "anvil",
    "chipped_anvil",
    "damaged_anvil",
];

/// Падает ли этот блок без опоры.
pub fn falls(state: i32) -> bool {
    blocks::block_at_state(state)
        .is_some_and(|name| FALLS.contains(&name) || name.ends_with("_concrete_powder"))
}

/// Может ли блок держать на себе песок и гравий.
///
/// Вики, Falling Block: такие блоки падают, когда «block below is
/// replaceable», — воздух, вода, лава, огонь, трава, снежный слой в один
/// слой и прочее из тега `minecraft:replaceable`. Всё остальное держит,
/// даже факел и цветок: на них песок стоит.
fn holds_up(state: i32) -> bool {
    !free_to_fall_into(state)
}

/// Свободно ли место для падающего блока: можно ли упасть сквозь него
/// и приземлиться в него.
///
/// То же, что заменяемый блок: при приземлении такой блок просто
/// заменяется падающим (вики, Falling Block: «If it lands with the bottom
/// center of its hitbox in a replaceable block ... the falling block returns
/// to its block state»).
pub fn free_to_fall_into(state: i32) -> bool {
    blocks::replaceable(state)
}

/// Блоки, которые толкать можно, а тянуть нельзя.
///
/// Со страницы Sticky Piston/Table: «Glazed Terracotta — Cannot be pulled»,
/// то же у костра и у знамён со знаками.
fn can_be_pulled(state: i32) -> bool {
    let Some(name) = blocks::block_at_state(state) else {
        return false;
    };

    !(name.ends_with("_glazed_terracotta")
        || name == "campfire"
        || name == "soul_campfire"
        || name.ends_with("_banner")
        || name.ends_with("_sign"))
}

/// Что поршень сделает с этим блоком.
fn push_reaction(state: i32) -> Push {
    if state == AIR {
        return Push::Breaks;
    }

    let Some(name) = blocks::block_at_state(state) else {
        return Push::Stays;
    };

    // Выдвинутый поршень не двигают — как и его голову.
    if kind_of(name) == Kind::Piston {
        let extended = blocks::orientation(name)
            .and_then(|block| block.value(state, "extended"))
            .is_some_and(|value| value == "true");

        return if extended { Push::Stays } else { Push::Moves };
    }

    if IMMOVABLE.contains(&name) || WITH_CONTENTS.contains(&name) {
        return Push::Stays;
    }

    if MOVES_ANYWAY.contains(&name) {
        return Push::Moves;
    }

    if BREAKS_ANYWAY.contains(&name)
        || BREAKS_ANYWAY_SUFFIXES.iter().any(|end| name.ends_with(end))
        || name.starts_with("potted_")
    {
        return Push::Breaks;
    }

    // Всё, с чем нельзя столкнуться — трава, факелы, провода, плиты
    // нажимные, — ломается и выпадает предметом. Двери и кровати тоже
    // ломаются, хотя ящик у них есть.
    if !blocks::has_box(state)
        || name.ends_with("_door")
        || name.ends_with("_bed")
        || name.ends_with("shulker_box")
        || name == "cake"
        || name == "decorated_pot"
    {
        return Push::Breaks;
    }

    Push::Moves
}

// ---------------------------------------------------------------------------
// Сила сигнала
// ---------------------------------------------------------------------------

/// К какому блоку прикреплён рычаг, кнопка или факел.
fn attached_to(block: &Block, pos: Pos) -> Option<Pos> {
    match block.kind {
        Kind::Lever | Kind::Button => match block.value("face")? {
            "floor" => Some(step(pos, Dir::Down)),
            "ceiling" => Some(step(pos, Dir::Up)),
            _ => Some(step(pos, block.facing()?.opposite())),
        },
        Kind::Torch => {
            if block.name == "redstone_wall_torch" {
                Some(step(pos, block.facing()?.opposite()))
            } else {
                Some(step(pos, Dir::Down))
            }
        }
        // Крючок с `facing=north` висит на блоке к югу от себя (вики,
        // Tripwire Hook); громоотвод стоит на блоке напротив своего `facing`.
        Kind::Hook | Kind::LightningRod => Some(step(pos, block.facing()?.opposite())),
        _ => None,
    }
}

/// Куда смотрит выход повторителя или сравнителя.
///
/// Свойство «facing» у них — сторона входа, поэтому выход напротив.
fn output_dir(block: &Block) -> Option<Dir> {
    Some(block.facing()?.opposite())
}

/// Взвешенная ли это плита: у неё вместо «нажата» сила от 0 до 15.
fn is_weighted(name: &str) -> bool {
    name.ends_with("_weighted_pressure_plate")
}

/// Включён ли источник.
fn is_on(block: &Block) -> bool {
    match block.kind {
        // У взвешенной плиты нет признака «нажата»: она включена, пока
        // её сила больше нуля.
        Kind::Plate if is_weighted(block.name) => plate_power(block) > 0,

        Kind::Lever
        | Kind::Button
        | Kind::Plate
        | Kind::Repeater
        | Kind::Comparator
        | Kind::Observer
        | Kind::Hook
        | Kind::LightningRod
        | Kind::DetectorRail
        | Kind::Lectern => block.is("powered", "true"),
        Kind::Torch => block.is("lit", "true"),
        Kind::RedstoneBlock => true,
        Kind::Target | Kind::Daylight => components::power_value(block) > 0,
        Kind::Sculk => sculk::power_value(block) > 0,
        _ => false,
    }
}

/// Какую силу даёт плита: обычная — все 15, взвешенная — по числу того,
/// что на ней лежит.
fn plate_power(block: &Block) -> u8 {
    if is_weighted(block.name) {
        return block
            .value("power")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
    }

    if block.is("powered", "true") {
        MAX_POWER
    } else {
        0
    }
}

/// Сила, которую источник в этом месте отдаёт в эту сторону — прямо соседу,
/// не через блоки.
fn source_output(world: &World, pos: Pos, block: &Block, toward: Dir) -> u8 {
    // Сундук-ловушка: сила — число смотрящих, во все стороны (вики,
    // Trapped Chest).
    if block.kind == Kind::TrappedChest {
        return containers::trapped_power(world, pos);
    }

    // Проигрыватель: 15 во все стороны, пока играет (вики, Jukebox).
    if block.kind == Kind::Jukebox {
        return if jukebox::playing(world, pos) { MAX_POWER } else { 0 };
    }

    if !is_on(block) {
        return 0;
    }

    match block.kind {
        Kind::Plate => plate_power(block),

        // Крючок и громоотвод питают во все стороны, как кнопка (вики,
        // Tripwire Hook, Lightning Rod), детекторная рельса — тоже (вики,
        // Detector Rail: пыль — 15, механизмы вокруг, «including above or
        // below»).
        Kind::Lever | Kind::Button | Kind::RedstoneBlock | Kind::Hook | Kind::LightningRod | Kind::DetectorRail => {
            MAX_POWER
        }

        // Кафедра во время импульса — 15 во все стороны (вики, Lectern:
        // «emit a full-strength redstone pulse»).
        Kind::Lectern => MAX_POWER,

        // Мишень и датчик света — своей силой во все стороны, блоки не
        // запитывают (вики, Target: «does not power any blocks»; Daylight
        // Detector: «does not power any adjacent conductive blocks»).
        Kind::Target | Kind::Daylight => components::power_value(block),

        // Датчик скалка — во все стороны, «as though the sensor itself is
        // the strongly powered block»; калиброванный — кроме грани ввода.
        Kind::Sculk => sculk::output_toward(block, toward),

        // Наблюдатель выдаёт сигнал только назад — со стороны красной точки.
        Kind::Observer => {
            if block.facing() == Some(toward.opposite()) {
                MAX_POWER
            } else {
                0
            }
        }

        // Факел не питает ту сторону, к которой прикреплён.
        Kind::Torch => {
            if attached_to(block, pos) == Some(step(pos, toward)) {
                0
            } else {
                MAX_POWER
            }
        }

        Kind::Repeater => {
            if output_dir(block) == Some(toward) {
                MAX_POWER
            } else {
                0
            }
        }

        Kind::Comparator if output_dir(block) == Some(toward) => {
            comparator_current(world, pos, block)
        }

        _ => 0,
    }
}

/// Насколько сильно запитан проводящий блок: от повторителя, сравнителя,
/// факела снизу, рычага, кнопки и плиты на нём.
fn strong_power(world: &World, pos: Pos) -> u8 {
    let mut power = 0;

    for dir in NEIGHBOUR_ORDER {
        let neighbour_pos = step(pos, dir);
        let neighbour = Block::at(world, neighbour_pos);

        // Сундук-ловушка сильно питает блок под собой.
        if neighbour.kind == Kind::TrappedChest && dir == Dir::Up {
            power = power.max(containers::trapped_power(world, neighbour_pos));
            continue;
        }

        // Кафедра во время импульса — тоже (вики: «It powers the conductive
        // block beneath it»).
        if neighbour.kind == Kind::Lectern {
            if dir == Dir::Up && is_on(&neighbour) {
                power = MAX_POWER;
            }

            continue;
        }

        if !is_on(&neighbour) {
            continue;
        }

        let given = match neighbour.kind {
            Kind::Repeater | Kind::Comparator => {
                if output_dir(&neighbour) == Some(dir.opposite()) {
                    source_output(world, neighbour_pos, &neighbour, dir.opposite())
                } else {
                    0
                }
            }
            // Факел сильно питает только блок над собой.
            Kind::Torch if dir == Dir::Down => MAX_POWER,
            // Крючок и громоотвод сильно питают блок, к которому
            // прикреплены (вики, Tripwire Hook, Lightning Rod).
            Kind::Lever | Kind::Button | Kind::Hook | Kind::LightningRod => {
                if attached_to(&neighbour, neighbour_pos) == Some(pos) {
                    MAX_POWER
                } else {
                    0
                }
            }
            // Плита лежит на блоке и питает его.
            Kind::Plate if dir == Dir::Up => plate_power(&neighbour),

            // Детекторная рельса сильно питает блок, на котором лежит (вики,
            // Detector Rail: «strongly powers the block supporting the
            // detector rail if that block is conductive»).
            Kind::DetectorRail if dir == Dir::Up => MAX_POWER,

            // Датчик скалка сильно питает блок под собой (вики, Sculk
            // Sensor: «powers the block below the sensor strongly»).
            Kind::Sculk if dir == Dir::Up => sculk::power_value(&neighbour),

            // Наблюдатель сильно питает блок у себя за спиной.
            Kind::Observer if neighbour.facing() == Some(dir) => MAX_POWER,
            _ => 0,
        };

        power = power.max(given);
    }

    power
}

/// Смотрит ли провод в эту сторону (или лежит на этом блоке, если сторона —
/// низ).
fn wire_points(wire: &Block, toward: Dir) -> bool {
    match toward {
        Dir::Down => true,
        Dir::Up => false,
        _ => !wire.is(toward.name(), "none"),
    }
}

/// Насколько запитан проводящий блок хоть как-то: сильно или проводом.
fn weak_power(world: &World, pos: Pos) -> u8 {
    let mut power = strong_power(world, pos);

    for dir in NEIGHBOUR_ORDER {
        let neighbour_pos = step(pos, dir);
        let neighbour = Block::at(world, neighbour_pos);

        if neighbour.kind == Kind::Wire && wire_points(&neighbour, dir.opposite()) {
            power = power.max(wire_power_of(&neighbour));
        }
    }

    power
}

/// Сила провода по его состоянию.
fn wire_power_of(wire: &Block) -> u8 {
    wire.value("power").and_then(|value| value.parse().ok()).unwrap_or(0)
}

/// Сила, которую место получает от соседа с этой стороны.
///
/// Провод разборчив: он слушает только источники и сильно запитанные блоки.
/// Всё остальное — механизмы, повторители — слушает и слабо запитанные блоки
/// и провод, который в них смотрит.
fn power_from(world: &World, receiver: Pos, dir: Dir, receiver_is_wire: bool) -> u8 {
    let neighbour_pos = step(receiver, dir);
    let neighbour = Block::at(world, neighbour_pos);

    match neighbour.kind {
        Kind::Wire => {
            if receiver_is_wire {
                0
            } else if wire_points(&neighbour, dir.opposite()) {
                wire_power_of(&neighbour)
            } else {
                0
            }
        }
        Kind::Other
        | Kind::Crafter
        | Kind::Lamp
        | Kind::Door
        | Kind::Trapdoor
        | Kind::Gate
        | Kind::Bulb
        | Kind::NoteBlock
        | Kind::Bell
        | Kind::Head => {
            if !neighbour.conductive() {
                0
            } else if receiver_is_wire {
                strong_power(world, neighbour_pos)
            } else {
                weak_power(world, neighbour_pos)
            }
        }
        // Мишень и проводит, как сплошной блок («Targets now conduct
        // redstone power»), и сама источник. [проверить]: вики пишет, что
        // запитанная мишень питает соседей «except dust and other targets», —
        // у нас это правило обычного проводящего блока (пыль — только от
        // сильно запитанного).
        Kind::Target | Kind::Jukebox => {
            let through = if !neighbour.conductive() {
                0
            } else if receiver_is_wire {
                strong_power(world, neighbour_pos)
            } else {
                weak_power(world, neighbour_pos)
            };

            through.max(source_output(world, neighbour_pos, &neighbour, dir.opposite()))
        }
        _ => source_output(world, neighbour_pos, &neighbour, dir.opposite()),
    }
}

/// Приходит ли к месту хоть какой-то сигнал — так слушают механизмы.
fn any_power(world: &World, pos: Pos) -> bool {
    NEIGHBOUR_ORDER
        .iter()
        .any(|dir| power_from(world, pos, *dir, false) > 0)
}

// ---------------------------------------------------------------------------
// Провод
// ---------------------------------------------------------------------------

/// Есть ли провод в этом месте.
fn wire_at(world: &World, pos: Pos) -> Option<Block> {
    let block = Block::at(world, pos);

    (block.kind == Kind::Wire).then_some(block)
}

/// Сила, которая должна быть у провода в этом месте.
fn wire_power(world: &World, pos: Pos) -> u8 {
    let mut power = 0;

    // От источников и сильно запитанных блоков вокруг.
    for dir in NEIGHBOUR_ORDER {
        power = power.max(power_from(world, pos, dir, true));
    }

    // От соседнего провода — на единицу слабее.
    let above_conductive = Block::at(world, step(pos, Dir::Up)).conductive();

    for dir in HORIZONTAL {
        let beside = step(pos, dir);

        if let Some(wire) = wire_at(world, beside) {
            power = power.max(wire_power_of(&wire).saturating_sub(1));
        }

        // Провод этажом выше. Правил два, и оба обязательны: связь режет
        // проводящий блок над нами, а вниз сигнал проходит только с
        // проводящего блока — со стекла или с плиты он вниз не идёт.
        if !above_conductive
            && Block::at(world, beside).conductive()
            && let Some(wire) = wire_at(world, step(beside, Dir::Up))
        {
            power = power.max(wire_power_of(&wire).saturating_sub(1));
        }

        // Провод этажом ниже: связь режет проводящий блок над ним.
        if !Block::at(world, beside).conductive()
            && let Some(wire) = wire_at(world, step(beside, Dir::Down))
        {
            power = power.max(wire_power_of(&wire).saturating_sub(1));
        }
    }

    power.min(MAX_POWER)
}

/// Тянется ли провод к тому, что стоит в этой стороне.
fn wire_connects_to(world: &World, pos: Pos, dir: Dir) -> bool {
    let neighbour = Block::at(world, step(pos, dir));

    match neighbour.kind {
        Kind::Wire
        | Kind::Torch
        | Kind::Lever
        | Kind::Button
        | Kind::Plate
        | Kind::RedstoneBlock
        | Kind::TrappedChest
        // Источники притягивают пыль со всех четырёх сторон: мишень
        // («redirects adjacent redstone wires to point to itself»), датчик
        // света, крючок и громоотвод (components-rest.md, §14).
        | Kind::Target
        | Kind::Daylight
        | Kind::Hook
        | Kind::LightningRod
        // Детекторная рельса — источник (components-rest.md, §14).
        | Kind::DetectorRail
        // Датчик скалка — источник. [проверить] тянется ли пыль к нему.
        | Kind::Sculk
        // Проигрыватель: «Being a power source, a jukebox redirects adjacent
        // redstone wires to point to itself»; кафедра — тоже источник.
        | Kind::Jukebox
        | Kind::Lectern => true,
        // У повторителя вход и выход только с торцов, поэтому пыль тянется
        // к нему лишь вдоль его оси.
        Kind::Repeater => neighbour
            .facing()
            .is_some_and(|facing| facing == dir || facing == dir.opposite()),

        // А у сравнителя вход есть и с боков — к нему пыль тянется со всех
        // четырёх сторон.
        Kind::Comparator => true,
        _ => false,
    }
}

/// Форма провода: куда он тянется по каждой из четырёх сторон.
fn wire_shape(world: &World, pos: Pos, current: &Block) -> [(Dir, &'static str); 4] {
    let above_conductive = Block::at(world, step(pos, Dir::Up)).conductive();

    let mut shape = [(Dir::West, "none"); 4];
    let mut connected = 0;

    for (slot, dir) in HORIZONTAL.into_iter().enumerate() {
        let beside = step(pos, dir);

        let value = if !above_conductive && wire_at(world, step(beside, Dir::Up)).is_some() {
            "up"
        } else if wire_connects_to(world, pos, dir)
            || (!Block::at(world, beside).conductive() && wire_at(world, step(beside, Dir::Down)).is_some())
        {
            "side"
        } else {
            "none"
        };

        if value != "none" {
            connected += 1;
        }

        shape[slot] = (dir, value);
    }

    // Одинокий провод — крестик, если его не превратили в точку.
    if connected == 0 {
        let is_dot = HORIZONTAL.iter().all(|dir| current.is(dir.name(), "none"));

        if !is_dot {
            for slot in &mut shape {
                slot.1 = "side";
            }
        }

        return shape;
    }

    // С одним соседом провод вытягивается в линию: и к соседу, и от него.
    if connected == 1 {
        let only = shape
            .iter()
            .position(|(_, value)| *value != "none")
            .expect("одна сторона занята");
        let opposite = shape[only].0.opposite();

        for slot in &mut shape {
            if slot.0 == opposite {
                slot.1 = "side";
            }
        }
    }

    shape
}

/// Каким должен быть провод: форма и сила.
fn wanted_wire(world: &World, pos: Pos, current: &Block) -> Option<i32> {
    let power = wire_power(world, pos);
    let shape = wire_shape(world, pos, current);

    let power_text = power.to_string();

    let mut values: Vec<(&str, &str)> = shape.iter().map(|(dir, value)| (dir.name(), *value)).collect();
    values.push(("power", power_text.as_str()));

    current.orientation?.state(&values)
}

// ---------------------------------------------------------------------------
// Повторитель и сравнитель
// ---------------------------------------------------------------------------

/// Есть ли сигнал на входе повторителя или сравнителя.
fn diode_input(world: &World, pos: Pos, block: &Block) -> u8 {
    match block.facing() {
        Some(input) => power_from(world, pos, input, false),
        None => 0,
    }
}

/// Смотрит ли диод в зад или бок другого диода — от этого зависит приоритет
/// его такта.
///
/// Сосед стоит впереди нас. Мы смотрим ему в зад, если он выдаёт сигнал
/// в ту же сторону, куда и мы (его вход обращён к нам), и в лицо — если
/// он выдаёт навстречу. Всё остальное — бок. Считается «зад или бок»,
/// то есть всё, кроме лица.
fn faces_diode(world: &World, pos: Pos, block: &Block) -> bool {
    let Some(output) = output_dir(block) else {
        return false;
    };

    let target = Block::at(world, step(pos, output));

    matches!(target.kind, Kind::Repeater | Kind::Comparator)
        && output_dir(&target) != Some(output.opposite())
}

/// Заперт ли повторитель: сбоку в него смотрит включённый повторитель или
/// сравнитель.
fn repeater_locked(world: &World, pos: Pos, block: &Block) -> bool {
    let Some(facing) = block.facing() else {
        return false;
    };

    facing.sides().into_iter().any(|side| {
        let beside = Block::at(world, step(pos, side));

        matches!(beside.kind, Kind::Repeater | Kind::Comparator)
            && is_on(&beside)
            && output_dir(&beside) == Some(side.opposite())
    })
}

/// Сила на боковом входе сравнителя.
///
/// Бок слушает провод, который в него смотрит, выход повторителя или
/// сравнителя, блок редстоуна и сильно запитанный блок — слабо запитанный
/// не считается.
fn comparator_side(world: &World, pos: Pos, side: Dir) -> u8 {
    let beside_pos = step(pos, side);
    let beside = Block::at(world, beside_pos);

    match beside.kind {
        Kind::Wire => {
            if wire_points(&beside, side.opposite()) {
                wire_power_of(&beside)
            } else {
                0
            }
        }
        Kind::Repeater | Kind::Comparator | Kind::RedstoneBlock => {
            source_output(world, beside_pos, &beside, side.opposite())
        }
        _ if beside.conductive() => strong_power(world, beside_pos),
        _ => 0,
    }
}

/// Какую силу сравнитель выдаёт сейчас.
///
/// Сила хранится в памяти, а память не переживает перезапуск сервера и
/// выгрузку чанка. Если её нет, а сравнитель включён, считаем заново —
/// иначе после перезапуска он молчал бы, хотя с виду работает.
fn comparator_current(world: &World, pos: Pos, block: &Block) -> u8 {
    if let Some(power) = world.redstone.comparator_output.get(&pos) {
        return *power;
    }

    if is_on(block) {
        comparator_output(world, pos, block)
    } else {
        0
    }
}

/// Что сравнитель должен выдавать на выходе.
fn comparator_output(world: &World, pos: Pos, block: &Block) -> u8 {
    let Some(facing) = block.facing() else {
        return 0;
    };

    let mut rear = diode_input(world, pos, block);

    // Сзади — то, что сравнитель читает (хранилище, торт, котёл…): сила
    // берётся по нему. Через один проводящий блок — тоже, если тот не
    // запитан на все 15 (вики, Redstone Comparator, «Read blocks»).
    let back = step(pos, facing);

    if let Some(reading) = comparator_reading(world, back) {
        rear = reading;
    } else if Block::at(world, back).conductive() && rear < MAX_POWER {
        // За блоком читается и хранилище, и рамка в той же клетке; из двух
        // берётся большее (вики, Redstone Comparator: «outputs whichever
        // signal is stronger»). Рамка прямо у сравнителя, без блока между
        // ними, не читается (чёрный ящик, item-frame-probe).
        let beyond = step(back, facing);

        if let Some(reading) = comparator_reading(world, beyond).into_iter().chain(frame_reading(world, beyond, facing)).max() {
            rear = reading;
        }
    }

    let side = facing
        .sides()
        .into_iter()
        .map(|side| comparator_side(world, pos, side))
        .max()
        .unwrap_or(0);

    if block.is("mode", "subtract") {
        rear.saturating_sub(side)
    } else if side > rear {
        0
    } else {
        rear
    }
}

/// Что сравнитель читает с блока: хранилище, торт, котёл — и детекторную
/// рельсу, на которой стоит вагонетка (вики, Detector Rail: «A comparator
/// can read the contents of a minecart with hopper or with chest on a
/// detector rail through a solid opaque block, as it can with other
/// container blocks»).
fn comparator_reading(world: &World, pos: Pos) -> Option<u8> {
    let block = Block::at(world, pos);

    if block.kind == Kind::DetectorRail {
        return Some(rails::comparator_reading(world, pos, &block));
    }

    // Датчик скалка — частоту последней вибрации.
    if block.kind == Kind::Sculk {
        return Some(sculk::comparator_reading(world, pos, &block));
    }

    // Командный блок — SuccessCount.
    if block.kind == Kind::CommandBlock {
        return crate::command_block::comparator_reading(world, pos);
    }

    let state=world.get_block(pos.0,pos.1,pos.2);
    if crate::hive::is_hive(state) {return Some(crate::hive::honey(state) as u8);}
    containers::comparator_reading(world, pos)
}

/// Задержка повторителя в тактах: по его свойству, от 2 до 8.
fn repeater_delay(block: &Block) -> u64 {
    block
        .value("delay")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1)
        * 2
}

// ---------------------------------------------------------------------------
// Разбор обновлений
// ---------------------------------------------------------------------------

/// Ставит блок и оставляет след для разбора: об изменении узнают соседи.
fn set(world: &mut World, pos: Pos, state: i32) {
    world.set_block(pos.0, pos.1, pos.2, state);
}

/// Ломает блок: он исчезает, а предмет из него выпадет — этим займётся такт.
/// Клиенту ничего не говорит: об этом месте скажет тот, кто ломал.
/// Возвращает true, если там что-то было.
///
/// Дверь и кровать ломаются целиком: вторая половина убирается сразу и без
/// выпадения — предмет один на обе (вики, Piston: двери и кровати поршень
/// ломает, а висящая половина двери или кровати не бывает). Если поршень
/// давит обе половины разом, вторая к своей очереди уже пуста, и второго
/// предмета тоже нет. Об этой половине клиенту говорится обычным порядком:
/// кроме мира, сказать о ней некому.
fn destroy(world: &mut World, pos: Pos) -> bool {
    let state = world.get_block(pos.0, pos.1, pos.2);

    if state == AIR {
        return false;
    }

    let name = blocks::block_at_state(state).unwrap_or("");
    let whole = name.ends_with("_door") || name.ends_with("_bed");
    let partner = blocks::pair_partner(state, pos)
        .filter(|other| whole && blocks::is_pair(state, pos, world.get_block(other.0, other.1, other.2), *other));

    // Кровать выпадает из изголовья, даже если давили изножье: предмет у
    // оригинала появляется на месте изголовья (снято чёрным ящиком,
    // сценарий piston-bed).
    let foot = name.ends_with("_bed")
        && blocks::orientation_of(state).and_then(|block| block.value(state, "part")) == Some("foot");

    match partner {
        Some(head) if foot => world.note_destroyed(head.0, head.1, head.2),
        _ => world.note_destroyed(pos.0, pos.1, pos.2),
    }

    world.set_block_quietly(pos.0, pos.1, pos.2, AIR);

    // Вторая половина уходит вместе с остальными переменами хода: клиентам —
    // на такт позже (сценарии piston-door и piston-bed). О ней оригинал
    // говорит и событием слома (2001 с её состоянием: частицы и звук), а о
    // самой раздавленной половине — нет, как и о раздавленной траве
    // (сценарии piston-door, piston-bed, grass-crush).
    if let Some(other) = partner {
        let other_state = world.get_block(other.0, other.1, other.2);

        world.note_level_event(other.0, other.1, other.2, 2001, other_state);
        world.set_block_quietly(other.0, other.1, other.2, AIR);
        world.tell_later(other.0, other.1, other.2, PISTON_TELL_LATER);
    }

    true
}

/// Деталь без опоры отваливается: предмет выпадет, а клиентам уходит
/// обычное изменение — в той же пачке, что и то, из-за чего она отвалилась
/// (сценарий чёрного ящика dust-on-slab). `destroy` клиенту не говорит
/// ничего — он для хода поршня, где о месте говорит сам ход.
fn pop(world: &mut World, pos: Pos) {
    world.note_destroyed(pos.0, pos.1, pos.2);
    set(world, pos, AIR);
}

/// Отвал по обновлению формы: у оригинала блок при этом ломается «как
/// положено» — с событием слома (частицы и звук, 2001 с его состоянием;
/// сценарий water-washes: трава без опоры). По обновлению соседа и при
/// постановке командой отвал тихий (сценарий dust-on-slab).
fn pop_breaking(world: &mut World, pos: Pos) {
    let state = world.get_block(pos.0, pos.1, pos.2);

    world.note_level_event(pos.0, pos.1, pos.2, 2001, state);
    pop(world, pos);
}

/// Раз во сколько тактов нажатая плита пересчитывает, кто на ней: обычная —
/// раз в 20, взвешенная — раз в 10 (вики, Pressure Plate: «checks every 20
/// game ticks» / «every 10 game ticks»).
const PLATE_RELEASE: u64 = 20;
const WEIGHTED_RELEASE: u64 = 10;

/// Докуда над низом клетки плита чувствует сущности (доля блока).
const PLATE_REACH: f64 = 0.25;

/// Сколько сущностей лежит на каждой плите: плита тонкая, и стоящий на ней
/// находится в её же клетке. Каменную плиту и плиту из полированного
/// чернокамня нажимают только живые: «activated only by mobs, players, and
/// armor stands» — не предметы, стрелы, вагонетки (вики, Pressure Plate).
fn plate_counts(world: &World, occupants: &[Occupant]) -> HashMap<Pos, u32> {
    let mut counts: HashMap<Pos, u32> = HashMap::new();

    for occupant in occupants {
        let pos = (occupant.x.floor() as i32, occupant.y.floor() as i32, occupant.z.floor() as i32);
        let plate = Block::at(world, pos);

        // Плиту нажимает только то, что задевает её нижнюю четверть:
        // предмет, который выбрасыватель подкинул вверх из клетки плиты,
        // жмёт её, лишь упав обратно (чёрный ящик, plate-items).
        // [проверить]: точные границы области у оригинала.
        if plate.kind == Kind::Plate
            && (occupant.living || !only_living(plate.name))
            && occupant.crosses(pos, 0.0, PLATE_REACH)
        {
            *counts.entry(pos).or_insert(0) += 1;
        }
    }

    counts
}

/// Через сколько тактов плита пересчитывает, кто на ней.
fn plate_period(block: &Block) -> u64 {
    if is_weighted(block.name) {
        WEIGHTED_RELEASE
    } else {
        PLATE_RELEASE
    }
}

/// Нажимает ненажатые плиты, на которые кто-то встал, — в фазе сущностей,
/// по местам после шага предметов: предмет жмёт плиту в тот такт, когда
/// на неё упал (чёрный ящик, plate-items, plate-weights).
///
/// Нажатая плита на сущности уже не смотрит: кто на ней, она пересчитывает
/// своим тактом раз в 10 или 20 тактов (`plate_ticked`) — там же меняется
/// сила взвешенной и там же плита отпускается. Сила взвешенной поэтому
/// меняется не в тот такт, когда на неё легли новые предметы, а на ближайшем
/// пересчёте; и отпускается плита на пересчёте, а не через срок после ухода
/// (чёрный ящик, plate-weights).
pub fn step_plates(world: &mut World, occupants: &[Occupant]) {
    // Оригинал проверяет весь путь сущности за шаг, а у нас есть только
    // концы: где она была до шага (записано фазой сущностей) и где стала.
    // Предмет, который выбрасыватель выкинул из клетки плиты вверх, жмёт
    // её уже в такт появления, а упавший обратно — когда упал (чёрный ящик,
    // plate-items: у оригинала бывает и так, и так). На плиту берём
    // большее из двух чисел, чтобы лежащее не считалось дважды.
    let before = world.redstone.occupants.clone();
    let mut counts = plate_counts(world, occupants);

    for (pos, count) in plate_counts(world, &before) {
        let now = counts.entry(pos).or_insert(0);
        *now = (*now).max(count);
    }
    let mut pressed: Vec<(&Pos, &u32)> = counts.iter().collect();

    pressed.sort();

    for (pos, count) in pressed {
        let block = Block::at(world, *pos);

        if plate_power(&block) > 0 {
            continue;
        }

        if let Some(state) = pressed_state(&block, *count)
            && state != block.state
        {
            plate_click(world, *pos, &block, state);
            set(world, *pos, state);
            world.schedule_once(pos.0, pos.1, pos.2, plate_period(&block), PRIORITY_NORMAL);
        }
    }
}

/// Такт нажатой плиты: пересчитать, кто на ней (по местам к началу такта),
/// сменить силу или отпустить; пока нажата — следующий пересчёт.
fn plate_ticked(world: &mut World, pos: Pos, block: &Block) {
    let occupants = world.redstone.occupants.clone();
    let count = plate_counts(world, &occupants).get(&pos).copied().unwrap_or(0);

    if let Some(state) = pressed_state(block, count)
        && state != block.state
    {
        plate_click(world, pos, block, state);
        set(world, pos, state);
    }

    if count > 0 {
        world.schedule_once(pos.0, pos.1, pos.2, plate_period(block), PRIORITY_NORMAL);
    }
}


/// Щелчок плиты, когда её нажали или отпустили: звук раньше смены блока,
/// как у кнопки. У оригинала деревянная плита под упавшим предметом
/// щёлкает «click_on» с громкостью 1 и высотой 1 (сценарий plate-items), —
/// не 0,3 и 0,8, как в таблице вики. Отпускание и прочие плиты — так же.
/// [проверить]: громкость и высота отпускания и каменной, медной плит.
fn plate_click(world: &mut World, pos: Pos, before: &Block, after: i32) {
    let mut now = *before;
    now.state = after;

    let (was, is) = (plate_power(before) > 0, plate_power(&now) > 0);

    if was == is {
        return;
    }

    let name = before.name;
    let (on, off) = if is_weighted(name) {
        ("minecraft:block.metal_pressure_plate.click_on", "minecraft:block.metal_pressure_plate.click_off")
    } else if only_living(name) {
        ("minecraft:block.stone_pressure_plate.click_on", "minecraft:block.stone_pressure_plate.click_off")
    } else if name.starts_with("cherry") {
        ("minecraft:block.cherry_wood_pressure_plate.click_on", "minecraft:block.cherry_wood_pressure_plate.click_off")
    } else if name.starts_with("bamboo") {
        ("minecraft:block.bamboo_wood_pressure_plate.click_on", "minecraft:block.bamboo_wood_pressure_plate.click_off")
    } else if name.starts_with("crimson") || name.starts_with("warped") {
        ("minecraft:block.nether_wood_pressure_plate.click_on", "minecraft:block.nether_wood_pressure_plate.click_off")
    } else {
        ("minecraft:block.wooden_pressure_plate.click_on", "minecraft:block.wooden_pressure_plate.click_off")
    };

    world.note_sound(pos.0, pos.1, pos.2, if is { on } else { off }, 1.0, 1.0);
    game_event(world, pos, if is { FREQ_ACTIVATE } else { FREQ_DEACTIVATE });
}

/// Плита, которую нажимают только живые.
fn only_living(name: &str) -> bool {
    matches!(name, "stone_pressure_plate" | "polished_blackstone_pressure_plate")
}

/// Кто где стоит в этом такте: игроки и то, что лежит в мире. Такт мира
/// зовёт это в начале, до запланированных тактов, — растяжка в своём такте
/// проверяет, не ушли ли с неё.
pub fn note_occupants(world: &mut World, occupants: Vec<Occupant>) {
    world.redstone.occupants = occupants;
}

/// Фаза сущностей для редстоуна: плиты, растяжки, руда под ногами.
pub fn step_entities(world: &mut World) {
    components::step_entities(world);
    rails::press_detectors(world);
}

/// Рамка, как её видит сравнитель: клетка, сторона (номера игры, frame.rs) и
/// сила — поворот плюс один, у пустой ноль.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameMark {
    pub pos: Pos,
    pub facing: u8,
    pub power: u8,
}

/// Какие рамки есть и что они показывают сравнителю. Зовут при каждой
/// перемене рамки и в каждом такте мира.
pub fn note_frames(world: &mut World, frames: Vec<FrameMark>) {
    world.redstone.frames = frames;
}

/// Сила рамки в клетке `pos`, смотрящей в сторону `facing` (номера игры):
/// что о ней знает редстоун. None — рамки нет.
pub fn frame_power(world: &World, pos: Pos, facing: u8) -> Option<u8> {
    world.redstone.frames.iter().find(|mark| mark.pos == pos && mark.facing == facing).map(|mark| mark.power)
}

/// Что читает сравнитель, глядящий в сторону `facing`, с рамки в клетке
/// `pos`: рамка должна смотреть туда же, куда смотрит сравнитель — от него
/// (вики, Redstone Comparator: «the comparator must be placed behind this
/// block, facing away from the item frame»).
fn frame_reading(world: &World, pos: Pos, facing: Dir) -> Option<u8> {
    let number = match facing {
        Dir::Down => crate::frame::DOWN,
        Dir::Up => crate::frame::UP,
        Dir::North => crate::frame::NORTH,
        Dir::South => crate::frame::SOUTH,
        Dir::West => crate::frame::WEST,
        Dir::East => crate::frame::EAST,
    };

    frame_power(world, pos, number)
}

/// Где стоят вагонетки в этом такте — для детекторных рельс. Такт мира
/// зовёт это вместе с `note_occupants`.
pub fn note_carts(world: &mut World, carts: Vec<Cart>) {
    world.redstone.carts = carts;
}

/// Каким должно быть состояние плиты, когда на ней столько всего.
///
/// У обычной плиты только «нажата» или нет. У золотой сила равна числу
/// предметов, у железной — одна единица за каждый десяток.
fn pressed_state(block: &Block, count: u32) -> Option<i32> {
    if block.kind != Kind::Plate {
        return None;
    }

    if !is_weighted(block.name) {
        return block.with("powered", yes_no(count > 0));
    }

    let power = if block.name.starts_with("heavy") {
        count.div_ceil(10)
    } else {
        count
    };

    block.with("power", &power.min(MAX_POWER as u32).to_string())
}

/// Разбирает всё, что изменилось: сообщает соседям, те меняются сами,
/// и так до конца. Звать после любого изменения блоков и на каждом такте.
pub fn settle(world: &mut World) {
    settle_from(world, 0);
}

fn settle_from(world: &mut World, depth: usize) {
    if depth > MAX_DEPTH {
        // Слишком глубоко — остаток разберётся при следующем вызове.
        return;
    }

    while let Some((pos, old)) = world.pop_changed() {
        block_changed(world, pos, old, depth);
    }
}

/// Блок в этом месте изменился: сообщаем соседям в порядке игры, и каждого
/// разбираем до конца, прежде чем перейти к следующему.
///
/// `old` — что стояло здесь раньше: снятая деталь тоже должна предупредить
/// тех, кого она питала.
fn block_changed(world: &mut World, pos: Pos, old: i32, depth: usize) {
    let new_block = Block::at(world, pos);
    let old_block = Block {
        state: old,
        name: blocks::block_at_state(old).unwrap_or("air"),
        kind: kind_of(blocks::block_at_state(old).unwrap_or("air")),
        orientation: blocks::orientation(blocks::block_at_state(old).unwrap_or("air")),
    };

    // Сменилось только состояние того же блока — например, лампа загорелась
    // или дверь открылась. Часть деталей при этом соседей не предупреждает
    // вовсе: на вики они перечислены отдельным списком «Some changes do not
    // produce General NC updates». На этом стоят «определители обновления
    // блока», поэтому список надо соблюдать.
    let same_block = old_block.name == new_block.name;

    // Команда, сменившая только состояние того же блока, просто заменяет
    // его: соседям — общее «сосед изменился» и обновление формы, но ничего
    // из того, что деталь рассылает сама при своей смене (чёрный ящик,
    // rail-activator: пыль-крестик с силой 0 так и остаётся с нулём).
    let commanded = world.take_commanded(pos);
    let replaced = commanded && same_block;
    let quiet = same_block && !replaced && keeps_quiet(new_block.kind);

    // Порядок рассылки при одном изменении — по таблице «Priorities» на
    // вики (Block update): особые обновления снятого блока (−2), потом
    // поставленного (−1), общее «сосед изменился» (0), обновления
    // сравнителей (1), обновление формы (2), поздние (3). Каждое
    // обновление разбирается до конца, прежде чем пойдёт следующее (обход
    // в глубину).

    // Поставленный блок сперва приспосабливается к окружению сам — это не
    // обновление, а часть постановки (вики, Block update: «Strictly
    // speaking, this isn't a block update»).
    if !same_block {
        // Пыль, поставленная командой, форму не подбирает — остаётся такой,
        // какой её дали, и пересчитывает только силу: поставленная концом
        // в пустоту так и смотрит туда, и когда там появится провод, сосед
        // ничего не шлёт (чёрный ящик, dust-join).
        if commanded && new_block.kind == Kind::Wire {
            react(world, pos, false, false);
        } else {
            itself_changed(world, pos);
        }

        settle_from(world, depth + 1);
    }

    // −2 и −1: особые обновления снятого и поставленного блока (пыль,
    // факел, диоды, наблюдатель — у поставленного и при смене состояния
    // факела и диода).
    let mut special = Vec::new();

    if !same_block {
        special.extend(early_reach(&old_block, pos));
        special.extend(early_reach(&new_block, pos));
    } else if !replaced && matches!(new_block.kind, Kind::Torch | Kind::Repeater | Kind::Comparator | Kind::Observer) {
        special.extend(early_reach(&new_block, pos));
    }

    // Обычная рельса слушает только обновления от деталей, дающих сигнал
    // (вики, Block update: «a rail accepts only NC updates from power
    // components»). У особых обновлений источник — та деталь, чьи они; у
    // общего — поставленный.
    let special_signal = gives_signal(old_block.kind) || gives_signal(new_block.kind);

    for far in special {
        if far != pos {
            neighbour_changed(world, far, special_signal);
            settle_from(world, depth + 1);
        }
    }

    // 0: общее «сосед изменился» шести соседям: з, в, н, вх, с, ю.
    // Источник — поставленный блок: снятый блок редстоуна развилку не
    // переключает обратно (чёрный ящик, rail-shapes).
    let signal = gives_signal(new_block.kind);

    if !quiet {
        for dir in NEIGHBOUR_ORDER {
            neighbour_changed(world, step(pos, dir), signal);
            settle_from(world, depth + 1);
        }
    }

    // Растяжка: поставили, порвали, задели или отпустили — сперва соседи
    // (выше), потом крючки к югу и западу (вики, String: «the tripwire first
    // updates its immediate neighbors, and then searches to the south and
    // west»). Порванная не ножницами — для крючков ещё задета: импульс.
    // Смена `attached` и `disarmed` крючков не будит.
    let wire_moved = if same_block {
        new_block.kind == Kind::Tripwire && old_block.is("powered", "true") != new_block.is("powered", "true")
    } else {
        old_block.kind == Kind::Tripwire || new_block.kind == Kind::Tripwire
    };

    if wire_moved {
        let broken = !same_block && old_block.kind == Kind::Tripwire && !old_block.is("disarmed", "true");

        components::wake_hooks(world, pos, broken);
        settle_from(world, depth + 1);
    }

    // Сняли крючок: второй крючок цепи узнаёт, что цепь порвана.
    // [проверить]: в какой момент рассылки это происходит у оригинала.
    if !same_block && old_block.kind == Kind::Hook {
        components::hook_removed(world, pos, &old_block);
        settle_from(world, depth + 1);
    }

    // 2: обновление формы шести соседям: з, в, с, ю, н, вх. Шлётся при
    // любой смене — и у тех, кто общего «сосед изменился» не шлёт
    // (поэтому наблюдатель видит, как зажглась лампа).
    for dir in SHAPE_ORDER {
        shape_changed(world, step(pos, dir), dir.opposite());
        settle_from(world, depth + 1);
    }

    // У пыли — ещё и блокам над и под тем, куда она смотрит вбок, если там
    // не пыль (так пыль узнаёт о пыли на ступеньку выше и ниже), кроме
    // наблюдателя (вики, Block update, исключение для redstone wire).
    for wire in [&old_block, &new_block] {
        if wire.kind != Kind::Wire {
            continue;
        }

        for dir in HORIZONTAL {
            if wire.is(dir.name(), "none") {
                continue;
            }

            let beside = step(pos, dir);

            if Block::at(world, beside).kind == Kind::Wire {
                continue;
            }

            for (vertical, from) in [(Dir::Up, Dir::Down), (Dir::Down, Dir::Up)] {
                let target = step(beside, vertical);

                if Block::at(world, target).kind != Kind::Observer {
                    shape_changed(world, target, from);
                    settle_from(world, depth + 1);
                }
            }
        }
    }

    // 3: поздние — включение рычага, кнопки, плиты (соседи опоры), смена
    // силы пыли (соседи семи опорных точек), сундук-ловушка; рельса с
    // питанием и детекторная, у которых сменилось питание.
    if same_block && !replaced {
        let mut late = late_reach(&new_block, pos);

        if old_block.is("powered", "true") != new_block.is("powered", "true") {
            match new_block.kind {
                Kind::PoweredRail => late.extend(rails::power_reach(&new_block, pos)),
                Kind::DetectorRail => late.extend(rails::detector_reach(pos)),
                _ => {}
            }
        }

        let signal = gives_signal(new_block.kind);

        for far in late {
            neighbour_changed(world, far, signal);
            settle_from(world, depth + 1);
        }
    }
}

/// Даёт ли деталь такого вида сигнал — от таких обычная рельса принимает
/// «сосед изменился» (вики, Block update). Пыль, диоды и наблюдатель тоже
/// здесь: развилку переключают и ими. [проверить]: вики говорит «power
/// components», а пыль и диоды у неё — передатчики.
fn gives_signal(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Wire
            | Kind::Torch
            | Kind::Lever
            | Kind::Button
            | Kind::Plate
            | Kind::RedstoneBlock
            | Kind::Repeater
            | Kind::Comparator
            | Kind::Observer
            | Kind::TrappedChest
            | Kind::Target
            | Kind::Daylight
            | Kind::Hook
            | Kind::LightningRod
            | Kind::DetectorRail
            | Kind::Sculk
            | Kind::Jukebox
            | Kind::Lectern
    )
}

/// Порядок обновления формы: з, в, с, ю, н, вх (вики, Block update).
const SHAPE_ORDER: [Dir; 6] = [Dir::West, Dir::East, Dir::North, Dir::South, Dir::Down, Dir::Up];

/// Особые обновления при постановке и сломе (и при смене состояния
/// факела и диода) — «−2/−1» в таблице вики.
fn early_reach(block: &Block, pos: Pos) -> Vec<Pos> {
    match block.kind {
        Kind::Wire | Kind::Torch | Kind::Repeater | Kind::Comparator | Kind::Observer => reach_of(block, pos),
        // Снятые (или поставленные) рычаг, кнопка, плита и ловушка
        // предупреждают соседей опоры сразу.
        Kind::Lever | Kind::Button | Kind::Plate | Kind::TrappedChest | Kind::Hook | Kind::LightningRod | Kind::Lectern
            if is_on(block) || block.kind == Kind::TrappedChest =>
        {
            reach_of(block, pos)
        }
        // Снятая включённая детекторная рельса — соседей блока под собой,
        // который она сильно питала.
        Kind::DetectorRail if is_on(block) => rails::detector_reach(pos),
        Kind::Sculk if is_on(block) => sculk::reach(pos),
        _ => Vec::new(),
    }
}

/// Поздние обновления при смене состояния — «3» в таблице вики.
fn late_reach(block: &Block, pos: Pos) -> Vec<Pos> {
    let around = |centre: Pos| NEIGHBOUR_ORDER.iter().map(move |dir| step(centre, *dir));

    match block.kind {
        // Смена силы пыли: соседи семи опорных точек — самой пыли и её
        // шести соседей, 42 обновления (вики, Block update). Порядок точек у
        // оригинала зависит от места в мире; здесь — сама пыль, потом
        // соседи по общему порядку.
        Kind::Wire => std::iter::once(pos)
            .chain(around(pos))
            .flat_map(around)
            .collect(),
        Kind::Lever | Kind::Button | Kind::Plate | Kind::TrappedChest | Kind::Hook | Kind::LightningRod | Kind::Lectern => {
            reach_of(block, pos)
        }
        Kind::Sculk => sculk::reach(pos),
        _ => Vec::new(),
    }
}

/// Молчит ли деталь такого вида, когда у неё меняется только состояние.
///
/// Вики перечисляет их поимённо: повторитель (загорелся или погас),
/// сравнитель (сменился сигнал или режим), лампа (загорелась или погасла),
/// дверь, люк и калитка (открылись, закрылись, включились, выключились),
/// нажимная плита (сменился сигнал). У повторителя, сравнителя и плиты
/// есть своя, особая рассылка — она ниже, в reach_of; у лампы, двери и люка
/// нет никакой.
fn keeps_quiet(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Repeater
            | Kind::Comparator
            | Kind::Lamp
            | Kind::Door
            | Kind::Trapdoor
            | Kind::Gate
            | Kind::Plate
            | Kind::Observer
            | Kind::Wire
            | Kind::Dispenser
            | Kind::Hopper
    )
}

/// Кого ещё, кроме соседей, предупреждает изменившийся блок такого вида.
///
/// Провод и факел — соседей своих соседей: сила провода видна через блок,
/// а факел питает блок над собой. Повторитель и сравнитель — блок, в который
/// смотрят, и его соседей. Рычаг и кнопка — соседей блока, к которому
/// прикреплены. Плита — соседей блока под собой.
fn reach_of(block: &Block, pos: Pos) -> Vec<Pos> {
    let around = |centre: Pos| NEIGHBOUR_ORDER.iter().map(move |dir| step(centre, *dir));

    match block.kind {
        Kind::Wire | Kind::Torch => [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East]
            .into_iter()
            .flat_map(|first| around(step(pos, first)))
            .collect(),

        // Наблюдатель предупреждает то, во что выдаёт сигнал, — блок
        // у себя за спиной, и его соседей.
        Kind::Observer => match block.facing() {
            Some(facing) => {
                let behind = step(pos, facing.opposite());

                std::iter::once(behind).chain(around(behind)).collect()
            }
            None => Vec::new(),
        },

        Kind::Repeater | Kind::Comparator => match output_dir(block) {
            Some(output) => {
                let ahead = step(pos, output);

                std::iter::once(ahead).chain(around(ahead)).collect()
            }
            None => Vec::new(),
        },

        // Крючок и громоотвод — как кнопка: соседи блока, на котором висят.
        Kind::Lever | Kind::Button | Kind::Hook | Kind::LightningRod => match attached_to(block, pos) {
            Some(attached) => around(attached).collect(),
            None => Vec::new(),
        },

        // Плита предупреждает своих соседей и соседей блока, на котором
        // лежит. Своих — здесь же: общего оповещения у неё нет.
        Kind::Plate => around(pos).chain(around(step(pos, Dir::Down))).collect(),

        // Сундук-ловушка сильно питает блок под собой — его соседи тоже
        // должны узнать о смене силы.
        // Кафедра — так же, импульсом.
        Kind::TrappedChest | Kind::Lectern => around(step(pos, Dir::Down)).collect(),

        _ => Vec::new(),
    }
}

/// В громоотвод ударила молния (lightning.rs): 8 тактов сигнала.
pub fn lightning_struck(world: &mut World, pos: Pos) {
    components::strike(world, pos);
}

/// Обновление сравнителей: у блока в этом месте сменилось то, что они
/// читают (содержимое хранилища). Будятся четыре соседа по бокам в порядке
/// север, восток, юг, запад — сравнитель рядом или за одним проводящим
/// блоком (вики, Block update, «Comparator update»).
pub fn comparator_update(world: &mut World, pos: Pos) {
    for dir in [Dir::North, Dir::East, Dir::South, Dir::West] {
        let beside = step(pos, dir);
        let block = Block::at(world, beside);

        if block.kind == Kind::Comparator {
            neighbour_changed(world, beside, false);
        } else if block.conductive() {
            let beyond = step(beside, dir);

            if Block::at(world, beyond).kind == Kind::Comparator {
                neighbour_changed(world, beyond, false);
            }
        }
    }

    settle(world);
}

/// У блока в этом месте изменился сосед: решаем, что с ним делать.
/// `from_signal` — изменилась ли деталь, дающая сигнал (это слушает
/// обычная рельса).
fn neighbour_changed(world: &mut World, pos: Pos, from_signal: bool) {
    react(world, pos, false, from_signal);
}

/// Блок только что поставлен или сменился сам. Разбирается как «сосед
/// изменился», кроме растений: у оригинала растение проверяет опору, лишь
/// когда меняется сосед, — трава, поставленная командой на камень, стоит,
/// пока её не тронут (сценарий чёрного ящика grass-crush).
fn itself_changed(world: &mut World, pos: Pos) {
    react(world, pos, true, false);
}

fn react(world: &mut World, pos: Pos, itself: bool, from_signal: bool) {
    let block = Block::at(world, pos);

    // Полка: ток — `powered` и сцепка с соседками (redstone/shelf.rs).
    // Поставленная командой питание сама не проверяет. [проверить]
    if block.kind == Kind::Other && shelf::is_shelf(block.name) {
        if !itself {
            shelf::react(world, pos);
        }

        return;
    }

    match block.kind {
        // Пыль на «сосед изменился» пересчитывает только силу; форму — по
        // обновлению формы, а поставленная — и то и другое сразу.
        Kind::Wire => {
            if !supported(world, pos, &block) {
                pop(world, pos);
                return;
            }

            let wanted = if itself {
                wanted_wire(world, pos, &block)
            } else {
                block.with("power", &wire_power(world, pos).to_string())
            };

            if let Some(wanted) = wanted
                && wanted != block.state
            {
                set(world, pos, wanted);
            }
        }

        // Факел на «сосед изменился» только сверяет свет; опору — по
        // обновлению формы.
        Kind::Torch => {
            if itself && !supported(world, pos, &block) {
                pop(world, pos);
                return;
            }

            if torch_should_be_lit(world, pos, &block) != is_on(&block) {
                world.schedule_once(pos.0, pos.1, pos.2, TORCH_DELAY, PRIORITY_NORMAL);
            }
        }

        // Диод при своей установке опору не проверяет — только когда
        // меняется сосед (повторитель, поставленный командой на нижнюю
        // плиту, стоит до первого обновления: сценарий dust-on-slab).
        Kind::Repeater => {
            if !itself && !supported(world, pos, &block) {
                pop(world, pos);
                return;
            }

            // Признак «заперт» меняется по обновлению формы (вики, Block
            // update: «Redstone repeater — lock check» среди откликов на PP);
            // здесь — только живая проверка: запертый не слушает ничего.
            let locked = repeater_locked(world, pos, &block);

            if itself
                && locked != block.is("locked", "true")
                && let Some(state) = block.with("locked", yes_no(locked))
            {
                set(world, pos, state);
            }

            if locked {
                return;
            }

            let powered = is_on(&block);
            let input = diode_input(world, pos, &block) > 0;

            if powered != input {
                let priority = if faces_diode(world, pos, &block) {
                    PRIORITY_INTO_DIODE
                } else if powered {
                    PRIORITY_DEPOWERING
                } else {
                    PRIORITY_DIODE
                };

                world.schedule_once(pos.0, pos.1, pos.2, repeater_delay(&block), priority);
            }
        }

        Kind::Comparator => {
            if !itself && !supported(world, pos, &block) {
                pop(world, pos);
                return;
            }

            let wanted = comparator_output(world, pos, &block);
            let current = comparator_current(world, pos, &block);

            if wanted != current || (wanted > 0) != is_on(&block) {
                let priority = if faces_diode(world, pos, &block) {
                    PRIORITY_DIODE
                } else {
                    PRIORITY_NORMAL
                };

                world.schedule_once(pos.0, pos.1, pos.2, COMPARATOR_DELAY, priority);
            }
        }

        // Медная лампа переключается по приходу сигнала: пришёл — сменила
        // свет на обратный. «When a copper bulb first receives a redstone
        // signal, the bulb becomes lit ... or if the bulb was already lit,
        // then it becomes unlit.» Задержки у неё нет.
        Kind::Bulb => {
            let powered = any_power(world, pos);

            if powered == block.is("powered", "true") {
                return;
            }

            let mut state = block.with("powered", yes_no(powered));

            // Свет меняется только в тот миг, когда сигнал приходит.
            if powered && let Some(current) = state {
                state = block
                    .orientation
                    .and_then(|kind| kind.with(current, "lit", yes_no(!block.is("lit", "true"))));
            }

            // Сменила свет — щелчок `turn_on` или `turn_off`, громкость
            // и высота 1, раньше нового блока (чёрный ящик, copper-bulb).
            if powered {
                let sound = if block.is("lit", "true") {
                    "minecraft:block.copper_bulb.turn_off"
                } else {
                    "minecraft:block.copper_bulb.turn_on"
                };

                world.note_sound(pos.0, pos.1, pos.2, sound, 1.0, 1.0);
            }

            if let Some(state) = state {
                set(world, pos, state);
            }
        }

        // Музыкальный блок играет ноту на приходе сигнала — один раз, по
        // фронту, а не всё время, пока питание есть. Заодно он пересчитывает
        // инструмент: тот задаётся блоком прямо под ним и мог смениться.
        Kind::NoteBlock => {
            let powered = any_power(world, pos);
            let was = block.is("powered", "true");
            let instrument = instrument_of(world, pos);

            let mut state = block.with("powered", yes_no(powered)).unwrap_or(block.state);

            if let Some(kind) = block.orientation
                && let Some(next) = kind.with(state, "instrument", instrument)
            {
                state = next;
            }

            // Сперва новое состояние блока, потом нота — так у оригинала.
            // Нота — блочное событие: звучит в своей фазе такта, после общей
            // рассылки блоков (сценарии note-heads, note-pitch).
            if state != block.state {
                set(world, pos, state);
            }

            if powered && !was {
                world.queue_block_event(pos.0, pos.1, pos.2, NOTE_PLAY, 0);
            }
        }

        // Лампа загорается сразу, а гаснет через четыре такта: если сигнал
        // за это время вернётся, она даже не мигнёт.
        // Лампа, поставленная командой, питание сама не проверяет — только
        // когда меняется сосед (сценарий чёрного ящика torch-inverter).
        Kind::Lamp if itself => {}

        Kind::Lamp => {
            let powered = any_power(world, pos);
            let lit = block.is("lit", "true");

            if powered && !lit {
                if let Some(state) = block.with("lit", "true") {
                    set(world, pos, state);
                }
            } else if !powered && lit {
                world.schedule_once(pos.0, pos.1, pos.2, LAMP_DELAY, PRIORITY_NORMAL);
            }
        }

        Kind::Door | Kind::Trapdoor | Kind::Gate => {
            let powered = any_power(world, pos)
                || (block.kind == Kind::Door && door_other_half_powered(world, pos, &block));

            if powered != block.is("powered", "true") {
                let opened = block
                    .with("powered", yes_no(powered))
                    .and_then(|state| block.orientation?.with(state, "open", yes_no(powered)));

                // Открылась или закрылась от сигнала — звук и вибрация
                // `block_open` или `block_close`, если `open` и правда
                // сменился. Звук раньше, чем новый блок (чёрный ящик,
                // door-lever, trapdoor, fence-gate).
                if block.is("open", "true") != powered {
                    let pitch = 0.9 + world.random_unit() * 0.1;

                    world.note_sound(pos.0, pos.1, pos.2, opening_sound(&block, powered), 1.0, pitch);
                }

                if let Some(state) = opened {
                    set(world, pos, state);

                    // Вторая половина двери повторяет первую: у оригинала
                    // она подстраивается по обновлению формы и уходит
                    // клиенту той же пачкой (чёрный ящик, door-lever).
                    if block.kind == Kind::Door {
                        door_copy_to_other_half(world, pos, &block, state);
                    }
                }

                if block.is("open", "true") != powered {
                    game_event(world, pos, if powered { FREQ_ACTIVATE } else { FREQ_DEACTIVATE });
                }
            }
        }

        // Рычаг, кнопка и плита опору проверяют по обновлению формы.
        Kind::Lever | Kind::Button | Kind::Plate => {
            if itself && !supported(world, pos, &block) {
                pop(world, pos);
            }
        }

        // Крючок: поставленный проверяет опору и собирает цепь; на соседей
        // потом не смотрит — его будят растяжки.
        // Крючок проверяет опору; цепь он собирает, только когда его
        // ставит игрок (`placed_by_player`): поставленный командой у
        // оригинала остаётся вне цепи (сценарий tripwire-hook-break). Потом
        // его будят растяжки.
        Kind::Hook => {
            if itself && !supported(world, pos, &block) {
                pop(world, pos);
            }
        }

        // Голова: от сигнала у неё только `powered` (сценарий note-heads —
        // голова над запитанным музыкальным блоком).
        Kind::Head => {
            let powered = any_power(world, pos);

            if block.is("powered", yes_no(!powered))
                && let Some(state) = block.with("powered", yes_no(powered))
            {
                set(world, pos, state);
            }
        }

        // Поставленная растяжка тянется к соседям; крючки будит
        // block_changed.
        Kind::Tripwire => {
            if itself
                && let Some(state) = components::wire_shape(world, pos, &block)
                && state != block.state
            {
                set(world, pos, state);
            }
        }

        // Колокол звонит по фронту сигнала.
        Kind::Bell => components::bell_changed(world, pos, &block),

        // Командный блок запоминает сигнал; на подъёме заводит такт.
        // Поставленный командой сам сигнал не проверяет [проверить].
        Kind::CommandBlock => {
            if !itself {
                crate::command_block::react(world, pos, any_power(world, pos));
            }
        }

        // Рельсы: поставленная берёт форму и перестраивает соседок, на
        // «сосед изменился» — опора, развилка, питание.
        Kind::Rail | Kind::PoweredRail | Kind::DetectorRail => {
            if itself {
                rails::placed(world, pos, &block);
            } else {
                rails::neighbour_changed(world, pos, &block, from_signal);
            }
        }

        // Динамит загорается от любого сигнала рядом — и поставленный рядом
        // с работающим источником тоже, сразу (вики, TNT: «activated when it
        // receives a redstone signal»). Квазисвязности нет. [проверить]
        // чёрным ящиком: в тот же такт при установке (tnt-lever).
        Kind::Tnt => {
            if any_power(world, pos) {
                crate::tnt::ignite(world, pos);
            }
        }

        // Мишень, датчик света и громоотвод на соседей не смотрят: «Block
        // updates have no effect on daylight detectors».
        Kind::Target | Kind::Daylight | Kind::LightningRod | Kind::Sculk | Kind::Jukebox | Kind::Lectern => {}

        Kind::Piston => {
            // Голову сломали — основание остаётся стоять выдвинутым.
            // Так в игре с беты 1.7_01: «Headless pistons do not break upon
            // receiving a block update anymore». Такой поршень больше ничего
            // не делает, пока его не сломают.
            if block.is("extended", "true")
                && !head_in_front(world, pos, &block)
                && !world.redstone.moving.contains_key(&pos)
            {
                return;
            }

            // Как у оригинала: поршень, решив сменить состояние, заводит
            // блочное событие своего вида — «выдвинуться» или
            // «задвинуться»; оба вида могут стоять в очереди разом (так
            // работают 0-тактовые импульсы).
            let wanted = piston_powered(world, pos, &block);
            let extended = block.is("extended", "true");

            if wanted && !extended {
                world.queue_block_event(pos.0, pos.1, pos.2, PISTON_EXTENDS, 0);
            } else if !wanted && extended {
                world.queue_block_event(pos.0, pos.1, pos.2, PISTON_RETRACTS, 0);
            }
        }

        // Голова передаёт «сосед изменился» основанию: поршень замечает
        // перемены и у своей головы (сценарий piston-bud: убрали блок
        // редстоуна над головой — оригинал задвинул поршень сразу). Снимается
        // голова только обновлением формы сзади (см. `shape_changed`).
        Kind::PistonHead => {
            if let Some(facing) = block.facing() {
                let base = step(pos, facing.opposite());

                if Block::at(world, base).kind == Kind::Piston {
                    neighbour_changed(world, base, from_signal);
                }
            }
        }

        // Служебный блок на месте едущего ни на что не смотрит: он живёт
        // ровно столько, сколько идёт ход, и убирается вместе с ним.
        Kind::MovingPiston => {}

        // Сундук сам на соседей не смотрит; сундук-ловушка — источник.
        Kind::Chest | Kind::TrappedChest => {}

        // Выбрасыватель и раздатчик: по фронту сигнала — `triggered` и
        // такт через 4; сигнал пропал — `triggered` снимается. Слушают и
        // клетку над собой (вики, Dropper: квазисвязность).
        Kind::Dispenser => {
            let powered = any_power(world, pos) || any_power(world, step(pos, Dir::Up));
            let triggered = block.is("triggered", "true");

            if powered && !triggered {
                if let Some(state) = block.with("triggered", "true") {
                    set(world, pos, state);
                }

                world.schedule_once(pos.0, pos.1, pos.2, containers::DISPENSE_DELAY, PRIORITY_NORMAL);
            } else if !powered
                && triggered
                && let Some(state) = block.with("triggered", "false")
            {
                set(world, pos, state);
            }
        }

        // Крафтер: как выбрасыватель, но без квазисвязности (вики, Crafter).
        Kind::Crafter => crafter::neighbour_changed(world, pos, any_power(world, pos)),

        // Воронка: любой сигнал её запирает (`enabled=false`),
        // квазисвязности нет (вики, Hopper).
        Kind::Hopper => {
            let enabled = !any_power(world, pos);

            if block.is("enabled", yes_no(!enabled))
                && let Some(state) = block.with("enabled", yes_no(enabled))
            {
                set(world, pos, state);
            }
        }

        // Наблюдатель на соседей не смотрит: его будит только изменение
        // блока прямо перед глазами, и делает это block_changed.
        Kind::RedstoneBlock | Kind::Observer | Kind::Other => {
            // Лава, которой коснулась вода, твердеет сразу.
            let hardened = {
                let view = |x, y, z| world.get_block(x, y, z);
                crate::fluids::hardens_now(pos, &view)
            };

            if let Some(state) = hardened {
                // Шипение застывшей лавы (событие мира 1501).
                world.note_level_event(pos.0, pos.1, pos.2, 1501, 0);
                set(world, pos, state);
                return;
            }

            // Песку и гравию нужна опора: поставленный без неё падает, но
            // не сразу, а своим тактом через два (как у оригинала, снято
            // чёрным ящиком: поставленный песок сперва виден на месте).
            // Потом опору он проверяет по обновлению формы.
            if itself && falls(block.state) && !holds_up(world.get_block(pos.0, pos.1 - 1, pos.2)) {
                world.schedule_once(pos.0, pos.1, pos.2, FALL_DELAY, PRIORITY_NORMAL);
            }
        }
    }
}

/// Блок поставлен командой (`setblock`, `fill`): как у оригинала, он сразу
/// подстраивается под соседей — получает обновление формы с каждой стороны
/// в порядке низ, верх, север, юг, запад, восток. Так наблюдатель,
/// поставленный командой лицом к блоку, срабатывает (сценарий
/// observer-clock), а пыль и заборы соединяются.
pub fn command_placed(world: &mut World, pos: Pos) {
    // Сверено только для наблюдателя: трава, поставленная командой на
    // камень, у оригинала стоит (grass-crush, water-washes), повторитель на
    // нижней плите — тоже (dust-on-slab). Прочим подстройка не нужна.
    if Block::at(world, pos).kind != Kind::Observer {
        return;
    }

    for from in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
        shape_update(world, pos, from, false);
    }
}

/// Обновление формы (PP) пришло к блоку в этом месте от соседа со стороны
/// `from`. На него отзываются те, кому нужна опора или кто смотрит на
/// соседей по форме (вики, Block update, «Post-placement update»).
fn shape_changed(world: &mut World, pos: Pos, from: Dir) {
    shape_update(world, pos, from, true);
}

/// Обновление формы; `loud` — ломается ли отвалившееся с событием слома
/// (при постановке командой — нет).
fn shape_update(world: &mut World, pos: Pos, from: Dir, loud: bool) {
    let block = Block::at(world, pos);
    let fall_off = |world: &mut World| if loud { pop_breaking(world, pos) } else { pop(world, pos) };

    // Половина кровати, у которой пропала пара (со стороны пары): ломается
    // с частицами (2001), без предмета (чёрный ящик, bed-respawn: изголовье
    // убрано командой — изножье ломается следом, предмета нет).
    if from.is_horizontal() && crate::bed::lost_partner(world, pos, from.offset()) {
        world.note_level_event(pos.0, pos.1, pos.2, 2001, block.state);
        set(world, pos, AIR);
        return;
    }

    match block.kind {
        // Пыль: держится ли, и куда тянется. Сила остаётся прежней.
        Kind::Wire => {
            if !supported(world, pos, &block) {
                fall_off(world);
                return;
            }

            let shape = wire_shape(world, pos, &block);
            let mut state = Some(block.state);

            for (dir, value) in shape {
                state = state.and_then(|state| block.orientation?.with(state, dir.name(), value));
            }

            if let Some(state) = state
                && state != block.state
            {
                set(world, pos, state);
            }
        }

        // Опору по форме проверяют факел, рычаг, кнопка, плита и крючок.
        // Диоды — нет: у них опора проверяется по «сосед изменился»
        // (повторитель, поставленный командой на нижнюю плиту, стоит до
        // обновления — сценарий dust-on-slab).
        Kind::Torch | Kind::Lever | Kind::Button | Kind::Plate | Kind::Hook => {
            if !supported(world, pos, &block) {
                fall_off(world);
            }
        }

        // Растяжка тянется к соседней растяжке и к крючку.
        Kind::Tripwire if from != Dir::Up && from != Dir::Down => {
            if let Some(state) = components::wire_shape(world, pos, &block)
                && state != block.state
            {
                set(world, pos, state);
            }
        }

        // Повторитель: замок.
        Kind::Repeater => {
            let locked = repeater_locked(world, pos, &block);

            if locked != block.is("locked", "true")
                && let Some(state) = block.with("locked", yes_no(locked))
            {
                set(world, pos, state);
            }
        }

        // Наблюдатель ловит обновления формы от того, во что смотрит (вики,
        // Observer: «detects shape updates coming from the block it's
        // facing»).
        // Такт, который должен сработать в этом такте, но ещё не сработал,
        // и для наблюдателя считается ожидающим: второй не заводится. Так
        // выходит запись оригинала (сценарий observer-clock: период часов
        // на двух наблюдателях — 6 тактов); исключения из MC-189954 в 26.1.2
        // нет.
        Kind::Observer => {
            if block.facing() == Some(from) {
                world.schedule_once(pos.0, pos.1, pos.2, OBSERVER_DELAY, PRIORITY_NORMAL);
            }
        }

        // Голова снимается обновлением формы сзади, если поршня там больше
        // нет (вики, Piston/Technical components).
        Kind::PistonHead => {
            if block.facing().map(Dir::opposite) == Some(from) && !piston_behind(world, pos, &block) {
                set(world, pos, AIR);
            }
        }

        // Половина сундука следит за соседом по форме: сосед-сундук того же
        // вида и стороны смотрит на неё своей половиной — она становится
        // парной; её пара пропала — снова одиночная (вики, Chest).
        Kind::Chest | Kind::TrappedChest if from.is_horizontal() => {
            let Some(facing) = block.facing() else {
                return;
            };
            let other = Block::at(world, step(pos, from));
            let kind = block.value("type").unwrap_or("single");
            // Сосед по часовой от стороны сундука — пара для «левой».
            let clockwise = facing.clockwise();
            let side_of = |dir: Dir| if dir == clockwise { "left" } else { "right" };
            // Медные сундуки в пару идут с любым медным, какой бы ни была
            // стадия (вики, Copper Chest).
            let copper = |name: &str| name.ends_with("copper_chest");
            let points_here = |other: &Block| {
                (other.name == block.name || (copper(other.name) && copper(block.name)))
                    && other.facing() == Some(facing)
                    && other.value("type") == Some(if side_of(from) == "left" { "right" } else { "left" })
            };

            // Смотрим только на бока: спереди и сзади пары не бывает.
            if from == facing || from == facing.opposite() {
                return;
            }

            // Пара медных сундуков разных стадий становится одним сундуком:
            // обе половины — наименее окисленной стадии, вощёные, только
            // если вощены обе (вики, Copper Chest; сценарий copper-chest).
            if kind != "single" && points_here(&other) && other.name != block.name && copper(block.name) {
                let merged = containers::copper_chest_merged(block.name, other.name);

                for (at, half) in [(pos, block), (step(pos, from), other)] {
                    if half.name != merged
                        && let Some(state) = blocks::orientation(&merged).and_then(|o| o.state(&[
                            ("facing", half.value("facing").unwrap_or("north")),
                            ("type", half.value("type").unwrap_or("single")),
                            ("waterlogged", half.value("waterlogged").unwrap_or("false")),
                        ]))
                    {
                        set(world, at, state);
                    }
                }

                return;
            }

            let wanted = if kind == "single" && points_here(&other) {
                side_of(from)
            } else if kind != "single" && side_of(from) == kind && !points_here(&other) {
                "single"
            } else {
                return;
            };

            if let Some(state) = block.with("type", wanted) {
                set(world, pos, state);
            }
        }

        // Музыкальный блок: инструмент — по голове сверху или блоку снизу.
        Kind::NoteBlock if from == Dir::Down || from == Dir::Up => {
            let instrument = instrument_of(world, pos);

            if let Some(state) = block.with("instrument", instrument)
                && state != block.state
            {
                set(world, pos, state);
            }
        }

        Kind::Other | Kind::RedstoneBlock => {
            // Портал без рамки гаснет — без выпадения, но с частицами
            // разрушения (2001, чёрный ящик, nether-portal); следом гаснут и
            // соседние блоки проёма.
            if crate::portal::is_portal(block.state) && !crate::portal::holds(world, pos, block.state) {
                world.note_level_event(pos.0, pos.1, pos.2, 2001, block.state);
                set(world, pos, AIR);
                return;
            }

            // Растению нужна своя опора: убрали землю из-под цветка — цветок
            // ломается и выпадает (правила — в модуле plants).
            if crate::plants::check(world, pos.0, pos.1, pos.2, loud) {
                return;
            }

            // Грядка под твёрдым блоком, лист без опоры (farming.rs).
            if crate::farming::on_neighbour(world, pos) {
                return;
            }

            // Песок и гравий без опоры падают своим тактом через два.
            if falls(block.state) && !holds_up(world.get_block(pos.0, pos.1 - 1, pos.2)) {
                world.schedule_once(pos.0, pos.1, pos.2, FALL_DELAY, PRIORITY_NORMAL);
            }
        }

        _ => {}
    }
}

/// Запитан ли поршень.
///
/// Слушает он все стороны, кроме той, куда смотрит голова. И ещё клетку прямо
/// над собой — это «квазисвязность»: поршень срабатывает, если сигнал пришёл
/// бы в место над ним, даже когда там пусто.
fn piston_powered(world: &World, pos: Pos, block: &Block) -> bool {
    let head = block.facing();

    let powered_at = |place: Pos, skip: Option<Dir>| {
        NEIGHBOUR_ORDER
            .iter()
            .filter(|dir| Some(**dir) != skip)
            .any(|dir| power_from(world, place, *dir, false) > 0)
    };

    // Со стороны головы поршень не питается, а вот место над ним — питается
    // с любой стороны.
    powered_at(pos, head) || powered_at(step(pos, Dir::Up), None)
}

/// Стоит ли перед поршнем его голова.
fn head_in_front(world: &World, pos: Pos, base: &Block) -> bool {
    let Some(facing) = base.facing() else {
        return false;
    };

    let head = Block::at(world, step(pos, facing));

    head.kind == Kind::PistonHead && head.facing() == Some(facing)
}

/// Стоит ли за головой выдвинутый поршень, которому она принадлежит.
fn piston_behind(world: &World, pos: Pos, head: &Block) -> bool {
    let Some(facing) = head.facing() else {
        return false;
    };

    let base = Block::at(world, step(pos, facing.opposite()));

    base.kind == Kind::Piston && base.is("extended", "true") && base.facing() == Some(facing)
}

/// Липкий ли это блок и какой именно.
///
/// Слизь и мёд тянут за собой соседей. Липкий поршень к ним не относится:
/// у него другое устройство — он тянет один блок у самой головы.
fn sticky_kind(state: i32) -> Option<&'static str> {
    match blocks::block_at_state(state)? {
        name @ ("slime_block" | "honey_block") => Some(name),
        _ => None,
    }
}

/// Прилипает ли сосед к липкому блоку.
///
/// Слизь и мёд друг к другу не липнут — на этом держится половина
/// технических построек. Глазурованная керамика не липнет ни к чему, хотя
/// толкать её можно. И липкий блок тянет только то, что можно тянуть: то,
/// что ломается, не тянется.
fn sticks_to(sticky: &str, neighbour: i32) -> bool {
    if push_reaction(neighbour) != Push::Moves {
        return false;
    }

    let Some(name) = blocks::block_at_state(neighbour) else {
        return false;
    };

    if name.ends_with("glazed_terracotta") {
        return false;
    }

    match sticky_kind(neighbour) {
        // Разные липкие блоки друг друга не держат.
        Some(other) => other == sticky,
        None => true,
    }
}

/// Какие блоки поедут, если поршень двинется в эту сторону.
///
/// Считается не линия, а связка: липкий блок тянет за собой соседей по
/// граням, те — своих, и каждый толкает то, что стоит перед ним.
///
/// None означает, что ход не состоится: на пути непосильный блок или в связке
/// больше двенадцати блоков.
fn push_group(world: &World, from: Pos, facing: Dir, piston: Pos) -> Option<Vec<Pos>> {
    let mut group: Vec<Pos> = Vec::new();
    let mut waiting: VecDeque<Pos> = VecDeque::from([from]);

    while let Some(place) = waiting.pop_front() {
        // Сам поршень в связку не входит: он толкает, а не едет. Иначе слизь,
        // прилипшая к нему, тащила бы его самого.
        if place == piston || group.contains(&place) {
            continue;
        }

        let state = world.get_block(place.0, place.1, place.2);

        match push_reaction(state) {
            // Пусто или ломается: такое связке не мешает и в неё не входит.
            Push::Breaks => continue,
            // Непосильный блок на пути — ход отменяется весь.
            Push::Stays => return None,
            Push::Moves => {}
        }

        group.push(place);

        if group.len() > PUSH_LIMIT {
            return None;
        }

        // Всё, что стоит перед ним, поедет тоже.
        waiting.push_back(step(place, facing));

        // А липкий блок тянет ещё и соседей по граням. Непосильного соседа
        // он просто не трогает — тот не мешает.
        if let Some(sticky) = sticky_kind(state) {
            for dir in NEIGHBOUR_ORDER {
                let neighbour = step(place, dir);

                if sticks_to(sticky, world.get_block(neighbour.0, neighbour.1, neighbour.2)) {
                    waiting.push_back(neighbour);
                }
            }
        }
    }

    Some(group)
}



/// На сколько тактов позже клиенту сообщают о блоках, которые ставит ход
/// поршня: признак «выдвинут», опустевшие и занятые места, блоки в конце
/// хода.
///
/// Так у оригинала (сверено чёрным ящиком): о ходе он сообщает сразу, а
/// изменения блоков этой фазы такта рассылает только в следующем такте.
/// Клиенту это на руку: он рисует ход три такта, и блок, присланный на такт
/// раньше, вставал бы рывком.
const PISTON_TELL_LATER: u64 = 1;

/// Ставит блок ходом поршня: в мире сразу, клиенту — на такт позже.
fn set_moved(world: &mut World, pos: Pos, state: i32) {
    world.set_block_quietly(pos.0, pos.1, pos.2, state);
    world.tell_later(pos.0, pos.1, pos.2, PISTON_TELL_LATER);
}

/// Едущий блок встаёт на место в конце хода.
///
/// Наблюдатель после переезда срабатывает: «Observers also emit a pulse after
/// getting moved by pistons» (вики, Observer); «moving an observer while it's
/// not active causes it to output a pulse after its usual delay; however,
/// moving an observer that is currently emitting a redstone signal causes it
/// to turn off instead». Такт наблюдателя сам решает, что делать: выключенный
/// включается, включённый гаснет. Сверено чёрным ящиком (сценарий
/// piston-moves-observer): загорается через два такта после приезда. Когда
/// гаснет приехавший включённым, не замерено [проверить].
fn arrive(world: &mut World, pos: Pos, state: i32) {
    set_moved(world, pos, state);

    if blocks::block_at_state(state).is_some_and(|name| kind_of(name) == Kind::Observer) {
        world.schedule_once(pos.0, pos.1, pos.2, OBSERVER_PULSE, PRIORITY_NORMAL);
    }
}

/// Состояние служебного блока «движущийся поршень».
///
/// Клиенту он не отправляется: сервер держит его у себя, чтобы место
/// едущего блока было занято, но ничего не проводило и ни на что не влияло.
fn moving_block(facing: Dir, sticky: bool) -> Option<i32> {
    blocks::orientation("moving_piston")?.state(&[
        ("facing", facing.name()),
        ("type", if sticky { "sticky" } else { "normal" }),
    ])
}

/// Ставит служебный блок, ничего не сообщая клиенту.
fn set_hidden(world: &mut World, pos: Pos, state: i32) {
    world.set_block_quietly(pos.0, pos.1, pos.2, state);
}




/// Вторая половина двери принимает `open` и `powered` первой.
fn door_copy_to_other_half(world: &mut World, pos: Pos, block: &Block, state: i32) {
    let other = match block.value("half") {
        Some("lower") => step(pos, Dir::Up),
        _ => step(pos, Dir::Down),
    };
    let other_block = Block::at(world, other);

    if other_block.kind != Kind::Door || other_block.name != block.name {
        return;
    }

    let Some(kind) = block.orientation else {
        return;
    };

    let mut copied = Some(other_block.state);

    for property in ["open", "powered"] {
        let value = kind.value(state, property);
        copied = copied.and_then(|current| value.and_then(|value| kind.with(current, property, value)));
    }

    if let Some(copied) = copied
        && copied != other_block.state
    {
        set(world, other, copied);
    }
}

/// Звук открытия и закрытия двери, люка, калитки — по материалу (имена
/// звуков из отчёта `--reports`): у железа, меди, вишни, бамбука и
/// незерских пород свои, у прочего дерева — общий деревянный.
fn opening_sound(block: &Block, open: bool) -> &'static str {
    let name = block.name;
    let material = if name.starts_with("iron_") {
        "iron"
    } else if name.contains("copper") {
        "copper"
    } else if name.starts_with("cherry") {
        "cherry"
    } else if name.starts_with("bamboo") {
        "bamboo"
    } else if name.starts_with("crimson") || name.starts_with("warped") {
        "nether"
    } else {
        "wood"
    };

    match (block.kind, material, open) {
        (Kind::Door, "iron", true) => "minecraft:block.iron_door.open",
        (Kind::Door, "iron", false) => "minecraft:block.iron_door.close",
        (Kind::Door, "copper", true) => "minecraft:block.copper_door.open",
        (Kind::Door, "copper", false) => "minecraft:block.copper_door.close",
        (Kind::Door, "cherry", true) => "minecraft:block.cherry_wood_door.open",
        (Kind::Door, "cherry", false) => "minecraft:block.cherry_wood_door.close",
        (Kind::Door, "bamboo", true) => "minecraft:block.bamboo_wood_door.open",
        (Kind::Door, "bamboo", false) => "minecraft:block.bamboo_wood_door.close",
        (Kind::Door, "nether", true) => "minecraft:block.nether_wood_door.open",
        (Kind::Door, "nether", false) => "minecraft:block.nether_wood_door.close",
        (Kind::Door, _, true) => "minecraft:block.wooden_door.open",
        (Kind::Door, _, false) => "minecraft:block.wooden_door.close",
        (Kind::Trapdoor, "iron", true) => "minecraft:block.iron_trapdoor.open",
        (Kind::Trapdoor, "iron", false) => "minecraft:block.iron_trapdoor.close",
        (Kind::Trapdoor, "copper", true) => "minecraft:block.copper_trapdoor.open",
        (Kind::Trapdoor, "copper", false) => "minecraft:block.copper_trapdoor.close",
        (Kind::Trapdoor, "cherry", true) => "minecraft:block.cherry_wood_trapdoor.open",
        (Kind::Trapdoor, "cherry", false) => "minecraft:block.cherry_wood_trapdoor.close",
        (Kind::Trapdoor, "bamboo", true) => "minecraft:block.bamboo_wood_trapdoor.open",
        (Kind::Trapdoor, "bamboo", false) => "minecraft:block.bamboo_wood_trapdoor.close",
        (Kind::Trapdoor, "nether", true) => "minecraft:block.nether_wood_trapdoor.open",
        (Kind::Trapdoor, "nether", false) => "minecraft:block.nether_wood_trapdoor.close",
        (Kind::Trapdoor, _, true) => "minecraft:block.wooden_trapdoor.open",
        (Kind::Trapdoor, _, false) => "minecraft:block.wooden_trapdoor.close",
        (_, "cherry", true) => "minecraft:block.cherry_wood_fence_gate.open",
        (_, "cherry", false) => "minecraft:block.cherry_wood_fence_gate.close",
        (_, "bamboo", true) => "minecraft:block.bamboo_wood_fence_gate.open",
        (_, "bamboo", false) => "minecraft:block.bamboo_wood_fence_gate.close",
        (_, "nether", true) => "minecraft:block.nether_wood_fence_gate.open",
        (_, "nether", false) => "minecraft:block.nether_wood_fence_gate.close",
        (_, _, true) => "minecraft:block.fence_gate.open",
        (_, _, false) => "minecraft:block.fence_gate.close",
    }
}

/// Запитана ли вторая половина двери: дверь открывается целиком.
fn door_other_half_powered(world: &World, pos: Pos, block: &Block) -> bool {
    let other = match block.value("half") {
        Some("lower") => step(pos, Dir::Up),
        _ => step(pos, Dir::Down),
    };

    Block::at(world, other).kind == Kind::Door && any_power(world, other)
}

/// Должен ли факел гореть: горит, пока не запитан блок, к которому
/// прикреплён.
fn torch_should_be_lit(world: &World, pos: Pos, block: &Block) -> bool {
    match attached_to(block, pos) {
        Some(attached) => {
            let support = Block::at(world, attached);

            if support.conductive() {
                weak_power(world, attached) == 0
            } else {
                // Прикреплён к непроводящему — запитать его нельзя.
                true
            }
        }
        None => true,
    }
}

/// Держится ли деталь на чём-то: провод, диод и плита — на блоке снизу,
/// факел, рычаг и кнопка — на том, к чему прикреплены.
fn supported(world: &World, pos: Pos, block: &Block) -> bool {
    match block.kind {
        // Пыль и диоды на нижней плите не держатся: у неё верх посередине
        // блока (вики, Redstone Dust и Redstone Repeater).
        Kind::Wire | Kind::Repeater | Kind::Comparator => {
            let below = world.get_block(pos.0, pos.1 - 1, pos.2);

            !is_leaves(below) && blocks::carries_dust(below)
        }
        Kind::Plate => holds_redstone(world.get_block(pos.0, pos.1 - 1, pos.2)),
        Kind::Torch | Kind::Lever | Kind::Button | Kind::Hook => attached_to(block, pos).is_some_and(|attached| {
            holds_redstone(world.get_block(attached.0, attached.1, attached.2))
        }),
        _ => true,
    }
}

/// Держится ли на этом блоке редстоун.
///
/// Листва — особый случай: блок полный, но провод, повторитель, сравнитель,
/// кнопку, рычаг и рельсы на неё поставить нельзя. Вики перечисляет их
/// поимённо на странице Conductivity.
fn holds_redstone(state: i32) -> bool {
    !is_leaves(state) && blocks::solid_top(state)
}

/// Листва: полный блок, на котором редстоун не держится.
fn is_leaves(state: i32) -> bool {
    blocks::block_at_state(state).is_some_and(|name| name.ends_with("_leaves"))
}

/// Пришло блочное событие: поршень трогается с места.
///
/// Это отдельная фаза такта мира, после всех запланированных тактов, —
/// так в игре, и от этого зависит, что успевает сработать раньше.
pub fn block_event(world: &mut World, pos: Pos, kind: u8, param: u8) {
    let block = Block::at(world, pos);

    match block.kind {
        Kind::Piston => {}
        Kind::Chest | Kind::TrappedChest => {
            containers::tell_lid(world, pos, param);
            return;
        }

        // Нота: инструмент и ступень — из состояния блока в миг события.
        Kind::NoteBlock => {
            play_note(world, pos, instrument_of(world, pos), note_of(&block));
            return;
        }

        Kind::Bell if kind == components::BELL_RING => {
            components::bell_event(world, pos, param);
            return;
        }
        _ => return,
    }

    // Перед исполнением событие проверяется заново (вики, Piston):
    // выдвигаться — только запитанному, задвигаться — только
    // незапитанному.
    let powered = piston_powered(world, pos, &block);
    let extended = block.is("extended", "true");
    let extending = kind == PISTON_EXTENDS;

    if extending != powered {
        return;
    }

    // `block_activate` при выдвигании, `block_deactivate` при задвигании.
    game_event(world, pos, if extending { FREQ_ACTIVATE } else { FREQ_DEACTIVATE });

    // Выдвигание можно прервать: ход доигрывается сразу, а липкий поршень
    // при этом бросает блок, который толкал, — не тянет его назад (вики,
    // Sticky Piston, «Block dropping»). Задвигание не прерывается.
    let mut drop_block = false;
    let mut interrupted = false;
    let mut cut_short = false;

    if let Some(moving) = world.redstone.moving.get(&pos) {
        if extending || !moving.extending {
            return;
        }

        let moving = world.redstone.moving.remove(&pos).expect("ход только что был");

        drop_block = block.name.starts_with("sticky") && !moving.cargo.is_empty();
        // «Отмену» оригинал шлёт, только если поршень что-то толкал; без
        // груза — обычное задвигание (чёрный ящик, sticky-spit-many).
        interrupted = !moving.cargo.is_empty();
        cut_short = true;
        interrupt_extension(world, moving, drop_block);
    } else if extended == extending {
        return;
    }

    let block = Block::at(world, pos);

    let Some(facing) = block.facing() else {
        return;
    };

    let Some(moving) = start_move(world, pos, &block, facing, extending, drop_block, cut_short) else {
        // Двигать нечего — поршень даже не трогается: упёршийся поршень
        // так и стоит задвинутым.
        return;
    };

    // Сперва сообщаем о ходе, и только потом меняем состояние поршня:
    // клиент по этому сообщению проигрывает ход сам, а если он сначала
    // увидит уже выдвинутый поршень, рисовать ему будет нечего.
    if let Some(id) = blocks::block_id(block.name) {
        // Прерванное выдвигание оригинал сообщает действием 2 — «отмена»
        // (чёрный ящик, sticky-spit-many), обычное задвигание — 1.
        let (action, sound) = if extending {
            (PISTON_EXTENDS, "minecraft:block.piston.extend")
        } else if interrupted {
            (PISTON_CANCELS, "minecraft:block.piston.contract")
        } else {
            (PISTON_RETRACTS, "minecraft:block.piston.contract")
        };

        // Звук идёт раньше сообщения о ходе — так у оригинала. Высота
        // у него случайная, от 0,6 до 0,85 (вики: «Piston», звуки).
        let pitch = 0.6 + world.random_unit() * 0.25;
        world.note_sound(pos.0, pos.1, pos.2, sound, 0.5, pitch);
        world.note_action(crate::world::BlockAction {
            x: pos.0,
            y: pos.1,
            z: pos.2,
            action,
            param: facing.number(),
            block: id,
        });
    }

    // Признак «выдвинут» ставится в начале хода, а блоки встают на новые
    // места в конце: так в игре, и на этом держатся тайминги схем.
    // Клиенту же обо всех блоках хода говорят на такт позже, чем о самом
    // ходе: у оригинала изменения блоков рассылаются раз в такт, а ход
    // поршня идёт уже после этой рассылки (сверено чёрным ящиком).
    if extending && let Some(state) = block.with("extended", "true") {
        set_moved(world, pos, state);
    }

    // Bedrock рисует ход блок-акторами: ему отдельно говорим, какие блоки
    // едут (src/bedrock/pistons.rs).
    let step_back = if extending { facing.opposite() } else { facing };
    let head_state = extending
        .then(|| {
            blocks::orientation("piston_head").and_then(|head| {
                head.state(&[
                    ("facing", facing.name()),
                    ("type", if block.name.starts_with("sticky") { "sticky" } else { "normal" }),
                    ("short", "false"),
                ])
            })
        })
        .flatten();

    world.note_piston_move(crate::world::PistonMove {
        base: pos,
        extending,
        sticky: block.name.starts_with("sticky"),
        head: moving.head,
        head_state,
        cargo: moving.cargo.iter().rev().map(|(place, state)| (step(*place, step_back), *place, *state)).collect(),
    });

    let mut moving = moving;

    moving.due = world.tick() + PISTON_MOVE;
    moving.order = world.redstone.next_move;
    world.redstone.next_move += 1;
    world.redstone.moving.insert(pos, moving);
}

/// Выдвигание прервано задвиганием. Голова так и не встаёт; толкаемые
/// блоки едут дальше, как ехали, и приезжают в свой срок (вики, Piston:
/// «Any block which was being pushed by that piston continues to move»).
/// Липкий поршень при этом бросает блок: тот, что он толкал сам, встаёт на
/// конечное место сразу, а задвигание его не тянет (techmcdocs, Piston:
/// «the directly pushed block arrives immediately … The retraction does not
/// pull the block»).
fn interrupt_extension(world: &mut World, moving: Moving, drop_block: bool) {
    let first = step(moving.head, moving.facing);
    let mut rest = Vec::new();

    for (place, state) in moving.cargo {
        if drop_block && place == first {
            arrive(world, place, state);
        } else {
            rest.push((place, state));
        }
    }

    if !rest.is_empty() {
        world.redstone.detached.push((moving.due, moving.order, rest));
    }
}

/// Фаза блок-сущностей: ходы поршней, чьё время вышло, доигрываются —
/// блоки встают на места. У оригинала так: ход — блок-сущность, её
/// прогресс растёт на 0,5 за такт, считая такт появления, и в начале такта
/// с прогрессом 1 блок встаёт (вики, Piston/Technical components). Отсюда
/// цепочка «поршень толкает блок редстоуна к следующему» идёт шагом в
/// 3 такта.
pub fn finish_due_moves(world: &mut World) {
    let now = world.tick();
    let mut due: Vec<(u64, Pos)> = world
        .redstone
        .moving
        .iter()
        .filter(|(_, moving)| moving.due <= now)
        .map(|(pos, moving)| (moving.order, *pos))
        .collect();

    due.sort_unstable();

    // Блоки прерванных ходов доезжают сами, без поршня.
    let (arrived, waiting): (Vec<_>, Vec<_>) =
        std::mem::take(&mut world.redstone.detached).into_iter().partition(|(when, _, _)| *when <= now);

    world.redstone.detached = waiting;

    for (_, _, cargo) in arrived {
        for (place, state) in cargo {
            arrive(world, place, state);
        }

        settle(world);
    }

    for (_, pos) in due {
        let Some(moving) = world.redstone.moving.remove(&pos) else {
            continue;
        };

        if Block::at(world, pos).kind == Kind::Piston {
            finish_move(world, pos, moving);
        } else {
            abandon_move(world, &moving);
        }

        settle(world);
    }
}

/// Едущий блок в этом такте — чем он толкает сущности: где его рамка после
/// шага и куда он едет (единичный вектор). `bounce` — это выдвигаемая
/// слизь: задетое она ещё и подбрасывает (`LAUNCH`). `drag` — это мёд:
/// лежащее на его верху едет вместе с ним (`drags`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shove {
    pub min: (f64, f64, f64),
    pub max: (f64, f64, f64),
    pub dir: (f64, f64, f64),
    pub bounce: bool,
    pub drag: bool,
}

/// Шаг едущего блока за такт: полблока.
pub const SHOVE_STEP: f64 = 0.5;

/// Везёт ли едущий мёд сущность, стоящую на земле с рамкой `min..max`:
/// она лежит на его верху (низ — от верха мёда до верха его клетки: наши
/// предметы ложатся на верх клетки [проверить]) и над ним по горизонтали
/// до шага или после. Вверх мёд и так толкает. Везёт на шаг блока — и при
/// толкании, и когда липкий поршень тянет обратно (вики, Honey Block:
/// «entities on a honey block's top surface move with it»; honey-drag:
/// лежавший на мёде предмет уехал на 1,0 и вернулся; предмет без тяжести,
/// висевший на той же высоте, не поехал — он не «на земле»).
pub fn drags(shove: &Shove, on_ground: bool, min: (f64, f64, f64), max: (f64, f64, f64)) -> Option<f64> {
    const EPSILON: f64 = 1e-6;

    if !shove.drag || !on_ground || shove.dir.1 > 0.0 {
        return None;
    }

    // Рамка мёда до шага и после — вместе.
    let back = (-shove.dir.0 * SHOVE_STEP, -shove.dir.1 * SHOVE_STEP, -shove.dir.2 * SHOVE_STEP);
    let low = (shove.min.0 + back.0.min(0.0), shove.min.2 + back.2.min(0.0));
    let high = (shove.max.0 + back.0.max(0.0), shove.max.2 + back.2.max(0.0));
    let top = shove.max.1 - shove.dir.1 * SHOVE_STEP;
    let cell_top = (top - EPSILON).floor() + 1.0;
    let above = min.0 < high.0 && max.0 > low.0 && min.2 < high.1 && max.2 > low.1;
    let resting = min.1 >= top - EPSILON && min.1 <= cell_top + EPSILON;

    (above && resting).then_some(SHOVE_STEP)
}

/// Скорость, которую выдвигаемая слизь даёт задетой сущности по ходу, —
/// блок за такт (вики, Slime Block: «launched into the direction the block
/// is pushed into, at an initial speed of 20 blocks a second»; чёрный ящик,
/// slime-launch: у предмета без тяжести через 10 тактов 0,817 = 0,98¹⁰ и
/// через 6 — 0,886 = 0,98⁶, то есть 1,0 в первом такте хода).
pub const LAUNCH: f64 = 1.0;

/// Насколько сущность отодвигается дальше края едущего блока (чёрный ящик,
/// piston-push-items: предмет шириной 0,25, лежавший серединой в 0,5 блока
/// перед головой, уехал на 2601/4096 = 0,635 — ровно до края головы и ещё
/// на 0,01; робот — до 4,31 при краю головы в 4 и половине ширины 0,3).
pub const SHOVE_MARGIN: f64 = 0.01;

/// Больше этого за такт по одной оси едущий блок сущность не сдвигает:
/// шаг блока 0,5 и зазор 0,01 [проверить].
pub const SHOVE_LIMIT: f64 = 0.51;

/// Рамка: ближний и дальний углы.
type Corners = ((f64, f64, f64), (f64, f64, f64));

/// Рамки едущего блока внутри его клетки (доли блока, от угла клетки):
/// толкает настоящая форма, а не куб (чёрный ящик):
///
/// - голова — плита в 4/16 у передней грани и шток 4/16 × 4/16 за ней
///   (piston-push-shape: предмет в клетке поршня мимо штока не сдвинут);
/// - мёд — на 1/16 уже с боков и ниже на 1/16 (honey-drag: предмет,
///   висевший на высоте 15/16 над мёдом, мёд не задел — сдвинула только
///   голова, на 0,635, и обратно липкий поршень его не утянул);
/// - нижняя и верхняя плиты — своя половина (piston-push-shape: предмет
///   над нижней плитой на 0,6 не сдвинут);
/// - прочее с ящиком — куб [проверить]: ступени, заборы и остальное неполное.
fn push_boxes(state: i32, head: Option<Dir>) -> Vec<Corners> {
    const PLATE: f64 = 4.0 / 16.0;
    const ARM_LOW: f64 = 6.0 / 16.0;
    const ARM_HIGH: f64 = 10.0 / 16.0;
    const HONEY_SIDE: f64 = 1.0 / 16.0;
    const HONEY_TOP: f64 = 15.0 / 16.0;

    if let Some(facing) = head {
        // Вдоль хода: плита у передней грани, шток — от задней до плиты.
        let (dx, dy, dz) = step((0, 0, 0), facing);
        let forward = dx + dy + dz > 0;
        let (plate, arm) = if forward { ((1.0 - PLATE, 1.0), (0.0, 1.0 - PLATE)) } else { ((0.0, PLATE), (PLATE, 1.0)) };
        let span = |axis: i32, along: (f64, f64), across: (f64, f64)| if axis != 0 { along } else { across };
        let make = |along: (f64, f64), across: (f64, f64)| {
            let x = span(dx, along, across);
            let y = span(dy, along, across);
            let z = span(dz, along, across);

            ((x.0, y.0, z.0), (x.1, y.1, z.1))
        };

        return vec![make(plate, (0.0, 1.0)), make(arm, (ARM_LOW, ARM_HIGH))];
    }

    match blocks::block_at_state(state) {
        Some("honey_block") => vec![((HONEY_SIDE, 0.0, HONEY_SIDE), (1.0 - HONEY_SIDE, HONEY_TOP, 1.0 - HONEY_SIDE))],
        Some(name) if name.ends_with("_slab") => match blocks::orientation(name).and_then(|block| block.value(state, "type")) {
            Some("bottom") => vec![((0.0, 0.0, 0.0), (1.0, 0.5, 1.0))],
            Some("top") => vec![((0.0, 0.5, 0.0), (1.0, 1.0, 1.0))],
            _ => vec![((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))],
        },
        _ => vec![((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))],
    }
}

/// Фаза блок-сущностей: едущие блоки делают шаг и толкают сущности у себя
/// на пути (вики, Piston: «Any entities in the path of an extending piston
/// (or any block it might be moving) are pushed along»). Толкает он в два
/// такта после такта начала хода: в первом — с половины пути, во втором —
/// с полного, и в том же такте встаёт на место (вызывающий двигает сущности
/// до `finish_due_moves`). Такт снят с оригинала: у подброшенного слизью
/// предмета без тяжести скорость в отчёте на одно затухание 0,98 больше,
/// чем при толчке в такте начала (slime-launch: 0,785 против 0,769), а
/// везомый мёдом предмет в такте после начала хода стоит на полпути
/// (honey-drag, первый прогон). Возвращает
/// рамки блоков после шага — в порядке ходов; двигает сущности вызывающий.
///
/// Голова при задвигании не тянет за собой (вики: «Retracting pistons can
/// no longer pull entities through them»); вытягиваемые блоки толкают то,
/// что у них на пути к поршню [проверить]. Подбрасывает только выдвигаемая
/// слизь (вики, Slime Block: «When pulled by a piston, no entities are
/// launched»).
pub fn shoves(world: &World) -> Vec<Shove> {
    let now = world.tick();
    let mut moves: Vec<(u64, &Moving)> = world
        .redstone
        .moving
        .values()
        .filter(|moving| moving.due >= now && moving.due < now + PISTON_MOVE)
        .map(|moving| (moving.order, moving))
        .collect();

    moves.sort_unstable_by_key(|(order, _)| *order);

    let slime = blocks::state_by_name("slime_block");
    let honey = blocks::state_by_name("honey_block");
    let mut found = Vec::new();

    for (_, moving) in moves {
        // Сколько пути пройдено после этого шага: половина, потом весь.
        let progress = 1.0 - (moving.due - now) as f64 / PISTON_MOVE as f64;
        let dir = if moving.extending { moving.facing } else { moving.facing.opposite() };
        let unit = step((0, 0, 0), dir);
        let unit = (unit.0 as f64, unit.1 as f64, unit.2 as f64);
        let back = progress - 1.0;

        // Задвигаемая голова не толкает [проверить]: у оригинала предмет
        // в её клетке мимо штока при задвигании получает случайную скорость
        // 0,12…0,25 к поршню (piston-push-shape, три прогона) — похоже на
        // выталкивание застрявшего в блоке, а не на толчок плитой; игрока,
        // прижатого к стене в клетке головы, задвигание не сдвинуло
        // (piston-push-wall: сверки нет).
        let head = moving.extending.then_some((moving.head, AIR, Some(moving.facing)));
        let cells = moving
            .cargo
            .iter()
            .filter(|(_, state)| blocks::has_box(*state))
            .map(|(place, state)| (*place, *state, None))
            .chain(head);

        for ((x, y, z), state, head) in cells {
            let corner = (x as f64 + unit.0 * back, y as f64 + unit.1 * back, z as f64 + unit.2 * back);
            let bounce = moving.extending && Some(state) == slime && head.is_none();
            let drag = Some(state) == honey && head.is_none();

            for (low, high) in push_boxes(state, head) {
                found.push(Shove {
                    min: (corner.0 + low.0, corner.1 + low.1, corner.2 + low.2),
                    max: (corner.0 + high.0, corner.1 + high.1, corner.2 + high.2),
                    dir: unit,
                    bounce,
                    drag,
                });
            }
        }
    }

    found
}

/// Насколько едущий блок сдвигает сущность с рамкой `min..max` (по
/// направлению его хода): до своего края и ещё на зазор, но не больше
/// предела за такт. None — не задевает.
pub fn shove_distance(shove: &Shove, min: (f64, f64, f64), max: (f64, f64, f64)) -> Option<f64> {
    let overlaps = min.0 < shove.max.0
        && max.0 > shove.min.0
        && min.1 < shove.max.1
        && max.1 > shove.min.1
        && min.2 < shove.max.2
        && max.2 > shove.min.2;

    if !overlaps {
        return None;
    }

    // Ход всегда вдоль одной оси: по ней и считаем.
    let pick = |v: (f64, f64, f64)| {
        if shove.dir.0 != 0.0 {
            v.0
        } else if shove.dir.1 != 0.0 {
            v.1
        } else {
            v.2
        }
    };
    let forward = pick(shove.dir) > 0.0;
    let need = if forward { pick(shove.max) - pick(min) } else { pick(max) - pick(shove.min) };

    Some((need + SHOVE_MARGIN).clamp(0.0, SHOVE_LIMIT))
}

/// Сколько из сдвига `wanted` (вдоль единичного `dir`) рамка `min..max`
/// проходит, не входя в сплошные клетки впереди (`solid`): толкнутый игрок
/// упирается в стену (piston-push-wall). Клетки, в которые рамка уже
/// залезла, не мешают — как у вагонетки (`minecart::sweep`). Клетки —
/// полные кубы [проверить]: плиты и прочие неполные блоки.
pub fn free_run<F>(min: (f64, f64, f64), max: (f64, f64, f64), dir: (f64, f64, f64), wanted: f64, solid: &F) -> f64
where
    F: Fn(i32, i32, i32) -> bool,
{
    if wanted <= 0.0 {
        return wanted.max(0.0);
    }

    let low = [min.0, min.1, min.2];
    let high = [max.0, max.1, max.2];
    let dir = [dir.0, dir.1, dir.2];
    let Some(along) = (0..3).find(|index| dir[*index] != 0.0) else {
        return wanted;
    };
    let forward = dir[along] > 0.0;

    // Клетки, которые рамка заденет по пути.
    let mut from = low;
    let mut to = high;

    if forward {
        to[along] += wanted;
    } else {
        from[along] -= wanted;
    }

    let cells = |index: usize| (from[index].floor() as i32)..=((to[index] - 1e-9).floor() as i32);
    let mut allowed = wanted;

    for x in cells(0) {
        for y in cells(1) {
            for z in cells(2) {
                if !solid(x, y, z) {
                    continue;
                }

                let cube = [x as f64, y as f64, z as f64];

                if forward && cube[along] >= high[along] {
                    allowed = allowed.min(cube[along] - high[along]);
                } else if !forward && cube[along] + 1.0 <= low[along] {
                    allowed = allowed.min(low[along] - cube[along] - 1.0);
                }
            }
        }
    }

    allowed.max(0.0)
}

/// Доигрывает ход, у которого не стало поршня: блоки встают на места
/// назначения, а служебные блоки убираются.
fn abandon_move(world: &mut World, moving: &Moving) {
    for (place, state) in &moving.cargo {
        arrive(world, *place, *state);
    }

    if Block::at(world, moving.head).kind == Kind::MovingPiston {
        set_moved(world, moving.head, AIR);
    }
}

/// Приводит в порядок только что прочитанный с диска чанк.
///
/// На диске мир может оказаться несогласованным: сервер остановили посреди
/// хода поршня, и в чанке остались служебные блоки «движущийся поршень»,
/// поршень без головы или голова без поршня; провод записан горящим, а того,
/// что его питало, уже нет. В игре такого не бывает, потому что она пишет
/// всё вместе; у нас проще — чанк чинится при чтении:
///
/// - служебный блок остаётся, только если про этот ход помним (тогда ход
///   доиграется сам), иначе убирается;
/// - поршню без головы ставится голова, голова без поршня убирается.
///
/// Провода, факелы и диоды при чтении НЕ пересчитываются — как в игре. Их
/// состояние верно по построению, раз такты и ходы записаны вместе с блоками;
/// а пересчёт по соседям вредил бы: сосед может лежать в ещё не прочитанном
/// чанке и выглядеть воздухом, повторители получили бы лишние такты, часы
/// и защёлки сбились бы. Именно так ломается редстоун в тикете MC-151049.
pub fn repair_chunk(world: &mut World, chunk_x: i32, chunk_z: i32) {
    /// Что в этом чанке требует внимания.
    struct Found {
        moving: Vec<Pos>,
        pistons: Vec<Pos>,
        heads: Vec<Pos>,

        /// Горящая руда и датчики света: их мир помнит поимённо, а
        /// прочитанный чанк мимо правки блоков не проходил.
        remembered: Vec<(Pos, i32)>,
    }

    let ranges = |names: &[&str]| -> Vec<(i32, i32)> {
        names
            .iter()
            .filter_map(|name| blocks::orientation(name).map(|block| (block.min, block.max)))
            .collect()
    };

    let moving_range = ranges(&["moving_piston"]);
    let piston_range = ranges(&["piston", "sticky_piston"]);
    let head_range = ranges(&["piston_head"]);

    let within = |state: i32, ranges: &[(i32, i32)]| {
        ranges.iter().any(|(low, high)| (*low..=*high).contains(&state))
    };

    let mut found = Found {
        moving: Vec::new(),
        pistons: Vec::new(),
        heads: Vec::new(),
        remembered: Vec::new(),
    };

    // Обход по непустым секциям: в воздушных искать нечего.
    for section in 0..crate::world::SECTIONS as usize {
        let Some(states) = world.section_blocks(chunk_x, chunk_z, section) else {
            continue;
        };

        for (index, state) in states.iter().enumerate() {
            // В памяти состояния лежат в двух байтах — разворачиваем.
            let state = *state as i32;

            if state == AIR {
                continue;
            }

            let index = index as i32;
            let pos = (
                chunk_x * 16 + index % 16,
                crate::world::MIN_Y + section as i32 * 16 + index / 256,
                chunk_z * 16 + (index / 16) % 16,
            );

            if within(state, &moving_range) {
                found.moving.push(pos);
            } else if within(state, &piston_range) {
                found.pistons.push(pos);
            } else if within(state, &head_range) {
                found.heads.push(pos);
            } else if components::is_lit_ore(state) || components::is_daylight(state) || sculk::is_sensor_state(state)
                || matches!(blocks::block_at_state(state), Some("sculk_shrieker" | "sculk_catalyst" | "creaking_heart" | "dried_ghast")) {
                found.remembered.push((pos, state));
            }
        }
    }

    for (pos, state) in found.remembered {
        world.redstone.note_state(pos, state);
    }

    // Служебные блоки: те, что принадлежат известному ходу, останутся до его
    // конца, остальные — мусор.
    for pos in found.moving {
        let expected = world.redstone.moving.values().any(|moving| {
            moving.head == pos || moving.cargo.iter().any(|(place, _)| *place == pos)
        });

        if !expected {
            set(world, pos, AIR);
        }
    }

    for pos in found.pistons {
        if !world.redstone.moving.contains_key(&pos) {
            let block = Block::at(world, pos);
            repair_piston(world, pos, &block);
        }
    }

    for pos in found.heads {
        let block = Block::at(world, pos);

        if !piston_behind(world, pos, &block) {
            set(world, pos, AIR);
        }
    }

    settle(world);
}

/// Приводит в порядок поршень, ход которого оборвался: сервер остановили
/// посреди движения, и чем он был занят, уже не узнать.
fn repair_piston(world: &mut World, pos: Pos, block: &Block) {
    let Some(facing) = block.facing() else {
        return;
    };

    let front = step(pos, facing);
    let extended = block.is("extended", "true");

    // Место перед поршнем могло остаться занятым служебным блоком от хода,
    // о котором никто не помнит: его надо освободить в любом случае.
    if Block::at(world, front).kind == Kind::MovingPiston {
        set(world, front, AIR);
    }

    let has_head = head_in_front(world, pos, block);

    if extended == has_head {
        return;
    }

    if extended {
        let sticky = block.name.starts_with("sticky");
        let head = blocks::orientation("piston_head").and_then(|head| {
            head.state(&[
                ("facing", facing.name()),
                ("type", if sticky { "sticky" } else { "normal" }),
                ("short", "false"),
            ])
        });

        if let Some(head) = head {
            set(world, front, head);
        }
    } else {
        set(world, front, AIR);
    }
}

/// Начинает ход поршня: снимает едущие блоки с их мест.
///
/// Как в игре: на местах едущих блоков остаётся служебный «движущийся
/// поршень», а сами блоки запоминаются и встают на новые места в конце хода.
/// Возвращает None, если двигать нечего — тогда поршень не трогается вовсе.
fn start_move(
    world: &mut World,
    pos: Pos,
    block: &Block,
    facing: Dir,
    extending: bool,
    drop_block: bool,
    cut_short: bool,
) -> Option<Moving> {
    let front = step(pos, facing);
    // Липкий поршень, бросивший блок, задвигается как обычный.
    let sticky = block.name.starts_with("sticky") && !drop_block;
    let hidden = moving_block(facing, sticky)?;

    // При задвигании голова уезжает первой: пока она стоит, тянущемуся
    // блоку некуда ехать. Сперва убираем её совсем — иначе она же и
    // перегородит расчёт, — а служебный блок ставим уже после.
    if !extending {
        set_hidden(world, front, AIR);
    }

    // Что поедет: при выдвигании — связка перед поршнем, при задвигании —
    // то, что липкий поршень тянет за собой.
    let mut failed_pull = false;
    let group = if extending {
        push_group(world, front, facing, pos)?
    } else {
        pulled_group(world, pos, facing, sticky).unwrap_or_else(|| {
            failed_pull = true;
            Vec::new()
        })
    };

    // Место головы занято ходом: оно не пустое, но и ничего не проводит.
    // Если туда ничего не приедет, клиенту говорят «пусто» — сверено
    // с оригиналом: при вытягивании блока о голове он не говорит вовсе.
    if !extending {
        set_hidden(world, front, hidden);

        // Пусто, если тянуть нечего. Если есть что — туда едет блок; тогда
        // оригинал говорит о месте служебным «движущимся поршнем», только
        // если голова так и не встала — выдвигание прервали (чёрный ящик:
        // sticky-spit-many — говорит, sticky-pull — молчит).
        // Если же тянуть пытался, но связка не пошла (больше двенадцати или
        // упёрлась), о месте головы оригинал молчит и в начале, и в конце
        // хода (чёрный ящик, slime-limit).
        if group.is_empty() && !failed_pull {
            world.tell_later(front.0, front.1, front.2, PISTON_TELL_LATER);
        } else if !group.is_empty() && cut_short {
            world.tell_later_as_is(front.0, front.1, front.2, PISTON_TELL_LATER);
        }
    }

    let step_to = if extending { facing } else { facing.opposite() };

    // То, во что связка упрётся, ломается сразу. Клиенту о таком месте
    // оригинал сообщает служебным блоком «движущийся поршень» — на такт
    // позже, как обо всём, что меняет ход (сверено чёрным ящиком).
    let mut crushed = Vec::new();

    for place in &group {
        let ahead = step(*place, step_to);

        // При задвигании впереди тянущегося блока стоит место головы —
        // оно уже занято ходом, ломать там нечего.
        if ahead != front && !group.contains(&ahead) && destroy(world, ahead) {
            crushed.push(ahead);
        }
    }

    // Место под голову освобождается в начале хода — там могла быть трава.
    // Но если там стоит то, что поедет, ломать нельзя: оно в связке.
    if extending && !group.contains(&front) && destroy(world, front) {
        crushed.push(front);
    }

    let mut cargo = Vec::new();

    for place in &group {
        let state = world.get_block(place.0, place.1, place.2);

        cargo.push((step(*place, step_to), state));
    }

    // Старые места пустеют: блоки уже едут. Клиенту об опустевшем месте
    // говорят (на такт позже), если туда не приедет другой блок связки;
    // при толкании молчат и о месте головы — его займёт она сама. В линию
    // таких мест не бывает, а у связки на слизи бывают: задний слой
    // (чёрный ящик, slime-limit, slime-honey, sticky-glazed).
    for place in &group {
        set_hidden(world, *place, AIR);

        let refilled = group.iter().any(|other| step(*other, step_to) == *place);

        if !refilled && (!extending || *place != front) {
            world.tell_later(place.0, place.1, place.2, PISTON_TELL_LATER);
        }
    }

    // А места назначения занимает служебный блок — так в игре: пока идёт
    // ход, туда нельзя ни встать, ни поставить что-нибудь.
    // У притянутого липким поршнем служебный блок — «обычного» вида
    // (чёрный ящик, sticky-spit-many).
    let cargo_hidden = if extending { hidden } else { moving_block(facing, false).unwrap_or(hidden) };

    for (place, _) in &cargo {
        set_hidden(world, *place, cargo_hidden);
    }

    // Место под голову — тоже её будущее место.
    if extending {
        set_hidden(world, front, hidden);
    }

    // Где что-то сломалось, теперь стоит служебный блок — о нём и говорят.
    for place in crushed {
        world.tell_later_as_is(place.0, place.1, place.2, PISTON_TELL_LATER);
    }

    Some(Moving {
        extending,
        cargo,
        head: front,
        facing,
        due: 0,
        order: 0,
    })
}

/// Что липкий поршень тянет за собой, задвигаясь.
///
/// None — поршень пытался тянуть, но связка не идёт: больше двенадцати
/// блоков или упирается в неподвижное. Пустой список — тянуть нечего.
fn pulled_group(world: &World, pos: Pos, facing: Dir, sticky: bool) -> Option<Vec<Pos>> {
    if !sticky {
        return Some(Vec::new());
    }

    let ahead = step(step(pos, facing), facing);
    let pulled = world.get_block(ahead.0, ahead.1, ahead.2);

    if push_reaction(pulled) != Push::Moves || !can_be_pulled(pulled) {
        return Some(Vec::new());
    }

    push_group(world, ahead, facing.opposite(), pos)
}

/// Доигрывает ход поршня: время вышло, блоки встают на новые места.
fn finish_move(world: &mut World, pos: Pos, moving: Moving) {
    let block = Block::at(world, pos);

    if block.kind != Kind::Piston {
        // Поршня на месте больше нет — доигрывать нечего. Но едущие блоки
        // уже сняты со своих мест, и бросать их нельзя: на местах назначения
        // остался бы служебный блок, невидимый и непроходимый.
        abandon_move(world, &moving);
        return;
    }

    // Голова встаёт на своё место, если поршень выдвигался; если задвигался
    // — на её месте стоит служебный блок, и его надо убрать, когда туда
    // ничего не приедет.
    // Клиенту про пустоту на месте головы сказали ещё в начале хода, второй
    // раз не повторяем — так у оригинала.
    if !moving.extending
        && Block::at(world, moving.head).kind == Kind::MovingPiston
        && !moving.cargo.iter().any(|(place, _)| *place == moving.head)
    {
        set_hidden(world, moving.head, AIR);
    }

    if moving.extending {
        let sticky = block.name.starts_with("sticky");
        let head = blocks::orientation("piston_head").and_then(|head| {
            head.state(&[
                ("facing", moving.facing.name()),
                ("type", if sticky { "sticky" } else { "normal" }),
                ("short", "false"),
            ])
        });

        if let Some(head) = head {
            set_moved(world, moving.head, head);
        }
    }

    // И сами блоки — на новые места.
    for (place, state) in &moving.cargo {
        arrive(world, *place, *state);
    }

    if !moving.extending && let Some(retracted) = block.with("extended", "false") {
        set_moved(world, pos, retracted);
    }
}

/// Пришёл запланированный такт для детали в этом месте.
pub fn ticked(world: &mut World, pos: Pos) {
    world.begin_tick(pos, crate::world::TickKind::Block);
    if crate::mob::small_world::hatch_egg(world, pos) { return; }
    if blocks::block_at_state(world.get_block(pos.0,pos.1,pos.2)) == Some("dried_ghast") {
        crate::mob::newblocks::hydration_tick(world,pos);
        return;
    }

    if matches!(blocks::block_at_state(world.get_block(pos.0, pos.1, pos.2)), Some("fire" | "soul_fire")) { crate::fire::tick(world, pos); return; }
    let block = Block::at(world, pos);

    match block.kind {
        Kind::Plate => plate_ticked(world, pos, &block),

        // Поршень: вышло время хода.
        // Запланированный такт поршня — только из мира, записанного
        // прежней версией (тогда ход кончался тактом). Ход, о котором
        // помним, доиграет фаза блок-сущностей; о котором не помним —
        // поршень приводится в порядок: помечен выдвинутым — ставим голову,
        // задвинутым — убираем.
        Kind::Piston => {
            if !world.redstone.moving.contains_key(&pos) {
                repair_piston(world, pos, &block);
            }
        }

        // Наблюдатель: заметил изменение — даёт короткий сигнал, и через
        // два такта сам его снимает.
        Kind::Observer => {
            let powered = block.is("powered", "true");

            if let Some(state) = block.with("powered", yes_no(!powered)) {
                set(world, pos, state);

                // У оригинала обновления от смены состояния уходят сразу,
                // ещё до того, как наблюдатель заведёт себе такт выключения:
                // сосед-наблюдатель заводит свой такт раньше (сценарий
                // observer-clock).
                settle(world);
            }

            if !powered {
                world.schedule_once(pos.0, pos.1, pos.2, OBSERVER_PULSE, PRIORITY_NORMAL);
            }
        }

        // Сундук, пока открыт, раз в 5 тактов повторяет крышку.
        Kind::Chest | Kind::TrappedChest => containers::recheck_lid(world, pos),

        // Выбрасыватель и раздатчик: прошли 4 такта от фронта сигнала.
        Kind::Dispenser => containers::dispense(world, pos),

        // Крафтер: прошли 4 такта от фронта сигнала.
        Kind::Crafter => crafter::craft(world, pos),

        // Кафедра: импульс от перелистывания кончился.
        Kind::Lectern => lectern::pulse_over(world, pos),

        // Командный блок: в очередь на выполнение (команду выполнит такт без
        // замка мира), повторяющийся — ещё и такт на следующий.
        Kind::CommandBlock => crate::command_block::ticked(world, pos),

        // Время вышло: если сигнал так и не вернулся, лампа гаснет.
        Kind::Lamp => {
            if !any_power(world, pos)
                && block.is("lit", "true")
                && let Some(state) = block.with("lit", "false")
            {
                set(world, pos, state);
            }
        }

        Kind::Torch => {
            let wanted = torch_should_be_lit(world, pos, &block);

            if wanted == is_on(&block) {
                return;
            }

            // Выгорание: слишком частые переключения гасят факел.
            let tick = world.tick();
            let toggles = world.redstone.torch_toggles.entry(pos).or_default();

            while toggles.front().is_some_and(|when| tick - *when > BURNOUT_WINDOW) {
                toggles.pop_front();
            }

            if !wanted {
                toggles.push_back(tick);
            }

            // Выгорает факел на восьмом выключении за 60 тактов: гаснет
            // как обычно, но с шипением и дымом (событие мира 1502), и
            // больше сам не загорается (чёрный ящик, torch-burnout).
            // Загореться снова может, только когда выключений в окне
            // осталось меньше восьми — «until the number of state changes
            // in the last 60 game ticks drops to fewer than eight».
            let count = toggles.len();

            if !wanted && count >= BURNOUT_TOGGLES {
                world.redstone.burned_torches.insert(pos);
                world.note_level_event(pos.0, pos.1, pos.2, 1502, 0);
            } else if count < BURNOUT_TOGGLES {
                world.redstone.burned_torches.remove(&pos);
            }

            if wanted && world.redstone.burned_torches.contains(&pos) {
                // Выгоревший не загорается, пока не остынет.
                return;
            }

            if let Some(state) = block.with("lit", yes_no(wanted)) {
                set(world, pos, state);
            }
        }

        Kind::Repeater => {
            if block.is("locked", "true") {
                return;
            }

            let powered = is_on(&block);
            let input = diode_input(world, pos, &block) > 0;

            if powered && !input {
                if let Some(state) = block.with("powered", "false") {
                    set(world, pos, state);
                }
            } else if !powered && input {
                if let Some(state) = block.with("powered", "true") {
                    set(world, pos, state);
                }

                // Включились, а сигнал уже пропал — выключимся следом.
                if diode_input(world, pos, &Block::at(world, pos)) == 0 {
                    world.schedule_once(
                        pos.0,
                        pos.1,
                        pos.2,
                        repeater_delay(&block),
                        PRIORITY_DEPOWERING,
                    );
                }
            }
        }

        Kind::Comparator => {
            let output = comparator_output(world, pos, &block);

            world.redstone.comparator_output.insert(pos, output);

            let powered = output > 0;

            match block.with("powered", yes_no(powered)) {
                Some(state) if state != block.state => set(world, pos, state),
                // Состояние то же, а сила другая: соседи узнают о ней только
                // если им сказать.
                _ => world.note_changed(pos.0, pos.1, pos.2),
            }
        }

        // Мишень и громоотвод: сигнал вышел.
        Kind::Target => {
            if let Some(state) = block.with("power", "0")
                && state != block.state
            {
                set(world, pos, state);
            }
        }

        Kind::LightningRod => {
            if let Some(state) = block.with("powered", "false")
                && state != block.state
            {
                set(world, pos, state);
            }
        }

        // Датчик скалка: `active` → `cooldown` → `inactive`.
        Kind::Sculk => sculk::ticked(world, pos, &block),

        // Крикун скалка докричал.
        Kind::Other if block.name == "sculk_shrieker" => sculk::shrieker_ticked(world, pos, &block),
        Kind::Other if block.name == "sculk_catalyst" => sculk_family::catalyst_ticked(world, pos, &block),

        // Растяжка проверяет, не ушли ли с неё; крючок — цепь.
        Kind::Tripwire => components::tripwire_ticked(world, pos, &block),
        Kind::Hook => components::update_hook(world, pos, None),

        // Детекторная рельса: вагонетка ещё на ней — ждёт дальше, нет —
        // выключается.
        Kind::DetectorRail => rails::detector_ticked(world, pos, &block),

        // Кнопку отпустило время — щелчок слышат все (сценарии button-stone
        // и button-wood: звук раньше смены состояния).
        Kind::Button => {
            world.note_sound(pos.0, pos.1, pos.2, button_click_off(block.name), 1.0, 1.0);
            game_event(world, pos, FREQ_DEACTIVATE);

            if let Some(state) = block.with("powered", "false") {
                set(world, pos, state);
            }
        }

        // Такт песка или гравия: опоры так и нет — в полёт. Сущность
        // заводит такт мира, здесь место только запоминается.
        _ if falls(block.state) && free_to_fall_into(world.get_block(pos.0, pos.1 - 1, pos.2)) => {
            world.note_falling(pos.0, pos.1, pos.2);
        }

        // Лист пересчитывает расстояние до бревна (farming.rs).
        _ => crate::farming::ticked(world, pos, block.state),
    }
}

// ---------------------------------------------------------------------------
// Музыкальный блок
// ---------------------------------------------------------------------------

/// Вид блочного события ноты. Его число оригиналом не читается: всё
/// берётся из состояния блока.
const NOTE_PLAY: u8 = 0;

/// Сколько ступеней у ноты: две октавы плюс исходная, от 0 до 24.
const NOTES: i32 = 25;

/// Имена ступеней — свойство `note` записывается числом-строкой.
const NOTE_NAMES: [&str; NOTES as usize] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23", "24",
];

/// Какая ступень стоит у блока сейчас.
fn note_of(block: &Block) -> i32 {
    block
        .value("note")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// Играет ноту: звук инструмента с высотой по ступени.
///
/// Высота — `2^((ступень - 12) / 12)`, то есть ступени идут полутонами,
/// а двенадцатая звучит как записано в файле (вики: «Note Block»).
/// Звука нет, если сверху не воздух и не голова: блок заглушён («must have
/// either air or a mob head in the space directly above it»). Щелчок при
/// этом всё равно поднимает ноту — этим занимается вызывающий.
///
/// Голова звучит голосом моба, высота от ступени не зависит («Mob head
/// sounds are not affected by the note block's pitch»): громкость 3,0,
/// высота 1,0 (вики, Zombie Head). Голова игрока без компонента
/// `note_block_sound` молчит — а таких у нас других нет.
fn play_note(world: &mut World, pos: Pos, instrument: &'static str, note: i32) {
    let above = world.get_block(pos.0, pos.1 + 1, pos.2);

    if above != AIR && head_instrument(above).is_none() {
        return;
    }

    // Голова игрока без `note_block_sound` молчит совсем: у оригинала нет
    // ни звука, ни действия блока (сценарий note-heads). [проверить]:
    // событие `note_block_play` для датчиков скалка при этом.
    if head_sound(instrument) == Some(None) {
        return;
    }

    // `note_block_play` — частота 10 (вики, Vibration).
    game_event(world, pos, FREQ_ACTIVATE);
    world.allay_notes.push(pos);

    match head_sound(instrument) {
        Some(Some(sound)) => world.note_sound(pos.0, pos.1, pos.2, sound, 3.0, 1.0),
        Some(None) => {}
        None => {
            let pitch = 2f32.powf((note - 12) as f32 / 12.0);

            world.note_sound(pos.0, pos.1, pos.2, sound_of(instrument), 3.0, pitch);
        }
    }

    // Частицу-нотку клиент рисует сам, получив сообщение о действии блока:
    // отдельного пакета частиц оригинал не шлёт (сверено чёрным ящиком).
    // Оба числа действия — нули: инструмент и ступень клиент берёт
    // из состояния блока.
    if let Some(id) = blocks::block_id("note_block") {
        world.note_action(crate::world::BlockAction {
            x: pos.0,
            y: pos.1,
            z: pos.2,
            action: 0,
            param: 0,
            block: id,
        });
    }
}

/// Имя звука инструмента.
fn sound_of(instrument: &str) -> &'static str {
    match instrument {
        "basedrum" => "minecraft:block.note_block.basedrum",
        "snare" => "minecraft:block.note_block.snare",
        "hat" => "minecraft:block.note_block.hat",
        "bass" => "minecraft:block.note_block.bass",
        "flute" => "minecraft:block.note_block.flute",
        "bell" => "minecraft:block.note_block.bell",
        "guitar" => "minecraft:block.note_block.guitar",
        "chime" => "minecraft:block.note_block.chime",
        "xylophone" => "minecraft:block.note_block.xylophone",
        "iron_xylophone" => "minecraft:block.note_block.iron_xylophone",
        "cow_bell" => "minecraft:block.note_block.cow_bell",
        "didgeridoo" => "minecraft:block.note_block.didgeridoo",
        "bit" => "minecraft:block.note_block.bit",
        "banjo" => "minecraft:block.note_block.banjo",
        "pling" => "minecraft:block.note_block.pling",
        "trumpet" => "minecraft:block.note_block.trumpet",
        "trumpet_exposed" => "minecraft:block.note_block.trumpet_exposed",
        "trumpet_weathered" => "minecraft:block.note_block.trumpet_weathered",
        "trumpet_oxidized" => "minecraft:block.note_block.trumpet_oxidized",
        _ => "minecraft:block.note_block.harp",
    }
}

/// Голос головы: Some(звук) — голова моба, Some(None) — голова игрока
/// (молчит без `note_block_sound`), None — не голова. Крипер — звук
/// поджога, прочие — «ambient» моба (вики, Note Block, таблица «Mob head
/// instruments»).
fn head_sound(instrument: &str) -> Option<Option<&'static str>> {
    Some(Some(match instrument {
        "skeleton" => "minecraft:block.note_block.imitate.skeleton",
        "wither_skeleton" => "minecraft:block.note_block.imitate.wither_skeleton",
        "zombie" => "minecraft:block.note_block.imitate.zombie",
        "creeper" => "minecraft:block.note_block.imitate.creeper",
        "piglin" => "minecraft:block.note_block.imitate.piglin",
        "dragon" => "minecraft:block.note_block.imitate.ender_dragon",
        "custom_head" => return Some(None),
        _ => return None,
    }))
}

/// Какой инструмент задаёт голова, стоящая на музыкальном блоке. None — это
/// не голова. Настенная голова не в счёт: у оригинала под ней инструмент
/// берётся от блока снизу, а сам блок молчит — сверху не воздух (сценарий
/// note-heads: zombie_wall_head над блоком на камне — basedrum без звука).
fn head_instrument(state: i32) -> Option<&'static str> {
    let name = blocks::block_at_state(state)?;

    if name.ends_with("_wall_head") || name.ends_with("_wall_skull") {
        return None;
    }

    head_of(name)
}

/// Чья это голова по имени блока (настенная — тоже) — как инструмент
/// музыкального блока. None — не голова.
fn head_of(name: &str) -> Option<&'static str> {
    let kind = name
        .strip_suffix("_wall_skull")
        .or_else(|| name.strip_suffix("_skull"))
        .or_else(|| name.strip_suffix("_wall_head"))
        .or_else(|| name.strip_suffix("_head"))?;

    Some(match kind {
        "skeleton" => "skeleton",
        "wither_skeleton" => "wither_skeleton",
        "zombie" => "zombie",
        "creeper" => "creeper",
        "piglin" => "piglin",
        "dragon" => "dragon",
        "player" => "custom_head",
        _ => return None,
    })
}

/// Инструмент музыкального блока: голова сверху важнее блока снизу («Mob
/// head instruments take priority over block instruments»).
fn instrument_of(world: &World, pos: Pos) -> &'static str {
    head_instrument(world.get_block(pos.0, pos.1 + 1, pos.2)).unwrap_or_else(|| instrument_under(world, pos))
}

/// Какой инструмент задаёт блок под музыкальным блоком.
///
/// Список — со страницы вики «Note Block», таблица «Instruments & block
/// materials». Она перечисляет блоки поимённо, но имена складываются из
/// пород и видов, поэтому здесь они узнаются по окончаниям и приставкам;
/// полнота списка проверяется `tools/check_instruments.py` по той же таблице.
/// Всё, что не подошло (и воздух), — арфа.
fn instrument_under(world: &World, pos: Pos) -> &'static str {
    let below = world.get_block(pos.0, pos.1 - 1, pos.2);
    let name = blocks::block_at_state(below).unwrap_or("air");

    // Сперва блоки, названные поимённо: они могли бы попасть под общее
    // правило и зазвучать не тем.
    match name {
        "gold_block" => return "bell",
        "clay" => return "flute",
        "packed_ice" => return "chime",
        "bone_block" => return "xylophone",
        "iron_block" => return "iron_xylophone",
        "soul_sand" => return "cow_bell",
        "pumpkin" | "carved_pumpkin" | "jack_o_lantern" => return "didgeridoo",
        "emerald_block" => return "bit",
        "hay_block" => return "banjo",
        "glowstone" => return "pling",
        _ => {}
    }

    if is_copper_of(name, "oxidized") {
        return "trumpet_oxidized";
    }
    if is_copper_of(name, "weathered") {
        return "trumpet_weathered";
    }
    if is_copper_of(name, "exposed") {
        return "trumpet_exposed";
    }
    if is_copper_of(name, "") {
        return "trumpet";
    }

    if is_wool(name) {
        return "guitar";
    }
    if is_sandy(name) {
        return "snare";
    }
    if is_glassy(name) {
        return "hat";
    }
    if is_woody(name) {
        return "bass";
    }
    if is_stony(name) {
        return "basedrum";
    }

    "harp"
}

/// Медь — труба, и у каждой стадии окисления своя. Вощёная считается за ту же
/// стадию. Медные лампы, решётки, двери и люки сюда не входят: на вики в этой
/// строке только сам блок меди, тёсаная и резная медь.
fn is_copper_of(name: &str, stage: &str) -> bool {
    let bare = name.strip_prefix("waxed_").unwrap_or(name);

    let bare = match stage {
        "" => {
            if bare.starts_with("exposed_")
                || bare.starts_with("weathered_")
                || bare.starts_with("oxidized_")
            {
                return false;
            }
            bare
        }
        _ => match bare.strip_prefix(stage).and_then(|rest| rest.strip_prefix('_')) {
            Some(rest) => rest,
            None => return false,
        },
    };

    // Неокисленная медь зовётся copper_block, а окислившаяся — просто copper
    // с приставкой стадии: exposed_copper, weathered_copper, oxidized_copper.
    matches!(bare, "copper_block" | "copper" | "chiseled_copper")
        || bare.starts_with("cut_copper")
}

/// Породы дерева: из них складываются имена досок, лестниц, дверей и прочего.
const WOODS: [&str; 12] = [
    "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "pale_oak", "mangrove", "cherry",
    "bamboo", "crimson", "warped",
];

/// Что из дерева. Деревянные кнопки сюда не входят: в Java они звучат
/// как обычный блок (на вики эта строка помечена «только Bedrock»).
fn is_woody(name: &str) -> bool {
    let named = matches!(
        name,
        "mangrove_roots"
            | "muddy_mangrove_roots"
            | "mushroom_stem"
            | "brown_mushroom_block"
            | "red_mushroom_block"
            | "bee_nest"
            | "beehive"
            | "bamboo_block"
            | "stripped_bamboo_block"
            | "bamboo_mosaic"
            | "bamboo_mosaic_slab"
            | "bamboo_mosaic_stairs"
            | "shelf"
            | "chest"
            | "trapped_chest"
            | "barrel"
            | "crafting_table"
            | "cartography_table"
            | "fletching_table"
            | "smithing_table"
            | "loom"
            | "campfire"
            | "soul_campfire"
            | "composter"
            | "note_block"
            | "jukebox"
            | "bookshelf"
            | "chiseled_bookshelf"
            | "lectern"
    );

    if named {
        return true;
    }

    // Породные: доски и всё, что из них.
    let bare = name.strip_prefix("stripped_").unwrap_or(name);
    let wooden = [
        "_log", "_wood", "_stem", "_hyphae", "_planks", "_stairs", "_slab", "_door", "_trapdoor",
        "_pressure_plate", "_fence", "_fence_gate", "_sign", "_wall_sign", "_hanging_sign",
        "_wall_hanging_sign",
    ]
    .iter()
    .any(|tail| bare.ends_with(tail));

    let of_wood = WOODS
        .iter()
        .any(|wood| bare.starts_with(wood) && bare[wood.len()..].starts_with('_'));

    (wooden && of_wood) || name.ends_with("_banner")
}

/// Что из камня. Имена вида «камень — плита — ступени — стена» разбираются
/// заодно: у них общее начало, а хвост отбрасывается.
fn is_stony(name: &str) -> bool {
    let base = name
        .strip_suffix("_slab")
        .or_else(|| name.strip_suffix("_stairs"))
        .or_else(|| name.strip_suffix("_wall"))
        .unwrap_or(name);

    let by_tail = base.ends_with("_ore")
        || base.ends_with("_concrete")
        || base.ends_with("_terracotta")
        || base == "terracotta"
        || base.ends_with("_coral_block")
        || base.ends_with("_coral_fan")
        || base.ends_with("_coral_wall_fan");

    by_tail
        || matches!(
            base,
            // Камень и его родня.
            "stone" | "cobblestone" | "mossy_cobblestone" | "smooth_stone"
            | "stone_pressure_plate" | "petrified_oak"
            | "granite" | "polished_granite" | "diorite" | "polished_diorite"
            | "andesite" | "polished_andesite"
            // Глубинный сланец и туф.
            | "deepslate" | "cobbled_deepslate" | "chiseled_deepslate" | "polished_deepslate"
            | "reinforced_deepslate" | "tuff" | "chiseled_tuff" | "polished_tuff"
            // Песчаник.
            | "sandstone" | "chiseled_sandstone" | "smooth_sandstone" | "cut_sandstone"
            | "red_sandstone" | "chiseled_red_sandstone" | "smooth_red_sandstone"
            | "cut_red_sandstone"
            // Призмарин, базальт, чернокамень, край.
            | "prismarine" | "dark_prismarine" | "netherrack" | "basalt" | "smooth_basalt"
            | "polished_basalt" | "blackstone" | "gilded_blackstone" | "polished_blackstone"
            | "chiseled_polished_blackstone" | "polished_blackstone_pressure_plate" | "end_stone"
            // Кирпичи всех видов.
            | "bricks" | "brick" | "stone_bricks" | "stone_brick" | "cracked_stone_bricks"
            | "chiseled_stone_bricks" | "mossy_stone_bricks" | "mossy_stone_brick"
            | "deepslate_bricks" | "deepslate_brick" | "cracked_deepslate_bricks"
            | "deepslate_tiles" | "deepslate_tile" | "cracked_deepslate_tiles"
            | "tuff_bricks" | "tuff_brick" | "chiseled_tuff_bricks"
            | "mud_bricks" | "mud_brick" | "resin_block" | "resin_bricks" | "resin_brick"
            | "chiseled_resin_bricks" | "prismarine_bricks" | "prismarine_brick"
            | "nether_bricks" | "nether_brick" | "nether_brick_fence" | "cracked_nether_bricks"
            | "chiseled_nether_bricks" | "red_nether_bricks" | "red_nether_brick"
            | "polished_blackstone_bricks" | "polished_blackstone_brick"
            | "cracked_polished_blackstone_bricks" | "end_stone_bricks" | "end_stone_brick"
            // Пурпур и кварц.
            | "purpur" | "purpur_block" | "purpur_pillar" | "quartz" | "quartz_block"
            | "chiseled_quartz_block" | "quartz_pillar" | "smooth_quartz" | "quartz_bricks"
            // Прочее каменное.
            | "crimson_nylium" | "warped_nylium" | "dripstone_block" | "pointed_dripstone"
            | "magma_block" | "obsidian" | "crying_obsidian" | "bedrock" | "creaking_heart"
            | "coal_block" | "raw_copper_block" | "raw_gold_block" | "raw_iron_block"
            // Блоки-устройства из камня.
            | "furnace" | "smoker" | "blast_furnace" | "dispenser" | "dropper" | "observer"
            | "respawn_anchor" | "ender_chest" | "stonecutter" | "enchanting_table" | "vault"
            | "end_portal_frame" | "spawner" | "trial_spawner"
        )
}

fn is_wool(name: &str) -> bool {
    name.ends_with("_wool") || name.ends_with("_wool_slab") || name.ends_with("_wool_stairs")
}

fn is_sandy(name: &str) -> bool {
    matches!(name, "sand" | "red_sand" | "suspicious_sand" | "gravel" | "suspicious_gravel" | "heavy_core")
        || name.ends_with("_concrete_powder")
}

fn is_glassy(name: &str) -> bool {
    matches!(name, "glass" | "tinted_glass" | "sea_lantern" | "beacon" | "conduit")
        || name.ends_with("_stained_glass")
        || name.ends_with("_stained_glass_pane")
        || name == "glass_pane"
}

// ---------------------------------------------------------------------------
// Щелчки игрока
// ---------------------------------------------------------------------------

/// Игрок щёлкнул по детали: рычаг, кнопка, повторитель, сравнитель, дверь.
///
/// Возвращает true, если щелчок был по делу: тогда блок в руке ставить
/// не надо.
pub fn use_block(world: &mut World, pos: Pos) -> bool {
    let block = Block::at(world, pos);

    let new_state = match block.kind {
        // Щелчок слышно раньше, чем видно: громкость 0,3, высота 0,6 при
        // включении и 0,5 при выключении (вики: «Lever», звуки). Но звучит
        // он после того, как соседи узнали о рычаге: колокол, которому рычаг
        // дал сигнал, у оригинала слышен раньше щелчка (сценарий
        // bell-redstone). Поэтому здесь рычаг разбирается сам.
        Kind::Lever => {
            let pitch = if is_on(&block) { 0.5 } else { 0.6 };

            if let Some(state) = block.with("powered", yes_no(!is_on(&block))) {
                set(world, pos, state);
            }

            // Вибрация: включение — `block_activate`, выключение —
            // `block_deactivate` (вики, Vibration).
            game_event(world, pos, if is_on(&block) { FREQ_DEACTIVATE } else { FREQ_ACTIVATE });
            settle(world);
            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.lever.click", 0.3, pitch);
            return true;
        }

        // Щелчок поднимает ноту на полтона: двадцать пять ступеней по кругу,
        // после двадцать четвёртой снова ноль (вики: «Note Block»). Сама
        // нота звучит уже после того, как клиент увидел новый блок, —
        // этим занимается after_use.
        Kind::NoteBlock => {
            let note = (note_of(&block) + 1) % NOTES;

            block
                .orientation
                .and_then(|kind| kind.with(block.state, "note", NOTE_NAMES[note as usize]))
        }

        Kind::Button => {
            if is_on(&block) {
                return true;
            }

            let held = if is_wooden(block.name) {
                WOODEN_BUTTON_TICKS
            } else {
                STONE_BUTTON_TICKS
            };

            world.schedule_block(pos.0, pos.1, pos.2, held, PRIORITY_NORMAL);
            game_event(world, pos, FREQ_ACTIVATE);
            block.with("powered", "true")
        }

        // Задержка идёт по кругу: 1, 2, 3, 4 и снова 1.
        Kind::Repeater => {
            let next = match block.value("delay") {
                Some("1") => "2",
                Some("2") => "3",
                Some("3") => "4",
                _ => "1",
            };

            block.with("delay", next)
        }

        Kind::Comparator => {
            let mode = if block.is("mode", "compare") {
                "subtract"
            } else {
                "compare"
            };

            block.with("mode", mode)
        }

        // Деревянные двери, люки и калитки открываются рукой, железные — нет.
        Kind::Door | Kind::Trapdoor | Kind::Gate => {
            if block.name.starts_with("iron_") {
                return true;
            }

            let open = !block.is("open", "true");

            if block.kind == Kind::Door {
                let other = match block.value("half") {
                    Some("lower") => step(pos, Dir::Up),
                    _ => step(pos, Dir::Down),
                };
                let other_block = Block::at(world, other);

                if other_block.kind == Kind::Door
                    && let Some(state) = other_block.with("open", yes_no(open))
                {
                    set(world, other, state);
                }
            }

            // Звук слышат все, кроме щёлкнувшего: его клиент играет звук
            // сам (чёрный ящик, door-click — роботу звук не приходит).
            let pitch = 0.9 + world.random_unit() * 0.1;

            world.note_sound_by_hand(pos.0, pos.1, pos.2, opening_sound(&block, open), 1.0, pitch);
            game_event(world, pos, if open { FREQ_ACTIVATE } else { FREQ_DEACTIVATE });
            block.with("open", yes_no(open))
        }

        // Одинокий провод переключается между крестиком и точкой.
        Kind::Wire => {
            let lonely = HORIZONTAL.iter().all(|dir| !wire_connects_to(world, pos, *dir));

            if !lonely {
                return false;
            }

            let is_dot = HORIZONTAL.iter().all(|dir| block.is(dir.name(), "none"));
            let value = if is_dot { "side" } else { "none" };

            let mut state = Some(block.state);

            for dir in HORIZONTAL {
                state = state.and_then(|state| block.orientation?.with(state, dir.name(), value));
            }

            state
        }

        // Датчик света: щелчок меняет режим.
        Kind::Daylight => components::toggle_daylight(world, pos, &block),

        // Редстоуновая руда от щелчка загорается, но щелчок её не
        // «съедает»: блок в руке всё равно ставится (вики, Redstone Ore:
        // «Redstone ore enters this state instantly when a player interacts
        // with it by pressing either the break or the use control»).
        _ => {
            components::light_ore(world, pos);
            return false;
        }
    };

    // Соседей здесь не разбираем: порядок, в каком о щелчке узнают соседи
    // и клиент, задаёт тот, кто щёлкнул (см. place_block в сети).
    if let Some(state) = new_state {
        set(world, pos, state);
    }

    true
}

/// Щелчок, который меняет сам блок по тому, что в руке: око Края в рамку
/// портала, светокамень в якорь возрождения; торт едят рукой. Сравнитель
/// рядом читает новое состояние (чёрный ящик, comparator-click).
///
/// Some(true) — предмет из руки тратится, Some(false) — нет, None — щелчок
/// не про это. Торт у нас едят только в творческом: голода пока нет, а
/// сытый в выживании торт не ест. [проверить]: полный якорь от светокамня
/// в обычном мире у оригинала взрывается — у нас щелчок ничего не делает.
pub fn use_with_item(world: &mut World, pos: Pos, held: Option<&str>, creative: bool) -> Option<bool> {
    let block = Block::at(world, pos);

    match block.name {
        // Око: вспышка частиц у клиента — событие мира 1503, раньше повтора
        // блока.
        "end_portal_frame" if held == Some("ender_eye") && block.is("eye", "false") => {
            let state = block.with("eye", "true")?;

            world.note_level_event(pos.0, pos.1, pos.2, 1503, 0);
            set(world, pos, state);
            // Последнее око замкнуло кольцо — портал Края открывается.
            crate::portal::open_end_portal(world, pos);
            Some(true)
        }

        // Второе око в полную рамку: ничего не меняется и глаз не тратится,
        // но блок и блок за гранью повторяются (чёрный ящик, end-portal).
        "end_portal_frame" if held == Some("ender_eye") => Some(false),

        // Светокамень добавляет заряд, до четырёх; звук `charge`,
        // громкость и высота 1 — его слышит и тот, кто заряжал.
        "respawn_anchor" if held == Some("glowstone") => {
            let charges: u8 = block.value("charges")?.parse().ok()?;

            if charges >= 4 {
                return None;
            }

            let state = block.with("charges", &(charges + 1).to_string())?;

            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.respawn_anchor.charge", 1.0, 1.0);
            set(world, pos, state);
            Some(true)
        }

        // Торт: кусок за щелчок, седьмой съедает торт целиком.
        "cake" if creative => {
            let bites: u8 = block.value("bites")?.parse().ok()?;
            let state = if bites >= 6 { AIR } else { block.with("bites", &(bites + 1).to_string())? };

            set(world, pos, state);
            Some(false)
        }

        _ => None,
    }
}

/// Игрок щёлкнул по блоку гранью `face` (номер стороны, как в пакете).
/// Колоколу важна грань — по ней решается, звонит ли он и в какую сторону
/// качается; остальное — как в `use_block`.
pub fn use_block_on(world: &mut World, pos: Pos, face: i32) -> bool {
    if Block::at(world, pos).kind == Kind::Bell {
        return u8::try_from(face)
            .ok()
            .and_then(Dir::by_number)
            .is_some_and(|face| components::bell_clicked(world, pos, face));
    }

    use_block(world, pos)
}

/// Что деталь делает после щелчка — когда клиенту уже сказано, каким
/// стал блок. Пока это только музыкальный блок: у оригинала нота идёт
/// блочным событием — после изменения блока, его повтора и общей рассылки
/// (сценарий note-pitch).
pub fn after_use(world: &mut World, pos: Pos) {
    if Block::at(world, pos).kind == Kind::NoteBlock {
        world.queue_block_event(pos.0, pos.1, pos.2, NOTE_PLAY, 0);
    }
}

/// Блок поставил игрок (не команда). Крючок растяжки при этом собирает
/// цепь — у поставленного командой этого нет (сценарий tripwire-hook-break).
pub fn placed_by_player(world: &mut World, pos: Pos) {
    if Block::at(world, pos).kind == Kind::Hook {
        components::update_hook(world, pos, None);
        settle(world);
    }
}

/// Голова ли или череп в этом состоянии — у них есть блок-сущность, и
/// оригинал шлёт её данные после каждой смены блока (сценарий note-heads).
pub fn is_head(state: i32) -> bool {
    blocks::block_at_state(state).and_then(head_of).is_some()
}

/// Номер вида блок-сущности (реестр `minecraft:block_entity_type`,
/// tools/blackbox/data/registries.json) у блоков, которые не хранилища:
/// голова и датчик скалка (кафедра и проигрыватель — хранилища,
/// containers.rs). Их оригинал тоже
/// перечисляет в пакете чанка.
pub fn block_entity_type(state: i32) -> Option<i32> {
    use std::sync::OnceLock;

    /// Вид по номеру состояния; 0 — блок-сущности нет.
    static TABLE: OnceLock<Vec<u8>> = OnceLock::new();

    let table = TABLE.get_or_init(|| {
        (0..=i32::from(u16::MAX))
            .map(|state| match blocks::block_at_state(state) {
                Some(name) if head_of(name).is_some() => SKULL_ENTITY,
                Some("spawner") => crate::spawner::BLOCK_ENTITY as u8,
                Some("trial_spawner") => crate::trial::SPAWNER_ENTITY as u8,
                Some("vault") => crate::trial::VAULT_ENTITY as u8,
                Some("sculk_sensor") => 35,
                Some("calibrated_sculk_sensor") => 36,
                Some("sculk_shrieker") => 38,
                Some("command_block" | "chain_command_block" | "repeating_command_block") => crate::command_block::BLOCK_ENTITY as u8,
                Some(name) if statue::is_statue(name) => statue::BLOCK_ENTITY as u8,
                Some(name) if name.ends_with("_hanging_sign") => HANGING_SIGN_ENTITY,
                Some(name) if name.ends_with("_sign") => SIGN_ENTITY,
                _ => 0,
            })
            .collect()
    });

    match table.get(usize::try_from(state).ok()?) {
        Some(0) | None => None,
        Some(kind) => Some(i32::from(*kind)),
    }
}

/// Вид блок-сущности головы — `minecraft:skull`.
const SKULL_ENTITY: u8 = 16;

/// Виды блок-сущностей табличек: `minecraft:sign` и `minecraft:hanging_sign`
/// (настенные — те же).
const SIGN_ENTITY: u8 = 7;
const HANGING_SIGN_ENTITY: u8 = 8;

/// Табличка ли это (любая, в том числе подвесная и настенная): оригинал
/// после смены блока шлёт её данные — пустой текст с двух сторон (сценарий
/// note-instruments, cherry_hanging_sign).
pub fn is_sign(state: i32) -> bool {
    matches!(block_entity_type(state), Some(kind) if kind == i32::from(SIGN_ENTITY) || kind == i32::from(HANGING_SIGN_ENTITY))
}

/// Деревянная ли кнопка — по имени: каменные две, остальные из дерева.
fn is_wooden(name: &str) -> bool {
    !matches!(name, "stone_button" | "polished_blackstone_button")
}

fn yes_no(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Вики, Advancement: «Sneak within 8 blocks from a sculk sensor».
pub fn near_sculk_sensor(world: &World, at: (f64, f64, f64)) -> bool {
    world.redstone.sensors.keys().any(|pos| {
        (at.0 - f64::from(pos.0) - 0.5).powi(2) + (at.1 - f64::from(pos.1) - 0.5).powi(2) + (at.2 - f64::from(pos.2) - 0.5).powi(2) <= 64.0
            && sculk::is_sensor_state(world.get_block(pos.0, pos.1, pos.2))
    })
}

/// Вики, Advancement: сравнитель должен смотреть от резной книжной полки.
pub fn reads_bookshelf(world: &World, placed: Pos) -> bool {
    std::iter::once(placed).chain([Dir::North, Dir::South, Dir::East, Dir::West].map(|dir| step(placed, dir))).any(|pos| {
        let block = Block::at(world, pos);
        block.kind == Kind::Comparator && block.facing().is_some_and(|dir| Block::at(world, step(pos, dir)).name == "chiseled_bookshelf")
    })
}

/// Попадание любого снаряда в мишень (вики, Target: «projectile»).
/// Координаты попадания — мировые; грань исключаем из расстояния до центра.
pub fn projectile_target(world: &mut World, pos: (i32, i32, i32), at: (f64, f64, f64), arrow: bool) -> Option<u8> {
    if blocks::block_at_state(world.get_block(pos.0, pos.1, pos.2)) != Some("target") { return None; }
    let mut offsets = [(at.0 - pos.0 as f64 - 0.5).abs(), (at.1 - pos.1 as f64 - 0.5).abs(), (at.2 - pos.2 as f64 - 0.5).abs()];
    offsets.sort_by(f64::total_cmp);
    let power = target_power(offsets[0], offsets[1]);
    target_hit(world, pos, power, arrow);
    Some(power)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: i32 = 1;

    fn state(name: &str, values: &[(&str, &str)]) -> i32 {
        blocks::orientation(name)
            .expect("блок есть в таблице")
            .state(values)
            .expect("свойства подходят")
    }

    #[test]
    fn comparator_reads_hive_honey_level() {
        let mut w=World::in_memory();
        for level in 0..=5 {
            let state=blocks::state_from_text(&format!("beehive[honey_level={level}]")).unwrap();
            w.set_block(0,4,0,state);
            assert_eq!(comparator_reading(&w,(0,4,0)),Some(level));
        }
    }

    /// Мир с каменным полом на высоте 0.
    fn world() -> World {
        let mut world = World::in_memory();

        for x in -8..=24 {
            for z in -8..=8 {
                world.set_block_silently(x, 0, z, STONE);
            }
        }

        world
    }

    fn put(world: &mut World, pos: Pos, state: i32) {
        world.set_block(pos.0, pos.1, pos.2, state);
        settle(world);
    }

    /// Три запитанные полки на восток: слева (юг) `left`, потом `center`
    /// и `right`; щелчок по средней меняет все девять ячеек панели
    /// (сценарий shelf).
    #[test]
    fn powered_shelves_chain_and_swap_the_hotbar() {
        let mut world = world();
        let oak = state("oak_shelf", &[("facing", "east"), ("powered", "false"), ("side_chain", "unconnected")]);
        let stone = blocks::item_named("stone").unwrap();

        for z in 2..=4 {
            put(&mut world, (1, 1, z), oak);
        }

        world.with_container(1, 1, 3, |c| c.slots[0] = Some(crate::inventory::Stack::new(stone, 5)));
        assert_eq!(shelf::reading(&world, (1, 1, 3)), 1);

        for z in 2..=4 {
            put(&mut world, (1, 0, z), blocks::state_by_name("redstone_block").unwrap());
        }

        let chain = |world: &World, z| Block::at(world, (1, 1, z)).value("side_chain");
        assert_eq!(chain(&world, 4), Some("left"));
        assert_eq!(chain(&world, 3), Some("center"));
        assert_eq!(chain(&world, 2), Some("right"));
        assert_eq!(shelf::chain(&world, (1, 1, 3)), vec![(1, 1, 4), (1, 1, 3), (1, 1, 2)]);

        let hotbar = vec![None; 9];
        let used = shelf::use_shelf(&mut world, (1, 1, 3), 5, (1.0, 0.5, 0.5), &hotbar, 0, true);

        // Средняя полка — ячейки 3–5 панели.
        match used {
            Some(shelf::Used::Hotbar(changes)) => assert!(changes.contains(&(3, Some(crate::inventory::Stack::new(stone, 5))))),
            other => panic!("ждали обмен панели, вышло {other:?}"),
        }

        assert_eq!(shelf::reading(&world, (1, 1, 3)), 0);

        // Ток ушёл у левой — она сама по себе, две оставшиеся — пара.
        put(&mut world, (1, 0, 4), STONE);
        assert_eq!(chain(&world, 4), Some("unconnected"));
        assert_eq!(chain(&world, 3), Some("left"));
        assert_eq!(chain(&world, 2), Some("right"));
    }

    /// Резной шкаф: книга ложится в ячейку под точкой щелчка, сравнитель
    /// читает последнюю тронутую ячейку (сценарий chiseled-bookshelf).
    #[test]
    fn chiseled_bookshelf_takes_books_by_cursor() {
        let mut world = world();
        let book = crate::inventory::Stack::new(blocks::item_named("book").unwrap(), 3);

        put(&mut world, (1, 1, 3), state("chiseled_bookshelf", &[("facing", "east")]));
        assert_eq!(bookshelf::reading(&world, (1, 1, 3)), 0);

        let placed = bookshelf::use_bookshelf(&mut world, (1, 1, 3), 5, (1.0, 0.25, 0.5), Some(book));

        assert_eq!(placed, Some(bookshelf::Used::Placed));
        assert_eq!(Block::at(&world, (1, 1, 3)).value("slot_4_occupied"), Some("true"));
        assert_eq!(bookshelf::reading(&world, (1, 1, 3)), 5);

        // Не по лицевой стороне — мимо.
        assert_eq!(bookshelf::use_bookshelf(&mut world, (1, 1, 3), 1, (0.5, 1.0, 0.5), Some(book)), None);

        let taken = bookshelf::use_bookshelf(&mut world, (1, 1, 3), 5, (1.0, 0.25, 0.5), None);

        assert_eq!(taken, Some(bookshelf::Used::Taken(book.with_count(1))));
        assert_eq!(Block::at(&world, (1, 1, 3)).value("slot_4_occupied"), Some("false"));
    }

    /// Статуя голема: поза по кругу, сравнитель 1–4.
    #[test]
    fn golem_statue_cycles_poses() {
        let mut world = world();

        put(&mut world, (0, 1, 0), state("copper_golem_statue", &[("copper_golem_pose", "star")]));
        assert!(statue::use_statue(&mut world, (0, 1, 0)));
        assert_eq!(Block::at(&world, (0, 1, 0)).value("copper_golem_pose"), Some("standing"));
        assert_eq!(containers::comparator_reading(&world, (0, 1, 0)), Some(1));
    }

    /// Крикун: игрок на нём — крик 90 тактов (сценарий sculk-shrieker).
    #[test]
    fn shrieker_shrieks_for_90_ticks_under_a_player() {
        let mut world = world();
        let pos = (0, 1, 0);

        put(&mut world, pos, state("sculk_shrieker", &[("can_summon", "false")]));
        // Игрок только что перенесён сюда и ещё не «встал» (такт берёт
        // это из `unsettled`): крикун молчит, как у оригинала — тот
        // вскрикивает через 2 такта после переноса (sculk-shrieker).
        let airborne = Occupant { on_ground: false, ..Occupant::player(0.5, 1.6, 0.5, false) };
        note_occupants(&mut world, vec![airborne]);
        components::step_entities(&mut world);
        assert_eq!(Block::at(&world, pos).value("shrieking"), Some("false"));

        note_occupants(&mut world, vec![Occupant::player(0.5, 1.5, 0.5, false)]);
        components::step_entities(&mut world);
        assert_eq!(Block::at(&world, pos).value("shrieking"), Some("true"));

        note_occupants(&mut world, Vec::new());
        run(&mut world, 89);
        assert_eq!(Block::at(&world, pos).value("shrieking"), Some("true"));
        run(&mut world, 1);
        assert_eq!(Block::at(&world, pos).value("shrieking"), Some("false"));
    }

    /// Медные сундуки разных стадий в паре уравниваются (сценарий
    /// copper-chest).
    #[test]
    fn copper_chest_halves_merge_to_the_least_oxidized() {
        let mut world = world();

        put(&mut world, (0, 1, 0), state("exposed_copper_chest", &[("facing", "north"), ("type", "left")]));
        put(&mut world, (1, 1, 0), state("waxed_oxidized_copper_chest", &[("facing", "north"), ("type", "right")]));

        assert_eq!(Block::at(&world, (1, 1, 0)).name, "exposed_copper_chest");
        assert_eq!(Block::at(&world, (1, 1, 0)).value("type"), Some("right"));
        assert_eq!(containers::halves(&world, (0, 1, 0)).len(), 2);
    }

    fn block(world: &World, pos: Pos) -> Block {
        Block::at(world, pos)
    }

    fn run(world: &mut World, ticks: usize) {
        for _ in 0..ticks {
            for (pos, kind) in world.advance(crate::tick::PER_TICK) {
                if kind == crate::world::TickKind::Block {
                    ticked(world, pos);
                }

                // Как и в такте мира: каждый запланированный такт
                // разбирается до конца, прежде чем начнётся следующий.
                settle(world);
            }

            // И отдельной фазой — блочные события: ход поршней.
            while let Some((place, kind, param)) = world.take_block_event() {
                block_event(world, place, kind, param);
                settle(world);
            }

            settle(world);

            // Фаза блок-сущностей: датчики света, ходы поршней кончаются.
            daylight_tick(world);
            sensors_tick(world);
            finish_due_moves(world);
            settle(world);
        }
    }

    /// Провод, заменённый командой на другое состояние провода, силу сам
    /// не пересчитывает: оригинал оставляет заданную (чёрный ящик,
    /// rail-activator — крестик с силой 0 на блоке редстоуна).
    #[test]
    fn a_wire_replaced_by_a_wire_keeps_the_given_power() {
        let mut world = world();
        let block_of = blocks::state_by_name("redstone_block").unwrap();
        put(&mut world, (1, 1, 3), block_of);

        let dot = state("redstone_wire", &[("east", "none"), ("north", "none"), ("south", "none"), ("west", "none"), ("power", "0")]);
        put(&mut world, (1, 2, 3), dot);
        assert_eq!(wire_power_of(&block(&world, (1, 2, 3))), 15);

        let cross = state("redstone_wire", &[("east", "side"), ("north", "side"), ("south", "side"), ("west", "side"), ("power", "0")]);
        world.note_commanded(1, 2, 3);
        put(&mut world, (1, 2, 3), cross);
        assert_eq!(wire_power_of(&block(&world, (1, 2, 3))), 0);
    }

    /// Рычаг питает провод, и сила падает на единицу за блок: через
    /// пятнадцать блоков сигнала уже нет.
    #[test]
    fn a_lever_powers_a_wire_that_fades() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        for x in 1..=16 {
            put(&mut world, (x, 1, 0), wire);
        }

        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        put(&mut world, (0, 1, 0), lever);

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0);

        assert!(use_block(&mut world, (0, 1, 0)));

        settle(&mut world);

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);
        assert_eq!(wire_power_of(&block(&world, (8, 1, 0))), 8);
        assert_eq!(wire_power_of(&block(&world, (15, 1, 0))), 1);
        assert_eq!(wire_power_of(&block(&world, (16, 1, 0))), 0);

        // Выключили — всё погасло.
        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0);
    }

    /// Провод питает лампу, на которой лежит, а лампа рядом с проводом,
    /// который в неё не смотрит, не горит.
    #[test]
    fn a_wire_lights_a_lamp() {
        let mut world = world();
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        put(&mut world, (2, 1, 0), lamp);
        put(&mut world, (1, 1, 0), wire);

        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        put(&mut world, (0, 1, 0), lever);

        assert!(block(&world, (2, 1, 0)).is("lit", "false"));

        use_block(&mut world, (0, 1, 0));

        settle(&mut world);
        assert!(block(&world, (2, 1, 0)).is("lit", "true"));

        // Гаснет лампа не сразу, а через четыре такта.
        use_block(&mut world, (0, 1, 0));
        settle(&mut world);
        assert!(block(&world, (2, 1, 0)).is("lit", "true"), "лампа погасла слишком быстро");

        run(&mut world, 5);
        assert!(block(&world, (2, 1, 0)).is("lit", "false"), "лампа не погасла");
    }

    /// Слабо запитанный блок не питает провод, а сильно — питает.
    #[test]
    fn weak_power_does_not_pass_through_a_block() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "true")]);

        // Рычаг → провод → камень → провод: второй провод должен молчать.
        put(&mut world, (0, 1, 0), lever);
        put(&mut world, (1, 1, 0), wire);
        put(&mut world, (2, 1, 0), STONE);
        put(&mut world, (3, 1, 0), wire);

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 0);

        // А если камень питает повторитель — второй провод оживает.
        let repeater = state("repeater", &[("facing", "west"), ("delay", "1")]);
        put(&mut world, (1, 1, 0), repeater);
        run(&mut world, 4);

        assert!(is_on(&block(&world, (1, 1, 0))));
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 15);
    }

    /// Факел — инвертор с задержкой в два такта.
    #[test]
    fn a_torch_inverts_with_a_delay() {
        let mut world = world();
        let torch = blocks::state_by_name("redstone_torch").unwrap();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        // Факел на камне, провод вплотную к факелу — на соседнем камне.
        put(&mut world, (0, 1, 0), STONE);
        put(&mut world, (0, 2, 0), torch);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (1, 2, 0), wire);

        assert!(is_on(&block(&world, (0, 2, 0))));
        assert_eq!(wire_power_of(&block(&world, (1, 2, 0))), 15);

        // Рычаг на том камне, где факел: включаем — факел гаснет через
        // два такта, не сразу.
        let lever = state("lever", &[("face", "wall"), ("facing", "west"), ("powered", "false")]);
        put(&mut world, (-1, 1, 0), lever);
        use_block(&mut world, (-1, 1, 0));
        settle(&mut world);

        assert!(is_on(&block(&world, (0, 2, 0))), "погас без задержки");
        run(&mut world, 1);
        assert!(is_on(&block(&world, (0, 2, 0))), "погас через такт");
        run(&mut world, 1);
        assert!(!is_on(&block(&world, (0, 2, 0))), "не погас");
        assert_eq!(wire_power_of(&block(&world, (1, 2, 0))), 0);
    }

    /// Повторитель задерживает сигнал на свою задержку и выдаёт 15.
    #[test]
    fn a_repeater_delays_and_restores() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let repeater = state("repeater", &[("facing", "west"), ("delay", "2")]);

        put(&mut world, (0, 1, 0), lever);
        for x in 1..=14 {
            put(&mut world, (x, 1, 0), wire);
        }
        put(&mut world, (15, 1, 0), repeater);
        put(&mut world, (16, 1, 0), wire);

        use_block(&mut world, (0, 1, 0));

        settle(&mut world);

        assert_eq!(wire_power_of(&block(&world, (14, 1, 0))), 2);
        assert!(!is_on(&block(&world, (15, 1, 0))));

        run(&mut world, 3);
        assert!(!is_on(&block(&world, (15, 1, 0))), "сработал раньше своих четырёх тактов");

        run(&mut world, 1);
        assert!(is_on(&block(&world, (15, 1, 0))));
        assert_eq!(wire_power_of(&block(&world, (16, 1, 0))), 15);
    }

    /// Повторитель, в бок которого смотрит другой включённый повторитель,
    /// заперт: вход он не слушает.
    #[test]
    fn a_repeater_can_be_locked() {
        let mut world = world();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "true")]);
        let main = state("repeater", &[("facing", "west"), ("delay", "1")]);
        let lock = state("repeater", &[("facing", "south"), ("delay", "1"), ("powered", "true")]);

        // Запирающий повторитель смотрит с юга на север — в бок главного —
        // и включён: его самого питает блок редстоуна сзади.
        put(&mut world, (1, 1, 2), blocks::state_by_name("redstone_block").unwrap());
        put(&mut world, (1, 1, 1), lock);
        put(&mut world, (1, 1, 0), main);

        assert!(block(&world, (1, 1, 0)).is("locked", "true"));

        put(&mut world, (0, 1, 0), lever);
        run(&mut world, 4);

        assert!(!is_on(&block(&world, (1, 1, 0))), "запертый повторитель включился");
    }

    /// Сравнитель в режиме вычитания отнимает большую из боковых сил.
    #[test]
    fn a_comparator_subtracts() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let comparator = state("comparator", &[("facing", "west"), ("mode", "subtract")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "true")]);

        // Сзади: рычаг через шесть проводов — сила 10 на входе.
        put(&mut world, (0, 1, 0), lever);
        for x in 1..=6 {
            put(&mut world, (x, 1, 0), wire);
        }
        put(&mut world, (7, 1, 0), comparator);
        put(&mut world, (8, 1, 0), wire);

        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (8, 1, 0))), 10);

        // Сбоку: блок редстоуна даёт 15 — больше заднего, выход гаснет.
        put(&mut world, (7, 1, 1), blocks::state_by_name("redstone_block").unwrap());
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (8, 1, 0))), 0);

        // В режиме сравнения — то же самое: бок больше заднего.
        use_block(&mut world, (7, 1, 0));
        settle(&mut world);
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (8, 1, 0))), 0);
    }

    /// Сравнитель читает рамку за полным блоком, если она смотрит туда же,
    /// куда сравнитель: сила — поворот плюс один, у пустой ноль (сценарий
    /// item-frame-rotate). Рамка не в той клетке или не в ту сторону не в счёт.
    #[test]
    fn a_comparator_reads_the_item_frame_behind_a_block() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let comparator = state("comparator", &[("facing", "west"), ("mode", "compare")]);

        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (2, 1, 0), comparator);
        put(&mut world, (3, 1, 0), wire);

        let mark = |pos, facing, power| FrameMark { pos, facing, power };

        // Рамка на западной стороне камня, с предметом на четвёртом повороте.
        note_frames(&mut world, vec![mark((0, 1, 0), crate::frame::WEST, 4)]);
        comparator_update(&mut world, (0, 1, 0));
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 4);

        // Повернули — сила 5.
        note_frames(&mut world, vec![mark((0, 1, 0), crate::frame::WEST, 5)]);
        comparator_update(&mut world, (0, 1, 0));
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 5);

        // Рамка смотрит в другую сторону или висит в клетке между камнем и
        // сравнителем — не читается.
        note_frames(&mut world, vec![mark((0, 1, 0), crate::frame::EAST, 6), mark((1, 1, 0), crate::frame::WEST, 7)]);
        comparator_update(&mut world, (0, 1, 0));
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 0);

        // И снова читается, когда рамка встала как надо.
        note_frames(&mut world, vec![mark((0, 1, 0), crate::frame::WEST, 8)]);
        comparator_update(&mut world, (0, 1, 0));
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 8);

        // Вынули предмет — ноль.
        note_frames(&mut world, vec![mark((0, 1, 0), crate::frame::WEST, 0)]);
        comparator_update(&mut world, (0, 1, 0));
        run(&mut world, 6);
        assert_eq!(wire_power_of(&block(&world, (3, 1, 0))), 0);
    }

    /// Кнопка отпускается сама: каменная через 20 тактов.
    #[test]
    fn a_button_lets_go_by_itself() {
        let mut world = world();
        let button = state(
            "stone_button",
            &[("face", "floor"), ("facing", "north"), ("powered", "false")],
        );
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        put(&mut world, (0, 1, 0), button);
        put(&mut world, (1, 1, 0), wire);

        use_block(&mut world, (0, 1, 0));

        settle(&mut world);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);

        run(&mut world, 19);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);

        run(&mut world, 1);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0);
        assert!(!is_on(&block(&world, (0, 1, 0))));
    }

    /// Факел, который дёргают слишком часто, выгорает.
    #[test]
    fn a_torch_burns_out() {
        let mut world = world();
        let torch = blocks::state_by_name("redstone_torch").unwrap();
        let lever = state("lever", &[("face", "wall"), ("facing", "west"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), STONE);
        put(&mut world, (0, 2, 0), torch);
        put(&mut world, (-1, 1, 0), lever);

        // Девять выключений подряд, быстрее чем за три секунды.
        for _ in 0..9 {
            use_block(&mut world, (-1, 1, 0));
            settle(&mut world);
            run(&mut world, 2);
            use_block(&mut world, (-1, 1, 0));
            settle(&mut world);
            run(&mut world, 2);
        }

        assert!(!is_on(&block(&world, (0, 2, 0))), "факел не выгорел");
    }

    /// Провод складывается в форму по соседям: линия, угол, крестик.
    #[test]
    fn a_wire_takes_the_shape_of_its_neighbours() {
        let mut world = world();
        let wire = state(
            "redstone_wire",
            &[("east", "side"), ("west", "side"), ("north", "side"), ("south", "side")],
        );

        put(&mut world, (0, 1, 0), wire);
        let lonely = block(&world, (0, 1, 0));
        assert!(HORIZONTAL.iter().all(|dir| lonely.is(dir.name(), "side")), "одинокий — крестик");

        // Сосед с востока — линия запад-восток.
        put(&mut world, (1, 1, 0), wire);
        let line = block(&world, (0, 1, 0));
        assert!(line.is("east", "side") && line.is("west", "side"));
        assert!(line.is("north", "none") && line.is("south", "none"));

        // Провод этажом выше на камне — тянется вверх.
        put(&mut world, (0, 1, 1), STONE);
        put(&mut world, (0, 2, 1), wire);
        assert!(block(&world, (0, 1, 0)).is("south", "up"));

        // Одинокий провод щелчком становится точкой.
        put(&mut world, (5, 1, 5), wire);
        assert!(use_block(&mut world, (5, 1, 5)));
        settle(&mut world);
        assert!(HORIZONTAL.iter().all(|dir| block(&world, (5, 1, 5)).is(dir.name(), "none")));
    }

    /// Убрали опору — провод пропадает.
    #[test]
    fn a_wire_without_support_falls_off() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        put(&mut world, (0, 1, 0), wire);
        put(&mut world, (0, 0, 0), AIR);

        assert_eq!(world.get_block(0, 1, 0), AIR);
    }

    /// Вниз сигнал идёт только с проводящего блока: со стекла пыль отдаёт
    /// вверх, но не вниз.
    #[test]
    fn power_goes_down_only_from_a_conductive_block() {
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let glass = blocks::state_by_name("glass").unwrap();
        let source = blocks::state_by_name("redstone_block").unwrap();

        // Ступенька, на ней верхняя пыль, а нижняя — рядом на полу.
        let steps = |step: i32| {
            let mut world = world();

            world.set_block(1, 1, 0, step);
            world.set_block(0, 2, 0, source); // питает верхнюю пыль
            world.set_block(1, 2, 0, wire); // верхняя пыль на ступеньке
            world.set_block(2, 1, 0, wire); // нижняя пыль на полу
            settle(&mut world);

            world
        };

        // По камню вниз проходит.
        let stone_steps = steps(STONE);
        assert_eq!(wire_power_of(&block(&stone_steps, (1, 2, 0))), 15);
        assert_eq!(
            wire_power_of(&block(&stone_steps, (2, 1, 0))),
            14,
            "по камню вниз не прошло"
        );

        // По стеклу — нет.
        let glass_steps = steps(glass);
        assert_eq!(wire_power_of(&block(&glass_steps, (1, 2, 0))), 15);
        assert_eq!(
            wire_power_of(&block(&glass_steps, (2, 1, 0))),
            0,
            "стекло пропустило сигнал вниз"
        );

        // А вверх со стекла проходит: нижняя пыль питает верхнюю.
        let mut upward = world();
        put(&mut upward, (2, 1, 0), glass);
        put(&mut upward, (0, 1, 0), source);
        put(&mut upward, (1, 1, 0), wire); // нижняя пыль на полу
        put(&mut upward, (2, 2, 0), wire); // верхняя пыль на стекле

        assert_eq!(wire_power_of(&block(&upward, (1, 1, 0))), 15);
        assert_eq!(wire_power_of(&block(&upward, (2, 2, 0))), 14, "вверх не прошло");
    }

    /// Блок редстоуна питает пыль и диоды, но соседние блоки — нет.
    #[test]
    fn a_block_of_redstone_does_not_power_blocks() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let redstone_block = blocks::state_by_name("redstone_block").unwrap();

        // Пыль рядом — питается.
        put(&mut world, (0, 1, 0), redstone_block);
        put(&mut world, (1, 1, 0), wire);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);

        // А через камень — нет: камень рядом с блоком редстоуна не запитан.
        put(&mut world, (0, 2, 0), STONE);
        put(&mut world, (1, 2, 0), wire);
        assert_eq!(wire_power_of(&block(&world, (1, 2, 0))), 0);
    }

    /// Поршень выдвигается от сигнала и толкает блок, а потеряв питание —
    /// задвигается.
    #[test]
    fn a_piston_pushes_and_retracts() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE); // что толкать
        put(&mut world, (0, 2, 0), lever); // рычаг сверху на поршне

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);

        // Голова встала перед поршнем, камень уехал на блок вперёд.
        assert!(block(&world, (0, 1, 0)).is("extended", "true"));
        assert_eq!(block(&world, (1, 1, 0)).kind, Kind::PistonHead);
        assert_eq!(world.get_block(2, 1, 0), STONE);

        // Выключили — голова убралась, камень остался на месте.
        use_block(&mut world, (0, 2, 0));
        settle(&mut world);
        run(&mut world, 6);

        assert!(block(&world, (0, 1, 0)).is("extended", "false"));
        assert_eq!(world.get_block(1, 1, 0), AIR);
        assert_eq!(world.get_block(2, 1, 0), STONE);
    }

    /// Едущие блоки толкают сущности два такта после такта начала хода:
    /// с половины пути, потом с полного; потом блок встаёт и больше не
    /// толкает. Предмет шириной 0,25 серединой в полублоке перед головой
    /// уезжает на 0,135 и 0,5 — всего 0,635, как 2601/4096 у оригинала
    /// (piston-push-items).
    #[test]
    fn moving_blocks_shove_entities_for_two_ticks() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));
        settle(&mut world);

        let mut item = (1.375, 1.0, 0.375);
        let mut pushes = Vec::new();

        for _ in 0..5 {
            for (pos, kind) in world.advance(crate::tick::PER_TICK) {
                if kind == crate::world::TickKind::Block {
                    ticked(&mut world, pos);
                }
                settle(&mut world);
            }

            while let Some((place, kind, param)) = world.take_block_event() {
                block_event(&mut world, place, kind, param);
                settle(&mut world);
            }

            for shove in shoves(&world) {
                let max = (item.0 + 0.25, item.1 + 0.25, item.2 + 0.25);

                if let Some(distance) = shove_distance(&shove, item, max) {
                    assert_eq!(shove.dir, (1.0, 0.0, 0.0));
                    item.0 += distance;
                    pushes.push(distance);
                }
            }

            finish_due_moves(&mut world);
            settle(&mut world);
        }

        assert_eq!(pushes.len(), 2, "толкает ровно два такта: {pushes:?}");
        assert!((pushes[0] - 0.135).abs() < 1e-9);
        assert!((pushes[1] - 0.5).abs() < 1e-9);
        assert!(((item.0 - 1.375) * 4096.0 - 2601.0).abs() < 0.5);
        assert_eq!(block(&world, (1, 1, 0)).kind, Kind::PistonHead);
    }

    /// Сдвиг — до края едущего блока и ещё на 0,01, но не больше 0,51 за
    /// такт; незадетое не двигается, сдвиг против хода — тоже.
    #[test]
    fn a_shove_reaches_the_edge_and_no_further() {
        let east = Shove { min: (0.5, 0.0, 0.0), max: (1.5, 1.0, 1.0), dir: (1.0, 0.0, 0.0), bounce: false, drag: false };

        // Игрок шириной 0,6 серединой в 3,5 перед головой в полпути.
        let west = Shove { min: (2.5, 0.0, 0.0), max: (3.5, 1.0, 1.0), dir: (1.0, 0.0, 0.0), bounce: false, drag: false };
        let pushed = shove_distance(&west, (3.2, 0.0, 0.2), (3.8, 1.8, 0.8)).expect("задет");
        assert!((pushed - 0.31).abs() < 1e-9);

        assert_eq!(shove_distance(&east, (1.5, 0.0, 0.0), (1.75, 0.25, 0.25)), None, "касание — не задет");
        assert_eq!(shove_distance(&east, (0.6, 0.0, 0.0), (0.85, 0.25, 0.25)), Some(SHOVE_LIMIT));

        let down = Shove { min: (0.0, -0.5, 0.0), max: (1.0, 0.5, 1.0), dir: (0.0, -1.0, 0.0), bounce: false, drag: false };
        let pushed = shove_distance(&down, (0.2, -0.8, 0.2), (0.4, -0.3, 0.4)).expect("задет");
        assert!((pushed - 0.21).abs() < 1e-9);
    }

    /// Толкает настоящая форма: голова — плитой у передней грани и штоком,
    /// мёд — ниже на 1/16, нижняя плита — нижней половиной; выдвигаемая
    /// слизь подбрасывает, втягиваемая — нет.
    #[test]
    fn moving_blocks_push_with_their_real_shape() {
        let head = push_boxes(AIR, Some(Dir::East));
        assert_eq!(head.len(), 2);
        assert_eq!(head[0], ((0.75, 0.0, 0.0), (1.0, 1.0, 1.0)), "плита");
        assert_eq!(head[1], ((0.0, 0.375, 0.375), (0.75, 0.625, 0.625)), "шток");

        let down = push_boxes(AIR, Some(Dir::Down));
        assert_eq!(down[0], ((0.0, 0.0, 0.0), (1.0, 0.25, 1.0)));
        assert_eq!(down[1], ((0.375, 0.25, 0.375), (0.625, 1.0, 0.625)));

        let honey = push_boxes(blocks::state_by_name("honey_block").unwrap(), None);
        assert_eq!(honey, vec![((0.0625, 0.0, 0.0625), (0.9375, 0.9375, 0.9375))]);

        let slab = push_boxes(state("stone_slab", &[("type", "bottom"), ("waterlogged", "false")]), None);
        assert_eq!(slab, vec![((0.0, 0.0, 0.0), (1.0, 0.5, 1.0))]);

        let stone = push_boxes(blocks::state_by_name("stone").unwrap(), None);
        assert_eq!(stone, vec![((0.0, 0.0, 0.0), (1.0, 1.0, 1.0))]);

        // Выдвигаемая слизь подбрасывает, втягиваемая — нет.
        let mut world = world();
        let piston = state("sticky_piston", &[("facing", "up"), ("extended", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 2, 0), slime);
        put(&mut world, (1, 1, 0), blocks::state_by_name("redstone_block").unwrap());
        settle(&mut world);

        let mut bounced = Vec::new();

        for round in 0..12 {
            if round == 6 {
                put(&mut world, (1, 1, 0), AIR);
                settle(&mut world);
            }

            for (pos, kind) in world.advance(crate::tick::PER_TICK) {
                if kind == crate::world::TickKind::Block {
                    ticked(&mut world, pos);
                }
                settle(&mut world);
            }

            while let Some((place, kind, param)) = world.take_block_event() {
                block_event(&mut world, place, kind, param);
                settle(&mut world);
            }

            for shove in shoves(&world) {
                bounced.push((shove.dir, shove.bounce));
            }

            finish_due_moves(&mut world);
            settle(&mut world);
        }

        assert!(bounced.contains(&((0.0, 1.0, 0.0), true)), "вверх подбрасывает: {bounced:?}");
        assert!(bounced.contains(&((0.0, -1.0, 0.0), false)), "вниз тянет без подброса: {bounced:?}");
        assert!(!bounced.contains(&((0.0, -1.0, 0.0), true)));
    }

    /// Мёд везёт лежащее на его верху на шаг, висящее без опоры и лежащее
    /// рядом — нет; слизь не везёт.
    #[test]
    fn moving_honey_drags_what_rests_on_top() {
        // Мёд после шага на восток: рамка 1,5625…2,4375, верх 0,9375.
        let honey = Shove { min: (1.5625, 0.0, 0.0625), max: (2.4375, 0.9375, 0.9375), dir: (1.0, 0.0, 0.0), bounce: false, drag: true };
        let item = |x: f64, y: f64| ((x - 0.125, y, 0.375), (x + 0.125, y + 0.25, 0.625));

        let (min, max) = item(1.5, 0.9375);
        assert_eq!(drags(&honey, true, min, max), Some(SHOVE_STEP));
        assert_eq!(drags(&honey, false, min, max), None, "не на земле");

        let (min, max) = item(1.5, 1.0);
        assert_eq!(drags(&honey, true, min, max), Some(SHOVE_STEP), "наш предмет лежит на верху клетки");

        let (min, max) = item(3.5, 0.9375);
        assert_eq!(drags(&honey, true, min, max), None, "рядом");

        let (min, max) = item(1.5, 2.0);
        assert_eq!(drags(&honey, true, min, max), None, "выше");

        let slime = Shove { drag: false, ..honey };
        let (min, max) = item(1.5, 1.0);
        assert_eq!(drags(&slime, true, min, max), None);
    }

    /// Толкнутый упирается в стену: проходит до неё и не дальше; клетка,
    /// в которую рамка уже залезла, не мешает; без стены — весь сдвиг.
    #[test]
    fn a_shoved_box_stops_at_a_wall() {
        let wall = |x: i32, _y: i32, z: i32| x == 4 && z == 3;
        let east = (1.0, 0.0, 0.0);
        let west = (-1.0, 0.0, 0.0);

        let run = free_run((3.2, 1.0, 3.2), (3.8, 2.8, 3.8), east, 0.31, &wall);
        assert!((run - 0.2).abs() < 1e-9, "{run}");

        assert!((free_run((3.2, 1.0, 1.2), (3.8, 2.8, 1.8), east, 0.31, &wall) - 0.31).abs() < 1e-9);
        assert!((free_run((4.2, 1.0, 3.2), (4.8, 2.8, 3.8), east, 0.31, &wall) - 0.31).abs() < 1e-9, "уже внутри");

        let back = free_run((5.3, 1.0, 3.2), (5.9, 2.8, 3.8), west, 0.5, &wall);
        assert!((back - 0.3).abs() < 1e-9, "{back}");
    }

    /// Липкий поршень тянет блок обратно, обычный — нет.
    #[test]
    fn a_sticky_piston_pulls_the_block_back() {
        let sticky = state("sticky_piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        let mut world = world();
        put(&mut world, (0, 1, 0), sticky);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);
        assert_eq!(world.get_block(2, 1, 0), STONE);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);

        // Блок вернулся вплотную к поршню.
        assert_eq!(world.get_block(1, 1, 0), STONE);
        assert_eq!(world.get_block(2, 1, 0), AIR);
    }

    /// Двенадцать блоков поршень толкает, тринадцать — нет.
    #[test]
    fn a_piston_pushes_at_most_twelve_blocks() {
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        let build = |count: i32| {
            let mut world = world();

            put(&mut world, (0, 1, 0), piston);
            for x in 1..=count {
                put(&mut world, (x, 1, 0), STONE);
            }
            put(&mut world, (0, 2, 0), lever);
            use_block(&mut world, (0, 2, 0));
            settle(&mut world);
            run(&mut world, 6);

            world
        };

        let twelve = build(12);
        assert!(block(&twelve, (0, 1, 0)).is("extended", "true"), "двенадцать не поехали");
        assert_eq!(twelve.get_block(13, 1, 0), STONE);

        let thirteen = build(13);
        assert!(
            block(&thirteen, (0, 1, 0)).is("extended", "false"),
            "тринадцать поехали, хотя не должны"
        );
    }

    /// Обсидиан поршень не двигает вовсе, а траву ломает.
    #[test]
    fn immovable_blocks_stop_the_piston() {
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let obsidian = blocks::state_by_name("obsidian").unwrap();
        let grass = blocks::state_by_name("short_grass").unwrap();

        // Обсидиан за камнем: поршень стоит на месте.
        let mut stuck = world();
        put(&mut stuck, (0, 1, 0), piston);
        put(&mut stuck, (1, 1, 0), STONE);
        put(&mut stuck, (2, 1, 0), obsidian);
        put(&mut stuck, (0, 2, 0), lever);

        use_block(&mut stuck, (0, 2, 0));
        run(&mut stuck, 6);

        assert!(block(&stuck, (0, 1, 0)).is("extended", "false"));
        assert_eq!(stuck.get_block(1, 1, 0), STONE);

        // Трава ломается, поршень выдвигается.
        let mut through_grass = world();
        put(&mut through_grass, (0, 1, 0), piston);
        // Траве нужна земля: на каменном полу она не стоит.
        put(&mut through_grass, (1, 0, 0), blocks::state_by_name("dirt").unwrap());
        put(&mut through_grass, (1, 1, 0), grass);
        put(&mut through_grass, (0, 2, 0), lever);

        use_block(&mut through_grass, (0, 2, 0));
        run(&mut through_grass, 6);

        assert!(block(&through_grass, (0, 1, 0)).is("extended", "true"));
        assert_eq!(block(&through_grass, (1, 1, 0)).kind, Kind::PistonHead);
    }

    /// Квазисвязность: пыль, питающая клетку над поршнем, включает и сам
    /// поршень, хотя его самого она не касается.
    #[test]
    fn a_piston_listens_above_itself() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let source = blocks::state_by_name("redstone_block").unwrap();

        // Пыль лежит с севера от клетки, которая прямо над поршнем, и
        // смотрит в неё. Самого поршня она не касается.
        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 1, -1), STONE); // подставка под пыль
        put(&mut world, (0, 2, -1), wire);
        put(&mut world, (0, 2, -2), source);

        run(&mut world, 6);

        assert!(
            block(&world, (0, 1, 0)).is("extended", "true"),
            "поршень не увидел сигнал над собой"
        );

        // А блок редстоуна наискось поршень, как у оригинала, сразу не
        // замечает: обновления от блока до него не дотягиваются. Заметит,
        // когда его обновит что-то вплотную (вики, Piston: «Update-based QC
        // Activation by Block of Redstone»).
        let mut diagonal = super::tests::world();
        put(&mut diagonal, (0, 1, 0), piston);
        put(&mut diagonal, (1, 2, 0), source);
        run(&mut diagonal, 6);

        assert!(
            block(&diagonal, (0, 1, 0)).is("extended", "false"),
            "поршень заметил блок наискось без обновления"
        );

        put(&mut diagonal, (0, 1, 1), STONE);
        run(&mut diagonal, 6);

        assert!(
            block(&diagonal, (0, 1, 0)).is("extended", "true"),
            "поршень не заметил блок наискось и после обновления"
        );
    }

    /// Сломали поршень — голова пропадает сама.
    #[test]
    fn the_head_goes_with_the_piston() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 2, 0), lever);
        use_block(&mut world, (0, 2, 0));
        settle(&mut world);
        run(&mut world, 6);

        assert_eq!(block(&world, (1, 1, 0)).kind, Kind::PistonHead);

        put(&mut world, (0, 1, 0), AIR);

        assert_eq!(world.get_block(1, 1, 0), AIR, "голова осталась без поршня");
    }

    /// Пыль тянется к сравнителю со всех четырёх сторон: у него есть и
    /// боковые входы. К повторителю — только с торцов.
    #[test]
    fn a_wire_reaches_a_comparator_from_the_side() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let comparator = state("comparator", &[("facing", "west"), ("mode", "subtract")]);
        let repeater = state("repeater", &[("facing", "west"), ("delay", "1")]);

        // Сравнитель смотрит вдоль оси запад-восток, пыль стоит сбоку, с юга.
        put(&mut world, (0, 1, 0), comparator);
        put(&mut world, (0, 1, 1), wire);

        assert!(
            block(&world, (0, 1, 1)).is("north", "side"),
            "пыль не потянулась к сравнителю сбоку"
        );

        // И этот бок сравнитель слышит: вычитаем боковой сигнал из заднего.
        let source = blocks::state_by_name("redstone_block").unwrap();
        put(&mut world, (1, 1, 0), source); // задний вход, сила 15
        put(&mut world, (0, 1, 2), source); // питаем боковую пыль
        put(&mut world, (-1, 1, 0), wire); // выход
        run(&mut world, 6);

        // Сбоку пришло 15, сзади 15: в режиме вычитания на выходе ноль.
        assert_eq!(wire_power_of(&block(&world, (-1, 1, 0))), 0);

        // А к боку повторителя пыль не тянется.
        let mut beside_repeater = super::tests::world();
        put(&mut beside_repeater, (0, 1, 0), repeater);
        put(&mut beside_repeater, (0, 1, 1), wire);

        assert!(block(&beside_repeater, (0, 1, 1)).is("north", "none"));
    }

    /// Выдвинутый поршень ломается целиком: и с какой стороны его ни ломай,
    /// второй половины не остаётся. Но это делает именно слом игроком —
    /// от обычного обновления безголовое основание остаётся стоять
    /// («Headless pistons do not break upon receiving a block update»).
    #[test]
    fn an_extended_piston_breaks_whole() {
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        let extended = || {
            let mut world = world();

            put(&mut world, (0, 1, 0), piston);
            put(&mut world, (0, 2, 0), lever);
            use_block(&mut world, (0, 2, 0));
            settle(&mut world);
            run(&mut world, 6);

            assert_eq!(block(&world, (1, 1, 0)).kind, Kind::PistonHead);
            world
        };

        // Сломали основание — голова пропала.
        let mut base_gone = extended();
        put(&mut base_gone, (0, 1, 0), AIR);
        run(&mut base_gone, 6);

        assert_eq!(base_gone.get_block(1, 1, 0), AIR, "голова осталась без поршня");

        // Сломали голову — вторую половину называет piston_pair, и её
        // убирает тот, кто ломает.
        let head_gone = extended();
        let head = head_gone.get_block(1, 1, 0);

        assert_eq!(
            piston_pair(&head_gone, (1, 1, 0), head),
            Some((0, 1, 0)),
            "вторая половина не найдена"
        );

        // А само по себе обновление основание не рушит.
        let mut headless = extended();
        put(&mut headless, (1, 1, 0), AIR);
        run(&mut headless, 5);

        assert_eq!(
            block(&headless, (0, 1, 0)).kind,
            Kind::Piston,
            "безголовое основание рассыпалось само"
        );
    }

    /// Раздавленное поршнем не пропадает бесследно: из него выпадает предмет.
    #[test]
    fn a_crushed_block_leaves_an_item() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let grass = blocks::state_by_name("short_grass").unwrap();

        put(&mut world, (0, 1, 0), piston);
        // Траве нужна земля: на каменном полу она не стоит.
        put(&mut world, (1, 0, 0), blocks::state_by_name("dirt").unwrap());
        put(&mut world, (1, 1, 0), grass);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);

        let destroyed = world.take_destroyed();

        assert!(
            destroyed.iter().any(|(place, state)| *place == (1, 1, 0) && *state == grass),
            "трава пропала без следа"
        );
    }

    /// Слизь тянет за собой соседа: поршень толкает слизь, а вместе с ней
    /// едет и камень, приклеившийся сбоку.
    #[test]
    fn a_slime_block_drags_its_neighbour() {
        // Схема висит в воздухе: на земле слизь цеплялась бы за пол и тянула
        // бы его — это верно, но проверять мы хотим не это.
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();

        put(&mut world, (0, 5, 0), piston);
        put(&mut world, (1, 5, 0), slime);
        put(&mut world, (1, 6, 0), STONE); // сосед сверху — приклеится
        put(&mut world, (0, 6, 0), lever);

        use_block(&mut world, (0, 6, 0));

        settle(&mut world);
        run(&mut world, 6);

        assert_eq!(world.get_block(2, 5, 0), slime, "слизь не поехала");
        assert_eq!(world.get_block(2, 6, 0), STONE, "сосед не поехал со слизью");
        assert_eq!(world.get_block(1, 6, 0), AIR);
    }

    /// Слизь и мёд друг к другу не липнут.
    #[test]
    fn slime_and_honey_do_not_stick_together() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();
        let honey = blocks::state_by_name("honey_block").unwrap();

        put(&mut world, (0, 5, 0), piston);
        put(&mut world, (1, 5, 0), slime);
        put(&mut world, (1, 6, 0), honey); // мёд сверху — не приклеится
        put(&mut world, (0, 6, 0), lever);

        use_block(&mut world, (0, 6, 0));

        settle(&mut world);
        run(&mut world, 6);

        assert_eq!(world.get_block(2, 5, 0), slime);
        assert_eq!(world.get_block(1, 6, 0), honey, "мёд поехал со слизью");
    }

    /// Липкий поршень тянет обратно слизь вместе с её связкой.
    #[test]
    fn a_sticky_piston_pulls_a_whole_group() {
        let mut world = world();
        let sticky = state("sticky_piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();

        put(&mut world, (0, 5, 0), sticky);
        put(&mut world, (1, 5, 0), slime);
        put(&mut world, (1, 6, 0), STONE);
        put(&mut world, (0, 6, 0), lever);

        use_block(&mut world, (0, 6, 0));

        settle(&mut world);
        run(&mut world, 6);
        assert_eq!(world.get_block(2, 5, 0), slime);

        use_block(&mut world, (0, 6, 0));

        settle(&mut world);
        run(&mut world, 6);

        assert_eq!(world.get_block(1, 5, 0), slime, "слизь не вернулась");
        assert_eq!(world.get_block(1, 6, 0), STONE, "сосед не вернулся со слизью");
    }

    /// Связка со слизью считается целиком: тринадцать блоков поршень не сдвинет.
    #[test]
    fn a_sticky_group_counts_towards_the_limit() {
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();

        // Слизь тянет ряд блоков над собой: вместе выходит больше двенадцати.
        let mut world = world();

        put(&mut world, (0, 5, 0), piston);
        put(&mut world, (1, 5, 0), slime);

        for x in 1..=12 {
            put(&mut world, (x, 6, 0), STONE);
        }

        put(&mut world, (0, 6, 0), lever);
        use_block(&mut world, (0, 6, 0));
        settle(&mut world);
        run(&mut world, 6);

        assert!(
            block(&world, (0, 5, 0)).is("extended", "false"),
            "поршень сдвинул больше двенадцати блоков"
        );
    }

    /// Поршень, которому не хватило места, сам не оживает: он ждёт, пока
    /// его потревожат.
    ///
    /// Так и в игре: поршень пересчитывает себя не постоянно, а когда рядом
    /// что-то изменилось. На этом построены «определители обновления блока».
    #[test]
    fn a_stuck_piston_waits_for_an_update() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        for x in 1..=13 {
            put(&mut world, (x, 1, 0), STONE);
        }
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);

        assert!(block(&world, (0, 1, 0)).is("extended", "false"), "сдвинул тринадцать");

        // Убрали дальний блок — их осталось двенадцать, но поршень об этом
        // не знает: до него оттуда никакие изменения не доходят.
        put(&mut world, (13, 1, 0), AIR);
        run(&mut world, 6);

        assert!(
            block(&world, (0, 1, 0)).is("extended", "false"),
            "ожил сам, хотя его никто не тревожил"
        );

        // А теперь тронули соседний с ним блок — и он пересчитался.
        put(&mut world, (0, 1, 1), STONE);
        run(&mut world, 6);

        assert!(
            block(&world, (0, 1, 0)).is("extended", "true"),
            "не ожил даже после того, как его потревожили"
        );
    }


    /// Блок красного камня по диагонали сверху — поршень-«BUD», как у
    /// оригинала: не замечает ни появления, ни исчезновения блока, пока его
    /// не обновит что-то вплотную, и так и остаётся выдвинутым.
    /// Липкий поршень от импульса наблюдателя в 2 такта: дважды подряд —
    /// как в сценарии чёрного ящика sticky-spit.
    #[test]
    fn sticky_piston_and_two_tick_pulses() {
        let mut world = world();
        let sticky = state("sticky_piston", &[("facing", "east"), ("extended", "false")]);
        let observer = state("observer", &[("facing", "west"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), sticky);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (-1, 1, 0), observer);
        run(&mut world, 5);

        let where_stone = |world: &World| {
            [(1, 1, 0), (2, 1, 0)].into_iter().filter(|pos| world.get_block(pos.0, pos.1, pos.2) == STONE).collect::<Vec<_>>()
        };

        put(&mut world, (-2, 1, 0), STONE);

        for tick in 0..8 {
            run(&mut world, 1);
            eprintln!("шаг 1, такт {}: камень {:?}, поршень {:?}", tick, where_stone(&world), block(&world, (0, 1, 0)).value("extended"));
        }

        put(&mut world, (-2, 1, 0), AIR);

        for tick in 0..8 {
            run(&mut world, 1);
            eprintln!("шаг 2, такт {}: камень {:?}, поршень {:?}", tick, where_stone(&world), block(&world, (0, 1, 0)).value("extended"));
        }
    }

    #[test]
    fn a_diagonal_redstone_block_makes_a_bud_piston() {
        let mut world = world();
        let redstone_block = blocks::state_by_name("redstone_block").unwrap();
        // Поршень смотрит на восток: голова встанет в (1, 1, 0). Блок
        // наискось над поршнем (-1, 2, 0) не касается ни поршня, ни головы.
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);

        let extended = |world: &World| block(world, (0, 1, 0)).is("extended", "true");

        put(&mut world, (0, 1, 0), piston);
        run(&mut world, 5);
        assert!(!extended(&world), "поршень выдвинулся без причины");

        // Блок наискось — поршень молчит.
        put(&mut world, (-1, 2, 0), redstone_block);
        run(&mut world, 5);
        assert!(!extended(&world), "поршень заметил блок наискось без обновления");

        // Обновили вплотную — выдвинулся.
        put(&mut world, (0, 1, -1), STONE);
        run(&mut world, 5);
        assert!(extended(&world), "поршень не выдвинулся от обновления");

        // Блок сняли — остаётся выдвинутым: обновление до него не дошло.
        put(&mut world, (-1, 2, 0), AIR);
        run(&mut world, 5);
        assert!(extended(&world), "поршень задвинулся без обновления");

        // Обновили снова — задвинулся.
        put(&mut world, (0, 1, -1), AIR);
        run(&mut world, 5);
        assert!(!extended(&world), "поршень не задвинулся от обновления");
    }


    /// Приоритет такта диода зависит от того, куда он смотрит: в зад или в бок
    /// соседнего диода — раньше всех, в лицо — как обычно.
    ///
    /// По вики: приоритет −3 у повторителя, который смотрит «в зад или в бок»
    /// другого повторителя или сравнителя.
    #[test]
    fn a_diode_knows_whether_it_faces_the_back_of_another() {
        // Наш повторитель стоит в начале и выдаёт сигнал на восток.
        let ours = state("repeater", &[("facing", "west"), ("delay", "1")]);

        let check = |their_facing: &str| {
            let mut world = world();
            put(&mut world, (0, 1, 0), ours);
            put(
                &mut world,
                (1, 1, 0),
                state("repeater", &[("facing", their_facing), ("delay", "1")]),
            );

            faces_diode(&world, (0, 1, 0), &block(&world, (0, 1, 0)))
        };

        // Сосед тоже выдаёт на восток — значит его вход обращён к нам,
        // мы смотрим ему в зад.
        assert!(check("west"), "зад соседа не распознан");

        // Сосед выдаёт навстречу — мы смотрим ему в лицо.
        assert!(!check("east"), "лицо соседа принято за зад");

        // Сосед развёрнут поперёк — это бок.
        assert!(check("north"), "бок соседа не распознан");
        assert!(check("south"), "бок соседа не распознан");
    }


    /// Детали из списка вики «Some changes do not produce General NC
    /// updates» при смене состояния соседей не предупреждают. На этом стоят
    /// «определители обновления блока», поэтому список надо соблюдать точно.
    #[test]
    fn some_parts_change_without_telling_the_neighbours() {
        for kind in [
            Kind::Repeater,
            Kind::Comparator,
            Kind::Lamp,
            Kind::Door,
            Kind::Trapdoor,
            Kind::Gate,
            Kind::Plate,
            Kind::Observer,
            Kind::Dispenser,
            Kind::Hopper,
            // У пыли вместо общего обновления — свои 42 (поздние).
            Kind::Wire,
        ] {
            assert!(keeps_quiet(kind), "{kind:?} должен молчать");
        }

        // А все остальные — предупреждают, как обычные блоки.
        for kind in [Kind::Torch, Kind::Lever, Kind::Button, Kind::Piston, Kind::Other] {
            assert!(!keeps_quiet(kind), "{kind:?} молчать не должен");
        }
    }

    /// Загоревшаяся лампа соседей не будит: поставили её вплотную к факелу,
    /// и его состояние от её огня не зависит.
    #[test]
    fn a_lamp_lighting_up_changes_nothing_around() {
        let mut world = world();
        let redstone_block = blocks::state_by_name("redstone_block").unwrap();
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();

        put(&mut world, (0, 1, 0), lamp);
        put(&mut world, (0, 2, 0), redstone_block);
        run(&mut world, 5);

        assert!(block(&world, (0, 1, 0)).is("lit", "true"), "лампа не загорелась");

        // Лампа непроводящая, и запитать через неё ничего нельзя —
        // ни до, ни после того, как она загорелась.
        assert_eq!(weak_power(&world, (0, 1, 0)), 0, "лампа стала передавать сигнал");
    }



    /// Кнопка отпускается даже если на её месте уже был запланирован такт:
    /// иначе она залипла бы навсегда.
    #[test]
    fn a_button_always_gets_its_release() {
        let mut world = world();
        let button = state(
            "stone_button",
            &[("face", "floor"), ("facing", "north"), ("powered", "false")],
        );

        put(&mut world, (0, 1, 0), button);

        // Занимаем место заявкой, как это может сделать соседний блок.
        world.schedule_once(0, 1, 0, 40, PRIORITY_NORMAL);

        assert!(use_block(&mut world, (0, 1, 0)), "кнопка не нажалась");
        assert!(block(&world, (0, 1, 0)).is("powered", "true"));

        // Каменная кнопка держится 20 тактов — за 25 должна отпуститься.
        run(&mut world, 25);

        assert!(
            block(&world, (0, 1, 0)).is("powered", "false"),
            "кнопка залипла"
        );
    }


    /// Плита нажимается тем, кто на ней стоит, и отпускается не сразу,
    /// а через секунду после того, как с неё сошли.
    #[test]
    fn a_pressure_plate_answers_to_whoever_stands_on_it() {
        let mut world = world();
        let plate = blocks::state_by_name("oak_pressure_plate").unwrap();
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();

        put(&mut world, (0, 1, 0), plate);
        put(&mut world, (1, 1, 0), lamp);
        run(&mut world, 6);

        assert!(!block(&world, (1, 1, 0)).is("lit", "true"), "лампа горит без нужды");

        // Встали на плиту.
        step_plates(&mut world, &[Occupant::player(0.5, 1.0, 0.5, false)]);
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "плита не нажалась");
        assert!(block(&world, (1, 1, 0)).is("lit", "true"), "лампа не загорелась");

        // Сошли — плита держится ещё секунду.
        for _ in 0..10 {
            for (pos, _) in world.advance(crate::tick::PER_TICK) {
                ticked(&mut world, pos);
            }

            step_plates(&mut world, &[]);
            settle(&mut world);
        }

        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "плита отпустилась слишком рано");

        for _ in 0..15 {
            for (pos, _) in world.advance(crate::tick::PER_TICK) {
                ticked(&mut world, pos);
            }

            step_plates(&mut world, &[]);
            settle(&mut world);
        }

        assert!(!block(&world, (0, 1, 0)).is("powered", "true"), "плита не отпустилась");
        assert!(!block(&world, (1, 1, 0)).is("lit", "true"), "лампа не погасла");
    }

    /// У золотой плиты сила равна числу лежащих на ней предметов,
    /// у железной — одна единица за каждый десяток.
    #[test]
    fn weighted_plates_count_what_lies_on_them() {
        let light = Block {
            state: blocks::state_by_name("light_weighted_pressure_plate").unwrap(),
            name: "light_weighted_pressure_plate",
            kind: Kind::Plate,
            orientation: blocks::orientation("light_weighted_pressure_plate"),
        };

        let heavy = Block {
            state: blocks::state_by_name("heavy_weighted_pressure_plate").unwrap(),
            name: "heavy_weighted_pressure_plate",
            kind: Kind::Plate,
            orientation: blocks::orientation("heavy_weighted_pressure_plate"),
        };

        let power_of = |block: &Block, count: u32| {
            let state = pressed_state(block, count).expect("состояние есть");
            let mut with = *block;
            with.state = state;
            plate_power(&with)
        };

        assert_eq!(power_of(&light, 0), 0);
        assert_eq!(power_of(&light, 3), 3);
        assert_eq!(power_of(&light, 40), 15, "сила не растёт выше пятнадцати");

        assert_eq!(power_of(&heavy, 0), 0);
        assert_eq!(power_of(&heavy, 1), 1);
        assert_eq!(power_of(&heavy, 10), 1);
        assert_eq!(power_of(&heavy, 11), 2);
    }


    /// Калитка открывается и рукой, и сигналом — как дверь.
    #[test]
    fn a_fence_gate_opens_by_hand_and_by_signal() {
        let mut world = world();
        let gate = blocks::state_by_name("oak_fence_gate").unwrap();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), gate);

        // Рукой.
        assert!(use_block(&mut world, (0, 1, 0)), "калитка не открылась рукой");
        assert!(block(&world, (0, 1, 0)).is("open", "true"));

        assert!(use_block(&mut world, (0, 1, 0)));

        settle(&mut world);
        assert!(block(&world, (0, 1, 0)).is("open", "false"), "калитка не закрылась");

        // Сигналом: рычаг на блоке рядом.
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (1, 2, 0), lever);
        assert!(use_block(&mut world, (1, 2, 0)));
        settle(&mut world);
        settle(&mut world);
        run(&mut world, 6);

        assert!(block(&world, (0, 1, 0)).is("open", "true"), "калитка не открылась от сигнала");
    }


    /// Наблюдатель даёт короткий сигнал назад, когда меняется блок у него
    /// перед глазами, и сам его снимает.
    #[test]
    fn an_observer_pulses_when_what_it_watches_changes() {
        let mut world = world();
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();

        // Смотрит на восток, значит сигнал выдаёт на запад — в лампу.
        put(&mut world, (0, 1, 0), state("observer", &[("facing", "east")]));
        put(&mut world, (-1, 1, 0), lamp);
        run(&mut world, 5);

        assert!(!block(&world, (-1, 1, 0)).is("lit", "true"), "лампа горит без нужды");

        // Ставим блок перед глазами.
        put(&mut world, (1, 1, 0), STONE);
        run(&mut world, 3);

        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "наблюдатель не заметил");
        assert!(block(&world, (-1, 1, 0)).is("lit", "true"), "сигнал не дошёл до лампы");

        // Сигнал короткий: сам снимается.
        run(&mut world, 10);
        assert!(!block(&world, (0, 1, 0)).is("powered", "true"), "сигнал не снялся");

        // А за спиной у него ничего не происходит.
        put(&mut world, (0, 1, 1), STONE);
        run(&mut world, 5);
        assert!(!block(&world, (0, 1, 0)).is("powered", "true"), "наблюдатель смотрит не туда");
    }


    /// Что поршень двигает, а что ломает, взято со страницы Piston/Table,
    /// а не угадывается по виду блока.
    #[test]
    fn the_piston_knows_what_breaks_and_what_rides() {
        let reaction = |name: &str| {
            push_reaction(blocks::state_by_name(name).unwrap_or_else(|| panic!("нет блока {name}")))
        };

        // Ломаются, хотя с виду обычные блоки.
        for name in [
            "melon",
            "pumpkin",
            "carved_pumpkin",
            "jack_o_lantern",
            "moss_block",
            "oak_leaves",
            "cactus",
            "ladder",
            "lantern",
            "bell",
            "turtle_egg",
            "flower_pot",
            "sea_pickle",
            "campfire",
            "lily_pad",
            "bamboo",
            "scaffolding",
            "dragon_egg",
            "anvil",
            "player_head",
            "candle",
        ] {
            assert_eq!(reaction(name), Push::Breaks, "{name} должен ломаться");
        }

        // Рельсы ящика не имеют, но едут.
        for name in ["rail", "powered_rail", "detector_rail", "activator_rail"] {
            assert_eq!(reaction(name), Push::Moves, "{name} должен ехать");
        }

        // А обычные блоки как ездили, так и ездят.
        for name in ["stone", "oak_planks", "redstone_lamp", "slime_block"] {
            assert_eq!(reaction(name), Push::Moves, "{name} должен ехать");
        }

        // И то, что не двигается вовсе.
        for name in ["obsidian", "chest", "furnace"] {
            assert_eq!(reaction(name), Push::Stays, "{name} должен стоять");
        }
    }


    /// Глазурованную керамику липкий поршень толкает, но не тянет обратно.
    #[test]
    fn glazed_terracotta_is_pushed_but_never_pulled() {
        let mut world = world();
        let piston = state("sticky_piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let glazed = blocks::state_by_name("white_glazed_terracotta").unwrap();

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), glazed);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 5);

        assert_eq!(world.get_block(2, 1, 0), glazed, "керамику не толкнуло");

        // Задвигаем — керамика должна остаться на месте.
        use_block(&mut world, (0, 2, 0));
        settle(&mut world);
        settle(&mut world);
        run(&mut world, 5);

        assert_eq!(world.get_block(2, 1, 0), glazed, "керамику притянуло обратно");
        assert_eq!(world.get_block(1, 1, 0), AIR, "на месте головы что-то осталось");
    }


    /// Медная лампа переключается по приходу сигнала, а не горит, пока он
    /// есть: подали — загорелась, сняли — горит дальше, подали снова — погасла.
    #[test]
    fn a_copper_bulb_switches_on_each_pulse() {
        let mut world = world();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), blocks::state_by_name("copper_bulb").unwrap());
        put(&mut world, (1, 1, 0), lever);

        assert!(!block(&world, (0, 1, 0)).is("lit", "true"));

        // Сигнал пришёл — загорелась.
        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        settle(&mut world);
        run(&mut world, 6);
        assert!(block(&world, (0, 1, 0)).is("lit", "true"), "не загорелась");

        // Сигнал сняли — горит дальше.
        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        settle(&mut world);
        run(&mut world, 6);
        assert!(block(&world, (0, 1, 0)).is("lit", "true"), "погасла раньше времени");

        // Второй сигнал — гаснет.
        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        settle(&mut world);
        run(&mut world, 6);
        assert!(!block(&world, (0, 1, 0)).is("lit", "true"), "не погасла");
    }


    /// Поршень трогается в своей фазе такта, а не в общей очереди: сигнал
    /// пришёл — на этом же такте он и пошёл.
    #[test]
    fn a_piston_moves_in_its_own_phase() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);

        // Событие заведено, но ещё не выполнено.
        assert!(block(&world, (0, 1, 0)).is("extended", "false"), "поршень пошёл до своей фазы");

        // В своей фазе поршень трогается: признак «выдвинут» ставится сразу.
        run(&mut world, 1);
        assert!(block(&world, (0, 1, 0)).is("extended", "true"), "поршень не тронулся");

        // А голова появляется через два такта — ход занимает время.
        assert_ne!(block(&world, (1, 1, 0)).kind, Kind::PistonHead, "ход занял ноль тактов");

        run(&mut world, 3);
        assert_eq!(block(&world, (1, 1, 0)).kind, Kind::PistonHead, "ход не закончился");
    }




    /// Песок и гравий без опоры просятся в полёт, а на земле — стоят.
    #[test]
    fn sand_without_support_asks_to_fall() {
        let mut world = world();
        let sand = blocks::state_by_name("sand").unwrap();

        // Падает не сразу, а своим тактом через два — как у оригинала.
        // На земле стоит спокойно.
        put(&mut world, (0, 1, 0), sand);
        run(&mut world, FALL_DELAY as usize);
        assert!(world.take_falling().is_empty(), "песок на земле собрался падать");

        // А в воздухе — падает, но не в тот же такт.
        put(&mut world, (0, 5, 0), sand);
        assert!(world.take_falling().is_empty(), "песок упал без задержки");
        run(&mut world, FALL_DELAY as usize);
        assert_eq!(world.take_falling(), vec![(0, 5, 0)], "песок в воздухе не упал");

        // Убрали опору из-под лежащего — тоже падает.
        put(&mut world, (0, 2, 0), sand);
        run(&mut world, FALL_DELAY as usize);
        world.take_falling();
        put(&mut world, (0, 1, 0), AIR);
        run(&mut world, FALL_DELAY as usize);

        assert!(
            world.take_falling().contains(&(0, 2, 0)),
            "песок не заметил, что опоры не стало"
        );
    }

    /// Над водой и травой песок не висит: они заменяемые, а факел держит
    /// (вики, Falling Block: падает, когда «block below is replaceable»).
    #[test]
    fn sand_falls_through_water_and_grass_but_stands_on_a_torch() {
        let mut world = world();
        let sand = blocks::state_by_name("sand").unwrap();

        for (x, below) in [(0, "water"), (2, "short_grass"), (4, "torch")] {
            world.set_block_silently(x, 1, 0, blocks::state_by_name(below).unwrap());
            put(&mut world, (x, 2, 0), sand);
        }

        run(&mut world, FALL_DELAY as usize);
        let falling = world.take_falling();

        assert!(falling.contains(&(0, 2, 0)), "над водой висит");
        assert!(falling.contains(&(2, 2, 0)), "над травой висит");
        assert!(!falling.contains(&(4, 2, 0)), "с факела упал");
    }

    /// Поршень, раздавивший половину двери, ломает дверь целиком — и
    /// выпадает из неё одна дверь.
    #[test]
    fn a_crushed_door_breaks_whole() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let lower = state("oak_door", &[("half", "lower"), ("facing", "east")]);
        let upper = state("oak_door", &[("half", "upper"), ("facing", "east")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), lower);
        put(&mut world, (1, 2, 0), upper);
        put(&mut world, (0, 2, 0), lever);
        world.take_destroyed();

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);

        assert_eq!(world.get_block(1, 2, 0), AIR, "верх двери висит");

        let doors = world
            .take_destroyed()
            .iter()
            .filter(|(_, state)| blocks::block_at_state(*state) == Some("oak_door"))
            .count();

        assert_eq!(doors, 1);
    }

    /// Провод на нижней плите не держится, на верхней — держится.
    #[test]
    fn a_wire_does_not_stay_on_a_bottom_slab() {
        let mut world = world();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        put(&mut world, (0, 1, 0), state("oak_slab", &[("type", "top")]));
        put(&mut world, (0, 2, 0), wire);
        assert_eq!(world.get_block(0, 2, 0), wire);

        put(&mut world, (2, 1, 0), state("oak_slab", &[("type", "bottom")]));
        put(&mut world, (2, 2, 0), wire);
        assert_eq!(world.get_block(2, 2, 0), AIR);
    }

    /// Что падает, а что нет — по списку с вики.
    #[test]
    fn the_list_of_falling_blocks_is_the_one_from_the_wiki() {
        let state = |name: &str| blocks::state_by_name(name).unwrap();

        for name in ["sand", "red_sand", "gravel", "dragon_egg", "anvil", "white_concrete_powder"] {
            assert!(falls(state(name)), "{name} должен падать");
        }

        for name in ["stone", "dirt", "oak_planks", "white_concrete"] {
            assert!(!falls(state(name)), "{name} падать не должен");
        }
    }


    /// Правила липких блоков с вики: слизь и мёд тянут соседей, но друг
    /// друга не держат, а глазурованную керамику не держит никто.
    #[test]
    fn slime_and_honey_stick_by_the_rules() {
        let state = |name: &str| blocks::state_by_name(name).unwrap();

        // Слизь держит обычные блоки и такую же слизь.
        assert!(sticks_to("slime_block", state("stone")));
        assert!(sticks_to("slime_block", state("slime_block")));

        // А мёд — нет: «Slime blocks and honey blocks do not stick
        // to each other».
        assert!(!sticks_to("slime_block", state("honey_block")));
        assert!(!sticks_to("honey_block", state("slime_block")));
        assert!(sticks_to("honey_block", state("honey_block")));

        // Керамику не держит ни то, ни другое.
        assert!(!sticks_to("slime_block", state("white_glazed_terracotta")));

        // И то, что вообще не двигается, тоже не держится.
        assert!(!sticks_to("slime_block", state("obsidian")));
    }

    /// Места, которые связка на слизи освобождает и которые никто не займёт,
    /// клиенту называют пустыми — на такт позже хода (чёрный ящик,
    /// slime-limit). О месте головы при толкании молчат.
    #[test]
    fn a_pushed_slime_group_tells_the_places_it_leaves() {
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();
        let mut world = world();

        put(&mut world, (0, 5, 0), piston);
        put(&mut world, (1, 5, 0), slime);
        put(&mut world, (1, 6, 0), STONE);

        let reader = 1;
        world.watch_changes(reader);

        put(&mut world, (-1, 5, 0), blocks::state_by_name("redstone_block").unwrap());
        run(&mut world, 2);

        let (changes, _) = world.changes_since(0, reader);
        let told = |pos: Pos| changes.iter().any(|change| (change.x, change.y, change.z) == pos && change.state == AIR);

        assert!(told((1, 6, 0)), "об освободившемся месте камня не сказали");
        assert!(!told((1, 5, 0)), "место головы назвали пустым");
    }

    /// Липкий поршень, которому связка не по силам (тринадцать блоков слизи),
    /// задвигается впустую и о месте головы не говорит ничего — ни в начале,
    /// ни в конце хода (чёрный ящик, slime-limit). Если же тянуть нечего,
    /// о пустоте на месте головы говорит.
    #[test]
    fn a_sticky_piston_that_cannot_pull_keeps_quiet_about_its_head() {
        let slime = blocks::state_by_name("slime_block").unwrap();
        let head = state("piston_head", &[("facing", "east"), ("type", "sticky"), ("short", "false")]);
        let extended = state("sticky_piston", &[("facing", "east"), ("extended", "true")]);

        for (count, quiet) in [(13, true), (0, false)] {
            let mut world = world();

            for x in 2..2 + count {
                world.set_block_silently(x, 5, 0, slime);
            }

            world.set_block_silently(1, 5, 0, head);
            world.set_block_silently(0, 5, 0, extended);

            let reader = 1;
            world.watch_changes(reader);

            // Питания нет — первое же обновление его задвигает.
            put(&mut world, (-1, 5, 0), STONE);
            run(&mut world, 5);

            assert!(block(&world, (0, 5, 0)).is("extended", "false"), "поршень не задвинулся");
            assert_eq!(world.get_block(2, 5, 0), if count > 0 { slime } else { AIR }, "связка сдвинулась");

            let (changes, _) = world.changes_since(0, reader);
            let told = changes.iter().any(|change| (change.x, change.y, change.z) == (1, 5, 0));

            assert_eq!(told, !quiet, "о месте головы сказали не так (блоков: {count})");
        }
    }

    /// Наблюдатель, приехавший на поршне, срабатывает через свою обычную
    /// задержку и гаснет (вики, Observer; чёрный ящик, piston-moves-observer).
    #[test]
    fn a_moved_observer_fires_after_it_arrives() {
        let piston = state("sticky_piston", &[("facing", "east"), ("extended", "false")]);
        let observer = state("observer", &[("facing", "north"), ("powered", "false")]);
        let mut world = world();

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), observer);
        put(&mut world, (-1, 1, 0), blocks::state_by_name("redstone_block").unwrap());

        let mut lit = Vec::new();

        for tick in 1..=10 {
            run(&mut world, 1);

            if block(&world, (2, 1, 0)).is("powered", "true") {
                lit.push(tick);
            }
        }

        assert_eq!(lit.len(), OBSERVER_PULSE as usize, "наблюдатель не дал импульса после переезда: {lit:?}");
        assert!(block(&world, (2, 1, 0)).is("powered", "false"), "наблюдатель не погас");
    }

    /// Связка слизи едет целиком, а если в ней окажется больше двенадцати
    /// блоков — не едет вовсе.
    #[test]
    fn a_slime_group_moves_whole_or_not_at_all() {
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let slime = blocks::state_by_name("slime_block").unwrap();

        // Строим в воздухе: слизь на полу прилипла бы к самому полу
        // и потянула бы его за собой.
        let mut world = world();
        put(&mut world, (0, 5, 0), piston);
        put(&mut world, (1, 5, 0), slime);
        put(&mut world, (1, 6, 0), STONE); // прилип сверху
        put(&mut world, (0, 6, 0), lever);

        use_block(&mut world, (0, 6, 0));

        settle(&mut world);
        run(&mut world, 6);

        assert_eq!(world.get_block(2, 5, 0), slime, "слизь не поехала");
        assert_eq!(world.get_block(2, 6, 0), STONE, "прилипший блок остался на месте");

        // А тринадцатый блок связку останавливает.
        let mut heavy = super::tests::world();
        put(&mut heavy, (0, 1, 0), piston);

        for x in 1..=13 {
            put(&mut heavy, (x, 1, 0), STONE);
        }

        put(&mut heavy, (0, 2, 0), lever);
        use_block(&mut heavy, (0, 2, 0));
        run(&mut heavy, 6);

        assert!(
            block(&heavy, (0, 1, 0)).is("extended", "false"),
            "поршень поехал с тринадцатью блоками"
        );
    }


    /// Начиная ход, поршень сообщает о нём клиентам: по этому сообщению
    /// клиент рисует движение сам, пока сервер выжидает два такта.
    #[test]
    fn a_piston_tells_the_clients_it_moved() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 2, 0), lever);

        let reader = 1;
        world.watch_actions(reader);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 1);

        let (actions, _) = world.actions_since(0, reader);

        assert_eq!(actions.len(), 1, "о ходе никому не сказали");
        assert_eq!(actions[0].action, PISTON_EXTENDS);
        assert_eq!(actions[0].param, Dir::East.number(), "сторона не та");
        assert_eq!(
            actions[0].block,
            blocks::block_id("piston").expect("номер блока есть")
        );
    }


    /// Пока поршень едет, он не замечает обновлений; доехав, он смотрит
    /// на себя заново — иначе переключение, случившееся в пути, пропало бы.
    #[test]
    fn a_piston_looks_again_when_it_arrives() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (0, 2, 0), lever);

        // Включили — поршень тронулся.
        use_block(&mut world, (0, 2, 0));
        settle(&mut world);
        settle(&mut world);
        run(&mut world, 1);

        assert!(block(&world, (0, 1, 0)).is("extended", "true"), "поршень не тронулся");

        // И тут же выключили, пока он ещё в пути.
        use_block(&mut world, (0, 2, 0));
        settle(&mut world);
        settle(&mut world);

        // Доехав, он должен заметить, что питания больше нет, и задвинуться.
        run(&mut world, 8);

        assert!(
            block(&world, (0, 1, 0)).is("extended", "false"),
            "поршень остался выдвинутым, хотя питание сняли"
        );
        assert_eq!(world.get_block(1, 1, 0), AIR, "голова осталась");
    }


    /// О ходе поршня клиент узнаёт раньше, чем о новом состоянии самого
    /// поршня, а к концу хода получает и уехавший блок.
    #[test]
    fn the_move_is_announced_before_the_piston_changes() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        let reader = 1;
        world.watch_events(reader);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        // О самом поршне клиенту говорят на такт позже хода — как у оригинала.
        run(&mut world, 2);

        // Журнал читается один раз: прочитанное всеми выбрасывается.
        let (events, next) = world.events_since(0, reader);

        let action = events
            .iter()
            .position(|event| matches!(event, crate::world::WorldEvent::Action(_)))
            .expect("о ходе не сказали");
        let piston_change = events
            .iter()
            .position(|event| {
                matches!(event, crate::world::WorldEvent::Change(change)
                    if (change.x, change.y, change.z) == (0, 1, 0))
            })
            .expect("про поршень не сказали вовсе");

        assert!(action < piston_change, "про поршень сказали раньше, чем о ходе");

        // К концу хода блок уехал, и клиенту об этом тоже сказали.
        run(&mut world, 5);

        assert_eq!(world.get_block(2, 1, 0), STONE, "блок не уехал");

        let (events, _) = world.events_since(next, reader);

        assert!(
            events.iter().any(|event| {
                matches!(event, crate::world::WorldEvent::Change(change)
                    if (change.x, change.y, change.z) == (2, 1, 0))
            }),
            "про уехавший блок клиенту не сказали"
        );
    }


    /// О новых местах блоков клиенту говорят на такт позже, чем они туда
    /// встают: у клиента ход длится три такта, и блок, присланный на такт
    /// раньше, вставал бы рывком.
    #[test]
    fn the_moved_block_is_told_a_tick_after_it_moves() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        let reader = 1;
        world.watch_changes(reader);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);

        let mut moved_at = None;
        let mut told_at = None;

        for tick in 1..=8 {
            run(&mut world, 1);

            if moved_at.is_none() && world.get_block(2, 1, 0) == STONE {
                moved_at = Some(tick);
            }

            let (changes, _) = world.changes_since(0, reader);

            if told_at.is_none()
                && changes.iter().any(|change| (change.x, change.y, change.z) == (2, 1, 0))
            {
                told_at = Some(tick);
            }
        }

        assert_eq!(
            told_at.expect("клиенту так и не сказали"),
            moved_at.expect("блок так и не уехал") + PISTON_TELL_LATER,
            "сказали не с положенным опозданием"
        );
    }

    /// Действия и изменения лежат в одном журнале и уходят в том порядке,
    /// в каком случились: действие поршня раньше «поршень выдвинут», а место
    /// сломанного блока уходит служебным блоком уже после действия.
    #[test]
    fn actions_and_changes_keep_their_order() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let grass = blocks::state_by_name("short_grass").unwrap();

        put(&mut world, (0, 1, 0), piston);
        // Траве нужна земля: на каменном полу она не стоит.
        put(&mut world, (1, 0, 0), blocks::state_by_name("dirt").unwrap());
        put(&mut world, (1, 1, 0), grass); // сломается при ходе
        put(&mut world, (0, 2, 0), lever);

        let reader = 1;
        world.watch_events(reader);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        // О самом поршне клиенту говорят на такт позже хода — как у оригинала.
        run(&mut world, 2);

        let (events, _) = world.events_since(0, reader);

        let position = |wanted: &dyn Fn(&crate::world::WorldEvent) -> bool| {
            events.iter().position(wanted).expect("событие есть")
        };

        let grass_place = position(&|event| {
            matches!(event, crate::world::WorldEvent::Change(change)
                if (change.x, change.y, change.z) == (1, 1, 0))
        });
        let action = position(&|event| matches!(event, crate::world::WorldEvent::Action(_)));
        let extended = position(&|event| {
            matches!(event, crate::world::WorldEvent::Change(change)
                if (change.x, change.y, change.z) == (0, 1, 0) && change.state != piston)
        });

        assert!(action < grass_place, "о месте травы сказали раньше действия");
        assert!(action < extended, "действие ушло после «поршень выдвинут»");
    }


    /// Ход, начатый до перезапуска сервера, доигрывается: память о нём
    /// не переживает остановку, а запланированный такт переживает.
    #[test]
    fn a_move_started_before_a_restart_is_finished() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 1);

        assert!(block(&world, (0, 1, 0)).is("extended", "true"), "поршень не тронулся");

        // Перезапуск посреди хода: память о ходе восстанавливается из файла
        // мира, поэтому ход доигрывается целиком.
        type Saved = (Pos, bool, Dir, Vec<(Pos, i32)>);

        let saved: Vec<Saved> = world
            .redstone
            .moving_pistons()
            .map(|(pos, moving)| (*pos, moving.extending, moving.facing, moving.cargo.clone()))
            .collect();

        world.redstone.forget_moving();

        for (pos, extending, facing, cargo) in saved {
            world
                .redstone
                .restore_moving(pos, Moving::restored(pos, extending, facing, cargo));
        }

        run(&mut world, 5);

        assert_eq!(
            block(&world, (1, 1, 0)).kind,
            Kind::PistonHead,
            "поршень остался без головы навсегда"
        );
        assert_eq!(world.get_block(2, 1, 0), STONE, "блок не уехал");
    }



    #[test]
    fn scratch_pull() {
        let sticky = state("sticky_piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        let mut world = world();
        put(&mut world, (0, 1, 0), sticky);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        run(&mut world, 6);
        println!("выдвинулся: 1={} 2={}", world.get_block(1, 1, 0), world.get_block(2, 1, 0));

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);

        for tick in 0..5 {
            run(&mut world, 1);
            println!(
                "такт {tick}: поршень {:?}, 1={} ({:?}), 2={}",
                block(&world, (0, 1, 0)).value("extended"),
                world.get_block(1, 1, 0),
                block(&world, (1, 1, 0)).kind,
                world.get_block(2, 1, 0),
            );
        }
    }


    /// В начале хода едущие блоки снимаются со своих мест — как в игре.
    /// Схемы, читающие исходную клетку, видят её пустой сразу, а не через
    /// два такта.
    #[test]
    fn the_cargo_leaves_its_place_at_the_start_of_the_move() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 1);

        // Ход начался: камня нет ни там, ни там — он едет. Оба места
        // заняты служебным блоком: одно под голову, другое под камень.
        assert_ne!(world.get_block(1, 1, 0), STONE, "камень остался стоять на месте");
        assert_eq!(
            block(&world, (1, 1, 0)).kind,
            Kind::MovingPiston,
            "место под голову не занято"
        );
        assert_eq!(
            block(&world, (2, 1, 0)).kind,
            Kind::MovingPiston,
            "место назначения не занято"
        );

        // И этот блок ничего не проводит — как пустое место.
        assert!(!blocks::conductive(world.get_block(2, 1, 0)), "служебный блок проводит сигнал");

        // А в конце хода камень встаёт на новое место.
        run(&mut world, 4);

        assert_eq!(block(&world, (1, 1, 0)).kind, Kind::PistonHead);
        assert_eq!(world.get_block(2, 1, 0), STONE, "камень не доехал");
    }


    /// Мир на диске для проверок починки при чтении: свой каталог на каждую.
    fn disk_world(what: &str) -> (World, std::path::PathBuf) {
        let mut path = std::env::temp_dir();
        path.push(format!("mcsheriffanya-repair-{}-{}", std::process::id(), what));
        let _ = std::fs::remove_dir_all(&path);

        let mut world = World::open(
            &path,
            Some(1),
            crate::config::server_properties::WorldKind::Flat,
        )
        .expect("мир открылся");
        world.ensure(0, 0);

        (world, path)
    }

    /// Провод при чтении чанка не пересчитывается — как в игре: его состояние
    /// верно по построению, а пересчёт по соседям из ещё не прочитанных
    /// чанков только ломал бы схемы (MC-151049). Даже если источник
    /// исчез тайком, провод останется таким, каким записан, пока его не
    /// обновит что-нибудь рядом.
    #[test]
    fn a_saved_wire_is_trusted_when_the_chunk_is_read() {
        let (mut world, path) = disk_world("wire");
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), lever);
        put(&mut world, (1, 1, 0), wire);
        use_block(&mut world, (0, 1, 0));
        settle(&mut world);
        settle(&mut world);

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);

        // Источник пропадает без ведома редстоуна — как при обрыве записи.
        world.set_block_silently(0, 1, 0, AIR);
        world.save_if_needed();
        drop(world);

        let mut again = World::open(
            &path,
            Some(1),
            crate::config::server_properties::WorldKind::Flat,
        )
        .expect("мир открылся снова");
        again.ensure(0, 0);

        assert_eq!(
            wire_power_of(&block(&again, (1, 1, 0))),
            15,
            "провод пересчитали при чтении, хотя его никто не обновлял"
        );

        // А обновление рядом его чинит — как и в игре.
        put(&mut again, (1, 2, 0), STONE);
        assert_eq!(wire_power_of(&block(&again, (1, 1, 0))), 0, "провод не погас от обновления");

        let _ = std::fs::remove_dir_all(&path);
    }

    /// Служебный блок «движущийся поршень», оставшийся на диске без хода,
    /// при чтении убирается, а поршень без головы получает её обратно.
    #[test]
    fn leftovers_of_a_lost_move_are_cleaned_up_when_the_chunk_is_read() {
        let (mut world, path) = disk_world("leftovers");
        let piston = state("piston", &[("facing", "east"), ("extended", "true")]);
        let hidden = moving_block(Dir::East, false).expect("служебный блок есть");
        let head = state("piston_head", &[("facing", "east"), ("type", "normal"), ("short", "false")]);

        // Так выглядит чанк, записанный посреди хода, о котором никто
        // не помнит: поршень выдвинут, головы нет, впереди служебный блок.
        world.set_block_silently(0, 1, 0, piston);
        world.set_block_silently(1, 1, 0, hidden);
        world.set_block_silently(2, 1, 0, hidden);

        // И голова, оставшаяся без поршня.
        world.set_block_silently(5, 1, 0, head);

        world.mark_dirty_for_tests(0, 0);
        world.save_if_needed();
        drop(world);

        let mut again = World::open(
            &path,
            Some(1),
            crate::config::server_properties::WorldKind::Flat,
        )
        .expect("мир открылся снова");
        again.ensure(0, 0);

        assert_eq!(block(&again, (0, 1, 0)).kind, Kind::Piston);
        assert_eq!(block(&again, (1, 1, 0)).kind, Kind::PistonHead, "голова не вернулась");
        assert_eq!(again.get_block(2, 1, 0), AIR, "служебный блок остался");
        assert_eq!(again.get_block(5, 1, 0), AIR, "голова без поршня осталась");

        let _ = std::fs::remove_dir_all(&path);
    }

    /// А если ход записан в файле мира, служебные блоки остаются до его конца,
    /// и ход доигрывается: блок встаёт на своё место.
    #[test]
    fn a_saved_move_is_finished_after_the_chunk_is_read() {
        let (mut world, path) = disk_world("saved-move");
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 1);

        assert!(block(&world, (0, 1, 0)).is("extended", "true"), "поршень не тронулся");

        // Остановка посреди хода: всё записано.
        world.save_if_needed();
        drop(world);

        let mut again = World::open(
            &path,
            Some(1),
            crate::config::server_properties::WorldKind::Flat,
        )
        .expect("мир открылся снова");
        again.ensure(0, 0);
        run(&mut again, 5);

        assert_eq!(block(&again, (1, 1, 0)).kind, Kind::PistonHead, "голова не встала");
        assert_eq!(again.get_block(2, 1, 0), STONE, "блок не доехал");

        let _ = std::fs::remove_dir_all(&path);
    }


    /// Поздняя рассылка берёт состояние в момент отправки, а не заранее:
    /// если блок успел смениться, клиенту уходит свежее, а не устаревшее.
    #[test]
    fn a_late_message_carries_the_freshest_state() {
        let mut world = world();
        let reader = 1;
        world.watch_events(reader);

        world.set_block_quietly(3, 1, 3, STONE);
        world.tell_later(3, 1, 3, 2);
        world.set_block_quietly(3, 1, 3, AIR);

        run(&mut world, 3);

        let (changes, _) = world.changes_since(0, reader);
        let told: Vec<i32> = changes
            .iter()
            .filter(|change| (change.x, change.y, change.z) == (3, 1, 3))
            .map(|change| change.state)
            .collect();

        assert_eq!(told, vec![AIR], "ушло устаревшее состояние: {told:?}");
    }

    /// Место травы, раздавленной поршнем, уходит клиенту служебным блоком
    /// «движущийся поршень» на такт позже хода — как у оригинала.
    #[test]
    fn a_crushed_block_is_told_as_the_moving_block() {
        let mut world = world();
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let grass = blocks::state_by_name("short_grass").unwrap();

        put(&mut world, (0, 1, 0), piston);
        // Траве нужна земля: на каменном полу она не стоит.
        put(&mut world, (1, 0, 0), blocks::state_by_name("dirt").unwrap());
        put(&mut world, (1, 1, 0), grass);
        put(&mut world, (0, 2, 0), lever);

        let reader = 1;
        world.watch_events(reader);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 2);

        let (changes, _) = world.changes_since(0, reader);
        let (low, high) = blocks::states_of("moving_piston").expect("служебный блок есть");

        assert!(
            changes.iter().any(|change| (change.x, change.y, change.z) == (1, 1, 0)
                && (low..=high).contains(&change.state)),
            "о месте травы не сказали служебным блоком"
        );
        assert!(
            !changes.iter().any(|change| (change.x, change.y, change.z) == (1, 1, 0)
                && change.state == AIR),
            "место травы ушло пустотой"
        );
    }

    /// Поршень, которого дёргают часто, не должен оставлять после себя
    /// служебные блоки «движущийся поршень».
    ///
    /// Так ломалась машина из ряда липких поршней: после нескольких десятков
    /// ходов над поршнем оставался невидимый блок, а голова пропадала.
    #[test]
    fn a_piston_driven_fast_leaves_no_service_blocks() {
        let (low, high) = blocks::states_of("moving_piston").expect("служебный блок есть");
        let redstone_block = blocks::state_by_name("redstone_block").expect("блок редстоуна есть");

        for period in 1..=8usize {
            let mut world = world();
            let piston = state("sticky_piston", &[("facing", "up"), ("extended", "false")]);

            put(&mut world, (0, 1, 0), piston);
            put(&mut world, (0, 2, 0), STONE); // груз на голове

            for step in 0..40 {
                let power = if step % 2 == 0 { redstone_block } else { AIR };

                world.set_block(1, 1, 0, power);
                settle(&mut world);
                run(&mut world, period);
            }

            // Дать доиграть всему, что осталось.
            world.set_block(1, 1, 0, AIR);
            settle(&mut world);
            run(&mut world, 40);

            let mut stuck = Vec::new();

            for x in -2..=2 {
                for y in 1..=6 {
                    for z in -2..=2 {
                        let state = world.get_block(x, y, z);

                        if (low..=high).contains(&state) {
                            stuck.push((x, y, z));
                        }
                    }
                }
            }

            assert!(
                stuck.is_empty(),
                "период {}: служебные блоки остались в {:?}",
                period,
                stuck
            );

            // И сам поршень должен быть цел: выдвинут — с головой,
            // задвинут — без неё.
            let piston = block(&world, (0, 1, 0));
            let head = block(&world, (0, 2, 0)).kind == Kind::PistonHead;

            assert_eq!(
                piston.is("extended", "true"),
                head,
                "период {}: поршень и голова разошлись",
                period
            );
        }
    }

    /// Ход, записанный на диск без своего запланированного такта, всё равно
    /// доигрывается при чтении мира.
    ///
    /// Так ломалась машина игрока: в `level.dat` оставалась запись о ходе, а
    /// такта, который его доигрывает, не было. Поршень навсегда оставался
    /// без головы, а над ним стоял невидимый служебный блок.
    #[test]
    fn a_restored_move_finishes_even_without_its_tick() {
        let (mut world, path) = disk_world("forgotten-move");
        let piston = state("piston", &[("facing", "east"), ("extended", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), piston);
        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (0, 2, 0), lever);

        use_block(&mut world, (0, 2, 0));

        settle(&mut world);
        settle(&mut world);
        run(&mut world, 1); // ход начался, но ещё не доигран

        world.save_if_needed();
        drop(world);

        // Такт хода пропадает — так выглядел мир игрока.
        let level = path.join("ticks.dat");
        let text = std::fs::read_to_string(&level).expect("ticks.dat читается");
        let without: String = text
            .lines()
            .filter(|line| !line.starts_with("due = 0 1 0 block"))
            .map(|line| format!("{}\n", line))
            .collect();

        assert!(text.contains("moving = 0 1 0"), "ход не записан: {}", text);

        std::fs::write(&level, without).expect("ticks.dat записывается");

        let mut again = World::open(
            &path,
            Some(1),
            crate::config::server_properties::WorldKind::Flat,
        )
        .expect("мир открылся снова");
        again.ensure(0, 0);
        run(&mut again, 5);

        let (low, high) = blocks::states_of("moving_piston").expect("служебный блок есть");

        assert_eq!(
            block(&again, (1, 1, 0)).kind,
            Kind::PistonHead,
            "поршень остался без головы"
        );
        assert_eq!(again.get_block(2, 1, 0), STONE, "блок не доехал");
        assert!(
            !(low..=high).contains(&again.get_block(2, 1, 0)),
            "на месте блока остался служебный"
        );

        let _ = std::fs::remove_dir_all(&path);
    }


    /// Звуки, которые мир накопил для клиентов с начала записи.
    fn sounds(world: &mut World, reader: crate::journal::Reader) -> Vec<(&'static str, f32)> {
        world
            .events_since(0, reader)
            .0
            .into_iter()
            .filter_map(|event| match event {
                crate::world::WorldEvent::Sound(sound) => Some((sound.name, sound.pitch)),
                _ => None,
            })
            .collect()
    }

    /// Мишень притягивает пыль, держит силу попадания 8 тактов и проводит
    /// сигнал, как сплошной блок.
    #[test]
    fn a_target_pulls_dust_and_holds_its_hit() {
        let mut world = world();
        let target = blocks::state_by_name("target").unwrap();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        put(&mut world, (0, 1, 0), target);
        put(&mut world, (1, 1, 0), wire);

        assert!(!block(&world, (1, 1, 0)).is("west", "none"), "пыль не тянется к мишени");

        components::target_hit(&mut world, (0, 1, 0), 7, false);

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 7);

        run(&mut world, 7);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 7, "сигнал кончился раньше 8 тактов");

        run(&mut world, 2);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0, "сигнал не кончился");

        // Рычаг на мишени сильно питает её — лампа по другую сторону горит.
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();
        let lever = state("lever", &[("face", "wall"), ("facing", "north"), ("powered", "true")]);

        put(&mut world, (0, 1, 1), lamp);
        put(&mut world, (0, 1, -1), lever);

        assert!(block(&world, (0, 1, 1)).is("lit", "true"), "мишень не проводит");
    }

    /// Датчик света под открытым небом в полдень выдаёт 15, но только
    /// раз в 20 тактов; щелчок делает его инвертированным, и выход
    /// сразу падает до нуля. Закрытый со всех сторон инвертированный — 15.
    #[test]
    fn a_daylight_detector_follows_the_sun() {
        let mut world = world();
        let detector = blocks::state_by_name("daylight_detector").unwrap();
        let wire = blocks::state_by_name("redstone_wire").unwrap();

        world.set_time_of_day(6_000);
        put(&mut world, (0, 1, 0), detector);
        put(&mut world, (1, 1, 0), wire);

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0, "датчик сработал сразу при установке");

        // Пересчёт — в такте с возрастом 19 по модулю 20 (daylight-dusk).
        while !(world.tick() + 2).is_multiple_of(20) {
            run(&mut world, 1);
        }

        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0, "датчик сработал не в своём такте");

        run(&mut world, 1);
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 15);
        assert!(!block(&world, (1, 1, 0)).is("west", "none"), "пыль не тянется к датчику");

        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("inverted", "true"));
        assert_eq!(wire_power_of(&block(&world, (1, 1, 0))), 0);

        // Закрыли со всех сторон — небесного света нет, инвертированный
        // даёт 15.
        world.set_block(1, 1, 0, AIR);
        settle(&mut world);

        for place in [(0, 2, 0), (1, 1, 0), (-1, 1, 0), (0, 1, 1), (0, 1, -1)] {
            put(&mut world, place, STONE);
        }

        run(&mut world, 20);
        assert_eq!(components::power_value(&block(&world, (0, 1, 0))), 15);
    }

    /// Цепь растяжки: два крючка и три растяжки между ними. Собирается при
    /// установке, включается тем, кто встал на растяжку, держится, пока он
    /// там, и отпускает в такте, кратном десяти.
    fn tripwire_line(world: &mut World) {
        let hook_east = state("tripwire_hook", &[("facing", "east"), ("attached", "false"), ("powered", "false")]);
        let hook_west = state("tripwire_hook", &[("facing", "west"), ("attached", "false"), ("powered", "false")]);
        let wire = blocks::state_by_name("tripwire").unwrap();
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();

        put(world, (-1, 1, 0), STONE);
        put(world, (5, 1, 0), STONE);
        put(world, (-2, 1, 0), lamp);
        put(world, (0, 1, 0), hook_east);
        put(world, (4, 1, 0), hook_west);

        for x in 1..=3 {
            put(world, (x, 1, 0), wire);
        }
    }

    #[test]
    fn a_tripwire_line_attaches_and_answers_to_a_player() {
        let mut world = world();
        let reader = 7;

        world.watch_events(reader);
        tripwire_line(&mut world);

        for x in [0, 4] {
            assert!(block(&world, (x, 1, 0)).is("attached", "true"), "крючок {} не в цепи", x);
        }

        for x in 1..=3 {
            let wire = block(&world, (x, 1, 0));
            assert!(wire.is("attached", "true"), "растяжка {} не в цепи", x);
            assert!(wire.is("east", "true") && wire.is("west", "true"), "растяжка {} не тянется", x);
        }

        assert!(sounds(&mut world, reader).iter().any(|(name, _)| name.ends_with("tripwire.attach")));

        // Встал на середину.
        note_occupants(&mut world, vec![Occupant::player(2.5, 1.0, 0.5, false)]);
        step_entities(&mut world);
        settle(&mut world);

        assert!(block(&world, (2, 1, 0)).is("powered", "true"), "растяжка не задета");
        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "крючок не включился");
        assert!(block(&world, (4, 1, 0)).is("powered", "true"), "второй крючок не включился");
        assert!(block(&world, (-2, 1, 0)).is("lit", "true"), "блок под крючком не запитан сильно");
        assert!(sounds(&mut world, reader).iter().any(|(name, _)| name.ends_with("tripwire.click_on")));

        // Стоит 15 тактов — держится.
        run(&mut world, 15);
        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "отпустило, хотя на растяжке стоят");

        // Ушёл: растяжка выключится на своём такте — через 20 от начала.
        note_occupants(&mut world, Vec::new());
        run(&mut world, 4);
        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "отпустило не в кратном десяти такте");

        run(&mut world, 1);
        assert!(!block(&world, (2, 1, 0)).is("powered", "true"), "растяжка не отпустила");
        assert!(!block(&world, (0, 1, 0)).is("powered", "true"), "крючок не отпустил");
        assert!(block(&world, (0, 1, 0)).is("attached", "true"), "крючок выпал из цепи");
    }

    /// Порванная рукой растяжка даёт импульс в 10 тактов, срезанная
    /// ножницами — нет; цепь в обоих случаях распадается.
    #[test]
    fn a_broken_tripwire_pulses_and_a_sheared_one_does_not() {
        let mut world = world();

        tripwire_line(&mut world);
        world.set_block(2, 1, 0, AIR);
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "порванная растяжка не дала импульса");

        run(&mut world, 9);
        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "импульс короче 10 тактов");

        run(&mut world, 1);
        assert!(!block(&world, (0, 1, 0)).is("powered", "true"), "импульс длиннее 10 тактов");
        assert!(!block(&world, (0, 1, 0)).is("attached", "true"), "цепь не распалась");
        assert!(!block(&world, (4, 1, 0)).is("attached", "true"), "второй крючок остался в цепи");

        // Ножницы.
        let mut sheared = self::world();

        tripwire_line(&mut sheared);
        disarm_tripwire(&mut sheared, (2, 1, 0));
        assert!(block(&sheared, (0, 1, 0)).is("attached", "true"), "обезвреживание само порвало цепь");

        sheared.set_block(2, 1, 0, AIR);
        settle(&mut sheared);

        assert!(!block(&sheared, (0, 1, 0)).is("powered", "true"), "срезанная растяжка дала импульс");
        assert!(!block(&sheared, (0, 1, 0)).is("attached", "true"), "крючок не заметил, что цепь порвана");
    }

    /// Больше сорока растяжек в цепи не бывает.
    #[test]
    fn a_tripwire_line_holds_at_most_forty_wires() {
        for (wires, attached) in [(40, true), (41, false)] {
            let mut world = world();
            let hook_east = state("tripwire_hook", &[("facing", "east"), ("attached", "false"), ("powered", "false")]);
            let hook_west = state("tripwire_hook", &[("facing", "west"), ("attached", "false"), ("powered", "false")]);
            let wire = blocks::state_by_name("tripwire").unwrap();

            put(&mut world, (-1, 1, 0), STONE);
            put(&mut world, (wires + 2, 1, 0), STONE);
            put(&mut world, (0, 1, 0), hook_east);
            put(&mut world, (wires + 1, 1, 0), hook_west);

            for x in 1..=wires {
                put(&mut world, (x, 1, 0), wire);
            }

            assert_eq!(block(&world, (0, 1, 0)).is("attached", "true"), attached, "растяжек {}", wires);
        }
    }

    /// Руда загорается от щелчка (и блок в руке всё равно ставится) и от
    /// того, кто на ней стоит, — кроме крадущегося; гаснет на случайном
    /// такте, если рядом есть игрок.
    #[test]
    fn redstone_ore_lights_up_and_goes_out() {
        let mut world = world();
        let ore = blocks::state_by_name("redstone_ore").unwrap();

        put(&mut world, (0, 1, 0), ore);
        put(&mut world, (3, 1, 0), ore);

        assert!(!use_block(&mut world, (0, 1, 0)), "щелчок по руде съел установку блока");
        settle(&mut world);
        assert!(block(&world, (0, 1, 0)).is("lit", "true"), "руда не загорелась от щелчка");

        note_occupants(&mut world, vec![Occupant::player(3.5, 2.0, 0.5, true)]);
        step_entities(&mut world);
        assert!(!block(&world, (3, 1, 0)).is("lit", "true"), "крадущийся зажёг руду");

        note_occupants(&mut world, vec![Occupant::player(3.5, 2.0, 0.5, false)]);
        step_entities(&mut world);
        assert!(block(&world, (3, 1, 0)).is("lit", "true"), "стоящий не зажёг руду");

        // Руда питания не даёт.
        assert_eq!(power_from(&world, (1, 1, 0), Dir::West, true), 0);

        // Игроков рядом нет — случайных тактов нет, руда горит.
        for _ in 0..20_000 {
            random_ticks(&mut world, &[(100, 100)], 10);
        }
        assert!(block(&world, (0, 1, 0)).is("lit", "true"), "руда погасла без игроков рядом");

        // Игрок рядом: в среднем 1365 тактов — за 20 000 погаснет.
        for _ in 0..20_000 {
            random_ticks(&mut world, &[(0, 0)], 10);
        }
        assert!(!block(&world, (0, 1, 0)).is("lit", "true"), "руда не гаснет");
        assert!(!block(&world, (3, 1, 0)).is("lit", "true"), "руда не гаснет");
        assert!(world.redstone.lit_ores.is_empty(), "погасшая руда осталась в списке");
    }

    /// Горящая руда гаснет в среднем через 4096 / 3 = 1365 тактов (вики,
    /// Tick: «randomTickSpeed» 3 блока на секцию 16×16×16 за такт; Redstone
    /// Ore: «68.27 seconds») — и только на случайном такте.
    #[test]
    fn lit_redstone_ore_goes_out_at_the_random_tick_rate() {
        let mut world = world();
        let ore = blocks::state_by_name("redstone_ore").unwrap();
        let (mut total, mut first) = (0u64, u64::MAX);
        const TRIES: u64 = 400;

        put(&mut world, (0, 1, 0), ore);

        for _ in 0..TRIES {
            use_block(&mut world, (0, 1, 0));
            settle(&mut world);
            assert!(block(&world, (0, 1, 0)).is("lit", "true"));

            let mut ticks = 0;

            while block(&world, (0, 1, 0)).is("lit", "true") {
                ticks += 1;
                random_ticks(&mut world, &[(0, 0)], 10);
            }

            total += ticks;
            first = first.min(ticks);
        }

        let mean = total as f64 / TRIES as f64;

        assert!((mean - 1365.3).abs() < 1365.3 * 0.15, "в среднем {mean} тактов");
        assert!(first < 100, "раньше {first} тактов не гасла ни разу");
    }

    /// Колокол звонит по фронту сигнала — один раз, пока сигнал держится;
    /// качается в сторону своего `facing`.
    /// Датчик скалка в четырёх блоках от рычага: щёлкает через 3 такта,
    /// сила 8 в провод, сравнитель читает частоту 10 (включение); активен
    /// 30 тактов, откат 10 (сценарий sculk-lever).
    #[test]
    fn a_sculk_sensor_hears_a_lever() {
        let mut world = world();
        let sensor = state("sculk_sensor", &[("power", "0"), ("sculk_sensor_phase", "inactive"), ("waterlogged", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let wire = state("redstone_wire", &[("east", "none"), ("north", "side"), ("power", "0"), ("south", "side"), ("west", "none")]);
        let comparator = state("comparator", &[("facing", "west"), ("mode", "compare"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), sensor);
        put(&mut world, (0, 1, -1), wire);
        put(&mut world, (1, 1, 0), comparator);
        put(&mut world, (0, 1, 4), lever);
        run(&mut world, 5);

        use_block(&mut world, (0, 1, 4));
        settle(&mut world);
        // Такт щелчка и ещё два — вибрация в пути; долетает в третьем.
        run(&mut world, 3);
        assert!(block(&world, (0, 1, 0)).is("sculk_sensor_phase", "inactive"), "вибрация долетела раньше");

        run(&mut world, 1);
        let active = block(&world, (0, 1, 0));
        assert!(active.is("sculk_sensor_phase", "active"));
        assert!(active.is("power", "8"));
        assert_eq!(wire_power_of(&block(&world, (0, 1, -1))), 8);
        assert_eq!(comparator_reading(&world, (0, 1, 0)), Some(10));

        // Пока датчик активен, второй щелчок он не слышит.
        use_block(&mut world, (0, 1, 4));
        settle(&mut world);
        run(&mut world, 30);
        assert!(block(&world, (0, 1, 0)).is("sculk_sensor_phase", "cooldown"));
        assert_eq!(comparator_reading(&world, (0, 1, 0)), Some(0));
        run(&mut world, 10);
        assert!(block(&world, (0, 1, 0)).is("sculk_sensor_phase", "inactive"));

        use_block(&mut world, (0, 1, 4));
        settle(&mut world);
        run(&mut world, 4);
        assert_eq!(comparator_reading(&world, (0, 1, 0)), Some(10));
    }

    /// Головы, датчики скалка, кафедры и проигрыватели — блок-сущности
    /// в пакете чанка.
    #[test]
    fn heads_and_sensors_have_block_entities() {
        let head = state("zombie_head", &[("powered", "false"), ("rotation", "0")]);
        let sensor = state("sculk_sensor", &[("power", "0"), ("sculk_sensor_phase", "inactive"), ("waterlogged", "false")]);

        assert_eq!(block_entity_type(head), Some(16));
        assert_eq!(block_entity_type(sensor), Some(35));
        assert_eq!(block_entity_type(blocks::state_by_name("stone").unwrap_or(1)), None);
        assert_eq!(block_entity_type(blocks::state_by_name("cherry_hanging_sign").unwrap()), Some(8));
        assert_eq!(block_entity_type(blocks::state_by_name("oak_wall_hanging_sign").unwrap()), Some(8));
        assert_eq!(block_entity_type(blocks::state_by_name("oak_wall_sign").unwrap()), Some(7));
        assert!(is_sign(blocks::state_by_name("birch_sign").unwrap()));
    }

    #[test]
    fn a_bell_rings_on_the_rising_edge() {
        let mut world = world();
        let bell = state("bell", &[("attachment", "floor"), ("facing", "north"), ("powered", "false")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        let reader = 3;

        put(&mut world, (0, 1, 0), bell);
        put(&mut world, (1, 1, 0), lever);
        world.watch_actions(reader);
        world.watch_events(reader + 1);

        use_block(&mut world, (1, 1, 0));
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("powered", "true"));

        // Звон колокола — раньше щелчка рычага, громкость 2, высота 1
        // (сценарий bell-redstone).
        assert_eq!(
            sounds(&mut world, reader + 1),
            vec![("minecraft:block.bell.use", 1.0), ("minecraft:block.lever.click", 0.6)]
        );

        // Качание — блочным событием, в своей фазе такта.
        assert!(world.actions_since(0, reader).0.is_empty(), "качание ушло раньше фазы событий");
        run(&mut world, 1);

        let (actions, _) = world.actions_since(0, reader);
        assert_eq!(actions.len(), 1, "колокол не качнулся или качнулся дважды");
        assert_eq!((actions[0].action, actions[0].param), (1, Dir::North.number()));
        sounds(&mut world, reader + 1);

        // Выключили — только `powered`, без звона.
        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        run(&mut world, 1);

        assert!(block(&world, (0, 1, 0)).is("powered", "false"));
        assert!(world.actions_since(0, reader).0.is_empty(), "колокол звенит на спаде");
        sounds(&mut world, reader + 1);

        // Щелчок сбоку звонит в сторону грани, сверху — нет.
        assert!(use_block_on(&mut world, (0, 1, 0), Dir::East.number() as i32));
        assert!(!use_block_on(&mut world, (0, 1, 0), Dir::Up.number() as i32));
        run(&mut world, 1);

        let (actions, _) = world.actions_since(0, reader);
        assert_eq!(actions.iter().map(|action| action.param).collect::<Vec<_>>(), vec![Dir::East.number()]);
    }

    /// Голова на музыкальном блоке задаёт инструмент поверх блока снизу и
    /// звучит голосом моба, без высоты ноты. Голова сверху блок не глушит.
    #[test]
    fn a_head_on_a_note_block_speaks_for_it() {
        let mut world = world();
        let note = blocks::state_by_name("note_block").unwrap();
        let head = blocks::state_by_name("zombie_head").unwrap();
        let reader = 5;

        put(&mut world, (0, 1, 0), note);
        put(&mut world, (0, 2, 0), head);

        assert!(block(&world, (0, 1, 0)).is("instrument", "zombie"), "голова не сменила инструмент");

        world.watch_events(reader);
        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);
        after_use(&mut world, (0, 1, 0));

        // Нота — блочным событием, в своей фазе такта.
        assert!(sounds(&mut world, reader).is_empty(), "нота прозвучала раньше фазы событий");
        run(&mut world, 1);

        assert_eq!(
            sounds(&mut world, reader),
            vec![("minecraft:block.note_block.imitate.zombie", 1.0)],
            "голова звучит не так"
        );

        // Голова над запитанным музыкальным блоком получает `powered`
        // (сценарий note-heads: провод смотрит в блок, блок слабо запитан).
        let wire = blocks::state_by_name("redstone_wire").unwrap();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "true")]);

        put(&mut world, (1, 1, 0), wire);
        put(&mut world, (2, 1, 0), lever);

        assert!(block(&world, (0, 2, 0)).is("powered", "true"), "голова не видит сигнала");
        assert!(is_head(block(&world, (0, 2, 0)).state));
        assert!(!is_head(blocks::state_by_name("piston_head").unwrap()));

        // Сняли голову — снова инструмент по блоку снизу (камень).
        world.set_block(0, 2, 0, AIR);
        settle(&mut world);
        assert!(block(&world, (0, 1, 0)).is("instrument", "basedrum"));
    }

    /// Настенная голова над музыкальным блоком инструмент не задаёт и блок
    /// глушит; голова игрока молчит без звука и без действия блока
    /// (сценарий note-heads).
    #[test]
    fn wall_heads_and_player_heads_stay_silent() {
        let mut world = world();
        let note = blocks::state_by_name("note_block").unwrap();
        let reader = 5;

        put(&mut world, (0, 1, 0), note);
        put(&mut world, (0, 2, 0), state("zombie_wall_head", &[("facing", "north"), ("powered", "false")]));
        assert!(block(&world, (0, 1, 0)).is("instrument", "basedrum"), "настенная голова задала инструмент");

        world.watch_events(reader);
        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);
        after_use(&mut world, (0, 1, 0));
        run(&mut world, 1);
        assert!(sounds(&mut world, reader).is_empty(), "заглушённый блок прозвучал");

        world.set_block(0, 2, 0, state("player_head", &[("powered", "false"), ("rotation", "0")]));
        settle(&mut world);
        assert!(block(&world, (0, 1, 0)).is("instrument", "custom_head"));

        let (_, from) = world.actions_since(0, reader);
        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);
        after_use(&mut world, (0, 1, 0));
        run(&mut world, 1);
        assert!(world.actions_since(from, reader).0.is_empty(), "голова игрока дала действие блока");
    }

    /// Громоотвод после удара 8 тактов сильно питает блок, на котором стоит.
    #[test]
    fn a_struck_lightning_rod_powers_its_block_for_eight_ticks() {
        let mut world = world();
        let rod = state("lightning_rod", &[("facing", "up"), ("powered", "false"), ("waterlogged", "false")]);
        let lamp = blocks::state_by_name("redstone_lamp").unwrap();

        put(&mut world, (0, 1, 0), rod);
        put(&mut world, (0, -1, 0), lamp);

        components::strike(&mut world, (0, 1, 0));
        assert!(block(&world, (0, -1, 0)).is("lit", "true"), "блок под громоотводом не запитан сильно");

        run(&mut world, 7);
        assert!(block(&world, (0, 1, 0)).is("powered", "true"));

        run(&mut world, 1);
        assert!(block(&world, (0, 1, 0)).is("powered", "false"), "сигнал держится дольше 8 тактов");
    }

    /// Каменную плиту предмет не нажимает, деревянную — нажимает.
    #[test]
    fn a_stone_plate_ignores_items() {
        let mut world = world();
        let stone_plate = blocks::state_by_name("stone_pressure_plate").unwrap();
        let oak_plate = blocks::state_by_name("oak_pressure_plate").unwrap();

        put(&mut world, (0, 1, 0), stone_plate);
        put(&mut world, (2, 1, 0), oak_plate);
        put(&mut world, (4, 1, 0), blocks::state_by_name("polished_blackstone_pressure_plate").unwrap());
        world.watch_events(11);

        step_plates(
            &mut world,
            &[
                Occupant::item(0.5, 1.0, 0.5, true),
                Occupant::item(2.5, 1.0, 0.5, true),
                Occupant::item(4.5, 1.0, 0.5, true),
            ],
        );
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("powered", "false"), "предмет нажал каменную плиту");
        assert!(block(&world, (4, 1, 0)).is("powered", "false"), "предмет нажал плиту из чернокамня");
        assert!(block(&world, (2, 1, 0)).is("powered", "true"), "предмет не нажал деревянную плиту");
        assert_eq!(
            sounds(&mut world, 11),
            vec![("minecraft:block.wooden_pressure_plate.click_on", 1.0)],
            "деревянная плита щёлкнула не так"
        );

        step_plates(&mut world, &[Occupant::player(0.5, 1.0, 0.5, false)]);
        settle(&mut world);
        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "игрок не нажал каменную плиту");
    }

    // -----------------------------------------------------------------------
    // Рельсы
    // -----------------------------------------------------------------------

    fn rail(name: &str) -> i32 {
        blocks::state_by_name(name).unwrap()
    }

    fn shape(world: &World, pos: Pos) -> &'static str {
        block(world, pos).value("shape").unwrap_or("нет рельсы")
    }

    /// Вторая рельса рядом с первой: обе ложатся вдоль, к ним третья сбоку
    /// — крайняя поворачивает.
    #[test]
    fn rails_line_up_and_turn() {
        let mut world = world();

        put(&mut world, (0, 1, 0), rail("rail"));
        assert_eq!(shape(&world, (0, 1, 0)), "north_south");

        put(&mut world, (1, 1, 0), rail("rail"));
        assert_eq!(shape(&world, (1, 1, 0)), "east_west");
        assert_eq!(shape(&world, (0, 1, 0)), "east_west", "первая не повернулась к соседке");

        put(&mut world, (0, 1, 1), rail("rail"));
        assert_eq!(shape(&world, (0, 1, 1)), "north_south");
        assert_eq!(shape(&world, (0, 1, 0)), "south_east", "крайняя не стала поворотом");

        // Середина линии занята: новая рельса рядом с ней не соединяется.
        put(&mut world, (5, 1, 0), rail("rail"));
        put(&mut world, (6, 1, 0), rail("rail"));
        put(&mut world, (7, 1, 0), rail("rail"));
        put(&mut world, (6, 1, 1), rail("rail"));
        assert_eq!(shape(&world, (6, 1, 0)), "east_west", "середина линии перестроилась");
    }

    /// Развилка: поворот к югу и востоку, под сигналом — к северу и западу.
    #[test]
    fn a_rail_junction_flips_with_a_signal() {
        let mut world = world();

        put(&mut world, (-1, 1, 0), rail("rail"));
        put(&mut world, (1, 1, 0), rail("rail"));
        put(&mut world, (0, 1, 1), rail("rail"));
        put(&mut world, (0, 1, 0), rail("rail"));
        assert_eq!(shape(&world, (0, 1, 0)), "south_east");

        // Блок редстоуна к северу: юга предпочтение больше не держит, запад
        // берёт верх над востоком.
        put(&mut world, (0, 1, -1), rail("redstone_block"));
        assert_eq!(shape(&world, (0, 1, 0)), "south_west");

        // Снятый блок редстоуна развилку обратно не переключает: источник
        // обновления — воздух (чёрный ящик, rail-shapes).
        put(&mut world, (0, 1, -1), AIR);
        assert_eq!(shape(&world, (0, 1, 0)), "south_west");

        // Перекрёсток на блоке редстоуна — к северу и западу.
        let mut world = self::world();

        put(&mut world, (0, 0, 0), rail("redstone_block"));
        for pos in [(-1, 1, 0), (1, 1, 0), (0, 1, 1), (0, 1, -1)] {
            put(&mut world, pos, rail("rail"));
        }
        put(&mut world, (0, 1, 0), rail("rail"));
        assert_eq!(shape(&world, (0, 1, 0)), "north_west");
    }

    /// Рельса к соседке на блок выше — наклон; прямая рельса с соседками по
    /// обеим осям — восток–запад.
    #[test]
    fn rails_climb_and_straight_rails_do_not_turn() {
        let mut world = world();

        put(&mut world, (1, 1, 0), STONE);
        put(&mut world, (1, 2, 0), rail("rail"));
        put(&mut world, (0, 1, 0), rail("rail"));
        assert_eq!(shape(&world, (0, 1, 0)), "ascending_east");
        assert_eq!(shape(&world, (1, 2, 0)), "east_west");

        // Убрали блок под верхним концом наклона: вместе с ним отвалилась
        // и верхняя рельса, и наклон.
        put(&mut world, (1, 1, 0), AIR);
        assert_eq!(block(&world, (1, 2, 0)).state, AIR);
        assert_eq!(block(&world, (0, 1, 0)).state, AIR);

        let mut world = self::world();

        put(&mut world, (0, 1, 1), rail("powered_rail"));
        put(&mut world, (0, 1, 0), rail("powered_rail"));
        assert_eq!(shape(&world, (0, 1, 0)), "north_south");

        put(&mut world, (1, 1, 0), rail("powered_rail"));
        assert_eq!(shape(&world, (0, 1, 0)), "east_west", "прямая с соседкой сбоку не легла восток–запад");
    }

    /// Рельса без опоры отваливается.
    #[test]
    fn a_rail_falls_without_support() {
        let mut world = world();

        put(&mut world, (0, 1, 0), rail("rail"));
        put(&mut world, (0, 0, 0), AIR);
        assert_eq!(block(&world, (0, 1, 0)).state, AIR);

        assert!(!holds_rail(blocks::state_by_name("oak_leaves").unwrap()));
        assert!(holds_rail(blocks::state_by_name("hopper").unwrap()));
    }

    /// Рычаг у первой из двенадцати рельс с питанием: включаются девять —
    /// она и восемь за ней. Активирующие питание рельсам с питанием не
    /// передают.
    #[test]
    fn powered_rails_pass_power_eight_further() {
        let mut world = world();

        for x in 1..=12 {
            put(&mut world, (x, 1, 0), rail("powered_rail"));
        }

        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);
        put(&mut world, (0, 1, 0), lever);
        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);

        for x in 1..=12 {
            assert_eq!(block(&world, (x, 1, 0)).is("powered", "true"), x <= 9, "рельса {x}");
        }

        assert!(use_block(&mut world, (0, 1, 0)));
        settle(&mut world);

        for x in 1..=12 {
            assert!(block(&world, (x, 1, 0)).is("powered", "false"), "рельса {x} не погасла");
        }

        // Активирующая посреди линии рвёт цепочку.
        let mut world = self::world();

        for x in 1..=4 {
            let name = if x == 3 { "activator_rail" } else { "powered_rail" };
            put(&mut world, (x, 1, 0), rail(name));
        }

        put(&mut world, (0, 1, 0), rail("redstone_block"));
        assert!(block(&world, (2, 1, 0)).is("powered", "true"));
        assert!(block(&world, (3, 1, 0)).is("powered", "false"));
        assert!(block(&world, (4, 1, 0)).is("powered", "false"));
    }

    /// Питание идёт и по наклону.
    #[test]
    fn powered_rails_pass_power_up_a_slope() {
        let mut world = world();

        put(&mut world, (2, 1, 0), STONE);
        put(&mut world, (3, 1, 0), STONE);
        put(&mut world, (1, 1, 0), rail("activator_rail"));
        put(&mut world, (2, 2, 0), rail("activator_rail"));
        put(&mut world, (3, 2, 0), rail("activator_rail"));
        assert_eq!(shape(&world, (1, 1, 0)), "ascending_east");

        put(&mut world, (0, 1, 0), rail("redstone_block"));
        assert!(block(&world, (3, 2, 0)).is("powered", "true"));
    }

    /// Детекторная рельса: вагонетка на ней — источник на 15, блок под ней
    /// запитан сильно, сравнитель читает вагонетку; вагонетка ушла — через
    /// 20 тактов рельса гаснет.
    #[test]
    fn a_detector_rail_feels_a_cart() {
        let mut world = world();
        let wire = rail("redstone_wire");

        put(&mut world, (0, 1, 0), rail("detector_rail"));
        put(&mut world, (1, 1, 0), state("comparator", &[("facing", "west"), ("mode", "compare"), ("powered", "false")]));
        put(&mut world, (2, 1, 0), wire);
        put(&mut world, (-1, 1, 0), wire);

        note_carts(&mut world, vec![Cart { x: 0.5, y: 1.0, z: 0.5, reading: Some(7) }]);
        step_entities(&mut world);
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("powered", "true"));
        assert_eq!(wire_power_of(&block(&world, (-1, 1, 0))), 15);
        assert_eq!(strong_power(&world, (0, 0, 0)), 15, "блок под рельсой не запитан сильно");

        run(&mut world, 3);
        assert_eq!(wire_power_of(&block(&world, (2, 1, 0))), 7, "сравнитель не прочёл вагонетку");

        note_carts(&mut world, Vec::new());
        run(&mut world, 16);
        assert!(block(&world, (0, 1, 0)).is("powered", "true"), "погасла раньше срока");

        run(&mut world, 4);
        assert!(block(&world, (0, 1, 0)).is("powered", "false"), "не погасла");
        assert_eq!(wire_power_of(&block(&world, (-1, 1, 0))), 0);
    }

    /// Крючок, поставленный командой, цепь не собирает — только поставленный
    /// игроком. Снятый крючок выключает второй, а растяжки остаются
    /// `attached`, и звука нет (сценарий tripwire-hook-break).
    #[test]
    fn a_hook_joins_the_line_only_when_a_player_places_it() {
        let mut world = world();
        let reader = 9;

        tripwire_line(&mut world);
        world.watch_events(reader);

        world.set_block(0, 1, 0, AIR);
        settle(&mut world);

        assert!(!block(&world, (4, 1, 0)).is("attached", "true"), "второй крючок остался в цепи");
        assert!(block(&world, (1, 1, 0)).is("attached", "true"), "растяжка потеряла attached");
        assert!(block(&world, (1, 1, 0)).is("west", "false"), "растяжка тянется к пустоте");
        assert!(sounds(&mut world, reader).is_empty(), "снятый командой крючок звучит");

        // Вернули командой — вне цепи.
        let hook_east = state("tripwire_hook", &[("facing", "east"), ("attached", "false"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), hook_east);
        assert!(!block(&world, (0, 1, 0)).is("attached", "true"), "крючок от команды собрал цепь");
        assert!(block(&world, (1, 1, 0)).is("west", "true"));

        // Поставил игрок — цепь собрана.
        placed_by_player(&mut world, (0, 1, 0));
        assert!(block(&world, (0, 1, 0)).is("attached", "true"));
        assert!(block(&world, (4, 1, 0)).is("attached", "true"));
    }

    /// Датчики под крышей 3×3 (небесный свет 13): в полночь обычный 0,
    /// инвертированный 13; в 13000 — 2 и 9. Так у оригинала (сценарий
    /// daylight-shade).
    #[test]
    fn daylight_detectors_in_the_shade_match_the_original() {
        let mut world = world();
        let normal = blocks::state_by_name("daylight_detector").unwrap();
        let inverted = state("daylight_detector", &[("inverted", "true"), ("power", "0")]);

        for x in -1..=1 {
            for z in [-1, 0, 1, 3, 4, 5] {
                put(&mut world, (x, 2, z), STONE);
            }
        }

        put(&mut world, (0, 1, 0), normal);
        put(&mut world, (0, 1, 4), inverted);

        let power = |world: &World, z| components::power_value(&block(world, (0, 1, z)));

        for (time, expected) in [(6_000, (13, 2)), (18_000, (0, 13)), (13_000, (2, 9))] {
            world.set_time_of_day(time);
            run(&mut world, 20);

            assert_eq!((power(&world, 0), power(&world, 4)), expected, "время {}", time);
        }
    }

    /// Настоящая вагонетка катится по линии рельс через детекторную: та
    /// включается в такт, когда ящик вагонетки её коснулся, и гаснет на
    /// проверке через 20 тактов после включения, когда вагонетка уже
    /// уехала (как у оригинала, rail-detector).
    #[test]
    fn a_rolling_minecart_presses_a_detector_rail() {
        use crate::minecart::{Kind, Minecart};

        let mut world = world();
        let straight = state("rail", &[("shape", "east_west")]);

        for x in 0..=20 {
            put(&mut world, (x, 1, 0), straight);
        }

        put(&mut world, (6, 1, 0), state("detector_rail", &[("shape", "east_west")]));

        let mut cart = Minecart::new(1, Kind::Plain, (0.5, 1.0, 0.5));
        cart.vx = 0.4;

        let mut pressed_at = None;

        for tick in 0..60 {
            run(&mut world, 1);

            let view = |x, y, z| world.get_block(x, y, z);
            crate::minecart::step_all(std::slice::from_mut(&mut cart), &view, &[], |_| true);

            note_carts(&mut world, vec![Cart { x: cart.x, y: cart.y, z: cart.z, reading: cart.reading() }]);
            step_entities(&mut world);
            settle(&mut world);

            let on = block(&world, (6, 1, 0)).is("powered", "true");

            if on && pressed_at.is_none() {
                // Ящик шириной 0,98 касается клетки 6, когда середина
                // дальше 5,51.
                assert!(cart.x > 5.51 && cart.x <= 5.51 + 0.4, "включилась не вовремя: {}", cart.x);
                pressed_at = Some(tick);
            }

            if let Some(at) = pressed_at
                && !on
            {
                assert_eq!(tick - at, 20, "погасла не на проверке через 20 тактов");
                return;
            }
        }

        panic!("детекторная рельса не сработала или не погасла");
    }

    /// Дверь от рычага открывается целиком — обе половины — и со звуком;
    /// закрывается тоже со звуком (чёрный ящик, door-lever).
    #[test]
    fn a_door_opens_whole_by_signal_and_is_heard() {
        let mut world = world();
        let lower = state("oak_door", &[("half", "lower"), ("facing", "south")]);
        let upper = state("oak_door", &[("half", "upper"), ("facing", "south")]);
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), lower);
        put(&mut world, (0, 2, 0), upper);
        put(&mut world, (1, 1, 0), lever);

        let reader = 1;
        world.watch_events(reader);

        use_block(&mut world, (1, 1, 0));
        settle(&mut world);

        assert!(block(&world, (0, 1, 0)).is("open", "true"), "низ не открылся");
        assert!(block(&world, (0, 2, 0)).is("open", "true"), "верх не открылся");
        assert!(block(&world, (0, 2, 0)).is("powered", "true"));

        let heard = sounds(&mut world, reader);

        assert_eq!(heard.first().map(|(name, _)| *name), Some("minecraft:block.wooden_door.open"));
        assert_eq!(heard.iter().filter(|(name, _)| name.contains("door")).count(), 1, "звук двери не один");

        use_block(&mut world, (1, 1, 0));
        settle(&mut world);

        assert!(block(&world, (0, 2, 0)).is("open", "false"), "верх не закрылся");
        assert!(sounds(&mut world, reader).iter().any(|(name, _)| *name == "minecraft:block.wooden_door.close"));
    }

    /// У люков, калиток и дверей из разного материала свои звуки.
    #[test]
    fn opening_sounds_follow_the_material() {
        let of = |name: &str, open: bool| {
            let block = Block {
                state: blocks::state_by_name(name).unwrap(),
                name: blocks::block_at_state(blocks::state_by_name(name).unwrap()).unwrap(),
                kind: kind_of(name),
                orientation: blocks::orientation(name),
            };

            opening_sound(&block, open)
        };

        assert_eq!(of("iron_door", true), "minecraft:block.iron_door.open");
        assert_eq!(of("iron_trapdoor", false), "minecraft:block.iron_trapdoor.close");
        assert_eq!(of("oak_trapdoor", true), "minecraft:block.wooden_trapdoor.open");
        assert_eq!(of("spruce_fence_gate", false), "minecraft:block.fence_gate.close");
        assert_eq!(of("cherry_door", true), "minecraft:block.cherry_wood_door.open");
        assert_eq!(of("warped_fence_gate", true), "minecraft:block.nether_wood_fence_gate.open");
        assert_eq!(of("bamboo_trapdoor", true), "minecraft:block.bamboo_wood_trapdoor.open");
        assert_eq!(of("waxed_oxidized_copper_door", false), "minecraft:block.copper_door.close");
    }

    /// Дверь, открытая рукой, звучит для всех, кроме того, кто открыл.
    #[test]
    fn a_door_opened_by_hand_is_not_heard_by_its_opener() {
        let mut world = world();
        let lower = state("oak_door", &[("half", "lower"), ("facing", "south")]);
        let upper = state("oak_door", &[("half", "upper"), ("facing", "south")]);

        put(&mut world, (0, 1, 0), lower);
        put(&mut world, (0, 2, 0), upper);

        let reader = 1;
        world.watch_events(reader);

        world.actor = Some(42);
        assert!(use_block(&mut world, (0, 1, 0)));
        world.actor = None;

        let excepted: Vec<Option<i32>> = world
            .events_since(0, reader)
            .0
            .into_iter()
            .filter_map(|event| match event {
                crate::world::WorldEvent::Sound(sound) if sound.name.contains("door") => Some(sound.except),
                _ => None,
            })
            .collect();

        assert_eq!(excepted, vec![Some(42)]);
    }

    /// Медная лампа щёлкает, когда меняет свет, и молчит, когда сигнал
    /// снимают (чёрный ящик, copper-bulb).
    #[test]
    fn a_copper_bulb_clicks_when_it_switches() {
        let mut world = world();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), blocks::state_by_name("copper_bulb").unwrap());
        put(&mut world, (1, 1, 0), lever);

        let reader = 1;
        world.watch_events(reader);

        let bulb_sounds = |world: &mut World| -> Vec<&'static str> {
            sounds(world, reader).into_iter().map(|(name, _)| name).filter(|name| name.contains("copper_bulb")).collect()
        };

        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        assert_eq!(bulb_sounds(&mut world), vec!["minecraft:block.copper_bulb.turn_on"]);

        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        assert!(bulb_sounds(&mut world).is_empty(), "снятие сигнала не звучит");

        use_block(&mut world, (1, 1, 0));
        settle(&mut world);
        assert_eq!(bulb_sounds(&mut world), vec!["minecraft:block.copper_bulb.turn_off"]);
    }

    /// Факел выгорает на восьмом выключении за 60 тактов — с шипением
    /// (событие 1502) — и больше не загорается (чёрный ящик, torch-burnout).
    #[test]
    fn a_torch_burns_out_on_the_eighth_turn_off() {
        let mut world = world();
        let torch = blocks::state_by_name("redstone_torch").unwrap();
        let lever = state("lever", &[("face", "wall"), ("facing", "west"), ("powered", "false")]);

        put(&mut world, (0, 1, 0), STONE);
        put(&mut world, (0, 2, 0), torch);
        put(&mut world, (-1, 1, 0), lever);

        let reader = 1;
        world.watch_events(reader);

        let fizzles = |world: &mut World| {
            world
                .events_since(0, reader)
                .0
                .into_iter()
                .filter(|event| matches!(event, crate::world::WorldEvent::Level { effect: 1502, .. }))
                .count()
        };

        for turn in 1..=8 {
            use_block(&mut world, (-1, 1, 0));
            settle(&mut world);
            run(&mut world, 2);
            assert!(!is_on(&block(&world, (0, 2, 0))), "факел не погас на {turn}-м выключении");
            assert_eq!(fizzles(&mut world), usize::from(turn == 8), "шипение не на восьмом");

            use_block(&mut world, (-1, 1, 0));
            settle(&mut world);
            run(&mut world, 2);
        }

        assert!(!is_on(&block(&world, (0, 2, 0))), "выгоревший факел загорелся");
    }

    /// Пыль, поставленная командой, форму не подбирает: остаётся какой
    /// дали, пересчитывается только сила (чёрный ящик, dust-join).
    #[test]
    fn a_wire_placed_by_command_keeps_its_shape() {
        let mut world = world();
        let lever = state("lever", &[("face", "floor"), ("facing", "north"), ("powered", "true")]);
        // Конец линии смотрит на восток, в пустоту.
        let wire = state("redstone_wire", &[("north", "side"), ("east", "side"), ("south", "none"), ("west", "none")]);

        put(&mut world, (0, 1, -1), lever);
        world.note_commanded(0, 1, 0);
        put(&mut world, (0, 1, 0), wire);

        let placed = block(&world, (0, 1, 0));

        assert!(placed.is("east", "side") && placed.is("north", "side") && placed.is("south", "none"), "форма пересчитана");
        assert_eq!(wire_power_of(&placed), 15, "сила не пересчитана");
    }

    /// Нажатая взвешенная плита меняет силу и отпускается только на своём
    /// пересчёте раз в 10 тактов, а не в тот такт, когда на ней стало
    /// больше или меньше предметов (чёрный ящик, plate-weights).
    #[test]
    fn a_pressed_weighted_plate_recounts_on_its_own_tick() {
        let mut world = world();
        let plate = blocks::state_by_name("light_weighted_pressure_plate").unwrap();

        put(&mut world, (0, 1, 0), plate);

        let items = |n: usize| -> Vec<Occupant> { (0..n).map(|_| Occupant::item(0.5, 1.0, 0.5, true)).collect() };
        let power = |world: &World| plate_power(&block(world, (0, 1, 0)));

        // Легла одна стопка — плита нажата сразу.
        note_occupants(&mut world, items(1));
        step_plates(&mut world, &items(1));
        settle(&mut world);
        assert_eq!(power(&world), 1);

        // Легли ещё четыре — сила прежняя до пересчёта.
        note_occupants(&mut world, items(5));
        step_plates(&mut world, &items(5));
        run(&mut world, 9);
        assert_eq!(power(&world), 1, "сила сменилась раньше пересчёта");
        run(&mut world, 1);
        assert_eq!(power(&world), 5, "пересчёт не пришёл через 10 тактов");

        // Все ушли — плита держится до следующего пересчёта.
        note_occupants(&mut world, Vec::new());
        step_plates(&mut world, &[]);
        run(&mut world, 9);
        assert_eq!(power(&world), 5, "отпустилась раньше пересчёта");
        run(&mut world, 1);
        assert_eq!(power(&world), 0, "не отпустилась на пересчёте");
    }

    /// Око в рамку, светокамень в якорь, кусок торта: сравнитель читает
    /// новое состояние (чёрный ящик, comparator-click).
    #[test]
    fn clicks_fill_frames_charge_anchors_and_eat_cakes() {
        let mut world = world();

        put(&mut world, (0, 1, 0), state("end_portal_frame", &[("eye", "false")]));
        put(&mut world, (2, 1, 0), state("respawn_anchor", &[("charges", "3")]));
        put(&mut world, (4, 1, 0), state("cake", &[("bites", "5")]));

        assert_eq!(use_with_item(&mut world, (0, 1, 0), Some("stone"), true), None);
        assert_eq!(use_with_item(&mut world, (0, 1, 0), Some("ender_eye"), true), Some(true));
        assert!(block(&world, (0, 1, 0)).is("eye", "true"));
        assert_eq!(use_with_item(&mut world, (0, 1, 0), Some("ender_eye"), true), Some(false), "второе око");

        assert_eq!(use_with_item(&mut world, (2, 1, 0), Some("glowstone"), false), Some(true));
        assert!(block(&world, (2, 1, 0)).is("charges", "4"));
        assert_eq!(use_with_item(&mut world, (2, 1, 0), Some("glowstone"), false), None, "полный якорь");

        assert_eq!(use_with_item(&mut world, (4, 1, 0), None, false), None, "сытый в выживании не ест");
        assert_eq!(use_with_item(&mut world, (4, 1, 0), None, true), Some(false));
        assert!(block(&world, (4, 1, 0)).is("bites", "6"));
        use_with_item(&mut world, (4, 1, 0), None, true);
        assert_eq!(world.get_block(4, 1, 0), AIR, "последний кусок не съел торт");
    }
}
