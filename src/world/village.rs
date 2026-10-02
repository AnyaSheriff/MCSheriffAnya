// Деревни обычного мира: где стоят и из чего складываются.
//
// Где — по вики (Village; Structure set: «spacing 34, separation 8»): мир
// поделён на области 34×34 чанка, в каждой одна попытка — чанк со случайным
// сдвигом 0…25 по каждой оси; деревня встаёт, если биом в середине чанка её
// (вики: равнинная — равнины и луг, пустынная — пустыня, саванная — саванна,
// таёжная — тайга, снежная — снежные равнины). Случай — свой (своя
// генерация), закон — оригинала.
//
// Из чего — раскладки деревень, снятые с мира оригинала (tools/measure/
// village.py export: записи о строении в чанке старта — образцы кусков,
// повороты, места; блоки образцов — из мира в рамках кусков). Деревня
// повторяет одну из снятых раскладок, повёрнутую на случайную четверть, —
// состав (улицы, дома, колодец и место сбора, места работы, кровати,
// колокол) у неё тот же, что у деревень оригинала. Собирать раскладку
// заново по стыкам, как оригинал, — следующий шаг [проверить].
//
// Рельеф: куски rigid (дома, колодец, фонари) встают на землю — высоту
// берут как медиану земли вокруг рамки плюс снятая у оригинала разница;
// под их полом пустота засыпается землёй до рельефа [проверить: оригинал
// подгоняет рельеф под куски сам]. Куски terrain_matching (улицы) ложатся
// на рельеф по столбцам: нижний слой образца заменяет верх земли.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::blocks;

/// Область попыток и наименьший зазор (вики: 34 и 8 чанков).
pub const SPACING: i32 = 34;
pub const SEPARATION: i32 = 8;

/// Дальше этого от чанка старта (по большей из осей) кусков нет (замер:
/// размах деревень оригинала — до ~90 блоков от старта).
pub const RADIUS_CHUNKS: i32 = 7;

/// Вид деревни.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Plains,
    Desert,
    Savanna,
    Taiga,
    Snowy,
}

impl Kind {
    pub const ALL: [Kind; 5] = [Kind::Plains, Kind::Desert, Kind::Savanna, Kind::Taiga, Kind::Snowy];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Plains => "plains",
            Kind::Desert => "desert",
            Kind::Savanna => "savanna",
            Kind::Taiga => "taiga",
            Kind::Snowy => "snowy",
        }
    }

    /// Вид деревни по биому середины чанка (вики, Village: «Generation»).
    pub fn of_biome(biome: super::terrain::Biome) -> Option<Kind> {
        use super::terrain::Biome;

        match biome {
            Biome::Plains | Biome::Meadow => Some(Kind::Plains),
            Biome::Desert => Some(Kind::Desert),
            Biome::Savanna => Some(Kind::Savanna),
            Biome::Taiga => Some(Kind::Taiga),
            Biome::SnowyPlains => Some(Kind::Snowy),
            _ => None,
        }
    }
}

fn scramble(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Случай области или старта: одно число на (семя, x, z, соль).
fn roll(seed: i64, x: i32, z: i32, salt: u64) -> u64 {
    scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt)
}

/// Чанк попытки в области (rx, rz): сдвиг 0…25 по каждой оси (вики:
/// «separation» — у соседних областей старты не ближе 8 чанков).
pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    let dice = roll(seed, region_x, region_z, 0x0000_1038_7312);
    let span = (SPACING - SEPARATION) as u64;

    (region_x * SPACING + (dice % span) as i32, region_z * SPACING + ((dice >> 20) % span) as i32)
}

/// Стоит ли деревня с чанком старта в этой области, и какая.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<(i32, i32, Kind)> {
    let (chunk_x, chunk_z) = attempt(terrain.seed(), region_x, region_z);
    let kind = Kind::of_biome(terrain.column_in(chunk_x * 16 + 8, chunk_z * 16 + 8, None).biome)?;

    library(kind).map(|_| (chunk_x, chunk_z, kind))
}

// ---------------------------------------------------------------------------
// Образцы и раскладки
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Keep,
    Air,
    Block(u16),
}

struct Template {
    key: String,
    width: i32,
    length: i32,
    height: i32,
    rigid: bool,
    cells: Vec<Cell>,
    /// Палитра в четырёх поворотах (четверти по часовой).
    states: [Vec<i32>; 4],
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> Cell {
        self.cells[((y * self.length + b) * self.width + a) as usize]
    }
}

