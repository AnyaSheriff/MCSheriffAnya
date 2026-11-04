// Крепости Незера: где стоят, из каких кусков складываются и чем кладутся.
//
// Всё — по замеру мира оригинала 26.1.2 (tools/measure/nether_fortress.py,
// 40 крепостей и 1600 регионов): сервер оригинала только складывал мир,
// а куски, их размеры, выходы и частоты сняты с region-файлов — с блоков
// и с записей о строениях в чанках. Кода игры здесь нет.
//
// Где: мир Незера поделён на регионы 27×27 чанков, в каждом ровно одна
// постройка; её чанк — в первых 23 чанках региона по каждой оси (замер:
// сдвиги 0..22 поровну). Крепость в 40 % регионов, в остальных попытка
// бастиона (nether_bastion.rs), которая в дельтах пропускается.
//
// Как: крепость — дерево кусков. Первый — перекрёсток моста (nebcr),
// серединой в клетке (11, 11) чанка старта, низом на высоте 48..69 (чаще
// всего 62). У каждого куска свои выходы; к выходу пристраивается кусок
// своей семьи (мост или замок) по весам, пока не кончились ограниченные
// виды; кончились — ставится обрыв моста (nebef). Кусок не встаёт, если
// задевает уже поставленный; выход на границе 112 блоков от угла старта
// остаётся пустым. Числа кусков у нас и у оригинала сходятся (см. тест
// fortress_piece_counts).
//
// Чем: образцы кусков — src/world/nether_fortress.txt, снятые с тех же 40
// крепостей: что ставится всегда, где всегда пусто, где остаётся рельеф
// и какие столбцы уходят кирпичом вниз до первого сплошного блока (опоры
// мостов и стен).
//
// Раскладка зависит только от семени и региона, поэтому каждый чанк сам
// строит ближние крепости целиком и кладёт только то, что в него попало.

use std::sync::OnceLock;

use crate::blocks;

/// Сторона региона в чанках.
pub const REGION: i32 = 27;

/// В скольких первых чанках региона может стоять старт.
const SPREAD: i32 = 23;

/// Доля крепостей среди построек вне базальтовых дельт.
pub const FORTRESS_SHARE: f64 = 0.4;

/// Дальше этого от середины старта (по большей из осей) новых кусков нет.
const REACH: i32 = 112;

/// Глубже этого по дереву — только обрыв моста.
const DEPTH: u32 = 30;

/// Сколько раз пробовать другой кусок, если выпавший не встаёт.
const TRIES: usize = 5;

/// На сколько чанков от чанка старта достаёт крепость: 112 блоков до
/// выхода и кусок до 19 блоков за ним (замер: не дальше 130).
pub const RADIUS_CHUNKS: i32 = 9;

/// Виды кусков. Имена — как в записях о строениях у оригинала.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    /// nebcr — перекрёсток моста.
    Crossing,
    /// nebs — пролёт моста.
    Bridge,
    /// nebef — обрыв моста.
    BridgeEnd,
    /// nerc — перекрёсток с комнатой.
    RoomCrossing,
    /// nesr — комната с лестницей наверх.
    StairsRoom,
    /// nemt — площадка со спавнером ифритов.
    Throne,
    /// nece — вход в замок с лавовым колодцем.
    Entrance,
    /// nesc — коридор замка.
    Corridor,
    /// nescsc — перекрёсток коридоров.
    CorridorCrossing,
    /// nesclt — поворот налево (бывает сундук).
    TurnLeft,
    /// nescrt — поворот направо (бывает сундук).
    TurnRight,
    /// nectb — развилка с балконом.
    Balcony,
    /// neccs — лестница коридора вниз.
    CorridorStairs,
    /// necsr — комната с незерским наростом.
    StalkRoom,
}

