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
// Рельеф: terrain_matching следует каждому столбцу исходной плотности.
// Rigid-дома привязаны к улице у входа; вокруг рамок structure_terrain
// добавляет плотность снизу и убирает её внутри (вики, Structure definition:
// «beard_thin»), со спадом наружу, без вертикальных земляных столбов.
// В снятых раскладках нет jigsaw-стыков: ближайшая улица к двери —
// приближение стыка [проверить], служебные куски сохраняют снятое смещение.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::blocks;

#[cfg(not(seedmap_core))]
#[path = "village_population.rs"]
mod population;
#[cfg(not(seedmap_core))]
pub(super) use population::populate;

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
    let dice = roll(seed, region_x, region_z, 10_387_312);
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
    static TAIGA: OnceLock<Library> = OnceLock::new();

    match kind {
        Kind::Plains => Some(PLAINS.get_or_init(|| parse(include_str!("village_plains.txt")))),
        Kind::Desert => Some(DESERT.get_or_init(|| parse(include_str!("village_desert.txt")))),
        Kind::Savanna => Some(SAVANNA.get_or_init(|| parse(include_str!("village_savanna.txt")))),
        Kind::Taiga => Some(TAIGA.get_or_init(|| parse(include_str!("village_taiga.txt")))),
        Kind::Snowy => None,
    }
}

// ---------------------------------------------------------------------------
// Постановка
// ---------------------------------------------------------------------------

/// Поставленный кусок: образец, поворот, начало в мире (y — низ у rigid),
/// рамка x0, z0, x1, z1.
#[derive(Clone, Debug, PartialEq, Eq)]
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
/// случаю, повёрнутая на случайную четверть; дома — от улицы у двери,
/// прочие rigid-куски — от земли `ground(x, z)` со снятым смещением.
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

    // Вики, Jigsaw structure: «moved vertically to align with the terrain».
    // Высота дороги в точке примыкания, а не медиана рамки дома.
    // Координаты стыков не сняты: ищем дорогу у нижней половины двери
    // [проверить]. Обход и разрешение равных расстояний фиксированы.
    let mut roads = Vec::new();
    for piece in &out {
        if piece.template == HAY_PILE { continue; }
        let template = &library.templates[piece.template];
        if template.rigid { continue; }
        for b in 0..template.length {
            for a in 0..template.width {
                for y in 0..template.height {
                    if let Cell::Block(index) = template.cell(a, y, b) {
                        let name = blocks::block_at_state(template.states[0][index as usize]).unwrap_or("");
                        if matches!(name, "dirt_path" | "smooth_sandstone") || name.ends_with("_planks") || name.ends_with("_stairs") {
                            let (dx, dz) = turn(a, b, piece.turns);
                            roads.push((piece.origin.0 + dx, piece.origin.2 + dz, y));
                            break;
                        }
                    }
                }
            }
        }
    }
    for piece in &mut out {
        if piece.template == HAY_PILE { continue; }
        let template = &library.templates[piece.template];
        if !template.rigid || !template.key.contains("/houses/") { continue; }
        let mut attachment = None;
        for y in 1..template.height {
            for b in 0..template.length {
                for a in 0..template.width {
                    let Cell::Block(index) = template.cell(a, y, b) else { continue };
                    let name = blocks::block_at_state(template.states[0][index as usize]).unwrap_or("");
                    if !name.ends_with("_door") { continue; }
                    // Верхняя половина той же двери не задаёт высоту пола.
                    if let Cell::Block(below) = template.cell(a, y - 1, b)
                        && blocks::block_at_state(template.states[0][below as usize]) == Some(name) { continue; }
                    let (dx, dz) = turn(a, b, piece.turns);
                    let (x, z) = (piece.origin.0 + dx, piece.origin.2 + dz);
                    for &(rx, rz, ry) in &roads {
                        let distance = (rx - x).pow(2) + (rz - z).pow(2);
                        let candidate = (distance, rx, rz, ry, y);
                        if attachment.is_none_or(|old| candidate < old) { attachment = Some(candidate); }
                    }
                }
            }
        }
        if let Some((_, x, z, road_y, door_y)) = attachment {
            piece.origin.1 = ground(x, z) + road_y - (door_y - 1);
        }
    }

    out
}