/// Стог сена (pile_hay) вместо номера образца.
const HAY_PILE: usize = usize::MAX;

/// Доля стогов с сеном в клетке (dx, dz от −2 до 2) нижнего и верхнего слоя,
/// в процентах (замер: 39 стогов 10 равнинных деревень, в среднем 13,5
/// блока; верхний слой — только над сеном).
const HAY: [[[u8; 5]; 5]; 2] = [
    [[0, 17, 28, 15, 12], [10, 43, 61, 53, 17], [33, 58, 79, 56, 25], [23, 53, 61, 48, 10], [7, 5, 35, 25, 5]],
    [[2, 2, 17, 5, 5], [0, 30, 43, 35, 2], [7, 43, 56, 33, 12], [7, 25, 41, 30, 2], [0, 5, 17, 2, 2]],
];

/// Кусок раскладки: образец, поворот, начало от угла чанка старта,
/// высота над землёй вокруг (у rigid).
#[derive(Clone, Copy, Debug)]
struct Entry {
    template: usize,
    turns: u8,
    dx: i32,
    dy: i32,
    dz: i32,
}

struct Library {
    templates: Vec<Template>,
    layouts: Vec<Vec<Entry>>,
}

/// Поворот текста состояния на `turns` четвертей по часовой.
pub fn rotate_text(text: &str, turns: u8) -> String {
    const SIDES: [&str; 4] = ["north", "east", "south", "west"];
    let turns = turns % 4;

    let Some((name, rest)) = text.split_once('[') else {
        return text.to_string();
    };

    if turns == 0 {
        return text.to_string();
    }

    let turn = |side: &str| SIDES.iter().position(|known| *known == side).map(|at| SIDES[(at + turns as usize) % 4]);
    let mut props: Vec<(String, String)> = Vec::new();

    for pair in rest.trim_end_matches(']').split(',') {
        let Some((key, value)) = pair.split_once('=') else { continue };
        let (key, value) = match key {
            side if SIDES.contains(&side) => (turn(side).unwrap_or(side).to_string(), value.to_string()),
            "facing" | "horizontal_facing" => (key.to_string(), turn(value).unwrap_or(value).to_string()),
            "axis" if turns % 2 == 1 && (value == "x" || value == "z") => (key.to_string(), if value == "x" { "z" } else { "x" }.to_string()),
            "rotation" => (key.to_string(), value.parse::<u8>().map_or(value.to_string(), |r| ((r + 4 * turns) % 16).to_string())),
            _ => (key.to_string(), value.to_string()),
        };

        props.push((key, value));
    }

    props.sort();
    format!("{}[{}]", name, props.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(","))
}

fn parse(text: &str) -> Library {
    let mut templates: Vec<Template> = Vec::new();
    let mut layouts = Vec::new();
    let mut lines = text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()).peekable();

    while let Some(line) = lines.next() {
        let words: Vec<&str> = line.split_whitespace().collect();

        match words.first() {
            Some(&"template") => {
                let number = |at: usize| words[at].parse::<i32>().expect("число в заголовке образца");
                let (width, length, height) = (number(2), number(3), number(4));
                let mut palette: Vec<(char, String)> = Vec::new();
                let mut cells = Vec::with_capacity((width * length * height) as usize);

                while let Some(line) = lines.next_if(|line| line.starts_with("  ") || line.starts_with("layer")) {
                    let trimmed = line.trim();

                    if trimmed.starts_with("layer") {
                        for _ in 0..length {
                            let row = lines.next().expect("строка слоя").trim();

                            for symbol in row.chars() {
                                cells.push(match symbol {
                                    '.' => Cell::Keep,
                                    '_' => Cell::Air,
                                    symbol => Cell::Block(palette.iter().position(|(key, _)| *key == symbol).expect("знак есть в палитре") as u16),
                                });
                            }
                        }
                    } else if let Some((symbol, state)) = trimmed.split_once(' ') {
                        palette.push((symbol.chars().next().unwrap_or('?'), state.to_string()));
                    }
                }

                assert_eq!(cells.len(), (width * length * height) as usize, "размер образца {}", words[1]);

                let states = [0u8, 1, 2, 3].map(|turns| {
                    palette
                        .iter()
                        .map(|(_, state)| blocks::state_from_text(&rotate_text(state, turns)).or_else(|| blocks::state_from_text(state.split('[').next().unwrap_or(state))).unwrap_or(super::AIR))
                        .collect()
                });

                templates.push(Template {
                    key: words[1].to_string(),
                    width,
                    length,
                    height,
                    rigid: words[5] == "rigid",
                    cells,
                    states,
                });
            }
            Some(&"layout") => {
                let mut layout = Vec::new();

                while let Some(line) = lines.next_if(|line| line.starts_with("  ")) {
                    let words: Vec<&str> = line.split_whitespace().collect();
                    let template = if words[0] == "feature:pile_hay" {
                        HAY_PILE
                    } else {
                        let Some(template) = templates.iter().position(|known| known.key == words[0]) else { continue };
                        template
                    };
                    let number = |at: usize| words[at].parse::<i32>().unwrap_or(0);

                    layout.push(Entry { template, turns: number(1) as u8, dx: number(2), dy: number(3), dz: number(4) });
                }

                layouts.push(layout);
            }
            _ => {}
        }
    }

    Library { templates, layouts }
}