const KINDS: [Kind; 14] = [
    Kind::Crossing,
    Kind::Bridge,
    Kind::BridgeEnd,
    Kind::RoomCrossing,
    Kind::StairsRoom,
    Kind::Throne,
    Kind::Entrance,
    Kind::Corridor,
    Kind::CorridorCrossing,
    Kind::TurnLeft,
    Kind::TurnRight,
    Kind::Balcony,
    Kind::CorridorStairs,
    Kind::StalkRoom,
];

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Crossing => "nebcr",
            Kind::Bridge => "nebs",
            Kind::BridgeEnd => "nebef",
            Kind::RoomCrossing => "nerc",
            Kind::StairsRoom => "nesr",
            Kind::Throne => "nemt",
            Kind::Entrance => "nece",
            Kind::Corridor => "nesc",
            Kind::CorridorCrossing => "nescsc",
            Kind::TurnLeft => "nesclt",
            Kind::TurnRight => "nescrt",
            Kind::Balcony => "nectb",
            Kind::CorridorStairs => "neccs",
            Kind::StalkRoom => "necsr",
        }
    }

    fn at(self) -> usize {
        KINDS.iter().position(|kind| *kind == self).unwrap_or(0)
    }

    /// Насколько низ куска выше уровня выхода, к которому он пристроен:
    /// комнаты моста стоят на 3 выше настила, комната с наростом — на 3
    /// ниже пола коридора, лестница вниз — на 7 (замер).
    fn lift(self) -> i32 {
        match self {
            Kind::RoomCrossing | Kind::StairsRoom | Kind::Throne => 3,
            Kind::StalkRoom => -3,
            Kind::CorridorStairs => -7,
            _ => 0,
        }
    }

    /// Выходы: клетка входа следующего куска в рамке этого (a, y, b) и куда
    /// он смотрит. Замер: у каждого вида выходы всегда одни и те же.
    fn exits(self) -> &'static [(i32, i32, i32, Turn)] {
        match self {
            Kind::Crossing => &[(9, 0, 19, Turn::Ahead), (-1, 0, 9, Turn::Minus), (19, 0, 9, Turn::Plus)],
            Kind::Bridge => &[(2, 0, 19, Turn::Ahead)],
            Kind::RoomCrossing => &[(3, -3, 7, Turn::Ahead), (-1, -3, 3, Turn::Minus), (7, -3, 3, Turn::Plus)],
            Kind::StairsRoom => &[(7, 3, 3, Turn::Plus)],
            Kind::Entrance => &[(6, 3, 13, Turn::Ahead)],
            Kind::Corridor => &[(2, 0, 5, Turn::Ahead)],
            Kind::CorridorCrossing => &[(2, 0, 5, Turn::Ahead), (-1, 0, 2, Turn::Minus), (5, 0, 2, Turn::Plus)],
            Kind::TurnLeft => &[(-1, 0, 2, Turn::Minus)],
            Kind::TurnRight => &[(5, 0, 2, Turn::Plus)],
            Kind::Balcony => &[(-1, 0, 2, Turn::Minus), (9, 0, 2, Turn::Plus)],
            Kind::CorridorStairs => &[(2, 0, 10, Turn::Ahead)],
            Kind::StalkRoom => &[(6, 3, 13, Turn::Ahead), (6, 11, 13, Turn::Ahead)],
            Kind::BridgeEnd | Kind::Throne => &[],
        }
    }

    /// Семья выходов: к мосту пристраивается мост, к замку — замок.
    fn castle(self) -> bool {
        matches!(
            self,
            Kind::Entrance
                | Kind::Corridor
                | Kind::CorridorCrossing
                | Kind::TurnLeft
                | Kind::TurnRight
                | Kind::Balcony
                | Kind::CorridorStairs
                | Kind::StalkRoom
        )
    }
}

/// Куда смотрит следующий кусок относительно этого.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Turn {
    Ahead,
    /// В сторону +a рамки.
    Plus,
    /// В сторону −a.
    Minus,
}

/// Куски мостовой семьи: вес и сколько их может быть в крепости (0 — без
/// предела). Предел — наибольшее число в замере (вики называет те же: 5
/// перекрёстков, 4 комнаты-перекрёстка, 3 комнаты с лестницей, 2 площадки
/// со спавнером, 1 вход в замок); веса подобраны так, что средние числа
/// кусков сходятся с замером.
const BRIDGE: [(Kind, u32, u32); 6] = [
    (Kind::Bridge, 30, 0),
    (Kind::Crossing, 10, 5),
    (Kind::RoomCrossing, 10, 4),
    (Kind::StairsRoom, 10, 3),
    (Kind::Throne, 5, 2),
    (Kind::Entrance, 5, 1),
];

/// Куски замка: так же (5 перекрёстков, по 10 поворотов, 3 лестницы вниз,
/// 2 балкона, 2 комнаты с наростом).
const CASTLE: [(Kind, u32, u32); 7] = [
    (Kind::Corridor, 25, 0),
    (Kind::CorridorCrossing, 15, 5),
    (Kind::TurnRight, 5, 10),
    (Kind::TurnLeft, 5, 10),
    (Kind::CorridorStairs, 10, 3),
    (Kind::Balcony, 7, 2),
    (Kind::StalkRoom, 5, 2),
];

