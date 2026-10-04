// Крепости (strongholds) обычного мира: где стоят, из каких кусков
// складываются и чем кладутся.
//
// Где — по вики (Stronghold): 128 крепостей в 8 кольцах вокруг середины
// мира (3, 6, 10, 15, 21, 28, 36 и 9), кольцо k — от 1280 + 3072·k до
// 2816 + 3072·k блоков; в кольце они стоят «at roughly equal angles», а
// потом сдвигаются к суше («biased towards biomes in #stronghold_biased_to
// by searching horizontally»). Замер оригинала (tools/measure/stronghold.py
// locate, 19 крепостей трёх колец): углы в кольце через равные доли круга
// с общим случайным поворотом, расстояния — в тех же пределах.
//
// Как — дерево кусков, снятое с 19 крепостей оригинала (записи о строении
// в чанке старта: вид, рамка, сторона, глубина и поля куска). Первый —
// винтовая лестница (shstart) углом в клетке (2, 2) чанка старта, за ней
// всегда перекрёсток на пять сторон (sh5c). У каждого куска свои выходы
// (замер: всегда одни и те же, у поворотов — по зеркальности рамки); к
// выходу пристраивается кусок по весам, пока не кончились ограниченные
// виды; не встаёт ничего — короткий проход-заглушка (shfc). Портальная
// комната — не ближе шестого куска по глубине (замер: 6..10), одна; нет её
// в раскладке — раскладка бросается заново.
//
// Чем — образцы src/world/stronghold.txt с тех же 19 крепостей: «#» —
// камень крепости (замер: 48 % обычный, 28 % замшелый, 19 % треснутый, 5 %
// с чешуйницей), «.» — рельеф остаётся, «_» — пусто. Двери входа и боковые
// проёмы кладутся отдельно (замер дверей по 4 видам). Рамки портала — с
// оком в 10 % каждая (вики: «Each portal frame has a 10% chance»).
//
// Раскладка зависит только от семени и чанка старта, поэтому каждый чанк
// сам строит ближние крепости целиком и кладёт только то, что в него попало.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::nether_fortress::{orient, scramble, Dice};
use crate::blocks;

/// Хранилище по семени (и чанку старта): считается один раз.
type Cache<K, V> = OnceLock<Mutex<HashMap<K, Arc<V>>>>;

/// Сколько крепостей в каждом кольце (вики).
pub const RINGS: [u32; 8] = [3, 6, 10, 15, 21, 28, 36, 9];

/// Середина первого кольца и шаг колец в чанках, полуширина кольца (вики:
/// 1280–2816 блоков у первого, каждое следующее на 3072 дальше).
const RING_MIDDLE: f64 = 128.0;
const RING_STEP: f64 = 192.0;
const RING_HALF: f64 = 48.0;

/// Как далеко (в чанках) старт ищет сушу вокруг точки кольца [проверить:
/// вики говорит только «searching horizontally»].
const LAND_SEARCH: i32 = 7;

/// Дальше этого от середины старта (по большей из осей) кусков нет (вики:
/// «within a horizontal radius of 112 blocks»).
const REACH: i32 = 112;

/// Глубже этого по дереву кусков нет (вики: «up to 50 rooms away»).
const DEPTH: u32 = 50;

/// Сколько раз пробовать другой кусок, если выпавший не встаёт.
const TRIES: usize = 5;

/// На сколько чанков от чанка старта достаёт крепость (замер: размах до
/// 120 блоков от середины старта).
pub const RADIUS_CHUNKS: i32 = 9;