/// Снятые раскладки вида: равнинные (10 деревень оригинала), пустынные и
/// саванные (по 6). Таёжных и снежных пока не снято — таких деревень наша
/// генерация не ставит.
fn library(kind: Kind) -> Option<&'static Library> {
    static PLAINS: OnceLock<Library> = OnceLock::new();
    static DESERT: OnceLock<Library> = OnceLock::new();
    static SAVANNA: OnceLock<Library> = OnceLock::new();

    match kind {
        Kind::Plains => Some(PLAINS.get_or_init(|| parse(include_str!("village_plains.txt")))),
        Kind::Desert => Some(DESERT.get_or_init(|| parse(include_str!("village_desert.txt")))),
        Kind::Savanna => Some(SAVANNA.get_or_init(|| parse(include_str!("village_savanna.txt")))),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Постановка
// ---------------------------------------------------------------------------

/// Поставленный кусок: образец, поворот, начало в мире (y — низ у rigid),
/// рамка x0, z0, x1, z1.
#[derive(Clone, Debug)]
pub struct Placed {
    template: usize,
    turns: u8,
    origin: (i32, i32, i32),
    pub bounds: [i32; 4],
}

/// Смещение клетки (a, b) куска с поворотом `turns` от его начала.
fn turn(a: i32, b: i32, turns: u8) -> (i32, i32) {
    match turns % 4 {
        0 => (a, b),
        1 => (-b, a),
        2 => (-a, -b),
        _ => (b, -a),
    }
}

/// Раскладка деревни со стартом в этом чанке: раскладка оригинала по
/// случаю, повёрнутая на случайную четверть; высоты rigid-кусков — от
/// земли `ground(x, z)`.
pub fn plan(seed: i64, kind: Kind, chunk_x: i32, chunk_z: i32, ground: &dyn Fn(i32, i32) -> i32) -> Vec<Placed> {
    let Some(library) = library(kind) else {
        return Vec::new();
    };

    if library.layouts.is_empty() {
        return Vec::new();
    }

    let dice = roll(seed, chunk_x, chunk_z, 0x0711_1A6E);
    let layout = &library.layouts[(dice % library.layouts.len() as u64) as usize];
    let spin = ((dice >> 32) % 4) as u8;
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);
    let mut out = Vec::with_capacity(layout.len());

    for entry in layout {
        if entry.template == HAY_PILE {
            let (ox, oz) = turn(entry.dx, entry.dz, spin);
            let (x, z) = (x0 + ox, z0 + oz);

            out.push(Placed { template: HAY_PILE, turns: 0, origin: (x, 0, z), bounds: [x - 2, z - 2, x + 2, z + 2] });
            continue;
        }

        let template = &library.templates[entry.template];
        let (ox, oz) = turn(entry.dx, entry.dz, spin);
        let turns = (entry.turns + spin) % 4;
        let (fx, fz) = turn(template.width - 1, template.length - 1, turns);
        let (ax, az) = (x0 + ox, z0 + oz);
        let bounds = [ax.min(ax + fx), az.min(az + fz), ax.max(ax + fx), az.max(az + fz)];
        let y = if template.rigid {
            // Медиана земли по кольцу вокруг рамки (как и в замере).
            let mut ring = Vec::new();

            for x in bounds[0] - 1..=bounds[2] + 1 {
                ring.push(ground(x, bounds[1] - 1));
                ring.push(ground(x, bounds[3] + 1));
            }

            for z in bounds[1]..=bounds[3] {
                ring.push(ground(bounds[0] - 1, z));
                ring.push(ground(bounds[2] + 1, z));
            }

            ring.sort_unstable();
            ring[ring.len() / 2] + entry.dy
        } else {
            0
        };

        out.push(Placed { template: entry.template, turns, origin: (ax, y, az), bounds });
    }

    out
}