/// Вперёд по сторонам: 0 — юг (+z), 1 — запад, 2 — север, 3 — восток.
const FORWARD: [(i32, i32); 4] = [(0, 1), (-1, 0), (0, -1), (1, 0)];

/// Вбок (+a) по сторонам. У сторон 2 и 3 рамка зеркальна: +a смотрит
/// туда же, что у 0 и 1 (замер по несимметричным кускам).
const SIDEWAYS: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 0), (0, 1)];

fn mirrored(facing: u8) -> bool {
    facing >= 2
}

/// Сторона по направлению.
fn facing_of(direction: (i32, i32)) -> u8 {
    FORWARD.iter().position(|d| *d == direction).unwrap_or(0) as u8
}

/// Поставленный кусок: вид, сторона, клетка середины входа (a = W/2, низ,
/// b = 0) и занятый объём.
#[derive(Clone, Debug)]
pub struct Placed {
    pub kind: Kind,
    pub facing: u8,
    origin: (i32, i32, i32),
    /// x0, y0, z0, x1, y1, z1 — включительно.
    pub bounds: [i32; 6],
    depth: u32,
    /// Своё число: обрыв моста и сундук решаются по нему.
    roll: u64,
}

impl Placed {
    fn new(kind: Kind, facing: u8, origin: (i32, i32, i32), roll: u64) -> Placed {
        let (width, length, height) = templates().size(kind);
        let mut placed = Placed { kind, facing, origin, bounds: [0; 6], depth: 0, roll };
        let (x0, _, z0) = placed.world(0, 0, 0);
        let (x1, _, z1) = placed.world(width - 1, 0, length - 1);

        placed.bounds = [x0.min(x1), origin.1, z0.min(z1), x0.max(x1), origin.1 + height - 1, z0.max(z1)];
        placed
    }

    /// Клетка (a, y, b) рамки куска в мире.
    fn world(&self, a: i32, y: i32, b: i32) -> (i32, i32, i32) {
        let (width, _, _) = templates().size(self.kind);
        let (fx, fz) = FORWARD[self.facing as usize];
        let (lx, lz) = SIDEWAYS[self.facing as usize];
        let side = a - width / 2;

        (self.origin.0 + lx * side + fx * b, self.origin.1 + y, self.origin.2 + lz * side + fz * b)
    }

    /// Низы столбцов-опор куска в мире — для замеров.
    #[cfg(all(test, not(seedmap_core)))]
    pub fn support_columns(&self) -> Vec<(i32, i32, i32)> {
        templates().pieces[self.kind.at()].supports.iter().map(|&(a, b)| self.world(a, 0, b)).collect()
    }

    fn touches(&self, other: &[i32; 6]) -> bool {
        let a = &self.bounds;

        a[0] <= other[3] && other[0] <= a[3] && a[1] <= other[4] && other[1] <= a[4] && a[2] <= other[5] && other[2] <= a[5]
    }
}

/// Решения по региону: чанк старта и бросок «крепость или бастион».
pub fn region_start(seed: i64, region_x: i32, region_z: i32) -> (i32, i32, f64) {
    // Чанк попытки и выбор семейства имеют независимые потоки.
    // Параметры проверены по началам обоих сохранённых миров 26.3.
    let placement_seed = seed.wrapping_add(i64::from(region_x).wrapping_mul(341_873_128_712))
        .wrapping_add(i64::from(region_z).wrapping_mul(132_897_987_541)).wrapping_add(30_084_232);
    let mut placement = super::nether::LegacyRandom::new(placement_seed);
    let chunk_x = region_x * REGION + placement.below(SPREAD);
    let chunk_z = region_z * REGION + placement.below(SPREAD);
    let mut selection = super::nether::LegacyRandom::for_chunk(seed, chunk_x, chunk_z);
    (chunk_x, chunk_z, f64::from(selection.below(5)) / 5.0)
}

/// Регионы, чья постройка может задеть этот чанк.
pub fn regions_near(chunk_x: i32, chunk_z: i32) -> impl Iterator<Item = (i32, i32)> {
    let span = |at: i32| (at - RADIUS_CHUNKS).div_euclid(REGION)..=(at + RADIUS_CHUNKS).div_euclid(REGION);

    span(chunk_x).flat_map(move |region_x| span(chunk_z).map(move |region_z| (region_x, region_z)))
}