/// Виды кусков. Имена — как в записях о строениях у оригинала.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    /// shstart — винтовая лестница старта.
    Start,
    /// sh5c — перекрёсток на пять сторон.
    FiveWay,
    /// shs — прямой коридор (бывают боковые проёмы).
    Corridor,
    /// shlt — поворот.
    LeftTurn,
    /// shrt — поворот в другую сторону.
    RightTurn,
    /// shrc — большая комната (пустая, с фонтаном, со столбом, кладовая).
    RoomCrossing,
    /// shph — тюремные камеры.
    Prison,
    /// shssd — прямая лестница вниз.
    StraightStairs,
    /// shsd — винтовая лестница вниз.
    SpiralStairs,
    /// shcc — коридор с сундуком.
    ChestCorridor,
    /// shli — библиотека.
    Library,
    /// shpr — портальная комната.
    PortalRoom,
    /// shfc — короткий проход-заглушка.
    Filler,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Start => "shstart",
            Kind::FiveWay => "sh5c",
            Kind::Corridor => "shs",
            Kind::LeftTurn => "shlt",
            Kind::RightTurn => "shrt",
            Kind::RoomCrossing => "shrc",
            Kind::Prison => "shph",
            Kind::StraightStairs => "shssd",
            Kind::SpiralStairs => "shsd",
            Kind::ChestCorridor => "shcc",
            Kind::Library => "shli",
            Kind::PortalRoom => "shpr",
            Kind::Filler => "shfc",
        }
    }

    /// Середина входа по a (замер: у широких кусков вход у пятой клетки).
    fn door_mid(self) -> i32 {
        match self {
            Kind::RoomCrossing | Kind::Library | Kind::PortalRoom | Kind::FiveWay => 5,
            _ => 2,
        }
    }

    /// Пол входа над низом куска (замер: у лестниц вход наверху).
    fn door_floor(self) -> i32 {
        match self {
            Kind::Start | Kind::SpiralStairs | Kind::StraightStairs => 6,
            Kind::FiveWay => 2,
            _ => 0,
        }
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

/// Выход: середина входа следующего куска (a, y от низа, b) в рамке этого,
/// куда он смотрит и какой флаг куска его открывает (0 — всегда).
type Exit = (i32, i32, i32, Turn, u8);

const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const LEFT_HIGH: u8 = 4;
const RIGHT_HIGH: u8 = 8;

/// Выходы по замеру (tools/measure/stronghold.py exits, 19 крепостей).
fn exits(kind: Kind, mirrored: bool) -> &'static [Exit] {
    match (kind, mirrored) {
        (Kind::Start, _) => &[(2, 0, 5, Turn::Ahead, 0)],
        (Kind::FiveWay, _) => &[
            (6, 0, 11, Turn::Ahead, 0),
            (-1, 2, 2, Turn::Minus, LEFT),
            (-1, 4, 8, Turn::Minus, LEFT_HIGH),
            (10, 2, 2, Turn::Plus, RIGHT),
            (10, 4, 8, Turn::Plus, RIGHT_HIGH),
        ],
        (Kind::Corridor, _) => &[(2, 0, 7, Turn::Ahead, 0), (-1, 0, 3, Turn::Minus, LEFT), (5, 0, 3, Turn::Plus, RIGHT)],
        (Kind::LeftTurn, false) | (Kind::RightTurn, true) => &[(5, 0, 2, Turn::Plus, 0)],
        (Kind::LeftTurn, true) | (Kind::RightTurn, false) => &[(-1, 0, 2, Turn::Minus, 0)],
        (Kind::RoomCrossing, _) => &[(-1, 0, 5, Turn::Minus, 0), (5, 0, 11, Turn::Ahead, 0), (11, 0, 5, Turn::Plus, 0)],
        (Kind::Prison, _) => &[(2, 0, 11, Turn::Ahead, 0)],
        (Kind::StraightStairs, _) => &[(2, 0, 8, Turn::Ahead, 0)],
        (Kind::SpiralStairs, _) => &[(2, 0, 5, Turn::Ahead, 0)],
        (Kind::ChestCorridor, _) => &[(2, 0, 7, Turn::Ahead, 0)],
        (Kind::Library, _) | (Kind::PortalRoom, _) | (Kind::Filler, _) => &[],
    }
}

/// Куски по весам: вид, вес, сколько их может быть (0 — без предела) и с
/// какой глубины. Пределы — вики («0–5 spiral staircases», «0–4 five-way
/// crossings», «0–6 large rooms», «0–2 libraries»…) и замер: у оригинала
/// ограниченные виды почти всегда выбраны до предела (19 крепостей:
/// sh5c 4,63 вместе с первым, shrc 5,53, shph 4,58, shcc 3,74). Веса
/// подобраны так, что средние числа кусков сходятся с замером (тест
/// stronghold_piece_counts).
const PIECES: [(Kind, u32, u32, u32); 11] = [
    (Kind::Corridor, 17, 0, 0),
    (Kind::Prison, 5, 5, 0),
    (Kind::LeftTurn, 11, 0, 0),
    (Kind::RightTurn, 11, 0, 0),
    (Kind::RoomCrossing, 10, 6, 0),
    (Kind::StraightStairs, 5, 5, 0),
    (Kind::SpiralStairs, 5, 5, 0),
    (Kind::FiveWay, 5, 5, 0),
    (Kind::ChestCorridor, 5, 4, 0),
    (Kind::Library, 10, 2, 5),
    (Kind::PortalRoom, 20, 1, 6),
];

/// Вид входа (замер, поворотов 578: проём 40 %, деревянная дверь 22 %,
/// железная 18 %, решётка 17 %).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Door {
    Opening,
    Wood,
    Grates,
    Iron,
}

/// Вперёд по сторонам: 0 — юг (+z), 1 — запад, 2 — север, 3 — восток.
const FORWARD: [(i32, i32); 4] = [(0, 1), (-1, 0), (0, -1), (1, 0)];

/// Вбок (+a) по сторонам; у сторон 2 и 3 рамка зеркальна (как у крепостей
/// Незера — замер по несимметричным кускам).
const SIDEWAYS: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 0), (0, 1)];

fn facing_of(direction: (i32, i32)) -> u8 {
    FORWARD.iter().position(|d| *d == direction).unwrap_or(0) as u8
}

/// Поставленный кусок.
#[derive(Clone, Debug)]
pub struct Placed {
    pub kind: Kind,
    pub facing: u8,
    /// Середина входа на полу входа.
    origin: (i32, i32, i32),
    /// x0, y0, z0, x1, y1, z1 — включительно.
    pub bounds: [i32; 6],
    pub depth: u32,
    /// Образец: вид и его поля (как в stronghold.txt).
    template: usize,
    flags: u8,
    door: Door,
    roll: u64,
}

impl Placed {
    fn new(kind: Kind, template: usize, facing: u8, origin: (i32, i32, i32), roll: u64) -> Placed {
        let (width, length, height) = templates().size(template);
        let mut placed = Placed { kind, facing, origin, bounds: [0; 6], depth: 0, template, flags: 0, door: Door::Opening, roll };
        let (x0, y0, z0) = placed.world(0, 0, 0);
        let (x1, y1, z1) = placed.world(width - 1, height - 1, length - 1);

        placed.bounds = [x0.min(x1), y0, z0.min(z1), x0.max(x1), y1, z0.max(z1)];
        placed
    }

    /// Клетка (a, y от низа, b) рамки куска в мире.
    fn world(&self, a: i32, y: i32, b: i32) -> (i32, i32, i32) {
        let (fx, fz) = FORWARD[self.facing as usize];
        let (lx, lz) = SIDEWAYS[self.facing as usize];
        let side = a - self.kind.door_mid();

        (self.origin.0 + lx * side + fx * b, self.origin.1 - self.kind.door_floor() + y, self.origin.2 + lz * side + fz * b)
    }