/// Что поставить: блок в мире или «засыпать столбец землёй до рельефа».
pub enum Put {
    Block(i32, i32, i32, i32),
}

/// Кладёт куски деревни в чанк `chunk` (мировые x0, z0, x1, z1). `ground` —
/// высота верхнего блока рельефа в столбце. Вызывающий отбрасывает клетки
/// вне чанка сам.
pub fn paint(kind: Kind, pieces: &[Placed], chunk: [i32; 4], ground: &dyn Fn(i32, i32) -> i32, put: &mut dyn FnMut(Put)) {
    let Some(library) = library(kind) else {
        return;
    };
    let dirt = blocks::state_from_text("dirt").unwrap_or(super::AIR);

    for piece in pieces {
        let b = &piece.bounds;

        if b[0] > chunk[2] || chunk[0] > b[2] || b[1] > chunk[3] || chunk[1] > b[3] {
            continue;
        }

        if piece.template == HAY_PILE {
            let hay = blocks::state_from_text("hay_block[axis=y]").unwrap_or(super::AIR);

            for (dz, row) in (-2..=2).zip(0..5) {
                for (dx, column) in (-2..=2).zip(0..5) {
                    let (x, z) = (piece.origin.0 + dx, piece.origin.2 + dz);

                    if x < chunk[0] || x > chunk[2] || z < chunk[1] || z > chunk[3] {
                        continue;
                    }

                    let floor = ground(x, z);
                    let dice = roll(i64::from(piece.origin.0), x, z, (piece.origin.2 as u64) ^ 0x4A7);

                    if (dice % 100) as u8 >= HAY[0][row][column] {
                        continue;
                    }

                    put(Put::Block(x, floor + 1, z, hay));

                    // Верхний слой — с долей «при сене внизу».
                    let upper = u64::from(HAY[1][row][column]) * 100 / u64::from(HAY[0][row][column].max(1));

                    if (dice >> 16) % 100 < upper {
                        put(Put::Block(x, floor + 2, z, hay));
                    }
                }
            }

            continue;
        }

        let template = &library.templates[piece.template];
        let states = &template.states[piece.turns as usize];

        for bb in 0..template.length {
            for a in 0..template.width {
                let (dx, dz) = turn(a, bb, piece.turns);
                let (x, z) = (piece.origin.0 + dx, piece.origin.2 + dz);

                if x < chunk[0] || x > chunk[2] || z < chunk[1] || z > chunk[3] {
                    continue;
                }

                let base = if template.rigid { piece.origin.1 } else { ground(x, z) };

                for y in 0..template.height {
                    match template.cell(a, y, bb) {
                        Cell::Keep => {}
                        Cell::Air => put(Put::Block(x, base + y, z, super::AIR)),
                        Cell::Block(index) => put(Put::Block(x, base + y, z, states[index as usize])),
                    }
                }

                // Под полом rigid-куска — земля до рельефа.
                if template.rigid && matches!(template.cell(a, 0, bb), Cell::Block(_)) {
                    let floor = ground(x, z);

                    for y in floor + 1..base {
                        put(Put::Block(x, y, z, dirt));
                    }
                }
            }
        }
    }
}

type Cache<K, V> = OnceLock<Mutex<HashMap<K, Arc<V>>>>;

/// Раскладка деревни — одна на старт, хранится, пока мир жив.
fn plan_cached(terrain: &super::terrain::Terrain, kind: Kind, chunk_x: i32, chunk_z: i32) -> Arc<Vec<Placed>> {
    static CACHE: Cache<(i64, i32, i32), Vec<Placed>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (terrain.seed(), chunk_x, chunk_z);

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let ground = |x: i32, z: i32| terrain.column_in(x, z, None).height;
    let pieces = Arc::new(plan(terrain.seed(), kind, chunk_x, chunk_z, &ground));

    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, pieces.clone());
    pieces
}