/// Раскладка крепости со стартом в этом чанке.
pub fn plan(seed: i64, chunk_x: i32, chunk_z: i32) -> Vec<Placed> {
    let mut dice = super::nether::LegacyRandom::for_chunk(seed, chunk_x, chunk_z);
    let (center_x, center_z) = (chunk_x * 16 + 11, chunk_z * 16 + 11);
    // Старт собирается на y 64; после дерева сдвигается вся рамка.
    // minecraft.wiki/w/Nether_fortress: «starts with a plain four-way crossing».
    // Поток и очередь проверены по 479 кускам шести сохранённых крепостей 26.3.
    let low = 64;
    let facing = [2, 3, 0, 1][dice.below(4) as usize];
    let (fx, fz) = FORWARD[facing as usize];
    let mut start = Placed::new(Kind::Crossing, facing, (center_x - fx * 9, low, center_z - fz * 9), 0);
    start.depth = 0;

    let mut pieces = vec![start];
    let mut pending = Vec::new();
    let mut next_parent = Some(0usize);
    let mut counts = [0u32; KINDS.len()];
    let mut last = None;
    counts[Kind::Crossing.at()] = 1;

    while let Some(index) = next_parent {
        let parent = pieces[index].clone();

        for &(a, y, b, turn) in parent.kind.exits() {
            let (x, level, z) = parent.world(a, y, b);

            let (fx, fz) = FORWARD[parent.facing as usize];
            let (lx, lz) = SIDEWAYS[parent.facing as usize];
            let direction = match turn {
                Turn::Ahead => (fx, fz),
                Turn::Plus => (lx, lz),
                Turn::Minus => (-lx, -lz),
            };
            let facing = facing_of(direction);
            if (x - (chunk_x * 16 + 2)).abs().max((z - (chunk_z * 16 + 2)).abs()) >= REACH {
                let end = Placed::new(Kind::BridgeEnd, facing, (x, level, z), 0);
                // Даже отвергнутый край расходует число, если рамка свободна.
                if end.bounds[1] > 10 && !pieces.iter().any(|other| end.touches(&other.bounds)) { dice.integer(); }
                continue;
            }

            let castle = if parent.kind == Kind::Balcony { dice.below(8) > 0 } else { parent.kind.castle() };
            let table: &[(Kind, u32, u32)] = if castle { &CASTLE } else { &BRIDGE };
            let room = table.iter().any(|&(kind, _, cap)| cap > 0 && counts[kind.at()] < cap);
            let mut placed = None;

            if parent.depth < DEPTH && room {
                'attempt: for _ in 0..TRIES {
                    let free: Vec<_> = table.iter().filter(|&&(kind, _, cap)| cap == 0 || counts[kind.at()] < cap).collect();
                    let total: u32 = free.iter().map(|(_, weight, _)| weight).sum();
                    let mut pick = dice.below(total as i32);
                    for &&(kind, weight, _) in &free {
                        pick -= weight as i32;
                        if pick >= 0 { continue; }
                        if last == Some(kind) && !matches!(kind, Kind::Bridge | Kind::Corridor) { continue 'attempt; }
                        let mut candidate = Placed::new(kind, facing, (x, level + kind.lift(), z), 0);
                        if candidate.bounds[1] > 10 && !pieces.iter().any(|other| candidate.touches(&other.bounds)) {
                            if matches!(kind, Kind::TurnLeft | Kind::TurnRight) { candidate.roll = if dice.below(3) == 0 { 0 } else { 1 }; }
                            last = Some(kind);
                            placed = Some(candidate);
                            break 'attempt;
                        }
                    }
                }
            }

            // Не встало ничего — обрыв моста; у замка он на 3 ниже пола.
            if placed.is_none() {
                let lift = if parent.kind.castle() { -3 } else { 0 };
                let mut end = Placed::new(Kind::BridgeEnd, facing, (x, level + lift, z), 0);

                if !pieces.iter().any(|other| end.touches(&other.bounds)) {
                    end.roll = dice.integer() as u32 as u64;
                    placed = Some(end);
                }
            }

            if let Some(mut child) = placed {
                child.depth = parent.depth + 1;
                counts[child.kind.at()] += 1;
                pieces.push(child);

                pending.push(pieces.len() - 1);
            }
        }
        next_parent = if pending.is_empty() { None } else { Some(pending.remove(dice.below(pending.len() as i32) as usize)) };
    }
    let min = pieces.iter().map(|p| p.bounds[1]).min().unwrap();
    let max = pieces.iter().map(|p| p.bounds[4]).max().unwrap();
    let room = 70 - 48 + 1 - (max - min + 1);
    let bottom = 48 + if room > 1 { dice.below(room) } else { 0 };
    for piece in &mut pieces {
        piece.origin.1 += bottom - min;
        piece.bounds[1] += bottom - min;
        piece.bounds[4] += bottom - min;
    }
    pieces
}