    fn touches(&self, other: &[i32; 6]) -> bool {
        let a = &self.bounds;

        a[0] <= other[3] && other[0] <= a[3] && a[1] <= other[4] && other[1] <= a[4] && a[2] <= other[5] && other[2] <= a[5]
    }

    fn mirrored(&self) -> bool {
        self.facing >= 2
    }
}

/// Номер образца для вида куска (у поворотов — по зеркальности, у комнат —
/// по типу, у библиотеки — высокая или низкая, у заглушки — по длине).
fn template_for(kind: Kind, facing: u8, variant: u32) -> usize {
    let key = match kind {
        Kind::LeftTurn | Kind::RightTurn => format!("{} m={}", kind.id(), u8::from(facing >= 2)),
        Kind::RoomCrossing => format!("{} Type={}", kind.id(), variant),
        Kind::Library => format!("{} Tall={}", kind.id(), variant),
        Kind::Filler => format!("{} Steps={}", kind.id(), variant),
        _ => kind.id().to_string(),
    };

    templates().index(&key)
}

/// Точки колец в чанках до сдвига к суше: на равных долях круга внутри
/// кольца, со случайным поворотом и расстоянием в пределах кольца.
pub fn ring_points(seed: i64) -> Vec<(i32, i32)> {
    let mut dice = Dice::new(seed, 0x5_7801, 0, 0);
    let mut out = Vec::with_capacity(128);

    for (ring, &count) in RINGS.iter().enumerate() {
        let turn = dice.unit() * std::f64::consts::TAU;

        for at in 0..count {
            let angle = turn + std::f64::consts::TAU * f64::from(at) / f64::from(count);
            let distance = RING_MIDDLE + RING_STEP * ring as f64 + (dice.unit() * 2.0 - 1.0) * RING_HALF;

            out.push(((angle.cos() * distance).round() as i32, (angle.sin() * distance).round() as i32));
        }
    }

    out
}

/// Чанки стартов всех 128 крепостей мира: точки колец, сдвинутые к
/// ближайшему чанку суши (`land` — суша ли под серединой чанка).
/// Считаются один раз на семя.
pub fn starts(seed: i64, land: &dyn Fn(i32, i32) -> bool) -> Arc<Vec<(i32, i32)>> {
    static CACHE: Cache<i64, Vec<(i32, i32)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&seed) {
        return found.clone();
    }

    let mut out = Vec::with_capacity(128);

    for (x, z) in ring_points(seed) {
        let mut best = (x, z);

        'search: for radius in 0..=LAND_SEARCH {
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    if dx.abs() != radius && dz.abs() != radius {
                        continue;
                    }

                    if land(x + dx, z + dz) {
                        best = (x + dx, z + dz);
                        break 'search;
                    }
                }
            }
        }

        out.push(best);
    }

    let out = Arc::new(out);
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(seed, out.clone());
    out
}

/// Раскладка крепости со стартом в этом чанке. Нет портальной комнаты —
/// раскладка бросается заново (вики: «the only room guaranteed to
/// generate»).
pub fn plan(seed: i64, chunk_x: i32, chunk_z: i32) -> Vec<Placed> {
    let mut last = Vec::new();

    for attempt in 0..16 {
        let pieces = plan_once(seed, chunk_x, chunk_z, attempt);

        if pieces.iter().any(|piece| piece.kind == Kind::PortalRoom) {
            return pieces;
        }

        last = pieces;
    }

    last
}

