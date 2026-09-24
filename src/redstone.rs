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

use std::collections::{HashMap, VecDeque};

use crate::blocks::{self, Orientation};
use crate::world::{AIR, World};

/// Место в мире.
pub type Pos = (i32, i32, i32);

/// Наибольшая сила сигнала.
pub const MAX_POWER: u8 = 15;

/// Что сообщается клиенту о ходе поршня: выдвигается или задвигается.
const PISTON_EXTENDS: u8 = 0;
const PISTON_RETRACTS: u8 = 1;

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
const PRIORITY_NORMAL: i8 = 0;

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

/// Память редстоуна: то, что не помещается в состояние блока.
pub struct Memory {
    /// Такты, в которые факел переключался: по ним считается выгорание.
    torch_toggles: HashMap<Pos, VecDeque<u64>>,

    /// Сила на выходе сравнителя: в состоянии блока есть только «включён».
    comparator_output: HashMap<Pos, u8>,

    /// Поршни в пути: пока идёт ход, поршень ничего нового не начинает.
    moving: HashMap<Pos, Moving>,

    /// Выгоревшие факелы: они не загораются, пока не остынут.
    burned_torches: std::collections::HashSet<Pos>,

    /// Нажатые плиты и такт, когда на них последний раз кто-то был:
    /// плита отпускается не сразу, а спустя время после того, как с неё сошли.
    plates: HashMap<Pos, u64>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            torch_toggles: HashMap::new(),
            comparator_output: HashMap::new(),
            moving: HashMap::new(),
            burned_torches: std::collections::HashSet::new(),
            plates: HashMap::new(),
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
        self.plates.retain(|(x, _, z), _| keep(*x, *z));
        self.burned_torches.retain(|(x, _, z)| keep(*x, *z));
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
}

impl Moving {
    /// Собирает ход заново — из того, что записано в файле мира.
    pub fn restored(pos: Pos, extending: bool, facing: Dir, cargo: Vec<(Pos, i32)>) -> Self {
        Self {
            extending,
            cargo,
            head: step(pos, facing),
            facing,
        }
    }
}

/// Что за блок стоит в этом месте — с точки зрения редстоуна.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Wire,
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
        "note_block" => Kind::NoteBlock,
        "repeater" => Kind::Repeater,
        "comparator" => Kind::Comparator,
        "piston" | "sticky_piston" => Kind::Piston,
        "piston_head" => Kind::PistonHead,
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