/// Блок образца в мировых координатах.
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
        // Пустые рамки появления мобов (desert/camel_spawn) не являются
        // домами: снятый в них воздух не должен вырезать пол места сбора.
        if !template.cells.iter().any(|cell| matches!(cell, Cell::Block(_))) { continue; }
        let states = &template.states[piece.turns as usize];

        for bb in 0..template.length {
            for a in 0..template.width {
                let (dx, dz) = turn(a, bb, piece.turns);
                let (x, z) = (piece.origin.0 + dx, piece.origin.2 + dz);

                if x < chunk[0] || x > chunk[2] || z < chunk[1] || z > chunk[3] {
                    continue;
                }

                let surface = ground(x, z);
                let base = if template.rigid { piece.origin.1 } else { surface };

                for y in 0..template.height {
                    match template.cell(a, y, bb) {
                        Cell::Keep => {}
                        // Воздух низкого куска не вырезает бороду под
                        // соседним высоким полом при пересечении рамок.
                        Cell::Air if template.rigid && base + y <= surface => {}
                        Cell::Air => put(Put::Block(x, base + y, z, super::AIR)),
                        Cell::Block(index) => put(Put::Block(x, base + y, z, states[index as usize])),
                    }
                }


            }
        }
    }
}

type Cache<K, V> = OnceLock<Mutex<HashMap<K, Arc<V>>>>;

/// Раскладка деревни — одна на старт, хранится, пока мир жив.
fn plan_cached(terrain: &super::terrain::Terrain, kind: Kind, chunk_x: i32, chunk_z: i32) -> Arc<Vec<Placed>> {
    static CACHE: Cache<(i64, bool, Kind, i32, i32), Vec<Placed>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (terrain.seed(), terrain.style() == super::terrain::Style::Vanilla, kind, chunk_x, chunk_z);

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let source = super::structure_terrain::Ground::new(terrain);
    let ground = |x: i32, z: i32| source.supported_height(x, z);
    let pieces = Arc::new(plan(terrain.seed(), kind, chunk_x, chunk_z, &ground));

    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, pieces.clone());
    pieces
}

/// Старты деревень, чьи куски могут достать до этого чанка.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, Kind)> {
    let mut out = Vec::new();
    let reach = RADIUS_CHUNKS + 1;

    for region_x in (chunk_x - reach).div_euclid(SPACING)..=(chunk_x + reach).div_euclid(SPACING) {
        for region_z in (chunk_z - reach).div_euclid(SPACING)..=(chunk_z + reach).div_euclid(SPACING) {
            let (x, z) = attempt(terrain.seed(), region_x, region_z);

            if (x - chunk_x).abs() > reach || (z - chunk_z).abs() > reach {
                continue;
            }

            if let Some(found) = start_in(terrain, region_x, region_z) {
                out.push(found);
            }
        }
    }

    out
}

#[cfg(not(seedmap_core))]
pub(super) fn beards_near(terrain: &super::terrain::Terrain, cx: i32, cz: i32) -> Vec<super::structure_terrain::Beard> {
    let mut out = Vec::new();
    for (sx, sz, kind) in near(terrain, cx, cz) {
        let library = library(kind).expect("библиотека старта");
        for piece in plan_cached(terrain, kind, sx, sz).iter() {
            if piece.template == HAY_PILE { continue; }
            let template = &library.templates[piece.template];
            if template.rigid && template.cells.iter().any(|cell| matches!(cell, Cell::Block(_))) {
                out.push(super::structure_terrain::Beard { bounds: piece.bounds, floor: piece.origin.1, ceiling: piece.origin.1 + template.height - 1 });
            }
        }
    }
    out
}