fn plan_once(seed: i64, chunk_x: i32, chunk_z: i32, attempt: u64) -> Vec<Placed> {
    let mut dice = Dice::new(seed ^ attempt as i64, 0x57_0E6, chunk_x, chunk_z);
    let facing = dice.below(4) as u8;
    // Низ старта: замер 19 крепостей — от −48 до 36 [проверить закон].
    let low = -48 + dice.below(85);
    let template = template_for(Kind::Start, facing, 0);
    // Угол рамки старта — в клетке (2, 2) чанка (замер: у всех 19).
    let corner = |placed: &Placed| (placed.bounds[0], placed.bounds[2]);
    let mut start = Placed::new(Kind::Start, template, facing, (0, low + Kind::Start.door_floor(), 0), dice.next());
    let (cx, cz) = corner(&start);
    let shift = (chunk_x * 16 + 2 - cx, chunk_z * 16 + 2 - cz);
    start = Placed::new(Kind::Start, template, facing, (shift.0, low + Kind::Start.door_floor(), shift.1), start.roll);
    let (center_x, center_z) = ((start.bounds[0] + start.bounds[3]) / 2, (start.bounds[2] + start.bounds[5]) / 2);

    let mut pieces = vec![start];
    let mut pending = vec![0usize];
    let mut counts = [0u32; PIECES.len()];

    while !pending.is_empty() {
        let parent = pieces[pending.swap_remove(dice.below(pending.len() as u32) as usize)].clone();

        for &(a, y, b, turn, flag) in exits(parent.kind, parent.mirrored()) {
            if flag != 0 && parent.flags & flag == 0 {
                continue;
            }

            let (x, level, z) = parent.world(a, y, b);

            if (x - center_x).abs().max((z - center_z).abs()) > REACH || parent.depth >= DEPTH {
                continue;
            }

            let (fx, fz) = FORWARD[parent.facing as usize];
            let (lx, lz) = SIDEWAYS[parent.facing as usize];
            let direction = match turn {
                Turn::Ahead => (fx, fz),
                Turn::Plus => (lx, lz),
                Turn::Minus => (-lx, -lz),
            };
            let facing = facing_of(direction);
            let depth = parent.depth + 1;
            let mut placed = None;
            // За стартом — всегда перекрёсток на пять сторон (замер: у всех 19).
            let forced = parent.kind == Kind::Start;

            // Кончились все ограниченные виды — дальше ничего не растёт
            // (как у крепостей Незера; замер: ограниченные выбраны почти
            // до предела, а коридоров лишь ~40).
            let room = PIECES.iter().zip(&counts).any(|(&(_, _, cap, _), &count)| cap > 0 && count < cap);

            for _ in 0..if room { TRIES } else { 0 } {
                let free: Vec<usize> = (0..PIECES.len())
                    .filter(|&at| {
                        let (kind, _, cap, from) = PIECES[at];
                        (cap == 0 || counts[at] < cap) && depth >= from && (!forced || kind == Kind::FiveWay)
                    })
                    .collect();
                let total: u32 = free.iter().map(|&at| PIECES[at].1).sum();

                if total == 0 {
                    break;
                }

                let mut pick = dice.below(total) as u32;
                let Some(&at) = free.iter().find(|&&at| {
                    let hit = pick < PIECES[at].1;
                    pick = pick.saturating_sub(PIECES[at].1);
                    hit
                }) else {
                    break;
                };
                let kind = PIECES[at].0;
                let variant = match kind {
                    Kind::RoomCrossing => dice.below(5) as u32,
                    _ => 1,
                };
                let mut candidate = Placed::new(kind, template_for(kind, facing, variant), facing, (x, level, z), dice.next());

                // Библиотека низкая, если высокой не хватает места (замер: 2
                // низких из 38) [проверить].
                if kind == Kind::Library && pieces.iter().any(|other| candidate.touches(&other.bounds)) {
                    candidate = Placed::new(kind, template_for(kind, facing, 0), facing, (x, level, z), candidate.roll);
                }

                if candidate.bounds[1] > super::MIN_Y + 10 && !pieces.iter().any(|other| candidate.touches(&other.bounds)) {
                    placed = Some((at, candidate));
                    break;
                }
            }

            let child = match placed {
                Some((at, child)) => {
                    counts[at] += 1;
                    Some(child)
                }
                // Не встало ничего — проход-заглушка до соседнего куска, если
                // тот не дальше 5 блоков (замер: длины только 2 и 3).
                None => [3, 2].into_iter().find_map(|steps| {
                    let filler = Placed::new(Kind::Filler, template_for(Kind::Filler, facing, steps), facing, (x, level, z), 0);
                    let ahead = Placed::new(Kind::Filler, filler.template, facing, (x + 2 * direction.0, level, z + 2 * direction.1), 0);
                    let blocked = |piece: &Placed| pieces.iter().any(|other| piece.touches(&other.bounds));
                    (!blocked(&filler) && blocked(&ahead)).then_some(filler)
                }),
            };

            if let Some(mut child) = child {
                child.depth = depth;
                child.roll = dice.next();
                // Двери: у старта, портальной комнаты и заглушки — проём.
                child.door = match child.kind {
                    Kind::Filler | Kind::PortalRoom => Door::Opening,
                    _ => match dice.below(5) {
                        0 | 1 => Door::Opening,
                        2 => Door::Wood,
                        3 => Door::Grates,
                        _ => Door::Iron,
                    },
                };
                // Боковые выходы — каждый в половине случаев (замер shs:
                // слева 43 %, справа 48 %) [проверить у sh5c].
                child.flags = (dice.next() & 0xF) as u8;
                pieces.push(child);
                pending.push(pieces.len() - 1);
            }
        }
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
    /// Камень крепости — вид по случаю.
    Brick,
    /// Блок: номер в палитре куска.
    Block(u16),
}

struct Template {
    key: String,
    width: i32,
    length: i32,
    height: i32,
    cells: Vec<Cell>,
    /// Палитра по сторонам и её тексты (по тексту ищутся рамки портала).
    states: [Vec<i32>; 4],
    texts: Vec<String>,
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> Cell {
        self.cells[((y * self.length + b) * self.width + a) as usize]
    }
}

struct Templates {
    pieces: Vec<Template>,
    bricks: [i32; 4],
    air: i32,
    /// Двери по видам и сторонам: клетки (смещение вбок −1..=1, y 1..=3).
    doors: HashMap<(u8, u8), [[i32; 3]; 3]>,
    button: [i32; 4],
    frame_eye: HashMap<i32, i32>,
}

impl Templates {
    fn size(&self, at: usize) -> (i32, i32, i32) {
        let template = &self.pieces[at];
        (template.width, template.length, template.height)
    }

    fn index(&self, key: &str) -> usize {
        self.pieces.iter().position(|template| template.key == key).unwrap_or_else(|| panic!("образец {} описан", key))
    }
}

fn templates() -> &'static Templates {
    static TEMPLATES: OnceLock<Templates> = OnceLock::new();

    TEMPLATES.get_or_init(|| parse(include_str!("stronghold.txt")))
}

fn named(text: &str) -> i32 {
    blocks::state_from_text(text).unwrap_or_else(|| panic!("блок {} есть в таблице", text))
}