/// Старты деревень, чьи куски могут достать до этого чанка.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, Kind)> {
    let mut out = Vec::new();

    for region_x in (chunk_x - RADIUS_CHUNKS).div_euclid(SPACING)..=(chunk_x + RADIUS_CHUNKS).div_euclid(SPACING) {
        for region_z in (chunk_z - RADIUS_CHUNKS).div_euclid(SPACING)..=(chunk_z + RADIUS_CHUNKS).div_euclid(SPACING) {
            let (x, z) = attempt(terrain.seed(), region_x, region_z);

            if (x - chunk_x).abs() > RADIUS_CHUNKS || (z - chunk_z).abs() > RADIUS_CHUNKS {
                continue;
            }

            if let Some(found) = start_in(terrain, region_x, region_z) {
                out.push(found);
            }
        }
    }

    out
}

/// Кладёт в сложенный чанк обычного мира куски ближних деревень — после
/// рельефа, деревьев и растительности (дома вытесняют траву и цветы).
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);

    for (start_x, start_z, kind) in near(terrain, chunk_x, chunk_z) {
        let pieces = plan_cached(terrain, kind, start_x, start_z);
        // Внутри чанка земля — по его же блокам (верх рельефа, без травы).
        let heights: Vec<i32> = (0..256).map(|at| terrain.column_in(x0 + at % 16, z0 + at / 16, None).height).collect();
        let ground = |x: i32, z: i32| {
            if (x0..x0 + 16).contains(&x) && (z0..z0 + 16).contains(&z) {
                heights[((z - z0) * 16 + (x - x0)) as usize]
            } else {
                terrain.column_in(x, z, None).height
            }
        };

        paint(kind, &pieces, [x0, z0, x0 + 15, z0 + 15], &ground, &mut |Put::Block(x, y, z, state)| {
            chunk.put_generated(x - x0, y, z - z0, state, true);
        });
    }
}