/// Клетка образца.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// Рельеф остаётся как есть.
    Keep,
    /// Всегда пусто.
    Air,
    /// Блок: номер в палитре куска.
    Block(u16),
}

/// Образец куска и его палитра в четырёх сторонах.
struct Template {
    width: i32,
    length: i32,
    height: i32,
    cells: Vec<Cell>,
    /// Состояния палитры по сторонам.
    states: [Vec<i32>; 4],
    /// Столбцы (a, b), уходящие кирпичом вниз.
    supports: Vec<(i32, i32)>,
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> Cell {
        self.cells[((y * self.length + b) * self.width + a) as usize]
    }
}

pub struct Templates {
    pieces: Vec<Template>,
    bricks: i32,
    chest: [i32; 4],
    chest_turn_right: [i32; 4],
}

impl Templates {
    fn size(&self, kind: Kind) -> (i32, i32, i32) {
        let template = &self.pieces[kind.at()];
        (template.width, template.length, template.height)
    }
}

/// Образцы из src/world/nether_fortress.txt — разбираются один раз.
fn templates() -> &'static Templates {
    static TEMPLATES: OnceLock<Templates> = OnceLock::new();

    TEMPLATES.get_or_init(|| parse(include_str!("nether_fortress.txt")))
}

/// Состояние с направлениями образца (вперёд — south, +a — east) в мире
/// для этой стороны.
pub(super) fn orient(text: &str, facing: u8) -> String {
    let Some((name, rest)) = text.split_once('[') else {
        return text.to_string();
    };
    let (fx, fz) = FORWARD[facing as usize];
    let (lx, lz) = SIDEWAYS[facing as usize];
    let world = |direction: &str| {
        let (x, z) = match direction {
            "south" => (fx, fz),
            "north" => (-fx, -fz),
            "east" => (lx, lz),
            _ => (-lx, -lz),
        };
        match (x, z) {
            (0, 1) => "south",
            (0, -1) => "north",
            (1, 0) => "east",
            _ => "west",
        }
    };
    let is_direction = |word: &str| matches!(word, "north" | "south" | "east" | "west");
    let properties: Vec<String> = rest
        .trim_end_matches(']')
        .split(',')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));

            if is_direction(key) {
                format!("{}={}", world(key), value)
            } else if key == "facing" && is_direction(value) {
                format!("facing={}", world(value))
            } else if key == "shape" && mirrored(facing) && (value.contains("left") || value.contains("right")) {
                let swapped = value.replace("left", "#").replace("right", "left").replace('#', "right");
                format!("shape={}", swapped)
            } else {
                pair.to_string()
            }
        })
        .collect();

    format!("{}[{}]", name, properties.join(","))
}