fn parse(text: &str) -> Templates {
    let mut pieces = Vec::new();
    let mut lines = text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).peekable();

    while let Some(line) = lines.next() {
        let words: Vec<&str> = line.split_whitespace().collect();

        if words.first() != Some(&"piece") {
            continue;
        }

        let number = |at: usize| words[at].parse::<i32>().expect("число в заголовке куска");
        let (width, length, height) = (number(2), number(3), number(4));
        let key = std::iter::once(words[1]).chain(words[6..].iter().copied()).collect::<Vec<_>>().join(" ");
        let mut palette: Vec<(char, String)> = Vec::new();
        let mut cells = Vec::with_capacity((width * length * height) as usize);

        while let Some(line) = lines.next_if(|line| !line.starts_with("piece")) {
            let trimmed = line.trim();

            if trimmed.starts_with("layer") {
                for _ in 0..length {
                    let row = lines.next().expect("строка слоя").trim();

                    for symbol in row.chars() {
                        cells.push(match symbol {
                            '.' => Cell::Keep,
                            '_' => Cell::Air,
                            '#' => Cell::Brick,
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

        assert_eq!(cells.len(), (width * length * height) as usize, "размер куска {}", key);

        let states = [0u8, 1, 2, 3].map(|facing| palette.iter().map(|(_, state)| named(&orient(state, facing))).collect());
        let texts = palette.into_iter().map(|(_, state)| state).collect();

        pieces.push(Template { key, width, length, height, cells, states, texts });
    }

    // Двери входа — замер (tools/measure/stronghold.py, 4 вида): рамка из
    // обычного камня, дверь посередине; решётка — прутья с проходом 1×2.
    let mut doors = HashMap::new();

    for facing in 0..4u8 {
        let state = |text: &str| named(&orient(text, facing));
        let brick = state("stone_bricks");
        let air = state("cave_air");
        let door = |name: &str| {
            [
                [brick, brick, brick],
                [
                    state(&format!("{}[facing=south,half=lower,hinge=left,open=false,powered=false]", name)),
                    state(&format!("{}[facing=south,half=upper,hinge=left,open=false,powered=false]", name)),
                    brick,
                ],
                [brick, brick, brick],
            ]
        };
        let bars = |east: bool, west: bool| state(&format!("iron_bars[east={},north=false,south=false,waterlogged=false,west={}]", east, west));

        doors.insert((Door::Opening as u8, facing), [[air; 3]; 3]);
        doors.insert((Door::Wood as u8, facing), door("oak_door"));
        doors.insert((Door::Iron as u8, facing), door("iron_door"));
        doors.insert(
            (Door::Grates as u8, facing),
            [[bars(false, true), bars(false, true), bars(true, true)], [air, air, bars(true, true)], [bars(true, false), bars(true, false), bars(true, true)]],
        );
    }

    let mut frame_eye = HashMap::new();

    for facing in ["north", "south", "east", "west"] {
        frame_eye.insert(
            named(&format!("end_portal_frame[eye=false,facing={}]", facing)),
            named(&format!("end_portal_frame[eye=true,facing={}]", facing)),
        );
    }

    Templates {
        pieces,
        bricks: [named("stone_bricks"), named("mossy_stone_bricks"), named("cracked_stone_bricks"), named("infested_stone_bricks")],
        air: named("cave_air"),
        doors,
        button: [0u8, 1, 2, 3].map(|facing| named(&orient("stone_button[face=wall,facing=north,powered=false]", facing))),
        frame_eye,
    }
}

/// Камень крепости в этой клетке (замер 19 крепостей: 48,2 % обычный,
/// 28,4 % замшелый, 18,8 % треснутый, 4,7 % с чешуйницей).
fn brick(seed: i64, x: i32, y: i32, z: i32) -> i32 {
    let roll = scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9) ^ (y as u64).wrapping_mul(0x85EB_CA6B) ^ (z as u64).wrapping_mul(0xC2B2_AE35) ^ 0x5B);
    let unit = (roll >> 11) as f64 / (1u64 << 53) as f64;
    let bricks = &templates().bricks;

    if unit < 0.482 {
        bricks[0]
    } else if unit < 0.766 {
        bricks[1]
    } else if unit < 0.953 {
        bricks[2]
    } else {
        bricks[3]
    }
}

/// Кладёт куски крепости в чанк: `put(x, y, z, state)` — мировые
/// координаты; клетки вне чанка `put` отбрасывает сам.
pub fn paint(seed: i64, pieces: &[Placed], chunk: [i32; 4], put: &mut dyn FnMut(i32, i32, i32, i32)) {
    let templates = templates();
    let inside = |x: i32, z: i32| x >= chunk[0] && x <= chunk[2] && z >= chunk[1] && z <= chunk[3];

    for piece in pieces {
        let b = &piece.bounds;

        // Кусок и клетки перед его входом (дверь соседа ставится им).
        if b[0] - 1 > chunk[2] || chunk[0] > b[3] + 1 || b[2] - 1 > chunk[3] || chunk[1] > b[5] + 1 {
            continue;
        }

        let template = &templates.pieces[piece.template];
        let states = &template.states[piece.facing as usize];

        for y in 0..template.height {
            for bb in 0..template.length {
                for a in 0..template.width {
                    let (x, wy, z) = piece.world(a, y, bb);

                    if !inside(x, z) {
                        continue;
                    }

                    match template.cell(a, y, bb) {
                        Cell::Keep => {}
                        Cell::Air => put(x, wy, z, templates.air),
                        Cell::Brick => put(x, wy, z, brick(seed, x, wy, z)),
                        Cell::Block(index) => {
                            let mut state = states[index as usize];

                            // Рамка портала: око — в 10 % (вики).
                            if let Some(&eye) = templates.frame_eye.get(&state)
                                && scramble(piece.roll ^ ((a as u64) << 16 | (bb as u64) << 8 | y as u64)).is_multiple_of(10)
                            {
                                state = eye;
                            }

                            put(x, wy, z, state);
                        }
                    }
                }
            }
        }

        // Боковые проёмы по флагам: 3×3 в стене против выхода.
        for &(_, y, bb, turn, flag) in exits(piece.kind, piece.mirrored()) {
            if flag == 0 || piece.flags & flag == 0 {
                continue;
            }

            let wall_a = if turn == Turn::Minus { 0 } else { template.width - 1 };

            for along in -1..=1 {
                for up in 1..=3 {
                    let (x, wy, z) = piece.world(wall_a, y + up, bb + along);

                    if inside(x, z) {
                        put(x, wy, z, templates.air);
                    }
                }
            }
        }

        // Вход: дверь по виду, у железной — кнопка снаружи справа.
        // У старта, портальной комнаты и заглушки вход свой, из образца.
        if !matches!(piece.kind, Kind::Start | Kind::PortalRoom | Kind::Filler) {
            let door = &templates.doors[&(piece.door as u8, piece.facing)];
            let mid = piece.kind.door_mid();
            let floor = piece.kind.door_floor();

            for (column, side) in (-1..=1).enumerate() {
                for up in 1..=3 {
                    let (x, wy, z) = piece.world(mid + side, floor + up, 0);

                    if inside(x, z) {
                        put(x, wy, z, door[column][up as usize - 1]);
                    }
                }
            }

            if piece.door == Door::Iron {
                let (x, wy, z) = piece.world(mid + 1, floor + 2, -1);

                if inside(x, z) {
                    put(x, wy, z, templates.button[piece.facing as usize]);
                }
            }
        }
    }
}

/// Что в куске требует своих данных: сундук с добычей или рассадник.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Special {
    /// Сундук с добычей таблицы: коридор с сундуком — алтарь, кладовая
    /// большой комнаты, библиотека (вики, Stronghold: «Loot»).
    Chest(crate::loot::Table),
    /// Рассадник чешуйниц портальной комнаты.
    Spawner,
}

/// Сундуки и рассадники кусков в пределах чанка `chunk` (мировые x, z).
pub fn specials(pieces: &[Placed], chunk: [i32; 4], found: &mut dyn FnMut((i32, i32, i32), Special)) {
    let templates = templates();

    for piece in pieces {
        let b = &piece.bounds;

        if b[0] > chunk[2] || chunk[0] > b[3] || b[2] > chunk[3] || chunk[1] > b[5] {
            continue;
        }

        let template = &templates.pieces[piece.template];
        let table = match piece.kind {
            Kind::ChestCorridor => crate::loot::Table::StrongholdCorridor,
            Kind::RoomCrossing => crate::loot::Table::StrongholdCrossing,
            Kind::Library => crate::loot::Table::StrongholdLibrary,
            Kind::PortalRoom => crate::loot::Table::StrongholdCorridor,
            _ => continue,
        };

        for y in 0..template.height {
            for bb in 0..template.length {
                for a in 0..template.width {
                    let Cell::Block(index) = template.cell(a, y, bb) else {
                        continue;
                    };
                    let text = &template.texts[index as usize];
                    let special = if text.starts_with("chest[") {
                        Special::Chest(table)
                    } else if text == "spawner" {
                        Special::Spawner
                    } else {
                        continue;
                    };
                    let (x, wy, z) = piece.world(a, y, bb);

                    if x >= chunk[0] && x <= chunk[2] && z >= chunk[1] && z <= chunk[3] {
                        found((x, wy, z), special);
                    }
                }
            }
        }
    }
}

/// Суша ли под серединой чанка — туда тянется старт крепости (вики:
/// #stronghold_biased_to — суша; у нас — всё, кроме океанов и рек).
fn land(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> bool {
    use super::terrain::Biome;

    let biome = terrain.column_in(chunk_x * 16 + 8, chunk_z * 16 + 8, None).biome;
    !(biome.is_ocean() || matches!(biome, Biome::River | Biome::FrozenRiver))
}

/// Чанки стартов крепостей мира с этим рельефом.
pub fn starts_of(terrain: &super::terrain::Terrain) -> Arc<Vec<(i32, i32)>> {
    starts(terrain.seed(), &|x, z| land(terrain, x, z))
}

/// Раскладка крепости — одна на старт, хранится, пока мир жив.
fn plan_cached(seed: i64, chunk_x: i32, chunk_z: i32) -> Arc<Vec<Placed>> {
    static CACHE: Cache<(i64, i32, i32), Vec<Placed>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&(seed, chunk_x, chunk_z)) {
        return found.clone();
    }

    let pieces = Arc::new(plan(seed, chunk_x, chunk_z));
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert((seed, chunk_x, chunk_z), pieces.clone());
    pieces
}

/// Кладёт в сложенный чанк обычного мира куски ближних крепостей —
/// после рельефа и пещер (у оригинала крепости — после вырезания пещер).
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let seed = terrain.seed();
    let near: Vec<(i32, i32)> = starts_of(terrain)
        .iter()
        .copied()
        .filter(|(x, z)| (x - chunk_x).abs() <= RADIUS_CHUNKS && (z - chunk_z).abs() <= RADIUS_CHUNKS)
        .collect();
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);

    for (start_x, start_z) in near {
        let pieces = plan_cached(seed, start_x, start_z);

        paint(seed, &pieces, [x0, z0, x0 + 15, z0 + 15], &mut |x, y, z, state| {
            chunk.put_generated(x - x0, y, z - z0, state, true);
        });

        // Сундуки — с добычей, брошенной по месту сундука.
        specials(&pieces, [x0, z0, x0 + 15, z0 + 15], &mut |(x, y, z), special| {
            if let Special::Chest(table) = special {
                let mut dice = crate::loot::Dice::new(scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9) ^ (y as u64).wrapping_mul(0x85EB_CA6B) ^ (z as u64).wrapping_mul(0xC2B2_AE35) ^ 0x1007));
                let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

                chest.slots = crate::loot::fill(table.pools(), chest.slots.len(), &mut dice);
                chunk.put_generated_container(x - x0, y, z - z0, chest);
            }

            // Спавнер крепости — с серебрянкой (вики, Monster Spawner: «silverfish
            // in strongholds»); самой серебрянки у нас ещё нет, вид — для клиента.
            if matches!(special, Special::Spawner) {
                chunk.put_generated_spawner(x - x0, y, z - z0, crate::spawner::Data::of("silverfish"));
            }
        });
    }
}