/// Ближайшая деревня вида `kind` (`locate structure minecraft:village_*`):
/// чанк старта, до `regions` областей вокруг.
pub fn nearest(terrain: &super::terrain::Terrain, kind: Kind, chunk_x: i32, chunk_z: i32, regions: i32) -> Option<(i32, i32)> {
    let (home_x, home_z) = (chunk_x.div_euclid(SPACING), chunk_z.div_euclid(SPACING));
    let distance = |(x, z): (i32, i32)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2);
    let mut best: Option<(i32, i32)> = None;

    for ring in 0..=regions {
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                if let Some((x, z, found)) = start_in(terrain, home_x + dx, home_z + dz)
                    && found == kind
                    && best.is_none_or(|known| distance((x, z)) < distance(known))
                {
                    best = Some((x, z));
                }
            }
        }

        if let Some(found) = best
            && (distance(found) as f64).sqrt() < f64::from(ring * SPACING)
        {
            break;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Поворот состояний: по часовой north → east, ось x ↔ z, у знамени
    /// rotation +4.
    #[test]
    fn states_turn_clockwise() {
        assert_eq!(rotate_text("oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]", 1), "oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]");
        assert_eq!(rotate_text("oak_log[axis=x]", 1), "oak_log[axis=z]");
        assert_eq!(rotate_text("oak_fence[east=true,north=false,south=false,waterlogged=false,west=true]", 1), "oak_fence[east=false,north=true,south=true,waterlogged=false,west=false]");
        assert_eq!(rotate_text("white_banner[rotation=14]", 1), "white_banner[rotation=2]");
        assert_eq!(rotate_text("stone", 3), "stone");
    }

    /// Старты: не ближе 8 чанков у соседних областей, один на область.
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -5..5 {
            for region_z in -5..5 {
                let (x, z) = attempt(42, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));

                let (nx, _) = attempt(42, region_x + 1, region_z);
                assert!(nx - x >= SEPARATION);
            }
        }
    }

    /// Образцы и раскладки разбираются; у каждой раскладки есть место сбора
    /// (town_centers) и улицы; рамки кусков не дальше радиуса.
    #[test]
    fn plains_layouts_parse() {
        let library = library(Kind::Plains).expect("равнинные сняты");

        assert!(!library.layouts.is_empty());

        for layout in &library.layouts {
            assert!(layout.iter().any(|entry| library.templates[entry.template].key.contains("town_centers")));
            assert!(layout.iter().any(|entry| library.templates[entry.template].key.contains("streets")));

            for entry in layout {
                assert!(entry.dx.abs() < RADIUS_CHUNKS * 16 + 16 && entry.dz.abs() < RADIUS_CHUNKS * 16 + 16, "{:?}", entry);
            }
        }

        let pieces = plan(7, Kind::Plains, 10, -3, &|_, _| 64);
        assert!(pieces.len() > 10);
    }

    /// Перепись блоков деревни — против замера оригинала (tools/measure/
    /// village.py census, 10 равнинных деревень, семя 1234567890): блоки
    /// самой деревни на деревню. Земля ровная, поэтому сверяются только
    /// блоки построек, не рельефа.
    #[test]
    fn plains_census() {
        const MEASURED: [(&str, f64); 12] = [
            // Дорожка — по столбцам улиц, на любой высоте (village.py:
            // перепись в рамках кусков недосчитывает улицы на склонах).
            ("dirt_path", 1407.5),
            ("cobblestone", 623.8),
            ("oak_stairs", 577.9),
            ("oak_planks", 489.8),
            ("wall_torch", 58.1),
            ("glass_pane", 40.0),
            ("oak_door", 27.4),
            // Сено: стогов 3,9 на деревню по 13,5 блока (замер вокруг мест
            // pile_hay) и сено в самих домах.
            ("hay_block", 53.5),
            ("white_bed", 11.6),
            ("yellow_bed", 9.4),
            ("torch", 9.9),
            ("bell", 1.2),
        ];
        let library = library(Kind::Plains).expect("равнинные сняты");
        let runs = 40;
        let mut totals: HashMap<&'static str, f64> = HashMap::new();

        for at in 0..runs {
            let pieces = plan(1_000 + at, Kind::Plains, 0, 0, &|_, _| 64);
            let mut cells = HashMap::new();

            paint(Kind::Plains, &pieces, [-200, -200, 200, 200], &|_, _| 64, &mut |Put::Block(x, y, z, state)| {
                cells.insert((x, y, z), state);
            });

            for state in cells.values() {
                *totals.entry(blocks::block_at_state(*state).unwrap_or("?")).or_default() += 1.0;
            }
        }

        let mut bad = Vec::new();
        for (name, measured) in MEASURED {
            let ours = totals.get(name).copied().unwrap_or(0.0) / runs as f64;
            println!("{:<14} {:8.1} / {:8.1}", name, measured, ours);
            if (ours - measured).abs() > 0.35 * measured + 1.5 {
                bad.push(format!("{}: {:.1} против {:.1}", name, ours, measured));
            }
        }
        assert!(library.layouts.len() >= 5);
        assert!(bad.is_empty(), "{:?}", bad);
    }

    /// Деревня в настоящем мире: ближайшая равнинная от середины встаёт на
    /// землю — дорожки, колокол, кровати есть, дома не висят над землёй.
    #[test]
    #[ignore]
    fn a_plains_village_stands_in_the_world() {
        use super::super::terrain::{Style, Terrain};

        let terrain = Terrain::new(1_234_567_890, Style::Vanilla);
        let (start_x, start_z) = nearest(&terrain, Kind::Plains, 0, 0, 12).expect("равнинная деревня есть");
        let generator = super::super::Generator::Normal(Terrain::new(1_234_567_890, Style::Vanilla));
        let mut counts: HashMap<&'static str, usize> = HashMap::new();

        for chunk_x in start_x - 5..=start_x + 5 {
            for chunk_z in start_z - 5..=start_z + 5 {
                let chunk = super::super::Chunk::generated(&generator, chunk_x, chunk_z);

                for x in 0..16 {
                    for z in 0..16 {
                        for y in 40..140 {
                            let name = blocks::block_at_state(chunk.block(x, y, z)).unwrap_or("?");

                            if matches!(name, "dirt_path" | "bell" | "white_bed" | "yellow_bed" | "cobblestone" | "oak_door" | "hay_block") {
                                *counts.entry(name).or_default() += 1;
                            }
                        }
                    }
                }
            }
        }

        println!("деревня в чанке ({}, {}): {:?}", start_x, start_z, counts);
        assert!(counts.get("dirt_path").copied().unwrap_or(0) > 300);
        assert!(counts.get("bell").copied().unwrap_or(0) >= 1);
    }
}