fn parse(text: &str) -> Templates {
    let named = |name: &str| blocks::state_from_text(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name));
    let mut pieces: Vec<Option<Template>> = (0..KINDS.len()).map(|_| None).collect();
    let mut lines = text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).peekable();

    while let Some(line) = lines.next() {
        let words: Vec<&str> = line.split_whitespace().collect();

        if words.first() != Some(&"piece") {
            continue;
        }

        let kind = KINDS.iter().copied().find(|kind| kind.id() == words[1]).expect("вид куска известен");
        let number = |at: usize| words[at].parse::<i32>().expect("число в заголовке куска");
        let (width, length, height) = (number(2), number(3), number(4));
        let mut palette: Vec<(char, String)> = vec![('#', "nether_bricks".into())];
        let mut cells = Vec::with_capacity((width * length * height) as usize);
        let mut supports = Vec::new();

        while let Some(line) = lines.next_if(|line| !line.starts_with("piece")) {
            let trimmed = line.trim();

            if let Some(list) = trimmed.strip_prefix("supports") {
                supports = list
                    .split_whitespace()
                    .filter_map(|pair| pair.split_once(','))
                    .map(|(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(0)))
                    .collect();
            } else if trimmed.starts_with("layer") {
                for _ in 0..length {
                    let row = lines.next().expect("строка слоя").trim();

                    for symbol in row.chars() {
                        cells.push(match symbol {
                            '.' => Cell::Keep,
                            '_' => Cell::Air,
                            symbol => Cell::Block(
                                palette.iter().position(|(key, _)| *key == symbol).expect("знак есть в палитре") as u16,
                            ),
                        });
                    }
                }
            } else if let Some((symbol, state)) = trimmed.split_once(' ') {
                palette.push((symbol.chars().next().unwrap_or('?'), state.to_string()));
            }
        }

        assert_eq!(cells.len(), (width * length * height) as usize, "размер куска {}", kind.id());

        let states = [0u8, 1, 2, 3].map(|facing| palette.iter().map(|(_, state)| named(&orient(state, facing))).collect());

        pieces[kind.at()] = Some(Template { width, length, height, cells, states, supports });
    }

    // Сундук в повороте: в какую сторону мира смотрит при каждой стороне
    // куска (замер: у поворота налево — север, север, юг, север; у
    // поворота направо — север, юг, юг, юг).
    let chest = |facing: &str| named(&format!("chest[facing={},type=single,waterlogged=false]", facing));

    Templates {
        pieces: pieces.into_iter().map(|piece| piece.expect("все куски описаны")).collect(),
        bricks: named("nether_bricks"),
        chest: [chest("north"), chest("north"), chest("south"), chest("north")],
        chest_turn_right: [chest("north"), chest("south"), chest("south"), chest("south")],
    }
}

/// Куда кладётся крепость: кусок чанка высотой 0..=127, уложенный так же,
/// как в nether.rs (по высоте, внутри слоя по Z, потом по X).
pub struct Canvas<'a> {
    pub blocks: &'a mut [i32],
    pub x0: i32,
    pub z0: i32,
    /// Пусто ли место — опору можно тянуть сквозь него.
    pub passable: &'a dyn Fn(i32) -> bool,
    /// Чем стало пустое место крепости (воздух).
    pub air: i32,
    /// Тянуть ли опоры вниз.
    pub supports: bool,
}

impl Canvas<'_> {
    pub(super) fn at(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (local_x, local_z) = (x - self.x0, z - self.z0);

        ((0..16).contains(&local_x) && (0..16).contains(&local_z) && (1..127).contains(&y))
            .then(|| ((y * 16 + local_z) * 16 + local_x) as usize)
    }
}

/// Кладёт куски крепости, попавшие в этот чанк.
pub fn paint(pieces: &[Placed], canvas: &mut Canvas) {
    let templates = templates();
    let chunk = [canvas.x0, 0, canvas.z0, canvas.x0 + 15, 127, canvas.z0 + 15];

    for piece in pieces {
        let reach = [piece.bounds[0], 0, piece.bounds[2], piece.bounds[3], piece.bounds[4], piece.bounds[5]];

        if reach[0] > chunk[3] || chunk[0] > reach[3] || reach[2] > chunk[5] || chunk[2] > reach[5] {
            continue;
        }

        let template = &templates.pieces[piece.kind.at()];
        let states = &template.states[piece.facing as usize];
        let end = (piece.kind == Kind::BridgeEnd).then(|| end_lengths(piece.roll));

        for y in 0..template.height {
            for b in 0..template.length {
                for a in 0..template.width {
                    let (x, wy, z) = piece.world(a, y, b);
                    let Some(at) = canvas.at(x, wy, z) else {
                        continue;
                    };
                    let cell = if let Some(lengths) = &end { end_cell(lengths, a, y, b) } else { template.cell(a, y, b) };

                    match cell {
                        Cell::Keep => {}
                        Cell::Air => canvas.blocks[at] = canvas.air,
                        Cell::Block(index) => canvas.blocks[at] = states[index as usize],
                    }
                }
            }
        }

        // Сундук в повороте — в каждом третьем (замер: 32 %).
        if matches!(piece.kind, Kind::TurnLeft | Kind::TurnRight) {
            let a = if piece.kind == Kind::TurnLeft { 3 } else { 1 };
            let (x, y, z) = piece.world(a, 2, 3);

            if let Some(at) = canvas.at(x, y, z) {
                canvas.blocks[at] = if piece.roll % 3 == 0 {
                    if piece.kind == Kind::TurnLeft {
                        templates.chest[piece.facing as usize]
                    } else {
                        templates.chest_turn_right[piece.facing as usize]
                    }
                } else {
                    canvas.air
                };
            }
        }

        // Опоры: кирпич от низа куска вниз, пока пусто или лава.
        let supports = if canvas.supports { template.supports.as_slice() } else { &[] };

        for &(a, b) in supports {
            let (x, y, z) = piece.world(a, 0, b);
            let Some(bottom) = canvas.at(x, y, z) else {
                continue;
            };

            if canvas.blocks[bottom] != templates.bricks {
                continue;
            }

            for down in (1..y).rev() {
                let Some(at) = canvas.at(x, down, z) else {
                    break;
                };

                if !(canvas.passable)(canvas.blocks[at]) {
                    break;
                }

                canvas.blocks[at] = templates.bricks;
            }
        }
    }
}