/// Может ли блок держать на себе упавшее.
///
/// Воздух и всё, во что можно встать — трава, вода, — не держит.
fn holds_up(state: i32) -> bool {
    state != AIR && blocks::has_box(state)
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
        | Kind::Observer => block.is("powered", "true"),
        Kind::Torch => block.is("lit", "true"),
        Kind::RedstoneBlock => true,
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
    if !is_on(block) {
        return 0;
    }

    match block.kind {
        Kind::Plate => plate_power(block),

        Kind::Lever | Kind::Button | Kind::RedstoneBlock => MAX_POWER,

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
            Kind::Lever | Kind::Button => {
                if attached_to(&neighbour, neighbour_pos) == Some(pos) {
                    MAX_POWER
                } else {
                    0
                }
            }
            // Плита лежит на блоке и питает его.
            Kind::Plate if dir == Dir::Up => plate_power(&neighbour),

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
        | Kind::Lamp
        | Kind::Door
        | Kind::Trapdoor
        | Kind::Gate
        | Kind::Bulb
        | Kind::NoteBlock => {
            if !neighbour.conductive() {
                0
            } else if receiver_is_wire {
                strong_power(world, neighbour_pos)
            } else {
                weak_power(world, neighbour_pos)
            }
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
        Kind::Wire | Kind::Torch | Kind::Lever | Kind::Button | Kind::Plate | Kind::RedstoneBlock => {
            true
        }
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

    let rear = diode_input(world, pos, block);
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
fn destroy(world: &mut World, pos: Pos) -> bool {
    if world.get_block(pos.0, pos.1, pos.2) == AIR {
        return false;
    }

    world.note_destroyed(pos.0, pos.1, pos.2);
    world.set_block_quietly(pos.0, pos.1, pos.2, AIR);
    true
}

/// Через сколько тактов отпускается обычная плита, когда с неё сошли.
const PLATE_RELEASE: u64 = 20;

/// То же у взвешенной плиты — она отпускается вдвое быстрее.
const WEIGHTED_RELEASE: u64 = 10;

/// Нажимает и отпускает плиты по тому, кто на них стоит.
///
/// Плита — единственная деталь, которая слушает не редстоун, а мир:
/// на неё встают игроки и падают предметы. Поэтому её считает такт мира,
/// а не общий разбор сигналов.
///
/// Отпускается плита не сразу: обычная через секунду после того, как с неё
/// сошли, взвешенная — вдвое быстрее.
pub fn step_plates(world: &mut World, occupants: &[(f64, f64, f64)]) {
    let tick = world.tick();

    // Кто на какой плите стоит. Плита тонкая, и стоящий на ней находится
    // в её же клетке.
    let mut counts: HashMap<Pos, u32> = HashMap::new();

    for (x, y, z) in occupants {
        let pos = (x.floor() as i32, y.floor() as i32, z.floor() as i32);

        if Block::at(world, pos).kind == Kind::Plate {
            *counts.entry(pos).or_insert(0) += 1;
        }
    }

    for (pos, count) in &counts {
        let block = Block::at(world, *pos);

        if let Some(state) = pressed_state(&block, *count)
            && state != block.state
        {
            set(world, *pos, state);
        }

        world.redstone.plates.insert(*pos, tick);
    }

    // Те, с кого сошли: отпускаем, когда вышло время.
    let free: Vec<Pos> = world
        .redstone
        .plates
        .iter()
        .filter(|(pos, last)| {
            let block = Block::at(world, **pos);
            let delay = if is_weighted(block.name) {
                WEIGHTED_RELEASE
            } else {
                PLATE_RELEASE
            };

            !counts.contains_key(*pos) && tick.saturating_sub(**last) >= delay
        })
        .map(|(pos, _)| *pos)
        .collect();

    for pos in free {
        world.redstone.plates.remove(&pos);

        let block = Block::at(world, pos);

        if let Some(state) = pressed_state(&block, 0)
            && state != block.state
        {
            set(world, pos, state);
        }
    }
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

/// Разбирает только первый круг: соседей уже изменённых блоков, но не то,
/// что от них пошло дальше. Остаток ждёт следующего `settle`.
///
/// Нужно щелчку игрока: у оригинала повтор щёлкнутого блока клиенту уходит
/// после того, как перемены дошли до ближайших соседей, но до того, как они
/// пошли дальше (сверено чёрным ящиком: провод меняется до повтора рычага,
/// а музыкальный блок за проводом — после).
pub fn settle_once(world: &mut World) {
    // Берём снимок очереди: что изменится по ходу разбора, останется лежать
    // до следующего `settle`. Глубина сразу предельная, чтобы разбор соседа
    // не пошёл вглубь сам.
    for (pos, old) in world.take_changed() {
        block_changed(world, pos, old, MAX_DEPTH);
    }
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
    let quiet = same_block && keeps_quiet(new_block.kind);

    if !quiet {
        // Сперва сам блок: поставленный повторитель должен увидеть сигнал на
        // входе сразу, а не ждать, пока шевельнётся сосед.
        itself_changed(world, pos);
        settle_from(world, depth + 1);

        for dir in NEIGHBOUR_ORDER {
            neighbour_changed(world, step(pos, dir));
            settle_from(world, depth + 1);
        }
    }

    // Наблюдатель просыпается от изменения блока прямо перед глазами —
    // и только от него. По вики он ловит «shape updates coming from the
    // block it's facing»: постановку, слом и смену состояния.
    for dir in NEIGHBOUR_ORDER {
        let watcher_pos = step(pos, dir);
        let watcher = Block::at(world, watcher_pos);

        if watcher.kind == Kind::Observer && watcher.facing() == Some(dir.opposite()) {
            world.schedule_once(
                watcher_pos.0,
                watcher_pos.1,
                watcher_pos.2,
                OBSERVER_DELAY,
                PRIORITY_NORMAL,
            );
        }
    }

    // Поршень слушает не только себя, но и клетку прямо над собой
    // («квазисвязность»). В игре перемены вокруг той клетки до поршня не
    // доходят, и он остаётся выдвинутым, пока его не обновит что-то
    // вплотную. У нас решено иначе: поршень сам замечает такие перемены,
    // и лишний раз залипать не будет.
    for dir in NEIGHBOUR_ORDER {
        let piston_pos = step(step(pos, dir), Dir::Down);

        if piston_pos != pos && Block::at(world, piston_pos).kind == Kind::Piston {
            neighbour_changed(world, piston_pos);
            settle_from(world, depth + 1);
        }
    }

    // Дальше — те, до кого сила этого блока доходит не напрямую. Список
    // взят из таблицы обновлений на вики. Считается и по новому блоку,
    // и по прежнему: рычаг питал блок, к которому был прикреплён, и, сняв
    // рычаг, надо предупредить соседей того блока.

    let mut further = reach_of(&new_block, pos);

    if old_block.kind != new_block.kind {
        further.extend(reach_of(&old_block, pos));
    }

    for far in further {
        if far != pos {
            neighbour_changed(world, far);
            settle_from(world, depth + 1);
        }
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

        Kind::Lever | Kind::Button => match attached_to(block, pos) {
            Some(attached) => around(attached).collect(),
            None => Vec::new(),
        },

        // Плита предупреждает своих соседей и соседей блока, на котором
        // лежит. Своих — здесь же: общего оповещения у неё нет.
        Kind::Plate => around(pos).chain(around(step(pos, Dir::Down))).collect(),

        _ => Vec::new(),
    }
}

/// У блока в этом месте изменился сосед: решаем, что с ним делать.
fn neighbour_changed(world: &mut World, pos: Pos) {
    react(world, pos, false);
}

/// Блок только что поставлен или сменился сам. Разбирается как «сосед
/// изменился», кроме растений: у оригинала растение проверяет опору, лишь
/// когда меняется сосед, — трава, поставленная командой на камень, стоит,
/// пока её не тронут (сценарий чёрного ящика grass-crush).
fn itself_changed(world: &mut World, pos: Pos) {
    react(world, pos, true);
}

fn react(world: &mut World, pos: Pos, itself: bool) {
    let block = Block::at(world, pos);

    match block.kind {
        Kind::Wire => {
            if !supported(world, pos, &block) {
                destroy(world, pos);
                return;
            }

            if let Some(wanted) = wanted_wire(world, pos, &block)
                && wanted != block.state
            {
                set(world, pos, wanted);
            }
        }

        Kind::Torch => {
            if !supported(world, pos, &block) {
                destroy(world, pos);
                return;
            }

            if torch_should_be_lit(world, pos, &block) != is_on(&block) {
                world.schedule_once(pos.0, pos.1, pos.2, TORCH_DELAY, PRIORITY_NORMAL);
            }
        }

        Kind::Repeater => {
            if !supported(world, pos, &block) {
                destroy(world, pos);
                return;
            }

            let locked = repeater_locked(world, pos, &block);

            if locked != block.is("locked", "true")
                && let Some(state) = block.with("locked", yes_no(locked))
            {
                set(world, pos, state);
            }

            // Запертый повторитель не слушает ничего.
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
            if !supported(world, pos, &block) {
                destroy(world, pos);
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
            let instrument = instrument_under(world, pos);

            let mut state = block.with("powered", yes_no(powered)).unwrap_or(block.state);

            if let Some(kind) = block.orientation
                && let Some(next) = kind.with(state, "instrument", instrument)
            {
                state = next;
            }

            // Сперва новое состояние блока, потом нота — так у оригинала.
            if state != block.state {
                set(world, pos, state);
            }

            if powered && !was {
                play_note(world, pos, instrument, note_of(&block));
            }
        }

        // Лампа загорается сразу, а гаснет через четыре такта: если сигнал
        // за это время вернётся, она даже не мигнёт.
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

                if let Some(state) = opened {
                    set(world, pos, state);
                }
            }
        }

        Kind::Lever | Kind::Button | Kind::Plate => {
            if !supported(world, pos, &block) {
                destroy(world, pos);
            }
        }

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

            let wanted = piston_powered(world, pos, &block);

            if wanted != block.is("extended", "true") {
                world.queue_block_event(pos.0, pos.1, pos.2);
            }
        }

        // Голова держится на выдвинутом поршне: не стало его — не стало
        // и головы.
        Kind::PistonHead => {
            if !piston_behind(world, pos, &block) {
                set(world, pos, AIR);
            }
        }

        // Служебный блок на месте едущего ни на что не смотрит: он живёт
        // ровно столько, сколько идёт ход, и убирается вместе с ним.
        Kind::MovingPiston => {}

        // Наблюдатель на соседей не смотрит: его будит только изменение
        // блока прямо перед глазами, и делает это block_changed.
        Kind::RedstoneBlock | Kind::Observer | Kind::Other => {
            // Растению нужна своя опора: убрали землю из-под цветка — цветок
            // ломается и выпадает (правила — в модуле plants).
            if !itself && crate::plants::check(world, pos.0, pos.1, pos.2) {
                return;
            }

            // Песку и гравию нужна опора: не стало её — блок падает.
            // Заводить сущность отсюда нечем, поэтому место просто
            // запоминается, а падение начнёт такт мира.
            if falls(block.state) && !holds_up(world.get_block(pos.0, pos.1 - 1, pos.2)) {
                world.note_falling(pos.0, pos.1, pos.2);
            }
        }
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
        Kind::Wire | Kind::Repeater | Kind::Comparator | Kind::Plate => {
            holds_redstone(world.get_block(pos.0, pos.1 - 1, pos.2))
        }
        Kind::Torch | Kind::Lever | Kind::Button => attached_to(block, pos).is_some_and(|attached| {
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
    if blocks::block_at_state(state).is_some_and(|name| name.ends_with("_leaves")) {
        return false;
    }

    blocks::solid_top(state)
}

/// Пришло блочное событие: поршень трогается с места.
///
/// Это отдельная фаза такта мира, после всех запланированных тактов, —
/// так в игре, и от этого зависит, что успевает сработать раньше.
pub fn block_event(world: &mut World, pos: Pos) {
    let block = Block::at(world, pos);

    if block.kind != Kind::Piston {
        return;
    }

    // Поршень уже в пути: начатый ход не прерывается и второй раз
    // не начинается.
    if world.redstone.moving.contains_key(&pos) {
        return;
    }

    let powered = piston_powered(world, pos, &block);
    let extended = block.is("extended", "true");

    if powered == extended {
        return;
    }

    let Some(facing) = block.facing() else {
        return;
    };

    let Some(moving) = start_move(world, pos, &block, facing, powered) else {
        // Двигать нечего — поршень даже не трогается: упёршийся поршень
        // так и стоит задвинутым.
        return;
    };

    // Сперва сообщаем о ходе, и только потом меняем состояние поршня:
    // клиент по этому сообщению проигрывает ход сам, а если он сначала
    // увидит уже выдвинутый поршень, рисовать ему будет нечего.
    if let Some(id) = blocks::block_id(block.name) {
        let (action, sound) = if powered {
            (PISTON_EXTENDS, "minecraft:block.piston.extend")
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
    if powered && let Some(state) = block.with("extended", "true") {
        set_moved(world, pos, state);
    }

    world.redstone.moving.insert(pos, moving);
    world.schedule_block(pos.0, pos.1, pos.2, PISTON_MOVE, PRIORITY_NORMAL);
}

/// Доигрывает ход, у которого не стало поршня: блоки встают на места
/// назначения, а служебные блоки убираются.
fn abandon_move(world: &mut World, moving: &Moving) {
    for (place, state) in &moving.cargo {
        set_moved(world, *place, *state);
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
            }
        }
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
) -> Option<Moving> {
    let front = step(pos, facing);
    let sticky = block.name.starts_with("sticky");
    let hidden = moving_block(facing, sticky)?;

    // При задвигании голова уезжает первой: пока она стоит, тянущемуся
    // блоку некуда ехать. Сперва убираем её совсем — иначе она же и
    // перегородит расчёт, — а служебный блок ставим уже после.
    if !extending {
        set_hidden(world, front, AIR);
    }

    // Что поедет: при выдвигании — связка перед поршнем, при задвигании —
    // то, что липкий поршень тянет за собой.
    let group = if extending {
        push_group(world, front, facing, pos)?
    } else {
        pulled_group(world, pos, facing, sticky)
    };

    // Место головы занято ходом: оно не пустое, но и ничего не проводит.
    // Если туда ничего не приедет, клиенту говорят «пусто» — сверено
    // с оригиналом: при вытягивании блока о голове он не говорит вовсе.
    if !extending {
        set_hidden(world, front, hidden);

        if group.is_empty() {
            world.tell_later(front.0, front.1, front.2, PISTON_TELL_LATER);
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

    // Старые места пустеют: блоки уже едут. При вытягивании клиенту об
    // опустевшем месте говорят (на такт позже), при толкании — нет: так
    // у оригинала, клиент и сам рисует, как блок уезжает.
    for place in &group {
        set_hidden(world, *place, AIR);

        if !extending && !group.iter().any(|other| step(*other, step_to) == *place) {
            world.tell_later(place.0, place.1, place.2, PISTON_TELL_LATER);
        }
    }

    // А места назначения занимает служебный блок — так в игре: пока идёт
    // ход, туда нельзя ни встать, ни поставить что-нибудь.
    for (place, _) in &cargo {
        set_hidden(world, *place, hidden);
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
    })
}

/// Что липкий поршень тянет за собой, задвигаясь.
fn pulled_group(world: &World, pos: Pos, facing: Dir, sticky: bool) -> Vec<Pos> {
    if !sticky {
        return Vec::new();
    }

    let ahead = step(step(pos, facing), facing);
    let pulled = world.get_block(ahead.0, ahead.1, ahead.2);

    if push_reaction(pulled) != Push::Moves || !can_be_pulled(pulled) {
        return Vec::new();
    }


    push_group(world, ahead, facing.opposite(), pos).unwrap_or_default()
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
        set_moved(world, *place, *state);
    }

    if !moving.extending && let Some(retracted) = block.with("extended", "false") {
        set_moved(world, pos, retracted);
    }

    // Пока поршень ехал, питание могло смениться, а все обновления в это
    // время он пропускал. Поэтому, доехав, он смотрит на себя заново.
    neighbour_changed(world, pos);
}

/// Пришёл запланированный такт для детали в этом месте.
pub fn ticked(world: &mut World, pos: Pos) {
    let block = Block::at(world, pos);

    match block.kind {
        // Поршень: вышло время хода.
        Kind::Piston => match world.redstone.moving.remove(&pos) {
            Some(moving) => finish_move(world, pos, moving),

            // Про этот ход мы ничего не помним — значит он начался до
            // перезапуска сервера: память не переживает остановку, а
            // запланированный такт переживает. Что именно ехало, уже
            // не узнать, поэтому просто приводим поршень в порядок:
            // помечен выдвинутым — ставим голову, задвинутым — убираем.
            None => repair_piston(world, pos, &block),
        },

        // Наблюдатель: заметил изменение — даёт короткий сигнал, и через
        // два такта сам его снимает.
        Kind::Observer => {
            let powered = block.is("powered", "true");

            if let Some(state) = block.with("powered", yes_no(!powered)) {
                set(world, pos, state);
            }

            if !powered {
                world.schedule_once(pos.0, pos.1, pos.2, OBSERVER_PULSE, PRIORITY_NORMAL);
            }
        }

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

            // Пороги разные: выгорает факел, когда переключений стало
            // больше восьми, а загореться снова может, только когда их
            // осталось меньше восьми — «until the number of state changes
            // in the last 60 game ticks drops to fewer than eight».
            let count = toggles.len();

            if count > BURNOUT_TOGGLES {
                world.redstone.burned_torches.insert(pos);
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

        Kind::Button => {
            if let Some(state) = block.with("powered", "false") {
                set(world, pos, state);
            }
        }

        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Музыкальный блок
// ---------------------------------------------------------------------------

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
/// Звука нет, если сверху не воздух: блок заглушён. Щелчок при этом всё
/// равно поднимает ноту — этим занимается вызывающий.
fn play_note(world: &mut World, pos: Pos, instrument: &'static str, note: i32) {
    if world.get_block(pos.0, pos.1 + 1, pos.2) != AIR {
        return;
    }

    let pitch = 2f32.powf((note - 12) as f32 / 12.0);

    world.note_sound(pos.0, pos.1, pos.2, sound_of(instrument), 3.0, pitch);

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
        Kind::Lever => {
            // Щелчок слышно раньше, чем видно: громкость 0,3, высота 0,6
            // при включении и 0,5 при выключении (вики: «Lever», звуки).
            let pitch = if is_on(&block) { 0.5 } else { 0.6 };
            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.lever.click", 0.3, pitch);
            block.with("powered", yes_no(!is_on(&block)))
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

        _ => return false,
    };

    // Соседей здесь не разбираем: порядок, в каком о щелчке узнают соседи
    // и клиент, задаёт тот, кто щёлкнул (см. place_block в сети).
    if let Some(state) = new_state {
        set(world, pos, state);
    }

    true
}

/// Что деталь делает после щелчка — когда клиенту уже сказано, каким
/// стал блок. Пока это только музыкальный блок: у оригинала нота идёт
/// следом за изменением блока и его повтором, а не перед ними.
pub fn after_use(world: &mut World, pos: Pos) {
    let block = Block::at(world, pos);

    if block.kind == Kind::NoteBlock {
        play_note(world, pos, instrument_under(world, pos), note_of(&block));
    }
}

/// Деревянная ли кнопка — по имени: каменные две, остальные из дерева.
fn is_wooden(name: &str) -> bool {
    !matches!(name, "stone_button" | "polished_blackstone_button")
}

fn yes_no(value: bool) -> &'static str {
    if value { "true" } else { "false" }
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
            for place in world.take_block_events() {
                block_event(world, place);
                settle(world);
            }

            settle(world);
        }
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

        // И блока редстоуна наискось довольно: он стоит рядом с той же
        // клеткой. В игре поршень заметил бы это не сразу, а только от
        // стороннего обновления; у нас — сразу, так решено.
        let mut diagonal = super::tests::world();
        put(&mut diagonal, (0, 1, 0), piston);
        put(&mut diagonal, (1, 2, 0), source);
        run(&mut diagonal, 6);

        assert!(
            block(&diagonal, (0, 1, 0)).is("extended", "true"),
            "поршень не увидел блок наискось"
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


    /// Блок красного камня по диагонали сверху двигает поршень туда и
    /// обратно сразу, без посторонних обновлений.
    ///
    /// В игре это работает иначе: там поршень замечает такое питание только
    /// когда его обновит что-нибудь вплотную, и потому остаётся выдвинутым
    /// после снятия блока («Update-based QC Activation by Block of Redstone»).
    /// Мы решили не повторять это залипание: поршень сам следит за клеткой
    /// над собой.
    #[test]
    fn a_block_of_redstone_moves_a_piston_right_away() {
        let mut world = world();
        let redstone_block = blocks::state_by_name("redstone_block").unwrap();
        let piston = state("piston", &[("facing", "up"), ("extended", "false")]);

        let extended = |world: &World| block(world, (0, 1, 0)).is("extended", "true");

        put(&mut world, (0, 1, 0), piston);
        run(&mut world, 5);
        assert!(!extended(&world), "поршень выдвинулся без причины");

        // Блок стоит наискось над поршнем — поршень выдвигается.
        put(&mut world, (1, 2, 0), redstone_block);
        run(&mut world, 5);
        assert!(extended(&world), "поршень не заметил блок наискось");

        // И обратно: сняли блок — задвинулся, ничего толкать не надо.
        put(&mut world, (1, 2, 0), AIR);
        run(&mut world, 5);
        assert!(!extended(&world), "поршень остался выдвинутым без питания");
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
        ] {
            assert!(keeps_quiet(kind), "{kind:?} должен молчать");
        }

        // А все остальные — предупреждают, как обычные блоки.
        for kind in [Kind::Wire, Kind::Torch, Kind::Lever, Kind::Button, Kind::Piston, Kind::Other] {
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
        step_plates(&mut world, &[(0.5, 1.0, 0.5)]);
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

        // На земле стоит спокойно.
        put(&mut world, (0, 1, 0), sand);
        assert!(world.take_falling().is_empty(), "песок на земле собрался падать");

        // А в воздухе — падает.
        put(&mut world, (0, 5, 0), sand);
        assert_eq!(world.take_falling(), vec![(0, 5, 0)], "песок в воздухе не упал");

        // Убрали опору из-под лежащего — тоже падает.
        put(&mut world, (0, 2, 0), sand);
        world.take_falling();
        put(&mut world, (0, 1, 0), AIR);

        assert!(
            world.take_falling().contains(&(0, 2, 0)),
            "песок не заметил, что опоры не стало"
        );
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
        let level = path.join("level.dat");
        let text = std::fs::read_to_string(&level).expect("level.dat читается");
        let without: String = text
            .lines()
            .filter(|line| !line.starts_with("tick = 0 1 0 block"))
            .map(|line| format!("{}\n", line))
            .collect();

        assert!(text.contains("moving = 0 1 0"), "ход не записан: {}", text);
        assert!(
            without.len() < text.len(),
            "такта хода в записи не нашлось: {}",
            text
        );

        std::fs::write(&level, without).expect("level.dat записывается");

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
}