/// Кладёт в сложенный чанк обычного мира куски ближних деревень — после
/// рельефа, деревьев и растительности (дома вытесняют траву и цветы).
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);

    for (start_x, start_z, kind) in near(terrain, chunk_x, chunk_z) {
        let pieces = plan_cached(terrain, kind, start_x, start_z);
        let source = super::structure_terrain::Ground::new(terrain);
        let beards = super::structure_terrain::near(terrain, chunk_x, chunk_z);
        let ground = |x: i32, z: i32| super::structure_terrain::street_height(&source, x, z, &beards);

        paint(kind, &pieces, [x0, z0, x0 + 15, z0 + 15], &ground, &mut |Put::Block(x, y, z, state)| {
            chunk.put_generated(x - x0, y, z - z0, state, true);
        });
        population::chests(terrain.seed(),kind,&pieces,chunk,chunk_x,chunk_z);
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

#[cfg(all(test, not(seedmap_core)))]
#[path = "village_ground_tests.rs"]
mod ground_tests;

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    #[test]
    fn reported_village_streets_have_ground() {
        use super::super::{Chunk, Generator, terrain::{Style, Terrain}};
        let generator = Generator::Normal(Terrain::new(1_791_716_133_710_533_940, Style::Vanilla));
        let Generator::Normal(terrain)=&generator else {unreachable!()};
        let (sx,sz)=nearest(terrain,Kind::Plains,-11,5,12).unwrap();
        let mut streets = 0;
        for cx in sx-3..=sx+3 {
            for cz in sz-3..=sz+3 {
                let chunk = Chunk::generated(&generator, cx, cz);
                for x in 0..16 {
                    for z in 0..16 {
                        for y in 50..120 {
                            if blocks::block_at_state(chunk.block(x, y, z)) == Some("dirt_path") {
                                streets += 1;
                                let under = blocks::block_at_state(chunk.block(x, y - 1, z));
                                assert!(ground_tests::earth(chunk.block(x, y - 1, z)),
                                    "street ({}, {}, {}) above {:?}", cx * 16 + x, y, cz * 16 + z, under);
                            }
                        }
                    }
                }
            }
        }
        assert!(streets > 100, "streets: {streets}");
    }

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

// Офлайн-описание: те же принятые куски, без второго планировщика.
#[cfg(all(test, not(seedmap_core)))]
pub(super) fn audit_pieces(kind: Kind, pieces: &[Placed]) -> Vec<serde_json::Value> {
    let lib = library(kind).unwrap();
    pieces.iter().map(|p| {
        let (name, height, rigid) = if p.template == HAY_PILE { ("feature:pile_hay", 2, true) }
            else { let t = &lib.templates[p.template]; (t.key.as_str(), t.height, t.rigid) };
        serde_json::json!({"name":name,"rotation":p.turns,"box":[p.bounds[0],p.origin.1,p.bounds[1],p.bounds[2],p.origin.1+height-1,p.bounds[3]],"projection":if rigid {"rigid"} else {"terrain_matching"}})
    }).collect()
}

#[cfg(test)]
mod taiga_measurement_tests {
    use super::*;

    #[test]
    fn taiga_plan_paints_the_same_across_negative_chunk_borders() {
        for seed in 0..4 {
            let ground = |x: i32,z: i32| 70+(x+z).div_euclid(32);
            let pieces = plan(seed,Kind::Taiga,-1,-1,&ground);
            let mut whole = HashMap::new();
            let reach = RADIUS_CHUNKS*16;
            let bounds = [-16-reach,-16-reach,reach-1,reach-1];
            assert!(pieces.iter().all(|p| p.bounds[0]>=bounds[0] && p.bounds[1]>=bounds[1] && p.bounds[2]<=bounds[2] && p.bounds[3]<=bounds[3]));
            paint(Kind::Taiga,&pieces,bounds,&ground,&mut |Put::Block(x,y,z,state)| { whole.insert((x,y,z),state); });
            let mut tiled = HashMap::new();
            for cx in -1-RADIUS_CHUNKS..=-1+RADIUS_CHUNKS {
                for cz in -1-RADIUS_CHUNKS..=-1+RADIUS_CHUNKS {
                    paint(Kind::Taiga,&pieces,[cx*16,cz*16,cx*16+15,cz*16+15],&ground,&mut |Put::Block(x,y,z,state)| {
                        if x.div_euclid(16)==cx && z.div_euclid(16)==cz { tiled.insert((x,y,z),state); }
                    });
                }
            }
            assert_eq!(whole,tiled);
            assert!(whole.values().any(|&s| blocks::block_at_state(s)==Some("chest")));
        }
    }

    #[test]
    fn measured_taiga_layout_is_complete_and_rotates() {
        let lib = library(Kind::Taiga).unwrap();
        assert_eq!(lib.templates.len(),32);
        assert_eq!(lib.layouts.len(),1);
        assert!(!lib.layouts[0].is_empty());
        for t in &lib.templates { assert_eq!(t.cells.len(),(t.width*t.length*t.height) as usize); }
        let mut rotations = std::collections::HashSet::new();
        for seed in 0..32 {
            let pieces = plan(seed,Kind::Taiga,-5,7,&|_,_| 70);
            assert_eq!(pieces.len(),lib.layouts[0].len());
            rotations.insert(pieces[0].turns);
            assert!(pieces.iter().all(|p| p.bounds[0]<=p.bounds[2] && p.bounds[1]<=p.bounds[3]));
        }
        assert_eq!(rotations.len(),4);
        assert!(library(Kind::Snowy).is_none(),"без измерения снежная деревня не подменяется другой");
    }
}