/// Обрыв моста: в каждом ряду (a, y) кирпич идёт от входа на случайную
/// длину. Замер: в двух нижних слоях 1–3 блока, в третьем 1–5, в четвёртом
/// и пятом (и по краям шестого) 1–8; выше — рельеф.
fn end_lengths(seed: u64) -> [[i32; 6]; 5] {
    let mut random = super::nether::LegacyRandom::new(seed as i32 as i64);
    let mut lengths = [[0; 6]; 5];
    // Порядок рядов проверен по Seed и блокам 47 обрывов из регионов 26.3.
    for column in &mut lengths {
        for length in &mut column[3..=4] { *length = 1 + random.below(8); }
    }
    for a in [0, 4] { lengths[a][5] = 1 + random.below(8); }
    for column in &mut lengths { column[2] = 1 + random.below(5); }
    for column in &mut lengths {
        for length in &mut column[..=1] { *length = 1 + random.below(3); }
    }
    lengths
}

fn end_cell(lengths: &[[i32; 6]; 5], a: i32, y: i32, b: i32) -> Cell {
    if y < 6 && b < lengths[a as usize][y as usize] { Cell::Block(0) } else { Cell::Keep }
}

/// Кости: поток чисел из семени и места.
pub(super) struct Dice(u64);

impl Dice {
    pub(super) fn new(seed: i64, salt: u64, x: i32, z: i32) -> Dice {
        Dice(scramble(
            (seed as u64)
                ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93)
                ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9),
        ))
    }

    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        scramble(self.0)
    }

    pub(super) fn below(&mut self, bound: u32) -> i32 {
        (self.next() % u64::from(bound.max(1))) as i32
    }

    pub(super) fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub(super) fn scramble(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^ (value >> 33)
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    /// Отпечаток всех 479 рамок не позволяет заменить точную фазу средней частотой.
    #[test]
    fn census_263_fortress_layout_stream() {
        use sha2::{Digest, Sha256};
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/nether-pass3-fortresses.json"
        )).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let seed = case["seed"].as_i64().unwrap();
            let x = case["chunk"][0].as_i64().unwrap() as i32;
            let z = case["chunk"][1].as_i64().unwrap() as i32;
            let pieces = plan(seed, x, z);
            assert_eq!(pieces.len(), case["pieces"].as_u64().unwrap() as usize);
            let mut hash = Sha256::new();
            for piece in &pieces {
                let bounds = piece.bounds.map(|v| v.to_string()).join(",");
                hash.update(format!("{} {} {} {}\n", piece.kind.id(), piece.facing, bounds, piece.depth));
            }
            assert_eq!(format!("{:x}", hash.finalize()), case["layout_sha256"].as_str().unwrap(), "{seed} {x}/{z}");
        }
        for end in fixture["bridge_ends"].as_array().unwrap() {
            let actual = end_lengths(end["seed"].as_i64().unwrap() as u64);
            let expected: [[i32; 6]; 5] = serde_json::from_value(end["row_lengths"].clone()).unwrap();
            assert_eq!(actual, expected);
        }
    }

    /// Блоки настоящих чанков внутри рамок проверяют высоту, поворот и образцы.
    /// Опоры вне рамок зависят также от пока неточных узких ходов рельефа.
    #[test]
    fn census_263_fortress_generated_materials() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/nether-pass3-fortresses.json"
        )).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let seed = case["seed"].as_i64().unwrap();
            let start_x = case["chunk"][0].as_i64().unwrap() as i32;
            let start_z = case["chunk"][1].as_i64().unwrap() as i32;
            let pieces = plan(seed, start_x, start_z);
            let generator = crate::world::Generator::Nether(super::super::nether::Nether::new(seed));
            let names = ["nether_bricks", "nether_brick_fence", "nether_brick_stairs"];
            let mut counts = [0usize; 3];
            for pair in case["chunk_sample"].as_array().unwrap() {
                let (cx, cz) = (pair[0].as_i64().unwrap() as i32, pair[1].as_i64().unwrap() as i32);
                let chunk = crate::world::Chunk::generated(&generator, cx, cz);
                for y in 0..128 { for z in 0..16 { for x in 0..16 {
                    if !pieces.iter().any(|p| (p.bounds[0]..=p.bounds[3]).contains(&(cx*16+x))
                        && (p.bounds[1]..=p.bounds[4]).contains(&y)
                        && (p.bounds[2]..=p.bounds[5]).contains(&(cz*16+z))) { continue; }
                    let name = blocks::block_at_state(chunk.block(x, y, z));
                    for (i, expected) in names.iter().enumerate() {
                        counts[i] += usize::from(name == Some(*expected));
                    }
                } } }
            }
            for (i, name) in names.iter().enumerate() {
                let expected = case["sample_piece_box_blocks"][name].as_u64().unwrap_or(0) as usize;
                assert!(counts[i].abs_diff(expected) <= (expected / 5).max(8),
                    "{seed} {} {name}: {} / {expected}", case["chunk"], counts[i]);
            }
        }
    }

    /// Все образцы разбираются и совпадают по размеру с замером.
    #[test]
    fn templates_parse() {
        let templates = templates();

        assert_eq!(templates.size(Kind::Crossing), (19, 19, 10));
        assert_eq!(templates.size(Kind::Entrance), (13, 13, 14));
        assert_eq!(templates.size(Kind::Corridor), (5, 5, 7));
    }

    /// Одно семя — одна раскладка.
    #[test]
    fn plan_is_decided_by_seed() {
        let one = plan(5, 21, 9);
        let two = plan(5, 21, 9);

        assert_eq!(one.len(), two.len());
        assert!(one.iter().zip(&two).all(|(a, b)| a.kind == b.kind && a.bounds == b.bounds && a.facing == b.facing));
        assert!(one.len() > 5);
    }

    /// Куски не заходят друг в друга, первый — перекрёсток серединой
    /// в (11, 11) чанка старта.
    #[test]
    fn pieces_do_not_overlap() {
        for seed in 0..20 {
            let pieces = plan(seed, -3, 4);
            let (x, _, z) = pieces[0].world(9, 0, 9);

            assert_eq!((x, z), (-3 * 16 + 11, 4 * 16 + 11));
            for (at, one) in pieces.iter().enumerate() {
                assert!(pieces[at + 1..].iter().all(|other| !one.touches(&other.bounds)));
            }
        }
    }

    /// Числа кусков на крепость — против замера оригинала (40 крепостей,
    /// tools/measure/nether_fortress.py stats): средние сходятся.
    #[test]
    fn fortress_piece_counts() {
        const MEASURED: [(Kind, f64); 14] = [
            (Kind::Crossing, 4.90),
            (Kind::BridgeEnd, 12.53),
            (Kind::Bridge, 19.07),
            (Kind::CorridorStairs, 2.65),
            (Kind::Entrance, 1.00),
            (Kind::StalkRoom, 1.55),
            (Kind::Balcony, 1.73),
            (Kind::Throne, 2.00),
            (Kind::RoomCrossing, 3.90),
            (Kind::Corridor, 43.17),
            (Kind::TurnLeft, 7.42),
            (Kind::TurnRight, 7.17),
            (Kind::CorridorCrossing, 4.40),
            (Kind::StairsRoom, 2.92),
        ];
        let count = 400;
        let mut sums = [0usize; KINDS.len()];

        for at in 0..count {
            for piece in plan(100_554_032_945_340, at * 27, at % 7) {
                sums[piece.kind.at()] += 1;
            }
        }

        for (kind, measured) in MEASURED {
            let ours = sums[kind.at()] as f64 / count as f64;
            println!("{:<7} {:6.2} / {:6.2}", kind.id(), measured, ours);
            assert!((ours - measured).abs() <= 0.2 * measured + 0.3, "{}: {:.2} против {:.2}", kind.id(), ours, measured);
        }
    }
}