/// Верх винтовой лестницы старта крепости (куда зовёт око Края).
pub fn entrance_y(seed: i64, chunk_x: i32, chunk_z: i32) -> i32 {
    plan_cached(seed, chunk_x, chunk_z).first().map_or(0, |start| start.bounds[4])
}

/// Ближайший к точке старт крепости (`locate`, око Края): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Option<(i32, i32)> {
    starts_of(terrain)
        .iter()
        .copied()
        .min_by_key(|(x, z)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2))
}

/// Блоки рамок портала крепости и их место — для тестов и замеров.
#[cfg(test)]
fn texts_of(piece: &Placed) -> &'static [String] {
    &templates().pieces[piece.template].texts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все образцы разбираются; размеры — по замеру.
    #[test]
    fn templates_parse() {
        let templates = templates();

        assert_eq!(templates.size(templates.index("shpr")), (11, 16, 8));
        assert_eq!(templates.size(templates.index("sh5c")), (10, 11, 9));
        assert_eq!(templates.size(templates.index("shs")), (5, 7, 5));
        assert_eq!(templates.size(templates.index("shli Tall=1")), (14, 15, 11));
    }

    /// Кольца: 128 точек, первое кольцо — три на равных долях круга, все
    /// в пределах колец вики.
    #[test]
    fn rings_follow_the_wiki() {
        let points = ring_points(1234);
        assert_eq!(points.len(), 128);

        let mut at = 0;
        for (ring, &count) in RINGS.iter().enumerate() {
            let (low, high) = (1280.0 + 3072.0 * ring as f64, 2816.0 + 3072.0 * ring as f64);
            let mut angles = Vec::new();

            for &(x, z) in &points[at..at + count as usize] {
                let distance = f64::from(x * 16).hypot(f64::from(z * 16));
                assert!(distance >= low - 16.0 && distance <= high + 16.0, "кольцо {}: {}", ring, distance);
                angles.push(f64::from(z).atan2(f64::from(x)));
            }

            let step = std::f64::consts::TAU / f64::from(count);
            let gap = (angles[1] - angles[0]).rem_euclid(std::f64::consts::TAU);
            assert!((gap - step).abs() < 0.05, "кольцо {}: шаг {} против {}", ring, gap, step);
            at += count as usize;
        }
    }

    /// Одно семя — одна раскладка; куски не заходят друг в друга; старт
    /// углом в (2, 2) чанка; за стартом — перекрёсток; портальная комната
    /// одна и не ближе шестого куска.
    #[test]
    fn plans_are_sane() {
        for seed in 0..30 {
            let pieces = plan(seed, 40 + seed as i32, -7);
            let again = plan(seed, 40 + seed as i32, -7);

            assert_eq!(pieces.len(), again.len());
            assert_eq!((pieces[0].bounds[0], pieces[0].bounds[2]), ((40 + seed as i32) * 16 + 2, -7 * 16 + 2));
            assert_eq!(pieces[1].kind, Kind::FiveWay);

            for (at, one) in pieces.iter().enumerate() {
                assert!(pieces[at + 1..].iter().all(|other| !one.touches(&other.bounds)), "семя {}", seed);
            }

            let portals: Vec<_> = pieces.iter().filter(|piece| piece.kind == Kind::PortalRoom).collect();
            assert_eq!(portals.len(), 1, "семя {}", seed);
            assert!(portals[0].depth >= 6);
            assert!(texts_of(portals[0]).iter().any(|text| text.starts_with("end_portal_frame")));
        }
    }

    /// Числа кусков на крепость — против замера оригинала (19 крепостей,
    /// tools/measure/stronghold.py stats).
    #[test]
    fn stronghold_piece_counts() {
        const MEASURED: [(Kind, f64); 12] = [
            (Kind::FiveWay, 4.63),
            (Kind::ChestCorridor, 3.74),
            (Kind::Filler, 14.32),
            (Kind::Library, 2.00),
            (Kind::LeftTurn, 30.42),
            (Kind::Prison, 4.58),
            (Kind::PortalRoom, 1.00),
            (Kind::RoomCrossing, 5.53),
            (Kind::RightTurn, 28.84),
            (Kind::Corridor, 41.84),
            (Kind::SpiralStairs, 4.53),
            (Kind::StraightStairs, 4.53),
        ];
        let count = 200;
        let mut sums: HashMap<Kind, usize> = HashMap::new();

        for at in 0..count {
            for piece in plan(1_234_567_890, at * 37, at % 11) {
                *sums.entry(piece.kind).or_default() += 1;
            }
        }

        let mut bad = Vec::new();
        for (kind, measured) in MEASURED {
            let ours = sums.get(&kind).copied().unwrap_or(0) as f64 / count as f64;
            println!("{:<7} {:6.2} / {:6.2}", kind.id(), measured, ours);
            if (ours - measured).abs() > 0.35 * measured + 0.5 {
                bad.push(format!("{}: {:.2} против {:.2}", kind.id(), ours, measured));
            }
        }
        assert!(bad.is_empty(), "{:?}", bad);
    }

    /// Перепись блоков в рамках кусков на крепость — против замера оригинала
    /// (19 крепостей, tools/measure/stronghold.py blocks). Рельеф («.»)
    /// оригинала в счёт не идёт, поэтому сверяются блоки самой крепости.
    #[test]
    fn stronghold_census() {
        const MEASURED: [(&str, f64); 10] = [
            ("stone_bricks", 9774.8),
            ("mossy_stone_bricks", 5864.1),
            ("cracked_stone_bricks", 3878.5),
            ("infested_stone_bricks", 967.9),
            ("bookshelf", 458.4),
            ("iron_bars", 314.0),
            ("iron_door", 68.1),
            ("oak_door", 54.1),
            ("end_portal_frame", 12.0),
            ("spawner", 1.0),
        ];
        let count = 40;
        let mut totals: HashMap<&'static str, f64> = HashMap::new();
        let mut eyes = 0;

        for at in 0..count {
            let seed = 1_234_567_890 + at as i64;
            let pieces = plan(seed, at * 53, -at);
            let mut cells = HashMap::new();

            paint(seed, &pieces, [i32::MIN, i32::MIN, i32::MAX, i32::MAX], &mut |x, y, z, state| {
                if pieces.iter().any(|piece| {
                    let b = &piece.bounds;
                    (b[0]..=b[3]).contains(&x) && (b[1]..=b[4]).contains(&y) && (b[2]..=b[5]).contains(&z)
                }) {
                    cells.insert((x, y, z), state);
                }
            });

            for state in cells.values() {
                let name = blocks::block_at_state(*state).unwrap_or("?");
                *totals.entry(name).or_default() += 1.0;
                if templates().frame_eye.values().any(|eye| eye == state) {
                    eyes += 1;
                }
            }
        }

        let mut bad = Vec::new();
        for (name, measured) in MEASURED {
            let ours = totals.get(name).copied().unwrap_or(0.0) / f64::from(count);
            println!("{:<22} {:8.1} / {:8.1}", name, measured, ours);
            if (ours - measured).abs() > 0.3 * measured + 1.0 {
                bad.push(format!("{}: {:.1} против {:.1}", name, ours, measured));
            }
        }
        // Око в рамке — 10 % (вики; замер: 29 из 228).
        let share = f64::from(eyes) / (12.0 * f64::from(count));
        println!("око в рамке: {:.3}", share);
        assert!((0.04..0.18).contains(&share), "око в {:.3} рамок", share);
        assert!(bad.is_empty(), "{:?}", bad);
    }

    /// Сундуки — в коридоре с сундуком, кладовой и библиотеке (у высокой
    /// два), рассадник — один, в портальной комнате.
    #[test]
    fn chests_and_the_spawner_are_found() {
        let pieces = plan(1_234_567_890, 99, 30);
        let (mut chests, mut spawners) = (0, 0);

        specials(&pieces, [i32::MIN, i32::MIN, i32::MAX, i32::MAX], &mut |_, special| match special {
            Special::Chest(table) => {
                assert!(!table.pools().is_empty());
                chests += 1;
            }
            Special::Spawner => spawners += 1,
        });

        let expected = pieces.iter().map(|piece| match piece.kind {
            Kind::ChestCorridor => 1,
            Kind::Library => if templates().pieces[piece.template].key.ends_with("Tall=1") { 2 } else { 1 },
            Kind::RoomCrossing => usize::from(templates().pieces[piece.template].key.ends_with("Type=2")),
            _ => 0,
        }).sum::<usize>();

        assert_eq!(spawners, 1);
        assert_eq!(chests, expected);
    }

    /// Рамки портала в комнате смотрят внутрь кольца при любой стороне
    /// комнаты — иначе кольцо из 12 очей портал не откроет.
    #[test]
    fn portal_frames_face_the_middle() {
        for facing in 0..4u8 {
            let kind = Kind::PortalRoom;
            let room = Placed::new(kind, template_for(kind, facing, 0), facing, (100, 0, -50), 7);
            let mut frames = Vec::new();

            paint(1, std::slice::from_ref(&room), [i32::MIN, i32::MIN, i32::MAX, i32::MAX], &mut |x, y, z, state| {
                if blocks::block_at_state(state) == Some("end_portal_frame") {
                    frames.push((x, y, z, state));
                }
            });

            assert_eq!(frames.len(), 12, "сторона {}", facing);
            let middle_x = frames.iter().map(|f| f.0).sum::<i32>() as f64 / 12.0;
            let middle_z = frames.iter().map(|f| f.2).sum::<i32>() as f64 / 12.0;

            for &(x, _, z, state) in &frames {
                let text = (["north", "south", "east", "west"]).into_iter().find(|side| {
                    [false, true].iter().any(|&eye| blocks::state_from_text(&format!("end_portal_frame[eye={},facing={}]", eye, side)) == Some(state))
                });
                let (dx, dz) = match text.expect("сторона рамки") {
                    "north" => (0, -1),
                    "south" => (0, 1),
                    "east" => (1, 0),
                    _ => (-1, 0),
                };
                let before = (f64::from(x) - middle_x).abs() + (f64::from(z) - middle_z).abs();
                let after = (f64::from(x + dx) - middle_x).abs() + (f64::from(z + dz) - middle_z).abs();
                assert!(after < before, "сторона {}: рамка ({}, {}) смотрит наружу", facing, x, z);
            }
        }
    }
}
